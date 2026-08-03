//! P21 Task 3 — Generic `SolverSession<B>` orchestration tests.
//!
//! Uses a deterministic in-memory reference backend and a fault-injecting
//! variant to exercise every branch of the synchronization decision logic:
//!
//! 1. commit (fail before backend mutation)
//! 2. inspect backend health/revision
//! 3. terminal -> SolveError without retry
//! 4. requires rebuild or missing delta chain -> snapshot rebuild
//! 5. ready and behind -> sequential delta batches
//! 6. ready and current -> no synchronization
//! 7. recoverable/dirty sync failure -> one snapshot rebuild attempt
//! 8. solve exactly once after successful synchronization
//! 9. normalize result and attach metadata
//!
//! Also asserts: at most one rebuild retry; backend revision equals the
//! committed model revision before solve; and prior-solution invalidation
//! after mutation (API-01.5).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use roml::advanced::{
    BackendOp, BackendSnapshot, CompilationId, CompiledObjectiveId, CompiledObjectivePolicy,
    CompiledVariableId, EntityOrigin, OriginMap,
};
use roml::compiler::capability::BackendCapabilitySet;
use roml::delta::ModelOp;
use roml::id::{ObjId, VarId};
use roml::model::coefficient::CoefficientTarget;
use roml::model::Sense;
use roml::prelude::*;
use roml::revision::ModelRevision;
use roml::snapshot::ModelSnapshot;
use roml::solver::backend::{
    BackendCapabilities, BackendError, ErrorCategory, HealthEffect, TerminationStatus,
};
use roml::solver::request::{EffectiveConfig, SolveRequest, SolveResult, SolveSolution};
use roml::solver::session::{
    BackendMetadata, BackendSession, SessionHealth, SyncReceipt, Synchronization,
};
use roml::sync::AdapterHealth;
use roml::SolverSession;
use roml::SynchronizationMode;

// ── Reference + fault-injecting backend ──────────────────────────────────────

/// Shared fault-injection state for [`TestBackend`].
///
/// Knobs and observability counters live here (shared with the test via
/// `Rc<RefCell<..>>`) so tests can flip knobs and read counters without
/// reaching into the session's backend — `SolverSession` exposes only the
/// approved `new`/`solve`/`solve_with` interface.
struct FaultState {
    revision: ModelRevision,
    health: AdapterHealth,
    reject_next_delta: bool,
    fail_delta_terminal: bool,
    fail_rebuild: bool,
    solve_fails: bool,
    solve_termination: TerminationStatus,
    rebuilds: usize,
    deltas: usize,
    solves: usize,
    solve_revision_seen: Option<ModelRevision>,
    /// The exact `CompilationId` the backend holds after the most recent
    /// compiled synchronization (F2 / SM-03.9).
    current_compilation: Option<CompilationId>,
    /// When set, `solve` reports this `CompilationId` instead of the backend's
    /// real held compilation — simulates a backend returning a result tagged
    /// with the wrong compiled state (F2 mismatch rejection).
    report_wrong_compilation: Option<CompilationId>,
}

impl FaultState {
    fn force_rebuild(&mut self) {
        self.health = AdapterHealth::RequiresRebuild;
    }

    fn set_terminal(&mut self) {
        self.health = AdapterHealth::Terminal;
    }

    fn set_reject_next_delta(&mut self) {
        self.reject_next_delta = true;
    }

    fn set_fail_delta_terminal(&mut self) {
        self.fail_delta_terminal = true;
    }

    fn set_fail_rebuild(&mut self) {
        self.fail_rebuild = true;
    }

    fn set_revision_for_test(&mut self, revision: ModelRevision) {
        self.revision = revision;
    }

    fn rebuilds(&self) -> usize {
        self.rebuilds
    }
    fn deltas(&self) -> usize {
        self.deltas
    }
    fn solves(&self) -> usize {
        self.solves
    }
    fn solve_revision_seen(&self) -> Option<ModelRevision> {
        self.solve_revision_seen
    }
}

/// A deterministic in-memory backend that mirrors a model's projection and
/// "solves" by reporting an objective computed from the active objective's
/// cells at fixed variable values (1.0), plus the objective constant.
///
/// Returns `(backend, state)` — the test keeps the shared [`FaultState`]
/// handle to inject the recoverable/terminal/dirty behaviors the
/// orchestration must react to and to observe counters.
struct TestBackend {
    name: String,
    var_values: HashMap<VarId, f64>,
    objectives: HashMap<ObjId, (Sense, f64)>,
    active_objective: Option<ObjId>,
    objective_cells: HashMap<ObjId, HashMap<VarId, f64>>,
    state: Rc<RefCell<FaultState>>,
    /// Compiled id -> user variable, maintained by the compiled sync path.
    compiled_to_user_variable: HashMap<CompiledVariableId, VarId>,
    /// Compiled id -> user objective, maintained by the compiled sync path.
    compiled_to_user_objective: HashMap<CompiledObjectiveId, ObjId>,
    /// The authoritative typed capability set the façade gates on (F3).
    /// Defaults to the full M2 native surface; a test may override it to prove
    /// the typed view is the façade's source of truth even when the flat
    /// `capabilities()` view lies.
    typed_caps: BackendCapabilitySet,
}

