//! P27 Task 9 — assignments, solution locks, and the SolveOverlay contract
//! (SM-02.2 secondary, SM-06.1..SM-06.6; the plan's pinned SolveOverlay
//! contract resolving issue #26 item 1).
//!
//! Asserts:
//! - `PrimalAssignment` is a partial value map with lineage plus optional
//!   source instance/revision provenance and makes no feasibility/optimality
//!   claim (SM-06.1); `validate_for` gates on lineage + live generation +
//!   value/domain only (SM-02.2, SM-06.6). Instance/revision are provenance,
//!   never compatibility authority (D4).
//! - `Solution::primal_assignment` binds the SOLVED model's lineage/instance/
//!   revision and the solution's user-variable values; `subset` restricts the
//!   value map (SM-06.2).
//! - `SolutionLock`/`LockSelector`/`ContinuousLock` are distinct packet-shaped
//!   public types (SM-06.3); all five selectors resolve and both continuous
//!   locks produce the expected bounds; `Within` on an integer/binary variable
//!   is a typed error (SM-06.4, SM-06.5).
//! - `compile_overlay` implements the pinned SolveOverlay contract: contents,
//!   objective-override -> `SetObjectivePolicy(CompiledObjectivePolicy::Single)`,
//!   fresh `CompilationId` distinct from the base, exact-base staleness
//!   rejection, before-mutation assignment validation, and a `SolveOverlay`
//!   origin for every added temporary row (D5, issue #26 item 1).

use std::collections::{BTreeMap, BTreeSet};

use roml::advanced::{
    compile_overlay, BackendCapabilitySet, BackendFeature, CompilationId, CompilationSession,
    CompiledObjectiveId, CompiledObjectivePolicy, CompiledVariableId, CutoffDirection,
    EntityOrigin, FeatureSupport, GeneratedRole, OverlayOp, SupportLevel,
};
use roml::compiler::capability::CompilationPolicy;
use roml::model::{binary, continuous, integer, Bounds, ConstraintBounds};
use roml::{
    AssignmentError, ConstraintExprExt, ContinuousLock, LockSelector, Model, ObjectiveCutoff,
    ObjectiveLock, OverlayError, PrimalAssignment, SolutionBuilder, SolutionLock, SolveOverlay,
    SolveStatus,
};

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

/// Compile the current committed model into a fresh `CompilationSession`,
/// returning the session and the established base `CompilationId`.
fn compile_base(model: &Model) -> (CompilationSession, CompilationId) {
    let mut compiler = CompilationSession::new();
    let snapshot = model.take_snapshot().expect("snapshot of committed model");
    let compiled = compiler
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .expect("snapshot compiles under full capabilities");
    (compiler, compiled.compilation_id)
}

// ---------------------------------------------------------------------------
// 1. PrimalAssignment — packet shape, provenance, and validate_for (SM-06.1,
//    SM-02.2, SM-06.6, D4)
// ---------------------------------------------------------------------------

#[test]
fn primal_assignment_is_a_partial_value_map_with_lineage_and_provenance() {
    let lineage = Model::new().lineage();
    let x = roml::VarId::new(0, roml::id::Generation::new());
    let y = roml::VarId::new(1, roml::id::Generation::new());

    let assignment = PrimalAssignment {
        lineage,
        source_instance: None,
        source_revision: None,
        values: BTreeMap::from([(x, 1.0)]),
    };

    // Partial: x present, y absent — no feasibility/optimality claim.
    assert_eq!(assignment.value(x), Some(1.0));
    assert_eq!(assignment.value(y), None);
    assert_eq!(assignment.lineage, lineage);
    assert_eq!(assignment.values.len(), 1);
}

#[test]
fn validate_for_passes_for_same_lineage_live_in_domain_values() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let y = model.add_variable(integer().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();

    let assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: Some(model.instance()),
        source_revision: Some(model.current_revision()),
        values: BTreeMap::from([(x, 3.5), (y, 2.0)]),
    };
    assert!(assignment.validate_for(&model).is_ok());
}

#[test]
fn validate_for_rejects_independent_lineage() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();

    // An assignment from an independent model (fresh lineage).
    let assignment = PrimalAssignment {
        lineage: Model::new().lineage(),
        source_instance: None,
        source_revision: None,
        values: BTreeMap::from([(x, 3.0)]),
    };
    assert!(matches!(
        assignment.validate_for(&model),
        Err(AssignmentError::LineageMismatch { .. })
    ));
}

