#![allow(
    clippy::type_complexity,
    clippy::approx_constant,
    clippy::needless_range_loop
)]
//! Differential test harness — proves the commuting square property
//! using the `ReferenceBackend` as the reference implementation.
//!
//! The commuting square contract:
//! ```text
//! project(snapshot r1) == apply(project(snapshot r0), deltas r0→r1)
//! ```
//!
//! This file covers:
//!
//! 1. **Single operation round-trip** — For each of the 16 `ModelOp` variants,
//!    verify that building from a snapshot produces the same state as applying
//!    the corresponding delta batch.
//!
//! 2. **Random mutation sequences** — Generate random legal operation sequences
//!    with a fixed seed, split into intervals, and verify the commuting square
//!    at each interval boundary.
//!
//! 3. **Multi-adapter cursor independence** — Two `ReferenceBackend` instances
//!    sharing a journal, with cursor A fully caught up and cursor B lagging;
//!    verify both produce correct states after independent catch-up.
//!
//! 4. **Fault injection framework** — A `FaultInjectingBackend` wrapper that
//!    fails at configurable operation indices. Tests cover `RecoverableFailure`
//!    (state unchanged), `DirtyFailure` (state partially mutated, recoverable
//!    via snapshot rebuild), and full rebuild recovery.
//!
//! 5. **Rebuild determinism** — Two backends rebuilt from the same snapshot
//!    produce identical normalized views.
//!
//! 6. **Semi-continuous partial-apply scenario** — Models adding an ordinary
//!    bound followed by a semi-continuous change where incremental apply is
//!    not possible. Verifies the delta batch is preserved in the journal,
//!    the full state after rebuild includes both changes, and no delta is lost.

use roml::delta::{DeltaBatch, ModelOp};
use roml::id::{ConId, Generation, ObjId, ParamId, VarId};
use roml::model::coefficient::CoefficientTarget;
use roml::model::{Bounds, ConstraintBounds, Sense, VarType};
use roml::revision::ModelRevision;
use roml::snapshot::{take_snapshot, ModelSnapshot};
use roml::solver::reference::{NormalizedView, ReferenceBackend};
use roml::solver::session::{BackendSession, Synchronization};
use roml::sync::{AdapterCursor, AdapterHealth, ApplyOutcome, SyncCoordinator};
use roml::value_expr::ValueExpr;
use std::collections::HashMap;

// ── ID helpers ───────────────────────────────────────────────────────────────

fn var_id(index: u32) -> VarId {
    VarId::new(index, Generation::new())
}

fn con_id(index: u32) -> ConId {
    ConId::new(index, Generation::new())
}

fn obj_id(index: u32) -> ObjId {
    ObjId::new(index, Generation::new())
}

fn param_id(index: u32) -> ParamId {
    ParamId::new(index, Generation::new())
}

/// Build a `ModelRevision` from a raw u64 by chaining `.next()`.
/// `ModelRevision::from_u64` is `pub(crate)` so we cannot call it
/// from integration tests; this helper fills the gap.
fn rev_from_u64(n: u64) -> ModelRevision {
    let mut r = ModelRevision::ZERO;
    for _ in 0..n {
        r = r.next().expect("revision counter overflow");
    }
    r
}

// ── Setup helpers ────────────────────────────────────────────────────────────

/// Create a fresh backend for testing.
fn fresh() -> ReferenceBackend {
    ReferenceBackend::new()
}

