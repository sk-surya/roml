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

use std::collections::HashMap;

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

// ── Reference + fault-injecting backend ──────────────────────────────────────

/// A deterministic in-memory backend that mirrors a model's projection and
/// "solves" by reporting an objective computed from the active objective's
/// cells at fixed variable values (1.0), plus the objective constant. Fault
/// knobs inject the recoverable/terminal/dirty behaviors the orchestration
/// must react to.
struct TestBackend {
    name: String,
    revision: ModelRevision,
    health: AdapterHealth,
    var_values: HashMap<VarId, f64>,
    objectives: HashMap<ObjId, (Sense, f64)>,
    active_objective: Option<ObjId>,
    objective_cells: HashMap<ObjId, HashMap<VarId, f64>>,

    // Fault injection.
    reject_next_delta: bool,
    fail_rebuild: bool,
    solve_fails: bool,
    solve_termination: TerminationStatus,

    // Observability counters.
    rebuilds: usize,
    deltas: usize,
    solves: usize,
    solve_revision_seen: Option<ModelRevision>,
}

impl TestBackend {
    fn new() -> Self {
        Self {
            name: "TestBackend".to_string(),
            revision: ModelRevision::ZERO,
            health: AdapterHealth::Ready,
            var_values: HashMap::new(),
            objectives: HashMap::new(),
            active_objective: None,
            objective_cells: HashMap::new(),
            reject_next_delta: false,
            fail_rebuild: false,
            solve_fails: false,
            solve_termination: TerminationStatus::Optimal,
            rebuilds: 0,
            deltas: 0,
            solves: 0,
            solve_revision_seen: None,
        }
    }

    fn force_rebuild(&mut self) {
        self.health = AdapterHealth::RequiresRebuild;
    }

    fn set_terminal(&mut self) {
        self.health = AdapterHealth::Terminal;
    }

    fn set_reject_next_delta(&mut self) {
        self.reject_next_delta = true;
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
}

impl SessionHealth for TestBackend {
    fn health(&self) -> AdapterHealth {
        self.health
    }
    fn revision(&self) -> ModelRevision {
        self.revision
    }
}

impl BackendSession for TestBackend {
    fn synchronize(&mut self, sync: Synchronization) -> Result<SyncReceipt, BackendError> {
        match sync {
            Synchronization::Rebuild(snapshot) => {
                self.rebuilds += 1;
                if self.fail_rebuild {
                    return Err(BackendError::new(
                        "injected rebuild failure",
                        ErrorCategory::Internal,
                        HealthEffect::Recoverable,
                    ));
                }
                self.project_snapshot(&snapshot);
                self.revision = snapshot.revision;
                self.health = AdapterHealth::Ready;
            }
            Synchronization::DeltaBatch(batch) => {
                if self.reject_next_delta {
                    self.reject_next_delta = false;
                    self.health = AdapterHealth::RequiresRebuild;
                    return Err(BackendError::new(
                        "injected recoverable delta rejection",
                        ErrorCategory::InvalidInput,
                        HealthEffect::Recoverable,
                    ));
                }
                if batch.from != self.revision {
                    self.health = AdapterHealth::RequiresRebuild;
                    return Err(BackendError::new(
                        format!("delta from {} != backend at {}", batch.from, self.revision),
                        ErrorCategory::InvalidInput,
                        HealthEffect::Recoverable,
                    ));
                }
                for op in &batch.operations {
                    self.apply_op(op);
                }
                self.deltas += 1;
                self.revision = batch.to;
                self.health = AdapterHealth::Ready;
            }
        }
        Ok(SyncReceipt {
            cursor: roml::sync::AdapterCursor {
                applied_revision: self.revision,
                health: self.health,
            },
            health: self.health,
        })
    }