/// The full M2-native typed capability surface (F3 default for test backends).
fn full_typed_capabilities() -> BackendCapabilitySet {
    use roml::compiler::capability::{BackendFeature, FeatureSupport, SupportLevel};
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

impl TestBackend {
    fn new() -> (Self, Rc<RefCell<FaultState>>) {
        let state = Rc::new(RefCell::new(FaultState {
            revision: ModelRevision::ZERO,
            health: AdapterHealth::Ready,
            reject_next_delta: false,
            fail_delta_terminal: false,
            fail_rebuild: false,
            solve_fails: false,
            solve_termination: TerminationStatus::Optimal,
            rebuilds: 0,
            deltas: 0,
            solves: 0,
            solve_revision_seen: None,
            current_compilation: None,
            report_wrong_compilation: None,
        }));
        (
            Self {
                name: "TestBackend".to_string(),
                var_values: HashMap::new(),
                objectives: HashMap::new(),
                active_objective: None,
                objective_cells: HashMap::new(),
                state: state.clone(),
                compiled_to_user_variable: HashMap::new(),
                compiled_to_user_objective: HashMap::new(),
                typed_caps: full_typed_capabilities(),
            },
            state,
        )
    }

    fn compute_objective_value(&self) -> Option<f64> {
        let obj = self.active_objective?;
        let constant = self.objectives.get(&obj).map(|(_, c)| *c).unwrap_or(0.0);
        let cells = self.objective_cells.get(&obj).cloned().unwrap_or_default();
        let sum: f64 = cells
            .iter()
            .map(|(var, cost)| *cost * self.var_values.get(var).copied().unwrap_or(0.0))
            .sum();
        Some(sum + constant)
    }

    fn project_snapshot(&mut self, snapshot: &ModelSnapshot) {
        self.var_values = snapshot.variables.iter().map(|v| (v.id, 1.0)).collect();
        self.objectives = snapshot
            .objectives
            .iter()
            .map(|o| (o.id, (o.sense, o.constant)))
            .collect();
        self.active_objective = snapshot.objectives.iter().find(|o| o.active).map(|o| o.id);
        self.objective_cells.clear();
        for cell in &snapshot.cells {
            if let CoefficientTarget::Objective(obj) = cell.cell_key.0 {
                self.objective_cells
                    .entry(obj)
                    .or_default()
                    .insert(cell.cell_key.1, cell.evaluated_value);
            }
        }
    }

    /// Project a compiled backend snapshot into user-keyed state using the
    /// snapshot's mandatory origin map (SM-02.5).
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
        match &snapshot.objective_policy {
            CompiledObjectivePolicy::Single(cid) => {
                self.active_objective = self.compiled_to_user_objective.get(cid).copied();
            }
            _ => self.active_objective = None,
        }
    }

    /// Apply one compiled backend op, translating compiled ids to user ids via
    /// the maintained compiled->user maps and the batch's origin additions.
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
                    CompiledObjectivePolicy::Single(cid) => {
                        self.compiled_to_user_objective.get(cid).copied()
                    }
                    _ => None,
                };
            }
            // Constraint/row and variable-bound ops do not affect the
            // objective-value model this test backend tracks.
            BackendOp::SetVariableBounds { .. }
            | BackendOp::AddLinearRow(_)
            | BackendOp::RemoveLinearRow(_)
            | BackendOp::SetLinearRowBounds { .. }
            | BackendOp::SetLinearCoefficient { .. }
            | BackendOp::RemoveLinearCoefficient { .. } => {}
            // `BackendOp` is #[non_exhaustive]: future ops are ignored by this
            // objective-value test backend.
            _ => {}
        }
    }

    fn apply_op(&mut self, op: &ModelOp) {
        match op {
            ModelOp::AddVariable { var, .. } => {
                self.var_values.insert(*var, 1.0);
            }
            ModelOp::RemoveVariable { var } => {
                self.var_values.remove(var);
                for cells in self.objective_cells.values_mut() {
                    cells.remove(var);
                }
            }
            ModelOp::AddObjective { obj, sense } => {
                self.objectives.insert(*obj, (*sense, 0.0));
                self.objective_cells.entry(*obj).or_default();
            }
            ModelOp::RemoveObjective { obj } => {
                self.objectives.remove(obj);
                self.objective_cells.remove(obj);
                if self.active_objective == Some(*obj) {
                    self.active_objective = None;
                }
            }
            ModelOp::SetActiveObjective { obj } => {
                self.active_objective = *obj;
            }
            ModelOp::SetObjectiveSense { obj, sense } => {
                if let Some(entry) = self.objectives.get_mut(obj) {
                    entry.0 = *sense;
                }
            }
            ModelOp::SetObjectiveConstant { obj, constant } => {
                if let Some(entry) = self.objectives.get_mut(obj) {
                    entry.1 = *constant;
                }
            }
            ModelOp::SetCell {
                cell_key,
                evaluated_value,
                ..
            } => {
                if let CoefficientTarget::Objective(obj) = cell_key.0 {
                    self.objective_cells
                        .entry(obj)
                        .or_default()
                        .insert(cell_key.1, *evaluated_value);
                }
            }
            ModelOp::SetObjectiveCell {
                cell_key,
                evaluated_value,
                constant,
                ..
            } => {
                if let CoefficientTarget::Objective(obj) = cell_key.0 {
                    self.objective_cells
                        .entry(obj)
                        .or_default()
                        .insert(cell_key.1, *evaluated_value);
                    if let Some(entry) = self.objectives.get_mut(&obj) {
                        entry.1 = *constant;
                    }
                }
            }
            _ => {}
        }
    }
}

