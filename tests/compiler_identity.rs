//! P26 Task 5 — backend IR and exact compilation identity
//! (SM-02.4, SM-02.5, SM-03.3 extension surface, SM-03.5, SM-03.6, SM-03.9,
//! SM-13 compiler foundations).
//!
//! This suite pins the compiler boundary shapes from the approved design
//! (§4 identity, §8.3–8.5 backend IR) and the M3 packet interface contract:
//! dense deterministic compiled IDs; a unique checked `CompilationId` per
//! compiled state (zero reserved, typed overflow, never a wrap); builder
//! finalization that rejects any generated entity without a recorded origin
//! (D5, SM-02.5 — the Task 5 stopping condition); exact from/to compilation
//! ids on every `BackendDeltaBatch`; bidirectional `OriginMap` queries plus a
//! completeness validator; and a deterministic `RecipeFingerprint` that is
//! evidence/cache only and never stale-state authority (D28, SM-03.9).

use roml::advanced::{
    BackendDeltaBatch, BackendOp, BackendSnapshot, BackendSnapshotBuilder, CompileError,
    CompiledConstraintId, CompiledEntityRef, CompiledLinearRow, CompiledObjective,
    CompiledObjectiveId, CompiledObjectiveLevel, CompiledObjectivePolicy, CompiledVariable,
    CompiledVariableId, CompiledWeightedObjective, EntityOrigin, OriginMap,
};
use roml::id::Generation;
use roml::{Bounds, ConId, ConstraintBounds, Model, ModelRevision, ObjId, Sense, VarId, VarType};

/// Build a one-variable / one-row / one-objective compiled state with complete
/// origins, compiled from the given `model`'s source identity.
fn linear_snapshot_from(model: &Model) -> Result<BackendSnapshot, CompileError> {
    let instance = model.instance();
    let revision = model.current_revision();

    let var = CompiledVariable {
        id: CompiledVariableId(0),
        bounds: Bounds::NON_NEGATIVE,
        var_type: VarType::Continuous,
        name: None,
    };
    let row = CompiledLinearRow {
        id: CompiledConstraintId(0),
        bounds: ConstraintBounds::le(10.0),
        coefficients: vec![(CompiledVariableId(0), 2.0)],
        name: None,
    };
    let obj = CompiledObjective {
        id: CompiledObjectiveId(0),
        sense: Sense::Minimize,
        coefficients: vec![(CompiledVariableId(0), 1.0)],
        constant: 0.0,
        name: None,
    };

    let mut origins = OriginMap::new();
    origins.insert_variable(
        CompiledVariableId(0),
        EntityOrigin::UserVariable(VarId::new(0, Generation::new())),
    );
    origins.insert_constraint(
        CompiledConstraintId(0),
        EntityOrigin::UserConstraint(ConId::new(0, Generation::new())),
    );
    origins.insert_objective(
        CompiledObjectiveId(0),
        EntityOrigin::UserObjective(ObjId::new(0, Generation::new())),
    );

    BackendSnapshotBuilder::new(instance, revision)
        .origin_map(origins)
        .add_variable(var)
        .add_linear_row(row)
        .add_objective(obj)
        .objective_policy(CompiledObjectivePolicy::Single(CompiledObjectiveId(0)))
        .finalize()
}

/// Build a one-variable / one-row / one-objective compiled state with complete
/// origins. Each call builds an independent `Model` instance (distinct
/// `ModelInstanceId`) so the recipe-fingerprint and compilation-identity
/// properties are exercised against realistic source identity.
fn linear_snapshot() -> Result<BackendSnapshot, CompileError> {
    linear_snapshot_from(&Model::new())
}

// ---------------------------------------------------------------------------
// 1. Dense deterministic compiled IDs
// ---------------------------------------------------------------------------

