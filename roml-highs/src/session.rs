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
use crate::compiler::{
    apply_backend_delta, missing_variable, normalize_bound, project_objective_policy,
    rebuild_from_backend_snapshot,
};
use crate::error::{check_highs_status, from_native_status};
use crate::lifecycle::HighsSession;
use crate::solution::{extract_solution, map_termination_status};
use roml::advanced::{
    CompilationId, CompileError, CompiledConstraintId, CompiledEntityRegistry,
    CompiledObjectivePolicy, CompiledOverlay, CompiledVariableId, OverlayApplyReceipt, OverlayOp,
    OverlayRollbackOutcome, OverlaySession,
};
use roml::compiler::capability::{
    BackendCapabilitySet, BackendFeature, FeatureLimitations, FeatureSupport,
};
use roml::id::{ConId, ObjId, VarId};
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
                        // P27 Task 10: track the active compiled objective
                        // policy so an overlay objective override can be
                        // restored on rollback.
                        self.compiled_objective_policy = snapshot.objective_policy.clone();
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
                        // P27 Task 10: track the active compiled objective
                        // policy — the batch's LAST SetObjectivePolicy op is
                        // the target policy (A32 includes None).
                        if let Some(roml::advanced::BackendOp::SetObjectivePolicy(policy)) =
                            batch.operations.iter().rev().find(|op| {
                                matches!(op, roml::advanced::BackendOp::SetObjectivePolicy(_))
                            })
                        {
                            self.compiled_objective_policy = policy.clone();
                        }
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
            // The session does not know the overlay id; the façade records
            // `Some(overlay.id)` in the normalized metadata on an overlay solve.
            overlay_id: None,
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

// ── OverlaySession (P27 Task 10) ───────────────────────────────────────────────

/// Prior native state captured by an overlay apply on the HiGHS session,
/// enabling the transactional rollback (SM-07.4) and the post-rollback clean
/// verification.
pub(crate) struct HighsOverlayState {
    /// The exact base compiled state the overlay was applied on top of.
    base_compilation: CompilationId,
    /// The overlay-compounded compiled state.
    applied_compilation: CompilationId,
    /// Prior bounds of every compiled variable the overlay temporarily set
    /// (compiled id -> native (lb, ub)).
    prior_bounds: HashMap<CompiledVariableId, (f64, f64)>,
    /// Temporary native rows added by the overlay (compiled row id -> native
    /// row index).
    added_rows: Vec<(CompiledConstraintId, HighsInt)>,
    /// The base compiled objective policy (restored on rollback).
    prior_policy: CompiledObjectivePolicy,
    /// The native row/column counts at apply (rollback's clean target).
    base_row_count: HighsInt,
    base_col_count: HighsInt,
    /// IN-01: the FULL base native bound state (compiled id -> native
    /// (lb, ub)) captured at apply, so `verify_overlay_clean` can prove every
    /// bound (not just the row/col counts) is restored exactly.
    base_var_bounds: HashMap<CompiledVariableId, (f64, f64)>,
    /// IN-01: the base active native objective captured at apply.
    base_active_obj: Option<ObjId>,
}

