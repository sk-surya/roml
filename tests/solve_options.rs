//! P21 Task 4 — Ergonomically-built `SolveOptions` (API-01.3).
//!
//! Builders map onto the immutable `SolveRequest` contract; validation of
//! non-negative gaps and positive threads happens BEFORE synchronization, so a
//! failed validation leaves the model and backend state unchanged; the
//! effective configuration and any adjustments/rejections are preserved in the
//! returned solution's metadata.

use std::time::Duration;

use roml::advanced::CompilationId;
use roml::prelude::*;
use roml::revision::ModelRevision;
use roml::solver::backend::{BackendCapabilities, BackendError, TerminationStatus};
use roml::solver::request::{EffectiveConfig, SolveRequest, SolveResult, SolveSolution};
use roml::solver::session::{
    BackendMetadata, BackendSession, SessionHealth, SyncReceipt, Synchronization,
};
use roml::sync::AdapterHealth;
use roml::SolverSession;

/// Shared recording state: the test keeps the handle so it can observe the
/// backend's recorded request without reaching into the session's backend
/// (`SolverSession` exposes only `new`/`solve`/`solve_with`).
struct RecordingState {
    last_request: Option<SolveRequest>,
    solves: usize,
}

/// A backend that records the last `SolveRequest` it received and echoes it
/// back as the effective configuration so negotiation is observable.
struct RecordingBackend {
    revision: ModelRevision,
    health: AdapterHealth,
    /// The exact `CompilationId` of the compiled state held after the most
    /// recent compiled synchronization (F2 / SM-03.9).
    current_compilation: Option<CompilationId>,
    state: std::rc::Rc<std::cell::RefCell<RecordingState>>,
}

impl RecordingBackend {
    fn new() -> (Self, std::rc::Rc<std::cell::RefCell<RecordingState>>) {
        let state = std::rc::Rc::new(std::cell::RefCell::new(RecordingState {
            last_request: None,
            solves: 0,
        }));
        (
            Self {
                revision: ModelRevision::ZERO,
                health: AdapterHealth::Ready,
                current_compilation: None,
                state: state.clone(),
            },
            state,
        )
    }
}

impl BackendMetadata for RecordingBackend {
    fn name(&self) -> &str {
        "RecordingBackend"
    }
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::all()
    }
}

impl SessionHealth for RecordingBackend {
    fn health(&self) -> AdapterHealth {
        self.health
    }
    fn revision(&self) -> ModelRevision {
        self.revision
    }
}

impl BackendSession for RecordingBackend {
    fn synchronize(&mut self, sync: Synchronization) -> Result<SyncReceipt, BackendError> {
        match sync {
            Synchronization::Rebuild(snapshot) => {
                self.revision = snapshot.revision;
            }
            Synchronization::DeltaBatch(batch) => {
                self.revision = batch.to;
            }
            Synchronization::CompiledRebuild(snapshot) => {
                self.revision = snapshot.source_revision;
                self.current_compilation = Some(snapshot.compilation_id);
            }
            Synchronization::CompiledDeltaBatch(batch) => {
                self.revision = batch.to_revision;
                self.current_compilation = Some(batch.to_compilation);
            }
        }
        self.health = AdapterHealth::Ready;
        Ok(SyncReceipt {
            cursor: roml::sync::AdapterCursor {
                applied_revision: self.revision,
                health: self.health,
            },
            health: self.health,
        })
    }

    fn solve(&mut self, request: &SolveRequest) -> Result<SolveResult, BackendError> {
        let mut state = self.state.borrow_mut();
        state.solves += 1;
        state.last_request = Some(request.clone());
        drop(state);
        let effective = EffectiveConfig {
            lp_algorithm: request.lp_algorithm,
            time_limit_secs: request.time_limit_secs,
            mip_rel_gap: request.mip_rel_gap,
            threads: request.threads,
            enable_output: request.enable_output,
            adjustments: vec![],
            rejections: vec![],
        };
        Ok(SolveResult {
            effective_configuration: effective,
            termination: TerminationStatus::Optimal,
            solution: Some(SolveSolution {
                variable_values: vec![],
                objective_value: Some(0.0),
                dual_values: None,
                reduced_costs: None,
            }),
            compilation_id: self
                .current_compilation
                .expect("a solve must follow a compiled synchronization"),
        })
    }

    fn close(self) -> Result<(), BackendError> {
        Ok(())
    }
}

/// Build `maximize x` so a solve has an active objective.
fn build_model() -> Model {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    model.maximize(x).unwrap();
    model
}

/// `time_limit(Duration)` maps onto the request's seconds field.
#[test]
fn time_limit_builder_sets_seconds() {
    let mut model = build_model();
    let (backend, state) = RecordingBackend::new();
    let mut session = SolverSession::new(backend);
    session
        .solve_with(
            &mut model,
            SolveOptions::new().time_limit(Duration::from_secs(90)),
        )
        .expect("valid options solve");
    assert_eq!(
        state
            .borrow()
            .last_request
            .as_ref()
            .unwrap()
            .time_limit_secs,
        Some(90.0)
    );
}

/// Gap and thread builders map onto their request fields.
#[test]
fn gap_and_thread_builders_set_request_fields() {
    let mut model = build_model();
    let (backend, state) = RecordingBackend::new();
    let mut session = SolverSession::new(backend);
    session
        .solve_with(
            &mut model,
            SolveOptions::new()
                .relative_gap(0.01)
                .absolute_gap(0.001)
                .threads(4),
        )
        .expect("valid options solve");
    let state = state.borrow();
    let req = state.last_request.as_ref().unwrap();
    assert_eq!(req.mip_rel_gap, Some(0.01));
    assert_eq!(req.mip_abs_gap, Some(0.001));
    assert_eq!(req.threads, Some(4));
}