#[test]
fn compiled_ids_are_dense_deterministic_and_ordered() {
    // The compiled id newtypes wrap dense u32 indices; they are ordered and
    // equality-comparable (SM-02.4: distinct from user handles).
    assert!(CompiledVariableId(0) < CompiledVariableId(1));
    assert_eq!(CompiledVariableId(7), CompiledVariableId(7));
    assert!(CompiledConstraintId(0) < CompiledConstraintId(2));
    assert!(CompiledObjectiveId(0) < CompiledObjectiveId(1));

    // The builder preserves caller-assigned ids exactly (deterministic, no
    // renumbering): a compiled state's ids are the ids that were declared.
    let snap = linear_snapshot().unwrap();
    assert_eq!(snap.variables[0].id, CompiledVariableId(0));
    assert_eq!(snap.linear_rows[0].id, CompiledConstraintId(0));
    assert_eq!(snap.objectives[0].id, CompiledObjectiveId(0));
}

// ---------------------------------------------------------------------------
// 2. CompilationId — unique, checked, never wraps
// ---------------------------------------------------------------------------

#[test]
fn compilation_id_is_unique_per_compiled_state() {
    let a = linear_snapshot().unwrap();
    let b = linear_snapshot().unwrap();
    // D28: every compiled backend state receives a distinct opaque id, even
    // when the recipe content is identical (ids are exact identity, not
    // content hashes).
    assert_ne!(a.compilation_id, b.compilation_id);

    // CompilationId is Copy and value-comparable.
    let copy = a.compilation_id;
    assert_eq!(copy, a.compilation_id);
}

// ---------------------------------------------------------------------------
// 3. Builder finalization — no generated entity without an origin (D5)
// ---------------------------------------------------------------------------

#[test]
fn builder_finalization_rejects_variable_without_origin() {
    let model = Model::new();
    let var = CompiledVariable {
        id: CompiledVariableId(0),
        bounds: Bounds::NON_NEGATIVE,
        var_type: VarType::Continuous,
        name: None,
    };
    let result = BackendSnapshotBuilder::new(model.instance(), model.current_revision())
        .add_variable(var)
        .finalize();
    assert!(matches!(
        result,
        Err(CompileError::MissingOrigin {
            entity: CompiledEntityRef::Variable(CompiledVariableId(0))
        })
    ));
}

#[test]
fn builder_finalization_rejects_unoriginated_row_and_objective() {
    let model = Model::new();

    let row = CompiledLinearRow {
        id: CompiledConstraintId(0),
        bounds: ConstraintBounds::le(5.0),
        coefficients: vec![],
        name: None,
    };
    let err = BackendSnapshotBuilder::new(model.instance(), model.current_revision())
        .add_linear_row(row)
        .finalize()
        .unwrap_err();
    assert_eq!(
        err,
        CompileError::MissingOrigin {
            entity: CompiledEntityRef::Constraint(CompiledConstraintId(0))
        }
    );

    let obj = CompiledObjective {
        id: CompiledObjectiveId(0),
        sense: Sense::Minimize,
        coefficients: vec![],
        constant: 0.0,
        name: None,
    };
    let err = BackendSnapshotBuilder::new(model.instance(), model.current_revision())
        .add_objective(obj)
        .finalize()
        .unwrap_err();
    assert_eq!(
        err,
        CompileError::MissingOrigin {
            entity: CompiledEntityRef::Objective(CompiledObjectiveId(0))
        }
    );
}

// ---------------------------------------------------------------------------
// 4. BackendSnapshot carries identity and objective policy
// ---------------------------------------------------------------------------

#[test]
fn backend_snapshot_stores_identity_revision_and_objective_policy() {
    let model = Model::new();
    let instance = model.instance();
    let revision = model.current_revision();

    let snap = linear_snapshot_from(&model).unwrap();
    assert_eq!(snap.source_instance, instance);
    assert_eq!(snap.source_revision, revision);
    assert_eq!(
        snap.objective_policy,
        CompiledObjectivePolicy::Single(CompiledObjectiveId(0))
    );
    assert_eq!(snap.variables.len(), 1);
    assert_eq!(snap.linear_rows.len(), 1);
    assert_eq!(snap.objectives.len(), 1);
    // F-G: native_constraints stays empty in P26 (the #[non_exhaustive]
    // extension boundary carries the normalized primitives from P32/P33).
    assert!(snap.native_constraints.is_empty());
}