/// Build a `ModelSnapshot` from raw state vectors.
///
/// Convenience wrapper around `take_snapshot` that converts Vec inputs
/// into the required HashMap format.
fn make_snapshot(
    rev: ModelRevision,
    variables: Vec<(VarId, Bounds, VarType, bool, Option<f64>)>,
    constraints: Vec<(ConId, ConstraintBounds, bool)>,
    objectives: Vec<(ObjId, Sense, bool, f64)>,
    params: Vec<(ParamId, f64)>,
    cells: Vec<((CoefficientTarget, VarId), ValueExpr, f64, Vec<ParamId>)>,
) -> ModelSnapshot {
    let vars: HashMap<VarId, (Bounds, VarType, bool, Option<f64>)> = variables
        .into_iter()
        .map(|(id, b, vt, a, s)| (id, (b, vt, a, s)))
        .collect();
    let cons: HashMap<ConId, (ConstraintBounds, bool)> = constraints
        .into_iter()
        .map(|(id, b, a)| (id, (b, a)))
        .collect();
    let objs: HashMap<ObjId, (Sense, bool, f64)> = objectives
        .into_iter()
        .map(|(id, s, a, c)| (id, (s, a, c)))
        .collect();
    let params_map: HashMap<ParamId, f64> = params.into_iter().collect();
    take_snapshot(rev, &vars, &cons, &objs, &params_map, &cells)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Section 4: Fault injection framework
// ═══════════════════════════════════════════════════════════════════════════════
//
// A wrapper around ReferenceBackend that can be configured to inject faults
// at specific global operation indices.  Used to verify the recovery semantics
// of RecoverableFailure (state unchanged) and DirtyFailure (state partially
// mutated, recovery via rebuild).

/// Outcome configuration for a fault injection point.
#[derive(Clone, Debug)]
enum FaultOutcome {
    /// RecoverableFailure: operation fails, state is unchanged.
    Recoverable(String),
    /// DirtyFailure: operation partially mutates, needs rebuild.
    Dirty(String),
}

/// Configuration for a single fault injection point.
#[derive(Clone, Debug)]
struct FaultConfig {
    /// Global operation index to fail at (0-based).
    fail_at_op: usize,
    /// The outcome to return when the fault triggers.
    outcome: FaultOutcome,
}

/// A wrapper around `ReferenceBackend` that can inject failures at
/// configurable global operation indices.
struct FaultInjectingBackend {
    inner: ReferenceBackend,
    /// Counter tracking total operations applied across all batches.
    global_op_count: usize,
    /// Optional fault configuration.  When `Some`, the next batch that
    /// reaches the configured operation index will trigger the fault.
    fault: Option<FaultConfig>,
}

impl FaultInjectingBackend {
    fn new() -> Self {
        Self {
            inner: ReferenceBackend::new(),
            global_op_count: 0,
            fault: None,
        }
    }

    /// Configure a fault to fire at the given global operation index.
    fn configure_fault(&mut self, fail_at_op: usize, outcome: FaultOutcome) {
        self.fault = Some(FaultConfig {
            fail_at_op,
            outcome,
        });
    }

    /// Clear any previously configured fault.
    #[allow(dead_code)]
    fn clear_fault(&mut self) {
        self.fault = None;
    }

    /// Apply a batch, potentially injecting a fault at the configured index.
    ///
    /// For `RecoverableFailure`: the failing operation is NOT applied,
    /// the cursor is NOT advanced, and the backend state is unchanged
    /// from before this batch was attempted.
    ///
    /// For `DirtyFailure`: operations BEFORE the failing index are
    /// applied, then the failing operation IS applied (creating partial/
    /// dirty state), and `DirtyFailure` is returned.  The cursor is NOT
    /// advanced.
    fn apply_batch(
        &mut self,
        batch: &DeltaBatch,
        cursor: &mut AdapterCursor,
    ) -> Result<ApplyOutcome, String> {
        if batch.from != cursor.applied_revision {
            return Ok(ApplyOutcome::RecoverableFailure {
                reason: format!(
                    "batch from {} != cursor at {}",
                    batch.from, cursor.applied_revision
                ),
            });
        }

        for op in &batch.operations {
            if let Some(ref config) = self.fault {
                if self.global_op_count == config.fail_at_op {
                    match &config.outcome {
                        FaultOutcome::Recoverable(reason) => {
                            // Operation NOT applied; state unchanged.
                            return Ok(ApplyOutcome::RecoverableFailure {
                                reason: reason.clone(),
                            });
                        }
                        FaultOutcome::Dirty(reason) => {
                            // Apply this operation to create dirty/partial state.
                            self.inner.apply_op(op)?;
                            self.global_op_count += 1;
                            // Return DirtyFailure — cursor is NOT advanced.
                            return Ok(ApplyOutcome::DirtyFailure {
                                reason: reason.clone(),
                            });
                        }
                    }
                }
            }

            self.inner.apply_op(op)?;
            self.global_op_count += 1;
        }

        cursor.advance(batch).map_err(|e| e.to_string())?;
        self.inner.revision = cursor.applied_revision;
        Ok(ApplyOutcome::Applied {
            new_revision: cursor.applied_revision,
        })
    }

    fn rebuild(&mut self, snapshot: &ModelSnapshot, cursor: &mut AdapterCursor) {
        self.inner.rebuild(snapshot, cursor);
    }

    fn normalized_view(&self) -> NormalizedView {
        self.inner.normalized_view()
    }
}

// ── Fault injection: RecoverableFailure ─────────────────────────────────────

#[test]
fn dx_fault_injection_recoverable_failure() {
    // Set up a backend with one variable, then inject a fault that
    // produces RecoverableFailure.  Verify the state is unchanged.

    let v = var_id(0);
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let mut backend = FaultInjectingBackend::new();
    let mut cursor = AdapterCursor::new();

    // Apply batch 0→1: add a variable
    let init_batch = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::AddVariable {
            var: v,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        }],
    )
    .unwrap();
    let outcome = backend.apply_batch(&init_batch, &mut cursor).unwrap();
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));
    assert_eq!(cursor.applied_revision, r1);

    // Snapshot the state before the fault
    let view_before = backend.normalized_view();
    assert_eq!(view_before.variables.len(), 1);

    // Configure fault at global op 1 (first op of next batch)
    backend.configure_fault(1, FaultOutcome::Recoverable("simulated recoverable".into()));

    // Attempt to apply batch 1→2
    let fault_batch = DeltaBatch::new(
        r1,
        r2,
        vec![ModelOp::SetVariableBounds {
            var: v,
            bounds: Bounds::new(0.0, 50.0),
        }],
    )
    .unwrap();
    let outcome = backend.apply_batch(&fault_batch, &mut cursor).unwrap();

    // Must be RecoverableFailure with state unchanged
    assert!(
        matches!(outcome, ApplyOutcome::RecoverableFailure { .. }),
        "expected RecoverableFailure, got {outcome:?}"
    );

    let view_after = backend.normalized_view();
    assert_eq!(
        view_before, view_after,
        "backend state must be unchanged after RecoverableFailure"
    );
    assert_eq!(
        cursor.applied_revision, r1,
        "cursor must NOT advance after RecoverableFailure"
    );
    assert!(
        cursor.is_ready(),
        "cursor health should remain Ready on RecoverableFailure"
    );
}

