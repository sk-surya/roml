//! MPS staging-to-model semantic resolution.

use std::collections::{BTreeMap, HashMap};

use crate::{
    expr::{ConstraintSpec, LinExpr},
    model::{Bounds, Model, ModelError, Sense, VarType},
};

use super::{
    lexer::lex_records,
    record::{BoundKind, LexedDocument, MpsRecord, ObjectiveSense, RowKind},
    staging::{MpsBoundKind, MpsObjectiveSense, MpsRowKind, MpsStagedColumn, MpsStaging},
    MpsBoundOrigin, MpsBoundSide, MpsDiagnostic, MpsError, MpsErrorKind, MpsImport, MpsMetadata,
    MpsReadOptions, MpsSourceMap, MpsVariableBoundOrigin,
};

#[derive(Clone, Debug)]
struct VariableState {
    bounds: Bounds,
    var_type: VarType,
    lower_origin: Option<MpsBoundOrigin>,
    upper_origin: Option<MpsBoundOrigin>,
}

/// Lex and stage one input document for later semantic resolution.
pub(crate) fn stage_input<R: std::io::BufRead>(
    input: R,
    options: &MpsReadOptions,
) -> Result<(LexedDocument, MpsStaging), MpsError> {
    let document = lex_records(input, options.format, &options.limits)?;
    let mut staging = MpsStaging::new(options.limits.clone());
    for record in &document.records {
        stage_record(&mut staging, record)?;
    }
    Ok((document, staging.validate()?))
}

fn stage_record(staging: &mut MpsStaging, record: &MpsRecord) -> Result<(), MpsError> {
    let section = match record {
        MpsRecord::Name { .. } => super::MpsSection::Name,
        MpsRecord::ObjSense { .. } => super::MpsSection::ObjSense,
        MpsRecord::ObjName { .. } => super::MpsSection::ObjName,
        MpsRecord::Row { .. } => super::MpsSection::Rows,
        MpsRecord::Column { .. } | MpsRecord::Marker { .. } => super::MpsSection::Columns,
        MpsRecord::Rhs { .. } => super::MpsSection::Rhs,
        MpsRecord::Ranges { .. } => super::MpsSection::Ranges,
        MpsRecord::Bound { .. } => super::MpsSection::Bounds,
    };
    let mut staged = staging.begin_record(section, record.span().clone())?;
    match record {
        MpsRecord::Name { name, .. } => staged.set_problem_name(name.clone()),
        MpsRecord::ObjSense { sense, .. } => staged.set_objective_sense(match sense {
            ObjectiveSense::Minimize => MpsObjectiveSense::Minimize,
            ObjectiveSense::Maximize => MpsObjectiveSense::Maximize,
        }),
        MpsRecord::ObjName { name, .. } => staged.set_objective_name(name.clone()),
        MpsRecord::Row { kind, name, .. } => staged.add_row(
            match kind {
                RowKind::E => MpsRowKind::Equal,
                RowKind::G => MpsRowKind::GreaterThan,
                RowKind::L => MpsRowKind::LessThan,
                RowKind::N => MpsRowKind::Free,
            },
            name.clone(),
        )?,
        MpsRecord::Column {
            variable,
            entries,
            integer_marker_span,
            ..
        } => {
            for entry in entries {
                staged.add_column_entry(
                    variable.clone(),
                    entry.row.clone(),
                    entry.value,
                    integer_marker_span.as_ref(),
                )?;
            }
        }
        MpsRecord::Marker { marker, .. } => {
            let _ = marker;
        }
        MpsRecord::Rhs {
            vector, entries, ..
        } => {
            for entry in entries {
                staged.add_rhs_entry(vector.clone(), entry.row.clone(), entry.value)?;
            }
        }
        MpsRecord::Ranges {
            vector, entries, ..
        } => {
            for entry in entries {
                staged.add_range_entry(vector.clone(), entry.row.clone(), entry.value)?;
            }
        }
        MpsRecord::Bound {
            kind,
            vector,
            variable,
            value,
            ..
        } => staged.add_bound_entry(
            vector.clone(),
            match kind {
                BoundKind::Fr => MpsBoundKind::Free,
                BoundKind::Fx => MpsBoundKind::Fixed,
                BoundKind::Lo => MpsBoundKind::Lower,
                BoundKind::Mi => MpsBoundKind::MinusInfinity,
                BoundKind::Pl => MpsBoundKind::PlusInfinity,
                BoundKind::Up => MpsBoundKind::Upper,
                BoundKind::Bv => MpsBoundKind::Binary,
                BoundKind::Li => MpsBoundKind::IntegerLower,
                BoundKind::Ui => MpsBoundKind::IntegerUpper,
            },
            variable.clone(),
            *value,
        )?,
    }
    Ok(())
}