/// Output and random-seed builders map onto their request fields.
#[test]
fn output_and_random_seed_builders_set_request_fields() {
    let mut model = build_model();
    let (backend, state) = RecordingBackend::new();
    let mut session = SolverSession::new(backend);
    session
        .solve_with(
            &mut model,
            SolveOptions::new().output(false).random_seed(42),
        )
        .expect("valid options solve");
    let state = state.borrow();
    let req = state.last_request.as_ref().unwrap();
    assert_eq!(req.enable_output, Some(false));
    assert_eq!(req.random_seed, Some(42));
}

/// `backend_option` appends an extra solver-specific option.
#[test]
fn backend_option_builder_appends_extra_option() {
    let mut model = build_model();
    let (backend, state) = RecordingBackend::new();
    let mut session = SolverSession::new(backend);
    session
        .solve_with(
            &mut model,
            SolveOptions::new().backend_option("solver", "simplex"),
        )
        .expect("valid options solve");
    let state = state.borrow();
    let req = state.last_request.as_ref().unwrap();
    assert!(req
        .extra_options
        .contains(&("solver".to_string(), "simplex".to_string())));
}

/// A negative relative gap is rejected before synchronization.
#[test]
fn negative_relative_gap_is_rejected_before_sync() {
    let mut model = build_model();
    let rev_before = model.current_revision();
    let (backend, state) = RecordingBackend::new();
    let mut session = SolverSession::new(backend);
    let err = session
        .solve_with(&mut model, SolveOptions::new().relative_gap(-0.01))
        .expect_err("negative gap must be rejected");
    assert!(matches!(err, SolveError::InvalidOptions(_)));
    assert_eq!(model.current_revision(), rev_before, "model untouched");
    assert_eq!(state.borrow().solves, 0, "no solve attempted");
}

/// A NaN relative gap is rejected before synchronization.
#[test]
fn nan_relative_gap_is_rejected() {
    let mut model = build_model();
    let (backend, state) = RecordingBackend::new();
    let mut session = SolverSession::new(backend);
    let err = session
        .solve_with(&mut model, SolveOptions::new().relative_gap(f64::NAN))
        .expect_err("NaN gap must be rejected");
    assert!(matches!(err, SolveError::InvalidOptions(_)));
    assert_eq!(state.borrow().solves, 0);
}

/// A negative absolute gap is rejected before synchronization.
#[test]
fn negative_absolute_gap_is_rejected() {
    let mut model = build_model();
    let (backend, state) = RecordingBackend::new();
    let mut session = SolverSession::new(backend);
    let err = session
        .solve_with(&mut model, SolveOptions::new().absolute_gap(-0.5))
        .expect_err("negative absolute gap must be rejected");
    assert!(matches!(err, SolveError::InvalidOptions(_)));
    assert_eq!(state.borrow().solves, 0);
}

/// Zero and negative thread counts are rejected before synchronization.
#[test]
fn non_positive_threads_are_rejected() {
    for threads in [0, -1, -4] {
        let mut model = build_model();
        let (backend, state) = RecordingBackend::new();
        let mut session = SolverSession::new(backend);
        let err = session
            .solve_with(&mut model, SolveOptions::new().threads(threads))
            .expect_err("non-positive threads must be rejected");
        assert!(matches!(err, SolveError::InvalidOptions(_)), "{threads}");
        assert_eq!(state.borrow().solves, 0);
    }
}

/// Failed validation leaves both the model and the backend state unchanged.
#[test]
fn failed_validation_leaves_model_and_backend_unchanged() {
    let mut model = build_model();
    let (backend, state) = RecordingBackend::new();
    let mut session = SolverSession::new(backend);
    let rev_before = model.current_revision();

    // First a successful solve advances the model to r1.
    session.solve(&mut model).unwrap();
    let rev_after_first = model.current_revision();
    assert_ne!(rev_after_first, rev_before);

    // A subsequent invalid option set must not commit or solve.
    let rev_before_invalid = model.current_revision();
    let err = session
        .solve_with(&mut model, SolveOptions::new().threads(-2))
        .expect_err("invalid options rejected");
    assert!(matches!(err, SolveError::InvalidOptions(_)));
    assert_eq!(model.current_revision(), rev_before_invalid);
    assert_eq!(state.borrow().solves, 1, "only the first solve ran");
}

/// The effective configuration (including any adjustments/rejections) is
/// preserved in the returned solution's metadata.
#[test]
fn effective_configuration_is_preserved_in_metadata() {
    let mut model = build_model();
    let mut session = SolverSession::new(RecordingBackend::new().0);
    let solution = session
        .solve_with(
            &mut model,
            SolveOptions::new()
                .threads(6)
                .time_limit(Duration::from_secs(30)),
        )
        .expect("valid options solve");
    let effective = &solution.metadata().effective_configuration;
    assert_eq!(effective.threads, Some(6));
    assert_eq!(effective.time_limit_secs, Some(30.0));
}

/// A model without an active objective is still solved; the solution carries
/// no objective identity.
#[test]
fn default_options_solve_succeeds() {
    let mut model = Model::new();
    model.add_variable(continuous()).unwrap();
    let mut session = SolverSession::new(RecordingBackend::new().0);
    let solution = session
        .solve_with(&mut model, SolveOptions::new())
        .expect("default options are valid");
    assert_eq!(solution.status(), SolveStatus::Optimal);
    let _ = &solution;
}

/// `SolveOptions` is default-constructible and `Clone`.
#[test]
fn solve_options_is_default_and_clone() {
    let a = SolveOptions::default();
    let _b = a.clone();
    let _c = SolveOptions::new();
}
