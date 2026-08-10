//! Independent mathematical normalization for the P36 writer round-trip tests.
//!
//! This module intentionally knows nothing about writer projection, naming,
//! formatting, reports, or native-solver support. It reconstructs the
//! observable mathematical state from public model snapshots and public entity
//! name accessors only. Names are used as mathematical coordinate labels; no
//! writer naming allocator or projection helper is imported.

use std::collections::{HashMap, HashSet};

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
    pub name: String,
    pub bounds: Bounds,
    pub var_type: VarType,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NormalizedRow {
    pub name: String,
    pub bounds: roml::ConstraintBounds,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NormalizedMatrixCell {
    pub row: String,
    pub variable: String,
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
    pub variable: String,
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
    let variable_names = snapshot
        .variables
        .iter()
        .enumerate()
        .map(|(ordinal, entry)| {
            (
                entry.id,
                public_name(_model.variable_name(entry.id), "variable", ordinal),
            )
        })
        .collect::<HashMap<_, _>>();
    let active_variable_ids = active_variables
        .iter()
        .map(|entry| entry.id)
        .collect::<HashSet<_>>();
    let variables = active_variables
        .iter()
        .map(|entry| NormalizedVariable {
            name: variable_names[&entry.id].clone(),
            bounds: effective_bounds(entry),
            var_type: entry.var_type,
        })
        .collect::<Vec<_>>();
    let mut variables = variables;
    variables.sort_by(|left, right| left.name.cmp(&right.name));

    let active_rows = snapshot
        .constraints
        .iter()
        .filter(|entry| entry.active)
        .collect::<Vec<_>>();
    let row_names = snapshot
        .constraints
        .iter()
        .enumerate()
        .map(|(ordinal, entry)| {
            (
                entry.id,
                public_name(_model.constraint_name(entry.id), "row", ordinal),
            )
        })
        .collect::<HashMap<_, _>>();
    let active_row_ids = active_rows
        .iter()
        .map(|entry| entry.id)
        .collect::<HashSet<_>>();
    let rows = active_rows
        .iter()
        .map(|entry| NormalizedRow {
            name: row_names[&entry.id].clone(),
            bounds: entry.bounds,
        })
        .collect::<Vec<_>>();
    let mut rows = rows;
    rows.sort_by(|left, right| left.name.cmp(&right.name));

    let active_objective = snapshot.objectives.iter().find(|entry| entry.active);
    let mut matrix = Vec::new();
    let mut objective_coefficients = Vec::new();
    for cell in &snapshot.cells {
        let (target, variable) = cell.cell_key;
        if !active_variable_ids.contains(&variable) {
            continue;
        }
        // Structural zero cells are mathematically inert. The writer may use
        // one to keep an otherwise empty variable present in COLUMNS.
        if cell.evaluated_value == 0.0 {
            continue;
        }
        match target {
            CoefficientTarget::Constraint(row) => {
                if !active_row_ids.contains(&row) {
                    continue;
                }
                matrix.push(NormalizedMatrixCell {
                    row: row_names[&row].clone(),
                    variable: variable_names[&variable].clone(),
                    value: cell.evaluated_value,
                });
            }
            CoefficientTarget::Objective(objective)
                if active_objective.is_some_and(|entry| entry.id == objective) =>
            {
                objective_coefficients.push(NormalizedObjectiveCoefficient {
                    variable: variable_names[&variable].clone(),
                    value: cell.evaluated_value,
                });
            }
            _ => {}
        }
    }

    matrix.sort_by(|left, right| (&left.row, &left.variable).cmp(&(&right.row, &right.variable)));
    objective_coefficients.sort_by(|left, right| left.variable.cmp(&right.variable));

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

fn effective_bounds(entry: &VariableEntry) -> Bounds {
    entry.fixing.as_ref().map_or(entry.bounds, |fixing| {
        Bounds::new(fixing.value, fixing.value)
    })
}

fn public_name(
    result: Result<Option<&str>, roml::ModelError>,
    kind: &str,
    ordinal: usize,
) -> String {
    result
        .expect("snapshot IDs must remain valid while extracting")
        .map(str::to_owned)
        .unwrap_or_else(|| format!("__unnamed_{kind}_{ordinal:06}"))
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
