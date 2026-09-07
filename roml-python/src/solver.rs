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

/// GIL-independent shared session state (see `SharedModel`): the `Arc`
/// crosses the detach boundary; locks are acquired and released inside.
#[derive(Clone)]
struct SharedSession {
    state: std::sync::Arc<Mutex<SessionState>>,
    poison: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[pyclass(frozen, name = "Highs")]
pub struct Session {
    shared: SharedSession,
}

/// Operational failure inside detached native work, converted to a Python
/// error only after reattachment (`PyErr` needs the GIL).
#[derive(Debug)]
enum NativeError {
    SessionBusy,
    ModelBusy,
    Closed,
    Mismatch(String),
    Solver(String),
    Poisoned(&'static str),
}

/// Owned native result data. Everything here is plain `Send` data: no
/// `MutexGuard` and no Python object crosses the detach boundary.
#[allow(clippy::too_many_arguments)]
struct NativeSolve {
    status: roml::SolveStatus,
    objective: Option<f64>,
    values: std::collections::HashMap<roml::VarId, f64>,
    has_candidate: bool,
    backend: String,
    instance: ModelInstanceId,
    lineage: roml::ModelLineageId,
    revision: roml::ModelRevision,
    py_revision: u64,
    duals: Option<std::collections::HashMap<roml::ConId, f64>>,
    reduced_costs: Option<std::collections::HashMap<roml::VarId, f64>>,
    effective_time_limit: Option<f64>,
    wall_seconds: f64,
    warm_start: super::solution::WarmStart,
}

/// Nonblocking session-state acquisition with poison fusion.
fn lock_session(
    shared: &SharedSession,
) -> Result<std::sync::MutexGuard<'_, SessionState>, NativeError> {
    use std::sync::atomic::Ordering;
    if shared.poison.load(Ordering::SeqCst) {
        return Err(NativeError::Poisoned("session"));
    }
    match shared.state.try_lock() {
        Ok(guard) => Ok(guard),
        Err(std::sync::TryLockError::WouldBlock) => Err(NativeError::SessionBusy),
        Err(std::sync::TryLockError::Poisoned(_)) => {
            shared.poison.store(true, Ordering::SeqCst);
            Err(NativeError::Poisoned("session"))
        }
    }
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
            shared: SharedSession {
                state: std::sync::Arc::new(Mutex::new(SessionState {
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
                })),
                poison: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            },
        })
    }

    fn close(&self) -> PyResult<()> {
        // Close while a solve holds the session fails deterministically
        // instead of waiting; native destruction stays exactly-once by
        // ownership when the last handle drops.
        match lock_session(&self.shared) {
            Ok(mut state) => {
                state.closed = true;
                Ok(())
            }
            Err(NativeError::SessionBusy) => Err(super::errors::SessionBusyError::new_err(
                "cannot close a Highs session while it is solving",
            )),
            Err(e) => Err(native_error(e)),
        }
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
    #[allow(clippy::too_many_arguments)]
    fn solve(
        &self,
        model: Bound<'_, Model>,
        time_limit: Option<Bound<'_, PyAny>>,
        relative_gap: Option<Bound<'_, PyAny>>,
        absolute_gap: Option<Bound<'_, PyAny>>,
        random_seed: Option<Bound<'_, PyAny>>,
        start: Option<Bound<'_, PyAny>>,
    ) -> PyResult<Solution> {
        // Attached phase: validate options and the start request. No locks
        // are held here; per-call overrides never leak into the session.
        let mut merged = {
            let state = lock_session(&self.shared).map_err(native_error)?;
            if state.closed {
                return Err(ClosedSessionError::new_err("this Highs session is closed"));
            }
            state.options.clone()
        };
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
        let effective_time_limit = merged.time_limit_secs;
        let options = merged.build();
        // Start requests validate cheaply here (type + primal presence);
        // same-model identity is enforced inside, before any mutation.
        let start_data: Option<StartData> = match start {
            None => None,
            Some(s) if s.is_none() => None,
            Some(s) => {
                let requested = s.cast::<super::solution::Solution>().map_err(|_| {
                    InvalidModelError::new_err("start must be a Solution from a previous solve")
                })?;
                let snapshot = &requested.borrow().snapshot;
                if !snapshot.has_candidate {
                    return Err(InvalidModelError::new_err(
                        "start solution carries no primal values",
                    ));
                }
                Some(StartData {
                    values: snapshot.values.clone(),
                    instance: snapshot.instance,
                })
            }
        };
        // Lock ordering is fixed (session, then model) and every
        // acquisition is a nonblocking try_lock, so contention fails fast
        // instead of deadlocking. No guard crosses the detach boundary:
        // locks are acquired and released inside the closure, which moves
        // only owned `Send` data.
        let wall_start = std::time::Instant::now();
        let session_shared = self.shared.clone();
        let model_shared = model.borrow().shared.clone();
        let native = Python::attach(|py| {
            py.detach(|| {
                Self::solve_detached(
                    &session_shared,
                    &model_shared,
                    &options,
                    start_data.as_ref(),
                    effective_time_limit,
                )
            })
        });
        let wall_seconds = wall_start.elapsed().as_secs_f64();
        match native {
            Err(e) => Err(native_error(e)),
            Ok(mut solved) => {
                solved.wall_seconds = wall_seconds;
                Ok(Solution {
                    snapshot: Snapshot {
                        status: solved.status,
                        objective: solved.objective,
                        values: solved.values,
                        has_candidate: solved.has_candidate,
                        backend: solved.backend,
                        instance: solved.instance,
                        lineage: solved.lineage,
                        revision: solved.revision,
                        py_revision: solved.py_revision,
                        duals: solved.duals,
                        reduced_costs: solved.reduced_costs,
                        effective_time_limit: solved.effective_time_limit,
                        wall_seconds: solved.wall_seconds,
                        warm_start: solved.warm_start,
                    },
                })
            }
        }
    }

    fn __repr__(&self) -> String {
        "Highs(<session>)".to_string()
    }
}

