//! `BackendSession` trait implementation for HiGHS.
//!
//! Thin delegation layer that routes [`synchronize`], [`solve`], and
//! [`close`] to the projection, solution, and lifecycle modules. Also
//! implements the supplementary traits [`SessionHealth`], [`SolutionView`],
//! [`BackendMetadata`], and [`CallbackSession`].
//!
//! # Architecture
//!
//! ```text
//! BackendSession::synchronize  ->  projection::{rebuild_from_snapshot, apply_delta_batch}
//! BackendSession::solve        ->  solution::{map_termination_status, extract_solution}
//!                               +  negotiate_options (this module)
//! BackendSession::close        ->  lifecycle::Drop
//! ```
//!
//! # Solve request negotiation
//!
//! Every field of [`SolveRequest`] is either applied, adjusted, or rejected
//! with explicit choices. There is no silent best-effort behaviour. Required
//! options (algorithm, time limit, MIP gaps, threads, output) return errors
//! on failure; [`extra_options`](SolveRequest::extra_options) collect
//! rejections and continue.
//!
//! # Threat mitigations
//!
//! - T-11-14: Every `Highs_set*OptionValue` return code checked
//! - T-11-18: Solution invalidated after model mutation (synchronize clears it)
//! - T-11-19: Invalid options return `Err`; caller can inspect rejected list

use std::collections::HashMap;
use std::ffi::{c_void, CString};

use log::{info, warn};

use crate::bindings::*;
use crate::callback::{clear_callback, register_callback, CallbackState};
use crate::compiler::{apply_backend_delta, rebuild_from_backend_snapshot};
use crate::error::{check_highs_status, from_native_status};
use crate::lifecycle::HighsSession;
use crate::solution::{extract_solution, map_termination_status};
use roml::advanced::{
    CompileError, CompiledConstraintId, CompiledEntityRegistry, CompiledVariableId,
};
use roml::compiler::capability::{
    BackendCapabilitySet, BackendFeature, FeatureLimitations, FeatureSupport,
};
use roml::id::{ConId, VarId};
use roml::revision::ModelRevision;
use roml::solver::backend::{BackendCapabilities, BackendError, ErrorCategory, HealthEffect};
use roml::solver::callback::CallbackHandler;
use roml::solver::request::{
    ConfigAdjustment, ConfigRejection, EffectiveConfig, SolveRequest, SolveResult,
};
use roml::solver::session::{
    BackendMetadata, BackendSession, CallbackSession, SessionHealth, SolutionView, SyncReceipt,
    Synchronization,
};
use roml::sync::AdapterHealth;
use roml::LpAlgorithm;

// ── BackendSession ──────────────────────────────────────────────────────────────

impl BackendSession for HighsSession {
    /// Apply a [`Synchronization`] — a compiled rebuild from a
    /// `BackendSnapshot` or a compiled incremental delta batch.
    ///
    /// On success, invalidates any cached solution (T-11-18) and returns the
    /// updated cursor and health.
    ///
    /// P26 (Task 7): the HiGHS session synchronizes through backend IR only.
    /// No canonical `ModelSnapshot`/`DeltaBatch` reaches this session after
    /// the migration (SM-03.2); the `SolverSession` façade compiles canonical
    /// state first. The canonical `Synchronization::Rebuild`/`DeltaBatch`
    /// variants are not handled here.
    fn synchronize(&mut self, sync: Synchronization) -> Result<SyncReceipt, BackendError> {
        match sync {
            Synchronization::CompiledRebuild(snapshot) => {
                let revision = snapshot.source_revision;
                info!(
                    "Rebuilding HiGHS session from compiled backend snapshot at revision {}",
                    revision
                );

                // F5: reject a malformed snapshot (dangling references) before
                // any native mutation — the snapshot has all-pub fields and can
                // bypass builder finalization.
                if let Err(e) = snapshot.validate() {
                    self.cursor.mark_rebuild();
                    return Err(BackendError::new(
                        format!("compiled snapshot failed preflight validation: {e}"),
                        ErrorCategory::InvalidInput,
                        HealthEffect::RequiresRebuild,
                    ));
                }

                let result = rebuild_from_backend_snapshot(
                    self.raw,
                    &snapshot,
                    &mut self.col_map,
                    &mut self.row_map,
                    &mut self.compiled_to_user_variable,
                    &mut self.compiled_to_user_constraint,
                    &mut self.compiled_to_user_objective,
                    self.inf,
                    &mut self.var_bounds,
                    &mut self.con_bounds,
                    &mut self.obj_costs,
                    &mut self.obj_senses,
                    &mut self.obj_offsets,
                    &mut self.active_obj,
                );

                match result {
                    Ok(()) => {
                        self.cursor.mark_ready(revision);
                        // D28/SM-03.9 (WR-1): record the exact compiled id of
                        // the established base so compiled deltas can be
                        // validated against it.
                        self.current_compilation = Some(snapshot.compilation_id);
                        // T-11-18: Invalidate stale solution after model mutation.
                        self.current_solution = None;
                        info!("Rebuild complete, cursor at revision {}", revision);
                    }
                    Err(e) => {
                        match e.health_effect {
                            HealthEffect::Terminal => self.cursor.mark_terminal(),
                            _ => self.cursor.mark_rebuild(),
                        }
                        return Err(e);
                    }
                }
            }

            Synchronization::CompiledDeltaBatch(batch) => {
                info!(
                    "Applying compiled delta r{} -> r{} ({} ops)",
                    batch.from_revision,
                    batch.to_revision,
                    batch.operations.len()
                );

                // D28/SM-03.9 (WR-1): the exact `CompilationId` is the ONLY
                // stale-state authority. Reject a batch whose `from_compilation`
                // does not match the session's current compiled state — never
                // apply against `from_revision` alone (two divergent clones at
                // equal `ModelRevision` have distinct `CompilationId`s). This
                // check runs BEFORE any operation is applied, so a rejected
                // batch never mutates the native model.
                let actual = match self.current_compilation {
                    Some(id) => id,
                    None => {
                        let e = BackendError::new(
                            "compiled delta has no compiled base in the HiGHS session; \
                             establish a compiled base first (D28)",
                            ErrorCategory::InvalidInput,
                            HealthEffect::Recoverable,
                        );
                        self.cursor.mark_rebuild();
                        return Err(e);
                    }
                };
                if actual != batch.from_compilation {
                    let compile_err = CompileError::StaleCompilation {
                        expected: batch.from_compilation,
                        actual,
                    };
                    let e = BackendError::new(
                        format!("{compile_err}"),
                        ErrorCategory::InvalidInput,
                        HealthEffect::Recoverable,
                    );
                    self.cursor.mark_rebuild();
                    return Err(e);
                }

                // Reject a batch whose base revision doesn't match the cursor
                // BEFORE applying any operation, so a failed batch never
                // leaves the HiGHS model partially mutated.
                if batch.from_revision != self.cursor.applied_revision {
                    let e = BackendError::new(
                        format!(
                            "compiled delta from {} does not match cursor at {}",
                            batch.from_revision, self.cursor.applied_revision
                        ),
                        ErrorCategory::InvalidInput,
                        HealthEffect::Recoverable,
                    );
                    self.cursor.mark_rebuild();
                    return Err(e);
                }

                // F5: preflight-validate every op's referenced IDs against the
                // session's held compiled state BEFORE applying any op — a
                // malformed batch never partially mutates the native model.
                let registry = CompiledEntityRegistry {
                    variables: self.col_map.iter().map(|(id, _)| id).collect(),
                    rows: self.row_map.iter().map(|(id, _)| id).collect(),
                    objectives: self.compiled_to_user_objective.keys().copied().collect(),
                };
                if let Err(e) = batch.validate(&registry) {
                    self.cursor.mark_rebuild();
                    return Err(BackendError::new(
                        format!("compiled delta failed preflight validation: {e}"),
                        ErrorCategory::InvalidInput,
                        HealthEffect::Recoverable,
                    ));
                }

                let result = apply_backend_delta(
                    self.raw,
                    &batch,
                    &mut self.col_map,
                    &mut self.row_map,
                    &mut self.compiled_to_user_variable,
                    &mut self.compiled_to_user_constraint,
                    &mut self.compiled_to_user_objective,
                    self.inf,
                    &mut self.var_bounds,
                    &mut self.con_bounds,
                    &mut self.obj_costs,
                    &mut self.obj_senses,
                    &mut self.obj_offsets,
                    &mut self.active_obj,
                );

                match result {
                    Ok(()) => {
                        self.cursor.mark_ready(batch.to_revision);
                        // The exact compiled id of the target state becomes the
                        // session's current compiled state (D28).
                        self.current_compilation = Some(batch.to_compilation);
                        // T-11-18: Invalidate stale solution after model mutation.
                        self.current_solution = None;
                        info!(
                            "Compiled delta applied, cursor at revision {}",
                            batch.to_revision
                        );
                    }
                    Err(e) => {
                        // Map the failure onto the cursor health from the
                        // error's own health effect: a terminal failure (e.g.
                        // license) leaves the session terminally broken, not
                        // merely rebuild-required (PR #21 review round 2).
                        self.cursor.health = health_after_failed_delta(e.health_effect);
                        return Err(e);
                    }
                }
            }

            // The canonical variants are intentionally NOT handled: after the
            // P26 migration the HiGHS session synchronizes through backend IR
            // only (SM-03.2). A canonical sync request is a caller bug; the
            // cursor is marked RequiresRebuild so the caller re-synchronizes
            // through the compiled path.
            Synchronization::Rebuild(_) | Synchronization::DeltaBatch(_) => {
                self.cursor.mark_rebuild();
                return Err(BackendError::new(
                    "canonical synchronization is not supported by the compiled HiGHS session; \
                     compile canonical state to backend IR first (P26 migration, SM-03.8)",
                    ErrorCategory::InvalidInput,
                    HealthEffect::RequiresRebuild,
                ));
            }
        }

        Ok(SyncReceipt {
            cursor: self.cursor.clone(),
            health: self.cursor.health,
        })
    }

