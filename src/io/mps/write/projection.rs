//! Semantic projection for the P36 MPS writer.
//!
//! This module deliberately stops at an evaluated, normalized document.  It
//! does not know about bytes, streams, files, or solver APIs.  The later
//! formatter consumes [`MpsWriteDocument`] and is responsible only for the
//! fixed free-MPS spelling.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::{
    construct::ConstructKind,
    id::{ConId, ObjId, ParamId, VarId},
    io::mps::write::{
        MpsEntityKind, MpsEvaluatedParameter, MpsNamePolicy, MpsWriteContext, MpsWriteError,
        MpsWriteErrorKind, MpsWriteLowering, MpsWriteName, MpsWriteNameMap, MpsWriteReport,
    },
    model::{Bounds, ConstraintBounds, Sense, VarType},
    model::{CoefficientTarget, Model},
    snapshot::{CellEntry, ConstraintEntry, ModelSnapshot, VariableEntry},
    value_expr::ValueExpr,
};

/// The evaluated semantic document handed to the formatter.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MpsWriteDocument {
    /// The model name is metadata only; it never affects entity identity.
    pub(crate) model_name: Option<String>,
    /// The name policy used to allocate the document names.
    pub(crate) name_policy: MpsNamePolicy,
    /// Active variables in canonical snapshot order.
    pub(crate) variables: Vec<MpsWriteVariable>,
    /// Active rows in canonical snapshot order.
    pub(crate) rows: Vec<MpsWriteRow>,
    /// The one active objective, if present.
    pub(crate) objective: Option<MpsWriteObjective>,
    /// The completed report for this projection.
    pub(crate) report: MpsWriteReport,
}

/// One active variable in the normalized write document.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MpsWriteVariable {
    pub(crate) source_id: VarId,
    pub(crate) source_name: Option<String>,
    pub(crate) name: String,
    pub(crate) declared_bounds: Bounds,
    pub(crate) effective_bounds: Bounds,
    pub(crate) var_type: VarType,
}

/// One canonical matrix cell, evaluated exactly once for export.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MpsWriteCell {
    pub(crate) variable: VarId,
    pub(crate) value: f64,
}

/// One active linear constraint row.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MpsWriteRow {
    pub(crate) source_id: ConId,
    pub(crate) source_name: Option<String>,
    pub(crate) name: String,
    pub(crate) bounds: ConstraintBounds,
    pub(crate) cells: Vec<MpsWriteCell>,
}

/// The active scalar objective.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MpsWriteObjective {
    pub(crate) source_id: ObjId,
    pub(crate) source_name: Option<String>,
    pub(crate) name: String,
    pub(crate) sense: Sense,
    pub(crate) constant: f64,
    pub(crate) cells: Vec<MpsWriteCell>,
}

/// Project one model snapshot into an evaluated MPS document.
pub(crate) fn project_model(
    model: &Model,
    name_policy: MpsNamePolicy,
) -> Result<MpsWriteDocument, MpsWriteError> {
    let snapshot = model.take_snapshot().map_err(|error| {
        error_for_model(model, MpsWriteErrorKind::ModelValidation, error.to_string())
    })?;
    project_snapshot(model, &snapshot, name_policy)
}

/// Alias kept intentionally small for the later writer integration seam.
pub(crate) fn project(
    model: &Model,
    name_policy: MpsNamePolicy,
) -> Result<MpsWriteDocument, MpsWriteError> {
    project_model(model, name_policy)
}

