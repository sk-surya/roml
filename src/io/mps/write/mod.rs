//! Solver-free deterministic MPS write-back.

mod bounds;
mod error;
mod format;
mod objective;
mod path;
mod projection;
mod types;

pub use error::{
    MpsEntityKind, MpsPathStage, MpsWriteContext, MpsWriteDiagnostic, MpsWriteError,
    MpsWriteErrorKind,
};
pub use types::{
    MpsDestinationPolicy, MpsEvaluatedParameter, MpsNamePolicy, MpsWriteLowering, MpsWriteName,
    MpsWriteNameMap, MpsWriteOptions, MpsWriteReport,
};

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    io,
    path::Path,
};

use crate::{model::VarType, Model};

use self::{
    format::{MpsColumnRecord, MpsEntry, MpsRowKind, MpsRowRecord},
    projection::{MpsWriteCell, MpsWriteDocument as SemanticDocument},
};

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

    /// Serializes the model to a caller-provided stream.
    ///
    /// The model is projected and formatted before the first stream write.
    /// A stream failure may therefore leave a prefix of the deterministic MPS
    /// bytes in the caller's output.
    pub fn write<W: io::Write>(
        &self,
        model: &Model,
        mut output: W,
    ) -> Result<MpsWriteReport, MpsWriteError> {
        let serialized = self.serialize(model)?;
        output.write_all(&serialized.bytes).map_err(|cause| {
            MpsWriteError::with_source(
                MpsWriteErrorKind::Io,
                report_context(&serialized.report).with_message("writing MPS bytes to stream"),
                cause,
            )
        })?;
        Ok(serialized.report)
    }

    /// Serializes the model and commits it according to the destination policy.
    pub fn write_path<P: AsRef<Path>>(
        &self,
        model: &Model,
        path: P,
    ) -> Result<MpsWriteReport, MpsWriteError> {
        let destination = path.as_ref();
        let serialized = self
            .serialize(model)
            .map_err(|error| attach_write_path_context(error, destination))?;
        path::commit_path(
            &serialized.bytes,
            destination,
            self.options.destination_policy,
        )
        .map_err(|error| attach_report_context(error, &serialized.report))?;
        Ok(serialized.report)
    }

    fn serialize(&self, model: &Model) -> Result<SerializedMps, MpsWriteError> {
        let semantic = projection::project(model, self.options.name_policy)?;
        let mut report = semantic.report.clone();
        let normalized = normalize_document(&semantic, &mut report)?;
        let bytes = format::format_document(&normalized).map_err(|error| {
            MpsWriteError::with_source(
                MpsWriteErrorKind::Serialization,
                report_context(&report).with_feature("canonical formatter input"),
                error,
            )
        })?;
        Ok(SerializedMps { bytes, report })
    }
}

struct SerializedMps {
    bytes: Vec<u8>,
    report: MpsWriteReport,
}