#[test]
fn validate_for_rejects_stale_generation_variable() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();

    let assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: Some(model.instance()),
        source_revision: Some(model.current_revision()),
        values: BTreeMap::from([(x, 3.0)]),
    };

    // Remove the variable: the assignment's handle is now stale.
    model.remove_variable(x).unwrap();
    model.commit().unwrap();
    assert!(matches!(
        assignment.validate_for(&model),
        Err(AssignmentError::StaleVariable { variable }) if variable == x
    ));
}

#[test]
fn validate_for_rejects_out_of_domain_value() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();

    let assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: None,
        source_revision: None,
        values: BTreeMap::from([(x, 15.0)]),
    };
    assert!(matches!(
        assignment.validate_for(&model),
        Err(AssignmentError::ValueOutOfBounds { variable, value, .. })
            if variable == x && value == 15.0
    ));
}

#[test]
fn validate_for_rejects_non_integral_value_on_integer_variable() {
    let mut model = Model::new();
    let x = model.add_variable(integer().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();

    let assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: None,
        source_revision: None,
        values: BTreeMap::from([(x, 3.5)]),
    };
    assert!(matches!(
        assignment.validate_for(&model),
        Err(AssignmentError::ValueOutOfBounds { variable, value, .. })
            if variable == x && value == 3.5
    ));
}

#[test]
fn clones_at_same_revision_both_validate_an_assignment() {
    // D4: instance/revision are PROVENANCE, not compatibility authority. Two
    // clones at the same revision with the same lineage both validate.
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();

    let assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: Some(model.instance()),
        source_revision: Some(model.current_revision()),
        values: BTreeMap::from([(x, 5.0)]),
    };

    let clone = model.clone();
    assert_eq!(clone.lineage(), model.lineage());
    assert_ne!(clone.instance(), model.instance());
    assert_eq!(clone.current_revision(), model.current_revision());
    assert!(assignment.validate_for(&model).is_ok());
    assert!(assignment.validate_for(&clone).is_ok());
}

// ---------------------------------------------------------------------------
// 2. Solution::primal_assignment and subset (SM-06.2)
// ---------------------------------------------------------------------------

#[test]
fn solution_primal_assignment_binds_solved_lineage_and_values() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let y = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.add_constraint((x + y).le(10.0)).unwrap();
    model.maximize(x + y).unwrap();
    model.commit().unwrap();

    let solution = SolutionBuilder::new()
        .status(SolveStatus::Optimal)
        .value(x, 3.0)
        .value(y, 4.0)
        .objective_value(7.0)
        .metadata(roml::SolveMetadata {
            model_lineage: model.lineage(),
            model_instance: model.instance(),
            model_revision: model.current_revision(),
            ..roml::SolveMetadata::default()
        })
        .build();

    let assignment = solution.primal_assignment();
    // The SOLVED model's real lineage/instance/revision, never fresh defaults.
    assert_eq!(assignment.lineage, model.lineage());
    assert_eq!(assignment.source_instance, Some(model.instance()));
    assert_eq!(assignment.source_revision, Some(model.current_revision()));
    assert_eq!(assignment.value(x), Some(3.0));
    assert_eq!(assignment.value(y), Some(4.0));
    assert_eq!(assignment.values.len(), 2);
}

#[test]
fn primal_assignment_subset_restricts_the_value_map() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let y = model.add_variable(continuous()).unwrap();
    model.commit().unwrap();

    let solution = SolutionBuilder::new()
        .status(SolveStatus::Optimal)
        .value(x, 1.0)
        .value(y, 2.0)
        .metadata(roml::SolveMetadata {
            model_lineage: model.lineage(),
            model_instance: model.instance(),
            model_revision: model.current_revision(),
            ..roml::SolveMetadata::default()
        })
        .build();
    let assignment = solution.primal_assignment();

    let subset = assignment.subset(&[x]);
    assert_eq!(subset.lineage, assignment.lineage);
    assert_eq!(subset.source_instance, assignment.source_instance);
    assert_eq!(subset.value(x), Some(1.0));
    assert_eq!(subset.value(y), None);
    assert_eq!(subset.values.len(), 1);
}

