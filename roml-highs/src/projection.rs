//! Model projection — snapshot-to-HiGHS rebuild and delta application.
//!
//! Converts [`ModelSnapshot`] and [`ModelOp`] variants into HiGHS C API
//! operations, managing index bookkeeping alongside the process.

// TODO: implement in Plan 02