fn normalize_document(
    semantic: &SemanticDocument,
    report: &mut MpsWriteReport,
) -> Result<format::MpsWriteDocument, MpsWriteError> {
    validate_projection_report(semantic, report)?;

    let name = normalize_problem_name(semantic, report)?;
    let objective_name = semantic
        .objective
        .as_ref()
        .map(|objective| objective.name.clone())
        .unwrap_or_else(|| {
            let occupied = semantic
                .rows
                .iter()
                .map(|row| row.name.clone())
                .collect::<BTreeSet<_>>();
            if occupied.contains("OBJ") {
                next_generated_name("OBJ", 1, &occupied)
            } else {
                "OBJ".to_owned()
            }
        });
    validate_token_name(&objective_name, report, MpsEntityKind::Objective)?;
    if semantic.objective.is_none() {
        report.name_map.objective = Some(MpsWriteName {
            entity_kind: MpsEntityKind::Objective,
            ordinal: 1,
            source_name: None,
            emitted_name: objective_name.clone(),
        });
    }

    let mut rows = Vec::with_capacity(semantic.rows.len() + 1);
    rows.push(MpsRowRecord {
        kind: MpsRowKind::Free,
        name: objective_name.clone(),
    });

    let objective_encoding = objective::encode_objective(
        semantic.objective.as_ref().map(|objective| objective.sense),
        semantic
            .objective
            .as_ref()
            .map(|objective| objective.constant),
        &objective_name,
        report,
    )?;
    let mut rhs_entries = Vec::new();
    let mut range_entries = Vec::new();
    if let Some(rhs) = objective_encoding.rhs {
        rhs_entries.push(rhs);
    }

    let mut row_names = BTreeSet::new();
    for row in &semantic.rows {
        validate_token_name(&row.name, report, MpsEntityKind::Constraint)?;
        if !row_names.insert(row.name.clone()) {
            return Err(internal_invariant(
                report,
                "projection emitted duplicate constraint row names",
            ));
        }
        let row_encoding = objective::encode_row_bounds(row.bounds, &row.name, report)?;
        rows.push(MpsRowRecord {
            kind: row_encoding.kind,
            name: row.name.clone(),
        });
        rhs_entries.push(row_encoding.rhs);
        if let Some(range) = row_encoding.range {
            range_entries.push(range);
        }
    }

    let variable_indexes = variable_indexes(semantic, report)?;
    let mut entries_by_variable = vec![Vec::new(); semantic.variables.len()];
    let mut seen_cells = HashSet::new();
    for row in &semantic.rows {
        for cell in &row.cells {
            push_cell(
                &mut entries_by_variable,
                &variable_indexes,
                cell,
                &row.name,
                &mut seen_cells,
                report,
            )?;
        }
    }
    if let Some(objective) = &semantic.objective {
        for cell in &objective.cells {
            push_cell(
                &mut entries_by_variable,
                &variable_indexes,
                cell,
                &objective_name,
                &mut seen_cells,
                report,
            )?;
        }
    }

    // MPS declares a column through its first COLUMNS entry. A zero entry in
    // the synthetic objective row is mathematically inert but preserves an
    // otherwise empty canonical variable through the P35 reader.
    for entries in &mut entries_by_variable {
        if entries.is_empty() {
            entries.push(MpsEntry {
                row: objective_name.clone(),
                value: 0.0,
            });
        }
    }

    let columns = bounds::encode_columns(&semantic.variables, entries_by_variable, report)?;
    let bounds = bounds::encode_bounds(&semantic.variables, report)?;

    report.nonzeros = columns
        .iter()
        .filter_map(|column| match column {
            MpsColumnRecord::Entries { entries, .. } => Some(entries),
            MpsColumnRecord::Marker { .. } => None,
        })
        .flatten()
        .filter(|entry| entry.value != 0.0)
        .count();
    report.rhs_vector = (!rhs_entries.is_empty()).then(|| "RHS1".to_owned());
    report.ranges_vector = (!range_entries.is_empty()).then(|| "RNG1".to_owned());
    report.bounds_vector = (!bounds.is_empty()).then(|| "BND1".to_owned());

    Ok(format::MpsWriteDocument {
        name,
        objective_sense: objective_encoding.sense,
        objective_name: Some(objective_name),
        rows,
        columns,
        rhs: (!rhs_entries.is_empty()).then_some(rhs_entries),
        ranges: (!range_entries.is_empty()).then_some(range_entries),
        bounds: (!bounds.is_empty()).then_some(bounds),
    })
}

fn validate_projection_report(
    semantic: &SemanticDocument,
    report: &MpsWriteReport,
) -> Result<(), MpsWriteError> {
    if report.name_policy != semantic.name_policy
        || report.columns != semantic.variables.len()
        || report.rows != semantic.rows.len()
        || report.objective_present != semantic.objective.is_some()
        || report.integer_columns
            != semantic
                .variables
                .iter()
                .filter(|variable| matches!(variable.var_type, VarType::Integer | VarType::Binary))
                .count()
    {
        return Err(internal_invariant(
            report,
            "projection report dimensions do not match the semantic document",
        ));
    }
    Ok(())
}

fn normalize_problem_name(
    semantic: &SemanticDocument,
    report: &MpsWriteReport,
) -> Result<String, MpsWriteError> {
    match semantic.model_name.as_deref() {
        None => Ok("ROML".to_owned()),
        Some(name) if valid_token_name(name) => Ok(name.to_owned()),
        Some(_) if semantic.name_policy == MpsNamePolicy::StrictPreserve => {
            Err(MpsWriteError::new(
                MpsWriteErrorKind::NameAllocation,
                report_context(report)
                    .with_entity(MpsEntityKind::Model, "model")
                    .with_feature("invalid model name under StrictPreserve"),
            ))
        }
        Some(_) => Ok("ROML".to_owned()),
    }
}

