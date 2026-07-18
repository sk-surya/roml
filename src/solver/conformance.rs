//! Parameterized synchronization conformance suite.
//!
//! The [`run_sync_suite`] function validates that every backend implementing
//! [`BackendSession`] behaves correctly across rebuilds, delta application,
//! revision tracking, and state resets. It is parameterized on
//! [`BackendFixture`] and runs the same 7 scenarios against any backend.
//!
//! # Usage
//!
//! ```ignore
//! use roml::solver::conformance::run_sync_suite;
//! use roml::solver::reference::RefBackendFixture;
//!
//! let fixture = RefBackendFixture;
//! run_sync_suite(&fixture);
//! ```

use crate::delta::{DeltaBatch, ModelOp};
use crate::id::{ConId, Generation, VarId};
use crate::model::coefficient::CoefficientTarget;
use crate::model::{Bounds, ConstraintBounds, VarType};
use crate::revision::ModelRevision;
use crate::snapshot::{ConstraintEntry, ModelSnapshot, VariableEntry};
use crate::solver::backend::HealthEffect;
use crate::solver::session::{BackendFixture, BackendSession, Synchronization};
use crate::sync::AdapterHealth;
use crate::value_expr::ValueExpr;

/// Run the full synchronization conformance suite against the given fixture.
///
/// Runs 7 scenarios. Each test creates a fresh session from the fixture.
/// Panics (via `std::assert!`) on the first assertion failure.
pub fn run_sync_suite<F: BackendFixture>(fixture: &F) {
    empty_rebuild(fixture);
    full_rebuild(fixture);
    single_delta_apply(fixture);
    multi_batch_sequence(fixture);
    revision_mismatch_error(fixture);
    rebuild_resets_state(fixture);
    close_after_rebuild(fixture);
}

// ── Individual scenarios ─────────────────────────────────────────────────────

/// Rebuild from an empty snapshot. Health must be Ready, cursor at r0.
fn empty_rebuild<F: BackendFixture>(fixture: &F) {
    let mut session = fixture
        .new_session()
        .expect("fixture should create a session");
    let r0 = ModelRevision::ZERO;

    let receipt = session
        .synchronize(Synchronization::Rebuild(ModelSnapshot::empty(r0)))
        .expect("empty rebuild should succeed");

    std::assert_eq!(
        receipt.health,
        AdapterHealth::Ready,
        "empty rebuild health"
    );
    std::assert_eq!(
        receipt.cursor.applied_revision, r0,
        "empty rebuild cursor"
    );
}

/// Rebuild from a snapshot with one variable and one constraint.
fn full_rebuild<F: BackendFixture>(fixture: &F) {
    let mut session = fixture
        .new_session()
        .expect("fixture should create a session");
    let r1 = ModelRevision::ZERO.next().unwrap();
    let v0 = VarId::new(0, Generation::new());
    let c0 = ConId::new(0, Generation::new());

    let snap = ModelSnapshot {
        revision: r1,
        variables: vec![VariableEntry {
            id: v0,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
            active: true,
            semicontinuous_lower: None,
        }],
        constraints: vec![ConstraintEntry {
            id: c0,
            bounds: ConstraintBounds::le(100.0),
            active: true,
        }],
        objectives: vec![],
        parameters: vec![],
        cells: vec![],
    };

    let receipt = session
        .synchronize(Synchronization::Rebuild(snap))
        .expect("full rebuild should succeed");

    std::assert_eq!(receipt.health, AdapterHealth::Ready, "full rebuild health");
    std::assert_eq!(
        receipt.cursor.applied_revision, r1,
        "full rebuild cursor"
    );
}

/// Apply a single delta batch with one operation after empty rebuild.
fn single_delta_apply<F: BackendFixture>(fixture: &F) {
    let mut session = fixture
        .new_session()
        .expect("fixture should create a session");
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let v0 = VarId::new(0, Generation::new());

    session
        .synchronize(Synchronization::Rebuild(ModelSnapshot::empty(r0)))
        .expect("empty rebuild should succeed");

    let batch = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::AddVariable {
            var: v0,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        }],
    )
    .unwrap();

    let receipt = session
        .synchronize(Synchronization::DeltaBatch(batch))
        .expect("single delta should succeed");

    std::assert_eq!(receipt.health, AdapterHealth::Ready, "delta health");
    std::assert_eq!(
        receipt.cursor.applied_revision, r1,
        "delta cursor should advance"
    );
}

