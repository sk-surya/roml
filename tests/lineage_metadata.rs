//! P25 Task 2 — lineage, instance identity, and metadata.
//!
//! Requirements: SM-02.1 (independent models have distinct opaque
//! `ModelLineageId`; clones preserve lineage), SM-02.7 (every live `Model`
//! has a distinct `ModelInstanceId`; clone allocates a new one), SM-02.3
//! (metadata: description/group/tags/source per entity), SM-02.2 foundations
//! (lineage governs reuse compatibility).

use std::collections::HashSet;

use roml::advanced::CompilationId;
use roml::compiler::capability::{
    BackendCapabilitySet, BackendFeature, FeatureSupport, SupportLevel,
};
use roml::solver::backend::{BackendCapabilities, BackendError, TerminationStatus};
use roml::solver::request::{EffectiveConfig, SolveRequest, SolveResult, SolveSolution};
use roml::solver::session::{
    BackendMetadata, BackendSession, SessionHealth, SyncReceipt, Synchronization,
};
use roml::sync::{AdapterCursor, AdapterHealth};
use roml::{
    continuous, ConstraintExprExt, EntityMetadata, EntityRef, Model, ModelRevision, ModelSource,
    SolveMetadata, SolverSession,
};

#[test]
fn independent_models_never_share_lineage_or_instance() {
    let a = Model::new();
    let b = Model::new();
    assert_ne!(
        a.lineage(),
        b.lineage(),
        "independent models must never share a lineage"
    );
    assert_ne!(
        a.instance(),
        b.instance(),
        "independent models must never share an instance"
    );

    let named = Model::named("independent");
    assert_ne!(
        a.lineage(),
        named.lineage(),
        "named model gets a fresh lineage"
    );
    assert_ne!(
        a.instance(),
        named.instance(),
        "named model gets a fresh instance"
    );
}

#[test]
fn clone_preserves_lineage_but_allocates_new_instance() {
    let mut a = Model::new();
    let x = a.add_variable(continuous()).unwrap();

    let b = a.clone();
    assert_eq!(b.lineage(), a.lineage(), "clone must preserve lineage");
    assert_ne!(
        b.instance(),
        a.instance(),
        "clone must allocate a new instance id"
    );
    // Entity handles survive the clone.
    assert!(b.variable_bounds(x).is_some());

    // A second clone also shares the lineage but has its own instance.
    let c = a.clone();
    assert_eq!(c.lineage(), a.lineage());
    assert_ne!(c.instance(), b.instance());
}

#[test]
fn lineage_and_instance_ids_are_unique_across_many_models() {
    let lineages: HashSet<_> = (0..16).map(|_| Model::new().lineage()).collect();
    assert_eq!(lineages.len(), 16, "all lineages distinct");

    let instances: HashSet<_> = (0..16).map(|_| Model::new().instance()).collect();
    assert_eq!(instances.len(), 16, "all instances distinct");
}

#[test]
fn metadata_round_trips_per_entity() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let c = model.add_constraint((x).le(5.0)).unwrap();
    let obj = model.maximize(x).unwrap();
    let p = model.add_parameter(2.0).unwrap();

    let src = ModelSource {
        module: Some("mymod".to_string()),
        file: Some("model.rs".to_string()),
        line: Some(42),
        external_key: Some("ext-1".to_string()),
    };
    let meta = EntityMetadata {
        description: Some("a variable".to_string()),
        group: Some("g1".to_string()),
        tags: vec!["alpha".to_string(), "beta".to_string()],
        source: Some(src.clone()),
    };

    // Variable metadata round-trips in full.
    model
        .set_metadata(EntityRef::Variable(x), meta.clone())
        .unwrap();
    assert_eq!(model.metadata(EntityRef::Variable(x)), Some(&meta));

    // Other entities are independent until set.
    assert!(model.metadata(EntityRef::Constraint(c)).is_none());

    // Constraint: partial metadata with default-derived remainder.
    model
        .set_metadata(
            EntityRef::Constraint(c),
            EntityMetadata {
                description: Some("row".to_string()),
                ..EntityMetadata::default()
            },
        )
        .unwrap();
    let c_meta = model.metadata(EntityRef::Constraint(c)).unwrap();
    assert_eq!(c_meta.description.as_deref(), Some("row"));
    assert!(c_meta.group.is_none());
    assert!(c_meta.tags.is_empty());
    assert!(c_meta.source.is_none());

    // Objective: group only.
    model
        .set_metadata(
            EntityRef::Objective(obj),
            EntityMetadata {
                group: Some("objg".to_string()),
                ..EntityMetadata::default()
            },
        )
        .unwrap();
    assert_eq!(
        model
            .metadata(EntityRef::Objective(obj))
            .unwrap()
            .group
            .as_deref(),
        Some("objg")
    );

    // Parameter: full default round-trips.
    model
        .set_metadata(EntityRef::Parameter(p), EntityMetadata::default())
        .unwrap();
    assert_eq!(
        model.metadata(EntityRef::Parameter(p)),
        Some(&EntityMetadata::default())
    );

    // Metadata changes are canonical but non-solver-affecting: the revision
    // and changelog must not advance (EXECUTION.md "Incremental semantics").
    let revision_before = model.current_revision();
    model
        .set_metadata(EntityRef::Variable(x), EntityMetadata::default())
        .unwrap();
    assert_eq!(
        model.current_revision(),
        revision_before,
        "metadata changes must not advance the model revision"
    );

    // Removal returns the stored metadata and clears the entry.
    let removed = model.remove_metadata(EntityRef::Variable(x));
    assert_eq!(removed, Some(EntityMetadata::default()));
    assert!(model.metadata(EntityRef::Variable(x)).is_none());
}