// ---------------------------------------------------------------------------
// 3. Lock selectors and continuous bands (SM-06.3, SM-06.4, SM-06.5)
// ---------------------------------------------------------------------------

/// A committed model with one continuous, one integer, and one binary
/// variable, plus its compiled base. Returns `(model, compiler, cont, int, bin)`.
fn mixed_model() -> (
    Model,
    CompilationSession,
    roml::VarId,
    roml::VarId,
    roml::VarId,
) {
    let mut model = Model::new();
    let cont = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let int = model.add_variable(integer().bounds(0.0, 10.0)).unwrap();
    let bin = model.add_variable(binary()).unwrap();
    model.commit().unwrap();
    let (compiler, _) = compile_base(&model);
    (model, compiler, cont, int, bin)
}

fn all_values_assignment(
    model: &Model,
    cont: roml::VarId,
    int: roml::VarId,
    bin: roml::VarId,
) -> PrimalAssignment {
    PrimalAssignment {
        lineage: model.lineage(),
        source_instance: Some(model.instance()),
        source_revision: Some(model.current_revision()),
        values: BTreeMap::from([(cont, 1.5), (int, 2.0), (bin, 1.0)]),
    }
}

fn exact_lock(assignment: PrimalAssignment, selector: LockSelector) -> SolutionLock {
    SolutionLock {
        assignment,
        selector,
        continuous: ContinuousLock::Exact,
    }
}

#[test]
fn all_assigned_selector_selects_every_value() {
    let (model, compiler, cont, int, bin) = mixed_model();
    let lock = exact_lock(
        all_values_assignment(&model, cont, int, bin),
        LockSelector::AllAssigned,
    );
    let overlay = SolveOverlay::new(BTreeMap::new(), vec![lock], vec![], vec![]).unwrap();
    let compiled = compile_overlay(&model, &compiler, &overlay, None).unwrap();
    // cont(0), int(1), bin(2) — all three, dense order.
    let vars: Vec<_> = compiled
        .operations
        .iter()
        .filter_map(|op| match op {
            OverlayOp::SetTemporaryVariableBounds { variable, bounds } => {
                Some((*variable, *bounds))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        vars,
        vec![
            (CompiledVariableId(0), Bounds::new(1.5, 1.5)),
            (CompiledVariableId(1), Bounds::new(2.0, 2.0)),
            (CompiledVariableId(2), Bounds::new(1.0, 1.0)),
        ]
    );
}

#[test]
fn integer_assigned_selector_selects_only_integer_variables() {
    let (model, compiler, cont, int, bin) = mixed_model();
    let lock = exact_lock(
        all_values_assignment(&model, cont, int, bin),
        LockSelector::IntegerAssigned,
    );
    let overlay = SolveOverlay::new(BTreeMap::new(), vec![lock], vec![], vec![]).unwrap();
    let compiled = compile_overlay(&model, &compiler, &overlay, None).unwrap();
    assert_eq!(compiled.operations.len(), 1);
    assert!(matches!(
        &compiled.operations[0],
        OverlayOp::SetTemporaryVariableBounds { variable: CompiledVariableId(1), bounds }
            if *bounds == Bounds::new(2.0, 2.0)
    ));
}

#[test]
fn binary_assigned_selector_selects_only_binary_variables() {
    let (model, compiler, cont, int, bin) = mixed_model();
    let lock = exact_lock(
        all_values_assignment(&model, cont, int, bin),
        LockSelector::BinaryAssigned,
    );
    let overlay = SolveOverlay::new(BTreeMap::new(), vec![lock], vec![], vec![]).unwrap();
    let compiled = compile_overlay(&model, &compiler, &overlay, None).unwrap();
    assert_eq!(compiled.operations.len(), 1);
    assert!(matches!(
        &compiled.operations[0],
        OverlayOp::SetTemporaryVariableBounds { variable: CompiledVariableId(2), bounds }
            if *bounds == Bounds::new(1.0, 1.0)
    ));
}

#[test]
fn variables_selector_selects_exactly_the_set() {
    let (model, compiler, cont, int, bin) = mixed_model();
    let lock = exact_lock(
        all_values_assignment(&model, cont, int, bin),
        LockSelector::Variables(BTreeSet::from([int, bin])),
    );
    let overlay = SolveOverlay::new(BTreeMap::new(), vec![lock], vec![], vec![]).unwrap();
    let compiled = compile_overlay(&model, &compiler, &overlay, None).unwrap();
    assert_eq!(compiled.operations.len(), 2);
    let vars: BTreeSet<_> = compiled
        .operations
        .iter()
        .filter_map(|op| match op {
            OverlayOp::SetTemporaryVariableBounds { variable, .. } => Some(*variable),
            _ => None,
        })
        .collect();
    assert_eq!(
        vars,
        BTreeSet::from([CompiledVariableId(1), CompiledVariableId(2)])
    );
}

#[test]
fn except_selector_selects_all_assigned_minus_the_set() {
    let (model, compiler, cont, int, bin) = mixed_model();
    let lock = exact_lock(
        all_values_assignment(&model, cont, int, bin),
        LockSelector::Except(BTreeSet::from([int])),
    );
    let overlay = SolveOverlay::new(BTreeMap::new(), vec![lock], vec![], vec![]).unwrap();
    let compiled = compile_overlay(&model, &compiler, &overlay, None).unwrap();
    assert_eq!(compiled.operations.len(), 2);
    let vars: BTreeSet<_> = compiled
        .operations
        .iter()
        .filter_map(|op| match op {
            OverlayOp::SetTemporaryVariableBounds { variable, .. } => Some(*variable),
            _ => None,
        })
        .collect();
    assert_eq!(
        vars,
        BTreeSet::from([CompiledVariableId(0), CompiledVariableId(2)])
    );
}

#[test]
fn within_band_produces_band_bounds_for_continuous_variables() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();
    let (compiler, _) = compile_base(&model);

    let assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: Some(model.instance()),
        source_revision: Some(model.current_revision()),
        values: BTreeMap::from([(x, 3.0)]),
    };
    let lock = SolutionLock {
        assignment,
        selector: LockSelector::Variables(BTreeSet::from([x])),
        continuous: ContinuousLock::Within { absolute: 0.5 },
    };
    let overlay = SolveOverlay::new(BTreeMap::new(), vec![lock], vec![], vec![]).unwrap();
    let compiled = compile_overlay(&model, &compiler, &overlay, None).unwrap();
    assert_eq!(compiled.operations.len(), 1);
    assert!(matches!(
        &compiled.operations[0],
        OverlayOp::SetTemporaryVariableBounds { variable: CompiledVariableId(0), bounds }
            if *bounds == Bounds::new(2.5, 3.5)
    ));
}