// ── Fault injection: DirtyFailure ───────────────────────────────────────────

#[test]
fn dx_fault_injection_dirty_failure() {
    // Apply a batch with multiple ops; inject a DirtyFailure in the
    // middle.  Verify ops before the fault were applied, the cursor
    // was NOT advanced, and the state is recoverable via rebuild.

    let v = var_id(0);
    let c = con_id(0);
    let p = param_id(0);
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let mut backend = FaultInjectingBackend::new();
    let mut cursor = AdapterCursor::new();

    // Apply batch 0→1: set up a parameter for later use
    let init_batch = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::SetParameter {
            param: p,
            value: 5.0,
        }],
    )
    .unwrap();
    backend.apply_batch(&init_batch, &mut cursor).unwrap();
    assert_eq!(cursor.applied_revision, r1);

    // Configure fault at global op 2 (second op in the next batch)
    backend.configure_fault(2, FaultOutcome::Dirty("simulated dirty".into()));

    // Batch 1→2: three ops; fault fires on the second (index 1 within batch)
    let fault_batch = DeltaBatch::new(
        r1,
        r2,
        vec![
            ModelOp::AddVariable {
                var: v,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            },
            ModelOp::AddConstraint {
                con: c,
                bounds: ConstraintBounds::le(100.0),
            },
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c), v),
                value_expr: ValueExpr::constant(10.0),
                evaluated_value: 10.0,
            },
        ],
    )
    .unwrap();
    let outcome = backend.apply_batch(&fault_batch, &mut cursor).unwrap();

    assert!(
        matches!(outcome, ApplyOutcome::DirtyFailure { .. }),
        "expected DirtyFailure, got {outcome:?}"
    );

    // Cursor must NOT advance
    assert_eq!(
        cursor.applied_revision, r1,
        "cursor must NOT advance after DirtyFailure"
    );

    // Variable was applied (op 0 in batch, global op 1), constraint was
    // applied (op 1 in batch, global op 2), but SetCell was NOT.
    let dirty_view = backend.normalized_view();
    assert_eq!(
        dirty_view.variables.len(),
        1,
        "variable should have been applied"
    );
    assert_eq!(
        dirty_view.constraints.len(),
        1,
        "constraint should have been applied"
    );
    assert_eq!(
        dirty_view.cells.len(),
        0,
        "cell should NOT have been applied (fault triggered before it)"
    );
}