impl BackendMetadata for TestBackend {
    fn name(&self) -> &str {
        &self.name
    }
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::all()
    }
    fn typed_capabilities(&self) -> &BackendCapabilitySet {
        &self.typed_caps
    }
}

impl SessionHealth for TestBackend {
    fn health(&self) -> AdapterHealth {
        self.state.borrow().health
    }
    fn revision(&self) -> ModelRevision {
        self.state.borrow().revision
    }
}

impl BackendSession for TestBackend {
    fn synchronize(&mut self, sync: Synchronization) -> Result<SyncReceipt, BackendError> {
        // Scoped state borrows: never hold a RefMut across a &mut self call.
        match sync {
            Synchronization::Rebuild(snapshot) => {
                {
                    let mut s = self.state.borrow_mut();
                    s.rebuilds += 1;
                    if s.fail_rebuild {
                        return Err(BackendError::new(
                            "injected rebuild failure",
                            ErrorCategory::Internal,
                            HealthEffect::Recoverable,
                        ));
                    }
                }
                self.project_snapshot(&snapshot);
                let mut s = self.state.borrow_mut();
                s.revision = snapshot.revision;
                s.health = AdapterHealth::Ready;
                Ok(SyncReceipt {
                    cursor: roml::sync::AdapterCursor {
                        applied_revision: s.revision,
                        health: s.health,
                    },
                    health: s.health,
                })
            }
            Synchronization::DeltaBatch(batch) => {
                {
                    let mut s = self.state.borrow_mut();
                    if s.reject_next_delta {
                        s.reject_next_delta = false;
                        s.health = AdapterHealth::RequiresRebuild;
                        return Err(BackendError::new(
                            "injected recoverable delta rejection",
                            ErrorCategory::InvalidInput,
                            HealthEffect::Recoverable,
                        ));
                    }
                    if s.fail_delta_terminal {
                        s.fail_delta_terminal = false;
                        s.health = AdapterHealth::Terminal;
                        return Err(BackendError::new(
                            "injected terminal delta failure",
                            ErrorCategory::Internal,
                            HealthEffect::Terminal,
                        ));
                    }
                    if batch.from != s.revision {
                        s.health = AdapterHealth::RequiresRebuild;
                        return Err(BackendError::new(
                            format!("delta from {} != backend at {}", batch.from, s.revision),
                            ErrorCategory::InvalidInput,
                            HealthEffect::Recoverable,
                        ));
                    }
                }
                for op in &batch.operations {
                    self.apply_op(op);
                }
                let mut s = self.state.borrow_mut();
                s.deltas += 1;
                s.revision = batch.to;
                s.health = AdapterHealth::Ready;
                Ok(SyncReceipt {
                    cursor: roml::sync::AdapterCursor {
                        applied_revision: s.revision,
                        health: s.health,
                    },
                    health: s.health,
                })
            }
            Synchronization::CompiledRebuild(snapshot) => {
                {
                    let mut s = self.state.borrow_mut();
                    s.rebuilds += 1;
                    if s.fail_rebuild {
                        return Err(BackendError::new(
                            "injected rebuild failure",
                            ErrorCategory::Internal,
                            HealthEffect::Recoverable,
                        ));
                    }
                }
                self.project_compiled_snapshot(&snapshot);
                let mut s = self.state.borrow_mut();
                s.revision = snapshot.source_revision;
                s.health = AdapterHealth::Ready;
                s.current_compilation = Some(snapshot.compilation_id);
                Ok(SyncReceipt {
                    cursor: roml::sync::AdapterCursor {
                        applied_revision: s.revision,
                        health: s.health,
                    },
                    health: s.health,
                })
            }
            Synchronization::CompiledDeltaBatch(batch) => {
                {
                    let mut s = self.state.borrow_mut();
                    if s.reject_next_delta {
                        s.reject_next_delta = false;
                        s.health = AdapterHealth::RequiresRebuild;
                        return Err(BackendError::new(
                            "injected recoverable delta rejection",
                            ErrorCategory::InvalidInput,
                            HealthEffect::Recoverable,
                        ));
                    }
                    if s.fail_delta_terminal {
                        s.fail_delta_terminal = false;
                        s.health = AdapterHealth::Terminal;
                        return Err(BackendError::new(
                            "injected terminal delta failure",
                            ErrorCategory::Internal,
                            HealthEffect::Terminal,
                        ));
                    }
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
                let origins = batch.origin_additions.clone();
                for op in &batch.operations {
                    self.apply_compiled_op(op, &origins);
                }
                let mut s = self.state.borrow_mut();
                s.deltas += 1;
                s.revision = batch.to_revision;
                s.health = AdapterHealth::Ready;
                s.current_compilation = Some(batch.to_compilation);
                Ok(SyncReceipt {
                    cursor: roml::sync::AdapterCursor {
                        applied_revision: s.revision,
                        health: s.health,
                    },
                    health: s.health,
                })
            }
        }
    }

    fn solve(&mut self, _request: &SolveRequest) -> Result<SolveResult, BackendError> {
        let mut s = self.state.borrow_mut();
        s.solves += 1;
        s.solve_revision_seen = Some(s.revision);
        if s.solve_fails {
            return Err(BackendError::new(
                "injected solve failure",
                ErrorCategory::Internal,
                HealthEffect::Recoverable,
            ));
        }
        let termination = s.solve_termination;
        // F2 (SM-03.9): the result carries the exact `CompilationId` of the
        // compiled state the backend solved. The fault knob lets a test tag a
        // result with a WRONG id so the façade's mismatch rejection is
        // exercised.
        let compilation_id = match s.report_wrong_compilation {
            Some(forged) => forged,
            None => s
                .current_compilation
                .expect("a solve must follow a compiled synchronization"),
        };
        drop(s); // compute_objective_value takes &self
        let solution = match termination {
            TerminationStatus::Optimal | TerminationStatus::Feasible => {
                let mut values: Vec<(VarId, f64)> =
                    self.var_values.iter().map(|(var, v)| (*var, *v)).collect();
                values.sort_by_key(|(var, _)| *var);
                Some(SolveSolution {
                    variable_values: values,
                    objective_value: self.compute_objective_value(),
                    dual_values: None,
                    reduced_costs: None,
                })
            }
            _ => None,
        };
        Ok(SolveResult {
            effective_configuration: EffectiveConfig::default(),
            termination,
            solution,
            compilation_id,
        })
    }

    fn close(self) -> Result<(), BackendError> {
        Ok(())
    }
}

// ── Builders ──────────────────────────────────────────────────────────────────

/// `maximize 3x + y + 5` subject to `x + y <= 4` with two continuous vars.
fn build_constant_model() -> (Model, VarId, VarId) {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let y = model.add_variable(continuous()).unwrap();
    model.add_constraint((x + y).le(4.0)).unwrap();
    model.maximize(3.0 * x + y + 5.0).unwrap();
    (model, x, y)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// First solve from a new model: commit -> delta sync -> solve. The backend
/// ends synchronized at the committed revision and the solution metadata
/// records the Delta mode.
#[test]
fn first_solve_synchronizes_via_delta_and_reports_revision() {
    let (mut model, _x, _y) = build_constant_model();
    let (backend, state) = TestBackend::new();
    let mut session = SolverSession::new(backend);

    let solution = session.solve(&mut model).expect("first solve succeeds");
    assert_eq!(solution.status(), SolveStatus::Optimal);
    assert_eq!(
        solution.metadata().synchronization,
        SynchronizationMode::Delta
    );
    assert_eq!(solution.metadata().model_revision, model.current_revision());
    // WR-2 explicit contract: the first solve establishes the compiled EMPTY
    // base in the backend (exactly one base-establishment rebuild, NOT a
    // rebuild retry) and then flows the r0→r1 deltas — the backend always
    // holds a compiled base before receiving a CompiledDeltaBatch (D28).
    assert_eq!(
        state.borrow().rebuilds(),
        1,
        "first solve establishes the compiled empty base in the backend"
    );
    assert_eq!(state.borrow().deltas(), 1, "first solve applies the delta");
    // Backend revision equals the committed model revision before solve.
    let backend_rev_at_solve = state
        .borrow()
        .solve_revision_seen()
        .expect("solve must record its revision");
    assert_eq!(backend_rev_at_solve, model.current_revision());
}

/// A second solve without mutation performs no synchronization and re-solves.
#[test]
fn no_change_second_solve_uses_no_sync() {
    let (mut model, _x, _y) = build_constant_model();
    let (backend, state) = TestBackend::new();
    let mut session = SolverSession::new(backend);

    let first = session.solve(&mut model).unwrap();
    assert_eq!(first.metadata().synchronization, SynchronizationMode::Delta);
    let second = session.solve(&mut model).unwrap();
    assert_eq!(
        second.metadata().synchronization,
        SynchronizationMode::NoChange
    );
    assert_eq!(state.borrow().deltas(), 1);
    assert_eq!(state.borrow().solves(), 2);
    assert_eq!(second.metadata().model_revision, model.current_revision());
}

/// F1: a `SolverSession` is bound to ONE model instance (the instance whose
/// state is currently compiled in the backend). Solving a DIFFERENT model at
/// the SAME revision must NOT take the no-sync path against the previous
/// model's backend state — it forces a full snapshot rebuild and the solution
/// comes from the NEW model's state, with the new model's lineage/instance
/// recorded in the metadata.
#[test]
fn cross_model_reuse_at_same_revision_forces_rebuild_and_solves_new_model() {
    // Model A: maximize 3x + 5 with x <= 10 (continuous).
    let mut model_a = Model::new();
    let x = model_a.add_variable(continuous()).unwrap();
    model_a.add_constraint((x).le(10.0)).unwrap();
    model_a.maximize(3.0 * x + 5.0).unwrap();

    // Model B: an unrelated instance at the SAME revision number, but with a
    // different objective (maximize 10y + 5) so the optimal objective value
    // distinguishes the two models' states.
    let mut model_b = Model::new();
    let y = model_b.add_variable(continuous()).unwrap();
    model_b.add_constraint((y).le(10.0)).unwrap();
    model_b.maximize(10.0 * y + 5.0).unwrap();

    assert_ne!(
        model_a.instance(),
        model_b.instance(),
        "independent models must have distinct instance ids"
    );
    model_a.commit().unwrap();
    model_b.commit().unwrap();
    assert_eq!(
        model_a.current_revision(),
        model_b.current_revision(),
        "the two models are at the SAME revision — only the instance differs"
    );

    let (backend, state) = TestBackend::new();
    let mut session = SolverSession::new(backend);

    let sol_a = session.solve(&mut model_a).expect("model A solves");
    // Backend evaluates at fixed x = 1.0 => 3*1 + 5 = 8.
    assert!((sol_a.objective_value().unwrap() - 8.0).abs() < 1e-9);
    assert_eq!(sol_a.metadata().model_instance, model_a.instance());
    let rebuilds_after_a = state.borrow().rebuilds();

    let sol_b = session.solve(&mut model_b).expect("model B solves");
    // B's state has cost 10 on its own variable => 10*1 + 5 = 15. If the
    // session had taken the no-sync path, the backend would still hold A's
    // state and report 8.
    assert!(
        (sol_b.objective_value().unwrap() - 15.0).abs() < 1e-9,
        "B's solution must come from B's state, got {}",
        sol_b.objective_value().unwrap()
    );
    assert_eq!(
        sol_b.metadata().model_instance,
        model_b.instance(),
        "B's instance must be recorded in the metadata"
    );
    assert_eq!(
        sol_b.metadata().model_lineage,
        model_b.lineage(),
        "B's lineage must be recorded in the metadata"
    );
    assert_eq!(
        sol_b.metadata().synchronization,
        SynchronizationMode::Rebuild,
        "cross-model reuse must force a full snapshot rebuild, not the no-sync path"
    );
    assert_eq!(
        state.borrow().rebuilds(),
        rebuilds_after_a + 1,
        "exactly one rebuild to re-establish the compiled base for model B"
    );
}

/// A parameter change between solves is applied as a delta; the objective
/// value reflects the new parameter and the constant is included exactly once.
#[test]
fn parameter_delta_second_solve_is_fresh_and_single_counted() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let price = model.add_parameter(parameter(1.0).named("price")).unwrap();
    model.add_constraint(x.le(10.0)).unwrap();
    model.maximize(price * x + 5.0).unwrap();

    let mut session = SolverSession::new(TestBackend::new().0);
    let first = session.solve(&mut model).unwrap();
    // Backend x = 1.0, cost = price = 1.0, constant = 5.0 => 1*1 + 5 = 6.
    assert!((first.objective_value().unwrap() - 6.0).abs() < 1e-9);
    assert_eq!(first.metadata().synchronization, SynchronizationMode::Delta);

    model.set_parameter(price, 5.0).unwrap();
    let second = session.solve(&mut model).unwrap();
    assert_eq!(
        second.metadata().synchronization,
        SynchronizationMode::Delta
    );
    // cost = 5, x = 1, constant = 5 => 5 + 5 = 10.
    assert!((second.objective_value().unwrap() - 10.0).abs() < 1e-9);
    assert_eq!(second.metadata().model_revision, model.current_revision());
}

/// A bound change between solves is applied as a delta.
#[test]
fn bound_delta_second_solve_synchronizes_incrementally() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    model.add_constraint(x.le(10.0)).unwrap();
    model.maximize(x).unwrap();