#[test]
fn within_band_on_integer_variable_is_a_typed_error() {
    let mut model = Model::new();
    let int = model.add_variable(integer().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();
    let (compiler, _) = compile_base(&model);

    let assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: Some(model.instance()),
        source_revision: Some(model.current_revision()),
        values: BTreeMap::from([(int, 4.0)]),
    };
    let lock = SolutionLock {
        assignment,
        selector: LockSelector::AllAssigned,
        continuous: ContinuousLock::Within { absolute: 1.0 },
    };
    let overlay = SolveOverlay::new(BTreeMap::new(), vec![lock], vec![], vec![]).unwrap();
    let err = compile_overlay(&model, &compiler, &overlay, None).unwrap_err();
    assert!(matches!(
        err,
        OverlayError::WithinBandOnNonContinuous { variable } if variable == int
    ));
}

// ---------------------------------------------------------------------------
// 4. SolveOverlay contract compile (issue #26 item 1)
// ---------------------------------------------------------------------------

#[test]
fn compile_overlay_produces_the_pinned_mapping_and_origins() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let y = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let z = model.add_variable(continuous().bounds(0.0, 5.0)).unwrap();
    model.add_constraint((x + y).le(10.0)).unwrap();
    let obj = model.minimize(2.0 * x + y + 3.0).unwrap();
    model.commit().unwrap();
    let (compiler, c_base) = compile_base(&model);

    let assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: Some(model.instance()),
        source_revision: Some(model.current_revision()),
        values: BTreeMap::from([(x, 3.0), (y, 2.0)]),
    };
    let overlay = SolveOverlay::new(
        BTreeMap::from([(z, 1.5)]),
        vec![SolutionLock {
            assignment,
            selector: LockSelector::AllAssigned,
            continuous: ContinuousLock::Exact,
        }],
        vec![ObjectiveLock {
            objective: obj,
            absolute_tolerance: 1e-6,
            relative_tolerance: 1e-6,
        }],
        vec![ObjectiveCutoff {
            objective: obj,
            limit: 5.0,
            direction: CutoffDirection::Upper,
        }],
    )
    .unwrap();

    let compiled = compile_overlay(&model, &compiler, &overlay, None).unwrap();

    // Exact base and a FRESH overlay compilation id (D28).
    assert_eq!(compiled.base_compilation, c_base);
    assert_ne!(compiled.compilation_id, c_base);
    assert_eq!(compiled.overlay_id, overlay.id);
    assert_eq!(compiled.objective_policy_override, None);

    // Operation sequence: temp fixing z, lock x/y (Exact), objective-lock row,
    // cutoff row.
    assert_eq!(compiled.operations.len(), 5);
    assert!(matches!(
        &compiled.operations[0],
        OverlayOp::SetTemporaryVariableBounds { variable: CompiledVariableId(2), bounds }
            if *bounds == Bounds::new(1.5, 1.5)
    ));
    assert!(matches!(
        &compiled.operations[1],
        OverlayOp::SetTemporaryVariableBounds { variable: CompiledVariableId(0), bounds }
            if *bounds == Bounds::new(3.0, 3.0)
    ));
    assert!(matches!(
        &compiled.operations[2],
        OverlayOp::SetTemporaryVariableBounds { variable: CompiledVariableId(1), bounds }
            if *bounds == Bounds::new(2.0, 2.0)
    ));

    // Objective-lock row: the compiled row for f(x) = 2x + y + 3 with the
    // degradation RHS. P27 compiles the row with a zero reference optimum:
    // f(x) <= abs_tol  =>  2x + y <= abs_tol - constant(3).
    match &compiled.operations[3] {
        OverlayOp::AddTemporaryRow { row } => {
            assert_eq!(
                row.coefficients,
                vec![(CompiledVariableId(0), 2.0), (CompiledVariableId(1), 1.0)]
            );
            assert_eq!(row.bounds, ConstraintBounds::le(1e-6 - 3.0));
        }
        other => panic!("expected AddTemporaryRow for the objective lock, got {other:?}"),
    }
    // Cutoff row: f(x) <= 5  =>  2x + y <= 5 - 3.
    match &compiled.operations[4] {
        OverlayOp::AddTemporaryRow { row } => {
            assert_eq!(
                row.coefficients,
                vec![(CompiledVariableId(0), 2.0), (CompiledVariableId(1), 1.0)]
            );
            assert_eq!(row.bounds, ConstraintBounds::le(2.0));
        }
        other => panic!("expected AddTemporaryRow for the cutoff, got {other:?}"),
    }

    // Every added temporary row carries a SolveOverlay origin (D5).
    assert_eq!(compiled.origin_additions.len(), 2);
    let lock_origin = EntityOrigin::SolveOverlay {
        overlay: overlay.id,
        role: GeneratedRole::ObjectiveLockRow,
    };
    let cutoff_origin = EntityOrigin::SolveOverlay {
        overlay: overlay.id,
        role: GeneratedRole::CutoffRow,
    };
    assert_eq!(
        compiled
            .origin_additions
            .constraints_for_origin(&lock_origin)
            .len(),
        1
    );
    assert_eq!(
        compiled
            .origin_additions
            .constraints_for_origin(&cutoff_origin)
            .len(),
        1
    );
}