// ── Fault injection: Rebuild recovery after DirtyFailure ────────────────────

#[test]
fn dx_fault_injection_rebuild_recovery() {
    // After a DirtyFailure, rebuild from a snapshot of the intended state.
    // Verify that the rebuilt state matches a direct snapshot projection.

    let v = var_id(0);
    let c = con_id(0);
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let mut backend = FaultInjectingBackend::new();
    let mut cursor = AdapterCursor::new();

    // Apply a clean batch to r1
    let clean_batch = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddVariable {
                var: v,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            },
            ModelOp::AddConstraint {
                con: c,
                bounds: ConstraintBounds::le(100.0),
            },
        ],
    )
    .unwrap();
    backend.apply_batch(&clean_batch, &mut cursor).unwrap();
    assert_eq!(cursor.applied_revision, r1);

    // Configure fault at global op 2 (first op of next batch)
    backend.configure_fault(2, FaultOutcome::Dirty("simulated dirty".into()));

    // Attempt dirty batch r1→r2
    let dirty_batch = DeltaBatch::new(
        r1,
        r2,
        vec![ModelOp::SetCell {
            cell_key: (CoefficientTarget::Constraint(c), v),
            value_expr: ValueExpr::constant(7.5),
            evaluated_value: 7.5,
        }],
    )
    .unwrap();
    let outcome = backend.apply_batch(&dirty_batch, &mut cursor).unwrap();
    assert!(matches!(outcome, ApplyOutcome::DirtyFailure { .. }));

    // Build the intended snapshot at r2: variable + constraint + cell
    let snap_r2 = make_snapshot(
        r2,
        vec![(v, Bounds::NON_NEGATIVE, VarType::Continuous, true, None)],
        vec![(c, ConstraintBounds::le(100.0), true)],
        vec![],
        vec![],
        vec![(
            (CoefficientTarget::Constraint(c), v),
            ValueExpr::constant(7.5),
            7.5,
            vec![],
        )],
    );

    // Rebuild from snapshot — this is the recovery path
    backend.rebuild(&snap_r2, &mut cursor);
    assert!(
        cursor.is_ready(),
        "cursor should be Ready after rebuild recovery"
    );
    assert_eq!(
        cursor.applied_revision, r2,
        "cursor should be at r2 after rebuild"
    );

    // Rebuilt state should match: rebuilding directly from the snapshot
    let mut direct_backend = fresh();
    let mut direct_cursor = AdapterCursor::new();
    direct_backend.rebuild(&snap_r2, &mut direct_cursor);
    assert_eq!(
        backend.normalized_view(),
        direct_backend.normalized_view(),
        "rebuilt state after DirtyFailure must match direct snapshot projection"
    );

    // Verify the rebuilt state has the full intended content
    let view = backend.normalized_view();
    assert_eq!(view.variables.len(), 1);
    assert_eq!(view.constraints.len(), 1);
    assert_eq!(view.cells.len(), 1);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Section 1–3 and 5–6 will be inserted in Tasks 2 and 3
// ═══════════════════════════════════════════════════════════════════════════════