pub(crate) fn project_snapshot(
    model: &Model,
    snapshot: &ModelSnapshot,
    name_policy: MpsNamePolicy,
) -> Result<MpsWriteDocument, MpsWriteError> {
    reject_active_constructs(model, snapshot)?;

    let omitted_inactive_entities = snapshot
        .variables
        .iter()
        .filter(|entry| !entry.active)
        .count()
        + snapshot
            .constraints
            .iter()
            .filter(|entry| !entry.active)
            .count()
        + snapshot
            .objectives
            .iter()
            .filter(|entry| !entry.active)
            .count()
        + snapshot
            .constructs
            .iter()
            .filter(|entry| !entry.active)
            .count();

    let active_variables: Vec<_> = snapshot
        .variables
        .iter()
        .filter(|entry| entry.active)
        .collect();
    let active_constraints: Vec<_> = snapshot
        .constraints
        .iter()
        .filter(|entry| entry.active)
        .collect();
    let active_objective = snapshot.objectives.iter().find(|entry| entry.active);

    let objective_source_name = if let Some(entry) = active_objective {
        model
            .objective_name(entry.id)
            .map_err(|error| stale_error(model, MpsEntityKind::Objective, error.to_string()))?
            .map(str::to_owned)
    } else {
        None
    };

    validate_snapshot_cell_references(model, snapshot)?;

    let variable_sources = active_variables
        .iter()
        .map(|entry| {
            Ok(NamedSource {
                source_name: model
                    .variable_name(entry.id)
                    .map_err(|error| {
                        stale_error(model, MpsEntityKind::Variable, error.to_string())
                    })?
                    .map(str::to_owned),
                entity_kind: MpsEntityKind::Variable,
            })
        })
        .collect::<Result<Vec<_>, MpsWriteError>>()?;
    let row_sources = active_constraints
        .iter()
        .map(|entry| {
            Ok(NamedSource {
                source_name: model
                    .constraint_name(entry.id)
                    .map_err(|error| {
                        stale_error(model, MpsEntityKind::Constraint, error.to_string())
                    })?
                    .map(str::to_owned),
                entity_kind: MpsEntityKind::Constraint,
            })
        })
        .collect::<Result<Vec<_>, MpsWriteError>>()?;

    let empty_reserved_names = BTreeSet::new();
    let (variable_names, variable_name_map) = allocate_names(
        &variable_sources,
        name_policy,
        "X",
        model,
        &empty_reserved_names,
    )?;
    let objective_reserved_names = objective_source_name
        .as_deref()
        .filter(|name| valid_mps_name(name))
        .map(|name| BTreeSet::from([name.to_owned()]))
        .unwrap_or_default();
    let (row_names, row_name_map) = allocate_names(
        &row_sources,
        name_policy,
        "R",
        model,
        &objective_reserved_names,
    )?;

    let mut variables = Vec::with_capacity(active_variables.len());
    let mut variable_indexes = HashMap::with_capacity(active_variables.len());
    let mut lowerings = Vec::new();
    for (index, entry) in active_variables.iter().enumerate() {
        validate_variable_bounds(model, entry)?;
        let effective_bounds = entry.fixing.as_ref().map_or(entry.bounds, |fixing| {
            Bounds::new(fixing.value, fixing.value)
        });
        if let Some(fixing) = &entry.fixing {
            if !fixing.value.is_finite() {
                return Err(entity_error(
                    model,
                    MpsWriteErrorKind::NonFiniteValue,
                    MpsEntityKind::Variable,
                    &variable_name_map[index].emitted_name,
                    "persistent fixing value",
                ));
            }
            lowerings.push(MpsWriteLowering::PersistentFixingAsBound {
                variable: entry.id,
                value: fixing.value,
            });
        }
        variable_indexes.insert(entry.id, index);
        variables.push(MpsWriteVariable {
            source_id: entry.id,
            source_name: variable_sources[index].source_name.clone(),
            name: variable_names[index].clone(),
            declared_bounds: entry.bounds,
            effective_bounds,
            var_type: entry.var_type,
        });
        if entry.semicontinuous_lower.is_some() {
            return Err(entity_error(
                model,
                MpsWriteErrorKind::Unrepresentable,
                MpsEntityKind::Variable,
                &variable_names[index],
                "semi-continuous or semi-integer domain",
            ));
        }
    }

    let mut row_indexes = HashMap::with_capacity(active_constraints.len());
    let mut rows = Vec::with_capacity(active_constraints.len());
    for (index, entry) in active_constraints.iter().enumerate() {
        validate_constraint_bounds(model, entry)?;
        row_indexes.insert(entry.id, index);
        rows.push(MpsWriteRow {
            source_id: entry.id,
            source_name: row_sources[index].source_name.clone(),
            name: row_names[index].clone(),
            bounds: entry.bounds,
            cells: Vec::new(),
        });
    }

    let (objective, objective_name, objective_name_map) = if let Some(entry) = active_objective {
        let objective_source = [NamedSource {
            source_name: objective_source_name.clone(),
            entity_kind: MpsEntityKind::Objective,
        }];
        let occupied: BTreeSet<String> = row_names.iter().cloned().collect();
        let (mut names, mut maps) =
            allocate_names(&objective_source, name_policy, "OBJ", model, &occupied)?;
        let allocated_name = names
            .first()
            .cloned()
            .ok_or_else(|| internal_error(model, "objective name allocation is empty"))?;
        if objective_source_name
            .as_deref()
            .is_none_or(|name| !valid_mps_name(name) || allocated_name != name)
        {
            set_first_name(&mut names, &mut maps, "OBJ".to_owned(), model)?;
        }
        let allocated_name = names
            .first()
            .ok_or_else(|| internal_error(model, "objective name allocation is empty"))?;
        if occupied.contains(allocated_name) {
            if name_policy == MpsNamePolicy::StrictPreserve {
                return Err(entity_error(
                    model,
                    MpsWriteErrorKind::NameAllocation,
                    MpsEntityKind::Objective,
                    allocated_name,
                    "objective row collides with a constraint row",
                ));
            }
            let generated = next_generated_name("OBJ", 1, &occupied);
            set_first_name(&mut names, &mut maps, generated, model)?;
        }
        let objective_name = names
            .first()
            .cloned()
            .ok_or_else(|| internal_error(model, "objective name allocation is empty"))?;
        (Some(entry), Some(objective_name), Some(maps))
    } else {
        (None, None, None)
    };

    let mut consumed_parameters = BTreeSet::new();
    let mut consumed_parameter_order = Vec::new();
    for cell in &snapshot.cells {
        match cell.cell_key.0 {
            CoefficientTarget::Constraint(con) => {
                if let Some(&row_index) = row_indexes.get(&con) {
                    append_cell(
                        model,
                        snapshot,
                        cell,
                        &variable_indexes,
                        &mut rows[row_index].cells,
                        &mut consumed_parameters,
                        &mut consumed_parameter_order,
                    )?;
                }
            }
            CoefficientTarget::Objective(_) => {}
        }
    }

    let mut objective_cells = Vec::new();
    if let Some(entry) = objective {
        for cell in &snapshot.cells {
            if cell.cell_key.0 == CoefficientTarget::Objective(entry.id) {
                append_cell(
                    model,
                    snapshot,
                    cell,
                    &variable_indexes,
                    &mut objective_cells,
                    &mut consumed_parameters,
                    &mut consumed_parameter_order,
                )?;
            }
        }
    }

    let objective = if let Some(entry) = objective {
        let objective_name_map = objective_name_map
            .as_ref()
            .ok_or_else(|| internal_error(model, "active objective is missing its name map"))?;
        let objective_name_entry = objective_name_map.first().ok_or_else(|| {
            internal_error(model, "active objective is missing its name allocation")
        })?;
        let objective_emitted_name = objective_name_entry.emitted_name.clone();
        let objective_name = objective_name
            .clone()
            .ok_or_else(|| internal_error(model, "active objective is missing its emitted name"))?;
        Some(MpsWriteObjective {
            source_id: entry.id,
            source_name: model
                .objective_name(entry.id)
                .map_err(|error| stale_error(model, MpsEntityKind::Objective, error.to_string()))?
                .map(str::to_owned),
            name: objective_name,
            sense: entry.sense,
            constant: checked_finite(
                model,
                MpsEntityKind::Objective,
                &objective_emitted_name,
                entry.constant,
                "objective offset",
            )?,
            cells: objective_cells,
        })
    } else {
        None
    };

    let evaluated_parameters = consumed_parameter_order
        .into_iter()
        .map(|parameter_id| {
            let entry = snapshot
                .parameters
                .iter()
                .find(|entry| entry.id == parameter_id)
                .ok_or_else(|| {
                    parameter_error(
                        model,
                        parameter_id,
                        "parameter dependency is absent from the captured snapshot",
                        Vec::new(),
                    )
                })?;
            Ok(MpsEvaluatedParameter {
                id: entry.id,
                name: model
                    .parameter_name(entry.id)
                    .map_err(|error| {
                        stale_error(model, MpsEntityKind::Parameter, error.to_string())
                    })?
                    .map(str::to_owned),
                value: checked_finite(
                    model,
                    MpsEntityKind::Parameter,
                    "parameter",
                    entry.value,
                    "parameter value",
                )?,
            })
        })
        .collect::<Result<Vec<_>, MpsWriteError>>()?;

    let name_map = MpsWriteNameMap {
        variables: variable_name_map,
        rows: row_name_map,
        objective: objective_name_map.and_then(|mut names| names.pop()),
    };
    let nonzeros = rows.iter().map(|row| row.cells.len()).sum::<usize>()
        + objective.as_ref().map_or(0, |obj| obj.cells.len());
    let ranges_vector = rows
        .iter()
        .any(|row| {
            row.bounds.lower.is_finite()
                && row.bounds.upper.is_finite()
                && !row.bounds.is_equality()
        })
        .then(|| "RNG1".to_owned());
    let bounds_vector = variables
        .iter()
        .any(|variable| {
            variable.var_type == VarType::Binary
                || variable.effective_bounds != Bounds::NON_NEGATIVE
        })
        .then(|| "BND1".to_owned());
    let report = MpsWriteReport {
        model_lineage: model.lineage(),
        model_instance: model.instance(),
        model_revision: snapshot.revision,
        evaluated_parameters,
        columns: variables.len(),
        rows: rows.len(),
        nonzeros,
        integer_columns: variables
            .iter()
            .filter(|variable| matches!(variable.var_type, VarType::Integer | VarType::Binary))
            .count(),
        objective_present: objective.is_some(),
        rhs_vector: (!rows.is_empty()).then(|| "RHS1".to_owned()),
        ranges_vector,
        bounds_vector,
        name_map,
        lowerings,
        omitted_inactive_entities,
    };

    Ok(MpsWriteDocument {
        model_name: model.name.clone(),
        name_policy,
        variables,
        rows,
        objective,
        report,
    })
}

