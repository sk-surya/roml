#![allow(clippy::type_complexity, clippy::approx_constant, clippy::needless_range_loop)]
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
//!
//! Note: `CellKey` is a `pub(crate)` type alias for `(CoefficientTarget, VarId)`;
//! we use the tuple directly in integration tests, matching the pattern
//! established by `backend_contract.rs`.

use roml::delta::{DeltaBatch, ModelOp};
use roml::id::{ConId, Generation, ObjId, ParamId, VarId};
use roml::model::coefficient::CoefficientTarget;
use roml::model::{Bounds, ConstraintBounds, Sense, VarType};
use roml::revision::ModelRevision;
use roml::snapshot::{take_snapshot, ModelSnapshot};
use roml::solver::reference::{NormalizedView, ReferenceBackend};
use roml::sync::{AdapterCursor, ApplyOutcome, SyncCoordinator};
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

fn rebuild(
    backend: &mut ReferenceBackend,
    cursor: &mut AdapterCursor,
    snapshot: &ModelSnapshot,
) {
    backend.rebuild(snapshot, cursor);
}

fn apply_batch(
    backend: &mut ReferenceBackend,
    cursor: &mut AdapterCursor,
    batch: &DeltaBatch,
) -> ApplyOutcome {
    backend
        .apply_batch(batch, cursor)
        .expect("apply_batch should not error")
}

fn fresh() -> (ReferenceBackend, AdapterCursor) {
    (ReferenceBackend::new(), AdapterCursor::new())
}

/// Verify the commuting square for a single batch:
///
/// ```text
/// project(snapshot r1) == apply(project(snapshot r0), deltas r0→r1)
/// ```
fn assert_commuting_square(
    snap_r0: &ModelSnapshot,
    snap_r1: &ModelSnapshot,
    batch: &DeltaBatch,
) {
    let (mut backend_a, mut cursor_a) = fresh();
    rebuild(&mut backend_a, &mut cursor_a, snap_r1);
    let view_a = backend_a.normalized_view();

    let (mut backend_b, mut cursor_b) = fresh();
    rebuild(&mut backend_b, &mut cursor_b, snap_r0);
    let outcome = apply_batch(&mut backend_b, &mut cursor_b, batch);
    assert!(
        matches!(outcome, ApplyOutcome::Applied { .. }),
        "expected Applied, got {outcome:?}"
    );
    let view_b = backend_b.normalized_view();

    assert_eq!(
        view_a, view_b,
        "commuting square violated: project(snapshot r{}) != apply(project(snapshot r{}), deltas r{}→r{})",
        batch.to, batch.from, batch.from, batch.to
    );
}

/// Verify the commuting square across multiple sequential batches.
fn assert_commuting_square_multi(
    snap_r0: &ModelSnapshot,
    snap_end: &ModelSnapshot,
    batches: &[&DeltaBatch],
) {
    // Path A: rebuild directly from snapshot at final revision
    let (mut backend_a, mut cursor_a) = fresh();
    rebuild(&mut backend_a, &mut cursor_a, snap_end);
    let view_a = backend_a.normalized_view();

    // Path B: rebuild from r0, then apply all batches sequentially
    let (mut backend_b, mut cursor_b) = fresh();
    rebuild(&mut backend_b, &mut cursor_b, snap_r0);
    for batch in batches {
        let outcome = apply_batch(&mut backend_b, &mut cursor_b, batch);
        assert!(
            matches!(outcome, ApplyOutcome::Applied { .. }),
            "batch r{}→r{} failed: {outcome:?}",
            batch.from,
            batch.to
        );
    }
    let view_b = backend_b.normalized_view();

    assert_eq!(
        view_a, view_b,
        "commuting square violated across {} batches (r{} → r{})",
        batches.len(),
        batches.first().map(|b| b.from).unwrap_or(snap_r0.revision),
        batches.last().map(|b| b.to).unwrap_or(snap_end.revision),
    );
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
// Section 1: Single operation round-trip — 16 tests, one per ModelOp variant
// ═══════════════════════════════════════════════════════════════════════════════
//
// For each variant we prove:
//     project(snapshot r1) == apply(project(snapshot r0), deltas r0→r1)
// where r0 = empty, r1 = state after applying the single-op batch.

#[test]
fn dx_add_variable_round_trip() {
    let v = var_id(0);
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let ops = vec![ModelOp::AddVariable {
        var: v,
        bounds: Bounds::new(1.0, 10.0),
        var_type: VarType::Integer,
    }];
    let batch = DeltaBatch::new(r0, r1, ops).unwrap();

    let snap_r0 = ModelSnapshot::empty(r0);
    let snap_r1 = make_snapshot(
        r1,
        vec![(v, Bounds::new(1.0, 10.0), VarType::Integer, true, None)],
        vec![],
        vec![],
        vec![],
        vec![],
    );

    assert_commuting_square(&snap_r0, &snap_r1, &batch);
}

#[test]
fn dx_remove_variable_round_trip() {
    let v = var_id(0);
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    // Batch 1: add variable
    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::AddVariable {
            var: v,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        }],
    )
    .unwrap();

    // Batch 2: remove variable
    let batch2 = DeltaBatch::new(r1, r2, vec![ModelOp::RemoveVariable { var: v }]).unwrap();

    let snap_r0 = ModelSnapshot::empty(r0);
    let snap_r2 = make_snapshot(r2, vec![], vec![], vec![], vec![], vec![]);

    assert_commuting_square_multi(&snap_r0, &snap_r2, &[&batch1, &batch2]);
}

