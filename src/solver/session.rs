//! Backend session traits and synchronization types.
//!
//! This module defines the primary `BackendSession` trait and its
//! supplementary bounded traits: `SessionHealth`, `SolutionView`,
//! `CallbackSession`, and `BackendMetadata`.
//!
//! # Design (per CONTEXT.md D1)
//!
//! Unlike the legacy `SolverAdapter` (a 13-method monolith), session
//! capabilities are decomposed into independently implementable traits.
//! Every backend MUST implement `BackendSession`. Supplementary traits
//! are optional — each backend declares which it supports.
//!
//! See `.planning/phases/10-backend-contract-migration-closure/10-CONTEXT.md`
//! for the full design rationale.

use crate::delta::DeltaBatch;
use crate::id::{ConId, VarId};
use crate::revision::ModelRevision;
use crate::snapshot::ModelSnapshot;
use crate::solver::backend::{BackendCapabilities, BackendError};
use crate::solver::callback::CallbackHandler;
use crate::solver::request::{SolveRequest, SolveResult};
use crate::sync::{AdapterCursor, AdapterHealth};

/// The type of synchronization to perform on a session.
///
/// Each variant carries the data the session needs to advance its
/// internal state.
pub enum Synchronization {
    /// Apply a delta batch (incremental replay).
    DeltaBatch(DeltaBatch),
    /// Rebuild from a full model snapshot.
    Rebuild(ModelSnapshot),
}

/// Receipt returned after a successful synchronization.
///
/// Confirms the adapter's new cursor position and health state.
pub struct SyncReceipt {
    /// Updated cursor reflecting the synchronization result.
    pub cursor: AdapterCursor,
    /// Health of the adapter after synchronization.
    pub health: AdapterHealth,
}

/// Primary session trait — every backend MUST implement this.
///
/// Provides the canonical lifecycle: synchronize state, solve, and
/// close the session, releasing any native resources.
pub trait BackendSession {
    /// Apply a delta batch or rebuild from a snapshot to synchronize state.
    fn synchronize(&mut self, sync: Synchronization) -> Result<SyncReceipt, BackendError>;

    /// Solve with the given request, returning structured result.
    fn solve(&mut self, request: &SolveRequest) -> Result<SolveResult, BackendError>;

    /// Close the session, releasing native resources.
    fn close(self) -> Result<(), BackendError>;
}

/// Optional trait — exposes adapter health and cursor position.
///
/// Most backends implement this. A backend that reports `Ready` health
/// and the current revision enables the coordinator to decide whether
/// delta replay or a full rebuild is needed.
pub trait SessionHealth {
    /// Current adapter health status.
    fn health(&self) -> AdapterHealth;

    /// The revision the adapter has applied.
    fn revision(&self) -> ModelRevision;
}

/// Optional trait — borrowed/indexed access to solution data.
///
/// Backends that expose solution data implement this trait. It provides
/// lookup methods over the most recent solve result without requiring
/// HashMap cloning.
pub trait SolutionView {
    /// Primal value of a variable, if available.
    fn value(&self, var: VarId) -> Option<f64>;

    /// Dual value of a constraint, if available.
    fn dual(&self, con: ConId) -> Option<f64>;

    /// Reduced cost of a variable, if available.
    fn reduced_cost(&self, var: VarId) -> Option<f64>;

    /// The objective value from the last solve, if available.
    fn objective_value(&self) -> Option<f64>;
}

/// Optional trait — for backends that support solver callbacks.
///
/// MIP-capable backends that support lazy constraints, cuts, or
/// solution inspection during branch-and-cut implement this trait.
pub trait CallbackSession {
    /// Register a callback handler to be invoked during solve.
    fn set_callback_handler(&mut self, handler: Box<dyn CallbackHandler>) -> Result<(), BackendError>;

    /// Clear the callback handler (no callbacks during next solve).
    fn clear_callback_handler(&mut self) -> Result<(), BackendError>;
}

/// Factory trait for creating backend sessions in parameterized tests.
///
/// Each backend provides a fixture implementation that creates fresh
/// sessions via [`new_session`] and reports its name via [`backend_name`].
/// The associated [`Session`](Self::Session) type must implement
/// [`BackendSession`].
pub trait BackendFixture {
    /// The session type this fixture creates.
    type Session: BackendSession;

    /// Create a new backend session.
    fn new_session(&self) -> Result<Self::Session, BackendError>;

    /// Human-readable backend name (for diagnostics).
    fn backend_name(&self) -> &str;
}

/// Optional trait — backends that expose identity and capability metadata.
pub trait BackendMetadata {
    /// Human-readable backend name (e.g., "HiGHS 1.9.0").
    fn name(&self) -> &str;

    /// Declared capabilities of this backend.
    fn capabilities(&self) -> BackendCapabilities;
}

#[cfg(test)]
mod tests {
    // Contract tests for session traits will be added in Plan 02.
}