#[test]
fn objective_policy_none_represents_no_active_objective() {
    // A32: `CompiledObjectivePolicy::None` is the compiled representation of
    // the M2 no-active-objective case (B1 resolution), so objective-less
    // canonical state compiles without regressing M2 behavior.
    let model = Model::new();
    let snap = BackendSnapshotBuilder::new(model.instance(), model.current_revision())
        .objective_policy(CompiledObjectivePolicy::None)
        .finalize()
        .unwrap();
    assert_eq!(snap.objective_policy, CompiledObjectivePolicy::None);
}

#[test]
fn objective_policy_validation_rejects_dangling_objective() {
    // The policy must reference a compiled objective that actually exists;
    // `Single(id)` for a never-compiled id is a broken snapshot (design §8.4).
    let model = Model::new();
    let mut origins = OriginMap::new();
    origins.insert_variable(
        CompiledVariableId(0),
        EntityOrigin::UserVariable(VarId::new(0, Generation::new())),
    );
    let var = CompiledVariable {
        id: CompiledVariableId(0),
        bounds: Bounds::NON_NEGATIVE,
        var_type: VarType::Continuous,
        name: None,
    };
    let result = BackendSnapshotBuilder::new(model.instance(), model.current_revision())
        .origin_map(origins)
        .add_variable(var)
        .objective_policy(CompiledObjectivePolicy::Single(CompiledObjectiveId(0)))
        .finalize();
    assert!(matches!(
        result,
        Err(CompileError::InvalidObjectivePolicy(_))
    ));
}

#[test]
fn weighted_and_lexicographic_policies_validate_dangling_ids() {
    // Weighted/Lexicographic are representable now (P31 canonical ObjectivePolicy
    // reaches them later); the builder still rejects dangling ids in them.
    let model = Model::new();
    let err = BackendSnapshotBuilder::new(model.instance(), model.current_revision())
        .objective_policy(CompiledObjectivePolicy::Weighted(vec![
            CompiledWeightedObjective {
                objective: CompiledObjectiveId(0),
                weight: 2.0,
            },
        ]))
        .finalize()
        .unwrap_err();
    assert!(matches!(err, CompileError::InvalidObjectivePolicy(_)));

    let err = BackendSnapshotBuilder::new(model.instance(), model.current_revision())
        .objective_policy(CompiledObjectivePolicy::Lexicographic(vec![
            CompiledObjectiveLevel {
                objective: CompiledObjectiveId(0),
                absolute_tolerance: 1e-6,
                relative_tolerance: 1e-9,
            },
        ]))
        .finalize()
        .unwrap_err();
    assert!(matches!(err, CompileError::InvalidObjectivePolicy(_)));
}

// ---------------------------------------------------------------------------
// 5. BackendDeltaBatch — exact from/to compilation identity
// ---------------------------------------------------------------------------

#[test]
fn backend_delta_batch_carries_exact_from_to_compilation_ids() {
    let a = linear_snapshot().unwrap();
    let b = linear_snapshot().unwrap();
    let batch = BackendDeltaBatch {
        from_compilation: a.compilation_id,
        to_compilation: b.compilation_id,
        from_revision: ModelRevision::from_u64(0),
        to_revision: ModelRevision::from_u64(1),
        operations: vec![BackendOp::SetObjectivePolicy(CompiledObjectivePolicy::None)],
        recipe_fingerprint: b.recipe_fingerprint,
    };
    // B2: every batch carries exact from/to compilation ids and revisions.
    assert_eq!(batch.from_compilation, a.compilation_id);
    assert_eq!(batch.to_compilation, b.compilation_id);
    assert_eq!(batch.from_revision, ModelRevision::from_u64(0));
    assert_eq!(batch.to_revision, ModelRevision::from_u64(1));
    assert_eq!(batch.operations.len(), 1);
}

