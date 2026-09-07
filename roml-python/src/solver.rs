//! Persistent model-bound HiGHS sessions (DESIGN §5, preliminary).
//!
//! MPY-02 provides a blocking solve sufficient for the golden LP: the
//! session owns a `Mutex`-guarded `SolverSession<HighsSession>`, binds its
//! first model, applies constructor defaults with per-call overrides, and
//! returns immutable snapshots. MPY-04 completes the contract: detached
//! native execution, deterministic busy errors, exact-once destruction,
//! warm starts, and structured diagnostics.

use std::sync::Mutex;
use std::time::Duration;

use pyo3::prelude::*;
use roml::{ModelInstanceId, SolveOptions, SolverSession};
use roml_highs::HighsSession;

use super::errors::{ClosedSessionError, InvalidModelError, ModelMismatchError, SolverError};
use super::model::{py_numeric, Model};
use super::solution::{Snapshot, Solution};

type CoreSession = SolverSession<HighsSession>;

#[derive(Clone, Debug, Default)]
struct StoredOptions {
    threads: Option<i32>,
    output: Option<bool>,
    time_limit_secs: Option<f64>,
    relative_gap: Option<f64>,
    absolute_gap: Option<f64>,
    random_seed: Option<i32>,
}

impl StoredOptions {
    fn build(&self) -> SolveOptions {
        let mut options = SolveOptions::new();
        if let Some(threads) = self.threads {
            options = options.threads(threads);
        }
        if let Some(output) = self.output {
            options = options.output(output);
        }
        if let Some(limit) = self.time_limit_secs {
            options = options.time_limit(Duration::from_secs_f64(limit.max(0.0)));
        }
        if let Some(gap) = self.relative_gap {
            options = options.relative_gap(gap);
        }
        if let Some(gap) = self.absolute_gap {
            options = options.absolute_gap(gap);
        }
        if let Some(seed) = self.random_seed {
            options = options.random_seed(seed);
        }
        options
    }
}

struct SessionState {
    session: CoreSession,
    bound: Option<ModelInstanceId>,
    options: StoredOptions,
    closed: bool,
}

#[pyclass(frozen, name = "Highs")]
pub struct Session {
    state: Mutex<SessionState>,
}

fn optional_finite(value: Option<Bound<'_, PyAny>>, what: &str) -> PyResult<Option<f64>> {
    match value {
        None => Ok(None),
        Some(v) => {
            if v.is_none() {
                return Ok(None);
            }
            Ok(Some(py_numeric(&v, what)?))
        }
    }
}

