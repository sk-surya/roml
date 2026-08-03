//! P27 Task 9 + Task 10 — assignments, solution locks, the SolveOverlay
//! contract, and transactional reversible overlay execution.
//!
//! Task 9 (SM-02.2 secondary, SM-06.1..SM-06.6; the plan's pinned SolveOverlay
//! contract resolving issue #26 item 1):
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
//!
//! Task 10 (SM-07.3..SM-07.6; the overlay lifecycle):
//! - Transactional apply/rollback with explicit `OverlayApplyReceipt`/
//!   `OverlayRollbackOutcome`; rollback always attempted on failure; a
//!   `RequiresRebuild` outcome marks the session and the next solve rebuilds
//!   (D7, D22).
//! - The overlay solve validates `result.compilation_id == C_overlay` (the
//!   fresh overlay compilation id), NOT `compiler.current_compilation()`
//!   which stays `C_base`; the result is normalized with `compilation_id =
//!   C_overlay` and `overlay_id = Some(overlay.id)`.
//! - Failure injection at validation/compile/apply/solve/extraction/rollback/
//!   post-rollback proves no overlay leaks into any later solve and every
//!   later clean solve equals a fresh rebuild (SM-07.6, the phase gate).

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use roml::advanced::{
    compile_overlay, BackendCapabilitySet, BackendFeature, BackendOp, BackendSnapshot,
    CompilationId, CompilationSession, CompiledObjectiveId, CompiledObjectivePolicy,
    CompiledVariableId, CutoffDirection, EntityOrigin, FeatureSupport, GeneratedRole,
    OverlayApplyReceipt, OverlayOp, OverlayRollbackOutcome, OverlaySession, SolveRequest,
    SolveResult, SolveSolution, SupportLevel, Synchronization,
};
use roml::compiler::capability::CompilationPolicy;
use roml::model::objective::Sense;
use roml::model::{binary, continuous, integer, Bounds, ConstraintBounds};
use roml::revision::ModelRevision;
use roml::solver::backend::{BackendError, ErrorCategory, HealthEffect, TerminationStatus};
use roml::solver::reference::ReferenceBackend;
use roml::solver::request::EffectiveConfig;
use roml::solver::session::{BackendMetadata, BackendSession, SessionHealth, SyncReceipt};
use roml::sync::{AdapterCursor, AdapterHealth};
use roml::{
    AssignmentError, ConstraintExprExt, ContinuousLock, LockSelector, Model, ObjectiveCutoff,
    ObjectiveLock, OverlayError, PrimalAssignment, SolutionBuilder, SolutionLock, SolveError,
    SolveOptions, SolveOverlay, SolveStatus, SolverSession,
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

/// The full typed capability surface (Task 10 test backends).
fn full_typed_capabilities() -> BackendCapabilitySet {
    full_capabilities()
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

/// CR-01: `validate_for` rejects non-finite assignment values (NaN, ±inf)
/// with a typed error — a NaN/±inf value must never reach a native solver as
/// a bound. `NaN` passes the `<`/`>` bounds comparisons (both false) and `+inf`
/// passes when the upper bound is itself infinite, so the finiteness check must
/// come FIRST.
#[test]
fn validate_for_rejects_non_finite_values() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let unbounded = model.add_variable(continuous()).unwrap(); // [0, +inf)
    model.commit().unwrap();

    for (var, value) in [
        (x, f64::NAN),
        (x, f64::INFINITY),
        (x, f64::NEG_INFINITY),
        // +inf inside an unbounded-upper domain is exactly the case the
        // `value < lower || value > upper` gate misses.
        (unbounded, f64::INFINITY),
    ] {
        let assignment = PrimalAssignment {
            lineage: model.lineage(),
            source_instance: None,
            source_revision: None,
            values: BTreeMap::from([(var, value)]),
        };
        assert!(
            matches!(
                assignment.validate_for(&model),
                Err(AssignmentError::NonFiniteValue { variable, value: v })
                    if variable == var
                        && if value.is_nan() { v.is_nan() } else { v == value }
            ),
            "assignment value {value} for {var:?} must be rejected as non-finite"
        );
    }
}

/// CR-01: an overlay temporary fixing or lock value that is NaN/±inf is
/// rejected at COMPILE time with a typed error, BEFORE any backend op — the
/// NaN/±inf bound must never reach the native solver (e.g.
/// `Highs_changeColBounds(NaN, NaN)`).
#[test]
fn overlay_rejects_non_finite_temporary_fixing_and_lock_values() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let unbounded = model.add_variable(continuous()).unwrap(); // [0, +inf)
    model.commit().unwrap();
    let (compiler, _) = compile_base(&model);

    // NaN temporary fixing -> rejected before any op.
    let overlay =
        SolveOverlay::new(BTreeMap::from([(x, f64::NAN)]), vec![], vec![], vec![]).unwrap();
    let err = compile_overlay(&model, &compiler, &overlay, None).unwrap_err();
    assert!(
        matches!(
            err,
            OverlayError::Assignment(AssignmentError::NonFiniteValue { variable, value })
                if variable == x && value.is_nan()
        ),
        "a NaN temporary fixing must be a typed non-finite error, got {err:?}"
    );

    // +inf lock value on an unbounded-upper variable -> rejected before any op.
    let assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: None,
        source_revision: None,
        values: BTreeMap::from([(unbounded, f64::INFINITY)]),
    };
    let lock = SolutionLock {
        assignment,
        selector: LockSelector::AllAssigned,
        continuous: ContinuousLock::Exact,
    };
    let overlay = SolveOverlay::new(BTreeMap::new(), vec![lock], vec![], vec![]).unwrap();
    let err = compile_overlay(&model, &compiler, &overlay, None).unwrap_err();
    assert!(
        matches!(
            err,
            OverlayError::Assignment(AssignmentError::NonFiniteValue { variable, value })
                if variable == unbounded && value == f64::INFINITY
        ),
        "a +inf lock value must be a typed non-finite error, got {err:?}"
    );
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
// ---------------------------------------------------------------------------
// Task 10 — transactional reversible overlay execution (SM-07.3..SM-07.6)
// ---------------------------------------------------------------------------