// ---------------------------------------------------------------------------
// 6. BackendOp — the pinned B3 enumeration (including removal + policy ops)
// ---------------------------------------------------------------------------

#[test]
fn backend_op_enum_includes_removal_and_policy_ops() {
    let a = linear_snapshot().unwrap();
    let b = linear_snapshot().unwrap();
    let ops = vec![
        BackendOp::RemoveLinearCoefficient {
            constraint: CompiledConstraintId(0),
            variable: CompiledVariableId(0),
        },
        BackendOp::RemoveObjectiveCoefficient {
            objective: CompiledObjectiveId(0),
            variable: CompiledVariableId(0),
        },
        BackendOp::SetObjectivePolicy(CompiledObjectivePolicy::None),
        BackendOp::SetObjectiveConstant {
            objective: CompiledObjectiveId(0),
            value: 3.0,
        },
        BackendOp::SetObjectiveSense {
            objective: CompiledObjectiveId(0),
            sense: Sense::Maximize,
        },
        BackendOp::SetLinearRowBounds {
            constraint: CompiledConstraintId(0),
            bounds: ConstraintBounds::ge(1.0),
        },
        BackendOp::SetVariableBounds {
            variable: CompiledVariableId(0),
            bounds: Bounds::UNBOUNDED,
        },
        BackendOp::SetLinearCoefficient {
            constraint: CompiledConstraintId(0),
            variable: CompiledVariableId(0),
            value: 4.0,
        },
    ];
    assert!(ops.iter().any(|op| matches!(
        op,
        BackendOp::RemoveLinearCoefficient {
            constraint: CompiledConstraintId(0),
            variable: CompiledVariableId(0)
        }
    )));
    assert!(ops.iter().any(|op| matches!(
        op,
        BackendOp::RemoveObjectiveCoefficient {
            objective: CompiledObjectiveId(0),
            variable: CompiledVariableId(0)
        }
    )));
    assert!(ops
        .iter()
        .any(|op| matches!(op, BackendOp::SetObjectivePolicy(_))));

    // The op list is part of a delta batch and round-trips through PartialEq.
    let batch = BackendDeltaBatch {
        from_compilation: a.compilation_id,
        to_compilation: b.compilation_id,
        from_revision: ModelRevision::from_u64(0),
        to_revision: ModelRevision::from_u64(1),
        operations: ops.clone(),
        recipe_fingerprint: b.recipe_fingerprint,
    };
    assert_eq!(batch.operations, ops);
}

// ---------------------------------------------------------------------------
// 7. OriginMap — bidirectional queries + completeness validator
// ---------------------------------------------------------------------------

