//! Characterization tests for the current model ⇔ solver synchronization behavior.
//!
//! These tests use fake adapters that record applied operations and can be
//! configured to fail after operation `k`. They prove four distinct weaknesses
//! in the current architecture — all rooted in the destructive changelog:
//!
//! 1. **Drained changes disappear on error** — `drain_changes()` is destructive:
//!    if the adapter returns an error, the changes are gone with no replay.
//! 2. **Second adapter cannot observe consumed changes** — one changelog cannot
//!    serve multiple sessions; the second adapter gets an empty batch.
//! 3. **Partial application leaves no recovery path** — an adapter that fails
//!    mid-`apply_changes` has no deterministic recovery: model changes are gone
//!    and the adapter is partially mutated.
//! 4. **Reset/rebuild not tied to revision** — `SolverAdapter::reset()` wipes
//!    adapter state but there is no versioned check to determine whether a
//!    rebuild would reproduce the current model state.
//!
//! All tests are marked with an `ignore` attribute referencing the M1R-01
//! resolution because they document broken legacy behaviour that the
//! revisioned sync architecture (M1R-01) resolves.

use std::collections::HashMap;

use roml::{
    Bounds, Change, Model, SolverAdapter, SolverError, SolverModelExt, SolverStatus, VarId, VarType,
};

// =========================================================================
// Fake adapter implementations
// =========================================================================

/// A fake adapter that records every batch of changes applied to it.
///
/// It does not solve anything — it only serves as a passive observer of
/// the sync protocol.
#[derive(Debug, Default)]
struct RecordingAdapter {
    /// Every batch of changes ever received via `apply_changes`.
    applied: Vec<Change>,
    /// Number of times `apply_changes` has been called.
    apply_count: usize,
    /// Number of times `reset` has been called.
    reset_count: usize,
}

impl RecordingAdapter {
    fn new() -> Self {
        Self::default()
    }
}

impl SolverAdapter for RecordingAdapter {
    fn apply_changes(&mut self, changes: &[Change]) -> Result<(), SolverError> {
        self.apply_count += 1;
        self.applied.extend_from_slice(changes);
        Ok(())
    }

    fn solve(&mut self) -> Result<SolverStatus, SolverError> {
        Ok(SolverStatus::Optimal)
    }

    fn status(&self) -> SolverStatus {
        SolverStatus::Optimal
    }

    fn solution_values(&self) -> Option<HashMap<VarId, f64>> {
        None
    }

    fn reset(&mut self) {
        self.reset_count += 1;
        self.applied.clear();
        self.apply_count = 0;
    }

    fn supports_incremental(&self, _change: &Change) -> bool {
        true
    }
}

/// A fake adapter that fails after successfully applying `k` changes.
///
/// It counts the *total number* of individual `Change` operations that have
/// been applied across all `apply_changes` calls. Once that total reaches or
/// exceeds the failure threshold `fail_after_ops`, the next call to
/// `apply_changes` returns `Err(SolverError::InternalError(...))`.
///
/// This simulates the scenario where a solver backend fails partway through
/// applying a batch of model mutations.
#[derive(Debug)]
struct FailingAfterAdapter {
    /// Total number of individual Change ops successfully processed so far.
    ops_applied: usize,
    /// Fail when `ops_applied >= fail_after_ops`.
    fail_after_ops: usize,
    /// Record of every individual Change ever applied (like RecordingAdapter).
    applied: Vec<Change>,
    /// Number of times `apply_changes` returned Ok.
    successful_calls: usize,
    /// The first error returned (if any).
    last_error: Option<SolverError>,
    /// Number of times `reset` has been called.
    reset_count: usize,
}

impl FailingAfterAdapter {
    fn new(fail_after_ops: usize) -> Self {
        Self {
            ops_applied: 0,
            fail_after_ops,
            applied: Vec::new(),
            successful_calls: 0,
            last_error: None,
            reset_count: 0,
        }
    }
}

impl SolverAdapter for FailingAfterAdapter {
    fn apply_changes(&mut self, changes: &[Change]) -> Result<(), SolverError> {
        // Check if we would exceed the failure threshold by applying this batch.
        if self.ops_applied + changes.len() >= self.fail_after_ops {
            // Simulate partial application: record only up to the failure point.
            let actually_applied = changes
                .iter()
                .take(self.fail_after_ops.saturating_sub(self.ops_applied))
                .cloned();
            let count_applied = actually_applied.len();
            self.applied.extend(actually_applied);
            self.ops_applied += count_applied;

            let err = SolverError::InternalError(format!(
                "simulated failure after {} operations (batch would need {} more, hits threshold {})",
                self.ops_applied,
                changes.len() - count_applied,
                self.fail_after_ops,
            ));
            self.last_error = Some(err.clone());
            return Err(err);
        }

        // Apply the full batch successfully.
        self.applied.extend_from_slice(changes);
        self.ops_applied += changes.len();
        self.successful_calls += 1;
        Ok(())
    }

