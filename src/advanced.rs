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
    BackendFixture, BackendMetadata, BackendSession, CallbackSession, OverlaySession,
    SessionHealth, SolutionView, SyncReceipt, Synchronization,
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

/// Declared variable domain and persistent fixing state (P27 Task 8,
/// SM-05.1). Backend authors matching `ModelOp::SetVariableFixing` reach the
/// fixing record here alongside the delta/change surface.
pub use crate::model::variable::{FixingProvenance, SemiDomain, VariableDomain, VariableFixing};

/// Primal assignments and solution locks (P27 Task 9, SM-06). The neutral
/// partial value map and the solve-scoped lock types are canonical model
/// surface (design §11); backend authors consume them through the overlay
/// compiler below.
pub use crate::assignment::{
    AssignmentError, ContinuousLock, LockSelector, PrimalAssignment, SolutionLock,
};

/// Solve overlay contract, compiler, and transactional apply/rollback receipts
/// (P27 Task 9 types + compiler, issue #26 item 1; P27 Task 10 execution). The
/// overlay packet shapes are canonical solve surface; the compiled overlay,
/// its operations, and the explicit apply/rollback receipts are the
/// backend-facing forms Task 10 executes.
pub use crate::solver::overlay::{
    compile_overlay, CompiledOverlay, CutoffDirection, ObjectiveCutoff, ObjectiveLock,
    OverlayApplyReceipt, OverlayError, OverlayOp, OverlayRollbackOutcome, SolveOverlay,
};

/// Solve plan, warm-start, and unsupported/conversion policy types (P28
/// Task 1; SM-07.1, SM-08). Backend authors gate warm-start application on
/// these; the plan executor and the `OverlaySession` warm-start methods
/// consume them.
pub use crate::solver::plan::{
    HintPriority, LexStagePolicy, MipStart, ObjectivePolicy, PlanError, RepairPolicy, SolvePlan,
    UnsupportedFeaturePolicy, VariableHint, VariableHints,
};

/// Effective solve plan and feature recording (P28 Task 2; SM-04.5, SM-07.7).
/// Backend authors consume these when contributing applied/converted/rejected
/// records to the `EffectiveSolvePlan` the façade returns.
pub use crate::solver::effective_plan::{
    AppliedFeature, EffectiveSolvePlan, ObjectiveStageResult, PlanAdjustment, PlanRejection,
};

/// LP infeasibility analysis contracts for backend authors.
pub use crate::solver::infeasibility::{
    classify_feasibility, AnalysisBudget, AnalysisCompletion, AnalysisNumericalPolicy,
    AnalysisProviderRecord, AnalysisWarning, CandidateUniverseSummary, CompiledRestrictionRef,
    ConflictAtomId, ConflictGrouping, ConflictGuarantee, ConflictMember, FeasibilityEvidence,
    FeasibilityOracle, FeasibilityOutcome, FeasibilityProofStrength, InfeasibilityError,
    InfeasibilityMode, InfeasibilityPlan, InfeasibilityReport, InfeasibilityScope,
    InfeasibilityStatistics, MarkdownInfeasibilityReport, NativeConflict, NativeConflictEvidence,
    NativeConflictRequest, ReductionPolicy, RestrictionSelection, SeedPolicy,
    SemanticConflictUniverse, SemanticRestrictionAtom, TextInfeasibilityReport, UnknownReason,
};

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
/// The `Fixture` variant and `crate::construct::FixturePayload` are
/// `#[cfg(test)]`-gated test-only scaffolding (A30) and never appear here.
pub use crate::construct::{
    AbsoluteValueConstraint, AbsoluteValueVariant, BinaryProductConstraint, BooleanConstraint,
    BooleanKind, CardinalityConstraint, CardinalityKind, ConstructEntry, ConstructKind,
    ExtrapolationPolicy, IndicatorConstraint, IndicatorDirection, MinMaxConstraint, MinMaxRelation,
    MinMaxSense, PiecewiseLinearConstraint, ProductOperand, PwlCurvature, PwlPoint, PwlRelation,
    ReificationConstraint,
};
