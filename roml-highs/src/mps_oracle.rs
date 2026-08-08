//! Independent HiGHS `readModel` oracle for MPS qualification.
//!
//! This module is deliberately observation-only. ROML's frozen MPS semantics
//! remain normative; a native observation never changes an import result.

use std::{ffi::CString, path::Path};

use roml::model::coefficient::CoefficientTarget;
use roml::{
    io::mps::{MpsDiagnostic, MpsError, MpsErrorKind},
    Model,
};

use crate::{bindings, error::check_highs_status, lifecycle::HighsSession, HighsError};
use roml::solver::backend::{BackendError, ErrorCategory, HealthEffect};

/// Structural facts returned by native HiGHS `readModel`.
#[derive(Clone, Debug, PartialEq)]
pub struct HighsMpsSummary {
    /// Number of native columns.
    pub columns: usize,
    /// Number of native rows.
    pub rows: usize,
    /// Number of native matrix nonzeros.
    pub nonzeros: usize,
    /// Native objective sense constant.
    pub objective_sense: bindings::HighsInt,
    /// Native objective offset.
    pub objective_offset: f64,
}

/// Structural facts extracted from a ROML MPS import.
#[derive(Clone, Debug, PartialEq)]
pub struct RomlMpsSummary {
    /// Number of imported variables.
    pub columns: usize,
    /// Number of imported constraints.
    pub rows: usize,
    /// Number of canonical matrix cells.
    pub nonzeros: usize,
    /// Imported objective offset.
    pub objective_offset: f64,
}

/// Results from both independent interpretations of one MPS path.
#[derive(Debug)]
pub struct MpsDifferentialObservation {
    /// ROML's normative import result.
    pub roml: Result<RomlMpsSummary, MpsError>,
    /// HiGHS's independent native `readModel` result.
    pub highs: Result<HighsMpsSummary, HighsError>,
}

/// Required disposition for a recorded native/ROML divergence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MpsDifferentialDisposition {
    /// ROML implementation was corrected to the frozen semantics.
    RomlBugFixed,
    /// The accepted P35 dialect was narrowed and tests/requirements changed.
    DialectNarrowed,
    /// Authoritative evidence and owner approval document an intentional divergence.
    CompatibilityException {
        /// Durable reference to the owner-approved exception record.
        owner_approval: String,
    },
    /// ROML intentionally rejects a construct accepted by native HiGHS.
    IntentionalRomlRejection,
}

impl HighsSession {
    /// Read an MPS path with native HiGHS and return structural observations.
    ///
    /// This method does not mutate any ROML model or interpret the result as
    /// semantic authority. Each call is intended for a fresh qualification
    /// session.
    pub fn read_model_summary<P: AsRef<Path>>(
        &mut self,
        path: P,
    ) -> Result<HighsMpsSummary, HighsError> {
        let path = path.as_ref();
        let path = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| {
            BackendError::new(
                "MPS path contains an embedded NUL",
                ErrorCategory::InvalidInput,
                HealthEffect::None,
            )
        })?;
        // SAFETY: `self.raw` is the exclusively-owned live handle created by
        // `HighsSession::try_new`; the CString is NUL-terminated and remains
        // alive for the duration of the call. The symbol is from highs-sys's
        // generated official C API bindings.
        let status = unsafe { bindings::Highs_readModel(self.raw, path.as_ptr()) };
        check_highs_status(status, self.raw, "Highs_readModel")?;

        let columns = checked_count(unsafe { bindings::Highs_getNumCol(self.raw) }, "columns")?;
        let rows = checked_count(unsafe { bindings::Highs_getNumRow(self.raw) }, "rows")?;
        let nonzeros = checked_count(unsafe { bindings::Highs_getNumNz(self.raw) }, "nonzeros")?;
        let mut objective_sense = 0;
        let status = unsafe { bindings::Highs_getObjectiveSense(self.raw, &mut objective_sense) };
        check_highs_status(status, self.raw, "Highs_getObjectiveSense")?;
        let mut objective_offset = 0.0;
        let status = unsafe { bindings::Highs_getObjectiveOffset(self.raw, &mut objective_offset) };
        check_highs_status(status, self.raw, "Highs_getObjectiveOffset")?;

        Ok(HighsMpsSummary {
            columns,
            rows,
            nonzeros,
            objective_sense,
            objective_offset,
        })
    }
}

/// Run both the normative ROML reader and native HiGHS `readModel`.
pub fn observe_mps_differential(path: impl AsRef<Path>) -> MpsDifferentialObservation {
    let path = path.as_ref();
    let roml = roml::io::mps::MpsReader::new()
        .read_path(path)
        .and_then(|import| roml_summary(&import.model));
    let highs = HighsSession::try_new().and_then(|mut session| session.read_model_summary(path));
    MpsDifferentialObservation { roml, highs }
}

fn roml_summary(model: &Model) -> Result<RomlMpsSummary, MpsError> {
    let snapshot = model.take_snapshot().map_err(|error| {
        MpsError::with_source(
            MpsErrorKind::ModelConstruction,
            MpsDiagnostic::new().with_message("cannot snapshot imported ROML model"),
            error,
        )
    })?;
    let objective_offset = snapshot
        .objectives
        .iter()
        .find(|objective| objective.active)
        .map_or(0.0, |objective| objective.constant);
    Ok(RomlMpsSummary {
        columns: snapshot.variables.len(),
        rows: snapshot.constraints.len(),
        nonzeros: snapshot
            .cells
            .iter()
            .filter(|cell| matches!(cell.cell_key.0, CoefficientTarget::Constraint(_)))
            .count(),
        objective_offset,
    })
}

fn checked_count(value: bindings::HighsInt, label: &str) -> Result<usize, HighsError> {
    usize::try_from(value).map_err(|_| {
        BackendError::new(
            format!("HiGHS returned a negative {label} count"),
            ErrorCategory::Internal,
            HealthEffect::Recoverable,
        )
    })
}
