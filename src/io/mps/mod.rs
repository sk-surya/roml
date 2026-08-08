//! MPS import contract types.
//!
//! This module intentionally freezes the solver-free public seam used by the
//! MPS lexer, staging, resolution, and reader tasks. It does not implement
//! lexical parsing or model construction.

use crate::Model;

/// Selects the lexical layout used to interpret an MPS input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MpsFormat {
    /// Determine the layout by the P35 dual-interpretation policy.
    #[default]
    Auto,
    /// Interpret records using fixed-column MPS layout.
    Fixed,
    /// Interpret records using free-field MPS layout.
    Free,
}

/// Selects one named MPS rim vector or disables the vector class.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum MpsVectorSelection {
    /// Select the first vector encountered for the vector class.
    #[default]
    First,
    /// Select the vector with this exact name.
    Named(String),
    /// Do not apply any vector from this class.
    None,
}

/// Limits reserved for the streaming MPS reader.
///
/// Task 35-00 establishes this configuration seam only. Later lexical and
/// staging tasks enforce each limit with checked accounting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MpsResourceLimits {
    /// Maximum accepted line length in bytes.
    pub max_line_bytes: usize,
    /// Maximum number of input records.
    pub max_records: usize,
    /// Maximum number of declared rows.
    pub max_rows: usize,
    /// Maximum number of declared columns.
    pub max_columns: usize,
    /// Maximum number of staged matrix entries.
    pub max_nonzeros: usize,
}

impl Default for MpsResourceLimits {
    fn default() -> Self {
        Self {
            max_line_bytes: usize::MAX,
            max_records: usize::MAX,
            max_rows: usize::MAX,
            max_columns: usize::MAX,
            max_nonzeros: usize::MAX,
        }
    }
}

/// Configuration for an MPS import operation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MpsReadOptions {
    /// Requested fixed/free layout handling.
    pub format: MpsFormat,
    /// Selection for right-hand-side vectors.
    pub rhs: MpsVectorSelection,
    /// Selection for range vectors.
    pub ranges: MpsVectorSelection,
    /// Selection for bound vectors.
    pub bounds: MpsVectorSelection,
    /// Resource limits for streaming input and staging.
    pub limits: MpsResourceLimits,
}

/// Configured entry point for a future MPS read operation.
///
/// The reader is intentionally separate from [`Model`] so a future read
/// constructs a fresh canonical model rather than mutating a caller-owned
/// one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MpsReader {
    options: MpsReadOptions,
}

impl MpsReader {
    /// Creates a reader with deterministic default options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a reader with explicit import options.
    pub fn with_options(options: MpsReadOptions) -> Self {
        Self { options }
    }

    /// Returns the configured import options.
    pub fn options(&self) -> &MpsReadOptions {
        &self.options
    }
}

/// A source location within an MPS input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MpsSourceSpan {
    line: usize,
    start: usize,
    end: usize,
}

impl MpsSourceSpan {
    /// Creates a source span with parser-provided line and offset values.
    pub fn new(line: usize, start: usize, end: usize) -> Self {
        Self { line, start, end }
    }

    /// Returns the parser-provided source line.
    pub fn line(&self) -> usize {
        self.line
    }

    /// Returns the parser-provided span start offset.
    pub fn start(&self) -> usize {
        self.start
    }

    /// Returns the parser-provided span end offset.
    pub fn end(&self) -> usize {
        self.end
    }
}

/// Source context associated with an MPS diagnostic.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MpsDiagnostic {
    span: Option<MpsSourceSpan>,
    section: Option<String>,
}

impl MpsDiagnostic {
    /// Creates diagnostic context from an optional source span and section.
    pub fn new(span: Option<MpsSourceSpan>, section: Option<String>) -> Self {
        Self { span, section }
    }

    /// Returns the source span, when one is available.
    pub fn span(&self) -> Option<&MpsSourceSpan> {
        self.span.as_ref()
    }

    /// Returns the MPS section, when one is available.
    pub fn section(&self) -> Option<&str> {
        self.section.as_deref()
    }
}

/// Categories of failures defined by the P35 MPS reader contract.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MpsErrorKind {
    /// Input could not be read.
    Io,
    /// Input does not use the required encoding.
    InvalidEncoding,
    /// Sections appear in an invalid order.
    InvalidSectionOrder,
    /// A recognized but unsupported semantic section was encountered.
    UnsupportedSection {
        /// The recognized section name.
        section: String,
    },
    /// A record is malformed for its section.
    InvalidRecord,
    /// A numeric field is malformed or non-finite.
    InvalidNumber,
    /// A row name was declared more than once.
    DuplicateRow,
    /// A record refers to an undeclared row.
    UnknownRow,
    /// A record refers to an undeclared variable.
    UnknownVariable,
    /// Integer markers are unbalanced or incorrectly nested.
    InvalidMarkerNesting,
    /// A selected RHS vector contains duplicate row entries.
    DuplicateRhsEntry,
    /// A selected RANGES vector contains duplicate row entries.
    DuplicateRangeEntry,
    /// A selected RANGES entry targets an `N` row.
    InvalidRangeForNRow,
    /// A selected bound transition is invalid.
    InvalidBound,
    /// A selected range value is invalid.
    InvalidRange,
    /// A required section is absent.
    MissingRequiredSection,
    /// The input does not terminate with `ENDATA`.
    MissingEndata,
    /// An explicitly selected vector does not exist.
    UnknownVector,
    /// Automatic fixed/free interpretation is ambiguous.
    AmbiguousFormat,
    /// A model cannot be represented by the supported MPS dialect.
    RepresentationError,
    /// Constructing a fresh ROML model failed.
    ModelConstruction,
}

/// A typed, source-aware MPS import failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MpsError {
    kind: MpsErrorKind,
    diagnostic: MpsDiagnostic,
}

impl MpsError {
    /// Creates an MPS error from its category and source context.
    pub fn new(kind: MpsErrorKind, diagnostic: MpsDiagnostic) -> Self {
        Self { kind, diagnostic }
    }

    /// Returns the typed category of this failure.
    pub fn kind(&self) -> &MpsErrorKind {
        &self.kind
    }

    /// Returns the source context captured for this failure.
    pub fn diagnostic(&self) -> &MpsDiagnostic {
        &self.diagnostic
    }
}

impl std::fmt::Display for MpsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MPS import error: {:?}", self.kind)
    }
}

impl std::error::Error for MpsError {}

/// Non-semantic details recorded for a completed MPS import.
///
/// Later P35 tasks populate this type with deterministic import metadata.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MpsMetadata {
    _private: (),
}

/// Maps imported ROML entities and restrictions back to MPS source origins.
///
/// The map remains outside canonical [`Model`] state. Later P35 tasks add
/// explicit and synthetic provenance entries after semantic resolution.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MpsSourceMap {
    _private: (),
}

/// The result of successfully importing one MPS document into a fresh model.
pub struct MpsImport {
    /// The freshly constructed canonical ROML model.
    pub model: Model,
    /// Deterministic import metadata.
    pub metadata: MpsMetadata,
    /// MPS source provenance separate from canonical model state.
    pub source_map: MpsSourceMap,
    /// Non-fatal diagnostics emitted during import.
    pub diagnostics: Vec<MpsDiagnostic>,
}