    /// Solve the current model with the given [`SolveRequest`].
    ///
    /// Flow:
    /// 1. Negotiate solve options (map or reject each request field).
    /// 2. Register callback handler if one is set (consumed for this solve).
    /// 3. Call [`Highs_run`] and check the return code.
    /// 4. Map termination status (run status + model status).
    /// 5. Extract solution data if available.
    /// 6. Clean up callback state.
    /// 7. Store results and return.
    fn solve(&mut self, request: &SolveRequest) -> Result<SolveResult, BackendError> {
        info!("Solving with HiGHS");

        // F2 (SM-03.9): the result must carry the exact `CompilationId` of the
        // compiled state this session holds (set by `synchronize`). A solve
        // before any compiled synchronization is a caller bug — typed error,
        // never a fabricated id.
        let compilation_id = self.current_compilation.ok_or_else(|| {
            BackendError::new(
                "solve called before any compiled synchronization; the session holds no \
                 compiled state (F2, SM-03.9)",
                ErrorCategory::Internal,
                HealthEffect::RequiresRebuild,
            )
        })?;

        // Step 1: Negotiate solve options.
        let effective_config = negotiate_options(self.raw, request)?;

        // Step 2: Register callback if a handler is set.
        let cb_state: Option<*mut CallbackState> = if let Some(handler) =
            self.callback_handler.take()
        {
            info!("Registering MIP callback handler");
            let col_map_ptr: *const crate::index_map::IndexMap<CompiledVariableId> = &self.col_map;
            let row_map_ptr: *const crate::index_map::IndexMap<CompiledConstraintId> =
                &self.row_map;
            let compiled_to_user_ptr: *const HashMap<CompiledVariableId, VarId> =
                &self.compiled_to_user_variable;
            // SAFETY: self.raw is a valid HiGHS instance handle. col_map,
            // row_map, and compiled_to_user_variable remain valid for the
            // duration of the solve. The returned state pointer is stored
            // in self.callback_state and cleaned up after solve completes.
            let num_col = unsafe { Highs_getNumCol(self.raw) };
            match register_callback(
                self.raw,
                handler,
                col_map_ptr,
                row_map_ptr,
                compiled_to_user_ptr,
                num_col,
            ) {
                Ok(state) => {
                    self.callback_state = Some(state);
                    Some(state)
                }
                Err(e) => {
                    warn!("Failed to register MIP callback: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // Step 3: Run the solve.
        // SAFETY: self.raw is a valid HiGHS instance handle. Highs_run is
        // the main solve entry point. Sync is NOT implemented because
        // calling Highs_run from multiple threads on the same handle is UB.
        let run_ret = unsafe { Highs_run(self.raw) };

        // Check for fatal run errors (negative return code).
        if run_ret < 0 {
            // Clean up callback state before returning error.
            if let Some(state) = cb_state {
                if !self.raw.is_null() {
                    clear_callback(self.raw, state);
                }
                self.callback_state = None;
            }
            return Err(from_native_status(run_ret, "Highs_run"));
        }

        // Step 4: Map termination status.
        let status = map_termination_status(self.raw, run_ret);
        self.last_status = Some(status);
        info!("Solve completed with status: {:?}", status);

        // Step 5: Extract solution data (compiled ids are mapped back to user
        // ids via the origin-derived compiled->user maps, SM-02.5).
        let solution = extract_solution(
            self.raw,
            &status,
            &self.col_map,
            &self.row_map,
            &self.compiled_to_user_variable,
            &self.compiled_to_user_constraint,
        );
        self.current_solution = solution.clone();

        // Step 6: Clean up callback state.
        if let Some(state) = cb_state {
            if !self.raw.is_null() {
                clear_callback(self.raw, state);
            }
            self.callback_state = None;
        }

        Ok(SolveResult {
            effective_configuration: effective_config,
            termination: status,
            solution,
            // F5: a real solve always populates `Some` — the session holds a
            // compiled state (checked above) and its exact id is the only
            // stale-state authority.
            compilation_id: Some(compilation_id),
        })
    }

    /// Close the session, releasing native resources.
    ///
    /// Consumes `self` so that the [`Drop`] impl runs immediately, which
    /// calls [`Highs_destroy`] on the handle after cleaning up callback state.
    fn close(self) -> Result<(), BackendError> {
        // Drop handles all cleanup: callback state and Highs_destroy.
        info!("Closing HiGHS session");
        Ok(())
    }
}

// ── SessionHealth ───────────────────────────────────────────────────────────────

impl SessionHealth for HighsSession {
    fn health(&self) -> roml::sync::AdapterHealth {
        self.cursor.health
    }

    fn revision(&self) -> ModelRevision {
        self.cursor.applied_revision
    }
}

// ── SolutionView ────────────────────────────────────────────────────────────────

impl SolutionView for HighsSession {
    fn value(&self, var: VarId) -> Option<f64> {
        self.current_solution.as_ref().and_then(|sol| {
            sol.variable_values
                .iter()
                .find(|(id, _)| *id == var)
                .map(|(_, v)| *v)
        })
    }

    fn dual(&self, con: ConId) -> Option<f64> {
        self.current_solution.as_ref().and_then(|sol| {
            sol.dual_values
                .as_ref()
                .and_then(|duals| duals.iter().find(|(id, _)| *id == con).map(|(_, v)| *v))
        })
    }

    fn reduced_cost(&self, var: VarId) -> Option<f64> {
        self.current_solution.as_ref().and_then(|sol| {
            sol.reduced_costs
                .as_ref()
                .and_then(|costs| costs.iter().find(|(id, _)| *id == var).map(|(_, v)| *v))
        })
    }

    fn objective_value(&self) -> Option<f64> {
        self.current_solution
            .as_ref()
            .and_then(|sol| sol.objective_value)
    }
}

// ── BackendMetadata ─────────────────────────────────────────────────────────────

impl BackendMetadata for HighsSession {
    fn name(&self) -> &str {
        &self.version_string
    }

    fn capabilities(&self) -> BackendCapabilities {
        // D27 source-compatible compat view derived from the AUTHORITATIVE
        // typed capability set (SM-04.2, F3). This flat view is deliberately
        // non-authoritative: compilation gating and request validation use
        // `typed_capabilities()`. Only NATIVE support maps onto the flat
        // flags (a bridge-supported feature would not claim native support).
        let typed = &self.typed_capabilities;
        BackendCapabilities {
            lp: typed.supports(BackendFeature::Lp),
            mip: typed.supports(BackendFeature::Mip),
            add_variable: typed.supports(BackendFeature::IncrementalRows),
            add_constraint: typed.supports(BackendFeature::IncrementalRows),
            set_coefficient: typed.supports(BackendFeature::IncrementalCoefficients),
            set_bounds: typed.supports(BackendFeature::IncrementalBounds),
            // Flat-only facts with no typed BackendFeature equivalent,
            // preserved for D27 source compatibility:
            set_objective: true,
            delete: true,
            callbacks: true,
            solution: true,
            duals: true,
            reduced_costs: true,
            // H7: Semi-continuous is explicitly rejected — not supported.
            semicontinuous: false,
            semiinteger: false,
            parameter_update: false,
        }
    }

    fn typed_capabilities(&self) -> &BackendCapabilitySet {
        &self.typed_capabilities
    }
}

// ── Typed capability set ────────────────────────────────────────────────────────

/// The M2-native feature surface declared `Native` by HiGHS.
///
/// `Lp`, `Mip`, and the three incremental features map from the M2 flat
/// capability contract (`src/solver/backend.rs`) and are declared native for
/// every supported runtime version. The pinned bundled build is
/// `highs-sys 1.15.0`; the CI system-HiGHS floor is 1.9.0.
const M2_NATIVE_FEATURES: [BackendFeature; 5] = [
    BackendFeature::Lp,
    BackendFeature::Mip,
    BackendFeature::IncrementalBounds,
    BackendFeature::IncrementalRows,
    BackendFeature::IncrementalCoefficients,
];

/// Every M3 feature that P26 does **not** qualify as native for HiGHS.
///
/// These are declared `Unsupported` (SM-04.4): request/compilation paths gate
/// on them and reject or rebuild rather than silently proceeding. The list
/// matches the plan Task 6 bullet verbatim.
const UNQUALIFIED_M3_FEATURES: [BackendFeature; 12] = [
    BackendFeature::MipStart,
    BackendFeature::PartialMipStart,
    BackendFeature::MultipleMipStarts,
    BackendFeature::VariableHints,
    BackendFeature::InitialBasis,
    BackendFeature::Iis,
    BackendFeature::FeasibilityRelaxation,
    BackendFeature::Indicator,
    BackendFeature::Sos1,
    BackendFeature::Sos2,
    BackendFeature::NativePiecewiseLinear,
    BackendFeature::NativeMultiObjective,
];

/// Build the version-aware HiGHS typed capability set from pinned `highs-sys`
/// facts (SM-04.2, SM-04.3).
///
/// The M2-native feature surface is declared `Native` with the runtime version
/// recorded as the declaration's `minimum_version` (the version the support is
/// verified against: bundled `highs-sys 1.15.0`, CI system floor 1.9.0). Every
/// unqualified M3 feature is declared `Unsupported` for P26.
pub fn highs_capability_set(major: i32, minor: i32, patch: i32) -> BackendCapabilitySet {
    let version = format!("{}.{}.{}", major, minor, patch);
    let mut set = BackendCapabilitySet::new();

    for feature in M2_NATIVE_FEATURES {
        set.set(
            feature,
            FeatureSupport::native(FeatureLimitations {
                minimum_version: Some(version.clone()),
                notes: vec![
                    "declared against the runtime HiGHS version (pinned highs-sys 1.15.0; CI system floor 1.9.0)".to_string(),
                ],
                ..FeatureLimitations::default()
            }),
        );
    }

    for feature in UNQUALIFIED_M3_FEATURES {
        set.set(
            feature,
            FeatureSupport::unsupported(FeatureLimitations {
                notes: vec!["P26 does not qualify native support for this M3 feature".to_string()],
                ..FeatureLimitations::default()
            }),
        );
    }

    set
}

// ── CallbackSession ─────────────────────────────────────────────────────────────

impl CallbackSession for HighsSession {
    fn set_callback_handler(
        &mut self,
        handler: Box<dyn CallbackHandler>,
    ) -> Result<(), BackendError> {
        self.callback_handler = Some(handler);
        Ok(())
    }

    fn clear_callback_handler(&mut self) -> Result<(), BackendError> {
        self.callback_handler = None;
        if let Some(state) = self.callback_state.take() {
            if !self.raw.is_null() {
                clear_callback(self.raw, state);
            }
        }
        Ok(())
    }
}

// ── Solve Request Negotiation ───────────────────────────────────────────────────

/// Map a [`SolveRequest`] to HiGHS options, returning the [`EffectiveConfig`].
///
/// Every request field is explicitly applied or rejected — no silent
/// best-effort (M1R-H3, M1R-H5). Core options (algorithm, time limit, MIP
/// gaps, threads, output) return errors on failure. Extra options collect
/// rejections and continue.
///
/// # Safety
///
/// `raw` must be a valid HiGHS instance handle.
///
/// # CString safety (T-11-15)
///
/// All option keys and values are converted via [`CString::new`], which
/// returns an error if the string contains interior null bytes. This
/// prevents CString panics at the FFI boundary.
fn negotiate_options(
    raw: *mut c_void,
    request: &SolveRequest,
) -> Result<EffectiveConfig, BackendError> {
    let mut effective = EffectiveConfig::default();

    // ── Reset to HiGHS defaults ─────────────────────────────────────────────
    // HiGHS options persist on the native session between solves. A request
    // is self-contained: ALL options are reset to their HiGHS defaults before
    // the request's explicit values are applied, so a default solve after a
    // configured solve_with does not silently retain the previous time limit,
    // gaps, threads, output flag, random seed, algorithm, or arbitrary
    // backend_option(key, value) entries — the latter have no roml-level
    // default to reset individually, so the session-wide reset is required
    // (PR #21 review round 2).
    // SAFETY: raw is a valid HiGHS instance handle.
    let reset_ret = unsafe { Highs_resetOptions(raw) };
    if reset_ret != STATUS_OK {
        return Err(BackendError::new(
            "Highs_resetOptions failed to restore defaults",
            ErrorCategory::Internal,
            HealthEffect::Recoverable,
        ));
    }

    // ── lp_algorithm ────────────────────────────────────────────────────────
    if let Some(algo) = &request.lp_algorithm {
        match algo {
            LpAlgorithm::Automatic => {
                effective.lp_algorithm = Some(LpAlgorithm::Automatic);
            }
            LpAlgorithm::DualSimplex => {
                set_option(raw, "solver", "simplex")?;
                set_option(raw, "simplex_strategy", "1")?;
                effective.lp_algorithm = Some(LpAlgorithm::DualSimplex);
            }
            LpAlgorithm::Primal => {
                // HiGHS simplex_strategy: 1 = dual, 2 = primal. A Primal
                // request must actually run primal simplex, not silently fall
                // back to dual (which is what the HiGHS default does).
                set_option(raw, "solver", "simplex")?;
                set_option(raw, "simplex_strategy", "2")?;
                effective.lp_algorithm = Some(LpAlgorithm::Primal);
            }
            LpAlgorithm::Dual => {
                set_option(raw, "solver", "simplex")?;
                set_option(raw, "simplex_strategy", "1")?;
                effective.lp_algorithm = Some(LpAlgorithm::Dual);
            }
            LpAlgorithm::Barrier => {
                set_option(raw, "solver", "ipm")?;
                effective.lp_algorithm = Some(LpAlgorithm::Barrier);
            }
        }
    }

    // ── time_limit_secs ─────────────────────────────────────────────────────
    if let Some(t) = request.time_limit_secs {
        set_option(raw, "time_limit", &t.to_string())?;
        effective.time_limit_secs = Some(t);
    }

    // ── mip_rel_gap ─────────────────────────────────────────────────────────
    if let Some(g) = request.mip_rel_gap {
        set_option(raw, "mip_rel_gap", &g.to_string())?;
        effective.mip_rel_gap = Some(g);
    }

    // ── mip_abs_gap ─────────────────────────────────────────────────────────
    if let Some(g) = request.mip_abs_gap {
        match set_option(raw, "mip_abs_gap", &g.to_string()) {
            Ok(()) => {
                effective.adjustments.push(ConfigAdjustment {
                    key: "mip_abs_gap".into(),
                    requested: g.to_string(),
                    applied: g.to_string(),
                    reason: "set via Highs_setStringOptionValue".into(),
                });
            }
            Err(e) => {
                effective.rejections.push(ConfigRejection {
                    key: "mip_abs_gap".into(),
                    reason: format!("HiGHS rejected mip_abs_gap: {}", e),
                });
            }
        }
    }

    // ── threads ─────────────────────────────────────────────────────────────
    if let Some(t) = request.threads {
        set_option(raw, "threads", &t.to_string())?;
        effective.threads = Some(t);
    }

    // ── enable_output ───────────────────────────────────────────────────────
    if let Some(enabled) = request.enable_output {
        let val = if enabled { "true" } else { "false" };
        set_option(raw, "output_flag", val)?;
        effective.enable_output = Some(enabled);
    }

    // ── random_seed ─────────────────────────────────────────────────────────
    if let Some(s) = request.random_seed {
        match set_option(raw, "random_seed", &s.to_string()) {
            Ok(()) => {
                effective.adjustments.push(ConfigAdjustment {
                    key: "random_seed".into(),
                    requested: s.to_string(),
                    applied: s.to_string(),
                    reason: "set via Highs_setStringOptionValue".into(),
                });
            }
            Err(e) => {
                effective.rejections.push(ConfigRejection {
                    key: "random_seed".into(),
                    reason: format!("HiGHS rejected random_seed: {}", e),
                });
            }
        }
    }

    // ── extra_options ───────────────────────────────────────────────────────
    for (key, value) in &request.extra_options {
        let key_c = match CString::new(key.as_str()) {
            Ok(c) => c,
            Err(e) => {
                warn!("extra_options key contains null byte: {}", e);
                effective.rejections.push(ConfigRejection {
                    key: key.clone(),
                    reason: format!("key contains null byte at position {}", e.nul_position()),
                });
                continue;
            }
        };
        let value_c = match CString::new(value.as_str()) {
            Ok(c) => c,
            Err(e) => {
                warn!(
                    "extra_options value for '{}' contains null byte: {}",
                    key, e
                );
                effective.rejections.push(ConfigRejection {
                    key: key.clone(),
                    reason: format!("value contains null byte at position {}", e.nul_position()),
                });
                continue;
            }
        };

        // Try Highs_setStringOptionValue first.
        // SAFETY: raw is a valid HiGHS handle. CStrings are valid,
        // null-terminated strings. Return code is checked immediately.
        let ret = unsafe { Highs_setStringOptionValue(raw, key_c.as_ptr(), value_c.as_ptr()) };

        if ret == STATUS_OK {
            // Successful extra options are recorded in the effective
            // configuration so metadata reflects what the backend applied
            // (PR #21 review round 2).
            effective.adjustments.push(ConfigAdjustment {
                key: key.clone(),
                requested: value.clone(),
                applied: value.clone(),
                reason: "set via Highs_setStringOptionValue".into(),
            });
        } else {
            // Fallback: try Highs_setOptionValue. Some HiGHS options use
            // a different format parser in each variant.
            // SAFETY: same invariants as above.
            let ret2 = unsafe { Highs_setOptionValue(raw, key_c.as_ptr(), value_c.as_ptr()) };
            if ret2 != STATUS_OK {
                effective.rejections.push(ConfigRejection {
                    key: key.clone(),
                    reason: format!(
                        "HiGHS rejected option '{}' (string API: {}, option API: {})",
                        key, ret, ret2
                    ),
                });
            } else {
                effective.adjustments.push(ConfigAdjustment {
                    key: key.clone(),
                    requested: value.clone(),
                    applied: value.clone(),
                    reason: "set via Highs_setOptionValue".into(),
                });
            }
        }
    }

    Ok(effective)
}

/// Set a single HiGHS string option value via [`Highs_setStringOptionValue`].
///
/// Returns a [`BackendError`] on failure. Option keys and values are
/// converted to [`CString`] and may fail with [`ErrorCategory::InvalidInput`]
/// if they contain interior null bytes (T-11-15).
///
/// # Safety
///
/// `raw` must be a valid HiGHS instance handle.
/// Map a failed delta's error health effect onto the session cursor.
///
/// A terminal failure (e.g. license) leaves the session terminally broken;
/// anything else demands a snapshot rebuild. This keeps the session's
/// reported health consistent with the error it returned, so subsequent
/// attempts treat the session as terminal rather than rebuildable.
fn health_after_failed_delta(effect: HealthEffect) -> AdapterHealth {
    match effect {
        HealthEffect::Terminal => AdapterHealth::Terminal,
        _ => AdapterHealth::RequiresRebuild,
    }
}

fn set_option(raw: *mut c_void, key: &str, value: &str) -> Result<(), BackendError> {
    let key_c = CString::new(key).map_err(|e| {
        BackendError::new(
            format!(
                "option key '{}' contains null byte at position {}",
                key,
                e.nul_position()
            ),
            ErrorCategory::InvalidInput,
            HealthEffect::Recoverable,
        )
    })?;

    let value_c = CString::new(value).map_err(|e| {
        BackendError::new(
            format!(
                "option value for '{}' contains null byte at position {}",
                key,
                e.nul_position()
            ),
            ErrorCategory::InvalidInput,
            HealthEffect::Recoverable,
        )
    })?;

    // SAFETY: `raw` is a valid HiGHS instance handle. `key_c` and `value_c`
    // are valid, null-terminated C strings. Return code is checked.
    let ret = unsafe { Highs_setStringOptionValue(raw, key_c.as_ptr(), value_c.as_ptr()) };

    check_highs_status(ret, raw, &format!("Highs_setStringOptionValue({})", key))
}

// ── Tests ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use roml::advanced::{
        BackendDeltaBatch, BackendOp, BackendSnapshot, BackendSnapshotBuilder, CompilationId,
        CompilationPolicy, CompilationSession, CompiledLinearRow, CompiledObjectiveId,
        CompiledObjectiveLevel, CompiledObjectivePolicy, CompiledVariable, CompiledVariableId,
        CompiledWeightedObjective, EntityOrigin, OriginMap,
    };
    use roml::compiler::capability::SupportLevel;
    use roml::delta::{DeltaBatch, ModelOp};
    use roml::id::Generation;
    use roml::model::{continuous, Bounds, ConstraintBounds, VarType};
    use roml::snapshot::ModelSnapshot;
    use roml::{ConstraintExprExt, Model};

    /// A full typed capability set for the compiled-path tests (SM-04.4: an
    /// unqualified feature is rejected, never silently ignored).
    fn full_caps() -> BackendCapabilitySet {
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

    /// WR-1 (D28/SM-03.9): the exact `CompilationId` is the only stale-state
    /// authority. A `CompiledDeltaBatch` whose `from_compilation` does not
    /// match the session's current compiled state is rejected with a typed
    /// error BEFORE any op is applied — never applied against `from_revision`
    /// alone.
    #[test]
    fn compiled_delta_rejects_stale_from_compilation() {
        let caps = full_caps();
        let policy = CompilationPolicy::Auto;

        // A committed model at revision r1.
        let mut model = Model::new();
        let x = model.add_variable(continuous()).unwrap();
        model.add_constraint((x).le(10.0)).unwrap();
        model.maximize(x).unwrap();
        model.commit().unwrap();
        let snapshot = model.take_snapshot().unwrap();
        let r1 = snapshot.revision;

        // Establish the model snapshot as the session's compiled base.
        let mut session_a = CompilationSession::new();
        let base: BackendSnapshot = session_a
            .compile_snapshot(model.instance(), &snapshot, &policy, &caps)
            .expect("snapshot must compile");
        let mut highs = HighsSession::try_new().expect("HiGHS should be available");
        highs
            .synchronize(Synchronization::CompiledRebuild(base.clone()))
            .expect("base rebuild must succeed");

        // A SECOND, unrelated compilation (an empty base at the same revision)
        // compiles a delta whose `from_compilation` does not match the base the
        // session holds. `from_revision` matches the cursor, so only the exact
        // `CompilationId` check can catch the stale state.
        let mut session_b = CompilationSession::new();
        let empty: BackendSnapshot = session_b
            .compile_snapshot(model.instance(), &ModelSnapshot::empty(r1), &policy, &caps)
            .expect("empty snapshot must compile");
        let r2 = r1.next().unwrap();
        let batch = DeltaBatch::new(
            r1,
            r2,
            vec![ModelOp::AddVariable {
                var: roml::id::VarId::new(99, roml::id::Generation::new()),
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            }],
        )
        .unwrap();
        let delta = session_b
            .compile_delta(
                &batch,
                empty.compilation_id,
                model.instance(),
                &policy,
                &caps,
            )
            .expect("delta must compile against the other base");
        assert_ne!(
            delta.from_compilation, base.compilation_id,
            "the delta must be compiled against a DIFFERENT compiled base"
        );

        let err = match highs.synchronize(Synchronization::CompiledDeltaBatch(delta)) {
            Ok(_) => panic!("stale from_compilation must be rejected"),
            Err(e) => e,
        };
        assert_eq!(
            err.category,
            ErrorCategory::InvalidInput,
            "a stale compiled delta is invalid input"
        );
        assert!(
            err.message.contains("compilation"),
            "the error must name the stale compilation, got: {}",
            err.message
        );
    }

    #[test]
    fn highs_capability_set_declares_m2_native_and_m3_unsupported() {
        let set = highs_capability_set(1, 15, 0);

        // M2-native features are declared Native.
        for feature in M2_NATIVE_FEATURES {
            assert!(
                set.supports(feature),
                "M2-native feature {:?} must be declared Native",
                feature
            );
        }

        // Unqualified M3 features are declared Unsupported (SM-04.4).
        for feature in UNQUALIFIED_M3_FEATURES {
            assert!(
                !set.supports(feature),
                "M3 feature {:?} must be declared Unsupported in P26",
                feature
            );
            let support = set
                .support(feature)
                .unwrap_or_else(|| panic!("feature {:?} must be present", feature));
            assert_eq!(
                support.level,
                SupportLevel::Unsupported,
                "feature {:?} level",
                feature
            );
        }
    }

    #[test]
    fn highs_capability_set_records_minimum_version() {
        let set = highs_capability_set(1, 15, 0);
        let lp = set
            .support(BackendFeature::Lp)
            .expect("Lp must be declared");
        assert_eq!(lp.level, SupportLevel::Native);
        assert_eq!(
            lp.limitations.minimum_version.as_deref(),
            Some("1.15.0"),
            "minimum_version records the runtime version the declaration is verified against"
        );
    }

    #[test]
    fn health_after_failed_delta_maps_terminal_and_rebuild() {
        assert_eq!(
            health_after_failed_delta(HealthEffect::Terminal),
            AdapterHealth::Terminal,
            "terminal failure must leave the session terminal"
        );
        assert_eq!(
            health_after_failed_delta(HealthEffect::Recoverable),
            AdapterHealth::RequiresRebuild
        );
        assert_eq!(
            health_after_failed_delta(HealthEffect::RequiresRebuild),
            AdapterHealth::RequiresRebuild
        );
        assert_eq!(
            health_after_failed_delta(HealthEffect::None),
            AdapterHealth::RequiresRebuild
        );
    }

    #[test]
    fn negotiate_options_empty_request() {
        let request = SolveRequest::new();
        // SAFETY: negotiation resets every known option to its HiGHS default
        // before applying the request, so even an empty request touches the
        // native session — a real (non-null) instance is required.
        let raw = unsafe { Highs_create() };
        assert!(!raw.is_null(), "HiGHS instance must be created");
        let result = negotiate_options(raw, &request);
        unsafe { Highs_destroy(raw) };
        assert!(result.is_ok());
        let config = result.unwrap();
        assert!(config.lp_algorithm.is_none());
        assert!(config.time_limit_secs.is_none());
        assert!(config.mip_rel_gap.is_none());
        assert!(config.threads.is_none());
        assert!(config.enable_output.is_none());
        assert!(config.adjustments.is_empty());
        assert!(config.rejections.is_empty());
    }

    #[test]
    fn set_option_handles_null_bytes() {
        let result = set_option(std::ptr::null_mut(), "valid_key", "value_with\0null");
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e.category, ErrorCategory::InvalidInput);
        }
    }

    #[test]
    fn set_option_handles_null_bytes_in_key() {
        let result = set_option(std::ptr::null_mut(), "key\0with_null", "value");
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e.category, ErrorCategory::InvalidInput);
        }
    }

    #[test]
    fn set_option_rejects_native_failure_with_internal_error() {
        // A valid key with an invalid value makes Highs_setStringOptionValue
        // return a non-OK status, which check_highs_status converts to an
        // Internal/Recoverable BackendError carrying the native code.
        let session = HighsSession::try_new().expect("HiGHS should be available");
        let raw = session.raw_ptr();
        let result = set_option(raw, "output_flag", "not_a_bool");
        let err = result.expect_err("HiGHS must reject an invalid boolean option value");
        assert_eq!(err.category, ErrorCategory::Internal);
        assert_eq!(err.health_effect, HealthEffect::Recoverable);
        assert!(
            err.message
                .contains("Highs_setStringOptionValue(output_flag)"),
            "unexpected message: {}",
            err.message
        );
    }

    // ── F4: typed rejection of unsupported policies and future ops ───────────

    /// A committed one-variable maximize-x model's compiled base plus a
    /// trivial chained delta, with the HiGHS session holding the base.
    fn weighted_lexicographic_fixture() -> (Model, HighsSession, BackendDeltaBatch, CompilationId) {
        let caps = full_caps();
        let policy = CompilationPolicy::Auto;
        let mut model = Model::new();
        let x = model.add_variable(continuous()).unwrap();
        model.maximize(x).unwrap();
        model.commit().unwrap();
        let snapshot = model.take_snapshot().unwrap();

        let mut session = CompilationSession::new();
        let base = session
            .compile_snapshot(model.instance(), &snapshot, &policy, &caps)
            .expect("snapshot must compile");
        let mut highs = HighsSession::try_new().expect("HiGHS should be available");
        highs
            .synchronize(Synchronization::CompiledRebuild(base.clone()))
            .expect("base rebuild must succeed");

        let r_next = snapshot.revision.next().unwrap();
        let batch = DeltaBatch::new(
            snapshot.revision,
            r_next,
            vec![ModelOp::SetActiveObjective { obj: None }],
        )
        .unwrap();
        let delta = session
            .compile_delta(
                &batch,
                base.compilation_id,
                model.instance(),
                &policy,
                &caps,
            )
            .expect("delta must compile");
        (model, highs, delta, base.compilation_id)
    }

    /// F4: a `CompiledObjectivePolicy::Weighted` policy (a P31 construct) must
    /// be REJECTED with a typed error by the HiGHS projection — never silently
    /// treated as no-active-objective — and the session's compiled state must
    /// not advance.
    #[test]
    fn compiled_delta_with_weighted_policy_is_rejected_without_advancing() {
        let (_model, mut highs, mut delta, base_id) = weighted_lexicographic_fixture();
        delta.operations = vec![BackendOp::SetObjectivePolicy(
            CompiledObjectivePolicy::Weighted(vec![CompiledWeightedObjective {
                objective: CompiledObjectiveId(0),
                weight: 1.0,
            }]),
        )];

        let err = match highs.synchronize(Synchronization::CompiledDeltaBatch(delta)) {
            Ok(_) => {
                panic!("a weighted policy must be rejected, not silently treated as no-active-objective")
            }
            Err(e) => e,
        };
        assert_eq!(
            err.category,
            ErrorCategory::Unsupported,
            "a weighted policy is an unsupported feature for the P26 projection"
        );
        assert_eq!(
            highs.current_compilation,
            Some(base_id),
            "a rejected batch must not advance the session's compiled state"
        );
        assert_eq!(
            highs.cursor.health,
            AdapterHealth::RequiresRebuild,
            "a rejected batch leaves the session rebuild-required"
        );
    }

    /// F4: a `CompiledObjectivePolicy::Lexicographic` policy is rejected the
    /// same way — typed error, no silent success, no state advance.
    #[test]
    fn compiled_delta_with_lexicographic_policy_is_rejected_without_advancing() {
        let (_model, mut highs, mut delta, base_id) = weighted_lexicographic_fixture();
        delta.operations = vec![BackendOp::SetObjectivePolicy(
            CompiledObjectivePolicy::Lexicographic(vec![CompiledObjectiveLevel {
                objective: CompiledObjectiveId(0),
                absolute_tolerance: 0.0,
                relative_tolerance: 0.0,
            }]),
        )];

        let err = match highs.synchronize(Synchronization::CompiledDeltaBatch(delta)) {
            Ok(_) => {
                panic!("a lexicographic policy must be rejected, not silently treated as no-active-objective")
            }
            Err(e) => e,
        };
        assert_eq!(err.category, ErrorCategory::Unsupported);
        assert_eq!(
            highs.current_compilation,
            Some(base_id),
            "a rejected batch must not advance the session's compiled state"
        );
    }

    /// F4: a full snapshot rebuild whose objective policy is Weighted is also
    /// rejected with a typed error (the projection must never silently treat a
    /// weighted/lexicographic policy as no-active-objective).
    #[test]
    fn rebuild_from_snapshot_with_weighted_policy_is_rejected() {
        let caps = full_caps();
        let policy = CompilationPolicy::Auto;
        let mut model = Model::new();
        let x = model.add_variable(continuous()).unwrap();
        model.maximize(x).unwrap();
        model.commit().unwrap();
        let snapshot = model.take_snapshot().unwrap();
        let mut session = CompilationSession::new();
        let mut compiled = session
            .compile_snapshot(model.instance(), &snapshot, &policy, &caps)
            .expect("snapshot must compile");
        compiled.objective_policy =
            CompiledObjectivePolicy::Weighted(vec![CompiledWeightedObjective {
                objective: CompiledObjectiveId(0),
                weight: 1.0,
            }]);

        let mut highs = HighsSession::try_new().expect("HiGHS should be available");
        let err = match highs.synchronize(Synchronization::CompiledRebuild(compiled)) {
            Ok(_) => panic!("a weighted-policy snapshot must be rejected"),
            Err(e) => e,
        };
        assert_eq!(err.category, ErrorCategory::Unsupported);
        assert_eq!(
            highs.current_compilation, None,
            "a rejected rebuild must not establish a compiled state"
        );
    }

    // ── F3: unsupported policies rejected BEFORE any native mutation ────────

    /// A maximize-x model (x on [0,1]) compiled to a base snapshot, with the
    /// HiGHS session holding the base. Solving reports objective 1.0.
    fn cost_probe_fixture() -> (CompilationSession, BackendSnapshot, HighsSession) {
        let caps = full_caps();
        let policy = CompilationPolicy::Auto;
        let mut model = Model::new();
        let x = model.add_variable(continuous().bounds(0.0, 1.0)).unwrap();
        model.maximize(x).unwrap();
        model.commit().unwrap();
        let snapshot = model.take_snapshot().unwrap();

        let mut session = CompilationSession::new();
        let compiled = session
            .compile_snapshot(model.instance(), &snapshot, &policy, &caps)
            .expect("snapshot must compile");
        let mut highs = HighsSession::try_new().expect("HiGHS should be available");
        highs
            .synchronize(Synchronization::CompiledRebuild(compiled.clone()))
            .expect("base rebuild must succeed");
        (session, compiled, highs)
    }

    /// F3: a snapshot rebuild carrying a Weighted policy is rejected BEFORE
    /// `Highs_clear` — the native model is untouched (columns intact and the
    /// objective cost still 1.0 on a subsequent solve).
    #[test]
    fn weighted_policy_rebuild_leaves_native_model_unchanged() {
        let (_session, mut compiled, mut highs) = cost_probe_fixture();
        let request = SolveRequest {
            enable_output: Some(false),
            ..SolveRequest::new()
        };
        let baseline = highs.solve(&request).expect("base solve must succeed");
        assert_eq!(
            baseline.solution.as_ref().and_then(|s| s.objective_value),
            Some(1.0)
        );
        // SAFETY: highs.raw is a valid HiGHS instance handle.
        let cols_before = unsafe { Highs_getNumCol(highs.raw) };

        compiled.objective_policy =
            CompiledObjectivePolicy::Weighted(vec![CompiledWeightedObjective {
                objective: CompiledObjectiveId(0),
                weight: 1.0,
            }]);
        let err = match highs.synchronize(Synchronization::CompiledRebuild(compiled)) {
            Ok(_) => panic!("a weighted-policy rebuild must be rejected"),
            Err(e) => e,
        };
        assert_eq!(err.category, ErrorCategory::Unsupported);
        // SAFETY: highs.raw is a valid HiGHS instance handle.
        let cols_after = unsafe { Highs_getNumCol(highs.raw) };
        assert_eq!(
            cols_after, cols_before,
            "a rejected weighted rebuild must not clear the native model"
        );

        let after = highs
            .solve(&request)
            .expect("solve after the rejected rebuild must succeed");
        assert_eq!(
            after.solution.as_ref().and_then(|s| s.objective_value),
            Some(1.0),
            "a rejected weighted rebuild must not clear native objective costs"
        );
    }

    /// F3: a delta batch carrying a Weighted policy is rejected BEFORE the
    /// cost clearing its `SetObjectivePolicy` arm would perform — the native
    /// objective cost is intact on a subsequent solve.
    #[test]
    fn weighted_policy_delta_leaves_objective_costs_intact() {
        let (mut session, compiled, mut highs) = cost_probe_fixture();
        let request = SolveRequest {
            enable_output: Some(false),
            ..SolveRequest::new()
        };
        let baseline = highs.solve(&request).expect("base solve must succeed");
        assert_eq!(
            baseline.solution.as_ref().and_then(|s| s.objective_value),
            Some(1.0)
        );

        let r_next = compiled.source_revision.next().unwrap();
        let batch = DeltaBatch::new(
            compiled.source_revision,
            r_next,
            vec![ModelOp::SetActiveObjective { obj: None }],
        )
        .unwrap();
        let mut delta = session
            .compile_delta(
                &batch,
                compiled.compilation_id,
                compiled.source_instance,
                &CompilationPolicy::Auto,
                &full_caps(),
            )
            .expect("delta must compile");
        delta.operations = vec![BackendOp::SetObjectivePolicy(
            CompiledObjectivePolicy::Weighted(vec![CompiledWeightedObjective {
                objective: CompiledObjectiveId(0),
                weight: 1.0,
            }]),
        )];

        let err = match highs.synchronize(Synchronization::CompiledDeltaBatch(delta)) {
            Ok(_) => panic!("a weighted-policy delta must be rejected"),
            Err(e) => e,
        };
        assert_eq!(err.category, ErrorCategory::Unsupported);

        let after = highs
            .solve(&request)
            .expect("solve after the rejected delta must succeed");
        assert_eq!(
            after.solution.as_ref().and_then(|s| s.objective_value),
            Some(1.0),
            "a rejected weighted-policy delta must not clear native objective costs"
        );
    }

    // ── F5: preflight validation of snapshots and deltas ─────────────────────

    /// F5: a compiled delta op referencing a compiled variable not present in
    /// the session's held state is rejected by the preflight validator with a
    /// typed `InvalidInput` error BEFORE any native mutation, and the session's
    /// compiled state does not advance.
    #[test]
    fn compiled_delta_with_unknown_variable_reference_is_rejected() {
        let (_model, mut highs, mut delta, base_id) = weighted_lexicographic_fixture();
        delta.operations = vec![BackendOp::SetVariableBounds {
            variable: CompiledVariableId(999),
            bounds: Bounds::new(0.0, 1.0),
        }];

        let err = match highs.synchronize(Synchronization::CompiledDeltaBatch(delta)) {
            Ok(_) => {
                panic!(
                    "an op referencing an unknown compiled variable must be rejected, not skipped"
                )
            }
            Err(e) => e,
        };
        assert_eq!(
            err.category,
            ErrorCategory::InvalidInput,
            "a dangling reference is invalid input"
        );
        assert_eq!(
            highs.current_compilation,
            Some(base_id),
            "a rejected batch must not advance the session's compiled state"
        );
        assert_eq!(
            highs.cursor.health,
            AdapterHealth::RequiresRebuild,
            "a rejected batch leaves the session rebuild-required"
        );
    }

    // ── Envelope validation (fifth review) ───────────────────────────────────

    /// Fifth review: a compiled delta batch whose `from_compilation ==
    /// to_compilation` is rejected with a typed `InvalidInput` error BEFORE any
    /// native mutation, and the session's compiled state (exact id) and cursor
    /// revision do not advance.
    #[test]
    fn compiled_delta_rejects_identical_from_to_compilation_without_advancing() {
        let (_model, mut highs, delta, base_id) = weighted_lexicographic_fixture();
        let base_rev = delta.from_revision;
        let mut delta = delta;
        // Malform the envelope: the batch claims to produce the SAME exact
        // compiled state it starts from (retaining the old exact identity, D28)
        // while still carrying mutations. `from_compilation` stays equal to the
        // base so the stale check passes and the envelope check fires.
        delta.to_compilation = base_id;

        let err = match highs.synchronize(Synchronization::CompiledDeltaBatch(delta)) {
            Ok(_) => panic!("an identical from/to compilation envelope must be rejected"),
            Err(e) => e,
        };
        assert_eq!(
            err.category,
            ErrorCategory::InvalidInput,
            "a malformed envelope is invalid input"
        );
        assert_eq!(
            highs.current_compilation,
            Some(base_id),
            "a rejected envelope must not advance the session's compiled state"
        );
        assert_eq!(
            highs.cursor.applied_revision, base_rev,
            "a rejected envelope must not advance the cursor revision"
        );
    }

    /// Fifth review: a compiled delta batch whose `from_revision >=
    /// to_revision` is rejected with a typed `InvalidInput` error BEFORE any
    /// native mutation, and the session's compiled state and cursor revision do
    /// not advance.
    #[test]
    fn compiled_delta_rejects_non_advancing_revision_without_advancing() {
        let (_model, mut highs, delta, base_id) = weighted_lexicographic_fixture();
        let base_rev = delta.from_revision;
        let mut delta = delta;
        // Malform the envelope: the batch does not advance the canonical model
        // revision (from >= to). from_compilation still matches the base so the
        // stale check passes and the envelope check fires.
        delta.to_revision = delta.from_revision;

        let err = match highs.synchronize(Synchronization::CompiledDeltaBatch(delta)) {
            Ok(_) => panic!("a non-advancing revision envelope must be rejected"),
            Err(e) => e,
        };
        assert_eq!(
            err.category,
            ErrorCategory::InvalidInput,
            "a malformed envelope is invalid input"
        );
        assert_eq!(
            highs.current_compilation,
            Some(base_id),
            "a rejected envelope must not advance the session's compiled state"
        );
        assert_eq!(
            highs.cursor.applied_revision, base_rev,
            "a rejected envelope must not advance the cursor revision"
        );
    }

    /// Fifth review: a VALID envelope (advancing compilation id + revision)
    /// still applies and advances both the session's compiled state's exact id
    /// and its cursor revision.
    #[test]
    fn compiled_delta_valid_envelope_still_applies_and_advances() {
        let (_model, mut highs, delta, _base_id) = weighted_lexicographic_fixture();
        let to_compilation = delta.to_compilation;
        let to_revision = delta.to_revision;

        let result = highs.synchronize(Synchronization::CompiledDeltaBatch(delta));
        let receipt = result.expect("a valid envelope must apply");
        assert_eq!(
            highs.current_compilation,
            Some(to_compilation),
            "a valid envelope advances the session's compiled state"
        );
        assert_eq!(
            receipt.cursor.applied_revision, to_revision,
            "a valid envelope advances the cursor revision"
        );
        assert_eq!(
            highs.cursor.applied_revision, to_revision,
            "a valid envelope advances the session's held cursor revision"
        );
        assert_eq!(
            highs.cursor.health,
            AdapterHealth::Ready,
            "a valid envelope leaves the session ready"
        );
    }

    /// F5: a snapshot whose row coefficient references a compiled variable
    /// absent from the snapshot is rejected by the preflight validator when
    /// rebuilding — no silent omission of the coefficient.
    #[test]
    fn compiled_rebuild_with_unknown_variable_coefficient_is_rejected() {
        let source_instance = Model::new().instance();
        let row = CompiledLinearRow {
            id: CompiledConstraintId(0),
            bounds: ConstraintBounds::le(10.0),
            coefficients: vec![(CompiledVariableId(5), 1.0)],
            name: None,
        };
        let mut origin_map = OriginMap::new();
        origin_map.insert_variable(
            CompiledVariableId(0),
            EntityOrigin::UserVariable(VarId::new(0, Generation::new())),
        );
        origin_map.insert_constraint(
            CompiledConstraintId(0),
            EntityOrigin::UserConstraint(ConId::new(0, Generation::new())),
        );
        let snapshot = BackendSnapshotBuilder::new(source_instance, ModelRevision::ZERO)
            .origin_map(origin_map)
            .objective_policy(CompiledObjectivePolicy::None)
            .add_variable(CompiledVariable {
                id: CompiledVariableId(0),
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
                name: None,
            })
            .add_linear_row(row)
            .finalize()
            .expect("builder finalization checks origins, not coefficient references");

        let mut highs = HighsSession::try_new().expect("HiGHS should be available");
        let err = match highs.synchronize(Synchronization::CompiledRebuild(snapshot)) {
            Ok(_) => {
                panic!(
                    "a snapshot row coefficient referencing an unknown variable must be rejected"
                )
            }
            Err(e) => e,
        };
        assert_eq!(
            err.category,
            ErrorCategory::InvalidInput,
            "a dangling snapshot reference is invalid input"
        );
        assert_eq!(
            highs.current_compilation, None,
            "a rejected rebuild must not establish a compiled state"
        );
    }
}
