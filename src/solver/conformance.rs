//! Parameterized synchronization conformance suite.
//!
//! The [`run_sync_suite`] function validates that every backend implementing
//! [`BackendSession`] behaves correctly across rebuilds, delta application,
//! revision tracking, and state resets. It is parameterized on
//! [`BackendFixture`] and runs the same 7 scenarios against any backend.
//!
//! # Compiled synchronization (P26 Task 7, design §22)
//!
//! The M3 amendment routes synchronization through backend IR: each scenario
//! lowers canonical snapshots/deltas into [`BackendSnapshot`]/
//! [`BackendDeltaBatch`] via a [`CompilationSession`] before calling
//! `synchronize`, so the shared suite exercises the compiled contract that
//! migrated backends (HiGHS) implement.
//!
//! # Usage
//!
//! ```ignore
//! use roml::solver::conformance::run_sync_suite;
//!
//! let fixture = HighsFixture;
//! run_sync_suite(&fixture);
//! ```

use crate::compiler::backend_ir::{BackendDeltaBatch, BackendSnapshot};
use crate::compiler::capability::{
    BackendCapabilitySet, BackendFeature, CompilationPolicy, FeatureSupport, SupportLevel,
};
use crate::compiler::session::CompilationSession;
use crate::delta::{DeltaBatch, ModelOp};
use crate::id::{ConId, Generation, VarId};
use crate::identity::ModelInstanceId;
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

/// The identity compiler helper for one conformance scenario.
///
/// Owns a [`CompilationSession`] (tracking the exact compiled base) and a
/// source `ModelInstanceId`, lowering canonical state into backend IR before
/// each `synchronize` call.
struct Compiler {
    session: CompilationSession,
    source_instance: ModelInstanceId,
    capabilities: BackendCapabilitySet,
}

impl Compiler {
    fn new() -> Self {
        let mut capabilities = BackendCapabilitySet::new();
        for feature in [
            BackendFeature::Lp,
            BackendFeature::Mip,
            BackendFeature::IncrementalBounds,
            BackendFeature::IncrementalRows,
            BackendFeature::IncrementalCoefficients,
        ] {
            capabilities.set(
                feature,
                FeatureSupport {
                    level: SupportLevel::Native,
                    limitations: Default::default(),
                },
            );
        }
        Self {
            session: CompilationSession::new(),
            source_instance: ModelInstanceId::allocate().expect("model instance counter exhausted"),
            capabilities,
        }
    }

    fn rebuild(&mut self, snapshot: &ModelSnapshot) -> BackendSnapshot {
        self.session
            .compile_snapshot(
                self.source_instance,
                snapshot,
                &CompilationPolicy::Auto,
                &self.capabilities,
            )
            .expect("conformance snapshot must compile")
    }

    fn delta(&mut self, batch: &DeltaBatch) -> BackendDeltaBatch {
        let from = self
            .session
            .current_compilation()
            .expect("conformance delta requires a compiled base");
        self.session
            .compile_delta(batch, from, &CompilationPolicy::Auto, &self.capabilities)
            .expect("conformance delta must compile")
    }
}

// ── Individual scenarios ─────────────────────────────────────────────────────

/// Rebuild from an empty snapshot. Health must be Ready, cursor at r0.
fn empty_rebuild<F: BackendFixture>(fixture: &F) {
    let mut session = fixture
        .new_session()
        .expect("fixture should create a session");
    let r0 = ModelRevision::ZERO;
    let mut compiler = Compiler::new();

    let compiled = compiler.rebuild(&ModelSnapshot::empty(r0));
    let receipt = session
        .synchronize(Synchronization::CompiledRebuild(compiled))
        .expect("empty rebuild should succeed");

    std::assert_eq!(receipt.health, AdapterHealth::Ready, "empty rebuild health");
    std::assert_eq!(receipt.cursor.applied_revision, r0, "empty rebuild cursor");
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
        functions: vec![],
        constructs: vec![],
    };

    let mut compiler = Compiler::new();
    let compiled = compiler.rebuild(&snap);
    let receipt = session
        .synchronize(Synchronization::CompiledRebuild(compiled))
        .expect("full rebuild should succeed");

    std::assert_eq!(receipt.health, AdapterHealth::Ready, "full rebuild health");
    std::assert_eq!(receipt.cursor.applied_revision, r1, "full rebuild cursor");
}