#[test]
fn compile_overlay_objective_override_maps_to_single_objective_policy() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let obj = model.maximize(x).unwrap();
    model.commit().unwrap();
    let (compiler, c_base) = compile_base(&model);

    let overlay = SolveOverlay::new(BTreeMap::new(), vec![], vec![], vec![]).unwrap();
    let compiled = compile_overlay(&model, &compiler, &overlay, Some(obj)).unwrap();

    // The override solve is recorded by the FRESH overlay compilation id and a
    // Single-policy override op (P27 pins: objective_override ->
    // SetObjectivePolicy(CompiledObjectivePolicy::Single)).
    assert_eq!(compiled.base_compilation, c_base);
    assert_ne!(compiled.compilation_id, c_base);
    // The compiled objective id is dense: the first objective compiles to 0.
    assert_eq!(
        compiled.objective_policy_override,
        Some(CompiledObjectivePolicy::Single(CompiledObjectiveId(0)))
    );
    // The override is emitted as the LAST op.
    assert!(matches!(
        compiled.operations.last(),
        Some(OverlayOp::SetObjectivePolicy(
            CompiledObjectivePolicy::Single(CompiledObjectiveId(0))
        ))
    ));
}

#[test]
fn compile_overlay_rejects_an_unknown_objective_override() {
    let mut model = Model::new();
    let _x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();
    let (compiler, _) = compile_base(&model);

    // An objective that was never added to the model.
    let ghost = roml::ObjId::new(99, roml::id::Generation::new());
    let overlay = SolveOverlay::new(BTreeMap::new(), vec![], vec![], vec![]).unwrap();
    let err = compile_overlay(&model, &compiler, &overlay, Some(ghost)).unwrap_err();
    assert!(matches!(
        err,
        OverlayError::ObjectiveNotFound(obj) if obj == ghost
    ));
}

