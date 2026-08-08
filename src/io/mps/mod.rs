//! MPS import contract types.
//!
//! This module intentionally freezes the solver-free public seam used by the
//! MPS lexer, staging, resolution, and reader tasks. It does not implement
//! lexical parsing or model construction.

use std::{io, path::PathBuf};

use crate::Model;

#[allow(dead_code)]
mod staging;
#[allow(dead_code)]
mod vectors;

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
    /// Maximum number of physical, non-comment input records.
    ///
    /// A COLUMNS, RHS, or RANGES record with two pairs counts once; metadata
    /// and marker records count even when they carry no staged payload.
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

    /// Attempts to import an MPS document from a buffered stream.
    ///
    /// Task 35-00 exposes this entry point solely so malformed-input harnesses
    /// exercise production code. It deliberately does not inspect `input` or
    /// implement lexical semantics; until Task 35-05 connects the completed
    /// parser and resolver, every call returns
    /// [`MpsErrorKind::ParserUnavailable`].
    pub fn read<R: io::BufRead>(&self, _input: R) -> Result<MpsImport, MpsError> {
        Err(MpsError::new(
            MpsErrorKind::ParserUnavailable,
            MpsDiagnostic::new().with_message(
                "MPS parsing is deferred; this Task 35-00 reader seam does not inspect input",
            ),
        ))
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
    /// Creates a validated source span.
    ///
    /// Lines are one-based. Offsets are zero-based byte offsets within that
    /// line, expressed as a half-open range `[start, end)`. Empty spans are
    /// valid when `start == end`.
    pub fn try_new(line: usize, start: usize, end: usize) -> Result<Self, MpsSourceSpanError> {
        if line == 0 {
            return Err(MpsSourceSpanError::ZeroLine);
        }
        if end < start {
            return Err(MpsSourceSpanError::ReversedOffsets { start, end });
        }
        Ok(Self { line, start, end })
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

/// Failure to construct a valid [`MpsSourceSpan`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MpsSourceSpanError {
    /// Source lines are one-based and cannot be zero.
    ZeroLine,
    /// A half-open span has an end before its start.
    ReversedOffsets {
        /// The supplied start offset.
        start: usize,
        /// The supplied end offset.
        end: usize,
    },
}

impl std::fmt::Display for MpsSourceSpanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroLine => write!(f, "MPS source lines are one-based"),
            Self::ReversedOffsets { start, end } => {
                write!(
                    f,
                    "MPS source span ends at {end} before it starts at {start}"
                )
            }
        }
    }
}

impl std::error::Error for MpsSourceSpanError {}

/// Identifies the input associated with an MPS diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MpsInputSource {
    /// A filesystem path supplied to a future path-based reader.
    Path(PathBuf),
    /// A caller-supplied label for a stream input.
    Label(String),
}

impl std::fmt::Display for MpsInputSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Path(path) => write!(f, "{}", path.display()),
            Self::Label(label) => write!(f, "{label}"),
        }
    }
}

/// A recognized MPS section.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MpsSection {
    /// `NAME`.
    Name,
    /// `OBJSENSE`.
    ObjSense,
    /// `OBJNAME`.
    ObjName,
    /// `ROWS`.
    Rows,
    /// `COLUMNS`.
    Columns,
    /// `RHS`.
    Rhs,
    /// `RANGES`.
    Ranges,
    /// `BOUNDS`.
    Bounds,
    /// `ENDATA`.
    Endata,
    /// `QMATRIX`.
    QMatrix,
    /// `QSECTION`.
    QSection,
    /// `QUADOBJ`.
    QuadObj,
    /// `QCMATRIX`.
    QCMatrix,
    /// `CSECTION`.
    CSection,
    /// `SOS`.
    Sos,
    /// `INDICATORS`.
    Indicators,
    /// `PWLOBJ`.
    PwlObj,
    /// `LAZYCONS`.
    LazyCons,
    /// `USERCUTS`.
    UserCuts,
    /// A vendor or otherwise unrecognized section name.
    Other(String),
}

impl std::fmt::Display for MpsSection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Name => "NAME",
            Self::ObjSense => "OBJSENSE",
            Self::ObjName => "OBJNAME",
            Self::Rows => "ROWS",
            Self::Columns => "COLUMNS",
            Self::Rhs => "RHS",
            Self::Ranges => "RANGES",
            Self::Bounds => "BOUNDS",
            Self::Endata => "ENDATA",
            Self::QMatrix => "QMATRIX",
            Self::QSection => "QSECTION",
            Self::QuadObj => "QUADOBJ",
            Self::QCMatrix => "QCMATRIX",
            Self::CSection => "CSECTION",
            Self::Sos => "SOS",
            Self::Indicators => "INDICATORS",
            Self::PwlObj => "PWLOBJ",
            Self::LazyCons => "LAZYCONS",
            Self::UserCuts => "USERCUTS",
            Self::Other(name) => name,
        };
        write!(f, "{name}")
    }
}

/// Source context associated with an MPS diagnostic.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MpsDiagnostic {
    input_source: Option<MpsInputSource>,
    span: Option<MpsSourceSpan>,
    section: Option<MpsSection>,
    raw_field: Option<String>,
    entity: Option<String>,
    message: Option<String>,
}

impl MpsDiagnostic {
    /// Creates empty diagnostic context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Attaches the input source to this diagnostic.
    pub fn with_input_source(mut self, input_source: MpsInputSource) -> Self {
        self.input_source = Some(input_source);
        self
    }

    /// Attaches a validated source span to this diagnostic.
    pub fn with_span(mut self, span: MpsSourceSpan) -> Self {
        self.span = Some(span);
        self
    }

