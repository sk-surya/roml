//! Advanced backend-extension surface (API-07.3, D9).
//!
//! Ordinary model authors should not need anything in this module — the golden
//! path lives in [`crate::prelude`]. Framework and backend authors
//! (implementing [`BackendSession`] against a native solver) use the types
//! grouped here: the frozen backend contract, revisions, snapshots, delta
//! batches, adapter cursors, capabilities, callbacks, raw entity IDs, and
//! expression/constraint internals.
//!
//! These types are deliberately ABSENT from the default prelude (API-07.2):
//! exposing the incremental-synchronization protocol by default would make the
//! API's cognitive load depend on implementation mechanics rather than on
//! user concepts.
//!
//! # Stability and semver expectations
//!
//! This is a pre-1.0 surface (crate version 0.1.x). Items grouped here may
//! change between minor versions as the protocol evolves; no guarantees of
//! stability are made until 1.0.0. The one exception is the
//! [`BackendSession`] contract, which is frozen for the M2 milestone
//! (API-08.4) — any change to it requires an ADR amendment.
//!
//! # Reference implementations
//!
//! [`crate::solver::reference::ReferenceBackend`] and the
//! [`crate::solver::conformance`] suite are the executable specification for
//! backend authors. `roml-highs` is the production reference implementation.

// Re-exports: backend contract, revisions, snapshots, deltas, cursors,
// capabilities, callbacks, raw IDs, and expression internals.

/// Backend session contract and synchronization types.
pub use crate::solver::session::{
    BackendFixture, BackendMetadata, BackendSession, CallbackSession, SessionHealth, SolutionView,
    SyncReceipt, Synchronization,
};

/// Typed delta batches and model operations.
pub use crate::delta::{DeltaBatch, ModelOp};

/// Monotonic model revisions.
pub use crate::revision::{ModelRevision, RevisionError};

/// Deterministic model snapshots and their entry types.
pub use crate::snapshot::{
    take_snapshot, CellEntry, ConstraintEntry, ModelSnapshot, ObjectiveEntry, ParameterEntry,
    VariableEntry,
};

/// Canonical semantic snapshot/delta entries (P25).
pub use crate::function::FunctionEntry;

/// Adapter cursors, health, and synchronization coordination.
pub use crate::sync::{AdapterCursor, AdapterHealth, ApplyError, ApplyOutcome, SyncCoordinator};

/// Backend metadata, capabilities, and categorized errors.
pub use crate::solver::backend::{
    BackendCapabilities, BackendError, BackendInfo, ErrorCategory, HealthEffect, TerminationStatus,
};

/// MIP callback contract for backend authors.
pub use crate::solver::callback::{CallbackAction, CallbackCut, CallbackData, CallbackHandler};

/// Immutable solve-request/result protocol.
pub use crate::solver::request::{
    validate_request, ConfigAdjustment, ConfigRejection, EffectiveConfig, SolveRequest,
    SolveResult, SolveSolution,
};

/// Solve-policy enum and legacy solver error/status (compatibility).
pub use crate::solver::{LpAlgorithm, SolverError, SolverStatus};

pub use crate::solution::metadata::{SolveMetadata, SynchronizationMode};
/// Result construction/storage internals for frameworks.
pub use crate::solution::{SolutionBuilder, SolutionStore};

/// The model's change journal (raw event stream).
pub use crate::model::changelog::Change;

/// Raw opaque entity IDs and the generation counter.
pub use crate::id::{CoeffId, ConId, Generation, ObjId, ParamId, VarId};

pub use crate::model::coefficient::{CellKey, CoefficientData, CoefficientTarget};
/// Raw constraint-bounds form and sparse coefficient cell internals.
pub use crate::model::constraint::ConstraintBounds;

/// Persistent parameter-dependent coefficient expressions.
pub use crate::value_expr::ValueExpr;

/// Reference projection backend and conformance suite (executable spec).
pub use crate::solver::conformance;
pub use crate::solver::reference;

/// Compiler backend IR, exact compilation identity, origins, capabilities,
/// reports, and the identity compiler (P26). Compiler internals are
/// deliberately absent from the ordinary prelude (SM-03.x / API-07.2);
/// framework and backend authors reach them here.
pub use crate::compiler::{
    backend_ir::{
        BackendConstraint, BackendDeltaBatch, BackendOp, BackendSnapshot, BackendSnapshotBuilder,
        CompilationId, CompiledConstraintId, CompiledEntityRef, CompiledEntityRegistry,
        CompiledLinearRow, CompiledObjective, CompiledObjectiveId, CompiledObjectiveLevel,
        CompiledObjectivePolicy, CompiledVariable, CompiledVariableId, CompiledWeightedObjective,
        RecipeFingerprint,
    },
    capability::{
        BackendCapabilitySet, BackendFeature, CompilationPolicy, FeatureLimitations,
        FeatureSupport, SupportLevel,
    },
    origin::{EntityOrigin, GeneratedRole, OriginMap, OverlayId},
    report::{BackendIdentity, CompilationReport, FormulationDecision},
    session::CompilationSession,
    CompileError,
};

/// Deterministic interval bound analysis and one-sided Big-M helpers (P32
/// Task 15; design §9, SM-13). The construct-aware `UnboundedBigM` marker is
/// crate-private — bridges surface it as `CompileError::UnboundedBigM`.
pub use crate::compiler::bounds::{
    BigMImplication, BigMRequest, BoundAnalyzer, BoundError, BoundSource, BoundTrace, Interval,
};

/// Bridge contract and finalizer (P32 Task 15; design §8.5, SM-13.5).
pub use crate::compiler::bridge::{
    BridgeDependency, BridgeFinalizer, BridgeOutput, BridgeRepresentation,
};

/// Canonical semantic construct payloads and kinds (P32 Task 16; design §7).
///
/// The `Fixture` variant and [`crate::construct::FixturePayload`] are
/// `#[cfg(test)]`-gated test-only scaffolding (A30) and never appear here.
pub use crate::construct::{
    AbsoluteValueConstraint, AbsoluteValueVariant, BinaryProductConstraint, BooleanConstraint,
    BooleanKind, CardinalityConstraint, CardinalityKind, ConstructEntry, ConstructKind,
    IndicatorConstraint, IndicatorDirection, MinMaxConstraint, MinMaxRelation, MinMaxSense,
    ProductOperand, ReificationConstraint,
};