    fn solve(&mut self, _request: &SolveRequest) -> Result<SolveResult, BackendError> {
        self.solves += 1;
        self.solve_revision_seen = Some(self.revision);
        if self.solve_fails {
            return Err(BackendError::new(
                "injected solve failure",
                ErrorCategory::Internal,
                HealthEffect::Recoverable,
            ));
        }
        let termination = self.solve_termination;
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
    let mut session = SolverSession::new(TestBackend::new());

    let solution = session.solve(&mut model).expect("first solve succeeds");
    assert_eq!(solution.status(), SolveStatus::Optimal);
    assert_eq!(
        solution.metadata().synchronization,
        SynchronizationMode::Delta
    );
    assert_eq!(solution.metadata().model_revision, model.current_revision());
    // Backend revision equals the committed model revision before solve.
    let backend_rev_at_solve = session
        .backend_mut()
        .solve_revision_seen()
        .expect("solve must record its revision");
    assert_eq!(backend_rev_at_solve, model.current_revision());
    assert!(session.last_solution().is_some());
}

/// A second solve without mutation performs no synchronization and re-solves.
#[test]
fn no_change_second_solve_uses_no_sync() {
    let (mut model, _x, _y) = build_constant_model();
    let mut session = SolverSession::new(TestBackend::new());

    let first = session.solve(&mut model).unwrap();
    assert_eq!(first.metadata().synchronization, SynchronizationMode::Delta);
    let second = session.solve(&mut model).unwrap();
    assert_eq!(
        second.metadata().synchronization,
        SynchronizationMode::NoChange
    );
    assert_eq!(session.backend_mut().deltas(), 1);
    assert_eq!(session.backend_mut().solves(), 2);
    assert_eq!(second.metadata().model_revision, model.current_revision());
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

    let mut session = SolverSession::new(TestBackend::new());
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

    let mut session = SolverSession::new(TestBackend::new());
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
    let mut backend = TestBackend::new();
    backend.force_rebuild();
    let mut session = SolverSession::new(backend);

    let solution = session.solve(&mut model).expect("rebuild recoverable");
    assert_eq!(
        solution.metadata().synchronization,
        SynchronizationMode::Rebuild
    );
    assert_eq!(session.backend_mut().rebuilds(), 1);
    assert_eq!(
        session.backend_mut().solve_revision_seen(),
        Some(model.current_revision())
    );
}

/// A terminal backend returns `Err(SolveError)` without any retry or solve.
#[test]
fn terminal_health_returns_error_without_retry_or_solve() {
    let (mut model, _x, _y) = build_constant_model();
    let mut backend = TestBackend::new();
    backend.set_terminal();
    let mut session = SolverSession::new(backend);

    let err = session.solve(&mut model).expect_err("terminal must error");
    assert!(matches!(err, SolveError::Synchronization(_)));
    assert_eq!(session.backend_mut().rebuilds(), 0);
    assert_eq!(session.backend_mut().solves(), 0);
    assert_eq!(session.backend_mut().deltas(), 0);
}

/// A recoverable delta failure triggers exactly one snapshot rebuild, then a
/// successful solve — the one-rebuild retry bound (API-02.3).
#[test]
fn recoverable_delta_failure_triggers_one_rebuild_then_solves() {
    let (mut model, _x, _y) = build_constant_model();
    let mut backend = TestBackend::new();
    backend.set_reject_next_delta();
    let mut session = SolverSession::new(backend);

    let solution = session.solve(&mut model).expect("recovered via rebuild");
    assert_eq!(
        solution.metadata().synchronization,
        SynchronizationMode::Rebuild
    );
    assert_eq!(session.backend_mut().rebuilds(), 1);
    assert_eq!(session.backend_mut().deltas(), 0);
    assert_eq!(solution.metadata().model_revision, model.current_revision());
}

/// If the rebuild attempt also fails, the solve returns an error after exactly
/// one rebuild attempt — no retry loop (API-02.3).
#[test]
fn at_most_one_rebuild_retry_when_rebuild_also_fails() {
    let (mut model, _x, _y) = build_constant_model();
    let mut backend = TestBackend::new();
    backend.set_reject_next_delta();
    backend.set_fail_rebuild();
    let mut session = SolverSession::new(backend);

    let err = session.solve(&mut model).expect_err("must error");
    assert!(matches!(err, SolveError::Synchronization(_)));
    assert_eq!(
        session.backend_mut().rebuilds(),
        1,
        "exactly one rebuild attempt"
    );
    assert_eq!(session.backend_mut().solves(), 0);
}

/// A backend ahead of the model is re-synchronized via a snapshot rebuild.
#[test]
fn backend_ahead_of_model_triggers_rebuild() {
    let (mut model, _x, _y) = build_constant_model();
    let backend = TestBackend::new();
    let mut session = SolverSession::new(backend);
    session.solve(&mut model).unwrap();

    // Simulate the backend cursor drifting ahead of the model.
    session
        .backend_mut()
        .set_revision_for_test(ModelRevision::from_u64(5));

    let second = session.solve(&mut model).expect("rebuild recovers");
    assert_eq!(
        second.metadata().synchronization,
        SynchronizationMode::Rebuild
    );
    assert_eq!(session.backend_mut().rebuilds(), 1);
    assert_eq!(session.backend().revision(), model.current_revision());
}

/// A previous solution is invalidated once the model is mutated: the next
/// solve returns a fresh solution whose metadata revision is the new one, and
/// the session never surfaces the stale value.
#[test]
fn prior_solution_invalidated_after_mutation() {
    let (mut model, _x, _y) = build_constant_model();
    let backend = TestBackend::new();
    let mut session = SolverSession::new(backend);

    let first = session.solve(&mut model).unwrap();
    assert!(session.last_solution().is_some());
    let r1 = first.metadata().model_revision;

    // Mutate the model: tighten the constraint so a fresh solve differs.
    model
        .set_variable_bounds(_x, Bounds::new(0.0, 1.0))
        .unwrap();
    let second = session.solve(&mut model).unwrap();
    let r2 = second.metadata().model_revision;
    assert_ne!(r2, r1, "the model must have advanced");
    assert_eq!(
        session.last_solution().unwrap().metadata().model_revision,
        r2,
        "prior solution must be invalidated after mutation"
    );
}

/// A failed synchronization after mutation invalidates the prior solution so
/// no stale result is ever reported as current (API-01.5).
#[test]
fn failed_solve_invalidates_prior_solution() {
    let (mut model, _x, _y) = build_constant_model();
    let backend = TestBackend::new();
    let mut session = SolverSession::new(backend);
    session.solve(&mut model).unwrap();
    assert!(session.last_solution().is_some());

    // Mutate, then make both the delta and the rebuild attempt fail.
    model
        .set_variable_bounds(_x, Bounds::new(0.0, 1.0))
        .unwrap();
    session.backend_mut().set_reject_next_delta();
    session.backend_mut().set_fail_rebuild();
    let err = session.solve(&mut model).expect_err("solve must fail");
    assert!(matches!(err, SolveError::Synchronization(_)));
    assert!(
        session.last_solution().is_none(),
        "stale solution must not survive a failed solve"
    );
}

/// The objective constant reaches the backend on the delta path exactly once:
/// the façade value equals the backend's solve value and the model expression
/// evaluation at the solution values (API-03.5).
#[test]
fn objective_constant_is_reported_exactly_once_end_to_end() {
    let (mut model, x, y) = build_constant_model();
    let mut session = SolverSession::new(TestBackend::new());
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
