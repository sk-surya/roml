//! P26 Task 7 — identity compiler and synchronization migration
//! (SM-03.1, SM-03.5, SM-03.6, SM-03.7; A31, A32, D22, D28).
//!
//! Exercises `CompilationSession` against real `Model` state:
//! one-to-one snapshot compilation with the active `CompiledObjectivePolicy`
//! (A32 `None` for no-active-objective); exact from/to compilation ids on
//! every `BackendDeltaBatch`; rebuild-on-uncertainty (D22/design §18); and
//! A31-aware delta consumption (updates ride the ops, never treating
//! `DeltaBatch.functions` as exhaustive for pre-existing constraints).

use roml::advanced::{
    BackendCapabilitySet, BackendFeature, BackendOp, CompilationSession, CompileError,
    CompiledConstraintId, CompiledObjectiveId, CompiledObjectivePolicy, CompiledVariableId,
    FeatureSupport, SupportLevel,
};
use roml::compiler::capability::CompilationPolicy;
use roml::id::Generation;
use roml::model::coefficient::CoefficientTarget;
use roml::model::{continuous, integer, Bounds, ConstraintBounds, VarType};
use roml::value_expr::ValueExpr;
use roml::{ConstraintExprExt, DeltaBatch, Model, ModelOp};

/// A typed capability set declaring the full M2 primitive surface native.
fn full_capabilities() -> BackendCapabilitySet {
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

/// A capability set lacking the MIP feature (LP-only backend).
fn lp_only_capabilities() -> BackendCapabilitySet {
    let mut set = BackendCapabilitySet::new();
    for feature in [
        BackendFeature::Lp,
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

// ---------------------------------------------------------------------------
// 1. Identity compile — primitive linear snapshot compiles one-to-one
// ---------------------------------------------------------------------------

/// A primitive linear model: two variables, one constraint, one active
/// objective. Returns the model (committed).
fn linear_model() -> Model {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 40.0)).unwrap();
    let y = model.add_variable(continuous().bounds(0.0, 30.0)).unwrap();
    model.add_constraint((2.0 * x + y).le(60.0)).unwrap();
    model.maximize(3.0 * x + 4.0 * y + 5.0).unwrap();
    model.commit().unwrap();
    model
}

#[test]
fn identity_compile_primitive_linear_snapshot_one_to_one() {
    let model = linear_model();
    let snapshot = model.take_snapshot().unwrap();
    let mut session = CompilationSession::new();

    let compiled = session
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .expect("primitive linear snapshot must compile");

    // One variable per canonical variable, dense ids.
    assert_eq!(compiled.variables.len(), snapshot.variables.len());
    for (i, v) in compiled.variables.iter().enumerate() {
        assert_eq!(v.id, CompiledVariableId(i as u32));
        assert_eq!(v.bounds, snapshot.variables[i].bounds);
        assert_eq!(v.var_type, snapshot.variables[i].var_type);
    }
    // One row per canonical constraint.
    assert_eq!(compiled.linear_rows.len(), snapshot.constraints.len());
    assert_eq!(compiled.linear_rows[0].id, CompiledConstraintId(0));
    // One objective per canonical objective.
    assert_eq!(compiled.objectives.len(), snapshot.objectives.len());
    assert_eq!(compiled.objectives[0].id, CompiledObjectiveId(0));
    // The active objective compiles to `Single`.
    assert_eq!(
        compiled.objective_policy,
        CompiledObjectivePolicy::Single(CompiledObjectiveId(0))
    );
    // Exact source identity is recorded.
    assert_eq!(compiled.source_instance, model.instance());
    assert_eq!(compiled.source_revision, model.current_revision());
}

#[test]
fn identity_compile_objectiveless_snapshot_uses_policy_none() {
    // A32: a snapshot with no active objective compiles to `None`, preserving
    // the M2 reference-backend objective-less solve behavior (B1 resolution).
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.add_constraint((x).le(10.0)).unwrap();
    model.commit().unwrap();
    let snapshot = model.take_snapshot().unwrap();
    let mut session = CompilationSession::new();

    let compiled = session
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .unwrap();
    assert_eq!(compiled.objective_policy, CompiledObjectivePolicy::None);
}

#[test]
fn identity_compile_inactive_entities_fold_activity_into_bounds() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 40.0)).unwrap();
    let y = model.add_variable(continuous().bounds(0.0, 30.0)).unwrap();
    let c = model.add_constraint((2.0 * x + y).le(60.0)).unwrap();
    model.maximize(3.0 * x + 4.0 * y).unwrap();
    model.commit().unwrap();

    // Deactivate y and c, then re-snapshot.
    model.set_variable_active(y, false).unwrap();
    model.set_constraint_active(c, false).unwrap();
    model.commit().unwrap();
    let snapshot = model.take_snapshot().unwrap();
    let mut session = CompilationSession::new();

    let compiled = session
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .unwrap();

    // The compiled IR has no activity flag (design §8.3): inactive variables
    // fold to fixed [0,0] and inactive rows fold to unbounded [-inf, inf],
    // exactly the M2 projection semantics used by the HiGHS adapter.
    let inactive_var = compiled
        .variables
        .iter()
        .find(|v| v.bounds == Bounds::new(0.0, 0.0))
        .expect("inactive variable must fold to fixed [0,0]");
    let inactive_row = compiled
        .linear_rows
        .iter()
        .find(|r| r.bounds == ConstraintBounds::range(f64::NEG_INFINITY, f64::INFINITY));
    assert!(
        inactive_row.is_some(),
        "inactive row must fold to unbounded"
    );
    assert_eq!(inactive_var.var_type, VarType::Continuous);
    assert_eq!(
        compiled.linear_rows[0].coefficients.len(),
        2,
        "row keeps its coefficients"
    );
}