/// Resolve a structurally valid staging document into a fresh ROML model.
pub(crate) fn resolve(
    document: &LexedDocument,
    staging: MpsStaging,
    options: &MpsReadOptions,
) -> Result<MpsImport, MpsError> {
    let rhs = staging.rhs_vectors().select(&options.rhs)?;
    let ranges = staging.range_vectors().select(&options.ranges)?;
    let bounds = staging.bound_vectors().select(&options.bounds)?;

    let rows: BTreeMap<_, _> = staging.rows().iter().map(|row| (row.name(), row)).collect();
    let columns: BTreeMap<_, _> = staging
        .columns()
        .iter()
        .map(|column| (column.name(), column))
        .collect();

    let objective_row = select_objective(&staging, &rows)?;
    let rhs_values = selected_rhs(rhs)?;
    let ranges_values = selected_ranges(ranges, &rows)?;
    let variable_states = resolve_variable_states(&columns, bounds)?;

    let mut model = staging
        .problem_name()
        .map_or_else(Model::new, Model::with_name);
    let mut variable_ids = HashMap::with_capacity(columns.len());
    for (name, state) in &variable_states {
        let id = add_variable(&mut model, name, state).map_err(model_error)?;
        variable_ids.insert(name.clone(), id);
    }
    let row_expressions = row_expressions(&staging, &variable_ids);

    let mut source_map = MpsSourceMap::default();
    for row in staging.rows() {
        source_map
            .row_spans
            .insert(row.name().to_owned(), row.span().clone());
    }
    for column in staging.columns() {
        if let Some(span) = column.first_entry_span() {
            source_map
                .column_spans
                .insert(column.name().to_owned(), span.clone());
        }
    }
    for (name, state) in &variable_states {
        if let Some(origin) = &state.lower_origin {
            source_map
                .variable_bound_origins
                .push(MpsVariableBoundOrigin {
                    variable: name.clone(),
                    side: MpsBoundSide::Lower,
                    origin: origin.clone(),
                });
        }
        if let Some(origin) = &state.upper_origin {
            source_map
                .variable_bound_origins
                .push(MpsVariableBoundOrigin {
                    variable: name.clone(),
                    side: MpsBoundSide::Upper,
                    origin: origin.clone(),
                });
        }
    }
    source_map
        .variable_bound_origins
        .sort_by(|left, right| (&left.variable, left.side).cmp(&(&right.variable, right.side)));

    let objective_sense = staging.objective_sense().map_or(Sense::Minimize, mps_sense);
    let objective_id = objective_row
        .as_ref()
        .map(|name| model.add_objective_named(objective_sense, name.clone()))
        .unwrap_or_else(|| model.add_objective(objective_sense));
    let objective_expr = objective_row
        .as_deref()
        .and_then(|row| row_expressions.get(row).cloned())
        .unwrap_or_default();
    model
        .set_objective_expr(objective_id, objective_expr)
        .map_err(model_error)?;
    let objective_rhs = objective_row
        .as_deref()
        .and_then(|row| rhs_values.get(row).copied())
        .unwrap_or(0.0);
    model.set_objective_constant_internal(objective_id, -objective_rhs);
    model
        .set_active_objective(objective_id)
        .map_err(model_error)?;

    for row in staging.rows() {
        if row.kind() == MpsRowKind::Free {
            continue;
        }
        let rhs_value = rhs_values.get(row.name()).copied().unwrap_or(0.0);
        let range_value = ranges_values.get(row.name()).copied();
        let constraint_bounds =
            row_bounds(row.kind(), rhs_value, range_value, row.name(), row.span())?;
        let expression = row_expressions.get(row.name()).cloned().unwrap_or_default();
        model
            .add_constraint(ConstraintSpec::new(expression, constraint_bounds).named(row.name()))
            .map_err(model_error)?;
    }

    let metadata = MpsMetadata {
        format: document.format,
        problem_name: staging.problem_name().map(str::to_owned),
        objective_row,
        objective_sense: Some(objective_sense),
        rhs_vector: rhs.map(|vector| vector.name().to_owned()),
        ranges_vector: ranges.map(|vector| vector.name().to_owned()),
        bounds_vector: bounds.map(|vector| vector.name().to_owned()),
    };
    Ok(MpsImport {
        model,
        metadata,
        source_map,
        diagnostics: Vec::new(),
    })
}