    fn solve(&mut self) -> Result<SolverStatus, SolverError> {
        Ok(SolverStatus::Optimal)
    }

    fn status(&self) -> SolverStatus {
        SolverStatus::Optimal
    }

    fn solution_values(&self) -> Option<HashMap<VarId, f64>> {
        None
    }

    fn reset(&mut self) {
        self.reset_count += 1;
        self.ops_applied = 0;
        self.applied.clear();
        self.successful_calls = 0;
        self.last_error = None;
    }

    fn supports_incremental(&self, _change: &Change) -> bool {
        true
    }
}

// =========================================================================
// Test 1: Drained changes disappear on error
// =========================================================================

/// POST-M1R-01: This test documents the legacy drain_changes() behavior.
///
/// In the revisioned protocol, the journal retains the batch after emission.
/// A failed adapter can retry via the cursor without losing changes. The
/// SyncCoordinator does not destructively consume changes — the caller can
/// acknowledge a batch only after the adapter has successfully applied it.
#[ignore = "resolved in M1R-01 — drain_changes removal"]
#[test]
fn drained_changes_are_lost_on_adapter_error() {
    let mut model = Model::new();
    let _x = model.add_var();
    let _y = model.add_var();

    // Drain changes from the model (destructive).
    let drained = model.drain_changes();
    assert_eq!(drained.len(), 2, "should have 2 pending changes");

    // After drain, the model has nothing left.
    assert!(!model.has_pending_changes());
    let second_drain = model.drain_changes();
    assert!(
        second_drain.is_empty(),
        "second drain should be empty — changes are gone"
    );

    // The adapter that was supposed to receive these changes never saw them.
    let mut adapter = RecordingAdapter::new();
    // sync_model calls drain_changes internally — it would get an empty Vec.
    let result = adapter.apply_changes(&drained);
    assert!(result.is_ok(), "applying drained changes works");

    // But if the adapter had failed, the changes would be lost:
    // - They are gone from the model (drain_changes is destructive).
    // - No journal or revisioned batch exists to replay.
    // The test below proves this by using a FailingAfterAdapter.
    //
    // POST-M1R-01: In the revisioned protocol, changes survive adapter errors.
    // The journal retains the batch. The SyncCoordinator uses cursor
    // acknowledgement: the adapter acknowledges the batch only after
    // successful apply. On error, the batch remains available for retry.
}

/// POST-M1R-01: This test documents the legacy drain_changes() behavior.
///
/// In the revisioned protocol, changes survive apply errors. The journal is
/// not consumed on emission; the SyncCoordinator tracks acknowledged revisions
/// and the FailingAfterAdapter can retry from the cursor position.
#[ignore = "resolved in M1R-01 — drain_changes removal"]
#[test]
fn error_during_apply_loses_changes_from_model() {
    let mut model = Model::new();
    let _x = model.add_var();
    let _y = model.add_variable(Bounds::new(0.0, 5.0), VarType::Continuous);
    let _z = model.add_var();

    assert_eq!(model.drain_changes().len(), 3);
    // Re-create the changes for the failing adapter test.
    let _ = model.add_var();
    let _ = model.add_var();
    let _ = model.add_var();

    // Configure adapter to fail after 2 operations.
    let mut adapter = FailingAfterAdapter::new(2);

    let changes = model.drain_changes();
    assert_eq!(changes.len(), 3, "model has 3 pending changes");

    let result = adapter.apply_changes(&changes);
    assert!(
        result.is_err(),
        "adapter should fail mid-batch (2 applied, 3rd would exceed threshold)"
    );

    // Model has no remaining changes.
    assert!(
        !model.has_pending_changes(),
        "model's changes are gone — drain_changes was destructive"
    );

    // Adapter is partially applied.
    assert_eq!(
        adapter.ops_applied, 2,
        "adapter applied exactly 2 of 3 changes before failing"
    );
    assert_eq!(
        adapter.applied.len(),
        2,
        "adapter recorded 2 partial changes"
    );

    // No way to retry: drain_changes returns empty.
    let retry_changes = model.drain_changes();
    assert!(retry_changes.is_empty(), "no changes remain to retry with");

    // POST-M1R-01: In the revisioned protocol, changes survive apply errors.
    // The journal retains the batch. SyncCoordinator uses cursor
    // acknowledgement: failed adapters retry from their last acknowledged
    // position without losing data.
}