#[test]
fn identity_compile_rejects_mip_against_lp_only_backend() {
    // SM-04.4: an unqualified feature is rejected, never silently ignored.
    let mut model = Model::new();
    let x = model.add_variable(integer().bounds(0.0, 10.0)).unwrap();
    model.add_constraint((x).le(10.0)).unwrap();
    model.commit().unwrap();
    let snapshot = model.take_snapshot().unwrap();
    let mut session = CompilationSession::new();

    let err = session
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Auto,
            &lp_only_capabilities(),
        )
        .unwrap_err();
    assert!(matches!(err, CompileError::UnsupportedFeature(_)));
}

// ---------------------------------------------------------------------------
// 2. Compiled delta — exact from/to ids and op mapping
// ---------------------------------------------------------------------------

#[test]
fn compiled_delta_carries_exact_from_to_ids_and_maps_ops() {
    let model = linear_model();
    let snapshot_r1 = model.take_snapshot().unwrap();
    let mut session = CompilationSession::new();
    let base = session
        .compile_snapshot(
            model.instance(),
            &snapshot_r1,
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .unwrap();

    let x = snapshot_r1.variables[0].id;
    let _y = snapshot_r1.variables[1].id;
    let c0 = snapshot_r1.constraints[0].id;
    let z = roml::id::VarId::new(2, Generation::new());

    // Manual delta r1 -> r2: add a variable, set a cell, remove a cell,
    // change a constraint bound, and switch the active objective to None.
    let rev_r1 = snapshot_r1.revision;
    let rev_r2 = rev_r1.next().unwrap();
    let batch = DeltaBatch::new(
        rev_r1,
        rev_r2,
        vec![
            ModelOp::AddVariable {
                var: z,
                bounds: Bounds::new(0.0, 5.0),
                var_type: VarType::Continuous,
            },
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c0), z),
                value_expr: ValueExpr::constant(1.5),
                evaluated_value: 1.5,
            },
            ModelOp::RemoveCell {
                cell_key: (CoefficientTarget::Constraint(c0), x),
            },
            ModelOp::SetConstraintBounds {
                con: c0,
                bounds: ConstraintBounds::le(100.0),
            },
            ModelOp::SetActiveObjective { obj: None },
        ],
    )
    .unwrap();

    let compiled_delta = session
        .compile_delta(
            &batch,
            base.compilation_id,
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .expect("primitive delta must compile");

    assert_eq!(compiled_delta.from_compilation, base.compilation_id);
    assert_eq!(compiled_delta.from_revision, rev_r1);
    assert_eq!(compiled_delta.to_revision, rev_r2);
    assert_ne!(compiled_delta.to_compilation, base.compilation_id);

    // The new variable maps to a dense `AddVariable` op.
    assert!(compiled_delta
        .operations
        .iter()
        .any(|op| matches!(op, BackendOp::AddVariable(v) if v.id == CompiledVariableId(2))));
    // The new cell maps to a `SetLinearCoefficient` op on the new variable.
    assert!(compiled_delta.operations.iter().any(|op| matches!(
        op,
        BackendOp::SetLinearCoefficient {
            constraint: CompiledConstraintId(0),
            variable: CompiledVariableId(2),
            value: 1.5
        }
    )));
    // The removed cell maps to `RemoveLinearCoefficient`.
    assert!(compiled_delta.operations.iter().any(|op| matches!(
        op,
        BackendOp::RemoveLinearCoefficient {
            constraint: CompiledConstraintId(0),
            variable: CompiledVariableId(0)
        }
    )));
    // The bound change maps to `SetLinearRowBounds`.
    assert!(compiled_delta.operations.iter().any(|op| {
        matches!(op, BackendOp::SetLinearRowBounds {
            constraint: CompiledConstraintId(0),
            bounds
        } if *bounds == ConstraintBounds::le(100.0))
    }));
    // The active-objective change maps to `SetObjectivePolicy(None)` (A32).
    assert!(compiled_delta.operations.iter().any(|op| matches!(
        op,
        BackendOp::SetObjectivePolicy(CompiledObjectivePolicy::None)
    )));
}