fn select_objective(
    staging: &MpsStaging,
    rows: &BTreeMap<&str, &super::staging::MpsStagedRow>,
) -> Result<Option<String>, MpsError> {
    let selected = staging.objective_name().map(str::to_owned).or_else(|| {
        staging
            .rows()
            .iter()
            .find(|row| row.kind() == MpsRowKind::Free)
            .map(|row| row.name().to_owned())
    });
    if let Some(name) = &selected {
        if rows
            .get(name.as_str())
            .is_none_or(|row| row.kind() != MpsRowKind::Free)
        {
            return Err(semantic_error(
                MpsErrorKind::InvalidRecord,
                super::MpsSection::ObjName,
                name,
                staging.objective_name_span(),
                "OBJNAME must identify an N row",
            ));
        }
    }
    Ok(selected)
}

fn selected_rhs(
    vector: Option<&super::vectors::MpsNamedVector<super::staging::MpsRhsEntry>>,
) -> Result<BTreeMap<String, f64>, MpsError> {
    let mut values = BTreeMap::new();
    if let Some(vector) = vector {
        for entry in vector.entries() {
            if values
                .insert(entry.row_name().to_owned(), entry.value())
                .is_some()
            {
                return Err(semantic_error(
                    MpsErrorKind::DuplicateRhsEntry,
                    super::MpsSection::Rhs,
                    entry.row_name(),
                    Some(entry.span()),
                    "selected RHS vector repeats a row",
                ));
            }
        }
    }
    Ok(values)
}

fn selected_ranges(
    vector: Option<&super::vectors::MpsNamedVector<super::staging::MpsRangeEntry>>,
    rows: &BTreeMap<&str, &super::staging::MpsStagedRow>,
) -> Result<BTreeMap<String, f64>, MpsError> {
    let mut values = BTreeMap::new();
    if let Some(vector) = vector {
        for entry in vector.entries() {
            if values
                .insert(entry.row_name().to_owned(), entry.value())
                .is_some()
            {
                return Err(semantic_error(
                    MpsErrorKind::DuplicateRangeEntry,
                    super::MpsSection::Ranges,
                    entry.row_name(),
                    Some(entry.span()),
                    "selected RANGES vector repeats a row",
                ));
            }
            if rows
                .get(entry.row_name())
                .is_some_and(|row| row.kind() == MpsRowKind::Free)
            {
                return Err(semantic_error(
                    MpsErrorKind::InvalidRangeForNRow,
                    super::MpsSection::Ranges,
                    entry.row_name(),
                    Some(entry.span()),
                    "RANGES does not define a semantic transformation for an N row",
                ));
            }
        }
    }
    Ok(values)
}

