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
    AssignmentError, ConstraintExprExt, HintPriority, MipStart, Model, PlanError, PrimalAssignment,
    RepairPolicy, SolveOptions, SolvePlan, SolveStatus, UnsupportedFeaturePolicy, VariableHint,
    VariableHints,
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

// ---------------------------------------------------------------------------
// Task 2 — one plan executor, effective-plan recording, and the
//          feasibility/stale-leak/conversion proofs (SM-07.2, SM-07.7,
//          SM-08.3, SM-08.5, SM-04.5)
// ---------------------------------------------------------------------------

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use roml::advanced::{
    BackendCapabilitySet, BackendOp, BackendSnapshot, CompilationId, CompiledObjectiveId,
    CompiledOverlay, CompiledVariableId, EntityOrigin, FeatureSupport, OriginMap,
    OverlayApplyReceipt, OverlayOp, OverlayRollbackOutcome, OverlaySession, SolveRequest,
    SolveResult, SolveSolution, SupportLevel, Synchronization,
};
use roml::model::objective::Sense;
use roml::revision::ModelRevision;
use roml::solver::backend::{
    BackendCapabilities, BackendError, ErrorCategory, HealthEffect, TerminationStatus,
};
use roml::solver::reference::ReferenceBackend;
use roml::solver::request::EffectiveConfig;
use roml::solver::session::{BackendMetadata, BackendSession, SessionHealth, SyncReceipt};
use roml::sync::{AdapterCursor, AdapterHealth};
use roml::{Bounds, ObjId, SolveError, SolverSession, VarId};