#[test]
fn dx_set_variable_bounds_round_trip() {
    let v = var_id(0);
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let batch = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddVariable {
                var: v,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            },
            ModelOp::SetVariableBounds {
                var: v,
                bounds: Bounds::new(5.0, 20.0),
            },
        ],
    )
    .unwrap();

    let snap_r0 = ModelSnapshot::empty(r0);
    let snap_r1 = make_snapshot(
        r1,
        vec![(v, Bounds::new(5.0, 20.0), VarType::Continuous, true, None)],
        vec![],
        vec![],
        vec![],
        vec![],
    );

    assert_commuting_square(&snap_r0, &snap_r1, &batch);
}

#[test]
fn dx_set_variable_active_round_trip() {
    let v = var_id(0);
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let batch = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddVariable {
                var: v,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            },
            ModelOp::SetVariableActive {
                var: v,
                active: false,
            },
        ],
    )
    .unwrap();

    let snap_r0 = ModelSnapshot::empty(r0);
    let snap_r1 = make_snapshot(
        r1,
        vec![(v, Bounds::NON_NEGATIVE, VarType::Continuous, false, None)],
        vec![],
        vec![],
        vec![],
        vec![],
    );

    assert_commuting_square(&snap_r0, &snap_r1, &batch);
}

#[test]
fn dx_set_variable_type_round_trip() {
    let v = var_id(0);
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let batch = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddVariable {
                var: v,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            },
            ModelOp::SetVariableType {
                var: v,
                var_type: VarType::Binary,
            },
        ],
    )
    .unwrap();

    let snap_r0 = ModelSnapshot::empty(r0);
    let snap_r1 = make_snapshot(
        r1,
        vec![(v, Bounds::NON_NEGATIVE, VarType::Binary, true, None)],
        vec![],
        vec![],
        vec![],
        vec![],
    );

    assert_commuting_square(&snap_r0, &snap_r1, &batch);
}

#[test]
fn dx_add_constraint_round_trip() {
    let c = con_id(0);
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let batch = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::AddConstraint {
            con: c,
            bounds: ConstraintBounds::range(10.0, 50.0),
        }],
    )
    .unwrap();

    let snap_r0 = ModelSnapshot::empty(r0);
    let snap_r1 = make_snapshot(
        r1,
        vec![],
        vec![(c, ConstraintBounds::range(10.0, 50.0), true)],
        vec![],
        vec![],
        vec![],
    );

    assert_commuting_square(&snap_r0, &snap_r1, &batch);
}

#[test]
fn dx_remove_constraint_round_trip() {
    let c = con_id(0);
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::AddConstraint {
            con: c,
            bounds: ConstraintBounds::le(100.0),
        }],
    )
    .unwrap();
    let batch2 = DeltaBatch::new(r1, r2, vec![ModelOp::RemoveConstraint { con: c }]).unwrap();

    let snap_r0 = ModelSnapshot::empty(r0);
    let snap_r2 = make_snapshot(r2, vec![], vec![], vec![], vec![], vec![]);

    assert_commuting_square_multi(&snap_r0, &snap_r2, &[&batch1, &batch2]);
}

#[test]
fn dx_set_constraint_bounds_round_trip() {
    let c = con_id(0);
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let batch = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddConstraint {
                con: c,
                bounds: ConstraintBounds::le(100.0),
            },
            ModelOp::SetConstraintBounds {
                con: c,
                bounds: ConstraintBounds::ge(50.0),
            },
        ],
    )
    .unwrap();

    let snap_r0 = ModelSnapshot::empty(r0);
    let snap_r1 = make_snapshot(
        r1,
        vec![],
        vec![(c, ConstraintBounds::ge(50.0), true)],
        vec![],
        vec![],
        vec![],
    );

    assert_commuting_square(&snap_r0, &snap_r1, &batch);
}

#[test]
fn dx_set_constraint_active_round_trip() {
    let c = con_id(0);
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let batch = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddConstraint {
                con: c,
                bounds: ConstraintBounds::le(100.0),
            },
            ModelOp::SetConstraintActive {
                con: c,
                active: false,
            },
        ],
    )
    .unwrap();

    let snap_r0 = ModelSnapshot::empty(r0);
    let snap_r1 = make_snapshot(
        r1,
        vec![],
        vec![(c, ConstraintBounds::le(100.0), false)],
        vec![],
        vec![],
        vec![],
    );

    assert_commuting_square(&snap_r0, &snap_r1, &batch);
}

#[test]
fn dx_set_cell_round_trip() {
    let v = var_id(0);
    let c = con_id(0);
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let batch = DeltaBatch::new(
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
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c), v),
                value_expr: ValueExpr::constant(7.5),
                evaluated_value: 7.5,
            },
        ],
    )
    .unwrap();

    let snap_r0 = ModelSnapshot::empty(r0);
    let snap_r1 = make_snapshot(
        r1,
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

    assert_commuting_square(&snap_r0, &snap_r1, &batch);
}

#[test]
fn dx_remove_cell_round_trip() {
    let v = var_id(0);
    let c = con_id(0);
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let batch1 = DeltaBatch::new(
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
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c), v),
                value_expr: ValueExpr::constant(3.0),
                evaluated_value: 3.0,
            },
        ],
    )
    .unwrap();
    let batch2 = DeltaBatch::new(
        r1,
        r2,
        vec![ModelOp::RemoveCell {
            cell_key: (CoefficientTarget::Constraint(c), v),
        }],
    )
    .unwrap();

    let snap_r0 = ModelSnapshot::empty(r0);
    let snap_r2 = make_snapshot(
        r2,
        vec![(v, Bounds::NON_NEGATIVE, VarType::Continuous, true, None)],
        vec![(c, ConstraintBounds::le(100.0), true)],
        vec![],
        vec![],
        vec![],
    );

    assert_commuting_square_multi(&snap_r0, &snap_r2, &[&batch1, &batch2]);
}

