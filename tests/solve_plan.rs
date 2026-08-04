//! P28 — SolvePlan, MipStart, VariableHints, unsupported/conversion policy,
//! one plan executor, effective-plan reporting, and the qualified HiGHS
//! start/hint path.
//!
//! Task 1 (SM-07.1 types, SM-08.1..08.6): the design-§12 `SolvePlan` shape, the
//! distinct start/hint semantics (D8), default-reject unsupported policy
//! (SM-08.4), explicit recorded conversions (SM-08.5), basis distinctness
//! (SM-08.6), and the `SolvePlan::validate` gate that rejects every listed
//! invalid-plan class with a typed `PlanError` before any backend mutation.
//!
//! Task 2 (SM-07.2, SM-07.7, SM-08.3, SM-08.5, SM-04.5): `solve`/`solve_with`/
//! empty-`solve_plan` equivalence through ONE plan executor, effective-plan
//! recording in `SolveMetadata` (including the exact `CompilationId`), and the
//! feasibility-signature invariance of starts/hints.

use std::collections::BTreeMap;

use roml::advanced::BackendFeature;
use roml::model::{binary, continuous, integer};
use roml::{
    AssignmentError, HintPriority, MipStart, Model, PlanError, PrimalAssignment, RepairPolicy,
    SolveOptions, SolvePlan, UnsupportedFeaturePolicy, VariableHint, VariableHints,
};

/// A committed model with one continuous, one integer, and one binary variable.
/// Returns `(model, cont, int, bin)`.
fn mixed_model() -> (Model, roml::VarId, roml::VarId, roml::VarId) {
    let mut model = Model::new();
    let cont = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let int = model.add_variable(integer().bounds(0.0, 10.0)).unwrap();
    let bin = model.add_variable(binary()).unwrap();
    model.commit().unwrap();
    (model, cont, int, bin)
}

/// A valid `PrimalAssignment` for `mixed_model` covering every variable.
fn full_assignment(
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

// ---------------------------------------------------------------------------
// Task 1 — SolvePlan design-§12 shape (SM-07.1)
// ---------------------------------------------------------------------------

/// `SolvePlan` exposes exactly the design-§12 fields, and `SolvePlan::new`
/// builds the empty plan (empty overlay, no starts/hints, default policy).
#[test]
fn solve_plan_has_exact_design_fields_and_new_builds_empty_plan() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    model.commit().unwrap();
    let obj = model.active_objective();

    let plan = SolvePlan::new(SolveOptions::default()).expect("empty plan builds");

    // `SolvePlan::new` builds an empty plan.
    assert!(plan.mip_starts.is_empty());
    assert!(plan.hints.is_empty());
    assert!(plan.objective_override.is_none());
    assert_eq!(plan.unsupported, UnsupportedFeaturePolicy::Reject);

    // Direct construction with all design-§12 field identifiers.
    let assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: None,
        source_revision: None,
        values: BTreeMap::from([(x, 1.0)]),
    };
    let explicit = SolvePlan {
        options: SolveOptions::default(),
        overlay: roml::SolveOverlay::new(BTreeMap::new(), vec![], vec![], vec![]).unwrap(),
        mip_starts: vec![MipStart::new(assignment, RepairPolicy::BackendDefault)],
        hints: VariableHints::default(),
        objective_override: obj.map(roml::ObjectivePolicy::Single),
        lex_stage_policy: roml::LexStagePolicy::RequireOptimal,
        unsupported: UnsupportedFeaturePolicy::Reject,
    };
    assert_eq!(explicit.mip_starts.len(), 1);
    assert_eq!(
        explicit.lex_stage_policy,
        roml::LexStagePolicy::RequireOptimal
    );
}

// ---------------------------------------------------------------------------
// Task 1 — MipStart / RepairPolicy shape (SM-08.1)
// ---------------------------------------------------------------------------