#[test]
fn compiled_delta_rejects_stale_from_compilation() {
    let model = linear_model();
    let snapshot = model.take_snapshot().unwrap();
    let mut session = CompilationSession::new();
    let base = session
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .unwrap();

    // Compile a second, unrelated snapshot so the session's current id no
    // longer matches the caller's `base.compilation_id`.
    let empty = Model::new().take_snapshot().unwrap();
    let _other = session
        .compile_snapshot(
            model.instance(),
            &empty,
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .unwrap();

    let rev = model.current_revision();
    let next = rev.next().unwrap();
    let batch = DeltaBatch::new(rev, next, vec![]).unwrap();
    let err = session
        .compile_delta(
            &batch,
            base.compilation_id,
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .unwrap_err();
    assert!(matches!(err, CompileError::StaleCompilation { .. }));
}

// ---------------------------------------------------------------------------
// 3. Rebuild on uncertainty (D22, design §18)
// ---------------------------------------------------------------------------

#[test]
fn rebuild_on_uncertainty_for_non_incremental_ops() {
    let model = linear_model();
    let snapshot = model.take_snapshot().unwrap();
    let mut session = CompilationSession::new();
    let base = session
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .unwrap();

    // A batch containing a variable-activity op (which the identity compiler
    // cannot prove incrementally equivalent) forces a deterministic rebuild:
    // no `BackendDeltaBatch` is emitted.
    let rev = model.current_revision();
    let next = rev.next().unwrap();
    let var = snapshot.variables[0].id;
    let batch = DeltaBatch::new(
        rev,
        next,
        vec![ModelOp::SetVariableActive { var, active: false }],
    )
    .unwrap();
    let err = session
        .compile_delta(
            &batch,
            base.compilation_id,
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .unwrap_err();
    assert!(
        matches!(err, CompileError::RebuildRequired(_)),
        "uncertainty must select rebuild, got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// 4. A31-aware delta consumption
// ---------------------------------------------------------------------------

/// Updates to a pre-existing function must ride the ops
/// (`SetCell`/`SetConstraintBounds`/`RemoveCell`), never treating
/// `DeltaBatch.functions` as exhaustive for pre-existing constraints.
#[test]
fn a31_delta_consumes_ops_for_updates_to_pre_existing_constraints() {
    let model = linear_model();
    let snapshot_r1 = model.take_snapshot().unwrap();
    let mut session = CompilationSession::new();
    let base = session
        .compile_snapshot(
            model.instance(),
            &snapshot_r1,
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .unwrap();

    // Update a pre-existing constraint's coefficient and bounds.
    let x = snapshot_r1.variables[0].id;
    let c0 = snapshot_r1.constraints[0].id;
    let rev_r1 = snapshot_r1.revision;
    let rev_r2 = rev_r1.next().unwrap();
    let batch = DeltaBatch::new(
        rev_r1,
        rev_r2,
        vec![
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c0), x),
                value_expr: ValueExpr::constant(5.0),
                evaluated_value: 5.0,
            },
            ModelOp::SetConstraintBounds {
                con: c0,
                bounds: ConstraintBounds::le(100.0),
            },
        ],
    )
    .unwrap();

    // A31: `functions` is the view of ADDED constraints; the pre-existing row
    // does not appear in it.
    assert!(
        batch.functions.is_empty(),
        "A31: pre-existing constraint updates do not appear in DeltaBatch.functions"
    );

    let compiled_delta = session
        .compile_delta(
            &batch,
            base.compilation_id,
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .expect("primitive op update must compile");

    // The update rides ops: a SetLinearCoefficient and a SetLinearRowBounds.
    assert!(compiled_delta.operations.iter().any(|op| matches!(
        op,
        BackendOp::SetLinearCoefficient {
            constraint: CompiledConstraintId(0),
            ..
        }
    )));
    assert!(compiled_delta.operations.iter().any(|op| matches!(
        op,
        BackendOp::SetLinearRowBounds {
            constraint: CompiledConstraintId(0),
            ..
        }
    )));
}

// ---------------------------------------------------------------------------
// 5. Compile-before-mutation / fresh compilation id per target state
// ---------------------------------------------------------------------------

/// The compiler emits a fresh `CompilationId` per target state (B2 / D28):
/// two compilations of equal content from the same source never share an id.
#[test]
fn every_compiled_state_allocates_a_fresh_compilation_id() {
    let model = linear_model();
    let snapshot = model.take_snapshot().unwrap();
    let mut session = CompilationSession::new();

    let a = session
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .unwrap();
    let b = session
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .unwrap();
    assert_ne!(a.compilation_id, b.compilation_id);
}