/// A committed, compiled model fixture: maximize `x + y` over two continuous
/// variables `x, y` with `x + y <= 10`. Returns `(model, compiler, x, y, obj)`.
fn overlay_fixture() -> (
    Model,
    CompilationSession,
    roml::VarId,
    roml::VarId,
    roml::ObjId,
) {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let y = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.add_constraint((x + y).le(10.0)).unwrap();
    let obj = model.maximize(x + y).unwrap();
    model.commit().unwrap();
    let (compiler, _) = compile_base(&model);
    (model, compiler, x, y, obj)
}

/// Compile the base snapshot and rebuild a fresh `ReferenceBackend` from it,
/// returning `(compiler, backend, snapshot)` with the backend holding `C_base`.
fn reference_backend_at_base(
    model: &Model,
) -> (CompilationSession, ReferenceBackend, BackendSnapshot) {
    let mut compiler = CompilationSession::new();
    let snapshot = model.take_snapshot().expect("committed model snapshot");
    let compiled = compiler
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .expect("snapshot must compile under full capabilities");
    let mut backend = ReferenceBackend::new();
    backend
        .rebuild_compiled(&compiled)
        .expect("base rebuild must succeed");
    (compiler, backend, compiled)
}

/// A deterministic overlay: temporary fixing `x = 2.0` and a cutoff
/// `f(x) <= 5.0` (f = x + y).
fn sample_overlay(x: roml::VarId, obj: roml::ObjId) -> SolveOverlay {
    SolveOverlay::new(
        BTreeMap::from([(x, 2.0)]),
        vec![],
        vec![],
        vec![ObjectiveCutoff {
            objective: obj,
            limit: 5.0,
            direction: CutoffDirection::Upper,
        }],
    )
    .expect("overlay id allocates")
}

/// A trivial single-temp-fixing overlay.
fn trivial_overlay(x: roml::VarId) -> SolveOverlay {
    SolveOverlay::new(BTreeMap::from([(x, 1.0)]), vec![], vec![], vec![]).unwrap()
}

// ── 1. Transactional apply/rollback on the reference backend ────────────────

/// A full apply -> state-transition -> rollback -> verify round-trip on the
/// reference backend. The compiled state and `current_compilation` return
/// EXACTLY to `C_base`; `verify_overlay_clean` passes.
#[test]
fn reference_backend_overlay_apply_rollback_round_trip() {
    let (model, _compiler, x, _y, obj) = overlay_fixture();
    let (compiler, mut backend, compiled_snapshot) = reference_backend_at_base(&model);
    let c_base = compiled_snapshot.compilation_id;
    let base_view = backend.compiled_normalized_view();
    let base_rows = backend.compiled_rows.len();

    let overlay = sample_overlay(x, obj);
    let compiled = compile_overlay(&model, &compiler, &overlay, None).unwrap();
    assert_eq!(compiled.base_compilation, c_base);

    // Apply: current_compilation -> C_overlay; the temp bound and cutoff row
    // are present in the compiled state.
    let receipt = backend.apply_overlay(&compiled).unwrap();
    assert_eq!(receipt.overlay_id, overlay.id);
    assert_eq!(receipt.base_compilation, c_base);
    assert_eq!(receipt.applied_compilation, compiled.compilation_id);
    assert_eq!(backend.current_compilation, Some(compiled.compilation_id));
    assert_eq!(backend.compiled_rows.len(), base_rows + 1);
    assert_eq!(
        backend
            .compiled_variables
            .get(&CompiledVariableId(0))
            .unwrap()
            .0,
        Bounds::new(2.0, 2.0),
        "the temporary fixing must apply as equal compiled bounds"
    );

    // Rollback: current_compilation -> C_base; the compiled state returns
    // EXACTLY to the base view.
    let outcome = backend.rollback_overlay(&receipt).unwrap();
    assert!(
        matches!(
            outcome,
            OverlayRollbackOutcome::Clean { restored_compilation } if restored_compilation == c_base
        ),
        "a fully applied overlay must roll back Clean, got {outcome:?}"
    );
    assert_eq!(backend.current_compilation, Some(c_base));
    assert_eq!(
        backend.compiled_normalized_view(),
        base_view,
        "after a Clean rollback the compiled state must equal the base exactly"
    );

    // Verify: post-rollback verification asserts C_base is restored.
    backend.verify_overlay_clean().unwrap();
}

/// A stale `CompiledOverlay` (base_compilation != the backend's current
/// compiled state) is rejected BEFORE any mutation — the compiled maps and
/// `current_compilation` are unchanged (phase gate "exact compilation
/// mismatches reject before mutation").
#[test]
fn stale_overlay_apply_rejects_before_mutation() {
    let (model, _compiler, x, _y, obj) = overlay_fixture();
    let (_compiler_a, mut backend, _) = reference_backend_at_base(&model);
    let held = backend.current_compilation.expect("base compiled");

    // A SECOND independent compilation of the SAME model produces a DIFFERENT
    // exact `CompilationId` (D28). The overlay compiled against compiler_other's
    // base is stale for a backend holding the reference_backend_at_base state.
    let mut compiler_other = CompilationSession::new();
    let snapshot = model.take_snapshot().unwrap();
    let compiled_other = compiler_other
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .unwrap();
    assert_ne!(
        held, compiled_other.compilation_id,
        "an independent compilation must produce a distinct CompilationId (D28)"
    );

    let overlay = sample_overlay(x, obj);
    // Compile the overlay against the OTHER base, not the backend's held base.
    let compiled = compile_overlay(&model, &compiler_other, &overlay, None).unwrap();
    assert_ne!(compiled.base_compilation, held);
    let state_before = backend.compiled_normalized_view();

    // Apply to the backend holding a DIFFERENT base: rejected before mutation.
    let err = backend
        .apply_overlay(&compiled)
        .expect_err("a stale overlay must be rejected at apply time");
    assert_eq!(
        err.category,
        ErrorCategory::InvalidInput,
        "a stale overlay apply is invalid input"
    );
    assert!(
        err.message.contains("compilation") || err.message.contains("base"),
        "the error must name the stale compilation, got: {}",
        err.message
    );
    assert_eq!(
        backend.compiled_normalized_view(),
        state_before,
        "a rejected stale overlay must not mutate the compiled state"
    );
}