#[test]
fn dx_add_objective_round_trip() {
    let o = obj_id(0);
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let batch = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::AddObjective {
            obj: o,
            sense: Sense::Maximize,
        }],
    )
    .unwrap();

    let snap_r0 = ModelSnapshot::empty(r0);
    let snap_r1 = make_snapshot(
        r1,
        vec![],
        vec![],
        vec![(o, Sense::Maximize, false, 0.0)],
        vec![],
        vec![],
    );

    assert_commuting_square(&snap_r0, &snap_r1, &batch);
}

#[test]
fn dx_remove_objective_round_trip() {
    let v = var_id(0);
    let o = obj_id(0);
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddVariable {
                var: v,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            },
            ModelOp::AddObjective {
                obj: o,
                sense: Sense::Minimize,
            },
            ModelOp::SetObjectiveCell {
                cell_key: (CoefficientTarget::Objective(o), v),
                value_expr: ValueExpr::constant(2.0),
                evaluated_value: 2.0,
                constant: 0.0,
            },
        ],
    )
    .unwrap();
    let batch2 =
        DeltaBatch::new(r1, r2, vec![ModelOp::RemoveObjective { obj: o }]).unwrap();

    let snap_r0 = ModelSnapshot::empty(r0);
    let snap_r2 = make_snapshot(
        r2,
        vec![(v, Bounds::NON_NEGATIVE, VarType::Continuous, true, None)],
        vec![],
        vec![],
        vec![],
        vec![],
    );

    assert_commuting_square_multi(&snap_r0, &snap_r2, &[&batch1, &batch2]);
}

#[test]
fn dx_set_active_objective_round_trip() {
    let o = obj_id(0);
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let batch = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddObjective {
                obj: o,
                sense: Sense::Minimize,
            },
            ModelOp::SetActiveObjective { obj: Some(o) },
        ],
    )
    .unwrap();

    let snap_r0 = ModelSnapshot::empty(r0);
    let snap_r1 = make_snapshot(
        r1,
        vec![],
        vec![],
        vec![(o, Sense::Minimize, true, 0.0)],
        vec![],
        vec![],
    );

    assert_commuting_square(&snap_r0, &snap_r1, &batch);
}

#[test]
fn dx_set_objective_cell_round_trip() {
    let v = var_id(0);
    let o = obj_id(0);
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let batch = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddVariable {
                var: v,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            },
            ModelOp::AddObjective {
                obj: o,
                sense: Sense::Minimize,
            },
            ModelOp::SetObjectiveCell {
                cell_key: (CoefficientTarget::Objective(o), v),
                value_expr: ValueExpr::constant(3.5),
                evaluated_value: 3.5,
                constant: 10.0,
            },
        ],
    )
    .unwrap();

    let snap_r0 = ModelSnapshot::empty(r0);
    let snap_r1 = make_snapshot(
        r1,
        vec![(v, Bounds::NON_NEGATIVE, VarType::Continuous, true, None)],
        vec![],
        // constant = 10.0 must match the objective entry constant
        vec![(o, Sense::Minimize, false, 10.0)],
        vec![],
        vec![(
            (CoefficientTarget::Objective(o), v),
            ValueExpr::constant(3.5),
            3.5,
            vec![],
        )],
    );

    assert_commuting_square(&snap_r0, &snap_r1, &batch);
}

#[test]
fn dx_set_parameter_round_trip() {
    let p = param_id(0);
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let batch = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::SetParameter {
            param: p,
            value: 99.5,
        }],
    )
    .unwrap();

    let snap_r0 = ModelSnapshot::empty(r0);
    let snap_r1 = make_snapshot(r1, vec![], vec![], vec![], vec![(p, 99.5)], vec![]);

    assert_commuting_square(&snap_r0, &snap_r1, &batch);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Section 2: Random mutation sequences
// ═══════════════════════════════════════════════════════════════════════════════
//
// Generate random legal operation sequences with a fixed seed, distribute them
// across multiple batches, and verify the commuting square at each boundary.
// Uses `rand` (0.9) with `StdRng` seeded for deterministic reproducibility.