    let mut session = SolverSession::new(TestBackend::new().0);
    session.solve(&mut model).unwrap();
    model.set_variable_bounds(x, Bounds::new(0.0, 2.0)).unwrap();
    let second = session.solve(&mut model).unwrap();
    assert_eq!(
        second.metadata().synchronization,
        SynchronizationMode::Delta
    );
    assert_eq!(second.metadata().model_revision, model.current_revision());
}

/// A backend reporting `RequiresRebuild` is recovered via one snapshot rebuild.
#[test]
fn requires_rebuild_health_recovers_via_snapshot_rebuild() {
    let (mut model, _x, _y) = build_constant_model();
    let (backend, state) = TestBackend::new();
    state.borrow_mut().force_rebuild();
    let mut session = SolverSession::new(backend);

    let solution = session.solve(&mut model).expect("rebuild recoverable");
    assert_eq!(
        solution.metadata().synchronization,
        SynchronizationMode::Rebuild
    );
    assert_eq!(state.borrow().rebuilds(), 1);
    assert_eq!(
        state.borrow().solve_revision_seen(),
        Some(model.current_revision())
    );
}

/// A terminal backend returns `Err(SolveError)` without any retry or solve.
#[test]
fn terminal_health_returns_error_without_retry_or_solve() {
    let (mut model, _x, _y) = build_constant_model();
    let (backend, state) = TestBackend::new();
    state.borrow_mut().set_terminal();
    let mut session = SolverSession::new(backend);

    let err = session.solve(&mut model).expect_err("terminal must error");
    assert!(matches!(err, SolveError::Synchronization(_)));
    assert!(err.is_terminal(), "error must be terminal");
    assert_eq!(state.borrow().rebuilds(), 0);
    assert_eq!(state.borrow().solves(), 0);
    assert_eq!(state.borrow().deltas(), 0);
}