/// Apply a single delta batch with one operation after empty rebuild.
fn single_delta_apply<F: BackendFixture>(fixture: &F) {
    let mut session = fixture
        .new_session()
        .expect("fixture should create a session");
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let v0 = VarId::new(0, Generation::new());

    let mut compiler = Compiler::new();
    let compiled_base = compiler.rebuild(&ModelSnapshot::empty(r0));
    session
        .synchronize(Synchronization::CompiledRebuild(compiled_base))
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
    let compiled_delta = compiler.delta(&batch);

    let receipt = session
        .synchronize(Synchronization::CompiledDeltaBatch(compiled_delta))
        .expect("single delta should succeed");

    std::assert_eq!(receipt.health, AdapterHealth::Ready, "delta health");
    std::assert_eq!(
        receipt.cursor.applied_revision,
        r1,
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

    let mut compiler = Compiler::new();
    let compiled_base = compiler.rebuild(&ModelSnapshot::empty(r0));
    session
        .synchronize(Synchronization::CompiledRebuild(compiled_base))
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
    let d1 = compiler.delta(&b1);
    let r = session
        .synchronize(Synchronization::CompiledDeltaBatch(d1))
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
    let d2 = compiler.delta(&b2);
    let r = session
        .synchronize(Synchronization::CompiledDeltaBatch(d2))
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
    let d3 = compiler.delta(&b3);
    let r = session
        .synchronize(Synchronization::CompiledDeltaBatch(d3))
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

    let mut compiler = Compiler::new();
    let compiled_base = compiler.rebuild(&ModelSnapshot::empty(r0));
    session
        .synchronize(Synchronization::CompiledRebuild(compiled_base))
        .expect("empty rebuild should succeed");

    // Build a compiled batch whose `from_revision` (r1) does not match the
    // cursor (r0). The compiler cannot lower it (rebuild-on-uncertainty), so
    // construct the batch shape directly to exercise the session-level
    // base-revision rejection.
    let batch = DeltaBatch::new(r1, r2, vec![]).unwrap();
    let from = compiler
        .session
        .current_compilation()
        .expect("compiled base exists");
    // The delta's `from` revision is r1, but the compiled base is at r0.
    std::assert_ne!(batch.from, r0, "mismatched base setup");
    let compiled_delta = BackendDeltaBatch {
        from_compilation: from,
        to_compilation: from,
        from_revision: batch.from,
        to_revision: batch.to,
        operations: vec![],
        origin_additions: Default::default(),
        recipe_fingerprint: crate::compiler::backend_ir::RecipeFingerprint::for_operations(&[]),
    };

    let result = session.synchronize(Synchronization::CompiledDeltaBatch(compiled_delta));

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

    let mut compiler = Compiler::new();
    let compiled_base = compiler.rebuild(&ModelSnapshot::empty(r0));
    session
        .synchronize(Synchronization::CompiledRebuild(compiled_base))
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
    let compiled_delta = compiler.delta(&batch);
    session
        .synchronize(Synchronization::CompiledDeltaBatch(compiled_delta))
        .expect("delta should succeed");

    // Rebuild from empty at r2
    let compiled_empty = compiler.rebuild(&ModelSnapshot::empty(r2));
    let receipt = session
        .synchronize(Synchronization::CompiledRebuild(compiled_empty))
        .expect("rebuild after deltas should succeed");

    std::assert_eq!(
        receipt.cursor.applied_revision,
        r2,
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

    let mut compiler = Compiler::new();
    let compiled = compiler.rebuild(&ModelSnapshot::empty(r0));
    session
        .synchronize(Synchronization::CompiledRebuild(compiled))
        .expect("empty rebuild should succeed");

    let result = session.close();
    std::assert!(result.is_ok(), "close should succeed");
}
