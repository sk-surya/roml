//! P25 Task 2 — lineage, instance identity, and metadata.
//!
//! Requirements: SM-02.1 (independent models have distinct opaque
//! `ModelLineageId`; clones preserve lineage), SM-02.7 (every live `Model`
//! has a distinct `ModelInstanceId`; clone allocates a new one), SM-02.3
//! (metadata: description/group/tags/source per entity), SM-02.2 foundations
//! (lineage governs reuse compatibility).

use std::collections::HashSet;

use roml::{
    continuous, ConstraintExprExt, EntityMetadata, EntityRef, Model, ModelRevision, ModelSource,
    SolveMetadata,
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
    model.set_metadata(EntityRef::Variable(x), meta.clone());
    assert_eq!(model.metadata(EntityRef::Variable(x)), Some(&meta));

    // Other entities are independent until set.
    assert!(model.metadata(EntityRef::Constraint(c)).is_none());

    // Constraint: partial metadata with default-derived remainder.
    model.set_metadata(
        EntityRef::Constraint(c),
        EntityMetadata {
            description: Some("row".to_string()),
            ..EntityMetadata::default()
        },
    );
    let c_meta = model.metadata(EntityRef::Constraint(c)).unwrap();
    assert_eq!(c_meta.description.as_deref(), Some("row"));
    assert!(c_meta.group.is_none());
    assert!(c_meta.tags.is_empty());
    assert!(c_meta.source.is_none());

    // Objective: group only.
    model.set_metadata(
        EntityRef::Objective(obj),
        EntityMetadata {
            group: Some("objg".to_string()),
            ..EntityMetadata::default()
        },
    );
    assert_eq!(
        model
            .metadata(EntityRef::Objective(obj))
            .unwrap()
            .group
            .as_deref(),
        Some("objg")
    );

    // Parameter: full default round-trips.
    model.set_metadata(EntityRef::Parameter(p), EntityMetadata::default());
    assert_eq!(
        model.metadata(EntityRef::Parameter(p)),
        Some(&EntityMetadata::default())
    );

    // Metadata changes are canonical but non-solver-affecting: the revision
    // and changelog must not advance (EXECUTION.md "Incremental semantics").
    let revision_before = model.current_revision();
    model.set_metadata(EntityRef::Variable(x), EntityMetadata::default());
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
