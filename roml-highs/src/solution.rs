//! Solution extraction and status mapping.
//!
//! Maps HiGHS model status and run status to [`TerminationStatus`] and
//! extracts solution data (primal values, duals, reduced costs) from
//! a HiGHS instance after a solve.

// TODO: implement in Plan 03