/// A recoverable delta failure triggers exactly one snapshot rebuild, then a
/// successful solve — the one-rebuild retry bound (API-02.3).
#[test]
fn recoverable_delta_failure_triggers_one_rebuild_then_solves() {
    let (mut model, _x, _y) = build_constant_model();
    let (backend, state) = TestBackend::new();
    state.borrow_mut().set_reject_next_delta();
    let mut session = SolverSession::new(backend);

    let solution = session.solve(&mut model).expect("recovered via rebuild");
    assert_eq!(
        solution.metadata().synchronization,
        SynchronizationMode::Rebuild
    );
    // One base-establishment rebuild (the compiled empty base is now sent to
    // the backend — WR-2) plus exactly ONE error-recovery rebuild (the
    // API-02.3 retry bound is unchanged).
    assert_eq!(state.borrow().rebuilds(), 2);
    assert_eq!(state.borrow().deltas(), 0);
    assert_eq!(solution.metadata().model_revision, model.current_revision());
}

/// If the rebuild attempt also fails, the solve returns an error after exactly
/// one rebuild attempt — no retry loop (API-02.3).
#[test]
fn at_most_one_rebuild_retry_when_rebuild_also_fails() {
    let (mut model, _x, _y) = build_constant_model();
    let (backend, state) = TestBackend::new();
    state.borrow_mut().set_reject_next_delta();
    state.borrow_mut().set_fail_rebuild();
    let mut session = SolverSession::new(backend);

    let err = session.solve(&mut model).expect_err("must error");
    assert!(matches!(err, SolveError::Synchronization(_)));
    // One base-establishment rebuild (WR-2) plus exactly ONE failed retry
    // rebuild — the API-02.3 retry bound (at most one retry) is unchanged;
    // there is no retry loop.
    assert_eq!(state.borrow().rebuilds(), 2);
    assert_eq!(state.borrow().solves(), 0);
}

