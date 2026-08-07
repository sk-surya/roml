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

use crate::compiler::backend_ir::{BackendDeltaBatch, BackendSnapshot};
use crate::compiler::capability::BackendCapabilitySet;
use crate::delta::DeltaBatch;
use crate::id::{ConId, VarId};
use crate::revision::ModelRevision;
use crate::snapshot::ModelSnapshot;
use crate::solver::backend::{BackendCapabilities, BackendError, ErrorCategory, HealthEffect};
use crate::solver::callback::CallbackHandler;
use crate::solver::infeasibility::{
    FeasibilityOracle, NativeConflict, NativeConflictRequest, SemanticConflictUniverse,
};
use crate::solver::overlay::{CompiledOverlay, OverlayApplyReceipt, OverlayRollbackOutcome};
use crate::solver::plan::{MipStart, VariableHints};
use crate::solver::request::{SolveRequest, SolveResult};
use crate::sync::{AdapterCursor, AdapterHealth};

/// The type of synchronization to perform on a session.
///
/// Each variant carries the data the session needs to advance its
/// internal state.
///
/// # Compiled synchronization (P26 Task 7, design §22)
///
/// M3 amends the advanced backend synchronization contract: the ordinary M2
/// path (the `SolverSession` façade and the HiGHS session) flows through
/// backend IR via [`CompiledRebuild`](Self::CompiledRebuild) /
/// [`CompiledDeltaBatch`](Self::CompiledDeltaBatch) — the compiler lowers
/// canonical snapshots/deltas into [`BackendSnapshot`]/[`BackendDeltaBatch`]
/// before any backend mutation (the P26 gate). The canonical
/// [`Rebuild`](Self::Rebuild)/[`DeltaBatch`](Self::DeltaBatch) variants remain
/// for the shared conformance suite and for backend authors who have not yet
/// migrated (SM-03.8 migration guide); the production HiGHS session handles
/// only the compiled variants.
pub enum Synchronization {
    /// Apply a delta batch (incremental replay).
    DeltaBatch(DeltaBatch),
    /// Rebuild from a full model snapshot.
    Rebuild(ModelSnapshot),
    /// Apply a compiled delta batch (backend IR incremental replay).
    CompiledDeltaBatch(BackendDeltaBatch),
    /// Rebuild from a compiled backend snapshot (backend IR).
    CompiledRebuild(BackendSnapshot),
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

    /// Spawn an isolated feasibility oracle from one exact compiled snapshot.
    ///
    /// The default is typed unsupported so existing backends remain valid while
    /// the Phase 29 capability is adopted. Implementations must not borrow or
    /// mutate the persistent solve session through the returned oracle.
    fn spawn_infeasibility_oracle(
        &self,
        _snapshot: &BackendSnapshot,
        _universe: &SemanticConflictUniverse,
    ) -> Result<Box<dyn FeasibilityOracle>, BackendError> {
        Err(BackendError::new(
            "this backend does not qualify an isolated LP feasibility oracle",
            ErrorCategory::Unsupported,
            HealthEffect::Recoverable,
        ))
    }

    /// Request native compiled-membership IIS evidence on an isolated session.
    ///
    /// The default is typed unsupported. Native evidence never carries a
    /// semantic irreducibility guarantee by itself.
    fn native_conflict(
        &self,
        _request: &NativeConflictRequest,
    ) -> Result<NativeConflict, BackendError> {
        Err(BackendError::new(
            "this backend does not qualify a native LP IIS provider",
            ErrorCategory::Unsupported,
            HealthEffect::Recoverable,
        ))
    }
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

/// Optional trait — transactional solve-overlay apply/rollback (P27 Task 10,
/// design §12).
///
/// A backend that supports reversible solve overlays implements this bounded
/// trait alongside [`SessionHealth`]/[`SolutionView`]. The lifecycle is
/// transactional from the caller's perspective (SM-07.4):
///
/// 1. [`apply_overlay`](Self::apply_overlay) transitions the backend compiled
///    state `C_base → C_overlay` and returns an explicit
///    [`OverlayApplyReceipt`];
/// 2. the solve runs against `C_overlay`;
/// 3. [`rollback_overlay`](Self::rollback_overlay) transitions back
///    `C_overlay → C_base` and reports an explicit
///    [`OverlayRollbackOutcome`] — a fallible rollback is NEVER delegated
///    solely to `Drop`;
/// 4. [`verify_overlay_clean`](Self::verify_overlay_clean) asserts the backend
///    canonical state is restored to `C_base` after a `Clean` rollback.
///
/// An uncertain rollback returns
/// [`OverlayRollbackOutcome::RequiresRebuild`] and marks the session
/// `RequiresRebuild` (D7 invariant, D22); the next solve forces a snapshot
/// rebuild before reuse.
pub trait OverlaySession {
    /// Apply a compiled overlay against the backend's exact base compiled
    /// state, transitioning `C_base → C_overlay`.
    ///
    /// # Errors
    ///
    /// Returns a [`BackendError`] when the overlay's `base_compilation` does
    /// not match the backend's current compiled state (rejected BEFORE any
    /// mutation), or when a native apply call fails (the backend then marks
    /// itself `RequiresRebuild` so a partially applied overlay is never
    /// silently reused).
    fn apply_overlay(
        &mut self,
        _overlay: &CompiledOverlay,
    ) -> Result<OverlayApplyReceipt, BackendError> {
        // Default-reject (review P2-01, SM-08.4): a backend that does not
        // qualify overlays needs no implementation and can never silently
        // ignore an overlay request — the executor's C_overlay path fails
        // with this typed Unsupported error before any native mutation.
        Err(BackendError::new(
            "this backend does not qualify overlay application",
            ErrorCategory::Unsupported,
            HealthEffect::Recoverable,
        ))
    }