impl OverlaySession for HighsSession {
    /// Apply a compiled overlay against the exact base compiled state,
    /// transitioning `C_base -> C_overlay`.
    ///
    /// Temporary bounds go through [`Highs_changeColBounds`] (recording the
    /// prior bounds from `self.var_bounds`); temporary rows through
    /// [`Highs_addRows`] (recording the native row index in the overlay state);
    /// an objective override through the compiled-policy projection shared with
    /// `roml::advanced::BackendOp::SetObjectivePolicy`. A stale base is
    /// rejected BEFORE any native mutation (D28 exact-id authority).
    fn apply_overlay(
        &mut self,
        overlay: &CompiledOverlay,
    ) -> Result<OverlayApplyReceipt, BackendError> {
        // Exact-id stale rejection before mutation (D28 / SM-03.9). CR-02:
        // EVERY early-return error path inside apply_overlay marks the cursor
        // `RequiresRebuild` — the facade never trusts the backend to have
        // self-marked, and a partially applied overlay is never silently
        // reused (SM-07.4, D7).
        let actual = match self.current_compilation {
            Some(compilation) => compilation,
            None => {
                self.cursor.mark_rebuild();
                return Err(BackendError::new(
                    "the HiGHS session has no compiled base to apply the overlay on top of",
                    ErrorCategory::InvalidInput,
                    HealthEffect::RequiresRebuild,
                ));
            }
        };
        if actual != overlay.base_compilation {
            self.cursor.mark_rebuild();
            return Err(BackendError::new(
                format!(
                    "stale overlay base: the overlay requires compilation {:?}, but the session \
                     holds {:?} (D28 exact compilation authority)",
                    overlay.base_compilation, actual
                ),
                ErrorCategory::InvalidInput,
                HealthEffect::RequiresRebuild,
            ));
        }

        let mut state = HighsOverlayState {
            base_compilation: overlay.base_compilation,
            applied_compilation: overlay.compilation_id,
            prior_bounds: HashMap::new(),
            added_rows: Vec::new(),
            prior_policy: self.compiled_objective_policy.clone(),
            // SAFETY: self.raw is a valid HiGHS instance handle.
            base_row_count: unsafe { Highs_getNumRow(self.raw) },
            base_col_count: unsafe { Highs_getNumCol(self.raw) },
            // IN-01: capture the FULL base native state so the post-rollback
            // verification can prove the bounds/objective are restored exactly
            // (not just the row/col counts).
            base_var_bounds: self.var_bounds.clone(),
            base_active_obj: self.active_obj,
        };

        for op in &overlay.operations {
            match op {
                OverlayOp::SetTemporaryVariableBounds { variable, bounds } => {
                    let idx = match self.col_map.get(*variable) {
                        Some(idx) => idx,
                        None => {
                            self.cursor.mark_rebuild();
                            return Err(missing_variable(*variable));
                        }
                    };
                    let prior = match self.var_bounds.get(variable).copied() {
                        Some(prior) => prior,
                        None => {
                            self.cursor.mark_rebuild();
                            return Err(missing_variable(*variable));
                        }
                    };
                    let lb = normalize_bound(bounds.lower, self.inf);
                    let ub = normalize_bound(bounds.upper, self.inf);
                    // SAFETY: raw is valid; idx is a live native column.
                    unsafe {
                        if let Err(e) = check_highs_status(
                            Highs_changeColBounds(self.raw, idx, lb, ub),
                            self.raw,
                            "Highs_changeColBounds (overlay temporary bound)",
                        ) {
                            self.cursor.mark_rebuild();
                            return Err(e);
                        }
                    }
                    // Record the FIRST prior bound only — a later overlay op on
                    // the same variable must not overwrite the base capture.
                    state.prior_bounds.entry(*variable).or_insert(prior);
                    self.var_bounds.insert(*variable, (lb, ub));
                }
                OverlayOp::AddTemporaryRow { row } => {
                    let lb = normalize_bound(row.bounds.lower, self.inf);
                    let ub = normalize_bound(row.bounds.upper, self.inf);
                    let mut indices: Vec<HighsInt> = Vec::with_capacity(row.coefficients.len());
                    let mut values: Vec<f64> = Vec::with_capacity(row.coefficients.len());
                    for (cid, value) in &row.coefficients {
                        // F5: a coefficient referencing a compiled variable not
                        // present in the held state is a typed error, not a
                        // silent skip.
                        let col = match self.col_map.get(*cid) {
                            Some(col) => col,
                            None => {
                                self.cursor.mark_rebuild();
                                return Err(missing_variable(*cid));
                            }
                        };
                        indices.push(col);
                        values.push(*value);
                    }
                    let starts = [0 as HighsInt];
                    // SAFETY: raw is valid; the arrays are populated for the
                    // single new row; return code is checked immediately.
                    let status = unsafe {
                        Highs_addRows(
                            self.raw,
                            1,
                            &lb,
                            &ub,
                            indices.len() as HighsInt,
                            starts.as_ptr(),
                            indices.as_ptr(),
                            values.as_ptr(),
                        )
                    };
                    if status != STATUS_OK {
                        self.cursor.mark_rebuild();
                        return Err(from_native_status(status, "Highs_addRows (overlay row)"));
                    }
                    // SAFETY: raw is valid. The added row is the last row.
                    let native_row = unsafe { Highs_getNumRow(self.raw) } - 1;
                    state.added_rows.push((row.id, native_row));
                }
                // RemoveTemporaryRow is a rollback-only op; apply does not
                // produce it.
                OverlayOp::RemoveTemporaryRow { row } => {
                    self.cursor.mark_rebuild();
                    return Err(BackendError::new(
                        format!("apply_overlay does not accept RemoveTemporaryRow ({row:?})"),
                        ErrorCategory::InvalidInput,
                        HealthEffect::RequiresRebuild,
                    ));
                }
                OverlayOp::SetObjectivePolicy(policy) => {
                    if let Err(e) = project_objective_policy(
                        self.raw,
                        policy,
                        &self.col_map,
                        &self.compiled_to_user_objective,
                        &self.obj_costs,
                        &self.obj_senses,
                        &self.obj_offsets,
                        &mut self.active_obj,
                    ) {
                        self.cursor.mark_rebuild();
                        return Err(e);
                    }
                    self.compiled_objective_policy = policy.clone();
                }
                // `OverlayOp` is `#[non_exhaustive]` (P27 contract): a FUTURE
                // op variant this projection does not understand is a hard
                // typed error — never a silent no-op.
                _ => {
                    self.cursor.mark_rebuild();
                    return Err(BackendError::unsupported(
                        "overlay op variant not supported by this projection",
                    ));
                }
            }
        }

        self.current_compilation = Some(overlay.compilation_id);
        self.overlay_state = Some(state);
        Ok(OverlayApplyReceipt {
            overlay_id: overlay.overlay_id,
            base_compilation: overlay.base_compilation,
            applied_compilation: overlay.compilation_id,
        })
    }