/// A typed capability set declaring the full P28 surface native: the M2
/// primitives plus `MipStart`, `PartialMipStart`, and `VariableHints`.
fn warm_start_caps() -> BackendCapabilitySet {
    let mut set = BackendCapabilitySet::new();
    for feature in [
        BackendFeature::Lp,
        BackendFeature::Mip,
        BackendFeature::IncrementalBounds,
        BackendFeature::IncrementalRows,
        BackendFeature::IncrementalCoefficients,
        BackendFeature::MipStart,
        BackendFeature::PartialMipStart,
        BackendFeature::VariableHints,
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

/// A capability set with the M2 primitives and `MipStart`/`PartialMipStart`
/// but NOT `VariableHints` (for the default-rejection and conversion tests).
fn caps_without_variable_hints() -> BackendCapabilitySet {
    let mut set = warm_start_caps();
    set.set(
        BackendFeature::VariableHints,
        FeatureSupport::unsupported(Default::default()),
    );
    set
}

/// A capability set WITHOUT `MipStart`/`PartialMipStart`/`VariableHints`
/// (for the start->fixing conversion and default-reject tests).
fn caps_without_starts_or_hints() -> BackendCapabilitySet {
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
    for feature in [
        BackendFeature::MipStart,
        BackendFeature::PartialMipStart,
        BackendFeature::VariableHints,
    ] {
        set.set(feature, FeatureSupport::unsupported(Default::default()));
    }
    set
}

/// Shared observability state for [`PlanTestBackend`].
struct PlanTestState {
    revision: ModelRevision,
    health: AdapterHealth,
    solves: usize,
    current_compilation: Option<CompilationId>,
    fail_solve: bool,
}

impl Default for PlanTestState {
    fn default() -> Self {
        Self {
            revision: ModelRevision::ZERO,
            health: AdapterHealth::Ready,
            solves: 0,
            current_compilation: None,
            fail_solve: false,
        }
    }
}

/// A façade test backend for the P28 plan executor.
///
/// Wraps a [`ReferenceBackend`] for the compiled state and the overlay
/// apply/rollback/verify machinery. The deterministic `solve` reports values
/// computed from the projected objective coefficients at:
///
/// ```text
/// unit values -> clamped to the applied overlay's temporary bounds
///             -> overridden by the currently applied MIP start (one-shot)
/// ```
///
/// The start is CONSUMED by `solve` (cleared afterward), modelling a one-shot
/// warm-start lifecycle so a stale start can never seed a later solve (the
/// no-stale-leak assertion). Applied starts/hints are recorded in
/// [`OverlaySession`] overrides so the effective plan and the solve
/// observably agree.
struct PlanTestBackend {
    inner: ReferenceBackend,
    state: Rc<RefCell<PlanTestState>>,
    typed_caps: BackendCapabilitySet,
    compiled_to_user_variable: HashMap<CompiledVariableId, VarId>,
    compiled_to_user_objective: HashMap<CompiledObjectiveId, ObjId>,
    var_values: HashMap<VarId, f64>,
    objectives: HashMap<ObjId, (Sense, f64)>,
    objective_cells: HashMap<ObjId, HashMap<VarId, f64>>,
    active_objective: Option<ObjId>,
    overlay_bounds: HashMap<VarId, Bounds>,
    current_start: HashMap<VarId, f64>,
    hints_applied: usize,
}

impl PlanTestBackend {
    fn new() -> (Self, Rc<RefCell<PlanTestState>>) {
        Self::new_with_caps(warm_start_caps())
    }

    fn new_with_caps(caps: BackendCapabilitySet) -> (Self, Rc<RefCell<PlanTestState>>) {
        let state = Rc::new(RefCell::new(PlanTestState::default()));
        (
            Self {
                inner: ReferenceBackend::new(),
                state: state.clone(),
                typed_caps: caps,
                compiled_to_user_variable: HashMap::new(),
                compiled_to_user_objective: HashMap::new(),
                var_values: HashMap::new(),
                objectives: HashMap::new(),
                objective_cells: HashMap::new(),
                active_objective: None,
                overlay_bounds: HashMap::new(),
                current_start: HashMap::new(),
                hints_applied: 0,
            },
            state,
        )
    }

    fn compute_objective_value(&self, values: &HashMap<VarId, f64>) -> Option<f64> {
        let obj = self.active_objective?;
        let constant = self.objectives.get(&obj).map(|(_, c)| *c).unwrap_or(0.0);
        let cells = self.objective_cells.get(&obj).cloned().unwrap_or_default();
        let sum: f64 = cells
            .iter()
            .map(|(var, cost)| *cost * values.get(var).copied().unwrap_or(0.0))
            .sum();
        Some(sum + constant)
    }

    fn project_compiled_snapshot(&mut self, snapshot: &BackendSnapshot) {
        self.var_values.clear();
        self.objectives.clear();
        self.objective_cells.clear();
        self.compiled_to_user_variable.clear();
        self.compiled_to_user_objective.clear();
        self.current_start.clear();
        self.overlay_bounds.clear();
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
                let cells: HashMap<VarId, f64> = o
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
            roml::advanced::CompiledObjectivePolicy::Single(cid) => {
                self.compiled_to_user_objective.get(cid).copied()
            }
            _ => None,
        };
    }

    fn apply_compiled_op(&mut self, op: &BackendOp, origins: &OriginMap) {
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
                    roml::advanced::CompiledObjectivePolicy::Single(cid) => {
                        self.compiled_to_user_objective.get(cid).copied()
                    }
                    _ => None,
                };
            }
            _ => {}
        }
    }
}

impl BackendMetadata for PlanTestBackend {
    fn name(&self) -> &str {
        "PlanTestBackend"
    }
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::all()
    }
    fn typed_capabilities(&self) -> &BackendCapabilitySet {
        &self.typed_caps
    }
}

impl SessionHealth for PlanTestBackend {
    fn health(&self) -> AdapterHealth {
        self.state.borrow().health
    }
    fn revision(&self) -> ModelRevision {
        self.state.borrow().revision
    }
}