#[derive(Clone, Debug)]
struct NamedSource {
    source_name: Option<String>,
    entity_kind: MpsEntityKind,
}

fn allocate_names(
    sources: &[NamedSource],
    policy: MpsNamePolicy,
    prefix: &str,
    model: &Model,
    namespace_reserved: &BTreeSet<String>,
) -> Result<(Vec<String>, Vec<MpsWriteName>), MpsWriteError> {
    let mut counts = BTreeMap::<&str, usize>::new();
    for name in namespace_reserved {
        *counts.entry(name.as_str()).or_default() += 1;
    }
    for source in sources {
        if let Some(name) = source.source_name.as_deref() {
            *counts.entry(name).or_default() += 1;
        }
    }
    let mut reserved = namespace_reserved.clone();
    reserved.extend(
        sources
            .iter()
            .filter_map(|source| source.source_name.clone())
            .filter(|name| valid_mps_name(name)),
    );
    let mut used = BTreeSet::new();
    let mut generated = 0usize;
    let mut emitted = Vec::with_capacity(sources.len());
    let mut mappings = Vec::with_capacity(sources.len());
    for (index, source) in sources.iter().enumerate() {
        let source_name = source.source_name.clone();
        let preserve = source_name.as_deref().is_some_and(|name| {
            valid_mps_name(name) && counts.get(name).copied() == Some(1) && !used.contains(name)
        });
        let name = if preserve {
            match source_name.clone() {
                Some(name) => name,
                None => {
                    return Err(internal_error(
                        model,
                        "name allocator marked a nameless source for preservation",
                    ));
                }
            }
        } else {
            if policy == MpsNamePolicy::StrictPreserve {
                let display = source_name
                    .clone()
                    .unwrap_or_else(|| generated_name(prefix, index + 1));
                return Err(entity_error(
                    model,
                    MpsWriteErrorKind::NameAllocation,
                    source.entity_kind,
                    &display,
                    if source_name.is_some() {
                        "source name is missing, invalid, or colliding"
                    } else {
                        "source name is required by StrictPreserve"
                    },
                ));
            }
            loop {
                generated += 1;
                let candidate = generated_name(prefix, generated);
                if !reserved.contains(&candidate) && !used.contains(&candidate) {
                    break candidate;
                }
            }
        };
        used.insert(name.clone());
        emitted.push(name.clone());
        mappings.push(MpsWriteName {
            entity_kind: source.entity_kind,
            ordinal: index + 1,
            source_name,
            emitted_name: name,
        });
    }
    Ok((emitted, mappings))
}