    /// Attaches the recognized MPS section to this diagnostic.
    pub fn with_section(mut self, section: MpsSection) -> Self {
        self.section = Some(section);
        self
    }

    /// Attaches the raw field or record text relevant to this diagnostic.
    pub fn with_raw_field(mut self, raw_field: impl Into<String>) -> Self {
        self.raw_field = Some(raw_field.into());
        self
    }

    /// Attaches the named model entity relevant to this diagnostic.
    pub fn with_entity(mut self, entity: impl Into<String>) -> Self {
        self.entity = Some(entity.into());
        self
    }

    /// Attaches additional human-readable error detail.
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Returns the input source, when one is available.
    pub fn input_source(&self) -> Option<&MpsInputSource> {
        self.input_source.as_ref()
    }

    /// Returns the source span, when one is available.
    pub fn span(&self) -> Option<&MpsSourceSpan> {
        self.span.as_ref()
    }

    /// Returns the MPS section, when one is available.
    pub fn section(&self) -> Option<&MpsSection> {
        self.section.as_ref()
    }

    /// Returns the raw field or record text, when one is available.
    pub fn raw_field(&self) -> Option<&str> {
        self.raw_field.as_deref()
    }

    /// Returns the entity name, when one is available.
    pub fn entity(&self) -> Option<&str> {
        self.entity.as_deref()
    }

    /// Returns the additional human-readable detail, when one is available.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
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
        section: MpsSection,
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
    /// The public reader seam is present but lexical parsing is not yet wired.
    ParserUnavailable,
}

/// A typed, source-aware MPS import failure.
#[derive(Debug)]
pub struct MpsError {
    kind: MpsErrorKind,
    diagnostic: Box<MpsDiagnostic>,
    cause: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
}

impl MpsError {
    /// Creates an MPS error from its category and source context.
    pub fn new(kind: MpsErrorKind, diagnostic: MpsDiagnostic) -> Self {
        Self {
            kind,
            diagnostic: Box::new(diagnostic),
            cause: None,
        }
    }

    /// Creates an MPS error that retains an underlying cause.
    pub fn with_source<E>(kind: MpsErrorKind, diagnostic: MpsDiagnostic, cause: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self {
            kind,
            diagnostic: Box::new(diagnostic),
            cause: Some(Box::new(cause)),
        }
    }

    /// Creates an I/O failure that preserves its source error.
    pub fn io(diagnostic: MpsDiagnostic, cause: io::Error) -> Self {
        Self::with_source(MpsErrorKind::Io, diagnostic, cause)
    }

    /// Returns the typed category of this failure.
    pub fn kind(&self) -> &MpsErrorKind {
        &self.kind
    }

    /// Returns the source context captured for this failure.
    pub fn diagnostic(&self) -> &MpsDiagnostic {
        &self.diagnostic
    }

    /// Returns the I/O error kind when this error retains an I/O cause.
    pub fn io_kind(&self) -> Option<io::ErrorKind> {
        self.cause
            .as_deref()
            .and_then(|cause| cause.downcast_ref::<io::Error>())
            .map(io::Error::kind)
    }
}

impl std::fmt::Display for MpsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MPS import error: {}", self.kind)?;
        if let Some(input_source) = self.diagnostic.input_source() {
            write!(f, " in {input_source}")?;
        }
        if let Some(span) = self.diagnostic.span() {
            write!(
                f,
                " at line {} bytes {}..{}",
                span.line(),
                span.start(),
                span.end()
            )?;
        }
        if let Some(section) = self.diagnostic.section() {
            write!(f, " in section {section}")?;
        }
        if let Some(raw_field) = self.diagnostic.raw_field() {
            write!(f, " for field {raw_field:?}")?;
        }
        if let Some(entity) = self.diagnostic.entity() {
            write!(f, " for entity {entity}")?;
        }
        if let Some(message) = self.diagnostic.message() {
            write!(f, ": {message}")?;
        }
        if let Some(cause) = &self.cause {
            write!(f, ": {cause}")?;
        }
        Ok(())
    }
}

impl std::error::Error for MpsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.cause
            .as_deref()
            .map(|cause| cause as &(dyn std::error::Error + 'static))
    }
}

impl std::fmt::Display for MpsErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io => write!(f, "I/O failure"),
            Self::InvalidEncoding => write!(f, "invalid encoding"),
            Self::InvalidSectionOrder => write!(f, "invalid section order"),
            Self::UnsupportedSection { section } => write!(f, "unsupported MPS section {section}"),
            Self::InvalidRecord => write!(f, "invalid record"),
            Self::InvalidNumber => write!(f, "invalid number"),
            Self::DuplicateRow => write!(f, "duplicate row"),
            Self::UnknownRow => write!(f, "unknown row"),
            Self::UnknownVariable => write!(f, "unknown variable"),
            Self::InvalidMarkerNesting => write!(f, "invalid integer-marker nesting"),
            Self::DuplicateRhsEntry => write!(f, "duplicate selected RHS entry"),
            Self::DuplicateRangeEntry => write!(f, "duplicate selected RANGES entry"),
            Self::InvalidRangeForNRow => write!(f, "range entry for an N row"),
            Self::InvalidBound => write!(f, "invalid bound"),
            Self::InvalidRange => write!(f, "invalid range"),
            Self::MissingRequiredSection => write!(f, "missing required section"),
            Self::MissingEndata => write!(f, "missing ENDATA"),
            Self::UnknownVector => write!(f, "unknown vector"),
            Self::AmbiguousFormat => write!(f, "ambiguous MPS format"),
            Self::RepresentationError => write!(f, "representation error"),
            Self::ModelConstruction => write!(f, "model construction failure"),
            Self::ParserUnavailable => write!(f, "MPS parser unavailable"),
        }
    }
}

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