/// Owned warm-start request data (no Python objects, no guards).
struct StartData {
    values: std::collections::HashMap<roml::VarId, f64>,
    instance: ModelInstanceId,
}

fn native_error(err: NativeError) -> PyErr {
    match err {
        NativeError::SessionBusy => super::errors::SessionBusyError::new_err(
            "this Highs session is busy with another solve",
        ),
        NativeError::ModelBusy => {
            super::errors::ModelBusyError::new_err("model is busy with another operation")
        }
        NativeError::Closed => ClosedSessionError::new_err("this Highs session is closed"),
        NativeError::Mismatch(msg) => ModelMismatchError::new_err(msg),
        NativeError::Solver(msg) => SolverError::new_err(msg),
        NativeError::Poisoned(what) => {
            SolverError::new_err(format!("{what} state is poisoned and cannot be reused"))
        }
    }
}

impl Session {
    /// Execute one solve with the GIL released. Locks are acquired in
    /// fixed order (session, then model) with nonblocking `try_lock`;
    /// every guard drops before the closure returns. No Python API runs
    /// in here and no guard escapes.
    fn solve_detached(
        session_shared: &SharedSession,
        model_shared: &super::model::SharedModel,
        options: &SolveOptions,
        start: Option<&StartData>,
        effective_time_limit: Option<f64>,
    ) -> Result<NativeSolve, NativeError> {
        let mut session = lock_session(session_shared)?;
        if session.closed {
            return Err(NativeError::Closed);
        }
        let mut guard = super::model::try_model_state(model_shared).map_err(|fail| match fail {
            super::model::ModelLockFail::Busy => NativeError::ModelBusy,
            super::model::ModelLockFail::Poisoned => NativeError::Poisoned("model"),
        })?;
        let instance = guard.model.instance();
        // A solver binds to its first model and rejects another model
        // before any backend mutation.
        match session.bound {
            Some(bound) if bound != instance => {
                return Err(NativeError::Mismatch(
                    "this solver is bound to a different model".to_string(),
                ))
            }
            Some(_) => {}
            None => session.bound = Some(instance),
        }
        if let Some(requested) = start {
            if requested.instance != instance {
                return Err(NativeError::Mismatch(
                    "warm-start solution belongs to a different model".to_string(),
                ));
            }
        }
        let solved = match start {
            None => session
                .session
                .solve_with(&mut guard.model, options.clone())
                .map_err(|e| NativeError::Solver(e.to_string()))?,
            Some(requested) => {
                let assignment = roml::PrimalAssignment {
                    lineage: guard.model.lineage(),
                    source_instance: Some(instance),
                    source_revision: Some(guard.model.current_revision()),
                    values: requested.values.iter().map(|(v, x)| (*v, *x)).collect(),
                };
                let plan = roml::SolvePlan {
                    options: options.clone(),
                    overlay: roml::SolveOverlay::new(
                        std::collections::BTreeMap::new(),
                        vec![],
                        vec![],
                        vec![],
                    )
                    .map_err(|e| NativeError::Solver(format!("{e:?}")))?,
                    mip_starts: vec![roml::MipStart::new(
                        assignment,
                        roml::RepairPolicy::BackendDefault,
                    )],
                    hints: roml::VariableHints::default(),
                    objective_override: None,
                    lex_stage_policy: roml::LexStagePolicy::RequireOptimal,
                    unsupported: roml::UnsupportedFeaturePolicy::Reject,
                };
                session
                    .session
                    .solve_plan(&mut guard.model, plan)
                    .map_err(|e| NativeError::Solver(e.to_string()))?
            }
        };
        guard.pending = false;
        let metadata = solved.metadata();
        let warm_start = match start {
            None => super::solution::WarmStart::None,
            Some(_) => {
                let applied = metadata
                    .effective_plan
                    .applied_features
                    .iter()
                    .any(|f| f.feature == "mip_start");
                if applied {
                    super::solution::WarmStart::Applied
                } else {
                    super::solution::WarmStart::RequestedNotApplied
                }
            }
        };
        Ok(NativeSolve {
            status: solved.status(),
            objective: solved.objective_value(),
            has_candidate: !solved.values().is_empty(),
            values: solved.values().clone(),
            backend: metadata.backend_name.clone(),
            instance: metadata.model_instance,
            lineage: metadata.model_lineage,
            revision: metadata.model_revision,
            py_revision: guard.py_revision,
            duals: solved.duals().cloned(),
            reduced_costs: solved.reduced_costs().cloned(),
            effective_time_limit,
            wall_seconds: 0.0,
            warm_start,
        })
    }
}

#[cfg(test)]
mod lock_tests {
    use super::*;

    /// Session contention fails deterministically: holding the session
    /// guard makes a second acquisition (as `close` performs) report
    /// `SessionBusy` instead of waiting.
    #[test]
    fn session_try_lock_contention_is_busy() {
        let shared = SharedSession {
            state: std::sync::Arc::new(Mutex::new(SessionState {
                session: CoreSession::new(
                    HighsSession::try_new().expect("bundled HiGHS available"),
                ),
                bound: None,
                options: StoredOptions::default(),
                closed: false,
            })),
            poison: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        let _held = lock_session(&shared).expect("first acquisition succeeds");
        assert!(matches!(
            lock_session(&shared),
            Err(NativeError::SessionBusy)
        ));
    }
}
