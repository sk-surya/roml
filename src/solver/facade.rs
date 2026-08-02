//! User-facing solve façade and result normalization.
//!
//! This module provides:
//! - [`normalize_result`]: conversion of a backend [`SolveResult`] into the
//!   user-facing [`Solution`] (D4), kept here so HiGHS-specific code never
//!   constructs golden-path solutions directly;
//! - [`SolverSession`]: generic model-to-backend orchestration (D2).

use crate::id::ObjId;
use crate::model::Model;
use crate::revision::ModelRevision;
use crate::solution::metadata::{SolveMetadata, SynchronizationMode};
use crate::solution::{Solution, SolutionBuilder};
use crate::solver::backend::{BackendError, ErrorCategory, HealthEffect};
use crate::solver::options::SolveOptions;
use crate::solver::request::SolveResult;
use crate::solver::session::{BackendMetadata, BackendSession, SessionHealth, Synchronization};
use crate::solver::{SolveError, SolveStatus};
use crate::sync::{AdapterCursor, AdapterHealth};

/// Normalize a backend [`SolveResult`] into a user-facing [`Solution`].
///
/// The backend's reported objective value already includes any objective
/// constant exactly once (the projection applies the offset via
/// `Highs_changeObjectiveOffset` for HiGHS, and reference backends follow the
/// same contract). This conversion copies it unchanged — it never re-adds the
/// model's objective constant, which is what makes the objective-constant
/// appear exactly once in reported values (API-03.5).
///
/// Mathematical terminations map to `Ok(Solution)` (with no primal values when
/// the backend returned none, e.g. infeasible); uninterpretable terminations
/// (`Error`, `Unknown`) map to `Err(SolveError::Status)` (API-03.3).
pub fn normalize_result(
    result: &SolveResult,
    model_revision: ModelRevision,
    active_objective: Option<ObjId>,
    backend_name: &str,
    synchronization: SynchronizationMode,
) -> Result<Solution, SolveError> {
    let status = SolveStatus::from_termination(result.termination)?;

    let mut builder = SolutionBuilder::new()
        .status(status)
        .metadata(SolveMetadata {
            backend_name: backend_name.to_string(),
            model_revision,
            effective_configuration: result.effective_configuration.clone(),
            synchronization,
        });

    if let Some(value) = result.solution.as_ref().and_then(|s| s.objective_value) {
        builder = builder.objective_value(value);
    }

    if let Some(obj) = active_objective {
        builder = builder.objective_id(obj);
    }

    if let Some(solution) = &result.solution {
        for (var, value) in &solution.variable_values {
            builder = builder.value(*var, *value);
        }
        if let Some(duals) = &solution.dual_values {
            for (con, value) in duals {
                builder = builder.dual(*con, *value);
            }
        }
        if let Some(costs) = &solution.reduced_costs {
            for (var, value) in costs {
                builder = builder.reduced_cost(*var, *value);
            }
        }
    }

    Ok(builder.build())
}

/// Generic model-to-backend solve orchestration (D2).
///
/// Owns one backend session and coordinates commit → synchronization →
/// solve → normalization for repeated solves on one [`Model`]. The solve
/// algorithm (plan Task 3):
///
/// 1. `model.commit()` — fails before any backend mutation (D5);
/// 2. inspect backend health and revision;
/// 3. terminal health → [`SolveError`] without retry;
/// 4. requires rebuild or missing delta chain → snapshot rebuild;
/// 5. ready and behind → apply sequential delta batches;
/// 6. ready and current → no synchronization;
/// 7. recoverable/dirty sync failure → one snapshot rebuild attempt;
/// 8. solve exactly once after successful synchronization;
/// 9. normalize the result and attach [`SolveMetadata`].
///
/// At most one automatic rebuild retry per solve attempt (API-02.3), and
/// terminal failures (including license errors) return immediately without a
/// retry. Stale-result protection is structural (API-01.5): the only way to
/// obtain a solution is [`solve`](SolverSession::solve)/
/// [`solve_with`](SolverSession::solve_with),
/// which always re-synchronize before solving, and an error path never
/// surfaces a previously computed solution.
///
/// Public surface: `new`/`solve`/`solve_with` only — the approved interface
/// (plan Task 3). The wrapped backend is deliberately not exposed.
pub struct SolverSession<B> {
    backend: B,
}