// ── 2. Façade-level transactional overlay solve ──────────────────────────────

/// Full `solve_with_overlay` lifecycle: synchronize to `C_base`, compile the
/// overlay, apply, solve, validate the result against `C_overlay` (NOT
/// `compiler.current_compilation()` which stays `C_base`), roll back, verify
/// `C_base` restored, and normalize with `compilation_id = C_overlay` and
/// `overlay_id = Some(...)`. The model revision is unchanged (SM-07.3).
#[test]
fn solve_with_overlay_tags_result_with_overlay_compilation_and_restores_base() {
    let (mut model, _compiler, x, _y, obj) = overlay_fixture();
    let (backend, state) = OverlayTestBackend::new();
    let mut session = SolverSession::new(backend);

    // Establish the base with a plain solve (records C_base).
    let base_solution = session.solve(&mut model).expect("base solve succeeds");
    let c_base = base_solution
        .metadata()
        .compilation_id
        .expect("base compilation id");
    let rev_before = model.current_revision();

    let overlay = sample_overlay(x, obj);
    let solution = session
        .solve_with_overlay(&mut model, SolveOptions::default(), &overlay, Some(obj))
        .expect("overlay solve succeeds");

    // The override solve is recorded by the FRESH overlay compilation id, never
    // C_base; the overlay id is recorded in the metadata (D28).
    let c_overlay = solution
        .metadata()
        .compilation_id
        .expect("overlay compilation id");
    assert_ne!(c_overlay, c_base, "C_overlay must differ from C_base");
    assert_eq!(solution.metadata().overlay_id, Some(overlay.id));
    assert_eq!(solution.metadata().model_revision, rev_before);

    // Revision invariance (SM-07.3): a full overlay solve leaves the canonical
    // model revision unchanged and records no overlay Change/ModelOp.
    assert_eq!(model.current_revision(), rev_before);
    assert!(!model.has_pending_changes());

    // A subsequent plain solve is tagged with C_base again and carries no
    // overlay id — the overlay did not leak.
    let clean = session
        .solve(&mut model)
        .expect("subsequent clean solve succeeds");
    assert_eq!(clean.metadata().compilation_id, Some(c_base));
    assert_eq!(clean.metadata().overlay_id, None);
    assert_eq!(
        state.borrow().solves,
        3,
        "base + overlay + clean solves exactly"
    );
}

/// The overlay solve validates `result.compilation_id` against `C_overlay`,
/// NOT `compiler.current_compilation()` (which stays `C_base`). A plain solve
/// still validates against `C_base` — no false mismatch on either path.
#[test]
fn overlay_solve_validates_against_c_overlay_not_c_base() {
    let (mut model, _compiler, x, _y, obj) = overlay_fixture();
    let (backend, _state) = OverlayTestBackend::new();
    let mut session = SolverSession::new(backend);

    let base = session.solve(&mut model).unwrap();
    let c_base = base.metadata().compilation_id.unwrap();

    let overlay = SolveOverlay::new(BTreeMap::from([(x, 1.0)]), vec![], vec![], vec![]).unwrap();
    let solution = session
        .solve_with_overlay(&mut model, SolveOptions::default(), &overlay, Some(obj))
        .unwrap();
    let c_overlay = solution.metadata().compilation_id.unwrap();
    assert_ne!(c_overlay, c_base);
    // The result's tagged id equals the OVERLAY's fresh id, and the façade did
    // NOT reject it against the compiler's C_base (the false-fail trap the plan
    // flags).
    assert_ne!(solution.metadata().compilation_id, Some(c_base));
}

/// A stale or invalid overlay is rejected before any backend mutation
/// (validation/compile stage failure); the canonical revision is unchanged and
/// a subsequent clean solve works.
#[test]
fn invalid_overlay_compile_rejects_before_mutation() {
    let (mut model, _compiler, x, _y, _obj) = overlay_fixture();
    let (backend, _state) = OverlayTestBackend::new();
    let mut session = SolverSession::new(backend);
    let _ = session.solve(&mut model).unwrap();
    let rev = model.current_revision();

    // A wrong-lineage assignment forces compile_overlay to fail before any
    // backend mutation (SM-06.6).
    let mut other = Model::new();
    other.commit().unwrap();
    let assignment = PrimalAssignment {
        lineage: other.lineage(),
        source_instance: None,
        source_revision: None,
        values: BTreeMap::from([(x, 1.0)]),
    };
    let overlay = SolveOverlay::new(
        BTreeMap::new(),
        vec![SolutionLock {
            assignment,
            selector: LockSelector::AllAssigned,
            continuous: ContinuousLock::Exact,
        }],
        vec![],
        vec![],
    )
    .unwrap();

    let err = session
        .solve_with_overlay(&mut model, SolveOptions::default(), &overlay, None)
        .expect_err("a wrong-lineage lock must fail before mutation");
    assert!(
        matches!(err, SolveError::Overlay(OverlayError::Assignment(_))),
        "compile-stage failure must surface as SolveError::Overlay, got {err:?}"
    );
    assert_eq!(
        model.current_revision(),
        rev,
        "no canonical revision advance"
    );

    // The session recovers: a subsequent clean solve succeeds.
    session
        .solve(&mut model)
        .expect("clean solve after failed overlay");
}