fn resolve_variable_states(
    columns: &BTreeMap<&str, &MpsStagedColumn>,
    selected: Option<&super::vectors::MpsNamedVector<super::staging::MpsBoundEntry>>,
) -> Result<BTreeMap<String, VariableState>, MpsError> {
    let mut states = BTreeMap::new();
    for (name, column) in columns {
        let marked = column.first_integer_marker_span();
        let (var_type, bounds, lower_origin, upper_origin) = if let Some(marker_span) = marked {
            let columns_span = column.first_marked_entry_span().ok_or_else(|| {
                semantic_error(
                    MpsErrorKind::InvalidRecord,
                    super::MpsSection::Columns,
                    name,
                    Some(marker_span),
                    "integer marker has no marked COLUMNS entry",
                )
            })?;
            (
                VarType::Integer,
                Bounds::new(0.0, 1.0),
                Some(MpsBoundOrigin::ImplicitIntegerMarkerDefault {
                    marker_span: marker_span.clone(),
                    columns_span: columns_span.clone(),
                }),
                Some(MpsBoundOrigin::ImplicitIntegerMarkerDefault {
                    marker_span: marker_span.clone(),
                    columns_span: columns_span.clone(),
                }),
            )
        } else {
            let columns_span = column.first_entry_span().ok_or_else(|| {
                semantic_error(
                    MpsErrorKind::InvalidRecord,
                    super::MpsSection::Columns,
                    name,
                    None,
                    "COLUMNS declaration has no matrix entry",
                )
            })?;
            (
                VarType::Continuous,
                Bounds::NON_NEGATIVE,
                Some(MpsBoundOrigin::ImplicitContinuousDefault {
                    columns_span: columns_span.clone(),
                }),
                None,
            )
        };
        states.insert(
            (*name).to_owned(),
            VariableState {
                bounds,
                var_type,
                lower_origin,
                upper_origin,
            },
        );
    }
    if let Some(vector) = selected {
        for entry in vector.entries() {
            let Some(state) = states.get_mut(entry.variable_name()) else {
                return Err(semantic_error(
                    MpsErrorKind::UnknownVariable,
                    super::MpsSection::Bounds,
                    entry.variable_name(),
                    Some(entry.span()),
                    "BOUNDS references an undeclared variable",
                ));
            };
            let origin = MpsBoundOrigin::Explicit {
                span: entry.span().clone(),
            };
            match entry.kind() {
                MpsBoundKind::Free => {
                    state.bounds = Bounds::UNBOUNDED;
                    state.lower_origin = None;
                    state.upper_origin = None;
                }
                MpsBoundKind::Fixed => {
                    let value = bound_value(entry, "FX requires a value")?;
                    state.bounds = Bounds::fixed(value, Some(0.0));
                    state.lower_origin = Some(origin.clone());
                    state.upper_origin = Some(origin);
                }
                MpsBoundKind::Lower => {
                    state.bounds.lower = bound_value(entry, "LO requires a value")?;
                    state.lower_origin = Some(origin);
                }
                MpsBoundKind::MinusInfinity => {
                    state.bounds.lower = f64::NEG_INFINITY;
                    state.lower_origin = None;
                }
                MpsBoundKind::PlusInfinity => {
                    state.bounds.upper = f64::INFINITY;
                    state.upper_origin = None;
                }
                MpsBoundKind::Upper => {
                    state.bounds.upper = bound_value(entry, "UP requires a value")?;
                    state.upper_origin = Some(origin);
                }
                MpsBoundKind::Binary => {
                    state.var_type = VarType::Binary;
                    state.bounds = Bounds::BINARY;
                    state.lower_origin = Some(origin.clone());
                    state.upper_origin = Some(origin);
                }
                MpsBoundKind::IntegerLower => {
                    state.var_type = VarType::Integer;
                    state.bounds.lower = bound_value(entry, "LI requires a value")?.ceil();
                    state.lower_origin = Some(origin);
                }
                MpsBoundKind::IntegerUpper => {
                    state.var_type = VarType::Integer;
                    state.bounds.upper = bound_value(entry, "UI requires a value")?.floor();
                    state.upper_origin = Some(origin);
                }
            }
            if !state.bounds.is_valid()
                || (state.var_type == VarType::Binary
                    && (state.bounds.lower < 0.0 || state.bounds.upper > 1.0))
            {
                return Err(semantic_error(
                    MpsErrorKind::InvalidBound,
                    super::MpsSection::Bounds,
                    entry.variable_name(),
                    Some(entry.span()),
                    "ordered BOUNDS transitions produce an invalid variable domain",
                ));
            }
        }
    }
    Ok(states)
}

fn add_variable(
    model: &mut Model,
    name: &str,
    state: &VariableState,
) -> Result<crate::Variable, ModelError> {
    let definition = match state.var_type {
        VarType::Continuous => crate::continuous(),
        VarType::Integer => crate::integer(),
        VarType::Binary => crate::binary(),
    }
    .bounds(state.bounds.lower, state.bounds.upper)
    .named(name.to_owned());
    model.add_variable(definition)
}

