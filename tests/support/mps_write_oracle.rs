//! Independent mathematical normalization for the P36 writer round-trip tests.
//!
//! This module intentionally knows nothing about writer projection, naming,
//! formatting, reports, or native-solver support. It reconstructs the
//! observable mathematical state from public model snapshots only. Entity
//! identity is assigned by deterministic structural refinement over domains,
//! bounds, objective values, and matrix incidence; no writer naming allocator
//! or projection helper is imported.

use std::collections::{BTreeSet, HashMap};

use roml::{
    model::{Bounds, CoefficientTarget, Sense, VarType},
    snapshot::{ModelSnapshot, VariableEntry},
    Model,
};

const ABS_TOLERANCE: f64 = 1e-10;
const REL_TOLERANCE: f64 = 1e-10;

#[derive(Clone, Debug, PartialEq)]
pub struct NormalizedModel {
    pub variables: Vec<NormalizedVariable>,
    pub rows: Vec<NormalizedRow>,
    pub matrix: Vec<NormalizedMatrixCell>,
    pub objective: NormalizedObjective,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NormalizedVariable {
    pub bounds: Bounds,
    pub var_type: VarType,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NormalizedRow {
    pub bounds: roml::ConstraintBounds,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NormalizedMatrixCell {
    pub row: usize,
    pub variable: usize,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NormalizedObjective {
    pub sense: Sense,
    pub offset: f64,
    pub coefficients: Vec<NormalizedObjectiveCoefficient>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NormalizedObjectiveCoefficient {
    pub variable: usize,
    pub value: f64,
}

/// Extract the active evaluated mathematical model from public APIs.
pub fn normalize(model: &Model) -> NormalizedModel {
    let snapshot = model
        .take_snapshot()
        .expect("legal oracle fixtures must produce a snapshot");
    normalize_snapshot(model, &snapshot)
}

/// Normalize an explicitly captured evaluated snapshot.
pub fn normalize_snapshot(_model: &Model, snapshot: &ModelSnapshot) -> NormalizedModel {
    let active_variables = snapshot
        .variables
        .iter()
        .filter(|entry| entry.active)
        .collect::<Vec<_>>();
    let variable_ids = active_variables
        .iter()
        .enumerate()
        .map(|(ordinal, entry)| (entry.id, ordinal))
        .collect::<HashMap<_, _>>();

    let active_rows = snapshot
        .constraints
        .iter()
        .filter(|entry| entry.active)
        .collect::<Vec<_>>();
    let row_ids = active_rows
        .iter()
        .enumerate()
        .map(|(ordinal, entry)| (entry.id, ordinal))
        .collect::<HashMap<_, _>>();

    let active_objective = snapshot.objectives.iter().find(|entry| entry.active);
    let mut matrix_cells = Vec::new();
    let mut objective_values = vec![0.0; active_variables.len()];
    for cell in &snapshot.cells {
        let (target, variable) = cell.cell_key;
        let Some(&variable_ordinal) = variable_ids.get(&variable) else {
            continue;
        };
        // Structural zero cells are mathematically inert. The writer may use
        // one to keep an otherwise empty variable present in COLUMNS.
        if cell.evaluated_value == 0.0 {
            continue;
        }
        match target {
            CoefficientTarget::Constraint(row) => {
                let Some(&row_ordinal) = row_ids.get(&row) else {
                    continue;
                };
                matrix_cells.push((row_ordinal, variable_ordinal, cell.evaluated_value));
            }
            CoefficientTarget::Objective(objective)
                if active_objective.is_some_and(|entry| entry.id == objective) =>
            {
                objective_values[variable_ordinal] = cell.evaluated_value;
            }
            _ => {}
        }
    }

    let variable_bounds = active_variables
        .iter()
        .map(|entry| effective_bounds(entry))
        .collect::<Vec<_>>();
    let variable_types = active_variables
        .iter()
        .map(|entry| entry.var_type)
        .collect::<Vec<_>>();
    let row_bounds = active_rows
        .iter()
        .map(|entry| entry.bounds)
        .collect::<Vec<_>>();
    let (variable_order, row_order) = structural_orders(
        &variable_bounds,
        &variable_types,
        &row_bounds,
        &matrix_cells,
        &objective_values,
    );
    let variable_positions = positions(&variable_order);
    let row_positions = positions(&row_order);

    let variables = variable_order
        .iter()
        .map(|&ordinal| NormalizedVariable {
            bounds: variable_bounds[ordinal],
            var_type: variable_types[ordinal],
        })
        .collect::<Vec<_>>();
    let rows = row_order
        .iter()
        .map(|&ordinal| NormalizedRow {
            bounds: row_bounds[ordinal],
        })
        .collect::<Vec<_>>();
    let mut matrix = matrix_cells
        .iter()
        .map(|&(row, variable, value)| NormalizedMatrixCell {
            row: row_positions[row],
            variable: variable_positions[variable],
            value,
        })
        .collect::<Vec<_>>();
    matrix.sort_by_key(|cell| (cell.row, cell.variable));
    let mut objective_coefficients = objective_values
        .iter()
        .enumerate()
        .filter(|(_, value)| **value != 0.0)
        .map(|(variable, &value)| NormalizedObjectiveCoefficient {
            variable: variable_positions[variable],
            value,
        })
        .collect::<Vec<_>>();
    objective_coefficients.sort_by_key(|coefficient| coefficient.variable);

    let objective = active_objective.map_or(
        NormalizedObjective {
            sense: Sense::Minimize,
            offset: 0.0,
            coefficients: Vec::new(),
        },
        |entry| NormalizedObjective {
            sense: entry.sense,
            offset: entry.constant,
            coefficients: objective_coefficients,
        },
    );

    NormalizedModel {
        variables,
        rows,
        matrix,
        objective,
    }
}

/// Assign name-independent ordinals by repeatedly refining mathematical
/// structure. This makes MPS column/row order irrelevant without importing
/// writer ordering or name-allocation behavior.
fn structural_orders(
    variable_bounds: &[Bounds],
    variable_types: &[VarType],
    row_bounds: &[roml::ConstraintBounds],
    matrix_cells: &[(usize, usize, f64)],
    objective_values: &[f64],
) -> (Vec<usize>, Vec<usize>) {
    let mut variable_incidence = vec![Vec::new(); variable_bounds.len()];
    let mut row_incidence = vec![Vec::new(); row_bounds.len()];
    for &(row, variable, value) in matrix_cells {
        variable_incidence[variable].push((row, value));
        row_incidence[row].push((variable, value));
    }

    let variable_base = variable_bounds
        .iter()
        .zip(variable_types)
        .zip(objective_values)
        .map(|((bounds, var_type), objective)| {
            format!(
                "{}:{}:{}",
                bounds_key(*bounds),
                var_type_key(*var_type),
                scalar_key(*objective)
            )
        })
        .collect::<Vec<_>>();
    let row_base = row_bounds
        .iter()
        .map(|bounds| bounds_key(Bounds::new(bounds.lower, bounds.upper)))
        .collect::<Vec<_>>();
    let mut variable_labels = canonical_labels(&variable_base);
    let mut row_labels = canonical_labels(&row_base);

    for _ in 0..variable_bounds.len() + row_bounds.len() + 1 {
        let variable_signatures = variable_base
            .iter()
            .enumerate()
            .map(|(variable, base)| {
                let mut incidence = variable_incidence[variable]
                    .iter()
                    .map(|&(row, value)| format!("{}:{}", row_labels[row], scalar_key(value)))
                    .collect::<Vec<_>>();
                incidence.sort();
                format!("{base}|{}", incidence.join(","))
            })
            .collect::<Vec<_>>();
        let row_signatures = row_base
            .iter()
            .enumerate()
            .map(|(row, base)| {
                let mut incidence = row_incidence[row]
                    .iter()
                    .map(|&(variable, value)| {
                        format!("{}:{}", variable_labels[variable], scalar_key(value))
                    })
                    .collect::<Vec<_>>();
                incidence.sort();
                format!("{base}|{}", incidence.join(","))
            })
            .collect::<Vec<_>>();
        let next_variable_labels = canonical_labels(&variable_signatures);
        let next_row_labels = canonical_labels(&row_signatures);
        if next_variable_labels == variable_labels && next_row_labels == row_labels {
            break;
        }
        variable_labels = next_variable_labels;
        row_labels = next_row_labels;
    }

    (order_by_label(&variable_labels), order_by_label(&row_labels))
}

fn canonical_labels(signatures: &[String]) -> Vec<usize> {
    let unique = signatures.iter().cloned().collect::<BTreeSet<_>>();
    let ordered = unique.iter().collect::<Vec<_>>();
    signatures
        .iter()
        .map(|signature| ordered.binary_search(&signature).expect("signature exists"))
        .collect()
}

fn order_by_label(labels: &[usize]) -> Vec<usize> {
    let mut order = (0..labels.len()).collect::<Vec<_>>();
    order.sort_by_key(|&ordinal| labels[ordinal]);
    order
}

fn positions(order: &[usize]) -> Vec<usize> {
    let mut positions = vec![0; order.len()];
    for (position, &ordinal) in order.iter().enumerate() {
        positions[ordinal] = position;
    }
    positions
}

fn bounds_key(bounds: Bounds) -> String {
    format!("{}:{}", scalar_key(bounds.lower), scalar_key(bounds.upper))
}

fn var_type_key(var_type: VarType) -> u8 {
    match var_type {
        VarType::Continuous => 0,
        VarType::Integer => 1,
        VarType::Binary => 2,
    }
}

fn scalar_key(value: f64) -> String {
    let value = if value == 0.0 { 0.0 } else { value };
    format!("{value:016x}", value = value.to_bits())
}

fn effective_bounds(entry: &VariableEntry) -> Bounds {
    entry.fixing.as_ref().map_or(entry.bounds, |fixing| {
        Bounds::new(fixing.value, fixing.value)
    })
}

/// Compare two independently extracted models with the frozen P36 tolerance.
pub fn assert_equivalent(before: &NormalizedModel, after: &NormalizedModel) {
    assert_eq!(before.variables, after.variables, "variable domains differ");
    assert_eq!(before.rows, after.rows, "row bounds differ");

    assert_eq!(
        before.matrix.len(),
        after.matrix.len(),
        "matrix dimensions differ"
    );
    for (index, (expected, actual)) in before.matrix.iter().zip(&after.matrix).enumerate() {
        assert_eq!(expected.row, actual.row, "matrix row differs at {index}");
        assert_eq!(
            expected.variable, actual.variable,
            "matrix variable differs at {index}"
        );
        assert_close(
            &format!("matrix coefficient {index}"),
            expected.value,
            actual.value,
        );
    }

    assert_eq!(
        before.objective.sense, after.objective.sense,
        "objective sense differs"
    );
    assert_close(
        "objective offset",
        before.objective.offset,
        after.objective.offset,
    );
    assert_eq!(
        before.objective.coefficients.len(),
        after.objective.coefficients.len(),
        "objective dimensions differ"
    );
    for (index, (expected, actual)) in before
        .objective
        .coefficients
        .iter()
        .zip(&after.objective.coefficients)
        .enumerate()
    {
        assert_eq!(
            expected.variable, actual.variable,
            "objective variable differs at {index}"
        );
        assert_close(
            &format!("objective coefficient {index}"),
            expected.value,
            actual.value,
        );
    }
}

fn assert_close(field: &str, expected: f64, actual: f64) {
    if expected.is_infinite() || actual.is_infinite() {
        assert_eq!(expected, actual, "{field}: infinite values differ");
        return;
    }
    assert!(
        expected.is_finite() && actual.is_finite(),
        "{field}: non-finite value (expected {expected:?}, actual {actual:?})"
    );
    let tolerance = ABS_TOLERANCE + REL_TOLERANCE * expected.abs().max(actual.abs());
    assert!(
        (expected - actual).abs() <= tolerance,
        "{field}: expected {expected:?}, actual {actual:?}, tolerance {tolerance:?}"
    );
}