// ── 3. Rollback uncertainty -> RequiresRebuild (SM-07.5, D7) ─────────────────

/// A fault-injecting backend whose rollback fails mid-way returns
/// `OverlayRollbackOutcome::RequiresRebuild`; the session health becomes
/// RequiresRebuild and the NEXT solve forces a snapshot rebuild (no overlay
/// leak).
#[test]
fn rollback_uncertainty_marks_requires_rebuild_and_forces_rebuild() {
    let (mut model, _compiler, x, _y, _obj) = overlay_fixture();
    let (backend, state) = OverlayTestBackend::new();
    let mut session = SolverSession::new(backend);
    let _ = session.solve(&mut model).unwrap();
    let rev = model.current_revision();
    let rebuilds_before = state.borrow().rebuilds;

    // Inject a mid-rollback failure.
    state.borrow_mut().fail_rollback = true;
    let overlay = trivial_overlay(x);
    let result = session.solve_with_overlay(&mut model, SolveOptions::default(), &overlay, None);
    // The solve itself succeeded on C_overlay; the uncertain rollback marks the
    // session RequiresRebuild but the result is still returned.
    assert!(result.is_ok(), "the overlay solve result is still valid");
    assert_eq!(model.current_revision(), rev);
    state.borrow_mut().fail_rollback = false;

    // The next clean solve forces a snapshot rebuild (observed via the
    // backend's rebuild counter) and equals a fresh rebuild (no leak).
    let clean = session
        .solve(&mut model)
        .expect("clean solve after uncertain rollback");
    assert!(
        state.borrow().rebuilds > rebuilds_before,
        "the next solve must force a snapshot rebuild"
    );
    assert_eq!(clean.metadata().overlay_id, None);

    let (fresh_obj, _) = fresh_solve(&mut model.clone());
    assert_eq!(
        clean.objective_value(),
        fresh_obj,
        "a clean solve after an uncertain rollback must equal a fresh rebuild"
    );
}

// ── 4. Failure injection matrix (SM-07.6) ────────────────────────────────────

/// A deterministic fresh-session solve of `model` (the "fresh rebuild"
/// reference for leak assertions).
fn fresh_solve(model: &mut Model) -> (Option<f64>, Option<CompilationId>) {
    let (backend, _) = OverlayTestBackend::new();
    let mut session = SolverSession::new(backend);
    let solution = session.solve(model).expect("fresh solve succeeds");
    (
        solution.objective_value(),
        solution.metadata().compilation_id,
    )
}

/// Run one overlay scenario: establish the base, run `solve_with_overlay` with
/// the overlay produced by `configure` (which may also arm a backend fault),
/// assert the canonical revision is unchanged, then assert a subsequent clean
/// solve equals a fresh rebuild (no overlay leak). Returns the overlay result.
fn run_overlay_scenario(
    model: &mut Model,
    configure: impl FnOnce(&mut OverlayFaultState) -> SolveOverlay,
) -> (
    Result<roml::Solution, SolveError>,
    Rc<RefCell<OverlayFaultState>>,
) {
    let (backend, state) = OverlayTestBackend::new();
    let mut session = SolverSession::new(backend);
    let _ = session.solve(model).expect("base solve succeeds");
    let rev = model.current_revision();

    let overlay = configure(&mut state.borrow_mut());
    let result = session.solve_with_overlay(model, SolveOptions::default(), &overlay, None);

    assert_eq!(
        model.current_revision(),
        rev,
        "canonical revision must be unchanged"
    );
    if result.is_err() {
        // Clear the injected fault so the recovery solve runs clean.
        let mut s = state.borrow_mut();
        s.fail_solve = false;
        s.fail_apply = false;
        s.fail_apply_mid = false;
        s.fail_rollback = false;
        s.fail_verify = false;
        s.report_wrong_compilation = None;
        drop(s);
        // The session recovers: a subsequent clean solve equals a fresh rebuild.
        let clean = session
            .solve(model)
            .expect("clean solve after injected failure");
        let (fresh_obj, _) = fresh_solve(&mut model.clone());
        assert_eq!(
            clean.objective_value(),
            fresh_obj,
            "a clean solve after an injected failure must equal a fresh rebuild"
        );
    }
    (result, state)
}

/// Validation-stage failure: a wrong-lineage lock fails in `compile_overlay`
/// before any backend mutation -> `SolveError::Overlay(Assignment(_))`.
#[test]
fn injected_validation_failure_never_leaks() {
    let (mut model, _compiler, x, _y, _obj) = overlay_fixture();
    let (result, _state) = run_overlay_scenario(&mut model, |_s| {
        let mut other = Model::new();
        other.commit().unwrap();
        let assignment = PrimalAssignment {
            lineage: other.lineage(),
            source_instance: None,
            source_revision: None,
            values: BTreeMap::from([(x, 1.0)]),
        };
        SolveOverlay::new(
            BTreeMap::new(),
            vec![SolutionLock {
                assignment,
                selector: LockSelector::AllAssigned,
                continuous: ContinuousLock::Exact,
            }],
            vec![],
            vec![],
        )
        .unwrap()
    });
    assert!(matches!(
        result,
        Err(SolveError::Overlay(OverlayError::Assignment(_)))
    ));
}