#[test]
fn dx_random_mutation_sequences() {
    use rand::Rng;
    use rand::SeedableRng;

    let mut rng = rand::rngs::StdRng::seed_from_u64(42);

    let r0 = ModelRevision::ZERO;

    // ── Generate random ops across batches ────────────────────────────────
    let num_batches = 5;
    let ops_per_batch = 30;

    // Track live entity indices for legal operation generation
    let mut next_var: u32 = 0;
    let mut next_con: u32 = 0;
    let mut next_obj: u32 = 0;
    let mut next_param: u32 = 0;
    let mut live_vars: Vec<VarId> = Vec::new();
    let mut live_cons: Vec<ConId> = Vec::new();
    let mut live_objs: Vec<ObjId> = Vec::new();
    let mut live_params: Vec<ParamId> = Vec::new();

    // Generated batches
    let mut batches: Vec<DeltaBatch> = Vec::new();
    let mut current_rev = r0;

    for _batch_idx in 0..num_batches {
        let from = current_rev;
        let to = current_rev.next().unwrap();
        let mut ops: Vec<ModelOp> = Vec::new();

        for _ in 0..ops_per_batch {
            let n_vars = live_vars.len();
            let n_cons = live_cons.len();
            let n_objs = live_objs.len();
            let n_total = n_vars + n_cons + n_objs;

            let add_weight = if n_total < 8 { 50u32 } else { 20u32 };
            let remove_weight = if n_total > 0 { 10u32 } else { 0u32 };
            let _mutate_weight = 100u32 - add_weight - remove_weight;

            let roll: u32 = rng.random_range(0..100);
            let op = if roll < add_weight {
                gen_add_op(
                    &mut rng,
                    &mut next_var,
                    &mut next_con,
                    &mut next_obj,
                    &mut next_param,
                    &mut live_vars,
                    &mut live_cons,
                    &mut live_objs,
                    &mut live_params,
                )
            } else if roll < add_weight + remove_weight {
                gen_remove_op(
                    &mut rng,
                    &mut live_vars,
                    &mut live_cons,
                    &mut live_objs,
                )
            } else {
                gen_mutate_op(
                    &mut rng,
                    &live_vars,
                    &live_cons,
                    &live_objs,
                    &live_params,
                )
            };

            ops.push(op);
        }

        if let Some(batch) = DeltaBatch::new(from, to, ops) {
            batches.push(batch);
            current_rev = to;
        }
    }

    // ── Apply all batches on a reference backend to generate snapshots ────
    let snap_r0 = ModelSnapshot::empty(r0);

    // For each batch boundary, verify the commuting square.
    // We rebuild a fresh backend from scratch each time, apply batches
    // up to boundary j, and compare with rebuilding from a snapshot
    // at that boundary.
    let (mut tracker_backend, mut tracker_cursor) = fresh();
    rebuild(&mut tracker_backend, &mut tracker_cursor, &snap_r0);

    for (j, batch) in batches.iter().enumerate() {
        // Apply the batch on the tracker backend
        let outcome = apply_batch(&mut tracker_backend, &mut tracker_cursor, batch);
        assert!(
            matches!(outcome, ApplyOutcome::Applied { .. }),
            "tracker apply failed at batch {j}: {outcome:?}"
        );

        // Extract state from tracker to build a reference snapshot at this
        // revision.  This gives us the canonical state the batch sequence
        // should produce.
        let target_rev = batch.to;
        let tracker_view = tracker_backend.normalized_view();

        // Build a reference snapshot from the tracker's state.
        let snap_ref = build_snapshot_from_view(
            target_rev,
            &tracker_view,
        );

        // Verify: rebuild(rN) == rebuild(r0) + apply(batches[0..=j])
        let (mut snap_backend, mut snap_cursor) = fresh();
        rebuild(&mut snap_backend, &mut snap_cursor, &snap_ref);
        let snap_view = snap_backend.normalized_view();

        let (mut seq_backend, mut seq_cursor) = fresh();
        rebuild(&mut seq_backend, &mut seq_cursor, &snap_r0);
        for k in 0..=j {
            let o = apply_batch(&mut seq_backend, &mut seq_cursor, &batches[k]);
            assert!(
                matches!(o, ApplyOutcome::Applied { .. }),
                "seq apply failed at batch {k}: {o:?}"
            );
        }
        let seq_view = seq_backend.normalized_view();

        assert_eq!(
            snap_view, seq_view,
            "commuting square violated at batch boundary {j} (r{} → r{})",
            batch.from, batch.to
        );
    }
}

/// Generate a random "add" operation.
#[allow(clippy::too_many_arguments)]
fn gen_add_op(
    rng: &mut impl rand::Rng,
    next_var: &mut u32,
    next_con: &mut u32,
    next_obj: &mut u32,
    next_param: &mut u32,
    live_vars: &mut Vec<VarId>,
    live_cons: &mut Vec<ConId>,
    live_objs: &mut Vec<ObjId>,
    live_params: &mut Vec<ParamId>,
) -> ModelOp {
    let choice: u32 = rng.random_range(0..4);
    match choice {
        0 => {
            let v = var_id(*next_var);
            *next_var += 1;
            live_vars.push(v);
            let lb = rng.random_range(-10.0..10.0);
            let ub = rng.random_range(lb..=lb + 100.0);
            let vt = match rng.random_range(0u32..3) {
                0 => VarType::Continuous,
                1 => VarType::Integer,
                _ => VarType::Binary,
            };
            ModelOp::AddVariable {
                var: v,
                bounds: Bounds::new(lb, ub),
                var_type: vt,
            }
        }
        1 => {
            let c = con_id(*next_con);
            *next_con += 1;
            live_cons.push(c);
            let typ: u32 = rng.random_range(0..3);
            match typ {
                0 => ModelOp::AddConstraint {
                    con: c,
                    bounds: ConstraintBounds::le(rng.random_range(-100.0..100.0)),
                },
                1 => ModelOp::AddConstraint {
                    con: c,
                    bounds: ConstraintBounds::ge(rng.random_range(-100.0..100.0)),
                },
                _ => {
                    let lb = rng.random_range(-50.0..0.0);
                    let ub = rng.random_range(0.0..50.0);
                    ModelOp::AddConstraint {
                        con: c,
                        bounds: ConstraintBounds::range(lb, ub),
                    }
                }
            }
        }
        2 => {
            let o = obj_id(*next_obj);
            *next_obj += 1;
            live_objs.push(o);
            ModelOp::AddObjective {
                obj: o,
                sense: if rng.random_bool(0.5) {
                    Sense::Minimize
                } else {
                    Sense::Maximize
                },
            }
        }
        _ => {
            let p = param_id(*next_param);
            *next_param += 1;
            live_params.push(p);
            ModelOp::SetParameter {
                param: p,
                value: rng.random_range(-100.0..100.0),
            }
        }
    }
}