// =========================================================================
// Test 2: Second adapter cannot observe consumed changes
// =========================================================================

/// POST-M1R-01: This test documents the legacy drain_changes() behavior.
///
/// In the revisioned protocol, SyncCoordinator supports independent cursors.
/// Both adapters can observe the same changes because the journal is not
/// consumed on first read — each cursor tracks its own acknowledgement.
#[ignore = "resolved in M1R-01 — drain_changes removal"]
#[test]
fn two_adapters_cannot_both_sync_same_changes() {
    let mut model = Model::new();
    let _x = model.add_var();
    let _y = model.add_var();
    let _z = model.add_var();
    // 3 changes: VariableAdded x3

    let mut adapter_a = RecordingAdapter::new();
    let mut adapter_b = RecordingAdapter::new();

    // Adapter A drains and applies all 3 changes.
    let changes_a = model.drain_changes();
    assert_eq!(changes_a.len(), 3);
    adapter_a.apply_changes(&changes_a).unwrap();

    assert_eq!(adapter_a.applied.len(), 3, "adapter A received 3 changes");

    // Adapter B tries to sync — model has no more changes.
    let changes_b = model.drain_changes();
    assert!(
        changes_b.is_empty(),
        "adapter B gets nothing — changes were consumed by adapter A"
    );
    adapter_b.apply_changes(&changes_b).unwrap();

    assert_eq!(adapter_b.applied.len(), 0, "adapter B applied 0 changes");

    // POST-M1R-01: SyncCoordinator supports independent cursors. Both
    // adapters receive the same changes because each cursor tracks its
    // own acknowledgement position in the journal.
}

/// POST-M1R-01: This test documents the legacy drain_changes() behavior.
///
/// In the revisioned protocol, sync_model does not destructively consume
/// the journal. Both adapters observe the same mutations through their
/// independent cursors.
#[ignore = "resolved in M1R-01 — drain_changes removal"]
#[test]
fn sync_model_leaves_nothing_for_second_adapter() {
    let mut model = Model::new();
    let _x = model.add_var();
    let _y = model.add_var();
    // 2 changes

    let mut adapter_a = RecordingAdapter::new();
    let mut adapter_b = RecordingAdapter::new();

    // First adapter syncs — drain_changes consumed.
    adapter_a.sync_model(&mut model).unwrap();
    assert_eq!(
        adapter_a.applied.len(),
        2,
        "adapter A received 2 changes via sync_model"
    );

    // Second adapter syncs — nothing left.
    adapter_b.sync_model(&mut model).unwrap();
    assert_eq!(
        adapter_b.applied.len(),
        0,
        "adapter B received 0 changes — changelog was already drained"
    );

    // POST-M1R-01: sync_model uses cursor-based sync, not destructive drain.
    // Both adapters observe the same mutations via independent cursors into
    // the journal.
}

// =========================================================================
// Test 3: Partial application leaves no recovery path
// =========================================================================

/// POST-M1R-01: This test documents the legacy drain_changes() behavior.
///
/// In the revisioned protocol, ApplyOutcome::RequiresRebuild provides a
/// deterministic recovery path. The SyncCoordinator tracks the adapter's
/// acknowledged revision and can replay unacknowledged changes from the
/// journal after a reset.
#[ignore = "resolved in M1R-01 — drain_changes removal"]
#[test]
fn no_recovery_path_after_partial_apply() {
    let mut model = Model::new();
    let _x = model.add_var();
    let _y = model.add_variable(Bounds::new(0.0, 5.0), VarType::Continuous);
    let _z = model.add_var();
    // 3 changes

    let mut adapter = FailingAfterAdapter::new(2);

    let changes = model.drain_changes();
    assert_eq!(changes.len(), 3);

    let result = adapter.apply_changes(&changes);
    assert!(result.is_err(), "adapter should fail mid-batch");

    // State 1: model has no recoverable changes.
    assert!(!model.has_pending_changes());

    // State 2: adapter is partially mutated.
    assert_eq!(adapter.applied.len(), 2);

    // Attempted recovery approaches (all flawed):

    // --- Approach A: call sync_model again (gets nothing) ---
    let nop_changes = model.drain_changes();
    assert!(nop_changes.is_empty(), "retry gets nothing");

    // --- Approach B: reset adapter and sync (still gets nothing) ---
    adapter.reset();
    assert_eq!(adapter.reset_count, 1);
    assert!(adapter.applied.is_empty(), "adapter state cleared");

    let retry_changes = model.drain_changes();
    assert!(
        retry_changes.is_empty(),
        "even after reset, model has no changes to offer"
    );

    // POST-M1R-01: ApplyOutcome::RequiresRebuild provides recovery. The
    // SyncCoordinator replays unacknowledged changes from the journal after
    // reset, avoiding the manual full-rebuild approach required here.
}

