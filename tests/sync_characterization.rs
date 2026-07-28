#![allow(deprecated)]
//! Characterization tests for the current model ⇔ solver synchronization behavior.
//!
//! These tests use fake adapters that record applied operations and can be
//! configured to fail after operation `k`. They prove four distinct weaknesses
//! in the current architecture — all rooted in the destructive changelog:
//!
//! 1. **Drained changes disappear on error** — `drain_changes()` is destructive:
//!    if the adapter returns an error, the changes are gone with no replay.
//! 2. **Second adapter cannot observe consumed changes** — one changelog cannot
//!    serve multiple sessions; the second adapter gets an empty batch.
//! 3. **Partial application leaves no recovery path** — an adapter that fails
//!    mid-`apply_changes` has no deterministic recovery: model changes are gone
//!    and the adapter is partially mutated.
//! 4. **Reset/rebuild not tied to revision** — `SolverAdapter::reset()` wipes
//!    adapter state but there is no versioned check to determine whether a
//!    rebuild would reproduce the current model state.
//!
//! All tests were removed alongside the `SolverAdapter` and `SolverModelExt`
//! traits (removed in PR A — core protocol). The revisioned sync architecture
//! (BackendSession + SyncCoordinator + Journal) resolves all four weaknesses.
//! See `tests/semicontinuous_recovery.rs` for protocol-level proofs using
//! ReferenceBackend and SyncCoordinator.