/// Compile-stage failure: an unknown objective override fails in
/// `compile_overlay` -> `SolveError::Overlay(ObjectiveNotFound(_))`.
#[test]
fn injected_compile_failure_never_leaks() {
    let (mut model, _compiler, x, _y, _obj) = overlay_fixture();
    let ghost = roml::ObjId::new(999, roml::id::Generation::new());
    let (backend, state) = OverlayTestBackend::new();
    let mut session = SolverSession::new(backend);
    let _ = session.solve(&mut model).unwrap();
    let rev = model.current_revision();

    let overlay = trivial_overlay(x);
    let err = session
        .solve_with_overlay(&mut model, SolveOptions::default(), &overlay, Some(ghost))
        .expect_err("an unknown objective override must fail at compile time");
    assert!(
        matches!(err, SolveError::Overlay(OverlayError::ObjectiveNotFound(_))),
        "compile-stage failure must surface as SolveError::Overlay, got {err:?}"
    );
    assert_eq!(model.current_revision(), rev);
    let _ = state;

    session
        .solve(&mut model)
        .expect("clean solve after compile failure");
}

/// Apply-stage failure: the backend rejects `apply_overlay` and marks itself
/// RequiresRebuild; the overlay never leaks into a later solve.
#[test]
fn injected_apply_failure_never_leaks() {
    let (mut model, _compiler, x, _y, _obj) = overlay_fixture();
    let (result, _state) = run_overlay_scenario(&mut model, |s| {
        s.fail_apply = true;
        trivial_overlay(x)
    });
    assert!(
        matches!(result, Err(SolveError::Rollback(_))),
        "an apply-stage backend failure surfaces as SolveError::Rollback, got {result:?}"
    );
}

/// CR-02: a MID-apply failure (the second overlay op fails after the first
/// already mutated the backend) leaves the session `RequiresRebuild`, and a
/// subsequent plain solve FORCES a rebuild — the no-sync fast path never
/// silently reuses the half-overlaid state. The façade defensively forces the
/// rebuild even though the backend self-marked too.
#[test]
fn injected_mid_apply_failure_forces_rebuild_not_no_sync_reuse() {
    let (mut model, _compiler, x, _y, _obj) = overlay_fixture();
    let (backend, state) = OverlayTestBackend::new();
    let mut session = SolverSession::new(backend);
    let _ = session.solve(&mut model).expect("base solve succeeds");
    let rev = model.current_revision();

    let overlay = {
        state.borrow_mut().fail_apply_mid = true;
        trivial_overlay(x)
    };
    let result = session.solve_with_overlay(&mut model, SolveOptions::default(), &overlay, None);
    assert!(
        matches!(result, Err(SolveError::Rollback(_))),
        "a mid-apply failure surfaces as SolveError::Rollback, got {result:?}"
    );
    assert_eq!(
        model.current_revision(),
        rev,
        "canonical revision must be unchanged"
    );
    assert_eq!(
        state.borrow().health,
        AdapterHealth::RequiresRebuild,
        "a mid-apply failure must leave the session RequiresRebuild"
    );

    // Clear the fault: the next plain solve MUST force a rebuild (never a
    // silent no-sync reuse of the half-overlaid state) and reproduce a fresh
    // rebuild.
    let rebuilds_before = state.borrow().rebuilds;
    state.borrow_mut().fail_apply_mid = false;
    let clean = session
        .solve(&mut model)
        .expect("clean solve after mid-apply failure");
    let rebuilds_after = state.borrow().rebuilds;
    assert!(
        rebuilds_after > rebuilds_before,
        "a solve after a mid-apply failure must force a rebuild ({} -> {})",
        rebuilds_before,
        rebuilds_after
    );
    let (fresh_obj, _) = fresh_solve(&mut model.clone());
    assert_eq!(clean.objective_value(), fresh_obj);
    assert_eq!(
        state.borrow().health,
        AdapterHealth::Ready,
        "the forced rebuild must leave the session Ready"
    );
}

/// Solve-stage failure: the backend's solve fails -> `SolveError::Solve`.
#[test]
fn injected_solve_failure_never_leaks() {
    let (mut model, _compiler, x, _y, _obj) = overlay_fixture();
    let (result, _state) = run_overlay_scenario(&mut model, |s| {
        s.fail_solve = true;
        trivial_overlay(x)
    });
    assert!(matches!(result, Err(SolveError::Solve(_))));
}

/// Extraction-stage failure: the backend returns a result tagged with a
/// WRONG `CompilationId` -> `SolveError::CompilationMismatch`; rollback is
/// still attempted and no overlay leaks.
#[test]
fn injected_extraction_failure_never_leaks() {
    let (mut model, _compiler, x, _y, _obj) = overlay_fixture();
    // A FRESH compilation of the same model produces a DISTINCT exact
    // `CompilationId` (D28) — a forged result id that is neither C_base nor
    // C_overlay.
    let forged = reference_backend_at_base(&model).2.compilation_id;
    let (result, _state) = run_overlay_scenario(&mut model, |s| {
        s.report_wrong_compilation = Some(forged);
        trivial_overlay(x)
    });
    assert!(
        matches!(result, Err(SolveError::CompilationMismatch { .. })),
        "an extraction mismatch must surface as SolveError::CompilationMismatch, got {result:?}"
    );
}

/// Rollback-stage failure: `rollback_overlay` returns `RequiresRebuild`; the
/// overlay solve still returns a result, the session is marked RequiresRebuild,
/// and a subsequent clean solve rebuilds.
#[test]
fn injected_rollback_failure_never_leaks() {
    let (mut model, _compiler, x, _y, _obj) = overlay_fixture();
    let (result, _state) = run_overlay_scenario(&mut model, |s| {
        s.fail_rollback = true;
        trivial_overlay(x)
    });
    assert!(
        result.is_ok(),
        "an uncertain rollback still returns the overlay solve result, got {result:?}"
    );
}