impl<B> SolverSession<B>
where
    B: BackendSession + SessionHealth + BackendMetadata,
{
    /// Create a session wrapping a backend session.
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Solve the current model with default options.
    ///
    /// # Errors
    ///
    /// Returns [`SolveError`] when the model cannot be committed, options are
    /// invalid, synchronization fails (after at most one rebuild retry), the
    /// backend solve fails, or the native termination is uninterpretable.
    pub fn solve(&mut self, model: &mut Model) -> Result<Solution, SolveError> {
        self.solve_with(model, SolveOptions::default())
    }

    /// Solve the current model with explicit [`SolveOptions`].
    ///
    /// Options are validated before any synchronization, so a failed
    /// validation leaves the model and backend state unchanged.
    ///
    /// # Errors
    ///
    /// Same failure classes as [`SolverSession::solve`]; additionally returns
    /// [`SolveError::InvalidOptions`] when `options` fails validation.
    pub fn solve_with(
        &mut self,
        model: &mut Model,
        options: SolveOptions,
    ) -> Result<Solution, SolveError> {
        // 0. Validate options before any state change (extended in Task 4).
        options.validate()?;

        // 1. Commit; fail before backend mutation on error (D5).
        let committed = model.commit().map_err(SolveError::Commit)?;

        // 2. Inspect backend health and revision.
        let health = self.backend.health();
        let backend_rev = self.backend.revision();

        // 3. Terminal -> error without retry.
        if health == AdapterHealth::Terminal {
            return Err(SolveError::Synchronization(BackendError::new(
                "backend session is terminal; create a new session",
                ErrorCategory::Internal,
                HealthEffect::Terminal,
            )));
        }

        let mut sync_mode = SynchronizationMode::NoChange;

        if health == AdapterHealth::RequiresRebuild {
            // 4. Requires rebuild -> snapshot rebuild.
            self.rebuild_from_snapshot(model)?;
            sync_mode = SynchronizationMode::Rebuild;
        } else if backend_rev < committed {
            // 5. Ready and behind -> sequential delta batches.
            match self.apply_deltas(model, backend_rev) {
                Ok(()) => sync_mode = SynchronizationMode::Delta,
                // 3/7. Terminal failures (including license errors) return
                // immediately with no retry; only recoverable/dirty sync
                // failures get the one snapshot rebuild attempt (API-02.3).
                Err(e) if e.is_terminal() => return Err(e),
                Err(_) => {
                    // 7. Recoverable/dirty sync failure -> one rebuild attempt.
                    self.rebuild_from_snapshot(model)?;
                    sync_mode = SynchronizationMode::Rebuild;
                }
            }
        } else if backend_rev > committed {
            // Backend ahead of the model (foreign cursor / future revision):
            // re-synchronize via a snapshot rebuild.
            self.rebuild_from_snapshot(model)?;
            sync_mode = SynchronizationMode::Rebuild;
        }
        // 6. backend_rev == committed -> no synchronization.

        // Post-synchronization invariant: the backend must be Ready and
        // exactly at the committed revision before any solve.
        if self.backend.health() != AdapterHealth::Ready || self.backend.revision() != committed {
            return Err(SolveError::Synchronization(BackendError::new(
                format!(
                    "backend not synchronized: health {:?}, revision {} != model {}",
                    self.backend.health(),
                    self.backend.revision(),
                    committed
                ),
                ErrorCategory::Internal,
                HealthEffect::RequiresRebuild,
            )));
        }

        // 8. Solve exactly once.
        let request = options.into_request();
        let result = self
            .backend
            .solve(&request)
            .map_err(|e| SolveError::from_backend(false, e))?;

        // 9. Normalize and attach metadata.
        let active_objective = model.active_objective();
        let solution = normalize_result(
            &result,
            committed,
            active_objective,
            self.backend.name(),
            sync_mode,
        )?;

        Ok(solution)
    }

    fn rebuild_from_snapshot(&mut self, model: &Model) -> Result<(), SolveError> {
        let snapshot = model.take_snapshot().map_err(SolveError::Commit)?;
        self.backend
            .synchronize(Synchronization::Rebuild(snapshot))
            .map_err(|e| SolveError::from_backend(true, e))?;
        Ok(())
    }

    fn apply_deltas(
        &mut self,
        model: &Model,
        backend_rev: ModelRevision,
    ) -> Result<(), SolveError> {
        let cursor = AdapterCursor {
            applied_revision: backend_rev,
            health: AdapterHealth::Ready,
        };
        let batches = model.coordinator.batches_for_cursor(&cursor).map_err(|_| {
            SolveError::Synchronization(BackendError::new(
                "delta chain unavailable for backend revision; rebuild required",
                ErrorCategory::InvalidInput,
                HealthEffect::RequiresRebuild,
            ))
        })?;
        for batch in batches {
            self.backend
                .synchronize(Synchronization::DeltaBatch((*batch).clone()))
                .map_err(|e| SolveError::from_backend(true, e))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::ConstraintExprExt;
    use crate::id::Generation;
    use crate::model::{continuous, Model};
    use crate::solver::backend::{BackendError, TerminationStatus};
    use crate::solver::request::{EffectiveConfig, SolveSolution};

    fn make_var(index: u32) -> crate::id::VarId {
        crate::id::VarId::new(index, Generation::new())
    }

    fn make_con(index: u32) -> crate::id::ConId {
        crate::id::ConId::new(index, Generation::new())
    }

    fn make_obj(index: u32) -> ObjId {
        ObjId::new(index, Generation::new())
    }

    fn optimal_result() -> SolveResult {
        SolveResult {
            effective_configuration: EffectiveConfig::default(),
            termination: TerminationStatus::Optimal,
            solution: Some(SolveSolution {
                variable_values: vec![(make_var(0), 1.0), (make_var(1), 2.0)],
                objective_value: Some(10.0),
                dual_values: Some(vec![(make_con(0), 0.5)]),
                reduced_costs: Some(vec![(make_var(0), 0.0)]),
            }),
        }
    }

    /// An optimal result converts into a `Solution` with status, values,
    /// objective value, duals, reduced costs, and objective identity.
    #[test]
    fn normalize_optimal_result_builds_solution() {
        let obj = make_obj(3);
        let solution = normalize_result(
            &optimal_result(),
            ModelRevision::from_u64(2),
            Some(obj),
            "ReferenceBackend",
            SynchronizationMode::Rebuild,
        )
        .expect("optimal must normalize");

        assert_eq!(solution.status(), SolveStatus::Optimal);
        assert_eq!(solution.value(make_var(0)), Some(1.0));
        assert_eq!(solution.value(make_var(1)), Some(2.0));
        assert_eq!(solution.objective_value(), Some(10.0));
        assert_eq!(solution.objective_id(), Some(obj));
        assert_eq!(solution.dual(make_con(0)), Some(0.5));
        assert_eq!(solution.reduced_cost(make_var(0)), Some(0.0));
        assert_eq!(
            solution.metadata().model_revision,
            ModelRevision::from_u64(2)
        );
        assert_eq!(solution.metadata().backend_name, "ReferenceBackend");
        assert_eq!(
            solution.metadata().synchronization,
            SynchronizationMode::Rebuild
        );
    }

    /// A result with no variable values converts to a `Solution` with an empty
    /// value map but a preserved objective value and status.
    #[test]
    fn normalize_missing_primal_values_yields_empty_values() {
        let result = SolveResult {
            effective_configuration: EffectiveConfig::default(),
            termination: TerminationStatus::Feasible,
            solution: Some(SolveSolution {
                variable_values: vec![],
                objective_value: Some(3.0),
                dual_values: None,
                reduced_costs: None,
            }),
        };
        let solution = normalize_result(
            &result,
            ModelRevision::ZERO,
            None,
            "ReferenceBackend",
            SynchronizationMode::NoChange,
        )
        .expect("feasible must normalize");
        assert_eq!(solution.status(), SolveStatus::Feasible);
        assert!(!solution.has_values());
        assert_eq!(solution.objective_value(), Some(3.0));
        assert!(!solution.has_duals());
        assert!(!solution.has_reduced_costs());
        assert_eq!(solution.objective_id(), None);
    }

    /// Infeasible (no primal values) converts to `Ok(Solution)` with the
    /// infeasible status and no values (API-03.3).
    #[test]
    fn normalize_infeasible_has_no_primal_values() {
        let result = SolveResult {
            effective_configuration: EffectiveConfig::default(),
            termination: TerminationStatus::Infeasible,
            solution: None,
        };
        let solution = normalize_result(
            &result,
            ModelRevision::ZERO,
            None,
            "ReferenceBackend",
            SynchronizationMode::NoChange,
        )
        .expect("infeasible must normalize to Ok(Solution)");
        assert_eq!(solution.status(), SolveStatus::Infeasible);
        assert!(!solution.has_values());
        assert_eq!(solution.objective_value(), None);
    }

    /// Error and Unknown terminations are uninterpretable: `Err(SolveError)`.
    #[test]
    fn normalize_uninterpretable_termination_returns_solve_error() {
        for termination in [TerminationStatus::Error, TerminationStatus::Unknown] {
            let result = SolveResult {
                effective_configuration: EffectiveConfig::default(),
                termination,
                solution: None,
            };
            let err = normalize_result(
                &result,
                ModelRevision::ZERO,
                None,
                "ReferenceBackend",
                SynchronizationMode::NoChange,
            )
            .expect_err("uninterpretable termination must error");
            assert!(
                matches!(err, SolveError::Status(_)),
                "unexpected error: {err:?}"
            );
        }
    }

    /// Objective constants of +5, -5, and 0 are each reported exactly once:
    /// the normalized façade value equals the backend value (which already
    /// contains the constant once) and equals evaluating the model's objective
    /// expression at the solution's variable values.
    #[test]
    fn objective_constant_appears_exactly_once() {
        for constant in [5.0, -5.0, 0.0] {
            let mut model = Model::new();
            let x = model.add_variable(continuous()).unwrap();
            let y = model.add_variable(continuous()).unwrap();
            model.add_constraint((x + y).le(100.0)).unwrap();
            let obj = model.maximize(3.0 * x + y + constant).unwrap();
            assert_eq!(model.objective_constant(obj), Some(constant));

            // Solution x = 1, y = 2. Backend reports the value including the
            // constant exactly once: 3*1 + 1*2 + constant.
            let backend_value = 3.0 * 1.0 + 1.0 * 2.0 + constant;
            let result = SolveResult {
                effective_configuration: EffectiveConfig::default(),
                termination: TerminationStatus::Optimal,
                solution: Some(SolveSolution {
                    variable_values: vec![(x, 1.0), (y, 2.0)],
                    objective_value: Some(backend_value),
                    dual_values: None,
                    reduced_costs: None,
                }),
            };
            let solution = normalize_result(
                &result,
                model.current_revision(),
                Some(obj),
                "ReferenceBackend",
                SynchronizationMode::NoChange,
            )
            .expect("optimal must normalize");

            // (1) façade value == backend value (constant not re-added).
            assert_eq!(
                solution.objective_value(),
                Some(backend_value),
                "constant {constant}: façade must equal backend value"
            );

            // (2) model expression evaluation at the solution values also
            // equals the façade value — the constant appears exactly once.
            let expression_value = model.objective_expression(obj).unwrap().evaluate(
                |var| solution.value_or_zero(var),
                |param| model.parameter_value(param).unwrap_or(0.0),
            );
            assert!(
                (expression_value - backend_value).abs() < 1e-9,
                "constant {constant}: expression {expression_value} != façade {backend_value}"
            );
        }
    }

    /// A `BackendError` from a failed solve maps to `SolveError::Solve` while
    /// preserving the wrapped error (API-02.4) — the normalization layer never
    /// hides backend identity or category.
    #[test]
    fn backend_solve_error_is_preserved_through_facade() {
        let be = BackendError::new(
            "Highs_run failed",
            crate::solver::backend::ErrorCategory::Internal,
            crate::solver::backend::HealthEffect::Recoverable,
        );
        let err = SolveError::from_backend(false, be.clone());
        assert!(matches!(err, SolveError::Solve(_)));
        assert_eq!(err.backend(), Some(&be));
    }
}