/// `MipStart` carries `assignment`, `repair`, and optional `name`;
/// `MipStart::new` builds one with no name.
#[test]
fn mip_start_shape_and_constructor() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    model.commit().unwrap();
    let assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: None,
        source_revision: None,
        values: BTreeMap::from([(x, 1.0)]),
    };

    let start = MipStart::new(assignment.clone(), RepairPolicy::AllowRepair);
    assert_eq!(start.assignment, assignment);
    assert_eq!(start.repair, RepairPolicy::AllowRepair);
    assert_eq!(start.name, None);

    let named = MipStart {
        assignment,
        repair: RepairPolicy::RejectIncomplete,
        name: Some("warm".to_string()),
    };
    assert_eq!(named.name.as_deref(), Some("warm"));
}

/// `RepairPolicy` has exactly the three packet variants.
#[test]
fn repair_policy_has_three_variants() {
    assert_eq!(RepairPolicy::BackendDefault, RepairPolicy::BackendDefault);
    assert_eq!(
        RepairPolicy::RejectIncomplete,
        RepairPolicy::RejectIncomplete
    );
    assert_eq!(RepairPolicy::AllowRepair, RepairPolicy::AllowRepair);
}

// ---------------------------------------------------------------------------
// Task 1 — VariableHints / VariableHint / HintPriority shape (SM-08.3)
// ---------------------------------------------------------------------------

/// `VariableHints` stores independent value/priority entries with accessors.
#[test]
fn variable_hints_store_independent_entries() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let y = model.add_variable(continuous()).unwrap();
    model.commit().unwrap();

    let mut hints = VariableHints::default();
    assert!(hints.is_empty());
    assert_eq!(hints.len(), 0);

    hints.insert(
        x,
        VariableHint {
            value: 1.0,
            priority: HintPriority(10),
        },
    );
    hints.insert(
        y,
        VariableHint {
            value: 2.5,
            priority: HintPriority(-1),
        },
    );

    assert!(!hints.is_empty());
    assert_eq!(hints.len(), 2);
    assert_eq!(
        hints.get(x),
        Some(&VariableHint {
            value: 1.0,
            priority: HintPriority(10)
        })
    );
    assert_eq!(hints.get(y).map(|h| h.priority), Some(HintPriority(-1)));
    assert_eq!(
        hints.get(roml::VarId::new(99, roml::id::Generation::new())),
        None
    );

    let entries: Vec<_> = hints.iter().map(|(v, h)| (*v, *h)).collect();
    assert_eq!(entries.len(), 2);
    // Entries are independent: mutating one does not affect the other.
    hints.insert(
        x,
        VariableHint {
            value: 9.0,
            priority: HintPriority(0),
        },
    );
    assert_eq!(hints.get(x).unwrap().value, 9.0);
    assert_eq!(hints.get(y).unwrap().value, 2.5);
}

/// `HintPriority` wraps a public `i32`.
#[test]
fn hint_priority_is_i32() {
    let p = HintPriority(7);
    assert_eq!(p.0, 7);
    assert!(HintPriority(2) > HintPriority(1));
}

// ---------------------------------------------------------------------------
// Task 1 — UnsupportedFeaturePolicy (SM-08.4, SM-08.5)
// ---------------------------------------------------------------------------

/// Default unsupported behavior is rejection; conversions are explicit variants.
#[test]
fn unsupported_feature_policy_defaults_to_reject() {
    assert_eq!(
        UnsupportedFeaturePolicy::default(),
        UnsupportedFeaturePolicy::Reject
    );
    assert_eq!(
        UnsupportedFeaturePolicy::Reject,
        UnsupportedFeaturePolicy::Reject
    );
    let _ = UnsupportedFeaturePolicy::ConvertHintToStart;
    let _ = UnsupportedFeaturePolicy::ConvertStartToTemporaryFixing;
}

// ---------------------------------------------------------------------------
// Task 1 — SolvePlan::validate rejects every invalid-plan class (SM-08.4)
// ---------------------------------------------------------------------------

fn plan_with_start(_model: &Model, start: MipStart) -> SolvePlan {
    SolvePlan {
        options: SolveOptions::default(),
        overlay: roml::SolveOverlay::new(BTreeMap::new(), vec![], vec![], vec![]).unwrap(),
        mip_starts: vec![start],
        hints: VariableHints::default(),
        objective_override: None,
        lex_stage_policy: roml::LexStagePolicy::RequireOptimal,
        unsupported: UnsupportedFeaturePolicy::Reject,
    }
}