fn row_expressions(
    staging: &MpsStaging,
    variable_ids: &HashMap<String, crate::Variable>,
) -> BTreeMap<String, LinExpr> {
    let mut expressions: BTreeMap<String, LinExpr> = BTreeMap::new();
    for column in staging.columns() {
        if let Some(var) = variable_ids.get(column.name()) {
            for entry in column.entries() {
                let expression = expressions.entry(entry.row_name().to_owned()).or_default();
                let current = std::mem::take(expression);
                *expression = current.term(entry.value(), *var);
            }
        }
    }
    expressions
}

fn bound_value(
    entry: &super::staging::MpsBoundEntry,
    message: &'static str,
) -> Result<f64, MpsError> {
    entry.value().ok_or_else(|| {
        semantic_error(
            MpsErrorKind::InvalidRecord,
            super::MpsSection::Bounds,
            entry.variable_name(),
            Some(entry.span()),
            message,
        )
    })
}

fn row_bounds(
    kind: MpsRowKind,
    rhs: f64,
    range: Option<f64>,
    row_name: &str,
    span: &super::MpsSourceSpan,
) -> Result<crate::model::ConstraintBounds, MpsError> {
    let bounds = match (kind, range) {
        (MpsRowKind::Equal, None) => crate::model::ConstraintBounds::eq(rhs),
        (MpsRowKind::GreaterThan, None) => crate::model::ConstraintBounds::ge(rhs),
        (MpsRowKind::LessThan, None) => crate::model::ConstraintBounds::le(rhs),
        (MpsRowKind::Equal, Some(value)) if value >= 0.0 => {
            crate::model::ConstraintBounds::range(rhs, rhs + value)
        }
        (MpsRowKind::Equal, Some(value)) => crate::model::ConstraintBounds::range(rhs + value, rhs),
        (MpsRowKind::GreaterThan, Some(value)) => {
            crate::model::ConstraintBounds::range(rhs, rhs + value.abs())
        }
        (MpsRowKind::LessThan, Some(value)) => {
            crate::model::ConstraintBounds::range(rhs - value.abs(), rhs)
        }
        (MpsRowKind::Free, Some(_)) => {
            return Err(semantic_error(
                MpsErrorKind::InvalidRangeForNRow,
                super::MpsSection::Ranges,
                row_name,
                Some(span),
                "RANGES does not define a semantic transformation for an N row",
            ));
        }
        (MpsRowKind::Free, None) => {
            return Err(semantic_error(
                MpsErrorKind::InvalidRangeForNRow,
                super::MpsSection::Ranges,
                row_name,
                Some(span),
                "an N row cannot become a ROML constraint",
            ));
        }
    };
    if !bounds.is_valid() {
        return Err(semantic_error(
            MpsErrorKind::InvalidRange,
            super::MpsSection::Ranges,
            row_name,
            Some(span),
            "RANGES produced an invalid constraint interval",
        ));
    }
    Ok(bounds)
}

fn mps_sense(sense: MpsObjectiveSense) -> Sense {
    match sense {
        MpsObjectiveSense::Minimize => Sense::Minimize,
        MpsObjectiveSense::Maximize => Sense::Maximize,
    }
}

fn model_error(error: ModelError) -> MpsError {
    MpsError::with_source(
        MpsErrorKind::ModelConstruction,
        MpsDiagnostic::new().with_message("ROML model construction failed"),
        error,
    )
}

