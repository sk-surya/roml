//! HiGHS-native error construction helpers for `BackendError`.
//!
//! Provides convenience construction and status-checking functions that
//! convert HiGHS C API return codes into the frozen `BackendError` type
//! using appropriate `ErrorCategory` and `HealthEffect` values.

use std::ffi::c_void;

use crate::bindings::{HighsInt, STATUS_OK};
use roml::solver::backend::{BackendError, ErrorCategory, HealthEffect};

/// Convenience alias for the frozen `BackendError` type.
///
/// Callers use `HighsError` to refer to HiGHS-specific errors without
/// importing the full roml::solver::backend path.
pub type HighsError = BackendError;

/// Check a HiGHS return code and convert non-OK results to `BackendError`.
///
/// Returns `Ok(())` if `ret == STATUS_OK`. Otherwise constructs a
/// `BackendError` with `HealthEffect::Recoverable` (callers that need a
/// different effect should elevate the error).
pub fn check_highs_status(
    ret: HighsInt,
    _highs_ptr: *mut c_void,
    op: &str,
) -> Result<(), BackendError> {
    if ret == STATUS_OK {
        return Ok(());
    }
    Err(from_native_status(ret, op))
}

/// Construct a `BackendError` from a HiGHS operation's native return code.
///
/// Uses the return code of the failed HiGHS operation as the
/// `native_code` in the error. The error category defaults to `Internal`
/// since non-zero return codes from HiGHS C API functions indicate
/// internal solver errors or invalid inputs.
pub fn from_native_status(ret: HighsInt, op: &str) -> BackendError {
    BackendError::with_code(
        format!("HiGHS operation '{}' failed with status {}", op, ret),
        ErrorCategory::Internal,
        HealthEffect::Recoverable,
        ret,
    )
}