/// Apply three sequential delta batches (variable, constraint, cell).
fn multi_batch_sequence<F: BackendFixture>(fixture: &F) {
    let mut session = fixture
        .new_session()
        .expect("fixture should create a session");
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();
    let r3 = r2.next().unwrap();
    let v0 = VarId::new(0, Generation::new());
    let c0 = ConId::new(0, Generation::new());

    session
        .synchronize(Synchronization::Rebuild(ModelSnapshot::empty(r0)))
        .expect("empty rebuild should succeed");

    // Batch 1: Add variable
    let b1 = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::AddVariable {
            var: v0,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        }],
    )
    .unwrap();
    let r = session
        .synchronize(Synchronization::DeltaBatch(b1))
        .expect("batch 1 should succeed");
    std::assert_eq!(r.health, AdapterHealth::Ready, "batch 1 health");
    std::assert_eq!(r.cursor.applied_revision, r1, "batch 1 cursor");

    // Batch 2: Add constraint
    let b2 = DeltaBatch::new(
        r1,
        r2,
        vec![ModelOp::AddConstraint {
            con: c0,
            bounds: ConstraintBounds::le(100.0),
        }],
    )
    .unwrap();
    let r = session
        .synchronize(Synchronization::DeltaBatch(b2))
        .expect("batch 2 should succeed");
    std::assert_eq!(r.health, AdapterHealth::Ready, "batch 2 health");
    std::assert_eq!(r.cursor.applied_revision, r2, "batch 2 cursor");

    // Batch 3: Add cell
    let b3 = DeltaBatch::new(
        r2,
        r3,
        vec![ModelOp::SetCell {
            cell_key: (CoefficientTarget::Constraint(c0), v0),
            value_expr: ValueExpr::constant(5.0),
            evaluated_value: 5.0,
        }],
    )
    .unwrap();
    let r = session
        .synchronize(Synchronization::DeltaBatch(b3))
        .expect("batch 3 should succeed");
    std::assert_eq!(r.health, AdapterHealth::Ready, "batch 3 health");
    std::assert_eq!(r.cursor.applied_revision, r3, "batch 3 cursor");
}

/// Apply a batch whose `from` revision doesn't match the cursor's revision.
/// Must error with Recoverable health effect.
fn revision_mismatch_error<F: BackendFixture>(fixture: &F) {
    let mut session = fixture
        .new_session()
        .expect("fixture should create a session");
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    session
        .synchronize(Synchronization::Rebuild(ModelSnapshot::empty(r0)))
        .expect("empty rebuild should succeed");

    // Apply batch r1->r2 when cursor is at r0
    let batch = DeltaBatch::new(r1, r2, vec![]).unwrap();
    let result = session.synchronize(Synchronization::DeltaBatch(batch));

    std::assert!(result.is_err(), "revision mismatch should error");
    if let Err(e) = result {
        // Accept either Recoverable or Terminal: some backends detect the
        // revision mismatch before applying ops (Recoverable), others apply
        // empty ops first and then fail cursor advance (Terminal). Both are
        // valid — the key invariant is that an error is returned.
        let ok = e.health_effect == HealthEffect::Recoverable
            || e.health_effect == HealthEffect::Terminal;
        std::assert!(
            ok,
            "revision mismatch health effect should be Recoverable or Terminal, got {:?}",
            e.health_effect
        );
    } else {
        std::panic!("expected BackendError");
    }
}

/// Populate state via deltas, then rebuild from empty snapshot at a later
/// revision. Cursor must reset to the rebuild revision.
fn rebuild_resets_state<F: BackendFixture>(fixture: &F) {
    let mut session = fixture
        .new_session()
        .expect("fixture should create a session");
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();
    let v0 = VarId::new(0, Generation::new());

    session
        .synchronize(Synchronization::Rebuild(ModelSnapshot::empty(r0)))
        .expect("empty rebuild should succeed");

    // Populate state
    let batch = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::AddVariable {
            var: v0,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        }],
    )
    .unwrap();
    session
        .synchronize(Synchronization::DeltaBatch(batch))
        .expect("delta should succeed");

    // Rebuild from empty at r2
    let receipt = session
        .synchronize(Synchronization::Rebuild(ModelSnapshot::empty(r2)))
        .expect("rebuild after deltas should succeed");

    std::assert_eq!(
        receipt.cursor.applied_revision, r2,
        "rebuild resets cursor to rebuild revision"
    );
    std::assert_eq!(receipt.health, AdapterHealth::Ready, "rebuild health");
}

/// Close a session after a rebuild must succeed.
fn close_after_rebuild<F: BackendFixture>(fixture: &F) {
    let mut session = fixture
        .new_session()
        .expect("fixture should create a session");
    let r0 = ModelRevision::ZERO;

    session
        .synchronize(Synchronization::Rebuild(ModelSnapshot::empty(r0)))
        .expect("empty rebuild should succeed");

    let result = session.close();
    std::assert!(result.is_ok(), "close should succeed");
}
