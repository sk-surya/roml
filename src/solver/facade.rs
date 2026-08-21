//! User-facing solve façade and result normalization.
//!
//! This module provides:
//! - [`normalize_result`]: conversion of a backend [`SolveResult`] into the
//!   user-facing [`Solution`] (D4), kept here so HiGHS-specific code never
//!   constructs golden-path solutions directly;
//! - [`SolverSession`]: generic model-to-backend orchestration (D2).

use std::collections::BTreeMap;

use log::warn;

use crate::assignment::PrimalAssignment;
use crate::compiler::backend_ir::CompilationId;
use crate::compiler::capability::{BackendCapabilitySet, BackendFeature, CompilationPolicy};
use crate::compiler::origin::OverlayId;
use crate::compiler::session::CompilationSession;
use crate::id::ObjId;
use crate::identity::{ModelInstanceId, ModelLineageId};
use crate::model::{Model, VarType};
use crate::revision::ModelRevision;
use crate::snapshot::ModelSnapshot;
use crate::solution::metadata::{SolveMetadata, SynchronizationMode};
use crate::solution::{Solution, SolutionBuilder};
use crate::solver::backend::{BackendError, ErrorCategory, HealthEffect};
use crate::solver::effective_plan::{
    AppliedFeature, EffectiveSolvePlan, PlanAdjustment, PlanRejection,
};
use crate::solver::options::SolveOptions;
use crate::solver::overlay::{
    compile_overlay, OverlayApplyReceipt, OverlayRollbackOutcome, SolveOverlay,
};
use crate::solver::plan::{
    LexStagePolicy, MipStart, ObjectivePolicy, PlanError, RepairPolicy, SolvePlan,
    UnsupportedFeaturePolicy, VariableHints,
};
use crate::solver::relaxation::{
    compile_portable_overlay, report_members, unknown_reason, validate_plan_preflight,
    FeasibilityRelaxationError, FeasibilityRelaxationPlan, FeasibilityRelaxationReport,
    RelaxationAcceptance, RelaxationExecutionProvider, RelaxationOutcome, RelaxationProviderPolicy,
};
use crate::solver::request::{validate_request, SolveRequest, SolveResult};
use crate::solver::session::{
    BackendMetadata, BackendSession, OverlaySession, SessionHealth, Synchronization,
};
use crate::solver::{SolveError, SolveStatus};
use crate::sync::{AdapterCursor, AdapterHealth};
use crate::Variable;

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
#[allow(clippy::too_many_arguments)]
pub fn normalize_result(
    result: &SolveResult,
    model_revision: ModelRevision,
    active_objective: Option<ObjId>,
    backend_name: &str,
    synchronization: SynchronizationMode,
    effective_plan: EffectiveSolvePlan,
    model_lineage: ModelLineageId,
    model_instance: ModelInstanceId,
    compilation_id: Option<CompilationId>,
    overlay_id: Option<OverlayId>,
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
            // F2 (SM-03.9): the exact `CompilationId` of the compiled state
            // the backend solved. F5: the real solve path sets `Some(actual)`;
            // `None` is only ever a synthetic solution (no compiled state).
            compilation_id,
            // P27 Task 10 (D28 overlay artifacts): `Some(overlay.id)` on an
            // overlay solve; `None` for a plain solve.
            overlay_id,
            // P28 (SM-04.5, SM-07.7): the effective solve plan — applied
            // features, adjustments/conversions, rejections, and objective
            // stages — recorded on every real solve.
            effective_plan,
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
    /// The model instance whose compiled state the backend currently holds
    /// (F1). `None` before the first successful synchronization. Used to
    /// detect cross-model `SolverSession` reuse so a different model at an
    /// overlapping revision never silently solves the previous model's
    /// backend state.
    bound_instance: Option<ModelInstanceId>,
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
            bound_instance: None,
        }
    }

    /// Borrow the persistent backend for additive analysis orchestration.
    pub(crate) fn analysis_backend(&self) -> &B {
        &self.backend
    }

    /// Compile an analysis snapshot with an independent identity compiler.
    /// IIS probing must not advance or replace the compiler state associated
    /// with the persistent solve session.
    pub(crate) fn compile_analysis_snapshot(
        &self,
        model: &Model,
        snapshot: &ModelSnapshot,
        overlay: Option<&SolveOverlay>,
    ) -> Result<
        crate::compiler::backend_ir::BackendSnapshot,
        crate::solver::infeasibility::InfeasibilityError,
    > {
        let capabilities = self.compilation_capabilities();
        let mut compiler = CompilationSession::new();
        let base = compiler
            .compile_snapshot(
                model.instance(),
                snapshot,
                &CompilationPolicy::Auto,
                &capabilities,
            )
            .map_err(
                |error| crate::solver::infeasibility::InfeasibilityError::InvalidUniverse {
                    reason: format!("analysis snapshot compilation failed: {error}"),
                },
            )?;
        let Some(overlay) = overlay else {
            return Ok(base);
        };
        let compiled = compile_overlay(model, &compiler, overlay, None).map_err(|error| {
            crate::solver::infeasibility::InfeasibilityError::InvalidUniverse {
                reason: format!("analysis overlay compilation failed: {error:?}"),
            }
        })?;
        let mut analysis = base;
        for operation in compiled.operations {
            match operation {
                crate::solver::overlay::OverlayOp::AddTemporaryVariable { variable } => {
                    analysis.variables.push(variable);
                }
                crate::solver::overlay::OverlayOp::AddTemporaryObjective { objective } => {
                    analysis.objectives.push(objective);
                }
                crate::solver::overlay::OverlayOp::SetTemporaryVariableBounds {
                    variable,
                    bounds,
                } => {
                    let target = analysis
                        .variables
                        .iter_mut()
                        .find(|entry| entry.id == variable)
                        .ok_or_else(|| {
                            crate::solver::infeasibility::InfeasibilityError::InvalidUniverse {
                                reason: format!(
                                    "analysis overlay references missing variable {variable:?}"
                                ),
                            }
                        })?;
                    target.bounds = bounds;
                }
                crate::solver::overlay::OverlayOp::AddTemporaryRow { row } => {
                    analysis.linear_rows.push(row);
                }
                crate::solver::overlay::OverlayOp::SetTemporaryObjectiveCoefficient {
                    objective,
                    variable,
                    value,
                } => {
                    if let Some(target) = analysis
                        .objectives
                        .iter_mut()
                        .find(|entry| entry.id == objective)
                    {
                        if let Some(existing) = target
                            .coefficients
                            .iter_mut()
                            .find(|(id, _)| *id == variable)
                        {
                            existing.1 = value;
                        } else {
                            target.coefficients.push((variable, value));
                            target.coefficients.sort_by_key(|(id, _)| *id);
                        }
                    }
                }
                crate::solver::overlay::OverlayOp::SetTemporaryRowBounds { constraint, bounds } => {
                    if let Some(target) = analysis
                        .linear_rows
                        .iter_mut()
                        .find(|entry| entry.id == constraint)
                    {
                        target.bounds = bounds;
                    }
                }
                crate::solver::overlay::OverlayOp::RemoveTemporaryRow { .. }
                | crate::solver::overlay::OverlayOp::SetObjectivePolicy(_) => {}
            }
        }
        analysis.origin_map.merge(compiled.origin_additions);
        analysis.compilation_id = compiled.compilation_id;
        Ok(analysis)
    }

    /// Synchronize the backend to the model's committed canonical state,
    /// establishing the exact base `CompilationId` (`C_base`).
    ///
    /// Shared by [`solve_with`](Self::solve_with) and
    /// [`solve_with_overlay`](Self::solve_with_overlay). Steps 0-7 of the
    /// plan's solve algorithm: validate options, validate the request against
    /// the backend's authoritative typed capability set, commit, inspect
    /// backend health/revision, apply the synchronization decision (snapshot
    /// rebuild / sequential deltas / no sync), assert the post-sync invariant,
    /// and bind the session to the model instance.
    ///
    /// Returns the validated [`SolveRequest`], the committed [`ModelRevision`],
    /// and the [`SynchronizationMode`] for the solve's metadata.
    fn synchronize_base(
        &mut self,
        model: &mut Model,
        options: SolveOptions,
    ) -> Result<(SolveRequest, ModelRevision, SynchronizationMode), SolveError> {
        // 0. Validate options before any state change (extended in Task 4).
        options.validate()?;

        // 0b. F3 (SM-04.4): validate the request against the backend's
        // AUTHORITATIVE typed capability set BEFORE any state change. A
        // requested option whose feature the backend does not support natively
        // is rejected — never silently passed to the backend.
        let request = options.into_request();
        let capabilities = self.compilation_capabilities();
        let rejections = validate_request(&request, &capabilities);
        if !rejections.is_empty() {
            let reason = rejections
                .iter()
                .map(|r| format!("{}: {}", r.key, r.reason))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(SolveError::InvalidOptions(reason));
        }

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

        // F1 (cross-model session reuse): a `SolverSession` is bound to the
        // ONE model instance whose state is currently compiled in the backend.
        // When the incoming model is a DIFFERENT instance, the backend's
        // compiled base belongs to another model — the revision-based decision
        // below must NOT reuse it (two unrelated models at the same revision
        // would otherwise solve the previous model's state). Force the full
        // snapshot synchronization path: reset the per-model compiler so the
        // new model compiles fresh (the compiler's source-instance guard would
        // otherwise reject the cross-model recompile with `RebuildRequired`,
        // D28), then re-establish the compiled base for THIS instance.
        //
        // `None` (first solve) needs no separate branch: a fresh backend is at
        // revision ZERO, so the ordinary revision-based decision always
        // synchronizes fully (compiled empty base + deltas) and never reaches
        // the no-sync fast path.
        let cross_model = self.bound_instance.is_some_and(|b| b != model.instance());
        if cross_model {
            self.compiler = CompilationSession::new();
            self.rebuild_from_snapshot(model)?;
            sync_mode = SynchronizationMode::Rebuild;
        } else if health == AdapterHealth::RequiresRebuild {
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
        } else if self.compiler.current_compilation().is_none() {
            // 6a. (F4): backend_rev == committed BUT the compiler holds no
            // compiled base — a fresh backend at revision ZERO and a
            // revision-ZERO model (e.g. an untouched `Model::new()`). The
            // no-sync path would send the model straight to solve against a
            // backend with no compiled state ("no compiled synchronization").
            // Force the snapshot-rebuild path so the compiled base is
            // established before solve.
            self.rebuild_from_snapshot(model)?;
            sync_mode = SynchronizationMode::Rebuild;
        }
        // 6. backend_rev == committed && the compiler holds a compiled base ->
        //    no synchronization.

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

        // F1: bind the session to the model instance whose compiled state the
        // backend now holds. Bound only after a successful synchronization, so
        // a failed sync never leaves a stale binding.
        self.bound_instance = Some(model.instance());

        Ok((request, committed, sync_mode))
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
                    model.instance(),
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

    /// The typed capability set the compiler and request validation gate on.
    /// F3 (SM-04.1): the backend's `typed_capabilities()` is AUTHORITATIVE —
    /// the flat `capabilities()` compat view is never used for gating (a flat
    /// view can lie; the typed view cannot).
    fn compilation_capabilities(&self) -> BackendCapabilitySet {
        self.backend.typed_capabilities().clone()
    }

    /// Solve the current model with default options.
    ///
    /// Plain solve path (D27, review P1): requires ONLY the pre-P28 traits
    /// (`BackendSession + SessionHealth + BackendMetadata`). A backend that
    /// does not implement [`OverlaySession`] can call `solve`/`solve_with`
    /// unchanged — the methods live in this unbounded impl block.
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
    /// Plain solve path (D27, review P1) — see [`SolverSession::solve`].
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
        let mut effective = EffectiveSolvePlan::default();
        self.solve_base(model, options, &mut effective)
    }

    /// The shared plain solve core (the `C_base` path): options validation +
    /// capability preflight + commit + synchronize to `C_base` + solve + the
    /// exact `CompilationId` gate + metadata normalization.
    ///
    /// Used directly by [`solve`](Self::solve)/[`solve_with`](Self::solve_with)
    /// and by [`solve_plan`](Self::solve_plan) for plans whose effective
    /// content is empty — the `solve == solve_with == empty solve_plan`
    /// equivalence (D27, SM-07.2) is one code path, not a coincidence.
    fn solve_base(
        &mut self,
        model: &mut Model,
        options: SolveOptions,
        effective: &mut EffectiveSolvePlan,
    ) -> Result<Solution, SolveError> {
        let (request, committed, sync_mode) = self.synchronize_base(model, options)?;
        let result = self
            .backend
            .solve(&request)
            .map_err(|e| SolveError::from_backend(false, e))?;
        // Exact `CompilationId` gate (F2, SM-03.9): a plain solve's result
        // must be tagged C_base — anything else is a typed
        // CompilationMismatch (no overlay to roll back on this path).
        let expected = self.compiler.current_compilation();
        if expected != result.compilation_id {
            return Err(SolveError::CompilationMismatch {
                expected,
                actual: result.compilation_id,
            });
        }
        normalize_result(
            &result,
            committed,
            model.active_objective(),
            self.backend.name(),
            sync_mode,
            effective.clone(),
            model.lineage(),
            model.instance(),
            result.compilation_id,
            None,
        )
    }
}

