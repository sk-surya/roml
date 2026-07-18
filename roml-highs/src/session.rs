//! `BackendSession` trait implementation for HiGHS.
//!
//! Thin delegation layer that routes `synchronize()`, `solve()`, and
//! `close()` to the projection, solution, and lifecycle modules.

// TODO: implement in Plan 03