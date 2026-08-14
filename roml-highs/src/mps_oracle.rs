//! Independent HiGHS `readModel` oracle for MPS qualification.
//!
//! This module is deliberately observation-only. ROML's frozen MPS semantics
//! remain normative; a native observation never changes an import result.

use std::{
    collections::BTreeMap,
    ffi::{CStr, CString},
    os::raw::c_char,
    path::Path,
};

use roml::model::coefficient::CoefficientTarget;
use roml::{
    compiler::{capability::CompilationPolicy, session::CompilationSession},
    io::mps::{MpsDiagnostic, MpsError, MpsErrorKind},
    Model,
};

use crate::{
    bindings,
    compiler::rebuild_from_backend_snapshot,
    error::{check_highs_status, from_native_status},
    lifecycle::HighsSession,
    solution::map_termination_status,
    Highs, HighsError,
};
use roml::solver::backend::{BackendError, ErrorCategory, HealthEffect};
use roml::solver::SolveStatus;

/// Normalized column semantics used by the Q03 comparator.
#[derive(Clone, Debug, PartialEq)]
pub struct MpsColumnSemantics {
    /// Objective coefficient.
    pub cost: f64,
    /// Lower bound.
    pub lower: f64,
    /// Upper bound.
    pub upper: f64,
    /// HiGHS-compatible integrality code.
    pub integrality: bindings::HighsInt,
}

/// Normalized row-bound semantics used by the Q03 comparator.
#[derive(Clone, Debug, PartialEq)]
pub struct MpsRowSemantics {
    /// Lower bound.
    pub lower: f64,
    /// Upper bound.
    pub upper: f64,
}

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
    /// Native columns keyed by their MPS names.
    pub column_semantics: BTreeMap<String, MpsColumnSemantics>,
    /// Native column names in the order returned by HiGHS.
    pub column_order: Vec<String>,
    /// Native rows keyed by their MPS names.
    pub row_semantics: BTreeMap<String, MpsRowSemantics>,
    /// Native row names in the order returned by HiGHS.
    pub row_order: Vec<String>,
    /// Native constraint coefficients keyed by `(row, column)` names.
    pub matrix: BTreeMap<(String, String), f64>,
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
    /// ROML objective sense using HiGHS's `1` minimize / `-1` maximize codes.
    pub objective_sense: bindings::HighsInt,
    /// Imported columns keyed by their MPS names.
    pub column_semantics: BTreeMap<String, MpsColumnSemantics>,
    /// Imported rows keyed by their MPS names.
    pub row_semantics: BTreeMap<String, MpsRowSemantics>,
    /// Imported constraint coefficients keyed by `(row, column)` names.
    pub matrix: BTreeMap<(String, String), f64>,
}

/// Result of comparing all Q03 fields exposed by the two observations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MpsStructuralComparison {
    /// Whether every compared field matched within the declared tolerance.
    pub equivalent: bool,
    /// Stable human-readable mismatch descriptions.
    pub differences: Vec<String>,
}

/// Normalized termination and objective observation for one MPS solve.
#[derive(Clone, Debug, PartialEq)]
pub struct MpsSolveObservation {
    /// ROML-normalized termination class.
    pub status: SolveStatus,
    /// Objective value when the solver produced a comparable value.
    pub objective_value: Option<f64>,
}

/// Native and ROML solve observations for one MPS path.
#[derive(Debug)]
pub struct MpsSolveDifferentialObservation {
    /// ROML import followed by ROML-to-HiGHS projection and solve.
    pub roml: Result<MpsSolveObservation, String>,
    /// Native HiGHS `readModel` followed by native solve.
    pub highs: Result<MpsSolveObservation, HighsError>,
}

/// Result of comparing native and ROML Q04 solve observations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MpsSolveComparison {
    /// Whether termination and objective observations matched.
    pub equivalent: bool,
    /// Stable mismatch descriptions.
    pub differences: Vec<String>,
}

/// Absolute tolerance for native/ROML MPS coefficient and bound comparison.
pub const MPS_STRUCTURAL_ABS_TOLERANCE: f64 = 1e-9;