    /// Roll back an applied overlay, transitioning `C_overlay → C_base`.
    ///
    /// Returns [`OverlayRollbackOutcome::Clean`] when the base is provably
    /// restored and [`OverlayRollbackOutcome::RequiresRebuild`] when the
    /// rollback could not be proven clean (the session is then marked
    /// `RequiresRebuild`).
    fn rollback_overlay(
        &mut self,
        _receipt: &OverlayApplyReceipt,
    ) -> Result<OverlayRollbackOutcome, BackendError> {
        Err(BackendError::new(
            "this backend does not qualify overlay rollback",
            ErrorCategory::Unsupported,
            HealthEffect::Recoverable,
        ))
    }

    /// Verify the backend's canonical compiled state is restored to the base
    /// after a `Clean` rollback.
    ///
    /// # Errors
    ///
    /// Returns a [`BackendError`] when the compiled maps / `current_compilation`
    /// do not match the base — the session is marked `RequiresRebuild`.
    fn verify_overlay_clean(&mut self) -> Result<(), BackendError> {
        Err(BackendError::new(
            "this backend does not qualify overlay verification",
            ErrorCategory::Unsupported,
            HealthEffect::Recoverable,
        ))
    }

    /// Apply qualified MIP starts to the backend's current compiled state
    /// (P28; SM-08.1, SM-08.4).
    ///
    /// The default implementation REJECTS with a typed `Unsupported`
    /// [`BackendError`]: a backend that does not qualify starts needs no
    /// change and can never silently ignore a start request. A backend that
    /// qualifies starts (per its [`BackendCapabilitySet`]) overrides this
    /// method and maps each start's user-`Variable` values through its
    /// compiled-keyed origin maps.
    ///
    /// # Errors
    ///
    /// Returns a typed [`BackendError`] by default
    /// ([`crate::solver::backend::ErrorCategory::Unsupported`],
    /// [`crate::solver::backend::HealthEffect::Recoverable`]); a qualifying
    /// backend returns an error when a start cannot be applied (e.g. an
    /// index/value the native API rejects).
    fn apply_mip_starts(&mut self, starts: &[MipStart]) -> Result<(), BackendError> {
        let _ = starts;
        Err(BackendError::new(
            "this backend does not qualify MIP starts",
            crate::solver::backend::ErrorCategory::Unsupported,
            crate::solver::backend::HealthEffect::Recoverable,
        ))
    }

    /// Apply qualified variable hints to the backend's current compiled state
    /// (P28; SM-08.3, SM-08.4).
    ///
    /// The default implementation REJECTS with a typed `Unsupported`
    /// [`BackendError`] — absent hints reject by default (the pinned HiGHS
    /// audit finds no hint API in the bundled version). A qualifying backend
    /// overrides this method.
    ///
    /// # Errors
    ///
    /// Returns a typed [`BackendError`] by default
    /// ([`crate::solver::backend::ErrorCategory::Unsupported`],
    /// [`crate::solver::backend::HealthEffect::Recoverable`]).
    fn apply_variable_hints(&mut self, hints: &VariableHints) -> Result<(), BackendError> {
        let _ = hints;
        Err(BackendError::new(
            "this backend does not qualify variable hints",
            crate::solver::backend::ErrorCategory::Unsupported,
            crate::solver::backend::HealthEffect::Recoverable,
        ))
    }
}

/// Optional trait — for backends that support solver callbacks.
///
/// MIP-capable backends that support lazy constraints, cuts, or
/// solution inspection during branch-and-cut implement this trait.
pub trait CallbackSession {
    /// Register a callback handler to be invoked during solve.
    fn set_callback_handler(
        &mut self,
        handler: Box<dyn CallbackHandler>,
    ) -> Result<(), BackendError>;

    /// Clear the callback handler (no callbacks during next solve).
    fn clear_callback_handler(&mut self) -> Result<(), BackendError>;
}

/// Factory trait for creating backend sessions in parameterized tests.
///
/// Each backend provides a fixture implementation that creates fresh
/// sessions via `new_session` and reports its name via `backend_name`.
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
    ///
    /// The legacy flat record (D27) — a source-compatible **derived compat
    /// view**, NOT authoritative. Compilation gating and request validation
    /// use [`typed_capabilities`](Self::typed_capabilities).
    fn capabilities(&self) -> BackendCapabilities;

    /// The backend's authoritative typed capability set (D10, SM-04.1, F3).
    ///
    /// The façade gates compilation and request validation on THIS view; the
    /// flat [`capabilities`](Self::capabilities) is a compat view derived from
    /// it. A backend whose typed view lacks a feature must never have that
    /// feature silently compiled or requested.
    fn typed_capabilities(&self) -> &BackendCapabilitySet;
}

#[cfg(test)]
mod tests {
    // Contract tests for session traits will be added in Plan 02.
}