fn variable_indexes(
    semantic: &SemanticDocument,
    report: &MpsWriteReport,
) -> Result<HashMap<crate::id::VarId, usize>, MpsWriteError> {
    let mut indexes = HashMap::with_capacity(semantic.variables.len());
    let mut names = BTreeSet::new();
    for (index, variable) in semantic.variables.iter().enumerate() {
        validate_token_name(&variable.name, report, MpsEntityKind::Variable)?;
        if !names.insert(variable.name.clone())
            || indexes.insert(variable.source_id, index).is_some()
        {
            return Err(internal_invariant(
                report,
                "projection emitted duplicate variable identity or name",
            ));
        }
    }
    Ok(indexes)
}

fn push_cell(
    entries_by_variable: &mut [Vec<MpsEntry>],
    variable_indexes: &HashMap<crate::id::VarId, usize>,
    cell: &MpsWriteCell,
    row_name: &str,
    seen_cells: &mut HashSet<(usize, String)>,
    report: &MpsWriteReport,
) -> Result<(), MpsWriteError> {
    let Some(&variable_index) = variable_indexes.get(&cell.variable) else {
        return Err(internal_invariant(
            report,
            "projection cell references a variable absent from the column set",
        ));
    };
    let value = finite_value(cell.value, report, "matrix coefficient")?;
    if !seen_cells.insert((variable_index, row_name.to_owned())) {
        return Err(internal_invariant(
            report,
            "projection contains a duplicate normalized matrix cell",
        ));
    }
    entries_by_variable[variable_index].push(MpsEntry {
        row: row_name.to_owned(),
        value,
    });
    Ok(())
}

fn finite_value(value: f64, report: &MpsWriteReport, field: &str) -> Result<f64, MpsWriteError> {
    if value.is_finite() {
        Ok(if value == 0.0 { 0.0 } else { value })
    } else {
        Err(MpsWriteError::new(
            MpsWriteErrorKind::NonFiniteValue,
            report_context(report).with_numeric_field(field),
        ))
    }
}

fn validate_token_name(
    name: &str,
    report: &MpsWriteReport,
    entity_kind: MpsEntityKind,
) -> Result<(), MpsWriteError> {
    if valid_token_name(name) {
        Ok(())
    } else {
        Err(internal_invariant(
            report,
            format!("projection emitted invalid {entity_kind} MPS name"),
        ))
    }
}

fn valid_token_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name.chars().all(|character| {
            character.is_ascii()
                && !character.is_ascii_control()
                && !character.is_ascii_whitespace()
        })
}

fn next_generated_name(prefix: &str, mut ordinal: usize, occupied: &BTreeSet<String>) -> String {
    loop {
        let candidate = format!("{prefix}{ordinal:06}");
        if !occupied.contains(&candidate) {
            return candidate;
        }
        ordinal += 1;
    }
}

fn report_context(report: &MpsWriteReport) -> MpsWriteContext {
    MpsWriteContext::default().with_model_state(
        report.model_lineage,
        report.model_instance,
        report.model_revision,
    )
}

fn internal_invariant(report: &MpsWriteReport, message: impl Into<String>) -> MpsWriteError {
    MpsWriteError::new(
        MpsWriteErrorKind::InternalInvariant,
        report_context(report).with_message(message),
    )
}

fn attach_report_context(error: MpsWriteError, report: &MpsWriteReport) -> MpsWriteError {
    let mut context = error.context().clone();
    context.model_lineage = Some(report.model_lineage);
    context.model_instance = Some(report.model_instance);
    context.model_revision = Some(report.model_revision);
    error.with_context(context)
}

fn attach_write_path_context(error: MpsWriteError, destination: &Path) -> MpsWriteError {
    let mut context = error.context().clone();
    context.path = Some(destination.to_owned());
    error.with_context(context)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_path_context_keeps_preflight_errors_out_of_filesystem_stages() {
        for kind in [
            MpsWriteErrorKind::Unrepresentable,
            MpsWriteErrorKind::ParameterEvaluation,
            MpsWriteErrorKind::ModelValidation,
        ] {
            let error = attach_write_path_context(
                MpsWriteError::new(kind, MpsWriteContext::default()),
                Path::new("model.mps"),
            );

            assert_eq!(error.context().path(), Some(Path::new("model.mps")));
            assert_eq!(error.context().stage(), None);
        }
    }
}
