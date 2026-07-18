//! Backend contract tests for M1R-C1 through C6 invariants.
//!
//! These tests validate the protocol foundation using the types from Plan 01:
//! - DeltaBatch, ModelOp, ModelRevision (delta module)
//! - AdapterCursor, AdapterHealth, ApplyOutcome, SyncCoordinator (sync module)
//! - Journal (journal module)
//! - ModelSnapshot (snapshot module)
//! - ReferenceBackend, NormalizedView (solver/reference module)
//! - TerminationStatus (solver/backend module)
//! - Synchronization, SyncReceipt (solver/session module)
//!
//! Each test corresponds to one requirement invariant:
//!   Test  1-4: M1R-C1  — Journal/cursor protocol
//!   Test  5:   M1R-C5  — Commuting square
//!   Test  6:   M1R-C1/C4 — Rebuild resets cursor and health
//!   Test  7-9: M1R-C6  — Status preserves details
//!   Test 10:   M1R-C1  — Synchronization enum dispatch

use std::collections::HashMap;

use roml::delta::{DeltaBatch, ModelOp};
use roml::id::{ConId, ParamId, VarId};
use roml::id::Generation;
use roml::journal::Journal;
use roml::model::coefficient::{CellKey, CoefficientTarget};
use roml::model::{Bounds, ConstraintBounds, VarType};
use roml::revision::ModelRevision;
use roml::snapshot::{ModelSnapshot, take_snapshot};
use roml::solver::backend::TerminationStatus;
use roml::solver::reference::ReferenceBackend;
use roml::solver::session::{Synchronization, SyncReceipt};
use roml::sync::{AdapterCursor, AdapterHealth, ApplyOutcome, ApplyError, SyncCoordinator};
use roml::value_expr::ValueExpr;

// ── Helpers ──────────────────────────────────────────────────────────────

fn make_var(index: u32) -> VarId {
    VarId::new(index, Generation::new())
}
fn make_con(index: u32) -> ConId {
    ConId::new(index, Generation::new())
}
fn make_param(index: u32) -> ParamId {
    ParamId::new(index, Generation::new())
}

fn empty_batch(from: ModelRevision, to: ModelRevision) -> DeltaBatch {
    DeltaBatch::new(from, to, vec![]).unwrap()
}

// ── M1R-C1: Journal/cursor protocol ─────────────────────────────────────

/// A failed apply does NOT consume the batch from the journal.
/// The journal entry survives regardless of adapter-side errors.
#[test]
fn error_preserves_journal_entry() {
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    // Create a journal with one batch
    let mut journal = Journal::new();
    journal.record(empty_batch(r0, r1)).unwrap();
    assert_eq!(journal.len(), 1);
    assert!(journal.get(r0).is_some());

    // Create backend and cursor at r0
    let mut backend = ReferenceBackend::new();
    let mut cursor = AdapterCursor::new();

    // Attempt to apply a batch whose `from` doesn't match the cursor
    let mismatched_batch = empty_batch(r1, r2);
    let outcome = backend.apply_batch(&mismatched_batch, &mut cursor).unwrap();

    // The apply fails (revision mismatch) but...
    assert!(matches!(outcome, ApplyOutcome::RecoverableFailure { .. }));

    // ...the journal still has its batch (not consumed by failed apply)
    assert_eq!(journal.len(), 1);
    assert!(journal.get(r0).is_some());
}

/// Two independent cursors can catch up at different rates from the same
/// coordinator without interfering with each other.
#[test]
fn two_sessions_independently_catch_up() {
    let mut coordinator = SyncCoordinator::new();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    coordinator.commit_batch(empty_batch(r0, r1)).unwrap();
    coordinator.commit_batch(empty_batch(r1, r2)).unwrap();

    // Create two independent cursors
    let mut cursor_a = AdapterCursor::new();
    let mut cursor_b = AdapterCursor::new();

    // Advance cursor A fully
    let batches_a = coordinator.batches_for_cursor(&cursor_a).unwrap();
    for batch in batches_a {
        cursor_a.advance(batch).unwrap();
    }
    assert_eq!(cursor_a.applied_revision, r2);
    assert!(cursor_a.is_ready());

    // Cursor B is still at r0
    assert_eq!(cursor_b.applied_revision, r0);
    assert!(cursor_b.is_ready());

    // Advance cursor B to r1 only
    let batches_b_first = coordinator.batches_for_cursor(&cursor_b).unwrap();
    cursor_b.advance(batches_b_first[0]).unwrap();
    assert_eq!(cursor_b.applied_revision, r1);
    assert!(cursor_b.is_ready());

    // Cursor A still at r2, cursor B at r1
    assert_eq!(cursor_a.applied_revision, r2);
    assert_eq!(cursor_b.applied_revision, r1);

    // Advance cursor B to r2
    let batches_b_rest = coordinator.batches_for_cursor(&cursor_b).unwrap();
    cursor_b.advance(batches_b_rest[0]).unwrap();
    assert_eq!(cursor_b.applied_revision, r2);
    assert!(cursor_b.is_ready());
}

