//! Callback bridge for HiGHS MIP solve events.
//!
//! Implements the [`CallbackSession`] trait by registering a C callback
//! trampoline with HiGHS that forwards solve events to ROML's
//! [`CallbackHandler`].
//!
//! Only officially supported callback types are handled:
//! - `kCallbackMipLogging` (informational)
//! - `kCallbackMipInterrupt` (user interrupt)
//! - `kCallbackMipSolution` (candidate solution)
//! - `kCallbackMipImprovingSolution` (improving incumbent)
//! - `kCallbackMipDefineLazyConstraints` (lazy constraint checking)

// TODO: implement in Plan 03