// =========================================================================
// Test 4: Reset/rebuild not tied to revision
// =========================================================================

/// POST-M1R-01: This test documents the legacy drain_changes() behavior.
///
/// In the revisioned protocol, AdapterCursor tracks applied_revision. After
/// reset, the SyncCoordinator can check whether the adapter's cursor revision
/// matches the model's current revision and trigger a rebuild if needed.
#[ignore = "resolved in M1R-01 — drain_changes removal"]
#[test]
fn reset_has_no_revision_check() {
    let mut model = Model::new();
    let _x = model.add_var();
    let _y = model.add_var();
    // 2 changes: VariableAdded x2

    let mut adapter = RecordingAdapter::new();
    adapter.sync_model(&mut model).unwrap();
    assert_eq!(
        adapter.applied.len(),
        2,
        "adapter applied 2 changes from model"
    );

    // After sync, model has no more pending changes.
    assert!(!model.has_pending_changes());

    // Reset the adapter — it clears its state.
    adapter.reset();
    assert_eq!(adapter.reset_count, 1);
    assert!(adapter.applied.is_empty(), "adapter state cleared");

    // There is no way to check if the adapter is synchronized with the model.
    // - adapter.sync_model(&mut model) would get 0 changes (already drained).
    // - The model has no revision counter accessible to the adapter.
    // - SolverAdapter has no `is_synchronized` method.
    // - The caller must remember whether a rebuild is needed.
    //
    // This test documents the absence of any such check.

    // Calling sync_model again produces empty batch — no rebuild trigger.
    let empty_changes = model.drain_changes();
    assert!(
        empty_changes.is_empty(),
        "no changes remain — sync_model won't help reset adapter"
    );

    // POST-M1R-01: AdapterCursor tracks applied_revision. SyncCoordinator
    // compares the adapter's cursor revision against the model's current
    // revision to detect staleness and trigger RequiresRebuild when needed.
}

/// POST-M1R-01: This test documents the legacy drain_changes() behavior.
///
/// In the revisioned protocol, cursor revision comparison detects staleness.
/// After mutation, the model's current revision advances. The adapter's
/// cursor revision is compared against the model's revision — if they
/// diverge, the adapter is detected as stale and RequiresRebuild or
/// IncrementalSync is triggered automatically.
#[ignore = "resolved in M1R-01 — drain_changes removal"]
#[test]
fn no_staleness_detection_after_mutation() {
    let mut model = Model::new();
    let _x = model.add_var();
    let _y = model.add_var();
    // 2 changes

    let mut adapter = RecordingAdapter::new();
    adapter.sync_model(&mut model).unwrap();
    assert_eq!(adapter.applied.len(), 2);

    // Model mutates again without notifying the adapter.
    let _z = model.add_var();

    // There is no method on SolverAdapter or Model to check:
    // - "is the adapter up to date?"
    // - "what revision is the adapter at?"
    // - "does the model have changes the adapter hasn't seen?"
    //
    // The caller must infer staleness by checking has_pending_changes.
    assert!(
        model.has_pending_changes(),
        "model has pending changes the adapter hasn't seen"
    );

    // The only option is to sync again — but this is fragile because
    // the caller can forget to sync, and there's no compile-time
    // or runtime guard against stale reads.
    adapter.sync_model(&mut model).unwrap();
    assert_eq!(
        adapter.applied.len(),
        3,
        "adapter now has 3 changes after manual re-sync"
    );

    // POST-M1R-01: Cursor revision comparison detects staleness. After
    // mutation, the model's current revision advances. The adapter's cursor
    // revision is compared against the model's revision. Divergence triggers
    // auto-staleness detection, eliminating the fragile manual re-sync.
}