    /// Roll back an applied overlay, transitioning `C_overlay -> C_base`.
    ///
    /// Restores each prior bound, deletes each added temporary row, restores
    /// the base objective policy, and sets `current_compilation` back to the
    /// base. Any failing native call returns
    /// [`RequiresRebuild`](OverlayRollbackOutcome::RequiresRebuild) and marks
    /// the session rebuild-required (D7 invariant). The added temporary rows
    /// live at the END of the native model, so deleting them (in one set call)
    /// leaves every base row's native index unchanged.
    fn rollback_overlay(
        &mut self,
        receipt: &OverlayApplyReceipt,
    ) -> Result<OverlayRollbackOutcome, BackendError> {
        let state = self.overlay_state.as_ref().ok_or_else(|| {
            BackendError::new(
                "no applied overlay to roll back; the receipt does not match this session",
                ErrorCategory::Internal,
                HealthEffect::RequiresRebuild,
            )
        })?;
        if state.applied_compilation != receipt.applied_compilation {
            return Ok(OverlayRollbackOutcome::RequiresRebuild {
                reason: format!(
                    "receipt applied compilation {:?} does not match the session's applied overlay {:?}",
                    receipt.applied_compilation, state.applied_compilation
                ),
            });
        }

        // Restore prior bounds.
        for (variable, (lb, ub)) in &state.prior_bounds {
            let idx = match self.col_map.get(*variable) {
                Some(idx) => idx,
                None => {
                    self.cursor.mark_rebuild();
                    return Ok(OverlayRollbackOutcome::RequiresRebuild {
                        reason: format!(
                            "restoring prior bound: compiled variable {variable:?} is absent"
                        ),
                    });
                }
            };
            // SAFETY: raw is valid; idx is a live native column.
            let ret = unsafe { Highs_changeColBounds(self.raw, idx, *lb, *ub) };
            if ret != STATUS_OK {
                self.cursor.mark_rebuild();
                return Ok(OverlayRollbackOutcome::RequiresRebuild {
                    reason: format!(
                        "restoring prior bound for {variable:?} failed (native code {ret})"
                    ),
                });
            }
            self.var_bounds.insert(*variable, (*lb, *ub));
        }

        // Delete the added temporary rows (all at the end of the native model).
        if !state.added_rows.is_empty() {
            let indices: Vec<HighsInt> =
                state.added_rows.iter().map(|(_, native)| *native).collect();
            // SAFETY: raw is valid; the set contains live native row indices.
            let ret = unsafe {
                Highs_deleteRowsBySet(self.raw, indices.len() as HighsInt, indices.as_ptr())
            };
            if ret != STATUS_OK {
                self.cursor.mark_rebuild();
                return Ok(OverlayRollbackOutcome::RequiresRebuild {
                    reason: format!("deleting overlay rows failed (native code {ret})"),
                });
            }
        }

        // Restore the base objective policy.
        if let Err(e) = project_objective_policy(
            self.raw,
            &state.prior_policy,
            &self.col_map,
            &self.compiled_to_user_objective,
            &self.obj_costs,
            &self.obj_senses,
            &self.obj_offsets,
            &mut self.active_obj,
        ) {
            self.cursor.mark_rebuild();
            return Ok(OverlayRollbackOutcome::RequiresRebuild {
                reason: format!("restoring the base objective policy failed: {e}"),
            });
        }
        self.compiled_objective_policy = state.prior_policy.clone();

        let restored = state.base_compilation;
        self.current_compilation = Some(restored);
        Ok(OverlayRollbackOutcome::Clean {
            restored_compilation: restored,
        })
    }