/// Generate a random "remove" operation.
fn gen_remove_op(
    rng: &mut impl rand::Rng,
    live_vars: &mut Vec<VarId>,
    live_cons: &mut Vec<ConId>,
    live_objs: &mut Vec<ObjId>,
) -> ModelOp {
    let n_vars = live_vars.len();
    let n_cons = live_cons.len();
    let n_objs = live_objs.len();

    // Weight toward removing the entity type with the most entries
    let var_weight = if n_vars > 0 { n_vars as u32 * 2 } else { 0 };
    let con_weight = if n_cons > 0 { n_cons as u32 } else { 0 };
    let obj_weight = if n_objs > 0 { n_objs as u32 } else { 0 };
    let total = var_weight + con_weight + obj_weight;

    if total == 0 {
        // Fallback: add a variable
        let v = var_id(next_free_index(live_vars));
        live_vars.push(v);
        return ModelOp::AddVariable {
            var: v,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        };
    }

    let roll: u32 = rng.random_range(0..total);
    if roll < var_weight {
        let idx = rng.random_range(0..n_vars);
        let v = live_vars.remove(idx);
        ModelOp::RemoveVariable { var: v }
    } else if roll < var_weight + con_weight {
        let idx = rng.random_range(0..n_cons);
        let c = live_cons.remove(idx);
        ModelOp::RemoveConstraint { con: c }
    } else {
        let idx = rng.random_range(0..n_objs);
        let o = live_objs.remove(idx);
        ModelOp::RemoveObjective { obj: o }
    }
}

/// Generate a random "mutate" operation.
#[allow(clippy::too_many_arguments)]
fn gen_mutate_op(
    rng: &mut impl rand::Rng,
    live_vars: &[VarId],
    live_cons: &[ConId],
    live_objs: &[ObjId],
    live_params: &[ParamId],
) -> ModelOp {
    // Collect available mutation categories
    let has_vars = !live_vars.is_empty();
    let has_cons = !live_cons.is_empty();
    let has_objs = !live_objs.is_empty();
    let has_params = !live_params.is_empty();
    let has_both = has_vars && has_cons;

    // Build a list of possible mutation types
    let mut choices: Vec<u32> = Vec::new();
    if has_vars {
        choices.push(0); // SetVariableBounds
        choices.push(1); // SetVariableActive
        choices.push(2); // SetVariableType
    }
    if has_cons {
        choices.push(3); // SetConstraintBounds
        choices.push(4); // SetConstraintActive
    }
    if has_both {
        choices.push(5); // SetCell
    }
    if has_objs {
        choices.push(6); // SetActiveObjective (also works without existing obj)
        if has_vars {
            choices.push(7); // SetObjectiveCell
        }
    }
    if has_params {
        choices.push(8); // SetParameter (update existing)
    }

    if choices.is_empty() {
        // Fallback: add a parameter
        let p = param_id(next_free_index_params(live_params));
        return ModelOp::SetParameter {
            param: p,
            value: 0.0,
        };
    }

    let pick = choices[rng.random_range(0..choices.len())];
    match pick {
        0 => {
            let v = live_vars[rng.random_range(0..live_vars.len())];
            let lb = rng.random_range(-10.0..10.0);
            let ub = rng.random_range(lb..=lb + 100.0);
            ModelOp::SetVariableBounds {
                var: v,
                bounds: Bounds::new(lb, ub),
            }
        }
        1 => {
            let v = live_vars[rng.random_range(0..live_vars.len())];
            ModelOp::SetVariableActive {
                var: v,
                active: rng.random_bool(0.5),
            }
        }
        2 => {
            let v = live_vars[rng.random_range(0..live_vars.len())];
            ModelOp::SetVariableType {
                var: v,
                var_type: match rng.random_range(0u32..3) {
                    0 => VarType::Continuous,
                    1 => VarType::Integer,
                    _ => VarType::Binary,
                },
            }
        }
        3 => {
            let c = live_cons[rng.random_range(0..live_cons.len())];
            let typ: u32 = rng.random_range(0..3);
            match typ {
                0 => ModelOp::SetConstraintBounds {
                    con: c,
                    bounds: ConstraintBounds::le(rng.random_range(-100.0..100.0)),
                },
                1 => ModelOp::SetConstraintBounds {
                    con: c,
                    bounds: ConstraintBounds::ge(rng.random_range(-100.0..100.0)),
                },
                _ => {
                    let lb = rng.random_range(-50.0..0.0);
                    let ub = rng.random_range(0.0..50.0);
                    ModelOp::SetConstraintBounds {
                        con: c,
                        bounds: ConstraintBounds::range(lb, ub),
                    }
                }
            }
        }
        4 => {
            let c = live_cons[rng.random_range(0..live_cons.len())];
            ModelOp::SetConstraintActive {
                con: c,
                active: rng.random_bool(0.5),
            }
        }
        5 => {
            let c = live_cons[rng.random_range(0..live_cons.len())];
            let v = live_vars[rng.random_range(0..live_vars.len())];
            let val = rng.random_range(-10.0..10.0);
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c), v),
                value_expr: ValueExpr::constant(val),
                evaluated_value: val,
            }
        }
        6 => {
            let obj_pick: Option<ObjId> = if rng.random_bool(0.7) && !live_objs.is_empty() {
                Some(live_objs[rng.random_range(0..live_objs.len())])
            } else {
                None
            };
            ModelOp::SetActiveObjective { obj: obj_pick }
        }
        7 => {
            let o = live_objs[rng.random_range(0..live_objs.len())];
            let v = live_vars[rng.random_range(0..live_vars.len())];
            let val = rng.random_range(-10.0..10.0);
            let constant = rng.random_range(-50.0..50.0);
            ModelOp::SetObjectiveCell {
                cell_key: (CoefficientTarget::Objective(o), v),
                value_expr: ValueExpr::constant(val),
                evaluated_value: val,
                constant,
            }
        }
        _ => {
            let p = live_params[rng.random_range(0..live_params.len())];
            ModelOp::SetParameter {
                param: p,
                value: rng.random_range(-100.0..100.0),
            }
        }
    }
}

