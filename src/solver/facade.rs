//! User-facing solve façade and result normalization.
//!
//! This module provides:
//! - [`normalize_result`]: conversion of a backend [`SolveResult`] into the
//!   user-facing [`Solution`] (D4), kept here so HiGHS-specific code never
//!   constructs golden-path solutions directly;
//! - [`SolverSession`]: generic model-to-backend orchestration (D2).

use crate::compiler::capability::CompilationPolicy;
use crate::compiler::session::CompilationSession;
use crate::id::ObjId;
use crate::identity::{ModelInstanceId, ModelLineageId};
use crate::model::Model;
use crate::revision::ModelRevision;
use crate::snapshot::ModelSnapshot;
use crate::solution::metadata::{SolveMetadata, SynchronizationMode};
use crate::solution::{Solution, SolutionBuilder};
use crate::solver::backend::{BackendCapabilities, BackendError, ErrorCategory, HealthEffect};
use crate::solver::options::SolveOptions;
use crate::solver::request::SolveResult;
use crate::solver::session::{BackendMetadata, BackendSession, SessionHealth, Synchronization};
use crate::solver::{SolveError, SolveStatus};
use crate::sync::{AdapterCursor, AdapterHealth};

use crate::compiler::capability::{
    BackendCapabilitySet, BackendFeature, FeatureSupport, SupportLevel,
};

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
    model_lineage: ModelLineageId,
    model_instance: ModelInstanceId,
) -> Result<Solution, SolveError> {
    let status = SolveStatus::from_termination(result.termination)?;

    let mut builder = SolutionBuilder::new()
        .status(status)
        .metadata(SolveMetadata {
            backend_name: backend_name.to_string(),
            model_revision,
            effective_configuration: result.effective_configuration.clone(),
            synchronization,
            // CR-02 (SM-02.7): the solution records the SOLVED model's
            // lineage/instance ids, threaded from `SolverSession::solve_with`.
            // `SolveMetadata::default()` allocates fresh unrelated global
            // counter ids, so it must never be used here — that would make
            // every real solve report ids unequal to the solved model.
            model_lineage,
            model_instance,
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
    compiler: CompilationSession,
}

impl<B> SolverSession<B>
where
    B: BackendSession + SessionHealth + BackendMetadata,
{
    /// Create a session wrapping a backend session.
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            compiler: CompilationSession::new(),
        }
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

        // 9. Normalize and attach metadata. The solved model's lineage and
        // instance ids are bound here (CR-02, SM-02.7) — there is no separate
        // model-binds-solution step; normalize_result must never fall back to
        // fresh default ids.
        let active_objective = model.active_objective();
        let solution = normalize_result(
            &result,
            committed,
            active_objective,
            self.backend.name(),
            sync_mode,
            model.lineage(),
            model.instance(),
        )?;

        Ok(solution)
    }

    /// Compile a canonical snapshot into a [`BackendSnapshot`] for this
    /// backend (compile-before-mutation: no backend state is touched until
    /// the full canonical state has compiled successfully).
    fn compile_snapshot_for(
        &mut self,
        model: &Model,
        snapshot: &ModelSnapshot,
    ) -> Result<crate::compiler::backend_ir::BackendSnapshot, SolveError> {
        let capabilities = self.compilation_capabilities();
        self.compiler
            .compile_snapshot(
                model.instance(),
                snapshot,
                &CompilationPolicy::Auto,
                &capabilities,
            )
            .map_err(|e| {
                SolveError::Synchronization(BackendError::new(
                    format!("snapshot compilation failed: {e}"),
                    ErrorCategory::Internal,
                    HealthEffect::RequiresRebuild,
                ))
            })
    }

    fn rebuild_from_snapshot(&mut self, model: &Model) -> Result<(), SolveError> {
        let snapshot = model.take_snapshot().map_err(SolveError::Commit)?;
        let compiled = self.compile_snapshot_for(model, &snapshot)?;
        self.backend
            .synchronize(Synchronization::CompiledRebuild(compiled))
            .map_err(|e| SolveError::from_backend(true, e))?;
        Ok(())
    }

    fn apply_deltas(
        &mut self,
        model: &Model,
        backend_rev: ModelRevision,
    ) -> Result<(), SolveError> {
        // Establish the compiled base when the compiler has not yet compiled
        // anything. A fresh backend (revision ZERO) is exactly the empty
        // native state, so the compiler compiles the empty snapshot as its
        // base. The base is then SENT to the backend (as a `CompiledRebuild`)
        // before the first delta, so the backend records the exact
        // `CompilationId` it must validate deltas against (D28, WR-1). The
        // uniform contract: a backend always holds a compiled base before it
        // receives a `CompiledDeltaBatch` — there is no "un-sent empty base"
        // special case (a reference-contract backend rejects a delta whose base
        // it never received). The base-establishment rebuild is NOT a rebuild
        // retry: API-02.3's one-retry bound counts only the error-recovery
        // rebuild, and this happens on the ordinary delta path. If the backend
        // is already ahead and no base exists, a rebuild is required.
        let establish_base = self.compiler.current_compilation().is_none();
        let base_snapshot = if establish_base {
            if backend_rev == ModelRevision::ZERO {
                Some(self.compile_snapshot_for(model, &ModelSnapshot::empty(backend_rev))?)
            } else {
                return Err(SolveError::Synchronization(BackendError::new(
                    "no compiled base for the backend revision; rebuild required",
                    ErrorCategory::InvalidInput,
                    HealthEffect::RequiresRebuild,
                )));
            }
        } else {
            None
        };

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

        // Compile-before-mutation: every delta is lowered to backend IR before
        // any backend mutation. A delta that cannot be compiled incrementally
        // (rebuild-on-uncertainty, design §18 / D22) surfaces as a
        // synchronization error that the caller recovers with one deterministic
        // rebuild.
        let capabilities = self.compilation_capabilities();
        let mut compiled_batches = Vec::with_capacity(batches.len());
        for batch in batches {
            let from_compilation = self.compiler.current_compilation().ok_or_else(|| {
                SolveError::Synchronization(BackendError::new(
                    "no compiled base for delta; rebuild required",
                    ErrorCategory::InvalidInput,
                    HealthEffect::RequiresRebuild,
                ))
            })?;
            let compiled = self
                .compiler
                .compile_delta(
                    batch,
                    from_compilation,
                    &CompilationPolicy::Auto,
                    &capabilities,
                )
                .map_err(|e| {
                    SolveError::Synchronization(BackendError::new(
                        format!("delta compilation failed; rebuild required: {e}"),
                        ErrorCategory::InvalidInput,
                        HealthEffect::RequiresRebuild,
                    ))
                })?;
            compiled_batches.push(compiled);
        }

        // Now mutate the backend — compile-before-mutation is complete. Send
        // the newly established compiled base (if any) before the deltas that
        // chain from it, so the backend can validate each delta's exact
        // `from_compilation` (D28, WR-1).
        if let Some(base) = base_snapshot {
            self.backend
                .synchronize(Synchronization::CompiledRebuild(base))
                .map_err(|e| SolveError::from_backend(true, e))?;
        }
        for compiled in compiled_batches {
            self.backend
                .synchronize(Synchronization::CompiledDeltaBatch(compiled))
                .map_err(|e| SolveError::from_backend(true, e))?;
        }
        Ok(())
    }

    /// Build the typed capability set the compiler gates on, derived from the
    /// backend's flat M2 capability contract (D27 compat view). The M2-native
    /// surface maps onto the typed incremental features; flat-only fields have
    /// no typed equivalent and do not gate primitive linear compilation.
    fn compilation_capabilities(&self) -> BackendCapabilitySet {
        let flat = self.backend.capabilities();
        flat_to_typed_for_compilation(&flat)
    }
}