/// A delta failure with a TERMINAL health effect returns immediately with no
/// rebuild retry (plan step 3/7: only recoverable/dirty failures retry).
#[test]
fn terminal_delta_failure_returns_error_without_rebuild_retry() {
    let (mut model, _x, _y) = build_constant_model();
    let (backend, state) = TestBackend::new();
    state.borrow_mut().set_fail_delta_terminal();
    let mut session = SolverSession::new(backend);

    let err = session.solve(&mut model).expect_err("terminal must error");
    assert!(err.is_terminal(), "error must be terminal: {err:?}");
    // The base-establishment rebuild (WR-2) is not a retry: a TERMINAL delta
    // failure still returns immediately with NO error-recovery rebuild.
    assert_eq!(
        state.borrow().rebuilds(),
        1,
        "only the base-establishment rebuild"
    );
    assert_eq!(state.borrow().solves(), 0, "no solve");
}

/// A backend ahead of the model is re-synchronized via a snapshot rebuild.
#[test]
fn backend_ahead_of_model_triggers_rebuild() {
    let (mut model, _x, _y) = build_constant_model();
    let (backend, state) = TestBackend::new();
    let mut session = SolverSession::new(backend);
    session.solve(&mut model).unwrap();

    // Simulate the backend cursor drifting ahead of the model.
    state
        .borrow_mut()
        .set_revision_for_test(ModelRevision::from_u64(5));

    let second = session.solve(&mut model).expect("rebuild recovers");
    assert_eq!(
        second.metadata().synchronization,
        SynchronizationMode::Rebuild
    );
    // One base-establishment rebuild on the first solve (WR-2) plus the
    // backend-ahead re-synchronization rebuild on the second solve.
    assert_eq!(state.borrow().rebuilds(), 2);
    assert_eq!(state.borrow().revision, model.current_revision());
}