fn semantic_error(
    kind: MpsErrorKind,
    section: super::MpsSection,
    entity: &str,
    span: Option<&super::MpsSourceSpan>,
    message: &str,
) -> MpsError {
    let mut diagnostic = MpsDiagnostic::new()
        .with_section(section)
        .with_entity(entity)
        .with_message(message);
    if let Some(span) = span {
        diagnostic = diagnostic.with_span(span.clone());
    }
    MpsError::new(kind, diagnostic)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::io::mps::{MpsBoundOrigin, MpsBoundSide, MpsFormat, MpsVectorSelection};

    fn import(input: &str, options: MpsReadOptions) -> Result<MpsImport, MpsError> {
        let (document, staging) = stage_input(Cursor::new(input.as_bytes()), &options)?;
        resolve(&document, staging, &options)
    }

    #[test]
    fn resolves_objective_offset_rows_ranges_and_duplicate_cells() {
        let imported = import(
            "NAME P\nROWS\n N OBJ\n G R1\n L R2\nCOLUMNS\n X OBJ 2 R1 3\n X OBJ 4 R2 5\n X R1 1 R2 1\nRHS\n RHS1 OBJ 7 R1 8\n RHS1 R2 9\nRANGES\n RNG1 R1 2 R2 -3\nENDATA\n",
            MpsReadOptions::default(),
        )
        .expect("valid MPS semantics");
        let objective = imported.model.active_objective().expect("objective");
        assert_eq!(imported.model.objective_constant(objective), Some(-7.0));
        assert_eq!(imported.model.num_constraints(), 2);
        assert_eq!(imported.model.num_variables(), 1);
    }

    #[test]
    fn selected_duplicate_rim_entries_and_range_on_n_are_typed_errors() {
        let duplicate = import(
            "ROWS\n N OBJ\n L R1\nCOLUMNS\n X R1 1\nRHS\n R OBJ 0 R1 1\n R R1 2\nENDATA\n",
            MpsReadOptions::default(),
        )
        .expect_err("duplicate selected RHS must reject");
        assert_eq!(duplicate.kind(), &MpsErrorKind::DuplicateRhsEntry);

        let options = MpsReadOptions {
            ranges: MpsVectorSelection::Named("R".to_owned()),
            ..MpsReadOptions::default()
        };
        let error = import(
            "ROWS\n N OBJ\n L R1\nCOLUMNS\n X R1 1\nRANGES\n R OBJ 1\nENDATA\n",
            options,
        )
        .expect_err("selected RANGE on N must reject");
        assert_eq!(error.kind(), &MpsErrorKind::InvalidRangeForNRow);
    }

    #[test]
    fn marker_fr_is_unbounded_integer_and_defaults_have_origins() {
        let imported = import(
            "ROWS\n N OBJ\nCOLUMNS\n M1 'MARKER' 'INTORG'\n X OBJ 1\n M2 'MARKER' 'INTEND'\n Y OBJ 2\nBOUNDS\n FR B X\nENDATA\n",
            MpsReadOptions {
                format: MpsFormat::Free,
                ..MpsReadOptions::default()
            },
        )
        .expect("marker semantics must resolve");
        let x = imported
            .model
            .variables
            .iter()
            .find(|(_, data)| data.name.as_deref() == Some("X"))
            .and_then(|(id, _)| imported.model.variable_domain(id));
        assert!(x.is_some_and(
            |domain| domain.var_type == VarType::Integer && domain.bounds == Bounds::UNBOUNDED
        ));
        let origins = imported.source_map.variable_bound_origins();
        assert!(origins.iter().any(|origin| {
            origin.variable == "Y"
                && origin.side == MpsBoundSide::Lower
                && matches!(
                    origin.origin,
                    MpsBoundOrigin::ImplicitContinuousDefault { .. }
                )
        }));
    }

    #[test]
    fn row_expression_index_preserves_each_sparse_entry_once() {
        let input =
            "ROWS\n N OBJ\n L R1\n G R2\nCOLUMNS\n X OBJ 2 R1 3\n X R1 4 R2 5\n Y R1 6\nENDATA\n";
        let options = MpsReadOptions::default();
        let (_, staging) = stage_input(Cursor::new(input.as_bytes()), &options)
            .expect("staging must accept the sparse fixture");
        let mut variable_ids = HashMap::new();
        variable_ids.insert(
            "X".to_owned(),
            crate::id::VarId::new(0, crate::id::Generation::new()),
        );
        variable_ids.insert(
            "Y".to_owned(),
            crate::id::VarId::new(1, crate::id::Generation::new()),
        );

        let expressions = row_expressions(&staging, &variable_ids);

        assert_eq!(expressions["OBJ"].terms().len(), 1);
        assert_eq!(expressions["R1"].terms().len(), 3);
        assert_eq!(expressions["R2"].terms().len(), 1);
    }
}