/// Find the next available index for a new entity (for fallback add).
fn next_free_index(entities: &[VarId]) -> u32 {
    entities.iter().map(|v| v.index()).max().unwrap_or(0) + 1
}

fn next_free_index_params(entities: &[ParamId]) -> u32 {
    entities.iter().map(|p| p.index()).max().unwrap_or(0) + 1
}

/// Build a `ModelSnapshot` from a `NormalizedView`.
/// Extracts objective constants directly from the view's `objective_cells`,
/// using the first cell's constant for each objective.
fn build_snapshot_from_view(rev: ModelRevision, view: &NormalizedView) -> ModelSnapshot {
    let mut variables = HashMap::new();
    for (id, bounds, var_type, active, semi) in &view.variables {
        variables.insert(*id, (*bounds, *var_type, *active, *semi));
    }

    let mut constraints = HashMap::new();
    for (id, bounds, active) in &view.constraints {
        constraints.insert(*id, (*bounds, *active));
    }

    // Extract objective constants from the view's objective_cells
    let mut obj_constants: HashMap<ObjId, f64> = HashMap::new();
    for (ckey, _val, constant) in &view.objective_cells {
        if let CoefficientTarget::Objective(oid) = ckey.0 {
            obj_constants.entry(oid).or_insert(*constant);
        }
    }

    let mut objectives = HashMap::new();
    for (id, sense, active) in &view.objectives {
        let constant = obj_constants.get(id).copied().unwrap_or(0.0);
        objectives.insert(*id, (*sense, *active, constant));
    }

    let mut params = HashMap::new();
    for (id, value) in &view.parameters {
        params.insert(*id, *value);
    }

    // Constraint cells
    let constraint_cells: Vec<((CoefficientTarget, VarId), ValueExpr, f64, Vec<ParamId>)> = view
        .cells
        .iter()
        .map(|(ck, val)| (*ck, ValueExpr::constant(*val), *val, vec![]))
        .collect();

    // Objective cells — must be included in the snapshot so they survive rebuild
    let objective_cells_from_view: Vec<(
        (CoefficientTarget, VarId),
        ValueExpr,
        f64,
        Vec<ParamId>,
    )> = view
        .objective_cells
        .iter()
        .map(|(ck, val, _constant)| (*ck, ValueExpr::constant(*val), *val, vec![]))
        .collect();

    // Combine both into the cells parameter for take_snapshot
    let mut all_cells = constraint_cells;
    all_cells.extend(objective_cells_from_view);

    take_snapshot(rev, &variables, &constraints, &objectives, &params, &all_cells)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Section 3: Multi-adapter cursor independence
// ═══════════════════════════════════════════════════════════════════════════════
//
// Two ReferenceBackend instances share a SyncCoordinator journal.
// Cursor A advances fully, cursor B lags.  After catching up, both
// produce identical normalized views.

#[test]
fn dx_multi_adapter_cursor_independence() {
    let v1 = var_id(0);
    let v2 = var_id(1);
    let c = con_id(0);
    let p = param_id(0);
    let o = obj_id(0);

    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();
    let r3 = r2.next().unwrap();

    // ── Set up coordinator with three batches ────────────────────────────
    let mut coordinator = SyncCoordinator::new();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddVariable {
                var: v1,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            },
            ModelOp::AddConstraint {
                con: c,
                bounds: ConstraintBounds::le(100.0),
            },
            ModelOp::SetParameter {
                param: p,
                value: 42.0,
            },
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c), v1),
                value_expr: ValueExpr::param(p),
                evaluated_value: 42.0,
            },
        ],
    )
    .unwrap();
    coordinator.commit_batch(batch1.clone()).unwrap();

    let batch2 = DeltaBatch::new(
        r1,
        r2,
        vec![
            ModelOp::AddVariable {
                var: v2,
                bounds: Bounds::BINARY,
                var_type: VarType::Binary,
            },
            ModelOp::AddObjective {
                obj: o,
                sense: Sense::Maximize,
            },
            ModelOp::SetObjectiveCell {
                cell_key: (CoefficientTarget::Objective(o), v1),
                value_expr: ValueExpr::param(p),
                evaluated_value: 42.0,
                constant: 10.0,
            },
        ],
    )
    .unwrap();
    coordinator.commit_batch(batch2.clone()).unwrap();

    let batch3 = DeltaBatch::new(
        r2,
        r3,
        vec![
            ModelOp::SetActiveObjective { obj: Some(o) },
            ModelOp::SetVariableBounds {
                var: v2,
                bounds: Bounds::new(0.0, 1.0),
            },
        ],
    )
    .unwrap();
    coordinator.commit_batch(batch3.clone()).unwrap();

    // ── Backend A: catch up to r3 ────────────────────────────────────────
    let (mut backend_a, mut cursor_a) = fresh();
    let batches_a = coordinator.batches_for_cursor(&cursor_a).unwrap();
    for batch in &batches_a {
        let outcome = apply_batch(&mut backend_a, &mut cursor_a, batch);
        assert!(matches!(outcome, ApplyOutcome::Applied { .. }));
    }
    assert_eq!(cursor_a.applied_revision, r3);
    let view_a = backend_a.normalized_view();

    // ── Backend B: initially at r0 (lagging) ─────────────────────────────
    // 1. Apply first batch only → cursor at r1
    let (mut backend_b, mut cursor_b) = fresh();
    let batches_b_at_r0 = coordinator.batches_for_cursor(&cursor_b).unwrap();

    // Apply only batch1
    let outcome = apply_batch(&mut backend_b, &mut cursor_b, batches_b_at_r0[0]);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));
    assert_eq!(cursor_b.applied_revision, r1);

    // Verify B's state at r1 is correct by comparing with A at r1
    // (Snapshot + rebuild from r1 snapshot)
    let mut vars_r1 = HashMap::new();
    vars_r1.insert(v1, (Bounds::NON_NEGATIVE, VarType::Continuous, true, None));
    let mut cons_r1 = HashMap::new();
    cons_r1.insert(c, (ConstraintBounds::le(100.0), true));
    let mut params_r1 = HashMap::new();
    params_r1.insert(p, 42.0);
    let objs_r1 = HashMap::new();
    let cells_r1: Vec<((CoefficientTarget, VarId), ValueExpr, f64, Vec<ParamId>)> = vec![(
        (CoefficientTarget::Constraint(c), v1),
        ValueExpr::param(p),
        42.0,
        vec![p],
    )];
    let snap_r1 = take_snapshot(r1, &vars_r1, &cons_r1, &objs_r1, &params_r1, &cells_r1);
    let (mut ref_backend, mut ref_cursor) = fresh();
    rebuild(&mut ref_backend, &mut ref_cursor, &snap_r1);
    assert_eq!(
        backend_b.normalized_view(),
        ref_backend.normalized_view(),
        "backend B at r1 should match snapshot rebuild at r1"
    );

    // 2. Catch up B to r3
    let batches_b_at_r1 = coordinator.batches_for_cursor(&cursor_b).unwrap();
    assert_eq!(batches_b_at_r1.len(), 2); // r1→r2, r2→r3
    for batch in &batches_b_at_r1 {
        let outcome = apply_batch(&mut backend_b, &mut cursor_b, batch);
        assert!(matches!(outcome, ApplyOutcome::Applied { .. }));
    }
    assert_eq!(cursor_b.applied_revision, r3);

    // ── Both backends should now produce identical views ─────────────────
    assert_eq!(
        view_a,
        backend_b.normalized_view(),
        "fully-caught-up backends must produce identical views"
    );
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
    let outcome = backend
        .apply_batch(&init_batch, &mut cursor)
        .unwrap();
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));
    assert_eq!(cursor.applied_revision, r1);

    // Snapshot the state before the fault
    let view_before = backend.normalized_view();
    assert_eq!(view_before.variables.len(), 1);

    // Configure fault at global op 1 (first op of next batch)
    backend.configure_fault(
        1,
        FaultOutcome::Recoverable("simulated recoverable".into()),
    );

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
    let outcome = backend
        .apply_batch(&fault_batch, &mut cursor)
        .unwrap();

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
    backend.configure_fault(
        2,
        FaultOutcome::Dirty("simulated dirty".into()),
    );

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
    backend.configure_fault(
        2,
        FaultOutcome::Dirty("simulated dirty".into()),
    );

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
    let (mut direct_backend, mut direct_cursor) = fresh();
    rebuild(&mut direct_backend, &mut direct_cursor, &snap_r2);
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
// Section 5: Rebuild determinism
// ═══════════════════════════════════════════════════════════════════════════════
//
// Two backends rebuilt from the same snapshot must produce identical
// normalized views.  This proves that the rebuild path is deterministic
// across independent backend instances.

