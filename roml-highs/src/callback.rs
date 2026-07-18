//! Callback bridge for HiGHS MIP solve events.
//!
//! Implements the callback bridge between HiGHS C callbacks and ROML's
//! [`CallbackHandler`]. Only officially supported callback types are handled;
//! user cuts and incumbent injection are rejected per AD-4.
//!
//! # Architecture
//!
//! - [`CallbackState`]: Holds the boxed [`CallbackHandler`], pointers to
//!   column/row index maps, the HiGHS handle, and an interrupt flag.
//! - [`callback_trampoline`]: [`unsafe extern "C"`] function registered with
//!   HiGHS via [`Highs_setCallback`]. Dispatches callback events to the
//!   handler and returns a `c_int` status.
//! - [`register_callback`]: Boxes a `CallbackState`, registers the trampoline.
//! - [`clear_callback`]: Destroys the `CallbackState` and unregisters.
//!
//! # Safety
//!
//! The callback trampoline is called from within `Highs_run` on the main
//! solver thread. The `user_data` pointer references a `Box<CallbackState>`
//! that was created before solve and destroyed after solve. Access to
//! `CallbackState` from the trampoline is single-threaded during the solve.

use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CStr};

use log::{info, warn};

use crate::bindings::*;
use crate::index_map::IndexMap;
use roml::id::{ConId, VarId};
use roml::solver::backend::BackendError;
use roml::solver::callback::{CallbackAction, CallbackCut, CallbackData, CallbackHandler};

// ── CallbackState ────────────────────────────────────────────────────────────

/// State passed to the C callback trampoline via `user_data`.
///
/// Created before `Highs_run` and destroyed after via [`clear_callback`].
/// Single-threaded access during solve.
pub(crate) struct CallbackState {
    /// Boxed ROML callback handler.
    pub handler: Box<dyn CallbackHandler>,
    /// Pointer to the session's `col_map` (VarId → HiGHS column index).
    pub col_map: *const IndexMap<VarId>,
    /// Pointer to the session's `row_map` (ConId → HiGHS row index).
    pub row_map: *const IndexMap<ConId>,
    /// The HiGHS instance handle.
    pub highs_ptr: *mut c_void,
    /// Number of columns in the current model (for solution mapping).
    pub num_cols: i32,
    /// Flag set when user requests interruption.
    pub user_interrupt: bool,
}

// ── Callback Trampoline ──────────────────────────────────────────────────────

/// Trampoline called by HiGHS during MIP solve.
///
/// Dispatches callback events to the ROML [`CallbackHandler`]. Returns
/// `()` (void) per the HiGHS C callback type.
///
/// # Callback type disposition (per AD-4)
///
/// | Constant | Value | Disposition |
/// |----------|-------|-------------|
/// | `kHighsCallbackMipLogging` | 5 | Informational: log message |
/// | `kHighsCallbackMipInterrupt` | 6 | Interrupt: set `data_in.user_interrupt` |
/// | `kHighsCallbackMipSolution` | 3 | Informational: candidate solution |
/// | `kHighsCallbackMipImprovingSolution` | 4 | Informational: incumbent |
/// | `kHighsCallbackMipGetCutPool` | 7 | Read-only diagnostic: no-op |
/// | `kHighsCallbackMipDefineLazyConstraints` | 8 | Supported: lazy constraints |
///
/// # Safety
///
/// - `user_data` must be a valid `*mut CallbackState` created by
///   [`register_callback`] and not yet freed.
/// - `data_out` must be a valid pointer to a `HighsCallbackDataOut` when the
///   callback type provides data (e.g., solution, bounds).
/// - Called from within `Highs_run` — no HiGHS API calls that modify the
///   model state should be made from inside this function (except those
///   specifically permitted, like `Highs_addRow` for lazy constraints).
/// - Rust panics are caught via [`catch_unwind`] to prevent crossing the C
///   boundary (T-11-11).
#[allow(non_upper_case_globals)]
pub(crate) unsafe extern "C" fn callback_trampoline(
    event_type: c_int,
    message: *const c_char,
    data_out: *const HighsCallbackDataOut,
    data_in: *mut HighsCallbackDataIn,
    user_data: *mut c_void,
) {
    // SAFETY: user_data is a Box<CallbackState> we created in register_callback.
    // It is valid for the duration of Highs_run.
    let state = &mut *(user_data as *mut CallbackState);

    match event_type as HighsInt {
        kHighsCallbackMipLogging => {
            // Informational: log the HiGHS message string.
            if !message.is_null() {
                let msg = CStr::from_ptr(message).to_string_lossy();
                info!("[HiGHS MIP log] {}", msg.trim_end());
            }
        }

        kHighsCallbackMipInterrupt => {
            // Check the handler for user interruption. Set
            // `data_in.user_interrupt` to 1 to signal HiGHS to stop.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let cb_data = build_callback_data(data_out, state);
                let _action = state.handler.on_candidate(&cb_data);

                if state.user_interrupt && !data_in.is_null() {
                    (*data_in).user_interrupt = 1;
                }
            }));

            if let Err(e) = result {
                warn!("Panic in MIP interrupt callback handler: {:?}", e);
            }
        }

        kHighsCallbackMipSolution => {
            // Informational only: a candidate MIP solution was found.
            if !data_out.is_null() {
                let cd_out = &*data_out;
                info!(
                    "MIP candidate solution: {} vars, obj = {}, primal = {}, dual = {}, gap = {}",
                    cd_out.mip_solution_size,
                    cd_out.objective_function_value,
                    cd_out.mip_primal_bound,
                    cd_out.mip_dual_bound,
                    cd_out.mip_gap,
                );
            }
        }

        kHighsCallbackMipImprovingSolution => {
            // Informational only: an improving incumbent was found.
            if !data_out.is_null() {
                let cd_out = &*data_out;
                info!(
                    "MIP improving incumbent: obj = {}, primal = {}, dual = {}, gap = {}",
                    cd_out.objective_function_value,
                    cd_out.mip_primal_bound,
                    cd_out.mip_dual_bound,
                    cd_out.mip_gap,
                );
            }
        }

        kHighsCallbackMipGetCutPool => {
            // Read-only diagnostic. Safely ignored per the HiGHS callback contract.
        }

        kHighsCallbackMipDefineLazyConstraints => {
            // Officially supported: invoke the CallbackHandler, inject
            // lazy constraints via Highs_addRow if the handler requests them.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let cb_data = build_callback_data(data_out, state);
                let action = state.handler.on_candidate(&cb_data);

                if let CallbackAction::AddCuts(cuts) = action {
                    inject_lazy_constraints(state.highs_ptr, state.col_map, &cuts);
                }
            }));

            if let Err(e) = result {
                warn!("Panic in lazy constraint callback handler: {:?}", e);
            }
        }

        _ => {
            // Unknown callback type — safely ignore per HiGHS contract.
            // kHighsCallbackCallbackMipUserSolution (9) falls here.
            warn!("Unknown HiGHS callback type: {}", event_type);
        }
    }
}