#[pymethods]
impl Session {
    #[new]
    #[pyo3(signature = (*, threads = None, output = None, time_limit = None, relative_gap = None, absolute_gap = None, random_seed = None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        threads: Option<Bound<'_, PyAny>>,
        output: Option<Bound<'_, PyAny>>,
        time_limit: Option<Bound<'_, PyAny>>,
        relative_gap: Option<Bound<'_, PyAny>>,
        absolute_gap: Option<Bound<'_, PyAny>>,
        random_seed: Option<Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let threads_value = match threads {
            Some(v) => {
                let t = py_numeric(&v, "threads")?;
                if t < 1.0 || t.fract() != 0.0 {
                    return Err(InvalidModelError::new_err(
                        "threads must be a positive integer",
                    ));
                }
                t as i32
            }
            None => 1,
        };
        let output_value: bool = match output {
            Some(v) => v
                .extract()
                .map_err(|_| InvalidModelError::new_err("output must be a bool"))?,
            None => false,
        };
        let limit = optional_finite(time_limit, "time_limit")?;
        if let Some(t) = limit {
            if t <= 0.0 {
                return Err(InvalidModelError::new_err(
                    "time_limit must be positive when supplied",
                ));
            }
        }
        let relative_gap = optional_finite(relative_gap, "relative_gap")?;
        if let Some(g) = relative_gap {
            if !(0.0..=1.0).contains(&g) {
                return Err(InvalidModelError::new_err(
                    "relative_gap must lie in [0, 1]",
                ));
            }
        }
        let absolute_gap = optional_finite(absolute_gap, "absolute_gap")?;
        if let Some(g) = absolute_gap {
            if g < 0.0 {
                return Err(InvalidModelError::new_err(
                    "absolute_gap must be nonnegative",
                ));
            }
        }
        let seed = match random_seed {
            None => None,
            Some(v) => {
                if v.is_none() {
                    None
                } else {
                    let s = py_numeric(&v, "random_seed")?;
                    if s.fract() != 0.0 {
                        return Err(InvalidModelError::new_err("random_seed must be an integer"));
                    }
                    Some(s as i32)
                }
            }
        };
        let backend = HighsSession::try_new().map_err(|e| SolverError::new_err(e.to_string()))?;
        Ok(Self {
            state: Mutex::new(SessionState {
                session: SolverSession::new(backend),
                bound: None,
                options: StoredOptions {
                    threads: Some(threads_value),
                    output: Some(output_value),
                    time_limit_secs: limit,
                    relative_gap,
                    absolute_gap,
                    random_seed: seed,
                },
                closed: false,
            }),
        })
    }

    fn close(&self) -> PyResult<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SolverError::new_err("session state is invalid"))?;
        state.closed = true;
        Ok(())
    }

    fn __enter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    fn __exit__(
        &self,
        _exc_type: Bound<'_, PyAny>,
        _exc_value: Bound<'_, PyAny>,
        _traceback: Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        self.close()?;
        Ok(false)
    }

    #[pyo3(signature = (model, *, time_limit = None, relative_gap = None, absolute_gap = None, random_seed = None, start = None))]
    fn solve(
        &self,
        model: Bound<'_, Model>,
        time_limit: Option<Bound<'_, PyAny>>,
        relative_gap: Option<Bound<'_, PyAny>>,
        absolute_gap: Option<Bound<'_, PyAny>>,
        random_seed: Option<Bound<'_, PyAny>>,
        start: Option<Bound<'_, PyAny>>,
    ) -> PyResult<Solution> {
        if start.is_some() {
            return Err(super::errors::UnsupportedFeatureError::new_err(
                "warm starts are not supported by this solve",
            ));
        }
        // Per-call overrides apply to this call only; omitted values inherit
        // constructor defaults (None inherits, never clears).
        let options = {
            let state = self
                .state
                .lock()
                .map_err(|_| SolverError::new_err("session state is invalid"))?;
            if state.closed {
                return Err(ClosedSessionError::new_err("this Highs session is closed"));
            }
            let mut merged = state.options.clone();
            if let Some(v) = optional_finite(time_limit, "time_limit")? {
                if v <= 0.0 {
                    return Err(InvalidModelError::new_err(
                        "time_limit must be positive when supplied",
                    ));
                }
                merged.time_limit_secs = Some(v);
            }
            if let Some(v) = optional_finite(relative_gap, "relative_gap")? {
                merged.relative_gap = Some(v);
            }
            if let Some(v) = optional_finite(absolute_gap, "absolute_gap")? {
                merged.absolute_gap = Some(v);
            }
            if let Some(v) = random_seed {
                if !v.is_none() {
                    let s = py_numeric(&v, "random_seed")?;
                    merged.random_seed = Some(s as i32);
                }
            }
            merged.build()
        };
        // Bind-on-first-solve; a different model rejects before mutation.
        let instance = {
            let borrowed = model.borrow();
            let guard = borrowed
                .state
                .lock()
                .map_err(|_| SolverError::new_err("model state is invalid"))?;
            guard.model.instance()
        };
        let mut session = self
            .state
            .lock()
            .map_err(|_| SolverError::new_err("session state is invalid"))?;
        if session.closed {
            return Err(ClosedSessionError::new_err("this Highs session is closed"));
        }
        match session.bound {
            Some(bound) if bound != instance => {
                return Err(ModelMismatchError::new_err(
                    "this solver is bound to a different model",
                ))
            }
            Some(_) => {}
            None => session.bound = Some(instance),
        }
        // Preliminary blocking solve: both locks are held for the call.
        // MPY-04 releases the GIL, orders try_lock acquisition, and reports
        // deterministic busy errors instead.
        let borrowed = model.borrow();
        let mut guard = borrowed
            .state
            .lock()
            .map_err(|_| SolverError::new_err("model state is invalid"))?;
        let solved = session
            .session
            .solve_with(&mut guard.model, options)
            .map_err(|e| SolverError::new_err(e.to_string()))?;
        guard.pending = false;
        let snapshot = Snapshot {
            status: solved.status(),
            objective: solved.objective_value(),
            values: solved.values().clone(),
            // Preliminary candidate rule, documented: HiGHS reports values
            // only with a candidate. MPY-04 replaces this with exact
            // termination-based primal evidence.
            has_candidate: !solved.values().is_empty(),
            backend: solved.metadata().backend_name.clone(),
            instance: solved.metadata().model_instance,
            revision: solved.metadata().model_revision,
            py_revision: guard.py_revision,
        };
        Ok(Solution { snapshot })
    }

    fn __repr__(&self) -> String {
        "Highs(<session>)".to_string()
    }
}