/// After the model is mutated, the next solve returns a fresh solution from
/// the new revision — the session never reports the pre-mutation (stale)
/// result (API-01.5). Stale protection is structural: the only way to obtain
/// a solution is `solve`, which always re-synchronizes first.
#[test]
fn prior_solution_never_reported_after_mutation() {
    let (mut model, _x, _y) = build_constant_model();
    let mut session = SolverSession::new(TestBackend::new().0);

    let first = session.solve(&mut model).unwrap();
    let r1 = first.metadata().model_revision;

    // Mutate the model: tighten the constraint so a fresh solve differs.
    model
        .set_variable_bounds(_x, Bounds::new(0.0, 1.0))
        .unwrap();
    let second = session.solve(&mut model).unwrap();
    let r2 = second.metadata().model_revision;
    assert_ne!(r2, r1, "the model must have advanced");
    assert_eq!(
        second.metadata().model_revision,
        model.current_revision(),
        "returned solution is from the new revision, never the stale one"
    );
}

/// A failed solve after mutation returns an error — the error path never
/// surfaces the previously computed solution as current (API-01.5).
#[test]
fn failed_solve_never_surfaces_prior_solution() {
    let (mut model, _x, _y) = build_constant_model();
    let (backend, state) = TestBackend::new();
    let mut session = SolverSession::new(backend);
    session.solve(&mut model).unwrap();

    // Mutate, then make both the delta and the rebuild attempt fail.
    model
        .set_variable_bounds(_x, Bounds::new(0.0, 1.0))
        .unwrap();
    state.borrow_mut().set_reject_next_delta();
    state.borrow_mut().set_fail_rebuild();
    let err = session.solve(&mut model).expect_err("solve must fail");
    assert!(matches!(err, SolveError::Synchronization(_)));
    // No solution is returned: the stale result cannot be reported as current.
}

/// The objective constant reaches the backend on the delta path exactly once:
/// the façade value equals the backend's solve value and the model expression
/// evaluation at the solution values (API-03.5).
#[test]
fn objective_constant_is_reported_exactly_once_end_to_end() {
    let (mut model, x, y) = build_constant_model();
    let mut session = SolverSession::new(TestBackend::new().0);
    let solution = session.solve(&mut model).unwrap();

    let facade_value = solution.objective_value().unwrap();
    // Backend x = 1, y = 1: 3*1 + 1*1 + 5 = 9.
    assert!((facade_value - 9.0).abs() < 1e-9);

    // Direct backend value (recorded by the reference backend solve).
    let expression_value = model
        .objective_expression(model.active_objective().unwrap())
        .unwrap()
        .evaluate(
            |var| solution.value_or_zero(var),
            |param| model.parameter_value(param).unwrap_or(0.0),
        );
    assert!(
        (expression_value - facade_value).abs() < 1e-9,
        "expression {expression_value} != façade {facade_value}"
    );
    assert_eq!(solution.value(x), Some(1.0));
    assert_eq!(solution.value(y), Some(1.0));
}

// ── F2: CompilationId through SolveResult / Solution metadata (SM-03.9) ───────

/// Compile an unrelated snapshot to obtain a `CompilationId` guaranteed to
/// differ from any backend-held compiled state (F2 mismatch test).
fn forged_compilation_id() -> CompilationId {
    use roml::advanced::{CompilationPolicy, CompilationSession};
    use roml::compiler::capability::{
        BackendCapabilitySet, BackendFeature, FeatureSupport, SupportLevel,
    };
    let mut caps = BackendCapabilitySet::new();
    for feature in [
        BackendFeature::Lp,
        BackendFeature::Mip,
        BackendFeature::IncrementalBounds,
        BackendFeature::IncrementalRows,
        BackendFeature::IncrementalCoefficients,
    ] {
        caps.set(
            feature,
            FeatureSupport {
                level: SupportLevel::Native,
                limitations: Default::default(),
            },
        );
    }
    let mut cs = CompilationSession::new();
    let empty_model = Model::new();
    let snapshot = empty_model.take_snapshot().unwrap();
    let compiled = cs
        .compile_snapshot(
            empty_model.instance(),
            &snapshot,
            &CompilationPolicy::Auto,
            &caps,
        )
        .expect("an empty snapshot must compile");
    compiled.compilation_id
}