/// Post-rollback verification failure: `verify_overlay_clean` fails -> the
/// façade surfaces `SolveError::Rollback` and a subsequent clean solve rebuilds.
#[test]
fn injected_verify_failure_never_leaks() {
    let (mut model, _compiler, x, _y, _obj) = overlay_fixture();
    let (result, _state) = run_overlay_scenario(&mut model, |s| {
        s.fail_verify = true;
        trivial_overlay(x)
    });
    assert!(
        matches!(result, Err(SolveError::Rollback(_))),
        "a post-rollback verification failure surfaces as SolveError::Rollback, got {result:?}"
    );
}

// ── 5. Task 10 test-backend scaffolding ──────────────────────────────────────

/// Shared fault-injection and observability state for [`OverlayTestBackend`].
struct OverlayFaultState {
    revision: ModelRevision,
    health: AdapterHealth,
    rebuilds: usize,
    solves: usize,
    fail_solve: bool,
    fail_apply: bool,
    /// CR-02: fail the overlay apply AFTER the first op mutated the backend
    /// (the injected bad op references an unknown compiled variable). The
    /// session must end RequiresRebuild and a subsequent plain solve must
    /// force a rebuild — never a silent no-sync reuse of the half-overlaid
    /// state.
    fail_apply_mid: bool,
    fail_rollback: bool,
    fail_verify: bool,
    report_wrong_compilation: Option<CompilationId>,
    current_compilation: Option<CompilationId>,
}

impl Default for OverlayFaultState {
    fn default() -> Self {
        Self {
            revision: ModelRevision::ZERO,
            health: AdapterHealth::Ready,
            rebuilds: 0,
            solves: 0,
            fail_solve: false,
            fail_apply: false,
            fail_apply_mid: false,
            fail_rollback: false,
            fail_verify: false,
            report_wrong_compilation: None,
            current_compilation: None,
        }
    }
}

/// A façade test backend wrapping a [`ReferenceBackend`] for the compiled
/// state and the overlay apply/rollback machinery, with fault-injection knobs
/// for every lifecycle stage. The "solve" reports a deterministic objective
/// value from the active objective's coefficients at unit variable values.
struct OverlayTestBackend {
    inner: ReferenceBackend,
    state: Rc<RefCell<OverlayFaultState>>,
    compiled_to_user_variable: HashMap<CompiledVariableId, roml::VarId>,
    compiled_to_user_objective: HashMap<CompiledObjectiveId, roml::ObjId>,
    var_values: HashMap<roml::VarId, f64>,
    objectives: HashMap<roml::ObjId, (Sense, f64)>,
    objective_cells: HashMap<roml::ObjId, HashMap<roml::VarId, f64>>,
    active_objective: Option<roml::ObjId>,
    typed_caps: BackendCapabilitySet,
    /// IN-03: the temporary bounds of the currently-applied overlay, keyed by
    /// user variable. `solve` clamps its deterministic unit `var_values` to
    /// these bounds, so an overlay-bounds leak changes the reported objective
    /// and the failure-injection matrix actually catches it (a solve that
    /// ignores overlay bounds cannot detect a bounds leak).
    overlay_bounds: HashMap<roml::VarId, Bounds>,
}

impl OverlayTestBackend {
    fn new() -> (Self, Rc<RefCell<OverlayFaultState>>) {
        let state = Rc::new(RefCell::new(OverlayFaultState::default()));
        (
            Self {
                inner: ReferenceBackend::new(),
                state: state.clone(),
                compiled_to_user_variable: HashMap::new(),
                compiled_to_user_objective: HashMap::new(),
                var_values: HashMap::new(),
                objectives: HashMap::new(),
                objective_cells: HashMap::new(),
                active_objective: None,
                typed_caps: full_typed_capabilities(),
                overlay_bounds: HashMap::new(),
            },
            state,
        )
    }

    /// IN-03: compute the objective against an explicit value map (the clamped
    /// overlay-solve values), so a leak of the overlay's temporary bounds into
    /// `solve` changes the reported objective and the failure-injection matrix
    /// catches it.
    fn compute_objective_value(&self, values: &HashMap<roml::VarId, f64>) -> Option<f64> {
        let obj = self.active_objective?;
        let constant = self.objectives.get(&obj).map(|(_, c)| *c).unwrap_or(0.0);
        let cells = self.objective_cells.get(&obj).cloned().unwrap_or_default();
        let sum: f64 = cells
            .iter()
            .map(|(var, cost)| *cost * values.get(var).copied().unwrap_or(0.0))
            .sum();
        Some(sum + constant)
    }

    /// Project a compiled backend snapshot into user-keyed solve state using
    /// the snapshot's mandatory origin map (SM-02.5).
    fn project_compiled_snapshot(&mut self, snapshot: &BackendSnapshot) {
        self.var_values.clear();
        self.objectives.clear();
        self.objective_cells.clear();
        self.compiled_to_user_variable.clear();
        self.compiled_to_user_objective.clear();
        for v in &snapshot.variables {
            if let Some(EntityOrigin::UserVariable(var)) = snapshot.origin_map.variable_origin(v.id)
            {
                self.compiled_to_user_variable.insert(v.id, *var);
                self.var_values.insert(*var, 1.0);
            }
        }
        for o in &snapshot.objectives {
            if let Some(EntityOrigin::UserObjective(obj)) =
                snapshot.origin_map.objective_origin(o.id)
            {
                self.compiled_to_user_objective.insert(o.id, *obj);
                self.objectives.insert(*obj, (o.sense, o.constant));
                let cells: HashMap<roml::VarId, f64> = o
                    .coefficients
                    .iter()
                    .filter_map(|(cid, val)| {
                        self.compiled_to_user_variable.get(cid).map(|v| (*v, *val))
                    })
                    .collect();
                self.objective_cells.insert(*obj, cells);
            }
        }
        self.active_objective = match &snapshot.objective_policy {
            CompiledObjectivePolicy::Single(cid) => {
                self.compiled_to_user_objective.get(cid).copied()
            }
            _ => None,
        };
    }