#[test]
fn solve_metadata_records_every_state_id() {
    let model = Model::new();
    let meta = SolveMetadata {
        model_lineage: model.lineage(),
        model_instance: model.instance(),
        model_revision: ModelRevision::from_u64(3),
        ..SolveMetadata::default()
    };

    assert_eq!(meta.model_lineage, model.lineage());
    assert_eq!(meta.model_instance, model.instance());
    assert_eq!(meta.model_revision, ModelRevision::from_u64(3));

    // Two default metadata values carry distinct allocated ids (no sentinel).
    let other = SolveMetadata::default();
    assert_ne!(meta.model_lineage, other.model_lineage);
    assert_ne!(meta.model_instance, other.model_instance);
}

// ── Minimal session-trait backend for CR-02 ──────────────────────────────────
//
// A deterministic in-memory backend that accepts any synchronization and
// reports an Optimal result, so the REAL `SolverSession::solve` path (commit →
// sync → solve → normalize) is exercised. It deliberately carries no model
// identity: the metadata binding is the facade's job, not the backend's.

/// The full M2-native typed capability surface (F3 default for test backends).
fn full_typed_capabilities() -> BackendCapabilitySet {
    let mut set = BackendCapabilitySet::new();
    for feature in [
        BackendFeature::Lp,
        BackendFeature::Mip,
        BackendFeature::IncrementalBounds,
        BackendFeature::IncrementalRows,
        BackendFeature::IncrementalCoefficients,
    ] {
        set.set(
            feature,
            FeatureSupport {
                level: SupportLevel::Native,
                limitations: Default::default(),
            },
        );
    }
    set
}

/// A backend satisfying the session traits with no model identity of its own.
struct LineageTestBackend {
    revision: ModelRevision,
    /// The exact `CompilationId` of the compiled state held after the most
    /// recent compiled synchronization (F2 / SM-03.9).
    current_compilation: Option<CompilationId>,
    /// The authoritative typed capability set (F3, SM-04.1).
    typed_caps: BackendCapabilitySet,
}

impl LineageTestBackend {
    fn new() -> Self {
        Self {
            revision: ModelRevision::ZERO,
            current_compilation: None,
            typed_caps: full_typed_capabilities(),
        }
    }
}

impl BackendSession for LineageTestBackend {
    fn synchronize(&mut self, sync: Synchronization) -> Result<SyncReceipt, BackendError> {
        let revision = match sync {
            Synchronization::DeltaBatch(batch) => batch.to,
            Synchronization::Rebuild(snapshot) => snapshot.revision,
            Synchronization::CompiledDeltaBatch(batch) => {
                self.current_compilation = Some(batch.to_compilation);
                batch.to_revision
            }
            Synchronization::CompiledRebuild(snapshot) => {
                self.current_compilation = Some(snapshot.compilation_id);
                snapshot.source_revision
            }
        };
        self.revision = revision;
        Ok(SyncReceipt {
            cursor: AdapterCursor {
                applied_revision: revision,
                health: AdapterHealth::Ready,
            },
            health: AdapterHealth::Ready,
        })
    }