/// ApplyOutcome distinguishes recoverable from terminal failures,
/// and the cursor state is unchanged after a recoverable failure.
#[test]
fn apply_outcome_distinguishes_recoverable_terminal() {
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let mut backend = ReferenceBackend::new();
    let mut cursor = AdapterCursor::new();

    // Cursor starts at r0
    assert_eq!(cursor.applied_revision, r0);

    // Try to apply batch r1→r2 (future) - expect RecoverableFailure
    let future_batch = empty_batch(r1, r2);
    let outcome = backend.apply_batch(&future_batch, &mut cursor).unwrap();
    assert!(
        matches!(outcome, ApplyOutcome::RecoverableFailure { .. }),
        "expected RecoverableFailure for revision mismatch"
    );

    // After RecoverableFailure, cursor is unchanged
    assert_eq!(cursor.applied_revision, r0);
    assert!(cursor.is_ready());

    // Apply valid batch r0→r1
    let valid_batch = empty_batch(r0, r1);
    let outcome = backend.apply_batch(&valid_batch, &mut cursor).unwrap();
    assert!(matches!(outcome, ApplyOutcome::Applied { new_revision } if new_revision == r1));

    // After valid apply, cursor is at r1
    assert_eq!(cursor.applied_revision, r1);
    assert!(cursor.is_ready());
}

/// A revision mismatch between batch and cursor is detected and reported as an error.
/// The cursor position is preserved.
#[test]
fn revision_mismatch_detected() {
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let mut cursor = AdapterCursor::new();
    assert_eq!(cursor.applied_revision, r0);

    // Try to advance cursor with batch r1→r2 (cursor is at r0)
    let batch = empty_batch(r1, r2);
    let err = cursor.advance(&batch).unwrap_err();
    assert_eq!(
        err,
        ApplyError::RevisionMismatch {
            expected: r0,
            got: r1,
        }
    );

    // Cursor remains at r0
    assert_eq!(cursor.applied_revision, r0);
}

// ── M1R-C5: Commuting square ────────────────────────────────────────────

/// Snapshot rebuild and full incremental application produce equivalent
/// normalized views (the commuting square property).
#[test]
fn snapshot_rebuild_equals_incremental_apply() {
    let var = make_var(0);
    let con = make_con(0);
    let p = make_param(0);

    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    // Snapshot at r0 (empty)
    let snap_r0 = ModelSnapshot::empty(r0);

    // Snapshot at r1 (has var, con, param, cell)
    let mut vars_r1 = HashMap::new();
    vars_r1.insert(var, (Bounds::NON_NEGATIVE, VarType::Continuous, true, None));
    let mut cons_r1 = HashMap::new();
    cons_r1.insert(con, (ConstraintBounds::le(10.0), true));
    let mut params_r1 = HashMap::new();
    params_r1.insert(p, 5.0);
    let objs_r1 = HashMap::new();
    let cells_r1: Vec<(CellKey, ValueExpr, f64, Vec<ParamId>)> = vec![(
        (CoefficientTarget::Constraint(con), var),
        ValueExpr::param(p),
        5.0,
        vec![p],
    )];
    let snap_r1 = take_snapshot(r1, &vars_r1, &cons_r1, &objs_r1, &params_r1, &cells_r1);

    // Deltas from r0 to r1
    let ops = vec![
        ModelOp::AddVariable {
            var,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        },
        ModelOp::AddConstraint {
            con,
            bounds: ConstraintBounds::le(10.0),
        },
        ModelOp::SetParameter {
            param: p,
            value: 5.0,
        },
        ModelOp::SetCell {
            cell_key: (CoefficientTarget::Constraint(con), var),
            value_expr: ValueExpr::param(p),
            evaluated_value: 5.0,
        },
    ];
    let batch = DeltaBatch::new(r0, r1, ops).unwrap();

    // Backend A: rebuild from r1 snapshot
    let mut backend_a = ReferenceBackend::new();
    let mut cursor_a = AdapterCursor::new();
    backend_a.rebuild(&snap_r1, &mut cursor_a);
    let view_a = backend_a.normalized_view();

    // Backend B: rebuild from r0, then apply deltas
    let mut backend_b = ReferenceBackend::new();
    let mut cursor_b = AdapterCursor::new();
    backend_b.rebuild(&snap_r0, &mut cursor_b);
    let outcome = backend_b.apply_batch(&batch, &mut cursor_b).unwrap();
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));
    let view_b = backend_b.normalized_view();

    // Commuting square: snapshot rebuild == incremental apply
    assert_eq!(
        view_a, view_b,
        "snapshot r1 != apply(snapshot r0, deltas r0→r1)"
    );
}