    /// Apply one compiled backend op, translating compiled ids to user ids via
    /// the maintained maps and the batch's origin additions.
    fn apply_compiled_op(&mut self, op: &BackendOp, origins: &roml::advanced::OriginMap) {
        match op {
            BackendOp::AddVariable(v) => {
                if let Some(EntityOrigin::UserVariable(var)) = origins.variable_origin(v.id) {
                    self.compiled_to_user_variable.insert(v.id, *var);
                    self.var_values.insert(*var, 1.0);
                }
            }
            BackendOp::RemoveVariable(cid) => {
                if let Some(var) = self.compiled_to_user_variable.remove(cid) {
                    self.var_values.remove(&var);
                    for cells in self.objective_cells.values_mut() {
                        cells.remove(&var);
                    }
                }
            }
            BackendOp::AddObjective(o) => {
                if let Some(EntityOrigin::UserObjective(obj)) = origins.objective_origin(o.id) {
                    self.compiled_to_user_objective.insert(o.id, *obj);
                    self.objectives.insert(*obj, (o.sense, o.constant));
                    self.objective_cells.entry(*obj).or_default();
                }
            }
            BackendOp::RemoveObjective(cid) => {
                if let Some(obj) = self.compiled_to_user_objective.remove(cid) {
                    self.objectives.remove(&obj);
                    self.objective_cells.remove(&obj);
                    if self.active_objective == Some(obj) {
                        self.active_objective = None;
                    }
                }
            }
            BackendOp::SetObjectiveCoefficient {
                objective,
                variable,
                value,
            } => {
                if let (Some(obj), Some(var)) = (
                    self.compiled_to_user_objective.get(objective).copied(),
                    self.compiled_to_user_variable.get(variable).copied(),
                ) {
                    self.objective_cells
                        .entry(obj)
                        .or_default()
                        .insert(var, *value);
                }
            }
            BackendOp::RemoveObjectiveCoefficient {
                objective,
                variable,
            } => {
                if let (Some(obj), Some(var)) = (
                    self.compiled_to_user_objective.get(objective).copied(),
                    self.compiled_to_user_variable.get(variable).copied(),
                ) {
                    if let Some(cells) = self.objective_cells.get_mut(&obj) {
                        cells.remove(&var);
                    }
                }
            }
            BackendOp::SetObjectiveConstant { objective, value } => {
                if let Some(obj) = self.compiled_to_user_objective.get(objective).copied() {
                    if let Some(entry) = self.objectives.get_mut(&obj) {
                        entry.1 = *value;
                    }
                }
            }
            BackendOp::SetObjectiveSense { objective, sense } => {
                if let Some(obj) = self.compiled_to_user_objective.get(objective).copied() {
                    if let Some(entry) = self.objectives.get_mut(&obj) {
                        entry.0 = *sense;
                    }
                }
            }
            BackendOp::SetObjectivePolicy(policy) => {
                self.active_objective = match policy {
                    CompiledObjectivePolicy::Single(cid) => {
                        self.compiled_to_user_objective.get(cid).copied()
                    }
                    _ => None,
                };
            }
            _ => {}
        }
    }
}

impl BackendMetadata for OverlayTestBackend {
    fn name(&self) -> &str {
        "OverlayTestBackend"
    }
    fn capabilities(&self) -> roml::BackendCapabilities {
        roml::BackendCapabilities::all()
    }
    fn typed_capabilities(&self) -> &BackendCapabilitySet {
        &self.typed_caps
    }
}

impl SessionHealth for OverlayTestBackend {
    fn health(&self) -> AdapterHealth {
        self.state.borrow().health
    }
    fn revision(&self) -> ModelRevision {
        self.state.borrow().revision
    }
}

impl BackendSession for OverlayTestBackend {
    fn synchronize(&mut self, sync: Synchronization) -> Result<SyncReceipt, BackendError> {
        match sync {
            Synchronization::CompiledRebuild(snapshot) => {
                {
                    let mut s = self.state.borrow_mut();
                    s.rebuilds += 1;
                }
                self.inner.rebuild_compiled(&snapshot).map_err(|e| {
                    BackendError::new(
                        e.to_string(),
                        ErrorCategory::Internal,
                        HealthEffect::RequiresRebuild,
                    )
                })?;
                self.project_compiled_snapshot(&snapshot);
                let mut s = self.state.borrow_mut();
                s.revision = snapshot.source_revision;
                s.health = AdapterHealth::Ready;
                s.current_compilation = Some(snapshot.compilation_id);
                Ok(SyncReceipt {
                    cursor: AdapterCursor {
                        applied_revision: s.revision,
                        health: s.health,
                    },
                    health: s.health,
                })
            }
            Synchronization::CompiledDeltaBatch(batch) => {
                {
                    let mut s = self.state.borrow_mut();
                    if batch.from_revision != s.revision {
                        s.health = AdapterHealth::RequiresRebuild;
                        return Err(BackendError::new(
                            format!(
                                "compiled delta from {} != backend at {}",
                                batch.from_revision, s.revision
                            ),
                            ErrorCategory::InvalidInput,
                            HealthEffect::Recoverable,
                        ));
                    }
                }
                self.inner.apply_compiled_delta(&batch).map_err(|e| {
                    BackendError::new(
                        e.to_string(),
                        ErrorCategory::Internal,
                        HealthEffect::RequiresRebuild,
                    )
                })?;
                let origins = batch.origin_additions.clone();
                for op in &batch.operations {
                    self.apply_compiled_op(op, &origins);
                }
                let mut s = self.state.borrow_mut();
                s.revision = batch.to_revision;
                s.health = AdapterHealth::Ready;
                s.current_compilation = Some(batch.to_compilation);
                Ok(SyncReceipt {
                    cursor: AdapterCursor {
                        applied_revision: s.revision,
                        health: s.health,
                    },
                    health: s.health,
                })
            }
            Synchronization::Rebuild(_) | Synchronization::DeltaBatch(_) => Err(BackendError::new(
                "canonical synchronization is not supported by the compiled overlay test backend",
                ErrorCategory::InvalidInput,
                HealthEffect::RequiresRebuild,
            )),
        }
    }