    fn solve(&mut self, _request: &SolveRequest) -> Result<SolveResult, BackendError> {
        Ok(SolveResult {
            effective_configuration: EffectiveConfig::default(),
            termination: TerminationStatus::Optimal,
            solution: Some(SolveSolution {
                variable_values: vec![],
                objective_value: Some(0.0),
                dual_values: None,
                reduced_costs: None,
            }),
            // F5: a real solve always populates `Some`.
            compilation_id: Some(
                self.current_compilation
                    .expect("a solve must follow a compiled synchronization"),
            ),
        })
    }

    fn close(self) -> Result<(), BackendError> {
        Ok(())
    }
}

impl SessionHealth for LineageTestBackend {
    fn health(&self) -> AdapterHealth {
        AdapterHealth::Ready
    }

    fn revision(&self) -> ModelRevision {
        self.revision
    }
}

impl BackendMetadata for LineageTestBackend {
    fn name(&self) -> &str {
        "LineageTestBackend"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::all()
    }

    fn typed_capabilities(&self) -> &BackendCapabilitySet {
        &self.typed_caps
    }
}

/// CR-02: the REAL solve path must bind the solved model's lineage/instance
/// ids into the solution metadata (SM-02.7). Before the fix,
/// `normalize_result` filled them from `..SolveMetadata::default()`, which
/// allocates fresh unrelated global-counter ids on every solve, so this
/// comparison always failed.
#[test]
fn real_solve_binds_model_lineage_and_instance_into_metadata() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    model.add_constraint((x).le(10.0)).unwrap();
    model.maximize(x).unwrap();

    let mut session = SolverSession::new(LineageTestBackend::new());
    let solution = session.solve(&mut model).expect("reference solve succeeds");
    assert_eq!(
        solution.metadata().model_lineage,
        model.lineage(),
        "solution lineage must be the solved model's lineage"
    );
    assert_eq!(
        solution.metadata().model_instance,
        model.instance(),
        "solution instance must be the solved model's instance"
    );
}

/// WR-05: `set_metadata` must reject a stale/removed entity with a typed error
/// instead of silently storing orphaned metadata (the entity stores are the
/// liveness authority).
#[test]
fn set_metadata_rejects_stale_entities() {
    use roml::ModelError;
    let mut model = Model::new();
    let meta = EntityMetadata::default();

    let x = model.add_variable(continuous()).unwrap();
    model.remove_variable(x).unwrap();
    assert!(matches!(
        model.set_metadata(EntityRef::Variable(x), meta.clone()),
        Err(ModelError::VariableNotFound(_))
    ));

    let y = model.add_variable(continuous()).unwrap();
    let c = model.add_constraint((y).le(5.0)).unwrap();
    model.remove_constraint(c).unwrap();
    assert!(matches!(
        model.set_metadata(EntityRef::Constraint(c), meta.clone()),
        Err(ModelError::ConstraintNotFound(_))
    ));

    let obj = model.maximize(y).unwrap();
    model.remove_objective(obj).unwrap();
    assert!(matches!(
        model.set_metadata(EntityRef::Objective(obj), meta.clone()),
        Err(ModelError::ObjectiveNotFound(_))
    ));
}

/// WR-05: removing an entity cascades its metadata, leaving no orphaned
/// entries behind (add/remove churn cannot grow the metadata map without
/// bound).
#[test]
fn removing_entity_cascades_metadata() {
    let mut model = Model::new();

    let x = model.add_variable(continuous()).unwrap();
    model
        .set_metadata(EntityRef::Variable(x), EntityMetadata::default())
        .unwrap();
    assert!(model.metadata(EntityRef::Variable(x)).is_some());
    model.remove_variable(x).unwrap();
    assert!(
        model.metadata(EntityRef::Variable(x)).is_none(),
        "variable metadata cascaded on removal"
    );

    let y = model.add_variable(continuous()).unwrap();
    let c = model.add_constraint((y).le(5.0)).unwrap();
    model
        .set_metadata(EntityRef::Constraint(c), EntityMetadata::default())
        .unwrap();
    model.remove_constraint(c).unwrap();
    assert!(
        model.metadata(EntityRef::Constraint(c)).is_none(),
        "constraint metadata cascaded on removal"
    );

    let obj = model.maximize(y).unwrap();
    model
        .set_metadata(EntityRef::Objective(obj), EntityMetadata::default())
        .unwrap();
    model.remove_objective(obj).unwrap();
    assert!(
        model.metadata(EntityRef::Objective(obj)).is_none(),
        "objective metadata cascaded on removal"
    );
}