/// Relative tolerance for native/ROML MPS coefficient and bound comparison.
pub const MPS_STRUCTURAL_REL_TOLERANCE: f64 = 1e-9;

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
    /// Build the direct ROML-to-HiGHS observation used by P36 qualification.
    ///
    /// This path compiles the canonical model through the existing HiGHS
    /// backend projection and observes the resulting native model without
    /// passing through MPS. It is intentionally separate from the writer and
    /// from the P35 reader oracle.
    pub fn observe_model_structure(model: &Model) -> Result<HighsMpsSummary, HighsError> {
        let mut session = Self::try_new()?;
        let snapshot = model.take_snapshot().map_err(|error| {
            BackendError::new(
                format!("cannot snapshot model for direct HiGHS observation: {error}"),
                ErrorCategory::InvalidInput,
                HealthEffect::Recoverable,
            )
        })?;
        let mut compiler = CompilationSession::new();
        let compiled = compiler
            .compile_snapshot(
                model.instance(),
                &snapshot,
                &CompilationPolicy::Auto,
                &session.typed_capabilities,
            )
            .map_err(|error| {
                BackendError::new(
                    format!("cannot compile model for direct HiGHS observation: {error}"),
                    ErrorCategory::InvalidInput,
                    HealthEffect::Recoverable,
                )
            })?;
        compiled.validate().map_err(|error| {
            BackendError::new(
                format!("compiled model failed direct HiGHS observation validation: {error}"),
                ErrorCategory::Internal,
                HealthEffect::Recoverable,
            )
        })?;
        rebuild_from_backend_snapshot(
            session.raw,
            &compiled,
            &mut session.col_map,
            &mut session.row_map,
            &mut session.compiled_to_user_variable,
            &mut session.compiled_to_user_constraint,
            &mut session.compiled_to_user_objective,
            session.inf,
            &mut session.var_bounds,
            &mut session.con_bounds,
            &mut session.obj_costs,
            &mut session.obj_senses,
            &mut session.obj_offsets,
            &mut session.active_obj,
        )?;
        // The direct compiler path does not assign user-facing native names.
        // Use observation-local ordinals rather than asking HiGHS for names
        // that do not exist; the P36 test compares normalized positions.
        session.summarize_loaded_model(true)
    }

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

        let column_count = columns;
        let row_count = rows;
        let nonzero_count = nonzeros;
        let mut native_columns = highs_int(column_count, "columns")?;
        let mut native_rows = highs_int(row_count, "rows")?;
        let mut native_nonzeros = highs_int(nonzero_count, "nonzeros")?;
        let mut sense = objective_sense;
        let mut offset = objective_offset;
        let mut costs = vec![0.0; column_count];
        let infinity = unsafe { bindings::Highs_getInfinity(self.raw) };
        let mut column_lower = vec![0.0; column_count];
        let mut column_upper = vec![0.0; column_count];
        let mut row_lower = vec![0.0; row_count];
        let mut row_upper = vec![0.0; row_count];
        let mut starts = vec![0; column_count];
        let mut indices = vec![0; nonzero_count];
        let mut values = vec![0.0; nonzero_count];
        let mut integrality = vec![bindings::kHighsVarTypeContinuous; column_count];
        let status = unsafe {
            bindings::Highs_getLp(
                self.raw,
                bindings::kHighsMatrixFormatColwise,
                &mut native_columns as *mut _,
                &mut native_rows as *mut _,
                &mut native_nonzeros as *mut _,
                &mut sense as *mut _,
                &mut offset as *mut _,
                costs.as_mut_ptr(),
                column_lower.as_mut_ptr(),
                column_upper.as_mut_ptr(),
                row_lower.as_mut_ptr(),
                row_upper.as_mut_ptr(),
                starts.as_mut_ptr(),
                indices.as_mut_ptr(),
                values.as_mut_ptr(),
                integrality.as_mut_ptr(),
            )
        };
        check_highs_status(status, self.raw, "Highs_getLp")?;
        let native_columns = checked_count(native_columns, "columns")?;
        let native_rows = checked_count(native_rows, "rows")?;
        let native_nonzeros = checked_count(native_nonzeros, "nonzeros")?;
        if native_columns != column_count
            || native_rows != row_count
            || native_nonzeros != nonzero_count
        {
            return Err(BackendError::new(
                "HiGHS changed model dimensions while returning the LP observation",
                ErrorCategory::Internal,
                HealthEffect::Recoverable,
            ));
        }
        let column_names = native_names(self.raw, native_columns, true)?;
        let row_names = native_names(self.raw, native_rows, false)?;
        let column_semantics = column_names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                (
                    name.clone(),
                    MpsColumnSemantics {
                        cost: costs[index],
                        lower: normalize_bound(column_lower[index], infinity),
                        upper: normalize_bound(column_upper[index], infinity),
                        integrality: integrality[index],
                    },
                )
            })
            .collect();
        let row_semantics = row_names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                (
                    name.clone(),
                    MpsRowSemantics {
                        lower: normalize_bound(row_lower[index], infinity),
                        upper: normalize_bound(row_upper[index], infinity),
                    },
                )
            })
            .collect();
        let mut matrix = BTreeMap::new();
        for (column_index, column_name) in column_names.iter().enumerate() {
            let start = checked_index(starts[column_index], "matrix start")?;
            let end = if column_index + 1 < starts.len() {
                checked_index(starts[column_index + 1], "matrix end")?
            } else {
                native_nonzeros
            };
            if end > native_nonzeros || start > end {
                return Err(BackendError::new(
                    "HiGHS returned invalid compressed matrix bounds",
                    ErrorCategory::Internal,
                    HealthEffect::Recoverable,
                ));
            }
            for matrix_index in start..end {
                let row_index = checked_index(indices[matrix_index], "matrix row index")?;
                let Some(row_name) = row_names.get(row_index) else {
                    return Err(BackendError::new(
                        "HiGHS returned a matrix row index outside the observed rows",
                        ErrorCategory::Internal,
                        HealthEffect::Recoverable,
                    ));
                };
                matrix.insert(
                    (row_name.clone(), column_name.clone()),
                    values[matrix_index],
                );
            }
        }

        Ok(HighsMpsSummary {
            columns,
            rows,
            nonzeros,
            objective_sense,
            objective_offset,
            column_semantics,
            column_order: column_names,
            row_semantics,
            row_order: row_names,
            matrix,
        })
    }

    fn summarize_loaded_model(
        &mut self,
        use_observation_names: bool,
    ) -> Result<HighsMpsSummary, HighsError> {
        let columns = checked_count(unsafe { bindings::Highs_getNumCol(self.raw) }, "columns")?;
        let rows = checked_count(unsafe { bindings::Highs_getNumRow(self.raw) }, "rows")?;
        let nonzeros = checked_count(unsafe { bindings::Highs_getNumNz(self.raw) }, "nonzeros")?;
        let mut objective_sense = 0;
        let status = unsafe { bindings::Highs_getObjectiveSense(self.raw, &mut objective_sense) };
        check_highs_status(status, self.raw, "Highs_getObjectiveSense")?;
        let mut objective_offset = 0.0;
        let status = unsafe { bindings::Highs_getObjectiveOffset(self.raw, &mut objective_offset) };
        check_highs_status(status, self.raw, "Highs_getObjectiveOffset")?;

        let column_count = columns;
        let row_count = rows;
        let nonzero_count = nonzeros;
        let mut native_columns = highs_int(column_count, "columns")?;
        let mut native_rows = highs_int(row_count, "rows")?;
        let mut native_nonzeros = highs_int(nonzero_count, "nonzeros")?;
        let mut sense = objective_sense;
        let mut offset = objective_offset;
        let mut costs = vec![0.0; column_count];
        let infinity = unsafe { bindings::Highs_getInfinity(self.raw) };
        let mut column_lower = vec![0.0; column_count];
        let mut column_upper = vec![0.0; column_count];
        let mut row_lower = vec![0.0; row_count];
        let mut row_upper = vec![0.0; row_count];
        let mut starts = vec![0; column_count];
        let mut indices = vec![0; nonzero_count];
        let mut values = vec![0.0; nonzero_count];
        let mut integrality = vec![bindings::kHighsVarTypeContinuous; column_count];
        let status = unsafe {
            bindings::Highs_getLp(
                self.raw,
                bindings::kHighsMatrixFormatColwise,
                &mut native_columns as *mut _,
                &mut native_rows as *mut _,
                &mut native_nonzeros as *mut _,
                &mut sense as *mut _,
                &mut offset as *mut _,
                costs.as_mut_ptr(),
                column_lower.as_mut_ptr(),
                column_upper.as_mut_ptr(),
                row_lower.as_mut_ptr(),
                row_upper.as_mut_ptr(),
                starts.as_mut_ptr(),
                indices.as_mut_ptr(),
                values.as_mut_ptr(),
                integrality.as_mut_ptr(),
            )
        };
        check_highs_status(status, self.raw, "Highs_getLp")?;
        let native_columns = checked_count(native_columns, "columns")?;
        let native_rows = checked_count(native_rows, "rows")?;
        let native_nonzeros = checked_count(native_nonzeros, "nonzeros")?;
        if native_columns != column_count
            || native_rows != row_count
            || native_nonzeros != nonzero_count
        {
            return Err(BackendError::new(
                "HiGHS changed model dimensions while returning the LP observation",
                ErrorCategory::Internal,
                HealthEffect::Recoverable,
            ));
        }
        let column_names = if use_observation_names {
            observation_names(native_columns, "DIRECT_COL")
        } else {
            native_names(self.raw, native_columns, true)?
        };
        let row_names = if use_observation_names {
            observation_names(native_rows, "DIRECT_ROW")
        } else {
            native_names(self.raw, native_rows, false)?
        };
        let column_semantics = column_names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                (
                    name.clone(),
                    MpsColumnSemantics {
                        cost: costs[index],
                        lower: normalize_bound(column_lower[index], infinity),
                        upper: normalize_bound(column_upper[index], infinity),
                        integrality: integrality[index],
                    },
                )
            })
            .collect();
        let row_semantics = row_names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                (
                    name.clone(),
                    MpsRowSemantics {
                        lower: normalize_bound(row_lower[index], infinity),
                        upper: normalize_bound(row_upper[index], infinity),
                    },
                )
            })
            .collect();
        let mut matrix = BTreeMap::new();
        for (column_index, column_name) in column_names.iter().enumerate() {
            let start = checked_index(starts[column_index], "matrix start")?;
            let end = if column_index + 1 < starts.len() {
                checked_index(starts[column_index + 1], "matrix end")?
            } else {
                native_nonzeros
            };
            if end > native_nonzeros || start > end {
                return Err(BackendError::new(
                    "HiGHS returned invalid compressed matrix bounds",
                    ErrorCategory::Internal,
                    HealthEffect::Recoverable,
                ));
            }
            for matrix_index in start..end {
                let row_index = checked_index(indices[matrix_index], "matrix row index")?;
                let Some(row_name) = row_names.get(row_index) else {
                    return Err(BackendError::new(
                        "HiGHS returned a matrix row index outside the observed rows",
                        ErrorCategory::Internal,
                        HealthEffect::Recoverable,
                    ));
                };
                matrix.insert(
                    (row_name.clone(), column_name.clone()),
                    values[matrix_index],
                );
            }
        }

        Ok(HighsMpsSummary {
            columns,
            rows,
            nonzeros,
            objective_sense,
            objective_offset,
            column_semantics,
            column_order: column_names,
            row_semantics,
            row_order: row_names,
            matrix,
        })
    }

    /// Solve the model most recently loaded by [`Self::read_model_summary`].
    pub fn solve_loaded_mps_model(&mut self) -> Result<MpsSolveObservation, HighsError> {
        let run_status = unsafe { bindings::Highs_run(self.raw) };
        if run_status < bindings::STATUS_OK {
            return Err(from_native_status(run_status, "Highs_run"));
        }
        let termination = map_termination_status(self.raw, run_status);
        let status = match termination {
            roml::solver::backend::TerminationStatus::Optimal => SolveStatus::Optimal,
            roml::solver::backend::TerminationStatus::Feasible => SolveStatus::Feasible,
            roml::solver::backend::TerminationStatus::Infeasible => SolveStatus::Infeasible,
            roml::solver::backend::TerminationStatus::Unbounded => SolveStatus::Unbounded,
            roml::solver::backend::TerminationStatus::InfeasibleOrUnbounded => {
                SolveStatus::InfeasibleOrUnbounded
            }
            roml::solver::backend::TerminationStatus::TimeLimit => SolveStatus::TimeLimit,
            roml::solver::backend::TerminationStatus::IterationLimit => SolveStatus::IterationLimit,
            roml::solver::backend::TerminationStatus::NodeLimit => SolveStatus::NodeLimit,
            roml::solver::backend::TerminationStatus::Interrupted => SolveStatus::Interrupted,
            roml::solver::backend::TerminationStatus::NumericalIssue => SolveStatus::Numerical,
            roml::solver::backend::TerminationStatus::Error
            | roml::solver::backend::TerminationStatus::Unknown => {
                return Err(BackendError::new(
                    format!("HiGHS returned uninterpretable model status {termination:?}"),
                    ErrorCategory::Internal,
                    HealthEffect::Recoverable,
                ));
            }
        };
        let objective_value = match status {
            SolveStatus::Optimal | SolveStatus::Feasible => {
                Some(unsafe { bindings::Highs_getObjectiveValue(self.raw) })
            }
            _ => None,
        };
        Ok(MpsSolveObservation {
            status,
            objective_value,
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

/// Run native HiGHS and ROML-to-HiGHS solves for one MPS path.
pub fn observe_mps_solve_differential(path: impl AsRef<Path>) -> MpsSolveDifferentialObservation {
    let path = path.as_ref();
    let highs = HighsSession::try_new().and_then(|mut session| {
        session.read_model_summary(path)?;
        session.solve_loaded_mps_model()
    });
    let roml = (|| {
        let mut import = roml::io::mps::MpsReader::new()
            .read_path(path)
            .map_err(|error| error.to_string())?;
        let mut highs = Highs::new().map_err(|error| error.to_string())?;
        let solution = highs
            .solve(&mut import.model)
            .map_err(|error| error.to_string())?;
        Ok(MpsSolveObservation {
            status: solution.status(),
            objective_value: solution.objective_value(),
        })
    })();
    MpsSolveDifferentialObservation { roml, highs }
}

/// Compare Q04 termination classes and objective values.
pub fn compare_mps_solve(
    highs: &MpsSolveObservation,
    roml: &MpsSolveObservation,
    abs_tolerance: f64,
    rel_tolerance: f64,
) -> MpsSolveComparison {
    let mut differences = Vec::new();
    if highs.status != roml.status {
        differences.push(format!(
            "termination: {:?} != {:?}",
            highs.status, roml.status
        ));
    }
    match (highs.objective_value, roml.objective_value) {
        (Some(native), Some(imported)) => compare_float(
            "objective value",
            native,
            imported,
            abs_tolerance,
            rel_tolerance,
            &mut differences,
        ),
        (None, None) => {}
        (native, imported) => differences.push(format!(
            "objective presence: {:?} != {:?}",
            native.is_some(),
            imported.is_some()
        )),
    }
    MpsSolveComparison {
        equivalent: differences.is_empty(),
        differences,
    }
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
    let objective_sense = snapshot
        .objectives
        .iter()
        .find(|objective| objective.active)
        .map_or(
            bindings::kHighsObjSenseMinimize,
            |objective| match objective.sense {
                roml::model::objective::Sense::Minimize => bindings::kHighsObjSenseMinimize,
                roml::model::objective::Sense::Maximize => bindings::kHighsObjSenseMaximize,
            },
        );
    let variable_names = snapshot
        .variables
        .iter()
        .map(|variable| {
            model
                .variable_name(variable.id)
                .map(|name| (variable.id, name.unwrap_or_default().to_owned()))
                .map_err(|error| model_error("variable name", error))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let constraint_names = snapshot
        .constraints
        .iter()
        .map(|constraint| {
            model
                .constraint_name(constraint.id)
                .map(|name| (constraint.id, name.unwrap_or_default().to_owned()))
                .map_err(|error| model_error("constraint name", error))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let active_objective = snapshot
        .objectives
        .iter()
        .find(|objective| objective.active)
        .map(|objective| objective.id);
    let mut column_semantics = BTreeMap::new();
    for variable in &snapshot.variables {
        let Some(name) = variable_names.get(&variable.id) else {
            continue;
        };
        column_semantics.insert(
            name.clone(),
            MpsColumnSemantics {
                cost: 0.0,
                lower: variable.bounds.lower,
                upper: variable.bounds.upper,
                integrality: match variable.var_type {
                    roml::model::VarType::Continuous => bindings::kHighsVarTypeContinuous,
                    roml::model::VarType::Integer | roml::model::VarType::Binary => {
                        bindings::kHighsVarTypeInteger
                    }
                },
            },
        );
    }
    let mut row_semantics = BTreeMap::new();
    for constraint in &snapshot.constraints {
        let Some(name) = constraint_names.get(&constraint.id) else {
            continue;
        };
        row_semantics.insert(
            name.clone(),
            MpsRowSemantics {
                lower: constraint.bounds.lower,
                upper: constraint.bounds.upper,
            },
        );
    }
    let mut matrix = BTreeMap::new();
    for cell in &snapshot.cells {
        let Some(variable_name) = variable_names.get(&cell.cell_key.1) else {
            continue;
        };
        match cell.cell_key.0 {
            CoefficientTarget::Constraint(constraint) => {
                let Some(row_name) = constraint_names.get(&constraint) else {
                    continue;
                };
                matrix.insert(
                    (row_name.clone(), variable_name.clone()),
                    cell.evaluated_value,
                );
            }
            CoefficientTarget::Objective(objective) if Some(objective) == active_objective => {
                if let Some(column) = column_semantics.get_mut(variable_name) {
                    column.cost = cell.evaluated_value;
                }
            }
            CoefficientTarget::Objective(_) => {}
        }
    }
    Ok(RomlMpsSummary {
        columns: snapshot.variables.len(),
        rows: snapshot.constraints.len(),
        nonzeros: snapshot
            .cells
            .iter()
            .filter(|cell| matches!(cell.cell_key.0, CoefficientTarget::Constraint(_)))
            .count(),
        objective_offset,
        objective_sense,
        column_semantics,
        row_semantics,
        matrix,
    })
}

fn model_error(label: &str, error: impl std::fmt::Display) -> MpsError {
    MpsError::with_source(
        MpsErrorKind::ModelConstruction,
        MpsDiagnostic::new().with_message(format!("cannot read imported {label}")),
        BackendError::new(
            error.to_string(),
            ErrorCategory::Internal,
            HealthEffect::Recoverable,
        ),
    )
}

fn native_names(
    raw: *mut std::ffi::c_void,
    count: usize,
    columns: bool,
) -> Result<Vec<String>, HighsError> {
    let mut names = Vec::with_capacity(count);
    for index in 0..count {
        let index = bindings::HighsInt::try_from(index).map_err(|_| {
            BackendError::new(
                "native name index exceeds HiGHS integer range",
                ErrorCategory::Internal,
                HealthEffect::Recoverable,
            )
        })?;
        let mut buffer = vec![0 as c_char; bindings::kHighsMaximumStringLength as usize];
        let status = if columns {
            unsafe { bindings::Highs_getColName(raw, index, buffer.as_mut_ptr()) }
        } else {
            unsafe { bindings::Highs_getRowName(raw, index, buffer.as_mut_ptr()) }
        };
        check_highs_status(
            status,
            raw,
            if columns {
                "Highs_getColName"
            } else {
                "Highs_getRowName"
            },
        )?;
        let name = unsafe { CStr::from_ptr(buffer.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        names.push(name);
    }
    Ok(names)
}

fn observation_names(count: usize, prefix: &str) -> Vec<String> {
    (0..count)
        .map(|index| format!("{prefix}_{index:06}"))
        .collect()
}

fn normalize_bound(value: f64, infinity: f64) -> f64 {
    if value.is_infinite() || value.abs() >= infinity * 0.5 {
        if value.is_sign_negative() {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        }
    } else {
        value
    }
}

fn checked_index(value: bindings::HighsInt, label: &str) -> Result<usize, HighsError> {
    usize::try_from(value).map_err(|_| {
        BackendError::new(
            format!("HiGHS returned a negative {label}"),
            ErrorCategory::Internal,
            HealthEffect::Recoverable,
        )
    })
}

fn highs_int(value: usize, label: &str) -> Result<bindings::HighsInt, HighsError> {
    bindings::HighsInt::try_from(value).map_err(|_| {
        BackendError::new(
            format!("{label} count exceeds the HiGHS integer range"),
            ErrorCategory::Internal,
            HealthEffect::Recoverable,
        )
    })
}

/// Compare complete normalized Q03 observations.
pub fn compare_mps_structure(
    highs: &HighsMpsSummary,
    roml: &RomlMpsSummary,
    abs_tolerance: f64,
    rel_tolerance: f64,
) -> MpsStructuralComparison {
    let mut differences = Vec::new();
    if highs.columns != roml.columns {
        differences.push(format!("columns: {} != {}", highs.columns, roml.columns));
    }
    if highs.rows != roml.rows {
        differences.push(format!("rows: {} != {}", highs.rows, roml.rows));
    }
    if highs.nonzeros != roml.nonzeros {
        differences.push(format!("nonzeros: {} != {}", highs.nonzeros, roml.nonzeros));
    }
    if highs.objective_sense != roml.objective_sense {
        differences.push(format!(
            "objective sense: {} != {}",
            highs.objective_sense, roml.objective_sense
        ));
    }
    compare_float(
        "objective offset",
        highs.objective_offset,
        roml.objective_offset,
        abs_tolerance,
        rel_tolerance,
        &mut differences,
    );
    if highs.column_semantics.keys().collect::<Vec<_>>()
        != roml.column_semantics.keys().collect::<Vec<_>>()
    {
        differences.push("column names differ".to_owned());
    }
    for (name, native) in &highs.column_semantics {
        let Some(imported) = roml.column_semantics.get(name) else {
            continue;
        };
        compare_float(
            "column cost",
            native.cost,
            imported.cost,
            abs_tolerance,
            rel_tolerance,
            &mut differences,
        );
        compare_float(
            "column lower",
            native.lower,
            imported.lower,
            abs_tolerance,
            rel_tolerance,
            &mut differences,
        );
        compare_float(
            "column upper",
            native.upper,
            imported.upper,
            abs_tolerance,
            rel_tolerance,
            &mut differences,
        );
        if native.integrality != imported.integrality {
            differences.push(format!("column {name:?} integrality differs"));
        }
    }
    if highs.row_semantics.keys().collect::<Vec<_>>()
        != roml.row_semantics.keys().collect::<Vec<_>>()
    {
        differences.push("row names differ".to_owned());
    }
    for (name, native) in &highs.row_semantics {
        let Some(imported) = roml.row_semantics.get(name) else {
            continue;
        };
        compare_float(
            "row lower",
            native.lower,
            imported.lower,
            abs_tolerance,
            rel_tolerance,
            &mut differences,
        );
        compare_float(
            "row upper",
            native.upper,
            imported.upper,
            abs_tolerance,
            rel_tolerance,
            &mut differences,
        );
    }
    if highs.matrix.keys().collect::<Vec<_>>() != roml.matrix.keys().collect::<Vec<_>>() {
        differences.push("matrix coordinates differ".to_owned());
    }
    for (coordinate, native) in &highs.matrix {
        if let Some(imported) = roml.matrix.get(coordinate) {
            compare_float(
                "matrix coefficient",
                *native,
                *imported,
                abs_tolerance,
                rel_tolerance,
                &mut differences,
            );
        }
    }
    MpsStructuralComparison {
        equivalent: differences.is_empty(),
        differences,
    }
}

fn compare_float(
    label: &str,
    native: f64,
    imported: f64,
    abs_tolerance: f64,
    rel_tolerance: f64,
    differences: &mut Vec<String>,
) {
    let equal = if native.is_infinite() || imported.is_infinite() {
        native == imported
    } else {
        (native - imported).abs()
            <= abs_tolerance + rel_tolerance * native.abs().max(imported.abs())
    };
    if !equal {
        differences.push(format!("{label}: {native} != {imported}"));
    }
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