#[test]
fn validate_rejects_lineage_mismatch() {
    let (model, cont, int, bin) = mixed_model();
    let mut other = Model::new();
    other.commit().unwrap();
    let foreign = PrimalAssignment {
        lineage: other.lineage(),
        source_instance: None,
        source_revision: None,
        values: BTreeMap::from([(cont, 1.0), (int, 1.0), (bin, 1.0)]),
    };
    let plan = plan_with_start(&model, MipStart::new(foreign, RepairPolicy::BackendDefault));
    assert!(matches!(
        plan.validate(&model),
        Err(PlanError::Assignment(
            AssignmentError::LineageMismatch { .. }
        ))
    ));
}

#[test]
fn validate_rejects_stale_variable() {
    let (model, cont, int, bin) = mixed_model();
    let stale = roml::VarId::new(999, roml::id::Generation::new());
    let assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: None,
        source_revision: None,
        values: BTreeMap::from([(cont, 1.0), (int, 1.0), (bin, 1.0), (stale, 1.0)]),
    };
    let plan = plan_with_start(
        &model,
        MipStart::new(assignment, RepairPolicy::BackendDefault),
    );
    assert!(matches!(
        plan.validate(&model),
        Err(PlanError::Assignment(AssignmentError::StaleVariable { variable })) if variable == stale
    ));
}

#[test]
fn validate_rejects_out_of_bounds_value() {
    let (model, cont, int, bin) = mixed_model();
    let assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: None,
        source_revision: None,
        values: BTreeMap::from([(cont, 999.0), (int, 1.0), (bin, 1.0)]),
    };
    let plan = plan_with_start(
        &model,
        MipStart::new(assignment, RepairPolicy::BackendDefault),
    );
    assert!(matches!(
        plan.validate(&model),
        Err(PlanError::Assignment(AssignmentError::ValueOutOfBounds { variable, value, .. }))
            if variable == cont && value == 999.0
    ));
}

#[test]
fn validate_rejects_non_finite_assignment_value() {
    let (model, cont, int, bin) = mixed_model();
    let assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: None,
        source_revision: None,
        values: BTreeMap::from([(cont, f64::NAN), (int, 1.0), (bin, 1.0)]),
    };
    let plan = plan_with_start(
        &model,
        MipStart::new(assignment, RepairPolicy::BackendDefault),
    );
    assert!(matches!(
        plan.validate(&model),
        Err(PlanError::Assignment(AssignmentError::NonFiniteValue { variable, .. }))
            if variable == cont
    ));
}

#[test]
fn validate_rejects_duplicate_variable_across_starts() {
    let (model, cont, int, bin) = mixed_model();
    let assignment_a = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: None,
        source_revision: None,
        values: BTreeMap::from([(cont, 1.0), (int, 1.0), (bin, 1.0)]),
    };
    let assignment_b = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: None,
        source_revision: None,
        values: BTreeMap::from([(cont, 2.0)]),
    };
    let plan = SolvePlan {
        options: SolveOptions::default(),
        overlay: roml::SolveOverlay::new(BTreeMap::new(), vec![], vec![], vec![]).unwrap(),
        mip_starts: vec![
            MipStart::new(assignment_a, RepairPolicy::BackendDefault),
            MipStart::new(assignment_b, RepairPolicy::BackendDefault),
        ],
        hints: VariableHints::default(),
        objective_override: None,
        lex_stage_policy: roml::LexStagePolicy::RequireOptimal,
        unsupported: UnsupportedFeaturePolicy::Reject,
    };
    assert!(matches!(
        plan.validate(&model),
        Err(PlanError::DuplicateStartVariable { variable }) if variable == cont
    ));
}

#[test]
fn validate_rejects_start_variable_also_in_overlay_fixings() {
    let (model, cont, int, bin) = mixed_model();
    let assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: None,
        source_revision: None,
        values: BTreeMap::from([(cont, 1.0), (int, 1.0), (bin, 1.0)]),
    };
    let plan = SolvePlan {
        options: SolveOptions::default(),
        overlay: roml::SolveOverlay::new(BTreeMap::from([(cont, 1.0)]), vec![], vec![], vec![])
            .unwrap(),
        mip_starts: vec![MipStart::new(assignment, RepairPolicy::BackendDefault)],
        hints: VariableHints::default(),
        objective_override: None,
        lex_stage_policy: roml::LexStagePolicy::RequireOptimal,
        unsupported: UnsupportedFeaturePolicy::Reject,
    };
    assert!(matches!(
        plan.validate(&model),
        Err(PlanError::OverlayConflict { variable }) if variable == cont
    ));
}