/// F2 (SM-03.9): a normal solve's `Solution.metadata().compilation_id` equals
/// the exact `CompilationId` of the compiled state the backend solved, and is
/// stable across no-change solves while a mutation allocates a fresh compiled
/// state id.
#[test]
fn solution_metadata_carries_compilation_id_and_is_stable_across_solves() {
    let (mut model, x, _y) = build_constant_model();
    let (backend, state) = TestBackend::new();
    let mut session = SolverSession::new(backend);

    let first = session.solve(&mut model).unwrap();
    let held = state
        .borrow()
        .current_compilation
        .expect("the backend must hold a compiled state after sync");
    assert_eq!(
        first.metadata().compilation_id,
        held,
        "the solution's compilation id must equal the compiled state's id"
    );
    assert!(first.value(x).is_some());

    // A no-change second solve re-solves the SAME compiled state: the id is
    // stable across solves.
    let second = session.solve(&mut model).unwrap();
    assert_eq!(
        second.metadata().compilation_id,
        first.metadata().compilation_id,
        "no-change solves reuse the same compiled state id"
    );

    // A mutation compiles a NEW delta -> a NEW compiled state id.
    model.set_variable_bounds(x, Bounds::new(0.0, 1.0)).unwrap();
    let third = session.solve(&mut model).unwrap();
    assert_ne!(
        third.metadata().compilation_id,
        first.metadata().compilation_id,
        "a mutation must allocate a fresh compiled state id"
    );
    assert_eq!(
        third.metadata().compilation_id,
        state.borrow().current_compilation.unwrap()
    );
}

/// F2 (SM-03.9): the façade rejects a result tagged with a `CompilationId`
/// that does not match the compiled state it just synchronized to — a typed
/// error, never a silently accepted result from a different compiled state.
#[test]
fn facade_rejects_result_tagged_with_wrong_compilation_id() {
    let (mut model, _x, _y) = build_constant_model();
    let (backend, state) = TestBackend::new();
    let mut session = SolverSession::new(backend);
    session.solve(&mut model).unwrap();

    let forged = forged_compilation_id();
    assert_ne!(
        forged,
        state.borrow().current_compilation.unwrap(),
        "the forged id must differ from the backend's held compiled state"
    );

    state.borrow_mut().report_wrong_compilation = Some(forged);
    let err = session
        .solve(&mut model)
        .expect_err("a mismatched compilation id must be rejected");
    assert!(
        matches!(err, SolveError::CompilationMismatch { .. }),
        "expected CompilationMismatch, got {err:?}"
    );
}

// ── F3: typed capabilities authoritative (SM-04.1, SM-04.4) ──────────────────

/// F3(a): the façade's compilation gating uses the backend's TYPED capability
/// set — never the flat `capabilities()` compat view. A backend whose flat
/// view lies (`all()`) but whose typed view lacks `Lp` cannot compile even a
/// continuous model: the solve fails with a typed synchronization error.
#[test]
fn facade_gates_on_typed_capabilities_not_flat_view() {
    use roml::compiler::capability::{BackendFeature, FeatureSupport, SupportLevel};

    let (mut backend, _state) = TestBackend::new();
    // Flat `capabilities()` says `all()` (lies), typed says NO Lp.
    let mut typed = full_typed_capabilities();
    typed.set(
        BackendFeature::Lp,
        FeatureSupport {
            level: SupportLevel::Unsupported,
            limitations: Default::default(),
        },
    );
    backend.typed_caps = typed;

    let mut session = SolverSession::new(backend);
    let (mut model, _x, _y) = build_constant_model();
    let err = session
        .solve(&mut model)
        .expect_err("a backend whose typed view lacks Lp cannot compile any snapshot");
    assert!(
        matches!(err, SolveError::Synchronization(_)),
        "the compile failure must surface as a synchronization error, got {err:?}"
    );
}

/// F3(a): the production façade wires request validation through the typed
/// `validate_request` — a MIP option against a typed view without `Mip` is
/// rejected before any synchronization (SM-04.4).
#[test]
fn facade_uses_typed_validate_request_for_options() {
    use roml::compiler::capability::{BackendFeature, FeatureSupport, SupportLevel};

    let (mut backend, _state) = TestBackend::new();
    // Flat `capabilities()` says `all()`, but the typed view lacks Mip.
    let mut typed = full_typed_capabilities();
    typed.set(
        BackendFeature::Mip,
        FeatureSupport {
            level: SupportLevel::Unsupported,
            limitations: Default::default(),
        },
    );
    backend.typed_caps = typed;

    let mut session = SolverSession::new(backend);
    let (mut model, _x, _y) = build_constant_model();

    // A continuous model with default options solves fine (Lp + incremental
    // features are native in the typed view).
    session
        .solve(&mut model)
        .expect("continuous solve succeeds");

    // The same model with a MIP gap option is rejected by the typed
    // validate_request — never silently passed to the backend.
    let err = session
        .solve_with(&mut model, SolveOptions::new().relative_gap(0.01))
        .expect_err("a MIP option against a no-Mip typed view must be rejected");
    assert!(
        matches!(err, SolveError::InvalidOptions(_)),
        "expected InvalidOptions from the typed validate_request, got {err:?}"
    );
}