impl BackendSession for PlanTestBackend {
    fn synchronize(&mut self, sync: Synchronization) -> Result<SyncReceipt, BackendError> {
        match sync {
            Synchronization::CompiledRebuild(snapshot) => {
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
                drop(s);
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
                "canonical synchronization is not supported by the compiled plan test backend",
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
        let compilation_id = s.current_compilation.ok_or_else(|| {
            BackendError::new(
                "solve before any compiled synchronization",
                ErrorCategory::Internal,
                HealthEffect::RequiresRebuild,
            )
        })?;
        drop(s);

        let mut values = self.var_values.clone();
        for (var, b) in &self.overlay_bounds {
            if let Some(v) = values.get_mut(var) {
                *v = (*v).max(b.lower).min(b.upper);
            }
        }
        for (var, value) in &self.current_start {
            values.insert(*var, *value);
        }
        let solution = SolveSolution {
            variable_values: values.iter().map(|(var, v)| (*var, *v)).collect(),
            objective_value: self.compute_objective_value(&values),
            dual_values: None,
            reduced_costs: None,
        };
        // One-shot warm-start lifecycle: the start is consumed by this solve,
        // so a stale incumbent can never seed an unrelated later solve.
        self.current_start.clear();
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

impl OverlaySession for PlanTestBackend {
    fn apply_overlay(
        &mut self,
        overlay: &CompiledOverlay,
    ) -> Result<OverlayApplyReceipt, BackendError> {
        let receipt = self.inner.apply_overlay(overlay)?;
        let mut overlay_bounds = HashMap::new();
        for op in &overlay.operations {
            if let OverlayOp::SetTemporaryVariableBounds { variable, bounds } = op {
                if let Some(var) = self.compiled_to_user_variable.get(variable) {
                    overlay_bounds.insert(*var, *bounds);
                }
            }
        }
        self.overlay_bounds = overlay_bounds;
        self.state.borrow_mut().current_compilation = Some(overlay.compilation_id);
        Ok(receipt)
    }

    fn rollback_overlay(
        &mut self,
        receipt: &OverlayApplyReceipt,
    ) -> Result<OverlayRollbackOutcome, BackendError> {
        let outcome = self.inner.rollback_overlay(receipt)?;
        let mut s = self.state.borrow_mut();
        if let OverlayRollbackOutcome::Clean {
            restored_compilation,
        } = &outcome
        {
            s.current_compilation = Some(*restored_compilation);
            self.overlay_bounds.clear();
        }
        Ok(outcome)
    }

    fn verify_overlay_clean(&mut self) -> Result<(), BackendError> {
        self.inner.verify_overlay_clean()
    }

    fn apply_mip_starts(&mut self, starts: &[MipStart]) -> Result<(), BackendError> {
        for start in starts {
            for (var, value) in &start.assignment.values {
                self.current_start.insert(*var, *value);
            }
        }
        Ok(())
    }

    fn apply_variable_hints(&mut self, _hints: &VariableHints) -> Result<(), BackendError> {
        self.hints_applied += 1;
        Ok(())
    }
}

/// A MIP fixture: maximize `3x + y` over two binary/integer variables with a
/// capacity row. Returns `(model, x, y, obj)`.
fn mip_fixture() -> (Model, VarId, VarId, ObjId) {
    let mut model = Model::new();
    let x = model.add_variable(integer().bounds(0.0, 10.0)).unwrap();
    let y = model.add_variable(integer().bounds(0.0, 10.0)).unwrap();
    model.add_constraint((x + y).le(10.0)).unwrap();
    let obj = model.maximize(3.0 * x + y).unwrap();
    (model, x, y, obj)
}

fn assignment_for(model: &Model, values: BTreeMap<VarId, f64>) -> PrimalAssignment {
    PrimalAssignment {
        lineage: model.lineage(),
        source_instance: Some(model.instance()),
        source_revision: Some(model.current_revision()),
        values,
    }
}

// ── 1. Equivalence: solve == solve_with == empty solve_plan (SM-07.2) ───────

/// On ONE session against an established base, `solve()`, `solve_with(...)`,
/// and an empty `solve_plan(...)` produce identical status, objective value,
/// primal values, synchronization mode, revision, and the exact
/// `CompilationId` in metadata — the single-executor equivalence proof.
#[test]
fn solve_solve_with_and_empty_solve_plan_are_equivalent() {
    let (mut model, _x, _y, _obj) = mip_fixture();
    let (backend, _state) = PlanTestBackend::new();
    let mut session = SolverSession::new(backend);

    // Establish C_base with a warm-up solve so the three paths all run on the
    // same current compiled state (NoChange sync).
    let warm = session.solve(&mut model).unwrap();
    let c_base = warm.metadata().compilation_id.expect("base compilation");
    let rev = model.current_revision();

    let s1 = session.solve(&mut model).unwrap();
    let s2 = session
        .solve_with(&mut model, SolveOptions::default())
        .unwrap();
    let s3 = session
        .solve_plan(&mut model, SolvePlan::new(SolveOptions::default()).unwrap())
        .unwrap();

    for s in [&s1, &s2, &s3] {
        assert_eq!(s.status(), SolveStatus::Optimal);
        assert_eq!(s.objective_value(), warm.objective_value());
        assert_eq!(s.metadata().model_revision, rev);
        assert_eq!(
            s.metadata().synchronization,
            roml::SynchronizationMode::NoChange
        );
        assert_eq!(s.metadata().compilation_id, Some(c_base));
        assert_eq!(s.metadata().overlay_id, None);
        // Every real solve carries an effective plan (empty here) with no
        // objective stages in P28.
        assert!(s.metadata().effective_plan.objective_stages.is_empty());
        assert!(s.metadata().effective_plan.applied_features.is_empty());
        assert!(s.metadata().effective_plan.adjustments.is_empty());
        assert!(s.metadata().effective_plan.rejections.is_empty());
    }

    // Identical primal values.
    for var in [_x, _y] {
        assert_eq!(s1.value(var), s2.value(var));
        assert_eq!(s1.value(var), s3.value(var));
    }
}

// ── 2. Metadata recording (SM-07.7, SM-04.5) ────────────────────────────────

/// Every real solve's metadata carries an `EffectiveSolvePlan` and the exact
/// `CompilationId`; applied starts/hints appear in `applied_features`;
/// conversions appear in `adjustments`; unqualifiable features under a
/// conversion policy appear in `rejections`; `objective_stages` is empty in
/// P28.
#[test]
fn metadata_records_effective_plan_applications_conversions_rejections() {
    let (mut model, x, _y, _obj) = mip_fixture();
    // Backend qualifies MipStart but NOT VariableHints.
    let (backend, _state) = PlanTestBackend::new_with_caps(caps_without_variable_hints());
    let mut session = SolverSession::new(backend);

    let start = MipStart::new(
        assignment_for(&model, BTreeMap::from([(x, 2.0)])),
        RepairPolicy::BackendDefault,
    );
    let mut hints = VariableHints::default();
    hints.insert(
        x,
        VariableHint {
            value: 1.0,
            priority: HintPriority(1),
        },
    );

    // Policy = ConvertStartToTemporaryFixing (non-default): the qualified
    // start applies natively (applied_features); the unqualifiable hints are
    // recorded as rejections (never silently dropped).
    let plan = SolvePlan {
        options: SolveOptions::default(),
        overlay: roml::SolveOverlay::new(BTreeMap::new(), vec![], vec![], vec![]).unwrap(),
        mip_starts: vec![start],
        hints,
        objective_override: None,
        lex_stage_policy: roml::LexStagePolicy::RequireOptimal,
        unsupported: UnsupportedFeaturePolicy::ConvertStartToTemporaryFixing,
    };
    let solution = session.solve_plan(&mut model, plan).unwrap();
    let ep = &solution.metadata().effective_plan;

    assert!(
        ep.applied_features.iter().any(|f| f.feature == "mip_start"),
        "the qualified start must be recorded as an applied feature"
    );
    assert!(
        ep.rejections.iter().any(|r| r.key == "hints"),
        "the unqualifiable hints must be recorded as rejections, not dropped"
    );
    assert!(
        ep.objective_stages.is_empty(),
        "objective_stages is an empty declaration in P28 (P31 populates it)"
    );
    // The exact CompilationId is present and equals the compiled base id.
    assert!(solution.metadata().compilation_id.is_some());
}

/// A conversion (start -> overlay temporary fixing) is recorded in the
/// effective plan's `adjustments` and the fixing is applied to the solve
/// (SM-08.5).
#[test]
fn convert_start_to_temporary_fixing_is_recorded_and_applied() {
    let (mut model, x, _y, _obj) = mip_fixture();
    // Backend does NOT qualify MipStart.
    let (backend, _state) = PlanTestBackend::new_with_caps(caps_without_starts_or_hints());
    let mut session = SolverSession::new(backend);

    let start = MipStart::new(
        assignment_for(&model, BTreeMap::from([(x, 2.0)])),
        RepairPolicy::BackendDefault,
    );
    let plan = SolvePlan {
        options: SolveOptions::default(),
        overlay: roml::SolveOverlay::new(BTreeMap::new(), vec![], vec![], vec![]).unwrap(),
        mip_starts: vec![start],
        hints: VariableHints::default(),
        objective_override: None,
        lex_stage_policy: roml::LexStagePolicy::RequireOptimal,
        unsupported: UnsupportedFeaturePolicy::ConvertStartToTemporaryFixing,
    };
    let solution = session.solve_plan(&mut model, plan).unwrap();
    let ep = &solution.metadata().effective_plan;

    assert!(
        ep.adjustments.iter().any(|a| a.key == "mip_start[0]"
            && a.requested == "mip_start"
            && a.applied == "overlay_temporary_fixing"),
        "the conversion must be recorded as a PlanAdjustment, got {:?}",
        ep.adjustments
    );
    // The start's value reached the solve as a temporary fixing: x is fixed to
    // 2.0 (the deterministic backend value for x is 1.0, clamped by the fixing).
    assert_eq!(solution.value(x), Some(2.0));
}

/// `ConvertHintToStart` converts hints into a `MipStart` when `MipStart` is
/// qualified, and records the conversion (SM-08.5).
#[test]
fn convert_hint_to_start_is_recorded_when_mip_start_qualified() {
    let (mut model, x, _y, _obj) = mip_fixture();
    // Backend qualifies MipStart but NOT VariableHints.
    let (backend, _state) = PlanTestBackend::new_with_caps(caps_without_variable_hints());
    let mut session = SolverSession::new(backend);

    let mut hints = VariableHints::default();
    hints.insert(
        x,
        VariableHint {
            value: 3.0,
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
        unsupported: UnsupportedFeaturePolicy::ConvertHintToStart,
    };
    let solution = session.solve_plan(&mut model, plan).unwrap();
    let ep = &solution.metadata().effective_plan;

    assert!(
        ep.adjustments.iter().any(|a| a.key == "hints"
            && a.requested == "variable_hints"
            && a.applied == "mip_start"),
        "the hint->start conversion must be recorded as a PlanAdjustment, got {:?}",
        ep.adjustments
    );
    // The converted start seeded the incumbent: x = 3.0.
    assert_eq!(solution.value(x), Some(3.0));
}

/// Default unsupported behavior rejects with a typed error BEFORE any backend
/// mutation (SM-08.4).
#[test]
fn unqualified_starts_reject_by_default() {
    let (mut model, x, y, _obj) = mip_fixture();
    let (backend, state) = PlanTestBackend::new_with_caps(caps_without_starts_or_hints());
    let mut session = SolverSession::new(backend);

    // A FULL start (covers every active integer/binary variable) requires the
    // `MipStart` capability, which this backend does not qualify.
    let start = MipStart::new(
        assignment_for(&model, BTreeMap::from([(x, 2.0), (y, 2.0)])),
        RepairPolicy::BackendDefault,
    );
    let plan = SolvePlan {
        options: SolveOptions::default(),
        overlay: roml::SolveOverlay::new(BTreeMap::new(), vec![], vec![], vec![]).unwrap(),
        mip_starts: vec![start],
        hints: VariableHints::default(),
        objective_override: None,
        lex_stage_policy: roml::LexStagePolicy::RequireOptimal,
        unsupported: UnsupportedFeaturePolicy::Reject,
    };
    let err = session
        .solve_plan(&mut model, plan)
        .expect_err("default reject");
    assert!(
        matches!(
            err,
            SolveError::Plan(PlanError::UnsupportedFeature {
                feature: "MipStart",
                policy: UnsupportedFeaturePolicy::Reject
            })
        ),
        "unexpected error: {err:?}"
    );
    // No backend solve was reached.
    assert_eq!(state.borrow().solves, 0);
}

/// Unqualified hints reject by default (the absent-hints blocking decision
/// mirrors the pinned HiGHS finding; SM-08.4).
#[test]
fn unqualified_hints_reject_by_default() {
    let (mut model, x, _y, _obj) = mip_fixture();
    let (backend, _state) = PlanTestBackend::new_with_caps(caps_without_variable_hints());
    let mut session = SolverSession::new(backend);

    let mut hints = VariableHints::default();
    hints.insert(
        x,
        VariableHint {
            value: 1.0,
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
    let err = session
        .solve_plan(&mut model, plan)
        .expect_err("absent hints reject by default");
    assert!(
        matches!(
            err,
            SolveError::Plan(PlanError::UnsupportedFeature {
                feature: "VariableHints",
                policy: UnsupportedFeaturePolicy::Reject
            })
        ),
        "unexpected error: {err:?}"
    );
}

// ── 3. Feasibility signature invariance (SM-08.3) ───────────────────────────

/// Starts/hints never alter the feasible-region signatures: a MIP solved with
/// a valid start and hints yields the same optimal objective and status, the
/// same canonical revision, and the same compiled base `CompilationId` as the
/// same model solved without them.
#[test]
fn starts_and_hints_leave_feasible_region_signature_unchanged() {
    let (mut model, x, y, _obj) = mip_fixture();
    let (backend, _state) = PlanTestBackend::new();
    let mut session = SolverSession::new(backend);

    let base = session.solve(&mut model).unwrap();
    let c_base = base.metadata().compilation_id.expect("base compilation");
    let rev = model.current_revision();
    let base_obj = base.objective_value();
    let base_status = base.status();

    // A start whose values equal the deterministic unit values (a valid
    // incumbent) and a hints set.
    let start = MipStart::new(
        assignment_for(&model, BTreeMap::from([(x, 1.0), (y, 1.0)])),
        RepairPolicy::BackendDefault,
    );
    let mut hints = VariableHints::default();
    hints.insert(
        x,
        VariableHint {
            value: 1.0,
            priority: HintPriority(1),
        },
    );
    let plan = SolvePlan {
        options: SolveOptions::default(),
        overlay: roml::SolveOverlay::new(BTreeMap::new(), vec![], vec![], vec![]).unwrap(),
        mip_starts: vec![start],
        hints,
        objective_override: None,
        lex_stage_policy: roml::LexStagePolicy::RequireOptimal,
        unsupported: UnsupportedFeaturePolicy::Reject,
    };
    let with_start = session.solve_plan(&mut model, plan).unwrap();

    assert_eq!(with_start.objective_value(), base_obj);
    assert_eq!(with_start.status(), base_status);
    assert_eq!(
        model.current_revision(),
        rev,
        "starts/hints must never advance the canonical model revision (SM-07.3)"
    );
    assert_eq!(
        with_start.metadata().compilation_id,
        Some(c_base),
        "starts/hints must not change the compiled base identity (SM-08.3)"
    );
}

// ── 4. No stale-start leakage ────────────────────────────────────────────────

/// A start from one solve never seeds an unrelated later solve: after solving
/// with a start, a subsequent solve of a changed model without a start is
/// deterministic and equal to a fresh no-start solve of the modified model.
#[test]
fn no_stale_start_leakage_into_subsequent_solve() {
    let (mut model, x, _y, _obj) = mip_fixture();
    let (backend, _state) = PlanTestBackend::new();
    let mut session = SolverSession::new(backend);

    let _ = session.solve(&mut model).unwrap();

    // Solve with a start whose values DIFFER from the deterministic unit
    // values, so a stale start would observably change the next solve.
    let start = MipStart::new(
        assignment_for(&model, BTreeMap::from([(x, 5.0)])),
        RepairPolicy::BackendDefault,
    );
    let plan = SolvePlan {
        options: SolveOptions::default(),
        overlay: roml::SolveOverlay::new(BTreeMap::new(), vec![], vec![], vec![]).unwrap(),
        mip_starts: vec![start],
        hints: VariableHints::default(),
        objective_override: None,
        lex_stage_policy: roml::LexStagePolicy::RequireOptimal,
        unsupported: UnsupportedFeaturePolicy::Reject,
    };
    let with_start = session.solve_plan(&mut model, plan).unwrap();
    assert_eq!(
        with_start.value(x),
        Some(5.0),
        "the start must seed the incumbent in its own solve"
    );

    // Modify the model, then solve WITHOUT a start.
    let z = model.add_variable(integer().bounds(0.0, 10.0)).unwrap();
    model.add_constraint((x + z).le(12.0)).unwrap();
    let clean = session.solve(&mut model).unwrap();

    // A fresh no-start solve of the modified model must equal `clean`.
    let (fresh_backend, _) = PlanTestBackend::new();
    let mut fresh_session = SolverSession::new(fresh_backend);
    let fresh = fresh_session.solve(&mut model).unwrap();

    assert_eq!(clean.objective_value(), fresh.objective_value());
    assert_eq!(clean.value(x), fresh.value(x));
    assert_eq!(
        clean.value(x),
        Some(1.0),
        "the old start value (5.0) must NOT leak into the unrelated solve"
    );
    let _ = z;
}
