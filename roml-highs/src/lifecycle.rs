//! HiGHS session lifecycle — construction, ownership, Drop, and thread-safety.
//!
//! The [`HighsSession`] struct owns an opaque HiGHS instance handle
//! (`*mut c_void` from `Highs_create`) and manages its lifecycle:
//!
//! - **Construction:** [`HighsSession::try_new`] creates the HiGHS instance,
//!   validates the index width is 32-bit, caches the infinity value, and
//!   returns `Result` — never panics.
//! - **Destruction:** [`Drop`] calls `Highs_destroy` with a null-pointer guard.
//! - **Thread-safety:** `Send` is implemented with documented safety invariants;
//!   `Sync` is NOT implemented because HiGHS is not thread-safe.

use std::collections::HashMap;
use std::ffi::c_void;

use log::info;

use crate::bindings;
use crate::index_map::IndexMap;
use roml::id::{ConId, ObjId, VarId};
use roml::model::objective::Sense;
use roml::solver::backend::BackendError;
use roml::sync::AdapterCursor;

/// A HiGHS solver session.
///
/// Owns an opaque HiGHS instance handle and manages index mappings,
/// cached bounds, and objective state. Construction is fallible (returns
/// `Result`); the session destroys the handle in `Drop`.
pub struct HighsSession {
    /// Opaque HiGHS instance handle from `Highs_create`.
    raw: *mut c_void,

    /// Revision tracking cursor for the sync coordinator.
    cursor: AdapterCursor,

    /// `VarId` → HiGHS column index.
    col_map: IndexMap<VarId>,

    /// `ConId` → HiGHS row index.
    row_map: IndexMap<ConId>,

    /// Cached infinity value from `Highs_getInfinity`.
    inf: f64,

    /// Cached variable bounds (lb, ub).
    var_bounds: HashMap<VarId, (f64, f64)>,

    /// Cached constraint bounds (lb, ub).
    con_bounds: HashMap<ConId, (f64, f64)>,

    /// Per-objective stored costs: `ObjId` → `VarId` → cost.
    obj_costs: HashMap<ObjId, HashMap<VarId, f64>>,

    /// Sense for each known objective.
    obj_senses: HashMap<ObjId, Sense>,

    /// Currently active objective, if any.
    active_obj: Option<ObjId>,
}

impl HighsSession {
    /// Create a new HiGHS session, validating the instance at construction.
    ///
    /// Steps:
    /// 1. Call `Highs_create()` to obtain an opaque handle.
    /// 2. Validate that HiGHS was compiled with 32-bit `HighsInt`.
    /// 3. Cache the infinity value from `Highs_getInfinity`.
    /// 4. Log the HiGHS version string for diagnostics.
    ///
    /// # Errors
    ///
    /// Returns `BackendError` if:
    /// - `Highs_create()` returns a null pointer (library not found or
    ///   resource exhaustion).
    /// - `Highs_getSizeofHighsInt` returns a value other than 4 (64-bit
    ///   indexing is not supported).
    pub fn try_new() -> Result<Self, BackendError> {
        // SAFETY: `Highs_create` allocates a new HiGHS instance. The
        // returned pointer is opaque — we must check for null before
        // using it. The handle is exclusively owned by this session and
        // destroyed in `Drop`.
        let raw = unsafe { bindings::Highs_create() };

        if raw.is_null() {
            return Err(BackendError::library_not_found(
                "Highs_create returned null — HiGHS library not found or resource exhaustion",
            ));
        }

        // Validate 32-bit index width.
        // SAFETY: `raw` has been verified non-null. `Highs_getSizeofHighsInt`
        // reads an internal field and does not mutate HiGHS state.
        let sz = unsafe { bindings::Highs_getSizeofHighsInt(raw) };
        if sz != 4 {
            // Destroy the handle before returning — we own it exclusively.
            // SAFETY: `raw` is the handle we just created. No other code
            // holds a reference. After this call the handle is invalid.
            unsafe { bindings::Highs_destroy(raw) }
            return Err(BackendError::unsupported(format!(
                "64-bit HighsInt not supported (Highs_getSizeofHighsInt = {})",
                sz
            )));
        }

        // Cache the infinity value.
        // SAFETY: `raw` is valid (verified non-null, valid handle).
        // `Highs_getInfinity` reads a configuration value and does not
        // mutate HiGHS state.
        let inf = unsafe { bindings::Highs_getInfinity(raw) };

        // Log the HiGHS version string for diagnostic purposes.
        // `Highs_version()` is a static function that does not require
        // an instance handle.
        // SAFETY: `Highs_version()` returns a pointer to a static C string
        // that is always valid and null-terminated.
        let version_str = unsafe {
            let c_str = bindings::Highs_version();
            std::ffi::CStr::from_ptr(c_str)
        };
        info!(
            "HiGHS session created: {}",
            version_str.to_string_lossy()
        );

        info!(
            "HiGHS session initialised (HighsInt size = {}, inf = {})",
            sz, inf
        );

        Ok(Self {
            raw,
            cursor: AdapterCursor::new(),
            col_map: IndexMap::new(),
            row_map: IndexMap::new(),
            inf,
            var_bounds: HashMap::new(),
            con_bounds: HashMap::new(),
            obj_costs: HashMap::new(),
            obj_senses: HashMap::new(),
            active_obj: None,
        })
    }

    /// Create a new HiGHS session, panicking on failure.
    ///
    /// # Panics
    ///
    /// Panics if `Highs_create()` returns null or if index-width
    /// validation fails. Use [`try_new`](Self::try_new) for fallible
    /// construction.
    pub fn new_unchecked() -> Self {
        Self::try_new().expect("HighsSession::new_unchecked: HiGHS initialisation failed")
    }

    /// Access the raw HiGHS instance pointer for internal use.
    ///
    /// This is a `pub(crate)` accessor used by projection, session,
    /// and solution modules to call HiGHS C API functions.
    #[inline]
    pub(crate) fn raw_ptr(&self) -> *mut c_void {
        self.raw
    }

    /// The cached infinity value from `Highs_getInfinity`.
    #[inline]
    pub(crate) fn infinity(&self) -> f64 {
        self.inf
    }
}

// SAFETY: HighsSession owns the `*mut c_void` handle exclusively.
// 1. The handle is created in try_new() and destroyed in Drop — no other
//    code creates or frees it.
// 2. No internal references to the handle escape the session (all C API
//    calls are internal, passing `raw` back to HiGHS functions).
// 3. Callbacks are set up and torn down within solve() — they do not
//    survive session moves.
// Moving the session to another thread is safe because no other thread
// holds a reference to the handle. However, calling HiGHS functions
// concurrently on the same handle from multiple threads is UB — Sync
// is NOT implemented.
unsafe impl Send for HighsSession {}

impl Drop for HighsSession {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            // SAFETY:
            // - `self.raw` was created by `Highs_create` in `try_new()` and
            //   is a valid HiGHS instance handle.
            // - No other code holds a reference to this handle (exclusive
            //   ownership is enforced by the type system).
            // - After this call, no further C API calls are made using
            //   this handle — the session is being dropped.
            // - The null check prevents double-free: if `raw` has been set
            //   to null (e.g., after early-exit error handling), the
            //   destroy call is skipped.
            unsafe { bindings::Highs_destroy(self.raw) }
        }
    }
}