#[test]
fn compile_overlay_rejects_a_stale_base_compilation() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();

    // A fresh compiler with NO compiled base is stale: no exact base to apply
    // the overlay on top of.
    let compiler = CompilationSession::new();
    let assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: None,
        source_revision: None,
        values: BTreeMap::from([(x, 1.0)]),
    };
    let overlay = SolveOverlay::new(
        BTreeMap::new(),
        vec![exact_lock(assignment, LockSelector::AllAssigned)],
        vec![],
        vec![],
    )
    .unwrap();
    let err = compile_overlay(&model, &compiler, &overlay, None).unwrap_err();
    assert!(matches!(err, OverlayError::StaleCompilation { .. }));
}

#[test]
fn compile_overlay_rejects_a_compiler_bound_to_another_model() {
    let mut model_a = Model::new();
    model_a.add_variable(continuous()).unwrap();
    model_a.commit().unwrap();
    let (compiler, _) = compile_base(&model_a);

    // A different model (new lineage) passed to an overlay compiled against
    // model_a's base.
    let mut model_b = Model::new();
    let y = model_b.add_variable(continuous()).unwrap();
    model_b.commit().unwrap();
    let assignment = PrimalAssignment {
        lineage: model_b.lineage(),
        source_instance: Some(model_b.instance()),
        source_revision: Some(model_b.current_revision()),
        values: BTreeMap::from([(y, 1.0)]),
    };
    let overlay = SolveOverlay::new(
        BTreeMap::new(),
        vec![exact_lock(assignment, LockSelector::AllAssigned)],
        vec![],
        vec![],
    )
    .unwrap();
    let err = compile_overlay(&model_b, &compiler, &overlay, None).unwrap_err();
    assert!(matches!(err, OverlayError::StaleCompilation { .. }));
}

#[test]
fn compile_overlay_rejects_invalid_assignment_before_any_op() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();
    let (compiler, _) = compile_base(&model);

    // Wrong lineage: SM-06.6 requires the failure BEFORE any op is produced.
    let assignment = PrimalAssignment {
        lineage: Model::new().lineage(),
        source_instance: None,
        source_revision: None,
        values: BTreeMap::from([(x, 3.0)]),
    };
    let overlay = SolveOverlay::new(
        BTreeMap::new(),
        vec![exact_lock(assignment, LockSelector::AllAssigned)],
        vec![],
        vec![],
    )
    .unwrap();
    let err = compile_overlay(&model, &compiler, &overlay, None).unwrap_err();
    assert!(matches!(
        err,
        OverlayError::Assignment(AssignmentError::LineageMismatch { .. })
    ));
}

#[test]
fn temporary_fixings_and_locks_never_advance_the_model_revision() {
    // Task 9 guarantees revision invariance structurally: validate_for and
    // compile_overlay take `&Model` and cannot emit Change/ModelOp/revision.
    // Assert the read-only contract holds across the whole compile surface.
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.commit().unwrap();
    let (compiler, _) = compile_base(&model);
    let rev = model.current_revision();

    let assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: None,
        source_revision: None,
        values: BTreeMap::from([(x, 3.0)]),
    };
    let overlay = SolveOverlay::new(
        BTreeMap::from([(x, 2.0)]),
        vec![exact_lock(assignment, LockSelector::AllAssigned)],
        vec![],
        vec![],
    )
    .unwrap();
    let _ = compile_overlay(&model, &compiler, &overlay, None).unwrap();

    assert_eq!(model.current_revision(), rev);
    assert!(!model.has_pending_changes());
}
