//! Compiler boundary: backend IR, exact compilation identity, origins,
//! capabilities, and structured compilation reports (design §8, §19;
//! P26 Tasks 5-6).
//!
//! P26 establishes the compiler foundation: the backend IR types
//! ([`backend_ir`]), the mandatory origin mapping ([`origin`]), the typed
//! capability registry ([`capability`]), the compilation report
//! ([`report`]), and the identity compiler ([`session`]) that lowers
//! canonical snapshots/deltas into backend IR (Task 7). This module also
//! declares the shared error surface ([`CompileError`]).
//!
//! Compiler internals are deliberately NOT part of the ordinary prelude
//! (SM-03.x / API-07.2): framework and backend authors reach them through
//! [`crate::advanced`].

pub mod backend_ir;
pub mod capability;
pub mod origin;
pub mod report;
pub mod session;

use backend_ir::{CompilationId, CompiledEntityRef, CompiledObjectiveId};

/// A typed compilation/bridge failure (design §19, SM-13 foundations).
///
/// P26 declares the variants the identity compiler needs immediately: missing
/// origin rejection at builder finalization, stale-compilation and
/// unsupported-feature rejections (exercised by Task 7), objective-policy
/// validation, and identity-counter exhaustion. Later phases add the
/// bridge/Big-M variants (`UnboundedBigM`, ...) defined by the design.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompileError {
    /// A generated compiled entity has no recorded origin (D5, SM-02.5).
    ///
    /// Builder finalization rejects any compiled entity without an
    /// `OriginMap` entry — no generated entity is finalized without an
    /// origin (the P26 must-have truth 5 and Task 5 stopping condition).
    MissingOrigin {
        /// The compiled entity with no recorded origin.
        entity: CompiledEntityRef,
    },

    /// A compilation was requested against a stale source compilation.
    ///
    /// The exact `CompilationId` is the only stale-state authority (D28,
    /// SM-03.9): a delta whose `from_compilation` does not match the
    /// backend's current compiled state is rejected, never silently applied.
    StaleCompilation {
        /// The compilation id the delta expected to follow.
        expected: CompilationId,
        /// The compilation id the backend actually holds.
        actual: CompilationId,
    },

    /// The target backend lacks a required capability/feature.
    ///
    /// P26 uses a feature-name string (the typed `BackendFeature` registry
    /// lives in [`capability`]); unqualified features are rejected, never
    /// silently ignored (SM-04.4).
    UnsupportedFeature(String),

    /// The compiled objective policy references an objective that was not
    /// compiled into the snapshot (design §8.4).
    ///
    /// `Single(id)` / `Weighted` / `Lexicographic` must reference a compiled
    /// objective that actually exists; a dangling id is a broken backend
    /// snapshot.
    InvalidObjectivePolicy(CompiledObjectiveId),

    /// A compiled-IR reference points at an entity that does not exist in the
    /// target compiled state (F5): a row/objective coefficient references a
    /// compiled variable absent from the snapshot/registry, or a delta op
    /// references an unknown compiled entity. Malformed backend IR is a typed
    /// error — never a silent skip.
    InvalidReference {
        /// The referenced compiled entity that could not be resolved.
        entity: CompiledEntityRef,
    },

    /// A compiled entity was specified more than once (F1/F2): a duplicate
    /// `Add*` op within a delta batch, or a duplicate compiled id across a
    /// snapshot's collections. Compiled ids are dense and unique by
    /// construction (SM-02.4); a duplicate is malformed backend IR.
    DuplicateEntity {
        /// The duplicated compiled entity.
        entity: CompiledEntityRef,
    },

    /// A compiled snapshot's ids are not dense (F2): the design's
    /// deterministic dense allocation (`0..len` per family, SM-02.4) is
    /// violated by a gap or an id beyond the count.
    NonDenseCompilation {
        /// The compiled entity whose id breaks density.
        entity: CompiledEntityRef,
    },

    /// The delta could not be proven incrementally equivalent — a
    /// deterministic rebuild is required (design §18, D22).
    ///
    /// P26 (Task 7) returns this for any delta containing an op the identity
    /// compiler cannot lower exactly (variable/constraint activity changes,
    /// variable-type changes, parameter updates, semi-continuous bounds, and
    /// semantic construct ops), per the Task 0 acceptance record F-B1. No
    /// `BackendDeltaBatch` is emitted; the caller falls back to one compiled
    /// snapshot rebuild.
    RebuildRequired(String),

    /// An opaque identity counter was exhausted (ids never wrap).
    ///
    /// Mirrors [`crate::IdentityOverflow`]: checked atomic allocation
    /// saturates and reports this error rather than re-issuing ids.
    IdentityOverflow,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingOrigin { entity } => {
                write!(f, "generated entity has no recorded origin: {entity:?}")
            }
            Self::StaleCompilation { expected, actual } => write!(
                f,
                "stale compilation: expected {expected:?}, backend holds {actual:?}"
            ),
            Self::UnsupportedFeature(feature) => {
                write!(f, "unsupported backend feature: {feature}")
            }
            Self::InvalidObjectivePolicy(id) => write!(
                f,
                "objective policy references a non-compiled objective: {id:?}"
            ),
            Self::InvalidReference { entity } => {
                write!(f, "compiled IR references an unknown entity: {entity:?}")
            }
            Self::DuplicateEntity { entity } => {
                write!(f, "duplicate compiled entity: {entity:?}")
            }
            Self::NonDenseCompilation { entity } => {
                write!(f, "compiled ids are not dense (gap/overflow at {entity:?})")
            }
            Self::RebuildRequired(reason) => {
                write!(
                    f,
                    "rebuild required (delta not incrementally equivalent): {reason}"
                )
            }
            Self::IdentityOverflow => write!(f, "identity counter exhausted (ids never wrap)"),
        }
    }
}

impl std::error::Error for CompileError {}