fn append_cell(
    model: &Model,
    snapshot: &ModelSnapshot,
    cell: &CellEntry,
    variable_indexes: &HashMap<VarId, usize>,
    destination: &mut Vec<MpsWriteCell>,
    consumed_parameters: &mut BTreeSet<ParamId>,
    consumed_parameter_order: &mut Vec<ParamId>,
) -> Result<(), MpsWriteError> {
    if !variable_indexes.contains_key(&cell.cell_key.1) {
        return Ok(());
    }
    let mut dependencies = ordered_dependencies(&cell.value_expr);
    for &dependency in &cell.dependencies {
        if !dependencies.contains(&dependency) {
            dependencies.push(dependency);
        }
    }
    for dependency in dependencies {
        if !snapshot
            .parameters
            .iter()
            .any(|parameter| parameter.id == dependency)
        {
            return Err(missing_parameter_error(
                model,
                dependency,
                cell.dependencies.clone(),
            ));
        }
        if consumed_parameters.insert(dependency) {
            consumed_parameter_order.push(dependency);
        }
    }
    let value = cell.evaluated_value;
    if !value.is_finite() {
        return Err(entity_error(
            model,
            MpsWriteErrorKind::NonFiniteValue,
            MpsEntityKind::MatrixCell,
            "matrix cell",
            "evaluated coefficient",
        ));
    }
    destination.push(MpsWriteCell {
        variable: cell.cell_key.1,
        value,
    });
    Ok(())
}

