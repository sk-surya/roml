//! Structured error types for the P36 MPS writer contract.

use std::{
    error::Error,
    fmt, io,
    path::{Path, PathBuf},
};

use crate::{
    id::ParamId,
    identity::{ModelInstanceId, ModelLineageId},
    revision::ModelRevision,
};

/// The semantic kind of an MPS write failure.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MpsWriteErrorKind {
    /// The output stream or filesystem operation failed.
    Io,
    /// A `CreateNew` destination already exists.
    DestinationExists,
    /// The target does not provide the required atomic replacement primitive.
    AtomicReplaceUnavailable,
    /// A staged path transaction failed.
    PathTransaction,
    /// The canonical model failed validation for export.
    ModelValidation,
    /// An active model feature cannot be represented by standard MPS.
    Unrepresentable,
    /// A parameter-dependent value could not be evaluated.
    ParameterEvaluation,
    /// An emitted numeric value was non-finite or otherwise invalid.
    NonFiniteValue,
    /// A source or generated name could not be allocated under the policy.
    NameAllocation,
    /// Bytes could not be serialized to the MPS stream.
    Serialization,
    /// A referenced canonical entity is stale or no longer available.
    StaleEntity,
    /// An internal canonical invariant was violated.
    InternalInvariant,
    /// Transitional Wave 0 result: the writer implementation is not present.
    #[doc(hidden)]
    NotYetImplemented,
}

impl fmt::Display for MpsWriteErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Io => "I/O failure",
            Self::DestinationExists => "destination exists",
            Self::AtomicReplaceUnavailable => "atomic replacement unavailable",
            Self::PathTransaction => "path transaction failure",
            Self::ModelValidation => "model validation failure",
            Self::Unrepresentable => "unrepresentable model feature",
            Self::ParameterEvaluation => "parameter evaluation failure",
            Self::NonFiniteValue => "non-finite value",
            Self::NameAllocation => "name allocation failure",
            Self::Serialization => "serialization failure",
            Self::StaleEntity => "stale entity",
            Self::InternalInvariant => "internal invariant failure",
            Self::NotYetImplemented => "writer not yet implemented",
        };
        f.write_str(label)
    }
}

/// The model/entity categories that can be attached to writer context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MpsEntityKind {
    /// The canonical model as a whole.
    Model,
    /// A variable/column entity.
    Variable,
    /// A constraint/row entity.
    Constraint,
    /// An objective entity.
    Objective,
    /// A parameter entity.
    Parameter,
    /// A higher-level semantic construct.
    Construct,
    /// A matrix coefficient cell.
    MatrixCell,
}

impl fmt::Display for MpsEntityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Model => "model",
            Self::Variable => "variable",
            Self::Constraint => "constraint",
            Self::Objective => "objective",
            Self::Parameter => "parameter",
            Self::Construct => "construct",
            Self::MatrixCell => "matrix cell",
        };
        f.write_str(label)
    }
}

/// The stage of a transactional `write_path` operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MpsPathStage {
    /// Creating the same-directory temporary file.
    CreateTemp,
    /// Writing staged bytes.
    Write,
    /// Flushing the staged file.
    Flush,
    /// Synchronizing staged bytes to durable storage.
    Sync,
    /// Replacing or creating the destination.
    Replace,
    /// Cleaning up a temporary file after failure.
    Cleanup,
}

impl fmt::Display for MpsPathStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::CreateTemp => "CreateTemp",
            Self::Write => "Write",
            Self::Flush => "Flush",
            Self::Sync => "Sync",
            Self::Replace => "Replace",
            Self::Cleanup => "Cleanup",
        };
        f.write_str(label)
    }
}

/// Structured context attached to an MPS write failure.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MpsWriteContext {
    /// Source model lineage, when a model was available.
    pub model_lineage: Option<ModelLineageId>,
    /// Source model instance, when a model was available.
    pub model_instance: Option<ModelInstanceId>,
    /// Source model revision, when a model was available.
    pub model_revision: Option<ModelRevision>,
    /// Entity category associated with the failure.
    pub entity_kind: Option<MpsEntityKind>,
    /// Stable semantic name or descriptive handle for the entity.
    pub entity_name: Option<String>,
    /// Unsupported feature, domain, or construct description.
    pub feature: Option<String>,
    /// Numeric field associated with a value failure.
    pub numeric_field: Option<String>,
    /// Parameter handles used by the failed evaluation.
    pub parameter_dependencies: Vec<ParamId>,
    /// Destination path associated with a path operation.
    pub path: Option<PathBuf>,
    /// Path transaction stage at which the failure occurred.
    pub stage: Option<MpsPathStage>,
    /// Additional human-readable detail.
    pub message: Option<String>,
}