/// The concrete warm-start application list produced by
/// [`SolverSession::resolve_plan_features`] (P28 Task 2).
///
/// Starts/hints here are QUALIFIED (the backend declares native support) and
/// will be applied through the `OverlaySession` warm-start methods; the
/// effective overlay carries the plan's overlay plus any
/// `ConvertStartToTemporaryFixing` conversions.
struct ResolvedPlanFeatures {
    /// Qualified starts to apply natively, in plan order.
    starts: Vec<MipStart>,
    /// Qualified hints to apply natively.
    hints: VariableHints,
    /// The effective overlay (plan overlay + conversions).
    effective_overlay: SolveOverlay,
}

impl<B> SolverSession<B>
where
    B: BackendSession + SessionHealth + BackendMetadata + OverlaySession,
{
    /// Solve the current model under a reversible solve overlay (P27 Task 10,
    /// design §12, SM-07.3..SM-07.6).
    ///
    /// The pinned lifecycle, executed transactionally from the caller's
    /// perspective:
    ///
    /// ```text
    /// commit canonical model
    /// -> compile/synchronize canonical state          (C_base established)
    /// -> compile overlay against the exact C_base     (fresh C_overlay)
    /// -> apply overlay                                (C_base -> C_overlay)
    /// -> solve                                         (result tagged C_overlay)
    /// -> validate result.compilation_id == C_overlay  (NOT C_base — the
    ///    compiler stays at C_base throughout the overlay solve)
    /// -> rollback overlay (always attempted, even on solve/extraction
    ///    failure)                                     (C_overlay -> C_base)
    /// -> on RequiresRebuild, mark the backend RequiresRebuild (D7, D22)
    /// -> verify C_base restored
    /// -> normalize the result with compilation_id = C_overlay and
    ///    overlay_id = Some(overlay.id)
    /// ```
    ///
    /// Temporary fixings, solution locks, objective-lock rows, and cutoffs
    /// never advance the canonical model revision (SM-07.3). An uncertain
    /// rollback marks the session `RequiresRebuild`; the next solve forces a
    /// snapshot rebuild before reuse (SM-07.5).
    ///
    /// # Errors
    ///
    /// - [`SolveError::Overlay`] — the overlay failed to compile (stale base,
    ///   assignment/band/value validation, unknown objective) before any
    ///   backend mutation;
    /// - [`SolveError::Rollback`] — a backend error during apply, rollback, or
    ///   post-rollback verification (the backend is marked `RequiresRebuild`);
    /// - [`SolveError::CompilationMismatch`] — the solve result was tagged with
    ///   a `CompilationId` other than `C_overlay` (extraction failure);
    /// - the ordinary [`SolveError`] classes of [`solve_with`](Self::solve_with)
    ///   (commit, options, synchronization, solve, status).
    ///
    /// Execute a [`SolvePlan`] through the single plan executor (P28 Task 2,
    /// design §12; SM-07.1, SM-07.2).
    ///
    /// This is the ONE plan-execution entry point: `solve`, `solve_with`, and
    /// `solve_with_overlay` all construct a [`SolvePlan`] and delegate here, so
    /// there is no divergent code path (the plan executor is the single
    /// place where a solve attempt is validated, synchronized, overlaid,
    /// warm-started, solved, extracted, and normalized).
    ///
    /// The pinned lifecycle:
    ///
    /// ```text
    /// validate plan (typed SolveError::Plan before any synchronization)
    /// -> resolve starts/hints against backend typed capabilities + policy
    ///    (default-reject error, or recorded conversion/rejection)
    /// -> commit + compile/synchronize canonical state      (C_base established)
    /// -> compile overlay against the exact C_base          (fresh C_overlay)
    /// -> apply overlay                                     (C_base -> C_overlay)
    /// -> apply qualified starts/hints                      (recorded features)
    /// -> solve                                             (result tagged C_base
    ///    or C_overlay)
    /// -> validate result.compilation_id == expected (C_overlay on an overlay
    ///    solve, C_base otherwise) — a mismatch is a typed CompilationMismatch
    /// -> rollback overlay + verify C_base restored
    /// -> normalize with the EffectiveSolvePlan (SM-04.5, SM-07.7)
    /// ```
    ///
    /// An empty plan (no overlay content, no override, no starts/hints) is
    /// exactly `solve`/`solve_with`: the overlay path is skipped and the
    /// result is tagged `C_base` (D27, SM-07.2).
    ///
    /// # Errors
    ///
    /// - [`SolveError::Plan`] — the plan failed validation or requested an
    ///   unqualified feature under the default-reject policy (SM-08.4), before
    ///   any synchronization or backend mutation;
    /// - [`SolveError::Overlay`] — the overlay failed to compile (stale base,
    ///   assignment/band/value validation, unknown objective);
    /// - [`SolveError::Rollback`] — a backend error during apply, rollback, or
    ///   post-rollback verification (the backend is marked `RequiresRebuild`);
    /// - [`SolveError::CompilationMismatch`] — the solve result was tagged with
    ///   a `CompilationId` other than the expected compiled state;
    /// - the ordinary [`SolveError`] classes of `solve_with` (commit, options,
    ///   synchronization, solve, status).
    pub fn solve_plan(
        &mut self,
        model: &mut Model,
        plan: SolvePlan,
    ) -> Result<Solution, SolveError> {
        // 1. Validate the plan against the model BEFORE any synchronization
        //    (design §12 "validate plan" step; SM-08.4).
        plan.validate(model).map_err(SolveError::Plan)?;

        // 2. Resolve starts/hints against the backend's typed capabilities and
        //    the plan's unsupported-feature policy. Default rejection returns a
        //    typed error BEFORE any backend mutation; explicit conversions are
        //    recorded; unconvertible requests under a conversion policy are
        //    recorded as rejections (never silent — SM-04.5, SM-08.5).
        let mut effective = EffectiveSolvePlan::default();
        let resolved = self.resolve_plan_features(model, &plan, &mut effective)?;

        // Plain-path shortcut (D27, review P1): when the effective plan has no
        // overlay content, no objective override, and no starts/hints, the
        // solve is EXACTLY `solve`/`solve_with` — the shared unbounded plain
        // core (`solve_base`) runs it, so the equivalence holds and the
        // backend need not implement `OverlaySession`.
        let plain = plan.objective_override.is_none()
            && resolved.effective_overlay.temporary_fixings.is_empty()
            && resolved.effective_overlay.locks.is_empty()
            && resolved.effective_overlay.objective_locks.is_empty()
            && resolved.effective_overlay.cutoffs.is_empty()
            && resolved.starts.is_empty()
            && resolved.hints.is_empty();
        if plain {
            return self.solve_base(model, plan.options, &mut effective);
        }

        // 3. Commit + synchronize to C_base. Options and the request are
        //    validated against the backend's authoritative typed capability set
        //    inside `synchronize_base`.
        let (request, committed, sync_mode) = self.synchronize_base(model, plan.options)?;

        // The objective override (P28: `ObjectivePolicy::Single` only). P31's
        // `Weighted`/`Lexicographic` policies are executed by the portable
        // objective executor rather than the single-objective override path;
        // placing a non-single policy here is rejected before mutation.
        let objective_override = match plan.objective_override {
            None => None,
            Some(ObjectivePolicy::Single(obj)) => Some(obj),
            Some(_) => return Err(SolveError::Plan(PlanError::NonSingleObjectiveOverride)),
        };
        // SM-04.5 (review P2-04): an applied override is recorded — a consumer
        // reading the effective plan can tell that the objective was replaced.
        if objective_override.is_some() {
            effective.adjustments.push(PlanAdjustment {
                key: "objective_override".into(),
                requested: "model objective".into(),
                applied: "override objective".into(),
                reason: "ObjectivePolicy::Single override compiled into the overlay (P28)".into(),
            });
        }

        // An overlay apply is needed only when the effective overlay (the
        // plan's overlay plus any start->fixing conversions) has content or an
        // objective override is present. An empty overlay keeps the plain
        // C_base path so an empty `solve_plan` is exactly `solve`/`solve_with`
        // (D27, SM-07.2).
        let apply_overlay = objective_override.is_some()
            || !resolved.effective_overlay.temporary_fixings.is_empty()
            || !resolved.effective_overlay.locks.is_empty()
            || !resolved.effective_overlay.objective_locks.is_empty()
            || !resolved.effective_overlay.cutoffs.is_empty();
        let applied_overlay_id = if apply_overlay {
            Some(resolved.effective_overlay.id)
        } else {
            None
        };

        let mut overlay_receipt: Option<OverlayApplyReceipt> = None;
        let mut solved_compilation: Option<CompilationId> = None;

        if apply_overlay {
            // 4. Compile the overlay against the exact base (a stale/invalid
            //    overlay fails BEFORE any backend mutation) and apply it. The
            //    overlay is NEVER compiled into the CompilationSession;
            //    `compiler.current_compilation()` stays C_base throughout.
            let compiled = compile_overlay(
                model,
                &self.compiler,
                &resolved.effective_overlay,
                objective_override,
            )
            .map_err(SolveError::Overlay)?;
            solved_compilation = Some(compiled.compilation_id);
            let receipt = match self.backend.apply_overlay(&compiled) {
                Ok(receipt) => receipt,
                Err(e) => {
                    // CR-02: an apply failure leaves the session RequiresRebuild
                    // so the half-overlaid native state is never silently reused.
                    self.force_rebuild_on_next_sync();
                    return Err(SolveError::Rollback(e));
                }
            };
            overlay_receipt = Some(receipt);
        }

        // 5. Apply qualified starts/hints through the OverlaySession methods.
        if let Err(e) = self.apply_warm_starts(&resolved, &mut effective) {
            // CR-02 mirror: a warm-start application failure leaves the session
            // RequiresRebuild so a partially-applied incumbent is never
            // silently reused by a later solve of the unchanged model (the
            // no-sync fast path would otherwise keep the stale start).
            self.force_rebuild_on_next_sync();
            // Rollback is ALWAYS attempted on the failure paths (SM-07.4).
            if let Some(receipt) = &overlay_receipt {
                let _ = self.rollback_and_verify(receipt);
            }
            return Err(e);
        }

        // 6. Solve exactly once.
        let result = match self.backend.solve(&request) {
            Ok(result) => result,
            Err(e) => {
                let err = SolveError::from_backend(false, e);
                if let Some(receipt) = &overlay_receipt {
                    let _ = self.rollback_and_verify(receipt);
                }
                return Err(err);
            }
        };

        // 7. Exact `CompilationId` extraction gate (F2, SM-03.9). The result
        //    must be tagged with C_overlay on an overlay solve (NOT
        //    `compiler.current_compilation()`, which stays C_base), or C_base
        //    on a plain solve. Anything else is a typed CompilationMismatch
        //    and rollback is still attempted.
        let expected = solved_compilation.or_else(|| self.compiler.current_compilation());
        if expected != result.compilation_id {
            let err = SolveError::CompilationMismatch {
                expected,
                actual: result.compilation_id,
            };
            if let Some(receipt) = &overlay_receipt {
                let _ = self.rollback_and_verify(receipt);
            }
            return Err(err);
        }

        // 8. Rollback + verify C_base restored (SM-07.4, SM-07.5).
        if let Some(receipt) = &overlay_receipt {
            self.rollback_and_verify(receipt)?;
        }

        // 9. Normalize with the effective plan and the exact compilation id.
        //    The active objective for an override solve is the override; a
        //    plain solve keeps the model's active objective.
        let active_objective = objective_override.or_else(|| model.active_objective());
        normalize_result(
            &result,
            committed,
            active_objective,
            self.backend.name(),
            sync_mode,
            effective,
            model.lineage(),
            model.instance(),
            result.compilation_id,
            applied_overlay_id,
        )
    }

    /// Solve the current model under a reversible solve overlay (P27 Task 10,
    /// design §12, SM-07.3..SM-07.6).
    ///
    /// Convenience path: constructs a [`SolvePlan`] carrying the overlay and
    /// objective override, then routes through the single plan executor
    /// ([`solve_plan`](Self::solve_plan)).
    ///
    /// The pinned lifecycle (see [`solve_plan`](Self::solve_plan)):
    ///
    /// ```text
    /// commit canonical model
    /// -> compile/synchronize canonical state          (C_base established)
    /// -> compile overlay against the exact C_base     (fresh C_overlay)
    /// -> apply overlay                                (C_base -> C_overlay)
    /// -> solve                                         (result tagged C_overlay)
    /// -> validate result.compilation_id == C_overlay  (NOT C_base — the
    ///    compiler stays at C_base throughout the overlay solve)
    /// -> rollback overlay (always attempted, even on solve/extraction
    ///    failure)                                     (C_overlay -> C_base)
    /// -> on RequiresRebuild, mark the backend RequiresRebuild (D7, D22)
    /// -> verify C_base restored
    /// -> normalize the result with compilation_id = C_overlay and
    ///    overlay_id = Some(overlay.id)
    /// ```
    ///
    /// Temporary fixings, solution locks, objective-lock rows, and cutoffs
    /// never advance the canonical model revision (SM-07.3). An uncertain
    /// rollback marks the session `RequiresRebuild`; the next solve forces a
    /// snapshot rebuild before reuse (SM-07.5).
    ///
    /// # Errors
    ///
    /// - [`SolveError::Overlay`] — the overlay failed to compile (stale base,
    ///   assignment/band/value validation, unknown objective) before any
    ///   backend mutation;
    /// - [`SolveError::Rollback`] — a backend error during apply, rollback, or
    ///   post-rollback verification (the backend is marked `RequiresRebuild`);
    /// - [`SolveError::CompilationMismatch`] — the solve result was tagged with
    ///   a `CompilationId` other than `C_overlay` (extraction failure);
    /// - the ordinary [`SolveError`] classes of [`solve_with`](Self::solve_with)
    ///   (commit, options, synchronization, solve, status).
    pub fn solve_with_overlay(
        &mut self,
        model: &mut Model,
        options: SolveOptions,
        overlay: &SolveOverlay,
        objective_override: Option<ObjId>,
    ) -> Result<Solution, SolveError> {
        let plan = SolvePlan {
            options,
            overlay: overlay.clone(),
            mip_starts: Vec::new(),
            hints: VariableHints::default(),
            objective_override: objective_override.map(ObjectivePolicy::Single),
            lex_stage_policy: LexStagePolicy::RequireOptimal,
            unsupported: UnsupportedFeaturePolicy::Reject,
        };
        self.solve_plan(model, plan)
    }

    /// Run one isolated portable weighted-L1 feasibility repair.
    ///
    /// The canonical model is committed and synchronized to an exact base,
    /// then the generated violation variables/rows/objective are applied as a
    /// solve-scoped overlay. The overlay is always rolled back before a report
    /// can escape. Persistent soft constructs and the canonical revision are
    /// never changed by this method.
    pub fn solve_feasibility_relaxation(
        &mut self,
        model: &mut Model,
        plan: FeasibilityRelaxationPlan,
    ) -> Result<FeasibilityRelaxationReport, FeasibilityRelaxationError> {
        validate_plan_preflight(model, &plan)?;
        let (provider, provider_reason) = match plan.provider_policy {
            RelaxationProviderPolicy::PortableOnly => {
                (RelaxationExecutionProvider::PortableRoml, None)
            }
            RelaxationProviderPolicy::PreferNative => (
                RelaxationExecutionProvider::PortableRoml,
                Some(
                    "no qualified native feasibility-relaxation provider; used portable ROML"
                        .into(),
                ),
            ),
            RelaxationProviderPolicy::NativeRequired => {
                return Err(FeasibilityRelaxationError::NativeProviderRequired)
            }
        };

        let (request, committed, sync_mode) = self
            .synchronize_base(model, SolveOptions::default())
            .map_err(|error| FeasibilityRelaxationError::Backend(error.to_string()))?;
        let base_compilation = self.compiler.current_compilation().ok_or_else(|| {
            FeasibilityRelaxationError::Preflight(
                "synchronization produced no base compilation".into(),
            )
        })?;
        let (compiled, weighted) = compile_portable_overlay(model, &self.compiler, &plan)?;
        let relaxation_compilation_id = compiled.compilation_id;
        let overlay_id = compiled.overlay_id;
        let receipt = match self.backend.apply_overlay(&compiled) {
            Ok(receipt) => receipt,
            Err(error) => {
                // An apply error carries no receipt, but the backend may have
                // mutated before reporting it. Reconstruct the exact receipt
                // tuple from the immutable compiled overlay and run the same
                // rollback/verification path as every later failure. This
                // preserves both the primary apply error and any cleanup or
                // rebuild evidence.
                let possible_receipt = OverlayApplyReceipt {
                    overlay_id,
                    base_compilation: compiled.base_compilation,
                    applied_compilation: compiled.compilation_id,
                };
                return self.relaxation_failure(
                    FeasibilityRelaxationError::Backend(error.to_string()),
                    &possible_receipt,
                );
            }
        };
        if receipt.overlay_id != overlay_id
            || receipt.base_compilation != compiled.base_compilation
            || receipt.applied_compilation != relaxation_compilation_id
        {
            return self.relaxation_failure(
                FeasibilityRelaxationError::Backend(format!(
                    "relaxation apply returned mismatched receipt: {:?}",
                    receipt
                )),
                &receipt,
            );
        }

        let result = match self.backend.solve(&request) {
            Ok(result) => result,
            Err(error) => {
                return self.relaxation_failure(
                    FeasibilityRelaxationError::Backend(error.to_string()),
                    &receipt,
                )
            }
        };
        if result.compilation_id != Some(relaxation_compilation_id) {
            return self.relaxation_failure(
                FeasibilityRelaxationError::Backend(format!(
                    "relaxation solve returned compilation {:?}, expected {:?}",
                    result.compilation_id, relaxation_compilation_id
                )),
                &receipt,
            );
        }
        let status = match SolveStatus::from_termination(result.termination) {
            Ok(status) => status,
            Err(error) => {
                return self.relaxation_failure(
                    FeasibilityRelaxationError::Backend(error.to_string()),
                    &receipt,
                )
            }
        };
        if status == SolveStatus::Error || status == SolveStatus::Unknown {
            return self.relaxation_failure(
                FeasibilityRelaxationError::Backend(format!(
                    "uninterpretable relaxation termination: {status:?}"
                )),
                &receipt,
            );
        }
        if matches!(status, SolveStatus::Optimal | SolveStatus::Feasible)
            && result.solution.is_none()
        {
            return self.relaxation_failure(
                FeasibilityRelaxationError::Numerical(
                    "repair provider reported a candidate status without a solution".into(),
                ),
                &receipt,
            );
        }
        let (members, total_weighted_violation) = if let Some(candidate) = &result.solution {
            let evidence = match report_members(model, &weighted, &candidate.variable_values) {
                Ok(evidence) => evidence,
                Err(error) => return self.relaxation_failure(error, &receipt),
            };
            if matches!(status, SolveStatus::Optimal | SolveStatus::Feasible)
                && !candidate
                    .objective_value
                    .is_some_and(|objective| objective.is_finite())
            {
                return self.relaxation_failure(
                    FeasibilityRelaxationError::Numerical(
                        "repair provider returned a missing or non-finite objective value".into(),
                    ),
                    &receipt,
                );
            }
            if let Some(objective) = candidate.objective_value {
                if !objective.is_finite()
                    || (objective - evidence.1).abs()
                        > 1e-7 * objective.abs().max(evidence.1.abs()).max(1.0)
                {
                    return self.relaxation_failure(
                        FeasibilityRelaxationError::Numerical(format!(
                            "repair objective {objective} is inconsistent with evidence {}",
                            evidence.1
                        )),
                        &receipt,
                    );
                }
            }
            evidence
        } else {
            (Vec::new(), 0.0)
        };
        let outcome = match status {
            SolveStatus::Optimal => RelaxationOutcome::OptimalRepair,
            SolveStatus::Feasible => match plan.acceptance {
                RelaxationAcceptance::AcceptFeasible => RelaxationOutcome::FeasibleRepair,
                RelaxationAcceptance::RequireOptimal => {
                    RelaxationOutcome::Unknown(unknown_reason(status))
                }
            },
            SolveStatus::Infeasible => RelaxationOutcome::NoRepairFound,
            other => RelaxationOutcome::Unknown(unknown_reason(other)),
        };
        let solution = match normalize_result(
            &result,
            committed,
            None,
            self.backend.name(),
            sync_mode,
            EffectiveSolvePlan::default(),
            model.lineage(),
            model.instance(),
            result.compilation_id,
            Some(overlay_id),
        ) {
            Ok(solution) => solution,
            Err(error) => {
                return self.relaxation_failure(
                    FeasibilityRelaxationError::Backend(error.to_string()),
                    &receipt,
                )
            }
        };
        self.rollback_relaxation(&receipt)?;
        Ok(FeasibilityRelaxationReport {
            outcome,
            solution,
            members,
            total_weighted_violation,
            metadata: crate::solver::relaxation::RelaxationMetadata {
                model_lineage: model.lineage(),
                model_instance: model.instance(),
                model_revision: model.current_revision(),
                base_compilation_id: base_compilation,
                relaxation_compilation_id,
                provider,
                termination: status,
                numerics: crate::solver::relaxation::RelaxationNumerics {
                    objective_value: result.solution.as_ref().and_then(|s| s.objective_value),
                    ..Default::default()
                },
                provider_reason,
            },
        })
    }

    /// Run a repair over a complete P29 report selection and carry its source
    /// declarations into the resulting P30 members.
    ///
    /// The session must already hold the exact compiled base named by the
    /// report. Requiring that identity before entering the lifecycle keeps a
    /// stale P29 selection from causing any backend mutation.
    pub fn solve_feasibility_relaxation_from_p29(
        &mut self,
        model: &mut Model,
        plan: FeasibilityRelaxationPlan,
        report: &crate::solver::infeasibility::InfeasibilityReport,
    ) -> Result<FeasibilityRelaxationReport, FeasibilityRelaxationError> {
        if report.model_instance != model.instance()
            || report.model_revision != model.current_revision()
        {
            return Err(FeasibilityRelaxationError::StaleIdentity(
                "P29 report model identity/revision does not match the repair model".into(),
            ));
        }
        let base = self.compiler.current_compilation().ok_or_else(|| {
            FeasibilityRelaxationError::StaleIdentity(
                "P29 repair requires an already synchronized exact base compilation".into(),
            )
        })?;
        let mapped = crate::solver::relaxation::map_p29_members(
            report,
            model.instance(),
            model.current_revision(),
            base,
        )?;
        let mut plan = plan;
        plan.scope = crate::solver::relaxation::RelaxationScope::Explicit(
            mapped
                .iter()
                .map(|member| member.restriction.clone())
                .collect(),
        );
        let mut result = self.solve_feasibility_relaxation(model, plan)?;
        for member in &mut result.members {
            if let Some(mapped_member) = mapped
                .iter()
                .find(|mapped| mapped.restriction == member.restriction)
            {
                member.source_provenance = mapped_member.source_provenance.clone();
            }
        }
        Ok(result)
    }

    fn rollback_relaxation(
        &mut self,
        receipt: &OverlayApplyReceipt,
    ) -> Result<(), FeasibilityRelaxationError> {
        match self.backend.rollback_overlay(receipt) {
            Ok(OverlayRollbackOutcome::Clean { .. }) => {
                if let Err(error) = self.backend.verify_overlay_clean() {
                    self.force_rebuild_on_next_sync();
                    return Err(FeasibilityRelaxationError::Cleanup {
                        primary: "relaxation result was produced".into(),
                        cleanup: error.to_string(),
                        requires_rebuild: true,
                    });
                }
                Ok(())
            }
            Ok(OverlayRollbackOutcome::RequiresRebuild { reason }) => {
                self.force_rebuild_on_next_sync();
                Err(FeasibilityRelaxationError::Cleanup {
                    primary: "relaxation result was produced".into(),
                    cleanup: reason,
                    requires_rebuild: true,
                })
            }
            Err(error) => {
                self.force_rebuild_on_next_sync();
                Err(FeasibilityRelaxationError::Cleanup {
                    primary: "relaxation result was produced".into(),
                    cleanup: error.to_string(),
                    requires_rebuild: true,
                })
            }
        }
    }

    fn relaxation_failure(
        &mut self,
        primary: FeasibilityRelaxationError,
        receipt: &OverlayApplyReceipt,
    ) -> Result<FeasibilityRelaxationReport, FeasibilityRelaxationError> {
        match self.backend.rollback_overlay(receipt) {
            Ok(OverlayRollbackOutcome::Clean { .. }) => {
                if let Err(error) = self.backend.verify_overlay_clean() {
                    self.force_rebuild_on_next_sync();
                    return Err(FeasibilityRelaxationError::Cleanup {
                        primary: primary.to_string(),
                        cleanup: error.to_string(),
                        requires_rebuild: true,
                    });
                }
                Err(primary)
            }
            Ok(OverlayRollbackOutcome::RequiresRebuild { reason }) => {
                self.force_rebuild_on_next_sync();
                Err(FeasibilityRelaxationError::Cleanup {
                    primary: primary.to_string(),
                    cleanup: reason,
                    requires_rebuild: true,
                })
            }
            Err(error) => {
                self.force_rebuild_on_next_sync();
                Err(FeasibilityRelaxationError::Cleanup {
                    primary: primary.to_string(),
                    cleanup: error.to_string(),
                    requires_rebuild: true,
                })
            }
        }
    }

    /// Resolve the plan's starts/hints against the backend's typed
    /// capabilities and the plan's unsupported-feature policy, producing the
    /// concrete application list and recording every conversion/rejection in
    /// `effective`.
    ///
    /// - A qualified start/hint is queued for native application and recorded
    ///   as an [`AppliedFeature`] (SM-04.5).
    /// - Under [`UnsupportedFeaturePolicy::ConvertStartToTemporaryFixing`], an
    ///   unqualified start is merged into the effective overlay's temporary
    ///   fixings and recorded as a [`PlanAdjustment`] (SM-08.5).
    /// - Under [`UnsupportedFeaturePolicy::ConvertHintToStart`], unqualified
    ///   hints become a [`MipStart`] when `MipStart` is qualified, recorded as
    ///   a [`PlanAdjustment`]; otherwise they are recorded as a
    ///   [`PlanRejection`].
    /// - Under [`UnsupportedFeaturePolicy::Reject`] (the default), any
    ///   unqualified start/hint returns a typed
    ///   [`PlanError::UnsupportedFeature`] error BEFORE any backend mutation
    ///   (SM-08.4).
    fn resolve_plan_features(
        &self,
        model: &Model,
        plan: &SolvePlan,
        effective: &mut EffectiveSolvePlan,
    ) -> Result<ResolvedPlanFeatures, SolveError> {
        let caps = self.compilation_capabilities();
        let mip_start_qualified = caps.supports(BackendFeature::MipStart);
        let partial_mip_start_qualified = caps.supports(BackendFeature::PartialMipStart);
        let multiple_mip_starts_qualified = caps.supports(BackendFeature::MultipleMipStarts);
        let variable_hints_qualified = caps.supports(BackendFeature::VariableHints);

        // Active integer/binary variables, for classifying partial starts.
        let integer_binary: Vec<Variable> = model
            .take_snapshot()
            .map_err(|_| {
                SolveError::Plan(PlanError::UnsupportedFeature {
                    feature: "model snapshot",
                    policy: plan.unsupported,
                })
            })?
            .variables
            .iter()
            .filter(|v| v.active && matches!(v.var_type, VarType::Integer | VarType::Binary))
            .map(|v| v.id)
            .collect();

        let mut starts: Vec<MipStart> = Vec::new();
        let mut hints = VariableHints::default();
        let mut effective_overlay = plan.overlay.clone();
        let mut converted_fixings: Option<BTreeMap<Variable, f64>> = None;

        for (index, start) in plan.mip_starts.iter().enumerate() {
            let key = format!("mip_start[{index}]");
            let partial = integer_binary
                .iter()
                .any(|v| !start.assignment.values.contains_key(v));
            // SM-08.2 (Pass-1 review P1-01) + composition (owner review P1,
            // disposition 5180307378): a start qualifies only when its
            // PRIMITIVE capability (`MipStart`/`PartialMipStart`) is declared
            // AND, for the second+ start, `MultipleMipStarts` is declared.
            // The pinned HiGHS start primitive overwrites the previous
            // incumbent on every call, so an undeclared multiple-start
            // capability would silently drop earlier starts while recording
            // them as applied; and `MultipleMipStarts` alone never qualifies
            // any start — the primitive is never bypassed.
            let primitive = if partial {
                "PartialMipStart"
            } else {
                "MipStart"
            };
            let primitive_qualified = if partial {
                partial_mip_start_qualified
            } else {
                mip_start_qualified
            };
            let multiple = index >= 1;
            let qualified = primitive_qualified && (!multiple || multiple_mip_starts_qualified);
            // Name the missing capability for the error/conversion/rejection
            // messages: the multiple gate when the primitive holds, else the
            // primitive itself.
            let feature = if multiple && !multiple_mip_starts_qualified && primitive_qualified {
                "MultipleMipStarts"
            } else {
                primitive
            };

            if qualified {
                starts.push(start.clone());
                effective.applied_features.push(AppliedFeature {
                    feature: "mip_start".into(),
                    detail: key.clone(),
                });
            } else if plan.unsupported == UnsupportedFeaturePolicy::ConvertStartToTemporaryFixing {
                let fixings = converted_fixings
                    .get_or_insert_with(|| effective_overlay.temporary_fixings.clone());
                for (variable, value) in &start.assignment.values {
                    fixings.insert(*variable, *value);
                }
                effective.adjustments.push(PlanAdjustment {
                    key,
                    requested: "mip_start".into(),
                    applied: "overlay_temporary_fixing".into(),
                    reason: format!(
                        "backend does not qualify {feature}; ConvertStartToTemporaryFixing policy"
                    ),
                });
            } else if plan.unsupported == UnsupportedFeaturePolicy::Reject {
                return Err(SolveError::Plan(PlanError::UnsupportedFeature {
                    feature,
                    policy: plan.unsupported,
                }));
            } else {
                // ConvertHintToStart cannot convert a start; record the
                // rejection (never silent).
                effective.rejections.push(PlanRejection {
                    key,
                    reason: format!(
                        "backend does not qualify {feature} under {:?}; no applicable conversion",
                        plan.unsupported
                    ),
                });
            }
        }

        if !plan.hints.is_empty() {
            if variable_hints_qualified {
                hints = plan.hints.clone();
                effective.applied_features.push(AppliedFeature {
                    feature: "variable_hint".into(),
                    detail: format!("{} hint(s)", plan.hints.len()),
                });
            } else if plan.unsupported == UnsupportedFeaturePolicy::ConvertHintToStart {
                // The generated assignment is classified full/partial like any
                // start, and the SAME composed gates apply (owner review P1,
                // disposition 5180307378): the converted start needs its
                // primitive capability AND, when it would be a second start,
                // `MultipleMipStarts` — a conversion never bypasses either,
                // never silently overwrites the plan's own start.
                let hint_partial = integer_binary
                    .iter()
                    .any(|v| !plan.hints.iter().any(|(variable, _)| variable == v));
                let hint_primitive = if hint_partial {
                    "PartialMipStart"
                } else {
                    "MipStart"
                };
                let hint_primitive_qualified = if hint_partial {
                    partial_mip_start_qualified
                } else {
                    mip_start_qualified
                };
                let becomes_second = !starts.is_empty();
                if hint_primitive_qualified && (!becomes_second || multiple_mip_starts_qualified) {
                    let assignment = PrimalAssignment {
                        lineage: model.lineage(),
                        source_instance: Some(model.instance()),
                        source_revision: Some(model.current_revision()),
                        values: plan
                            .hints
                            .iter()
                            .map(|(variable, hint)| (*variable, hint.value))
                            .collect(),
                    };
                    starts.push(MipStart::new(assignment, RepairPolicy::BackendDefault));
                    effective.adjustments.push(PlanAdjustment {
                        key: "hints".into(),
                        requested: "variable_hints".into(),
                        applied: "mip_start".into(),
                        reason: "backend does not qualify VariableHints; \
                                 ConvertHintToStart policy"
                            .to_string(),
                    });
                    effective.applied_features.push(AppliedFeature {
                        feature: "mip_start".into(),
                        detail: "hints (converted to a MIP start)".into(),
                    });
                } else {
                    let missing = if !hint_primitive_qualified {
                        hint_primitive
                    } else {
                        "MultipleMipStarts"
                    };
                    effective.rejections.push(PlanRejection {
                        key: "hints".into(),
                        reason: format!(
                            "ConvertHintToStart policy but the backend does not qualify {missing} \
                             for the converted {hint_primitive} start; rejection recorded"
                        ),
                    });
                }
            } else if plan.unsupported == UnsupportedFeaturePolicy::Reject {
                return Err(SolveError::Plan(PlanError::UnsupportedFeature {
                    feature: "VariableHints",
                    policy: plan.unsupported,
                }));
            } else {
                // ConvertStartToTemporaryFixing cannot convert hints; record
                // the rejection (never silent).
                effective.rejections.push(PlanRejection {
                    key: "hints".into(),
                    reason: format!(
                        "backend does not qualify VariableHints under {:?}; no applicable \
                         conversion",
                        plan.unsupported
                    ),
                });
            }
        }

        if let Some(fixings) = converted_fixings {
            effective_overlay = SolveOverlay::new(
                fixings,
                effective_overlay.locks,
                effective_overlay.objective_locks,
                effective_overlay.cutoffs,
            )
            .map_err(|_| {
                SolveError::Plan(PlanError::UnsupportedFeature {
                    feature: "overlay identity",
                    policy: plan.unsupported,
                })
            })?;
        }

        Ok(ResolvedPlanFeatures {
            starts,
            hints,
            effective_overlay,
        })
    }

    /// Apply the resolved starts/hints through the backend's `OverlaySession`
    /// warm-start methods. A backend rejection (e.g. a native return-code
    /// failure) maps to a typed [`SolveError`].
    fn apply_warm_starts(
        &mut self,
        resolved: &ResolvedPlanFeatures,
        _effective: &mut EffectiveSolvePlan,
    ) -> Result<(), SolveError> {
        if !resolved.starts.is_empty() {
            self.backend
                .apply_mip_starts(&resolved.starts)
                .map_err(|e| SolveError::from_backend(false, e))?;
        }
        if !resolved.hints.is_empty() {
            self.backend
                .apply_variable_hints(&resolved.hints)
                .map_err(|e| SolveError::from_backend(false, e))?;
        }
        Ok(())
    }

    /// Roll back an applied overlay and verify the base compiled state is
    /// restored. Rollback is ALWAYS attempted on the overlay lifecycle's
    /// failure paths (SM-07.4); an uncertain rollback marks the backend
    /// `RequiresRebuild` (D7, D22).
    fn rollback_and_verify(&mut self, receipt: &OverlayApplyReceipt) -> Result<(), SolveError> {
        match self.backend.rollback_overlay(receipt) {
            Ok(OverlayRollbackOutcome::Clean { .. }) => {
                // Post-rollback verification: C_base must be restored exactly.
                self.backend
                    .verify_overlay_clean()
                    .map_err(SolveError::Rollback)
            }
            Ok(OverlayRollbackOutcome::RequiresRebuild { reason }) => {
                // IN-04: surface the reason the rollback could not be proven
                // clean — the diagnostic value otherwise never reaches the
                // caller or the logs. The overlay solve result is still valid.
                warn!(
                    "overlay rollback could not be proven clean (session marked RequiresRebuild): \
                     {reason}"
                );
                // F3: NEVER trust the backend to have self-marked — DEFENSIVELY
                // force the next solve to rebuild. Resetting the compiler makes
                // `current_compilation()` return `None`, so the next
                // `synchronize_base` takes the snapshot-rebuild branch even when
                // the backend reports `Ready` at the committed revision — the
                // no-sync fast path can never reuse the uncertain overlay state
                // (D7, D22).
                self.force_rebuild_on_next_sync();
                Ok(())
            }
            Err(e) => Err(SolveError::Rollback(e)),
        }
    }

    /// CR-02: defensively force the next solve to rebuild from a fresh
    /// snapshot. Called on overlay APPLY failure, where the backend's native
    /// state may be half-overlaid but the backend may not have self-marked.
    ///
    /// Resetting the compiler's compiled base makes `current_compilation()`
    /// return `None`, so the next `synchronize_base` takes the snapshot-rebuild
    /// branch even when the backend reports `Ready` at the committed revision —
    /// the no-sync fast path can never silently reuse the half-overlaid state.
    fn force_rebuild_on_next_sync(&mut self) {
        self.compiler = CompilationSession::new();
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
            compilation_id: Some(CompilationId::allocate().unwrap()),
            overlay_id: None,
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
            EffectiveSolvePlan::default(),
            lineage,
            instance,
            Some(CompilationId::allocate().unwrap()),
            None,
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
            compilation_id: Some(CompilationId::allocate().unwrap()),
            overlay_id: None,
        };
        let solution = normalize_result(
            &result,
            ModelRevision::ZERO,
            None,
            "ReferenceBackend",
            SynchronizationMode::NoChange,
            EffectiveSolvePlan::default(),
            ModelLineageId::allocate().unwrap(),
            ModelInstanceId::allocate().unwrap(),
            Some(CompilationId::allocate().unwrap()),
            None,
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
            compilation_id: Some(CompilationId::allocate().unwrap()),
            overlay_id: None,
        };
        let solution = normalize_result(
            &result,
            ModelRevision::ZERO,
            None,
            "ReferenceBackend",
            SynchronizationMode::NoChange,
            EffectiveSolvePlan::default(),
            ModelLineageId::allocate().unwrap(),
            ModelInstanceId::allocate().unwrap(),
            Some(CompilationId::allocate().unwrap()),
            None,
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
                compilation_id: Some(CompilationId::allocate().unwrap()),
                overlay_id: None,
            };
            let err = normalize_result(
                &result,
                ModelRevision::ZERO,
                None,
                "ReferenceBackend",
                SynchronizationMode::NoChange,
                EffectiveSolvePlan::default(),
                ModelLineageId::allocate().unwrap(),
                ModelInstanceId::allocate().unwrap(),
                Some(CompilationId::allocate().unwrap()),
                None,
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
                compilation_id: Some(CompilationId::allocate().unwrap()),
                overlay_id: None,
            };
            let solution = normalize_result(
                &result,
                model.current_revision(),
                Some(obj),
                "ReferenceBackend",
                SynchronizationMode::NoChange,
                EffectiveSolvePlan::default(),
                model.lineage(),
                model.instance(),
                Some(CompilationId::allocate().unwrap()),
                None,
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
