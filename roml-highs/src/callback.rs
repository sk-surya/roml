//! Callback bridge for HiGHS MIP solve events.
//!
//! Implements the callback bridge between HiGHS C callbacks and ROML's
//! [`CallbackHandler`]. Only officially supported callback types are handled;
//! advanced features (lazy constraints, cuts, incumbent injection) are
//! deferred to post-v0.1 per reviewer decision.
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
use roml::advanced::{CompiledConstraintId, CompiledVariableId};
use roml::id::VarId;
use roml::solver::backend::BackendError;
use roml::solver::callback::{CallbackData, CallbackHandler};

// ── CallbackState ────────────────────────────────────────────────────────────

/// State passed to the C callback trampoline via `user_data`.
///
/// Created before `Highs_run` and destroyed after via [`clear_callback`].
/// Single-threaded access during solve.
pub(crate) struct CallbackState {
    /// Boxed ROML callback handler.
    pub handler: Box<dyn CallbackHandler>,
    /// Pointer to the session's `col_map` (CompiledVariableId → HiGHS column
    /// index, P26 compiled path).
    pub col_map: *const IndexMap<CompiledVariableId>,
    /// Pointer to the session's `row_map` (CompiledConstraintId → HiGHS row
    /// index, P26 compiled path).
    #[allow(dead_code)]
    pub row_map: *const IndexMap<CompiledConstraintId>,
    /// Pointer to the session's compiled-id → user-variable map (SM-02.5).
    pub compiled_to_user_variable: *const HashMap<CompiledVariableId, VarId>,
    /// The HiGHS instance handle.
    #[allow(dead_code)]
    pub highs_ptr: *mut c_void,
    /// Number of columns in the current model (for solution mapping).
    #[allow(dead_code)]
    pub num_cols: i32,
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
/// | `kHighsCallbackMipInterrupt` | 6 | Interrupt-request check: invoke handler; native interrupt channel unused (deferred) |
/// | `kHighsCallbackMipSolution` | 3 | Informational: candidate solution |
/// | `kHighsCallbackMipImprovingSolution` | 4 | Informational: incumbent |
/// | `kHighsCallbackMipGetCutPool` | 7 | Read-only diagnostic: no-op |
///
/// # Safety
///
/// - `user_data` must be a valid `*mut CallbackState` created by
///   [`register_callback`] and not yet freed.
/// - `data_out` must be a valid pointer to a `HighsCallbackDataOut` when the
///   callback type provides data (e.g., solution, bounds).
/// - Called from within `Highs_run` — no HiGHS API calls that modify the
///   model state should be made from inside this function.
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
            // HiGHS interrupt-request check. The handler is invoked so it can
            // observe the current state. The native `data_in.user_interrupt`
            // channel is intentionally left untouched: nothing in this adapter
            // requests interruption (CallbackAction has no Interrupt variant,
            // and the feature is deferred post-v0.1 per AD-4).
            let _ = data_in;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let cb_data = build_callback_data(data_out, state);
                let _action = state.handler.on_candidate(&cb_data);
            }));

            if let Err(e) = result {
                warn!("Panic in MIP interrupt callback handler: {:?}", e);
            }
        }

        kHighsCallbackMipSolution => {
            // A candidate MIP solution was found — invoke the handler so it
            // can inspect the candidate. The returned action is observed but
            // not acted on: cut/lazy-constraint injection was removed from
            // this adapter (per the AD-4 cleanup), so only observation and
            // interruption are supported.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let cb_data = build_callback_data(data_out, state);
                let _action = state.handler.on_candidate(&cb_data);
            }));

            if let Err(e) = result {
                warn!("Panic in MIP solution callback handler: {:?}", e);
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

        _ => {
            // Unknown callback type — safely ignore per HiGHS contract.
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
    col_map: *const IndexMap<CompiledVariableId>,
    row_map: *const IndexMap<CompiledConstraintId>,
    compiled_to_user_variable: *const HashMap<CompiledVariableId, VarId>,
    num_cols: i32,
) -> Result<*mut CallbackState, BackendError> {
    // SAFETY: Box::into_raw gives us a raw pointer we control.
    // Highs_setCallback stores it as user_data and passes it back
    // in every callback invocation.
    let state = Box::into_raw(Box::new(CallbackState {
        handler,
        col_map,
        row_map,
        compiled_to_user_variable,
        highs_ptr: raw,
        num_cols,
    }));

    // SAFETY: `state` is a valid pointer to a CallbackState we just created.
    // Highs_setCallback registers the trampoline; no allocation happens
    // on the HiGHS side that would invalidate the pointer.
    let ret = unsafe { Highs_setCallback(raw, Some(callback_trampoline), state as *mut c_void) };
    if ret != STATUS_OK {
        // If registration fails, free the state to avoid leaking.
        // SAFETY: state is the same pointer from Box::into_raw above.
        unsafe {
            let _ = Box::from_raw(state);
        }
        return Err(crate::error::from_native_status(ret, "Highs_setCallback"));
    }

    // HiGHS requires each callback type to be explicitly enabled with
    // Highs_startCallback after the callback function is registered.
    // Without this, HiGHS never invokes the trampoline even though the
    // function was registered (the MIP callback feature was dead as a
    // result). Enable the MIP events this adapter handles.
    // SAFETY: `raw` is a valid HiGHS instance handle.
    for cb_type in [
        kHighsCallbackMipLogging,
        kHighsCallbackMipInterrupt,
        kHighsCallbackMipSolution,
        kHighsCallbackMipImprovingSolution,
        kHighsCallbackMipGetCutPool,
    ] {
        let ret = unsafe { Highs_startCallback(raw, cb_type) };
        if ret != STATUS_OK {
            // Free the state we already allocated and unregister.
            // SAFETY: state is the same pointer from Box::into_raw above.
            unsafe {
                let _ = Box::from_raw(state);
                Highs_setCallback(raw, None, std::ptr::null_mut());
            }
            return Err(crate::error::from_native_status(ret, "Highs_startCallback"));
        }
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
///
/// **Order:** The native callback is unregistered first, then the state is
/// freed. This prevents use-after-free in HiGHS's internal dispatch.
pub(crate) fn clear_callback(raw: *mut c_void, state: *mut CallbackState) {
    // SAFETY:
    // - `state` was created by Box::into_raw in register_callback.
    // - Reconstructing the Box drops the CallbackState and its handler.
    // The user_data lifecycle is: created before solve, destroyed after.
    // Order: unregister the native callback first, then free the state.
    unsafe {
        Highs_setCallback(raw, None, std::ptr::null_mut());
        let _ = Box::from_raw(state);
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
    // The incumbent MIP solution has one entry per model column in every
    // supported HiGHS version; 1.9.0 exposes no explicit size field, so the
    // column map length is the iteration bound (version-portable — the P24
    // CI system job compiles against system HiGHS 1.9.0 headers).
    if !cd_out.mip_solution.is_null() && !state.col_map.is_null() {
        // SAFETY: col_map and compiled_to_user_variable are valid for the
        // duration of the solve. We dereference the raw pointers to read the
        // mapped indices and translate compiled ids back to user ids (SM-02.5).
        let col_map_ref = &*state.col_map;
        let compiled_to_user = &*state.compiled_to_user_variable;
        let rev = col_map_ref.reverse_map();
        if !rev.is_empty() {
            // SAFETY: mip_solution is the full incumbent solution (one entry
            // per column); rev.len() is the HiGHS column count, and the map
            // lookup below bounds every read to indices that exist.
            let solution_slice =
                unsafe { std::slice::from_raw_parts(cd_out.mip_solution, rev.len()) };
            for (hi_idx, &val) in solution_slice.iter().enumerate() {
                if let Some(compiled) = rev.get(&(hi_idx as i32)).copied() {
                    if let Some(&var_id) = compiled_to_user.get(&compiled) {
                        var_values.insert(var_id, val);
                    }
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

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use roml::solver::callback::CallbackAction;

    /// A minimal handler that always accepts.
    struct AcceptHandler;

    impl CallbackHandler for AcceptHandler {
        fn on_candidate(&mut self, _data: &CallbackData) -> CallbackAction {
            CallbackAction::Accept
        }
    }

    #[test]
    fn build_callback_data_with_null_data_out() {
        // SAFETY: testing safe null handling
        let state = CallbackState {
            handler: Box::new(AcceptHandler),
            col_map: std::ptr::null(),
            row_map: std::ptr::null(),
            compiled_to_user_variable: std::ptr::null(),
            highs_ptr: std::ptr::null_mut(),
            num_cols: 0,
        };

        let data = unsafe { build_callback_data(std::ptr::null(), &state) };
        assert!(data.var_values.is_empty());
        assert!(data.primal_bound.is_infinite());
        assert!(data.dual_bound.is_infinite());
        assert!(data.mip_gap.is_infinite());
    }

    /// A handler that counts invocations.
    struct CountingHandler(std::sync::Arc<std::sync::atomic::AtomicUsize>);

    impl CallbackHandler for CountingHandler {
        fn on_candidate(&mut self, _data: &CallbackData) -> CallbackAction {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            CallbackAction::Accept
        }
    }

    /// Every supported callback type must dispatch through the trampoline
    /// without panicking or unwinding across the C boundary, invoking the
    /// handler exactly for the event types that carry a candidate solution
    /// (interrupt-check and solution). The native interrupt channel must stay
    /// untouched since interruption is deferred.
    #[test]
    fn trampoline_dispatches_supported_callback_types() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let calls = Arc::new(AtomicUsize::new(0));
        let mut state = CallbackState {
            handler: Box::new(CountingHandler(calls.clone())),
            col_map: std::ptr::null(),
            row_map: std::ptr::null(),
            compiled_to_user_variable: std::ptr::null(),
            highs_ptr: std::ptr::null_mut(),
            num_cols: 0,
        };
        let state_ptr: *mut c_void = &mut state as *mut CallbackState as *mut c_void;

        // Only `user_interrupt` exists across supported HiGHS versions;
        // `user_solution`/`cbdata`/`user_has_solution`/`user_solution_size`
        // are 1.15-only. A field literal is not version-portable, so the
        // test initializes via zeroed memory (all-int/pointer fields — a
        // valid zeroed bit pattern) and sets the common field explicitly
        // (P24 CI system job).
        let mut data_in: HighsCallbackDataIn = unsafe { std::mem::zeroed() };
        data_in.user_interrupt = 0;

        // SAFETY: test-only trampoline calls with a valid CallbackState and
        // no model mutation; the interrupt call passes a real data_in.
        unsafe {
            callback_trampoline(
                kHighsCallbackMipInterrupt,
                std::ptr::null(),
                std::ptr::null(),
                &mut data_in,
                state_ptr,
            );
            callback_trampoline(
                kHighsCallbackMipSolution,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                state_ptr,
            );
            callback_trampoline(
                kHighsCallbackMipLogging,
                c"hello".as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                state_ptr,
            );
            callback_trampoline(
                kHighsCallbackMipImprovingSolution,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                state_ptr,
            );
            callback_trampoline(
                kHighsCallbackMipGetCutPool,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                state_ptr,
            );
            callback_trampoline(
                9999,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                state_ptr,
            );
        }

        // Interrupt-check and solution events invoke the handler; logging,
        // improving-incumbent, cut-pool, and unknown events do not.
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "only interrupt-check and solution events should invoke the handler"
        );
        // The deferred interruption channel is never written.
        assert_eq!(data_in.user_interrupt, 0);
    }
}