#[test]
fn origin_map_supports_bidirectional_queries() {
    let x = VarId::new(0, Generation::new());
    let c = ConId::new(0, Generation::new());
    let o = ObjId::new(0, Generation::new());

    let mut origins = OriginMap::new();
    origins.insert_variable(CompiledVariableId(0), EntityOrigin::UserVariable(x));
    origins.insert_constraint(CompiledConstraintId(0), EntityOrigin::UserConstraint(c));
    origins.insert_objective(CompiledObjectiveId(0), EntityOrigin::UserObjective(o));

    // compiled -> origin
    assert_eq!(
        origins.variable_origin(CompiledVariableId(0)),
        Some(&EntityOrigin::UserVariable(x))
    );
    assert_eq!(
        origins.constraint_origin(CompiledConstraintId(0)),
        Some(&EntityOrigin::UserConstraint(c))
    );
    assert_eq!(
        origins.objective_origin(CompiledObjectiveId(0)),
        Some(&EntityOrigin::UserObjective(o))
    );

    // origin -> compiled
    assert_eq!(
        origins.variables_for_origin(&EntityOrigin::UserVariable(x)),
        vec![CompiledVariableId(0)]
    );
    assert_eq!(
        origins.constraints_for_origin(&EntityOrigin::UserConstraint(c)),
        vec![CompiledConstraintId(0)]
    );
    assert_eq!(
        origins.objectives_for_origin(&EntityOrigin::UserObjective(o)),
        vec![CompiledObjectiveId(0)]
    );

    // Unified origin -> compiled query spans all entity kinds.
    assert_eq!(
        origins.compiled_for_origin(&EntityOrigin::UserVariable(x)),
        vec![CompiledEntityRef::Variable(CompiledVariableId(0))]
    );

    // Absent queries return empty / None.
    assert_eq!(origins.variable_origin(CompiledVariableId(1)), None);
    assert!(origins
        .variables_for_origin(&EntityOrigin::UserVariable(VarId::new(
            1,
            Generation::new()
        )))
        .is_empty());
}

#[test]
fn origin_map_completeness_validator_flags_unoriginated_entities() {
    let mut origins = OriginMap::new();
    origins.insert_variable(
        CompiledVariableId(0),
        EntityOrigin::UserVariable(VarId::new(0, Generation::new())),
    );

    let unoriginated_var = CompiledVariable {
        id: CompiledVariableId(1),
        bounds: Bounds::NON_NEGATIVE,
        var_type: VarType::Continuous,
        name: None,
    };
    let unoriginated_row = CompiledLinearRow {
        id: CompiledConstraintId(0),
        bounds: ConstraintBounds::le(1.0),
        coefficients: vec![],
        name: None,
    };
    let missing = origins.missing_origins(&[unoriginated_var], &[unoriginated_row], &[]);
    assert_eq!(missing.len(), 2);
    assert!(missing.contains(&CompiledEntityRef::Variable(CompiledVariableId(1))));
    assert!(missing.contains(&CompiledEntityRef::Constraint(CompiledConstraintId(0))));
}

// ---------------------------------------------------------------------------
// 8. RecipeFingerprint — deterministic evidence, never authority (D28)
// ---------------------------------------------------------------------------

#[test]
fn recipe_fingerprints_are_deterministic_but_never_authority() {
    let a = linear_snapshot().unwrap();
    let b = linear_snapshot().unwrap();

    // Equal compiled states -> equal fingerprint. The fingerprint covers the
    // recipe (compiled variables/rows/objectives/policy), so the two builds
    // agree even though they came from distinct Model instances.
    assert_eq!(a.recipe_fingerprint, b.recipe_fingerprint);

    // ... but exact identity still differs: stale-state safety compares the
    // exact CompilationId, never the fingerprint (SM-03.9, D28).
    assert_ne!(a.compilation_id, b.compilation_id);
    assert!(
        a.compilation_id != b.compilation_id,
        "the authoritative stale-state comparison is the compilation id"
    );
}

// ---------------------------------------------------------------------------
// 9. CompilationReport — fingerprint, inventory, formulation decisions
// ---------------------------------------------------------------------------

#[test]
fn compilation_report_records_fingerprint_inventory_and_decisions() {
    let snap = linear_snapshot().unwrap();
    assert_eq!(snap.report.recipe_fingerprint, snap.recipe_fingerprint);
    assert!(snap
        .report
        .generated_entities
        .contains(&CompiledEntityRef::Variable(CompiledVariableId(0))));
    assert!(snap
        .report
        .generated_entities
        .contains(&CompiledEntityRef::Constraint(CompiledConstraintId(0))));
    assert!(snap
        .report
        .generated_entities
        .contains(&CompiledEntityRef::Objective(CompiledObjectiveId(0))));
    // Formulation decisions are recorded (e.g. the objective-policy choice).
    assert!(!snap.report.formulation_decisions.is_empty());
}
