//! P23 Task 2 — The `roml::advanced` namespace is the documented home for
//! backend-extension types (API-07.3, D9): the backend contract, revisions,
//! snapshots, deltas, cursors, capabilities, callbacks, and raw IDs. A backend
//! author can implement the frozen [`BackendSession`] contract using only this
//! namespace plus the golden model vocabulary, without importing the default
//! prelude's protocol items (which are absent there — API-07.2).

use roml::advanced::*;
use roml::compiler::capability::{BackendCapabilitySet, BackendFeature, FeatureSupport};
use roml::model::{Bounds, Sense, VarType};

/// A minimal backend-author session implementing the frozen contract.
struct MiniSession {
    revision: ModelRevision,
    health: AdapterHealth,
}

impl BackendSession for MiniSession {
    fn synchronize(&mut self, sync: Synchronization) -> Result<SyncReceipt, BackendError> {
        match sync {
            Synchronization::DeltaBatch(batch) => {
                assert!(!batch.operations.is_empty());
                self.revision = batch.to;
            }
            Synchronization::Rebuild(snapshot) => {
                self.revision = snapshot.revision;
            }
        }
        self.health = AdapterHealth::Ready;
        Ok(SyncReceipt {
            cursor: AdapterCursor {
                applied_revision: self.revision,
                health: self.health,
            },
            health: self.health,
        })
    }

    fn solve(&mut self, request: &SolveRequest) -> Result<SolveResult, BackendError> {
        let _ = request;
        Err(BackendError::new(
            "no-op session",
            ErrorCategory::Unsupported,
            HealthEffect::None,
        ))
    }

    fn close(self) -> Result<(), BackendError> {
        Ok(())
    }
}

impl SessionHealth for MiniSession {
    fn health(&self) -> AdapterHealth {
        self.health
    }
    fn revision(&self) -> ModelRevision {
        self.revision
    }
}

impl BackendMetadata for MiniSession {
    fn name(&self) -> &str {
        "MiniSession"
    }
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::all()
    }
}

/// A backend author can implement the frozen contract and synchronize a delta
/// batch using only `roml::advanced`.
#[test]
fn backend_contract_implementable_from_advanced() {
    let mut session = MiniSession {
        revision: ModelRevision::ZERO,
        health: AdapterHealth::Ready,
    };
    let r1 = ModelRevision::from_u64(1);
    let batch = DeltaBatch::new(
        ModelRevision::ZERO,
        r1,
        vec![ModelOp::AddVariable {
            var: VarId::new(0, Generation::new()),
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        }],
    )
    .expect("valid batch");

    let receipt = session
        .synchronize(Synchronization::DeltaBatch(batch))
        .expect("sync");
    assert_eq!(receipt.cursor.applied_revision, r1);
    assert_eq!(receipt.health, AdapterHealth::Ready);
    assert_eq!(session.revision(), r1);
}

/// Raw IDs, the change journal, expression internals, constraint-bounds form,
/// solve-request protocol, and solution-construction internals are all
/// reachable from `roml::advanced`.
#[test]
fn advanced_exposes_protocol_and_id_vocabulary() {
    let var = VarId::new(0, Generation::new());
    let _coeff: CoeffId = CoeffId::new(0, Generation::new());

    let _change: Change = Change::VariableAdded {
        var,
        bounds: Bounds::BINARY,
        var_type: VarType::Binary,
    };

    let _expr: ValueExpr = ValueExpr::constant(1.0);
    let _bounds: ConstraintBounds = ConstraintBounds::le(4.0);
    let _sense: Sense = Sense::Maximize;
    let _status: TerminationStatus = TerminationStatus::Optimal;
    let _algo: LpAlgorithm = LpAlgorithm::DualSimplex;
    let _sync_mode: SynchronizationMode = SynchronizationMode::NoChange;
    let _metadata = SolveMetadata::default();
    let _cursor = AdapterCursor::new();
    let _coordinator = SyncCoordinator::new();
    let _builder = SolutionBuilder::new();
    let _store = SolutionStore::new();

    // Solve-request protocol lives here for backend authors.
    let request = SolveRequest::new()
        .with_lp_algorithm(LpAlgorithm::Barrier)
        .with_time_limit(30.0);
    let _capabilities: BackendCapabilities = BackendCapabilities::all();
    // validate_request now validates against the typed capability set.
    let mut typed_capabilities = BackendCapabilitySet::new();
    typed_capabilities.set(
        BackendFeature::Lp,
        FeatureSupport::native(Default::default()),
    );
    typed_capabilities.set(
        BackendFeature::Mip,
        FeatureSupport::native(Default::default()),
    );
    let _rejections = validate_request(&request, &typed_capabilities);
    assert!(matches!(_status, TerminationStatus::Optimal));
}