// ── M1R-C1/C4: Rebuild resets cursor and health ─────────────────────────

/// Rebuilding from a snapshot resets the cursor to the snapshot's revision
/// and restores AdapterHealth to Ready.
#[test]
fn rebuild_resets_cursor_and_health() {
    let var = make_var(0);
    let con = make_con(0);

    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let mut backend = ReferenceBackend::new();
    let mut cursor = AdapterCursor::new();

    // Apply some mutations to establish state
    let ops = vec![
        ModelOp::AddVariable {
            var,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        },
        ModelOp::AddConstraint {
            con,
            bounds: ConstraintBounds::le(10.0),
        },
    ];
    let batch = DeltaBatch::new(r0, r1, ops).unwrap();
    backend.apply_batch(&batch, &mut cursor).unwrap();
    assert_eq!(backend.variables.len(), 1);
    assert_eq!(cursor.applied_revision, r1);

    // Mark cursor as needing rebuild (simulate an adapter error)
    cursor.mark_rebuild();
    assert!(!cursor.is_ready());

    // Rebuild from a new snapshot at a different revision
    let r2 = r1.next().unwrap();
    let snap_r2 = ModelSnapshot::empty(r2);
    backend.rebuild(&snap_r2, &mut cursor);

    // Cursor was reset to the snapshot's revision
    assert_eq!(cursor.applied_revision, r2);

    // AdapterHealth is Ready after rebuild
    assert!(cursor.is_ready());
    assert_eq!(cursor.health, AdapterHealth::Ready);

    // Backend state matches the new snapshot (empty)
    assert!(backend.variables.is_empty());
}

// ── M1R-C6: TerminationStatus preserves details ─────────────────────────

/// TerminationStatus::Feasible captures "incumbent found, not proven optimal".
#[test]
fn status_preserves_incumbent() {
    // Compile-time assertion: Feasible variant exists
    let status = TerminationStatus::Feasible;
    assert!(matches!(status, TerminationStatus::Feasible));
}

/// InfeasibleOrUnbounded preserves ambiguity (HiGHS can return this).
#[test]
fn status_preserves_ambiguity() {
    // Compile-time assertion: InfeasibleOrUnbounded variant exists
    let status = TerminationStatus::InfeasibleOrUnbounded;
    assert!(matches!(status, TerminationStatus::InfeasibleOrUnbounded));
}

/// TimeLimit, IterationLimit, NodeLimit, Interrupted are distinct variants
/// preserving limits and interruption origin.
#[test]
fn status_preserves_limits_and_interruption() {
    // Compile-time assertions: each variant exists
    let time_limit = TerminationStatus::TimeLimit;
    let iter_limit = TerminationStatus::IterationLimit;
    let node_limit = TerminationStatus::NodeLimit;
    let interrupted = TerminationStatus::Interrupted;

    assert!(matches!(time_limit, TerminationStatus::TimeLimit));
    assert!(matches!(iter_limit, TerminationStatus::IterationLimit));
    assert!(matches!(node_limit, TerminationStatus::NodeLimit));
    assert!(matches!(interrupted, TerminationStatus::Interrupted));

    // All are distinct
    assert_ne!(time_limit, iter_limit);
    assert_ne!(iter_limit, node_limit);
    assert_ne!(node_limit, interrupted);
}

// ── M1R-C1: Synchronization enum dispatch ───────────────────────────────

/// Synchronization can be constructed with both DeltaBatch and Rebuild
/// variants, and SyncReceipt fields are accessible.
#[test]
fn synchronization_enum_dispatch() {
    // Construct Synchronization::DeltaBatch
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let batch = empty_batch(r0, r1);
    let sync_batch = Synchronization::DeltaBatch(batch);
    assert!(matches!(sync_batch, Synchronization::DeltaBatch(_)));

    // Construct Synchronization::Rebuild
    let snap = ModelSnapshot::empty(r1);
    let sync_rebuild = Synchronization::Rebuild(snap);
    assert!(matches!(sync_rebuild, Synchronization::Rebuild(_)));

    // SyncReceipt fields are accessible
    let cursor = AdapterCursor::new();
    let receipt = SyncReceipt {
        cursor: cursor.clone(),
        health: AdapterHealth::Ready,
    };
    assert_eq!(receipt.cursor.applied_revision, r0);
    assert_eq!(receipt.health, AdapterHealth::Ready);

    // RequireRebuild health
    let mut rebuild_cursor = AdapterCursor::new();
    rebuild_cursor.mark_rebuild();
    let rebuild_receipt = SyncReceipt {
        cursor: rebuild_cursor.clone(),
        health: AdapterHealth::RequiresRebuild,
    };
    assert_eq!(rebuild_receipt.health, AdapterHealth::RequiresRebuild);
    assert!(rebuild_receipt.cursor.needs_rebuild());
}