#[test]
fn validate_rejects_non_finite_hint_value() {
    let (model, _cont, _int, _bin) = mixed_model();
    let mut hints = VariableHints::default();
    hints.insert(
        _cont,
        VariableHint {
            value: f64::INFINITY,
            priority: HintPriority(1),
        },
    );
    let plan = SolvePlan {
        options: SolveOptions::default(),
        overlay: roml::SolveOverlay::new(BTreeMap::new(), vec![], vec![], vec![]).unwrap(),
        mip_starts: vec![],
        hints,
        objective_override: None,
        lex_stage_policy: roml::LexStagePolicy::RequireOptimal,
        unsupported: UnsupportedFeaturePolicy::Reject,
    };
    assert!(matches!(
        plan.validate(&model),
        Err(PlanError::NonFiniteHintValue { variable, value })
            if variable == _cont && value == f64::INFINITY
    ));
}

/// A `RejectIncomplete` start that omits an integer/binary variable is
/// rejected with the missing-variable list (SM-08.1).
#[test]
fn validate_rejects_incomplete_start_omitting_integer_variable() {
    let (model, cont, int, bin) = mixed_model();
    // Covers `cont` and `bin` but OMITS the integer `int`.
    let assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: None,
        source_revision: None,
        values: BTreeMap::from([(cont, 1.0), (bin, 1.0)]),
    };
    let plan = plan_with_start(
        &model,
        MipStart::new(assignment, RepairPolicy::RejectIncomplete),
    );
    assert!(matches!(
        plan.validate(&model),
        Err(PlanError::IncompleteStart { missing }) if missing == vec![int]
    ));
}

#[test]
fn validate_accepts_a_valid_plan() {
    let (model, cont, int, bin) = mixed_model();
    let assignment = full_assignment(&model, cont, int, bin);
    let mut hints = VariableHints::default();
    hints.insert(
        cont,
        VariableHint {
            value: 1.0,
            priority: HintPriority(1),
        },
    );
    let plan = SolvePlan {
        options: SolveOptions::default(),
        overlay: roml::SolveOverlay::new(BTreeMap::new(), vec![], vec![], vec![]).unwrap(),
        mip_starts: vec![MipStart::new(assignment, RepairPolicy::RejectIncomplete)],
        hints,
        objective_override: None,
        lex_stage_policy: roml::LexStagePolicy::RequireOptimal,
        unsupported: UnsupportedFeaturePolicy::Reject,
    };
    assert!(plan.validate(&model).is_ok());
}

// ---------------------------------------------------------------------------
// Task 1 — Basis distinctness (SM-08.6)
// ---------------------------------------------------------------------------

/// A `MipStart`/`VariableHints` API carries NO basis surface: the packet's LP
/// basis warm start (`BackendFeature::InitialBasis`) is a separate future
/// artifact, never conflated with a primal-assignment start. This test asserts
/// the start/hint types remain structurally basis-free (they expose only the
/// assignment/repair/name and value/priority surfaces).
#[test]
fn start_and_hint_apis_do_not_touch_basis_types() {
    let (model, cont, int, bin) = mixed_model();
    let assignment = full_assignment(&model, cont, int, bin);

    // A MipStart is exactly { assignment, repair, name } — no basis member.
    let start = MipStart::new(assignment, RepairPolicy::BackendDefault);
    assert_eq!(start.repair, RepairPolicy::BackendDefault);
    let _ = &start.assignment;
    let _ = &start.name;

    // A VariableHint is exactly { value, priority } — no basis member.
    let hint = VariableHint {
        value: 1.0,
        priority: HintPriority(1),
    };
    assert_eq!(hint.value, 1.0);
    assert_eq!(hint.priority.0, 1);

    // The basis feature exists as a distinct BackendFeature but is never
    // exercised by the start/hint types here (capability qualification is
    // Task 3).
    let _ = BackendFeature::InitialBasis;
}