    fn solve(&mut self, _request: &SolveRequest) -> Result<SolveResult, BackendError> {
        let mut s = self.state.borrow_mut();
        s.solves += 1;
        if s.fail_solve {
            return Err(BackendError::new(
                "injected solve failure",
                ErrorCategory::Internal,
                HealthEffect::Recoverable,
            ));
        }
        let compilation_id = match s.report_wrong_compilation {
            Some(forged) => forged,
            None => s
                .current_compilation
                .expect("a solve must follow a compiled synchronization"),
        };
        drop(s);
        // IN-03: clamp the deterministic unit values to the applied overlay's
        // temporary bounds so an overlay-bounds leak changes the objective and
        // the matrix's "clean solve == fresh rebuild" assertions catch it.
        let clamped: HashMap<roml::VarId, f64> = self
            .var_values
            .iter()
            .map(|(var, v)| {
                let value = match self.overlay_bounds.get(var) {
                    Some(b) => (*v).max(b.lower).min(b.upper),
                    None => *v,
                };
                (*var, value)
            })
            .collect();
        let solution = SolveSolution {
            variable_values: clamped.iter().map(|(var, v)| (*var, *v)).collect(),
            objective_value: self.compute_objective_value(&clamped),
            dual_values: None,
            reduced_costs: None,
        };
        Ok(SolveResult {
            effective_configuration: EffectiveConfig::default(),
            termination: TerminationStatus::Optimal,
            solution: Some(solution),
            compilation_id: Some(compilation_id),
            overlay_id: None,
        })
    }

    fn close(self) -> Result<(), BackendError> {
        Ok(())
    }
}

impl OverlaySession for OverlayTestBackend {
    fn apply_overlay(
        &mut self,
        overlay: &roml::advanced::CompiledOverlay,
    ) -> Result<OverlayApplyReceipt, BackendError> {
        let mut s = self.state.borrow_mut();
        if s.fail_apply {
            s.health = AdapterHealth::RequiresRebuild;
            return Err(BackendError::new(
                "injected apply failure",
                ErrorCategory::Internal,
                HealthEffect::RequiresRebuild,
            ));
        }
        let inject_mid = s.fail_apply_mid;
        drop(s);

        // CR-02 mid-apply injection: append a bogus op (an unknown compiled
        // variable) AFTER the real overlay ops, so the apply fails after op 1
        // already mutated the backend — exactly "op N fails after ops
        // 1..N-1 mutated".
        let mut mid_overlay: Option<roml::advanced::CompiledOverlay> = None;
        if inject_mid {
            let mut mutated = overlay.clone();
            mutated
                .operations
                .push(OverlayOp::SetTemporaryVariableBounds {
                    variable: CompiledVariableId(u32::MAX),
                    bounds: Bounds::new(1.0, 1.0),
                });
            mid_overlay = Some(mutated);
        }
        let overlay_ref = mid_overlay.as_ref().unwrap_or(overlay);

        let receipt = match self.inner.apply_overlay(overlay_ref) {
            Ok(receipt) => receipt,
            Err(e) => {
                // CR-02: a partial apply must leave the session
                // RequiresRebuild — never a Ready session silently reusing the
                // half-overlaid state.
                self.state.borrow_mut().health = AdapterHealth::RequiresRebuild;
                return Err(e);
            }
        };

        // IN-03: record the overlay's temporary bounds so `solve` honors them
        // (clamping its unit values); a bounds leak then changes the objective.
        let mut overlay_bounds = HashMap::new();
        for op in &overlay.operations {
            if let OverlayOp::SetTemporaryVariableBounds { variable, bounds } = op {
                if let Some(var) = self.compiled_to_user_variable.get(variable) {
                    overlay_bounds.insert(*var, *bounds);
                }
            }
        }
        self.overlay_bounds = overlay_bounds;

        let mut s = self.state.borrow_mut();
        s.current_compilation = Some(overlay.compilation_id);
        Ok(receipt)
    }

    fn rollback_overlay(
        &mut self,
        receipt: &OverlayApplyReceipt,
    ) -> Result<OverlayRollbackOutcome, BackendError> {
        let mut s = self.state.borrow_mut();
        if s.fail_rollback {
            s.health = AdapterHealth::RequiresRebuild;
            s.current_compilation = None;
            return Ok(OverlayRollbackOutcome::RequiresRebuild {
                reason: "injected rollback failure".into(),
            });
        }
        drop(s);
        let outcome = self.inner.rollback_overlay(receipt)?;
        let mut s = self.state.borrow_mut();
        if let OverlayRollbackOutcome::Clean {
            restored_compilation,
        } = &outcome
        {
            s.current_compilation = Some(*restored_compilation);
        }
        // IN-03: a Clean rollback restores the base solve state — the overlay
        // bounds must not leak into the next plain solve.
        if matches!(outcome, OverlayRollbackOutcome::Clean { .. }) {
            self.overlay_bounds.clear();
        }
        Ok(outcome)
    }

    fn verify_overlay_clean(&mut self) -> Result<(), BackendError> {
        let mut s = self.state.borrow_mut();
        if s.fail_verify {
            s.health = AdapterHealth::RequiresRebuild;
            return Err(BackendError::new(
                "injected post-rollback verification failure",
                ErrorCategory::Internal,
                HealthEffect::RequiresRebuild,
            ));
        }
        drop(s);
        self.inner.verify_overlay_clean()
    }
}