impl MpsWriteContext {
    /// Attaches the exact source model identity tuple.
    pub fn with_model_state(
        mut self,
        lineage: ModelLineageId,
        instance: ModelInstanceId,
        revision: ModelRevision,
    ) -> Self {
        self.model_lineage = Some(lineage);
        self.model_instance = Some(instance);
        self.model_revision = Some(revision);
        self
    }

    /// Attaches an entity category and semantic name.
    pub fn with_entity(mut self, kind: MpsEntityKind, name: impl Into<String>) -> Self {
        self.entity_kind = Some(kind);
        self.entity_name = Some(name.into());
        self
    }

    /// Attaches unsupported-feature or construct context.
    pub fn with_feature(mut self, feature: impl Into<String>) -> Self {
        self.feature = Some(feature.into());
        self
    }

    /// Attaches the numeric field associated with a value failure.
    pub fn with_numeric_field(mut self, field: impl Into<String>) -> Self {
        self.numeric_field = Some(field.into());
        self
    }

    /// Attaches the parameter handles involved in an evaluation.
    pub fn with_parameter_dependencies<I>(mut self, dependencies: I) -> Self
    where
        I: IntoIterator<Item = ParamId>,
    {
        self.parameter_dependencies = dependencies.into_iter().collect();
        self
    }

    /// Attaches a filesystem destination path.
    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Attaches a path transaction stage.
    pub fn with_stage(mut self, stage: MpsPathStage) -> Self {
        self.stage = Some(stage);
        self
    }

    /// Attaches additional human-readable detail.
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Returns the destination path, when present.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Returns the path transaction stage, when present.
    pub fn stage(&self) -> Option<MpsPathStage> {
        self.stage
    }
}

/// Alias matching the diagnostic terminology used by the MPS reader seam.
pub type MpsWriteDiagnostic = MpsWriteContext;

/// A structured, source-preserving MPS write failure.
#[derive(Debug)]
pub struct MpsWriteError {
    kind: MpsWriteErrorKind,
    context: Box<MpsWriteContext>,
    cause: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl MpsWriteError {
    /// Creates an error with a top-level kind and structured context.
    pub fn new(kind: MpsWriteErrorKind, context: MpsWriteContext) -> Self {
        Self {
            kind,
            context: Box::new(context),
            cause: None,
        }
    }

    /// Creates an error with no additional context.
    pub fn from_kind(kind: MpsWriteErrorKind) -> Self {
        Self::new(kind, MpsWriteContext::default())
    }

    /// Creates an error while retaining its underlying cause.
    pub fn with_source<E>(kind: MpsWriteErrorKind, context: MpsWriteContext, cause: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self {
            kind,
            context: Box::new(context),
            cause: Some(Box::new(cause)),
        }
    }

    /// Creates an I/O error while retaining the original I/O cause.
    pub fn io(context: MpsWriteContext, cause: io::Error) -> Self {
        Self::with_source(MpsWriteErrorKind::Io, context, cause)
    }

    /// Creates the typed transitional error used by the Wave 0 writer stubs.
    pub(crate) fn not_yet_implemented() -> Self {
        Self::new(
            MpsWriteErrorKind::NotYetImplemented,
            MpsWriteContext::default().with_message(
                "MPS write projection and path transaction are not implemented in this slice",
            ),
        )
    }

    /// Returns the stable top-level kind.
    pub fn kind(&self) -> &MpsWriteErrorKind {
        &self.kind
    }

    /// Returns all structured context captured for the failure.
    pub fn context(&self) -> &MpsWriteContext {
        &self.context
    }

    /// Adds or replaces structured context on this error.
    pub fn with_context(mut self, context: MpsWriteContext) -> Self {
        self.context = Box::new(context);
        self
    }

    /// Returns the I/O error kind when the underlying cause is an I/O error.
    pub fn io_kind(&self) -> Option<io::ErrorKind> {
        self.cause
            .as_deref()
            .and_then(|cause| cause.downcast_ref::<io::Error>())
            .map(io::Error::kind)
    }
}

impl fmt::Display for MpsWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MPS write error: {}", self.kind)?;
        if let Some(path) = self.context.path() {
            write!(f, " at {}", path.display())?;
        }
        if let Some(stage) = self.context.stage() {
            write!(f, " during {stage}")?;
        }
        if let Some(kind) = self.context.entity_kind {
            write!(f, " for {kind}")?;
            if let Some(name) = &self.context.entity_name {
                write!(f, " {name}")?;
            }
        }
        if let Some(feature) = &self.context.feature {
            write!(f, " ({feature})")?;
        }
        if let Some(field) = &self.context.numeric_field {
            write!(f, " in numeric field {field}")?;
        }
        if let Some(message) = &self.context.message {
            write!(f, ": {message}")?;
        }
        if let Some(cause) = &self.cause {
            write!(f, ": {cause}")?;
        }
        Ok(())
    }
}

impl Error for MpsWriteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause
            .as_deref()
            .map(|cause| cause as &(dyn Error + 'static))
    }
}
