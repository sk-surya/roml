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

use std::ffi::{c_void, CString};

use log::{info, warn};

use crate::bindings::*;
use crate::callback::{clear_callback, register_callback, CallbackState};
use crate::error::{check_highs_status, from_native_status};
use crate::lifecycle::HighsSession;
use crate::projection::{apply_delta_batch, rebuild_from_snapshot};
use crate::solution::{extract_solution, map_termination_status};
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
    /// Apply a [`Synchronization`] — either a full rebuild from snapshot or
    /// an incremental delta batch.
    ///
    /// On success, invalidates any cached solution (T-11-18) and returns the
    /// updated cursor and health.
    fn synchronize(&mut self, sync: Synchronization) -> Result<SyncReceipt, BackendError> {
        match sync {
            Synchronization::Rebuild(snapshot) => {
                let revision = snapshot.revision;
                info!(
                    "Rebuilding HiGHS session from snapshot at revision {}",
                    revision
                );

                let result = rebuild_from_snapshot(
                    self.raw,
                    &snapshot,
                    &mut self.col_map,
                    &mut self.row_map,
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

            Synchronization::DeltaBatch(batch) => {
                info!(
                    "Applying delta batch r{} -> r{} ({} ops)",
                    batch.from,
                    batch.to,
                    batch.operations.len()
                );

                // Reject a batch whose base revision doesn't match the cursor
                // BEFORE applying any operation. Applying first and failing at
                // cursor.advance would leave the HiGHS model partially mutated
                // (dirty state) while the cursor still reports Ready — exactly
                // the partial-apply defect the differential harness guards
                // against. The reference backend checks this up front.
                if batch.from != self.cursor.applied_revision {
                    let e = BackendError::new(
                        format!(
                            "delta batch from {} does not match cursor at {}",
                            batch.from, self.cursor.applied_revision
                        ),
                        ErrorCategory::InvalidInput,
                        HealthEffect::Recoverable,
                    );
                    self.cursor.mark_rebuild();
                    return Err(e);
                }

                let result = apply_delta_batch(
                    self.raw,
                    &batch,
                    &mut self.col_map,
                    &mut self.row_map,
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
                        self.cursor.advance(&batch).map_err(|e| {
                            self.cursor.mark_terminal();
                            BackendError::new(
                                format!("cursor failed to advance after delta: {}", e),
                                ErrorCategory::Internal,
                                HealthEffect::Terminal,
                            )
                        })?;
                        // T-11-18: Invalidate stale solution after model mutation.
                        self.current_solution = None;
                        info!("Delta batch applied, cursor at revision {}", batch.to);
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

        // Step 1: Negotiate solve options.
        let effective_config = negotiate_options(self.raw, request)?;

        // Step 2: Register callback if a handler is set.
        let cb_state: Option<*mut CallbackState> =
            if let Some(handler) = self.callback_handler.take() {
                info!("Registering MIP callback handler");
                let col_map_ptr: *const crate::index_map::IndexMap<VarId> = &self.col_map;
                let row_map_ptr: *const crate::index_map::IndexMap<ConId> = &self.row_map;
                // SAFETY: self.raw is a valid HiGHS instance handle. col_map and
                // row_map remain valid for the duration of the solve. The returned
                // state pointer is stored in self.callback_state and cleaned up
                // after solve completes.
                let num_col = unsafe { Highs_getNumCol(self.raw) };
                match register_callback(self.raw, handler, col_map_ptr, row_map_ptr, num_col) {
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

        // Step 5: Extract solution data.
        let solution = extract_solution(self.raw, &status, &self.col_map, &self.row_map);
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
        BackendCapabilities {
            lp: true,
            mip: true,
            solution: true,
            duals: true,
            reduced_costs: true,
            callbacks: true,
            delete: true,
            add_variable: true,
            add_constraint: true,
            set_coefficient: true,
            set_bounds: true,
            set_objective: true,
            // H7: Semi-continuous is explicitly rejected — not supported.
            semicontinuous: false,
            semiinteger: false,
            parameter_update: false,
        }
    }
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
}