// ── Registration and Cleanup ─────────────────────────────────────────────────

/// Register a [`CallbackHandler`] with the HiGHS solver.
///
/// Boxes the handler into a [`CallbackState`] and registers it via
/// [`Highs_setCallback`]. Returns a pointer to the `CallbackState` for
/// later cleanup via [`clear_callback`].
///
/// # Safety
///
/// The returned `*mut CallbackState` must be freed via [`clear_callback`]
/// after the solve completes. Failure to do so leaks the handler.
///
/// `raw` must be a valid HiGHS instance handle.
/// `col_map` and `row_map` must remain valid for the duration of the solve.
pub(crate) fn register_callback(
    raw: *mut c_void,
    handler: Box<dyn CallbackHandler>,
    col_map: *const IndexMap<VarId>,
    row_map: *const IndexMap<ConId>,
    num_cols: i32,
) -> Result<*mut CallbackState, BackendError> {
    // SAFETY: Box::into_raw gives us a raw pointer we control.
    // Highs_setCallback stores it as user_data and passes it back
    // in every callback invocation.
    let state = Box::into_raw(Box::new(CallbackState {
        handler,
        col_map,
        row_map,
        highs_ptr: raw,
        num_cols,
        user_interrupt: false,
    }));

    // SAFETY: `state` is a valid pointer to a CallbackState we just created.
    // Highs_setCallback registers the trampoline; no allocation happens
    // on the HiGHS side that would invalidate the pointer.
    let ret = unsafe {
        Highs_setCallback(raw, Some(callback_trampoline), state as *mut c_void)
    };
    if ret != STATUS_OK {
        // If registration fails, free the state to avoid leaking.
        // SAFETY: state is the same pointer from Box::into_raw above.
        unsafe {
            let _ = Box::from_raw(state);
        }
        return Err(crate::error::from_native_status(ret, "Highs_setCallback"));
    }

    Ok(state)
}

/// Clear the callback handler after solve completion.
///
/// Reconstructs the `Box<CallbackState>` from the raw pointer to drop it,
/// then unregisters the callback with HiGHS.
///
/// # Safety
///
/// `state` must be a valid pointer returned by [`register_callback`] that
/// has not yet been freed.
pub(crate) fn clear_callback(raw: *mut c_void, state: *mut CallbackState) {
    // SAFETY:
    // - `state` was created by Box::into_raw in register_callback.
    // - Reconstructing the Box drops the CallbackState and its handler.
    // The user_data lifecycle is: created before solve, destroyed after.
    unsafe {
        let _ = Box::from_raw(state);
        Highs_setCallback(raw, None, std::ptr::null_mut());
    }
}

// ── Internal helpers ─────────────────────────────────────────────────────────