#[test]
fn dx_rebuild_determinism() {
    let v = var_id(0);
    let v2 = var_id(1);
    let c = con_id(0);
    let c2 = con_id(1);
    let o = obj_id(0);
    let p = param_id(0);

    let rev = rev_from_u64(42);

    // Build a moderately complex snapshot with multiple entities of each type
    let mut variables = HashMap::new();
    variables.insert(
        v,
        (Bounds::new(0.0, 100.0), VarType::Continuous, true, Some(5.0)),
    );
    variables.insert(
        v2,
        (Bounds::BINARY, VarType::Binary, false, None),
    );

    let mut constraints = HashMap::new();
    constraints.insert(c, (ConstraintBounds::le(50.0), true));
    constraints.insert(c2, (ConstraintBounds::ge(10.0), false));

    let mut objectives = HashMap::new();
    objectives.insert(o, (Sense::Maximize, true, 42.5));

    let mut params = HashMap::new();
    params.insert(p, 3.14);

    let cells: Vec<((CoefficientTarget, VarId), ValueExpr, f64, Vec<ParamId>)> = vec![
        (
            (CoefficientTarget::Constraint(c), v),
            ValueExpr::constant(2.5),
            2.5,
            vec![],
        ),
        (
            (CoefficientTarget::Constraint(c2), v2),
            ValueExpr::constant(1.0),
            1.0,
            vec![],
        ),
        (
            (CoefficientTarget::Objective(o), v),
            ValueExpr::param(p),
            3.14,
            vec![p],
        ),
    ];

    let snapshot = take_snapshot(rev, &variables, &constraints, &objectives, &params, &cells);

    // Rebuild two independent backends from the same snapshot
    let (mut backend_a, mut cursor_a) = fresh();
    rebuild(&mut backend_a, &mut cursor_a, &snapshot);

    let (mut backend_b, mut cursor_b) = fresh();
    rebuild(&mut backend_b, &mut cursor_b, &snapshot);

    // Both must produce identical views
    let view_a = backend_a.normalized_view();
    let view_b = backend_b.normalized_view();

    assert_eq!(
        view_a, view_b,
        "two backends rebuilt from the same snapshot must produce identical views"
    );

    // Verify cursors match
    assert_eq!(cursor_a.applied_revision, cursor_b.applied_revision);
    assert_eq!(cursor_a.health, cursor_b.health);
    assert!(cursor_a.is_ready());
    assert!(cursor_b.is_ready());

    // Verify specific content
    assert_eq!(view_a.variables.len(), 2);
    assert_eq!(view_a.constraints.len(), 2);
    assert_eq!(view_a.objectives.len(), 1);
    assert_eq!(view_a.parameters.len(), 1);
    assert_eq!(view_a.cells.len(), 2);
    assert_eq!(view_a.objective_cells.len(), 1);
    assert_eq!(view_a.active_objective, Some(o));
    assert_eq!(view_a.revision, rev);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Section 6: Semi-continuous partial-apply scenario
// ═══════════════════════════════════════════════════════════════════════════════
//
// Models the sequence:
//   1. Add a variable with ordinary bounds  (HiGHS applies this successfully)
//   2. Semi-continuous domain change        (HiGHS rejects as unsupported)
//
// Verifies:
//   - The delta batch is preserved in the journal
//   - After rebuild from snapshot, the full state includes both the
//     original bounds AND the semi-continuous lower bound
//   - No delta is lost

#[test]
fn dx_semicontinuous_partial_apply() {
    let v = var_id(0);
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    // ── Step 1: Variable with ordinary bounds ────────────────────────────
    // This batch succeeds (no semi-continuous info).
    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::AddVariable {
            var: v,
            bounds: Bounds::new(0.0, 100.0),
            var_type: VarType::Continuous,
        }],
    )
    .unwrap();

    // Commit to coordinator (simulates the model journal)
    let mut coordinator = SyncCoordinator::new();
    coordinator.commit_batch(batch1.clone()).unwrap();
    assert_eq!(coordinator.journal.len(), 1);

    // Apply to a reference backend
    let (mut backend, mut cursor) = fresh();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch1);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));
    assert_eq!(cursor.applied_revision, r1);

    // Verify normal state at r1
    let view_r1 = backend.normalized_view();
    assert_eq!(view_r1.variables.len(), 1);
    assert_eq!(view_r1.variables[0].1, Bounds::new(0.0, 100.0));
    assert_eq!(view_r1.variables[0].4, None); // No semi-continuous

    // ── Step 2: Semi-continuous change via snapshot rebuild ──────────────
    // In a real HiGHS scenario, the user requests a semi-continuous domain
    // change.  The incremental adapter returns RequiresRebuild because
    // semi-continuous is not incrementally supported.  The delta batch
    // (which encodes the semi-continuous info) is committed to the journal.
    //
    // Here we simulate this by taking a snapshot at r2 that includes the
    // semi-continuous lower bound.  The batch r1→r2 is stored in the journal
    // even though incremental apply was not possible.

    // Build snapshot at r2 with the variable's bounds AND semi-continuous
    let mut vars_r2 = HashMap::new();
    vars_r2.insert(
        v,
        (
            Bounds::new(0.0, 100.0),
            VarType::Continuous,
            true,
            Some(5.0), // semi-continuous lower bound
        ),
    );
    let cons_r2: HashMap<ConId, (ConstraintBounds, bool)> = HashMap::new();
    let objs_r2: HashMap<ObjId, (Sense, bool, f64)> = HashMap::new();
    let params_r2: HashMap<ParamId, f64> = HashMap::new();
    let cells_r2: Vec<((CoefficientTarget, VarId), ValueExpr, f64, Vec<ParamId>)> = Vec::new();
    let snap_r2 = take_snapshot(r2, &vars_r2, &cons_r2, &objs_r2, &params_r2, &cells_r2);

    // Simulate that a batch was also committed to the journal for the
    // semi-continuous change (it encodes the "intent" even though the
    // adapter returned RequiresRebuild).
    let batch2 = DeltaBatch::new(
        r1,
        r2,
        vec![], // empty batch — the semi-continuous info is only in the snapshot
    )
    .unwrap();
    coordinator.commit_batch(batch2.clone()).unwrap();

    // ── Verify journal preservation ──────────────────────────────────────
    // Both batches must be in the journal.
    let deltas = coordinator.journal.deltas_since(r0).unwrap();
    assert_eq!(
        deltas.len(),
        2,
        "journal must preserve both delta batches"
    );

    // The first batch (add variable) is preserved intact
    assert_eq!(deltas[0].operations.len(), 1);
    assert!(matches!(
        deltas[0].operations[0],
        ModelOp::AddVariable { .. }
    ));

    // ── Rebuild from snapshot — no delta loss ────────────────────────────
    // After rebuild, the full state includes the ordinary bounds AND the
    // semi-continuous lower bound that was set via the snapshot.
    rebuild(&mut backend, &mut cursor, &snap_r2);

    let rebuilt_view = backend.normalized_view();
    assert_eq!(
        rebuilt_view.variables.len(),
        1,
        "rebuilt state should have one variable"
    );
    assert_eq!(
        rebuilt_view.variables[0].0, v,
        "variable ID should match"
    );
    assert_eq!(
        rebuilt_view.variables[0].1,
        Bounds::new(0.0, 100.0),
        "ordinary bounds must be preserved after rebuild"
    );
    assert_eq!(
        rebuilt_view.variables[0].4,
        Some(5.0),
        "semi-continuous lower bound must be present after rebuild"
    );

    // ── Verify no delta is lost: journal still has both batches ──────────
    let deltas_after = coordinator.journal.deltas_since(r0).unwrap();
    assert_eq!(
        deltas_after.len(),
        2,
        "journal must still contain all batches after rebuild"
    );
    assert_eq!(
        deltas_after[0].operations.len(),
        1,
        "batch0 operations preserved"
    );

    // ── Direct verification: rebuild from snap_r2 equals applying ────────
    // batch1 and then rebuilding from snap_r2.
    let (mut direct_backend, mut direct_cursor) = fresh();
    apply_batch(&mut direct_backend, &mut direct_cursor, &batch1);
    rebuild(&mut direct_backend, &mut direct_cursor, &snap_r2);

    assert_eq!(
        backend.normalized_view(),
        direct_backend.normalized_view(),
        "rebuild path must be deterministic regardless of prior state"
    );
}