    /// Verify the native model is restored to the base after a `Clean`
    /// rollback: the row/column counts match the base and the session's
    /// compiled state is back at `C_base`.
    fn verify_overlay_clean(&mut self) -> Result<(), BackendError> {
        let state = self.overlay_state.as_ref().ok_or_else(|| {
            BackendError::new(
                "verify_overlay_clean called with no recorded overlay apply",
                ErrorCategory::Internal,
                HealthEffect::RequiresRebuild,
            )
        })?;
        // SAFETY: raw is a valid HiGHS instance handle; getters do not mutate.
        let rows = unsafe { Highs_getNumRow(self.raw) };
        let cols = unsafe { Highs_getNumCol(self.raw) };
        // IN-01: also compare the FULL native bound state, the active native
        // objective, and the compiled objective policy against the captured
        // base — a rollback that restored wrong bound values or wrong objective
        // costs while keeping the same row/col counts must NOT pass
        // verification. (The reference backend's `verify_overlay_clean` already
        // compares the full normalized compiled view.)
        if self.current_compilation != Some(state.base_compilation)
            || rows != state.base_row_count
            || cols != state.base_col_count
            || self.var_bounds != state.base_var_bounds
            || self.active_obj != state.base_active_obj
            || self.compiled_objective_policy != state.prior_policy
        {
            self.cursor.mark_rebuild();
            return Err(BackendError::new(
                "post-rollback verification failed: the native model does not match C_base",
                ErrorCategory::Internal,
                HealthEffect::RequiresRebuild,
            ));
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
        compile_overlay, BackendDeltaBatch, BackendOp, BackendSnapshot, BackendSnapshotBuilder,
        CompilationId, CompilationPolicy, CompilationSession, CompiledLinearRow,
        CompiledObjectiveId, CompiledObjectiveLevel, CompiledObjectivePolicy, CompiledVariable,
        CompiledVariableId, CompiledWeightedObjective, EntityOrigin, OriginMap,
    };
    use roml::compiler::capability::SupportLevel;
    use roml::delta::{DeltaBatch, ModelOp};
    use roml::id::Generation;
    use roml::model::{continuous, Bounds, ConstraintBounds, VarType};
    use roml::snapshot::ModelSnapshot;
    use roml::{ConstraintExprExt, CutoffDirection, Model, ObjectiveCutoff, SolveOverlay};

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

    // ── P27 Task 10: HiGHS reversible overlay execution ───────────────────────

    /// A full overlay apply -> solve -> rollback -> verify round-trip on the
    /// HiGHS session. The temporary bound (x = 4) and the cutoff row
    /// (x + y <= 6) change the solved objective; rollback restores the native
    /// model to C_base exactly and a subsequent solve equals the base solve.
    #[test]
    fn highs_overlay_apply_solve_rollback_round_trip() {
        use std::collections::BTreeMap;

        let caps = full_caps();
        let policy = CompilationPolicy::Auto;
        let mut model = Model::new();
        let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
        let y = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
        model.add_constraint((x + y).le(10.0)).unwrap();
        let obj = model.maximize(x + y).unwrap();
        model.commit().unwrap();
        let snapshot = model.take_snapshot().unwrap();

        let mut compiler = CompilationSession::new();
        let base = compiler
            .compile_snapshot(model.instance(), &snapshot, &policy, &caps)
            .expect("snapshot must compile");
        let mut highs = HighsSession::try_new().expect("HiGHS should be available");
        highs
            .synchronize(Synchronization::CompiledRebuild(base.clone()))
            .expect("base rebuild must succeed");
        let c_base = base.compilation_id;

        let request = SolveRequest {
            enable_output: Some(false),
            ..SolveRequest::new()
        };
        let base_result = highs.solve(&request).expect("base solve must succeed");
        let base_obj = base_result
            .solution
            .as_ref()
            .and_then(|s| s.objective_value)
            .expect("base objective value");
        assert_eq!(base_obj, 10.0, "max x + y with x + y <= 10 is 10");
        assert_eq!(base_result.compilation_id, Some(c_base));
        let base_rows = unsafe { Highs_getNumRow(highs.raw) };

        // Overlay: temp fixing x = 4 and cutoff f(x) <= 6 (f = x + y).
        let overlay = SolveOverlay::new(
            BTreeMap::from([(x, 4.0)]),
            vec![],
            vec![],
            vec![ObjectiveCutoff {
                objective: obj,
                limit: 6.0,
                direction: CutoffDirection::Upper,
            }],
        )
        .expect("overlay id allocates");
        let compiled =
            compile_overlay(&model, &compiler, &overlay, None).expect("overlay compiles");
        assert_eq!(compiled.base_compilation, c_base);
        let c_overlay = compiled.compilation_id;

        // Apply: C_base -> C_overlay; the native row count grows by one.
        let receipt = highs.apply_overlay(&compiled).expect("apply must succeed");
        assert_eq!(receipt.base_compilation, c_base);
        assert_eq!(receipt.applied_compilation, c_overlay);
        assert_eq!(highs.current_compilation, Some(c_overlay));
        assert_eq!(
            unsafe { Highs_getNumRow(highs.raw) },
            base_rows + 1,
            "the cutoff row must be added to the native model"
        );

        // Solve under the overlay: x fixed to 4, x + y <= 6 -> x = 4, y = 2,
        // objective 6.
        let overlay_result = highs.solve(&request).expect("overlay solve must succeed");
        assert_eq!(overlay_result.compilation_id, Some(c_overlay));
        assert_eq!(
            overlay_result
                .solution
                .as_ref()
                .and_then(|s| s.objective_value),
            Some(6.0),
            "the temp fixing and cutoff must constrain the overlay solve"
        );

        // Rollback: C_overlay -> C_base; the native row count returns to base.
        let outcome = highs
            .rollback_overlay(&receipt)
            .expect("rollback must succeed");
        assert!(
            matches!(
                outcome,
                OverlayRollbackOutcome::Clean { restored_compilation } if restored_compilation == c_base
            ),
            "a fully applied overlay must roll back Clean, got {outcome:?}"
        );
        assert_eq!(highs.current_compilation, Some(c_base));
        assert_eq!(
            unsafe { Highs_getNumRow(highs.raw) },
            base_rows,
            "rollback must remove the overlay's temporary row"
        );

        // Verify clean: native model matches C_base.
        highs
            .verify_overlay_clean()
            .expect("post-rollback verification");

        // The base solve is reproduced exactly — no overlay leak.
        let after = highs
            .solve(&request)
            .expect("post-rollback solve must succeed");
        assert_eq!(after.compilation_id, Some(c_base));
        assert_eq!(
            after.solution.as_ref().and_then(|s| s.objective_value),
            Some(base_obj),
            "a solve after rollback must equal the base solve (no overlay leak)"
        );
    }

    /// A stale overlay apply is rejected BEFORE any native mutation; the
    /// session's compiled state and native model are unchanged.
    #[test]
    fn highs_stale_overlay_apply_rejects_before_mutation() {
        use std::collections::BTreeMap;

        let caps = full_caps();
        let policy = CompilationPolicy::Auto;
        let mut model = Model::new();
        let x = model.add_variable(continuous()).unwrap();
        model.maximize(x).unwrap();
        model.commit().unwrap();
        let snapshot = model.take_snapshot().unwrap();

        let mut compiler_a = CompilationSession::new();
        let base = compiler_a
            .compile_snapshot(model.instance(), &snapshot, &policy, &caps)
            .expect("snapshot must compile");
        let mut highs = HighsSession::try_new().expect("HiGHS should be available");
        highs
            .synchronize(Synchronization::CompiledRebuild(base.clone()))
            .expect("base rebuild must succeed");
        let held = highs.current_compilation.expect("base compiled");

        // A SECOND independent compilation -> a DISTINCT exact id (D28).
        let mut compiler_b = CompilationSession::new();
        let compiled_b = compiler_b
            .compile_snapshot(model.instance(), &snapshot, &policy, &caps)
            .expect("second snapshot must compile");
        assert_ne!(held, compiled_b.compilation_id);

        // Compile the overlay against compiler_b's base (NOT the session's).
        let overlay =
            SolveOverlay::new(BTreeMap::from([(x, 1.0)]), vec![], vec![], vec![]).unwrap();
        let compiled = compile_overlay(&model, &compiler_b, &overlay, None).unwrap();
        assert_ne!(compiled.base_compilation, held);

        let cols_before = unsafe { Highs_getNumCol(highs.raw) };
        let err = highs
            .apply_overlay(&compiled)
            .expect_err("a stale overlay must be rejected at apply time");
        assert_eq!(err.category, ErrorCategory::InvalidInput);
        assert!(
            err.message.contains("compilation") || err.message.contains("base"),
            "the error must name the stale compilation, got: {}",
            err.message
        );
        assert_eq!(
            unsafe { Highs_getNumCol(highs.raw) },
            cols_before,
            "a rejected stale overlay must not mutate the native model"
        );
        assert_eq!(
            highs.current_compilation,
            Some(held),
            "a rejected stale overlay must not change the session's compiled state"
        );
    }

    /// IN-01: `verify_overlay_clean` compares the FULL native bound/objective
    /// state against the captured base — a rollback that restored the row/col
    /// counts but left a wrong bound value (here: a variable the overlay never
    /// touched) must fail verification. (Before this fix only the row/col
    /// counts and the compilation id were compared.)
    #[test]
    fn highs_verify_overlay_clean_detects_wrong_bound_state() {
        use std::collections::BTreeMap;

        let caps = full_caps();
        let policy = CompilationPolicy::Auto;
        let mut model = Model::new();
        let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
        let y = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
        model.maximize(x + y).unwrap();
        model.commit().unwrap();
        let snapshot = model.take_snapshot().unwrap();

        let mut compiler = CompilationSession::new();
        let base = compiler
            .compile_snapshot(model.instance(), &snapshot, &policy, &caps)
            .expect("snapshot must compile");
        let mut highs = HighsSession::try_new().expect("HiGHS should be available");
        highs
            .synchronize(Synchronization::CompiledRebuild(base.clone()))
            .expect("base rebuild must succeed");

        // Apply + rollback a real overlay (temp fixing x = 2.0) -> Clean.
        let overlay =
            SolveOverlay::new(BTreeMap::from([(x, 2.0)]), vec![], vec![], vec![]).unwrap();
        let compiled = compile_overlay(&model, &compiler, &overlay, None).unwrap();
        let receipt = highs.apply_overlay(&compiled).expect("apply must succeed");
        let outcome = highs
            .rollback_overlay(&receipt)
            .expect("rollback must succeed");
        assert!(
            matches!(outcome, OverlayRollbackOutcome::Clean { .. }),
            "a fully applied overlay must roll back Clean, got {outcome:?}"
        );

        // Corrupt a bound for a variable the overlay never touched: the native
        // row/col counts and the compilation id are unchanged, but the held
        // bound state diverges from the captured base.
        let y_idx = highs
            .col_map
            .get(CompiledVariableId(1))
            .expect("compiled variable 1 (y) present");
        unsafe {
            check_highs_status(
                Highs_changeColBounds(highs.raw, y_idx, 1.0, 1.0),
                highs.raw,
                "corrupt y bound",
            )
            .expect("native corruption applies");
        }
        highs.var_bounds.insert(CompiledVariableId(1), (1.0, 1.0));

        let err = highs
            .verify_overlay_clean()
            .expect_err("a wrong bound value must fail post-rollback verification (IN-01)");
        assert_eq!(
            err.category,
            ErrorCategory::Internal,
            "post-rollback verification failure is internal"
        );
        assert_eq!(
            highs.cursor.health,
            AdapterHealth::RequiresRebuild,
            "a failed verification must mark the session RequiresRebuild"
        );
    }

    /// CR-02: a MID-apply failure (op 2 fails after op 1 already mutated the
    /// native model) must mark the session `RequiresRebuild` — a subsequent
    /// plain solve then forces a full rebuild instead of silently solving
    /// against the half-overlaid native state on the no-sync fast path.
    #[test]
    fn highs_mid_apply_failure_marks_requires_rebuild() {
        use std::collections::BTreeMap;

        let caps = full_caps();
        let policy = CompilationPolicy::Auto;
        let mut model = Model::new();
        let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
        let y = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
        model.maximize(x + y).unwrap();
        model.commit().unwrap();
        let snapshot = model.take_snapshot().unwrap();

        let mut compiler = CompilationSession::new();
        let base = compiler
            .compile_snapshot(model.instance(), &snapshot, &policy, &caps)
            .expect("snapshot must compile");
        let mut highs = HighsSession::try_new().expect("HiGHS should be available");
        highs
            .synchronize(Synchronization::CompiledRebuild(base.clone()))
            .expect("base rebuild must succeed");
        let c_base = base.compilation_id;

        // Compile a VALID single-temp-fixing overlay, then append a SECOND op
        // that references an unknown compiled variable: op 1 (x -> [2,2])
        // mutates the native model, then op 2 fails. This is exactly CR-02's
        // "op N fails after ops 1..N-1 mutated".
        let overlay =
            SolveOverlay::new(BTreeMap::from([(x, 2.0)]), vec![], vec![], vec![]).unwrap();
        let mut compiled = compile_overlay(&model, &compiler, &overlay, None)
            .expect("overlay compiles against the exact base");
        assert_eq!(compiled.base_compilation, c_base);
        compiled
            .operations
            .push(OverlayOp::SetTemporaryVariableBounds {
                variable: CompiledVariableId(999),
                bounds: Bounds::new(3.0, 3.0),
            });

        let err = highs
            .apply_overlay(&compiled)
            .expect_err("the second op must fail apply");
        assert_eq!(
            err.category,
            ErrorCategory::InvalidInput,
            "an unknown compiled variable is invalid input"
        );
        assert_eq!(
            highs.cursor.health,
            AdapterHealth::RequiresRebuild,
            "a mid-apply failure must mark the session RequiresRebuild so the \
             half-overlaid native state is never silently reused"
        );
        assert_eq!(
            highs.health(),
            AdapterHealth::RequiresRebuild,
            "health() must reflect the marked cursor"
        );
    }
}