/// Build a [`CallbackData`] from HiGHS callback output data.
///
/// Extracts variable values, primal/dual bounds, and MIP gap from the
/// `HighsCallbackDataOut` struct, mapping HiGHS column indices back to
/// ROML `VarId` using the column map's reverse index.
///
/// # Safety
///
/// - `data_out` must be non-null and point to a valid `HighsCallbackDataOut`.
/// - `state.col_map` must be a valid pointer to the session's column map.
unsafe fn build_callback_data(
    data_out: *const HighsCallbackDataOut,
    state: &CallbackState,
) -> CallbackData {
    if data_out.is_null() {
        return CallbackData {
            var_values: HashMap::new(),
            primal_bound: f64::INFINITY,
            dual_bound: f64::NEG_INFINITY,
            mip_gap: f64::INFINITY,
        };
    }

    let cd_out = &*data_out;

    let mut var_values = HashMap::new();
    if !cd_out.mip_solution.is_null() && cd_out.mip_solution_size > 0 {
        let solution_slice =
            std::slice::from_raw_parts(cd_out.mip_solution, cd_out.mip_solution_size as usize);

        // SAFETY: col_map is valid for the duration of the solve.
        // We dereference the raw pointer to read mapped indices.
        if !state.col_map.is_null() {
            let col_map_ref = &*state.col_map;
            let rev = col_map_ref.reverse_map();
            for (hi_idx, &val) in solution_slice.iter().enumerate() {
                if let Some(&var_id) = rev.get(&(hi_idx as i32)) {
                    var_values.insert(var_id, val);
                }
            }
        }
    }

    CallbackData {
        var_values,
        primal_bound: cd_out.mip_primal_bound,
        dual_bound: cd_out.mip_dual_bound,
        mip_gap: cd_out.mip_gap,
    }
}

/// Inject lazy constraints returned by the callback handler into HiGHS.
///
/// Each [`CallbackCut`] in `cuts` is added as a new row via
/// [`Highs_addRow`]. The column indices are translated from ROML `VarId`
/// to HiGHS column indices using `col_map`.
///
/// # Panics
///
/// Panics if a `VarId` in a cut's terms is not found in `col_map`. This
/// is a programming error — the model state must be consistent.
///
/// # Safety
///
/// `highs_ptr` must be a valid HiGHS instance handle. `col_map` must be
/// valid and contain entries for all variables referenced in the cuts.
unsafe fn inject_lazy_constraints(
    highs_ptr: *mut c_void,
    col_map: *const IndexMap<VarId>,
    cuts: &[CallbackCut],
) {
    if col_map.is_null() || cuts.is_empty() {
        return;
    }

    let col_map_ref = &*col_map;

    for cut in cuts {
        let nz = cut.terms.len() as HighsInt;
        if nz == 0 {
            // Skip empty cuts (no variables).
            continue;
        }

        let indices: Vec<HighsInt> = cut
            .terms
            .iter()
            .map(|(var_id, _)| {
                col_map_ref
                    .get(*var_id)
                    .unwrap_or_else(|| panic!("VarId {:?} not found in col_map for lazy constraint", var_id))
            })
            .collect();

        let values: Vec<f64> = cut.terms.iter().map(|(_, val)| *val).collect();

        let ret = Highs_addRow(
            highs_ptr,
            cut.lower,
            cut.upper,
            nz,
            indices.as_ptr(),
            values.as_ptr(),
        );
        if ret < 0 {
            warn!(
                "Failed to inject lazy constraint row: Highs_addRow returned {}",
                ret
            );
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal handler that always accepts.
    struct AcceptHandler;

    impl CallbackHandler for AcceptHandler {
        fn on_candidate(&mut self, _data: &CallbackData) -> CallbackAction {
            CallbackAction::Accept
        }
    }

    /// A handler that always adds a dummy cut.
    struct CutHandler;

    impl CallbackHandler for CutHandler {
        fn on_candidate(&mut self, _data: &CallbackData) -> CallbackAction {
            CallbackAction::AddCuts(vec![CallbackCut {
                terms: vec![],
                lower: f64::NEG_INFINITY,
                upper: 0.0,
            }])
        }
    }

    #[test]
    fn build_callback_data_with_null_data_out() {
        // SAFETY: testing safe null handling
        let state = CallbackState {
            handler: Box::new(AcceptHandler),
            col_map: std::ptr::null(),
            row_map: std::ptr::null(),
            highs_ptr: std::ptr::null_mut(),
            num_cols: 0,
            user_interrupt: false,
        };

        let data = unsafe { build_callback_data(std::ptr::null(), &state) };
        assert!(data.var_values.is_empty());
        assert!(data.primal_bound.is_infinite());
        assert!(data.dual_bound.is_infinite());
        assert!(data.mip_gap.is_infinite());
    }

    #[test]
    fn inject_empty_cuts_is_noop() {
        // SAFETY: null pointers are handled gracefully by inject_lazy_constraints
        unsafe {
            inject_lazy_constraints(std::ptr::null_mut(), std::ptr::null(), &[]);
            inject_lazy_constraints(std::ptr::null_mut(), std::ptr::null(), &[
                CallbackCut {
                    terms: vec![],
                    lower: f64::NEG_INFINITY,
                    upper: 0.0,
                },
            ]);
        }
        // No panic — empty cuts are skipped.
    }
}