fn validate_snapshot_cell_references(
    model: &Model,
    snapshot: &ModelSnapshot,
) -> Result<(), MpsWriteError> {
    let variables: BTreeSet<VarId> = snapshot.variables.iter().map(|entry| entry.id).collect();
    let constraints: BTreeSet<ConId> = snapshot.constraints.iter().map(|entry| entry.id).collect();
    let objectives: BTreeSet<ObjId> = snapshot.objectives.iter().map(|entry| entry.id).collect();
    for cell in &snapshot.cells {
        if !variables.contains(&cell.cell_key.1) {
            return Err(entity_error(
                model,
                MpsWriteErrorKind::StaleEntity,
                MpsEntityKind::MatrixCell,
                "matrix cell",
                "coefficient references a stale variable",
            ));
        }
        let target_exists = match cell.cell_key.0 {
            CoefficientTarget::Constraint(id) => constraints.contains(&id),
            CoefficientTarget::Objective(id) => objectives.contains(&id),
        };
        if !target_exists {
            return Err(entity_error(
                model,
                MpsWriteErrorKind::StaleEntity,
                MpsEntityKind::MatrixCell,
                "matrix cell",
                "coefficient references a stale target",
            ));
        }
    }
    Ok(())
}

fn reject_active_constructs(model: &Model, snapshot: &ModelSnapshot) -> Result<(), MpsWriteError> {
    for (ordinal, construct) in snapshot.constructs.iter().enumerate() {
        if construct.active {
            return Err(entity_error(
                model,
                MpsWriteErrorKind::Unrepresentable,
                MpsEntityKind::Construct,
                &format!("construct-{}", ordinal + 1),
                construct_kind_name(&construct.kind),
            ));
        }
    }
    Ok(())
}

fn construct_kind_name(kind: &ConstructKind) -> &'static str {
    match kind {
        ConstructKind::Indicator(_) => "indicator construct",
        ConstructKind::Reification(_) => "reification construct",
        ConstructKind::Boolean(_) => "boolean construct",
        ConstructKind::Cardinality(_) => "cardinality construct",
        ConstructKind::MinMax(_) => "min/max construct",
        ConstructKind::AbsoluteValue(_) => "absolute-value construct",
        ConstructKind::BinaryProduct(_) => "binary-product construct",
        ConstructKind::PiecewiseLinear(_) => "piecewise-linear construct",
        _ => "semantic construct",
    }
}

fn validate_variable_bounds(model: &Model, entry: &VariableEntry) -> Result<(), MpsWriteError> {
    if !entry.bounds.is_valid()
        || entry.bounds.lower.is_nan()
        || entry.bounds.upper.is_nan()
        || entry.bounds.lower == f64::INFINITY
        || entry.bounds.upper == f64::NEG_INFINITY
    {
        return Err(entity_error(
            model,
            MpsWriteErrorKind::ModelValidation,
            MpsEntityKind::Variable,
            "variable",
            "invalid variable bounds",
        ));
    }
    Ok(())
}

fn validate_constraint_bounds(model: &Model, entry: &ConstraintEntry) -> Result<(), MpsWriteError> {
    if !entry.bounds.is_valid()
        || entry.bounds.lower.is_nan()
        || entry.bounds.upper.is_nan()
        || entry.bounds.lower == f64::INFINITY
        || entry.bounds.upper == f64::NEG_INFINITY
    {
        return Err(entity_error(
            model,
            MpsWriteErrorKind::ModelValidation,
            MpsEntityKind::Constraint,
            "constraint",
            "invalid constraint bounds",
        ));
    }
    Ok(())
}

fn checked_finite(
    model: &Model,
    entity_kind: MpsEntityKind,
    entity_name: &str,
    value: f64,
    field: &str,
) -> Result<f64, MpsWriteError> {
    if value.is_finite() {
        Ok(if value == 0.0 { 0.0 } else { value })
    } else {
        Err(entity_error(
            model,
            MpsWriteErrorKind::NonFiniteValue,
            entity_kind,
            entity_name,
            field,
        ))
    }
}

