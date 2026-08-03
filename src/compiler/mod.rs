//! Compiler boundary: backend IR, exact compilation identity, origins, and
//! structured compilation reports (design §8, §19; P26 Task 5).
//!
//! P26 establishes the compiler foundation: the backend IR types
//! ([`backend_ir`]), the mandatory origin mapping ([`origin`]), and the
//! compilation report ([`report`]). The capability registry (`capability`)
//! and the identity compiler (`session`) land in Tasks 6 and 7; this module
//! declares their shared error surface ([`CompileError`]) now.
//!
//! Compiler internals are deliberately NOT part of the ordinary prelude
//! (SM-03.x / API-07.2): framework and backend authors reach them through
//! [`crate::advanced`].

pub mod backend_ir;
pub mod origin;
pub mod report;

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
    /// lands in Task 6); unqualified features are rejected, never silently
    /// ignored (SM-04.4).
    UnsupportedFeature(String),

    /// The compiled objective policy references an objective that was not
    /// compiled into the snapshot (design §8.4).
    ///
    /// `Single(id)` / `Weighted` / `Lexicographic` must reference a compiled
    /// objective that actually exists; a dangling id is a broken backend
    /// snapshot.
    InvalidObjectivePolicy(CompiledObjectiveId),

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
            Self::IdentityOverflow => write!(f, "identity counter exhausted (ids never wrap)"),
        }
    }
}

impl std::error::Error for CompileError {}