/// Map the flat M2 capability contract onto the typed feature surface for
/// compilation gating (SM-04.4). This is a private compile-gating view derived
/// from the backend's own `BackendMetadata::capabilities()` output — the
/// transitional flat→typed conversion helper removed by Task 6 was a public
/// helper; this one stays private to the façade.
fn flat_to_typed_for_compilation(flat: &BackendCapabilities) -> BackendCapabilitySet {
    let mut set = BackendCapabilitySet::new();
    let mut declare = |feature: BackendFeature, supported: bool| {
        if supported {
            set.set(
                feature,
                FeatureSupport {
                    level: SupportLevel::Native,
                    limitations: Default::default(),
                },
            );
        }
    };
    declare(BackendFeature::Lp, flat.lp);
    declare(BackendFeature::Mip, flat.mip);
    declare(BackendFeature::IncrementalBounds, flat.set_bounds);
    declare(
        BackendFeature::IncrementalRows,
        flat.add_variable && flat.add_constraint,
    );
    declare(
        BackendFeature::IncrementalCoefficients,
        flat.set_coefficient,
    );
    set
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
        let lineage = ModelLineageId::allocate().unwrap();
        let instance = ModelInstanceId::allocate().unwrap();
        let solution = normalize_result(
            &optimal_result(),
            ModelRevision::from_u64(2),
            Some(obj),
            "ReferenceBackend",
            SynchronizationMode::Rebuild,
            lineage,
            instance,
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
        // CR-02: the passed lineage/instance ids are recorded verbatim.
        assert_eq!(solution.metadata().model_lineage, lineage);
        assert_eq!(solution.metadata().model_instance, instance);
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
            ModelLineageId::allocate().unwrap(),
            ModelInstanceId::allocate().unwrap(),
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
            ModelLineageId::allocate().unwrap(),
            ModelInstanceId::allocate().unwrap(),
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
                ModelLineageId::allocate().unwrap(),
                ModelInstanceId::allocate().unwrap(),
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
                model.lineage(),
                model.instance(),
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