fn ordered_dependencies(expression: &ValueExpr) -> Vec<ParamId> {
    let mut dependencies = Vec::new();
    collect_ordered_dependencies(expression, &mut dependencies);
    dependencies
}

fn collect_ordered_dependencies(expression: &ValueExpr, dependencies: &mut Vec<ParamId>) {
    match expression {
        ValueExpr::Constant(_) => {}
        ValueExpr::Param(id) => {
            if !dependencies.contains(id) {
                dependencies.push(*id);
            }
        }
        ValueExpr::Add(left, right)
        | ValueExpr::Sub(left, right)
        | ValueExpr::Mul(left, right)
        | ValueExpr::Div(left, right) => {
            collect_ordered_dependencies(left, dependencies);
            collect_ordered_dependencies(right, dependencies);
        }
        ValueExpr::Neg(inner) => collect_ordered_dependencies(inner, dependencies),
    }
}

fn generated_name(prefix: &str, ordinal: usize) -> String {
    format!("{prefix}{ordinal:06}")
}

fn next_generated_name(prefix: &str, mut ordinal: usize, occupied: &BTreeSet<String>) -> String {
    loop {
        let candidate = generated_name(prefix, ordinal);
        if !occupied.contains(&candidate) {
            return candidate;
        }
        ordinal += 1;
    }
}

fn valid_mps_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name.chars().all(|character| {
            character.is_ascii() && !character.is_ascii_control() && !character.is_whitespace()
        })
}

fn model_context(model: &Model) -> MpsWriteContext {
    MpsWriteContext::default().with_model_state(
        model.lineage(),
        model.instance(),
        model.current_revision(),
    )
}

fn error_for_model(model: &Model, kind: MpsWriteErrorKind, message: String) -> MpsWriteError {
    MpsWriteError::new(kind, model_context(model).with_message(message))
}

fn stale_error(model: &Model, entity_kind: MpsEntityKind, message: String) -> MpsWriteError {
    MpsWriteError::new(
        MpsWriteErrorKind::StaleEntity,
        model_context(model)
            .with_entity(entity_kind, "stale entity")
            .with_message(message),
    )
}

fn entity_error(
    model: &Model,
    kind: MpsWriteErrorKind,
    entity_kind: MpsEntityKind,
    entity_name: &str,
    feature: &str,
) -> MpsWriteError {
    let mut context = model_context(model)
        .with_entity(entity_kind, entity_name)
        .with_feature(feature);
    if matches!(&kind, MpsWriteErrorKind::NonFiniteValue) {
        context = context.with_numeric_field(feature);
    }
    MpsWriteError::new(kind, context)
}

fn parameter_error(
    model: &Model,
    parameter: ParamId,
    feature: &str,
    dependencies: Vec<ParamId>,
) -> MpsWriteError {
    MpsWriteError::new(
        MpsWriteErrorKind::ParameterEvaluation,
        model_context(model)
            .with_entity(MpsEntityKind::Parameter, "parameter")
            .with_feature(feature)
            .with_parameter_dependencies(if dependencies.is_empty() {
                vec![parameter]
            } else {
                dependencies
            }),
    )
}

fn missing_parameter_error(
    model: &Model,
    parameter: ParamId,
    dependencies: Vec<ParamId>,
) -> MpsWriteError {
    MpsWriteError::new(
        MpsWriteErrorKind::ParameterEvaluation,
        model_context(model)
            .with_entity(MpsEntityKind::MatrixCell, "matrix cell")
            .with_feature(format!("missing parameter dependency {parameter:?}"))
            .with_parameter_dependencies(dependencies),
    )
}

fn set_first_name(
    names: &mut [String],
    mappings: &mut [MpsWriteName],
    name: String,
    model: &Model,
) -> Result<(), MpsWriteError> {
    let emitted_name = names
        .first_mut()
        .ok_or_else(|| internal_error(model, "name allocation is empty"))?;
    let mapping = mappings
        .first_mut()
        .ok_or_else(|| internal_error(model, "name mapping is empty"))?;
    *emitted_name = name.clone();
    mapping.emitted_name = name;
    Ok(())
}

fn internal_error(model: &Model, message: &str) -> MpsWriteError {
    error_for_model(
        model,
        MpsWriteErrorKind::InternalInvariant,
        message.to_owned(),
    )
}
