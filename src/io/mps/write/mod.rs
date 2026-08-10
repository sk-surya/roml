//! Solver-free MPS write-back contract.
//!
//! This module freezes the public writer seam used by the later semantic
//! projection, formatting, and path-transaction tasks. The Wave 0 methods are
//! intentionally typed stubs; no output behavior is advertised until those
//! implementation units are qualified.

mod error;
mod types;

pub use error::{
    MpsEntityKind, MpsPathStage, MpsWriteContext, MpsWriteDiagnostic, MpsWriteError,
    MpsWriteErrorKind,
};
pub use types::{
    MpsDestinationPolicy, MpsEvaluatedParameter, MpsNamePolicy, MpsWriteLowering, MpsWriteName,
    MpsWriteNameMap, MpsWriteOptions, MpsWriteReport,
};

use std::{io, path::Path};

use crate::Model;

/// Configured solver-free writer for deterministic free MPS output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MpsWriter {
    options: MpsWriteOptions,
}

impl Default for MpsWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl MpsWriter {
    /// Creates a writer with the frozen P36 default options.
    pub fn new() -> Self {
        Self::with_options(MpsWriteOptions::default())
    }

    /// Creates a writer with explicit options from the frozen P36 surface.
    pub fn with_options(options: MpsWriteOptions) -> Self {
        Self { options }
    }

    /// Returns the options configured for this writer.
    pub fn options(&self) -> &MpsWriteOptions {
        &self.options
    }

    /// Serializes the model to a caller-provided stream.
    ///
    /// Stream writes may leave partial bytes when implemented and never
    /// perform destination replacement. The Wave 0 contract only establishes
    /// the typed failure boundary; serialization is added by a later task.
    pub fn write<W: io::Write>(
        &self,
        model: &Model,
        output: W,
    ) -> Result<MpsWriteReport, MpsWriteError> {
        let _ = (model, output);
        Err(MpsWriteError::not_yet_implemented())
    }

    /// Serializes the model and commits it according to the destination policy.
    ///
    /// Path transaction behavior is intentionally not implemented in this
    /// contract-freeze slice. The typed error does not create or replace a
    /// destination.
    pub fn write_path<P: AsRef<Path>>(
        &self,
        model: &Model,
        path: P,
    ) -> Result<MpsWriteReport, MpsWriteError> {
        let _ = model;
        Err(MpsWriteError::not_yet_implemented()
            .with_context(MpsWriteContext::default().with_path(path.as_ref().to_owned())))
    }
}
