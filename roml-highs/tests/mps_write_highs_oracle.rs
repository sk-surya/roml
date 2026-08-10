//! P36 Wave 2D: independent direct/native-via-MPS HiGHS qualification.
//!
//! The test owns its comparison normalization. It does not call MPS writer
//! projection, formatter, naming, report, or the P35 MPS oracle helpers as
//! an expected-value source.

use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use roml::io::mps::MpsWriter;
use roml::{continuous, integer, ConstraintExprExt, Model};
use roml_highs::{Highs, HighsMpsSummary, HighsSession};

const STRUCTURAL_ABS_TOLERANCE: f64 = 1e-10;
const STRUCTURAL_REL_TOLERANCE: f64 = 1e-10;
const SOLVE_ABS_TOLERANCE: f64 = 1e-7;
const SOLVE_REL_TOLERANCE: f64 = 1e-8;

static NEXT_PATH: AtomicU64 = AtomicU64::new(0);

fn temporary_mps_path(case_name: &str) -> PathBuf {
    let serial = NEXT_PATH.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "roml-p36-highs-oracle-{case_name}-{}-{serial}.mps",
        std::process::id()
    ))
}

fn assert_native_structure_equivalent(case_name: &str, model: &Model) -> PathBuf {
    let direct = HighsSession::observe_model_structure(model)
        .expect("direct ROML->HiGHS structure must load");
    let path = temporary_mps_path(case_name);
    MpsWriter::new()
        .write_path(model, &path)
        .expect("ROML->MPS write must succeed");
    let mut native_mps = HighsSession::try_new().expect("bundled HiGHS must be available");
    let from_mps = native_mps
        .read_model_summary(&path)
        .expect("native Highs_readModel must accept the P36 output");
    let differences = compare_native_structures(&direct, &from_mps);
    assert!(
        differences.is_empty(),
        "{case_name}: direct/native-via-MPS structure mismatch; unresolved mismatch requires a frozen disposition (roml_writer_bug, roml_reader_bug, roml_projection_bug, backend_oracle_limitation, intentional_roml_rejection, or corpus_out_of_contract):\n{}",
        differences.join("\n")
    );
    path
}

fn assert_solve_equivalent(case_name: &str, model: &Model, path: &PathBuf) {
    let mut direct_model = model.clone();
    let mut direct_highs = Highs::new().expect("bundled HiGHS must be available");
    let direct = direct_highs
        .solve(&mut direct_model)
        .expect("direct ROML->HiGHS solve must succeed");

    let mut native_mps = HighsSession::try_new().expect("bundled HiGHS must be available");
    native_mps
        .read_model_summary(path)
        .expect("native Highs_readModel must accept the P36 output");
    let from_mps = native_mps
        .solve_loaded_mps_model()
        .expect("native MPS solve must return a termination");

    assert_eq!(
        direct.status(),
        from_mps.status,
        "{case_name}: direct and native-via-MPS termination classes differ"
    );
    match (direct.objective_value(), from_mps.objective_value) {
        (Some(direct), Some(mps)) => assert_close(
            "objective",
            direct,
            mps,
            SOLVE_ABS_TOLERANCE,
            SOLVE_REL_TOLERANCE,
            case_name,
        ),
        (None, None) => {}
        (direct, mps) => {
            panic!("{case_name}: objective presence differs: direct={direct:?}, mps={mps:?}")
        }
    }
}

#[test]
fn native_oracle_qualifies_optimal_lp_and_milp() {
    let mut lp = Model::named("oracle-lp");
    let x = lp
        .add_variable(continuous().bounds(0.0, 10.0).named("x"))
        .unwrap();
    let y = lp
        .add_variable(continuous().bounds(0.0, 10.0).named("y"))
        .unwrap();
    lp.add_constraint((x + y).le(4.0).named("capacity"))
        .unwrap();
    lp.maximize(3.0 * x + y + 2.0).unwrap();
    let path = assert_native_structure_equivalent("optimal-lp", &lp);
    assert_solve_equivalent("optimal-lp", &lp, &path);
    fs::remove_file(path).unwrap();

    let mut milp = Model::named("oracle-milp");
    let x = milp
        .add_variable(integer().bounds(0.0, 10.0).named("x"))
        .unwrap();
    let y = milp
        .add_variable(integer().bounds(0.0, 10.0).named("y"))
        .unwrap();
    milp.add_constraint((x + y).le(5.0).named("capacity"))
        .unwrap();
    milp.maximize(3.0 * x + y).unwrap();
    let path = assert_native_structure_equivalent("optimal-milp", &milp);
    assert_solve_equivalent("optimal-milp", &milp, &path);
    fs::remove_file(path).unwrap();
}

#[test]
fn native_oracle_qualifies_infeasible_unbounded_and_no_objective_cases() {
    let mut infeasible = Model::named("oracle-infeasible");
    let x = infeasible
        .add_variable(continuous().bounds(0.0, 1.0).named("x"))
        .unwrap();
    infeasible
        .add_constraint(x.ge(2.0).named("contradiction"))
        .unwrap();
    infeasible.maximize(x).unwrap();
    let path = assert_native_structure_equivalent("infeasible", &infeasible);
    assert_solve_equivalent("infeasible", &infeasible, &path);
    fs::remove_file(path).unwrap();

    let mut unbounded = Model::named("oracle-unbounded");
    let x = unbounded
        .add_variable(continuous().lower_bound(0.0).named("x"))
        .unwrap();
    unbounded.maximize(x).unwrap();
    let path = assert_native_structure_equivalent("unbounded", &unbounded);
    assert_solve_equivalent("unbounded", &unbounded, &path);
    fs::remove_file(path).unwrap();

    let mut no_objective = Model::named("oracle-no-objective");
    let x = no_objective
        .add_variable(integer().bounds(-3.0, 3.0).named("x"))
        .unwrap();
    no_objective.add_constraint(x.eq(1.0).named("pin")).unwrap();
    let path = assert_native_structure_equivalent("no-objective", &no_objective);
    assert_solve_equivalent("no-objective", &no_objective, &path);
    fs::remove_file(path).unwrap();
}

#[test]
fn native_oracle_qualifies_ranged_rows_and_free_integer_structure() {
    let mut model = Model::named("oracle-ranged-free-integer");
    let x = model
        .add_variable(
            integer()
                .bounds(f64::NEG_INFINITY, f64::INFINITY)
                .named("x"),
        )
        .unwrap();
    let y = model
        .add_variable(continuous().bounds(-2.0, 8.0).named("y"))
        .unwrap();
    model
        .add_constraint((x + y).between(1.0, 5.0).named("band"))
        .unwrap();
    let path = assert_native_structure_equivalent("ranged-free-integer", &model);
    fs::remove_file(path).unwrap();
}

fn compare_native_structures(direct: &HighsMpsSummary, from_mps: &HighsMpsSummary) -> Vec<String> {
    let mut differences = Vec::new();
    if direct.columns != from_mps.columns {
        differences.push(format!(
            "columns: {} != {}",
            direct.columns, from_mps.columns
        ));
    }
    if direct.rows != from_mps.rows {
        differences.push(format!("rows: {} != {}", direct.rows, from_mps.rows));
    }
    if direct.nonzeros != from_mps.nonzeros {
        differences.push(format!(
            "nonzeros: {} != {}",
            direct.nonzeros, from_mps.nonzeros
        ));
    }
    if direct.objective_sense != from_mps.objective_sense {
        differences.push(format!(
            "objective sense: {} != {}",
            direct.objective_sense, from_mps.objective_sense
        ));
    }
    compare_scalar(
        "objective offset",
        direct.objective_offset,
        from_mps.objective_offset,
        STRUCTURAL_ABS_TOLERANCE,
        STRUCTURAL_REL_TOLERANCE,
        &mut differences,
    );

    let direct_columns = ordered_columns(direct);
    let mps_columns = ordered_columns(from_mps);
    for (index, (direct, mps)) in direct_columns.iter().zip(mps_columns.iter()).enumerate() {
        compare_scalar(
            &format!("column {index} cost"),
            direct.cost,
            mps.cost,
            STRUCTURAL_ABS_TOLERANCE,
            STRUCTURAL_REL_TOLERANCE,
            &mut differences,
        );
        compare_scalar(
            &format!("column {index} lower"),
            direct.lower,
            mps.lower,
            STRUCTURAL_ABS_TOLERANCE,
            STRUCTURAL_REL_TOLERANCE,
            &mut differences,
        );
        compare_scalar(
            &format!("column {index} upper"),
            direct.upper,
            mps.upper,
            STRUCTURAL_ABS_TOLERANCE,
            STRUCTURAL_REL_TOLERANCE,
            &mut differences,
        );
        if direct.integrality != mps.integrality {
            differences.push(format!("column {index} integrality differs"));
        }
    }
    for (index, (direct, mps)) in ordered_rows(direct)
        .iter()
        .zip(ordered_rows(from_mps))
        .enumerate()
    {
        compare_scalar(
            &format!("row {index} lower"),
            direct.lower,
            mps.lower,
            STRUCTURAL_ABS_TOLERANCE,
            STRUCTURAL_REL_TOLERANCE,
            &mut differences,
        );
        compare_scalar(
            &format!("row {index} upper"),
            direct.upper,
            mps.upper,
            STRUCTURAL_ABS_TOLERANCE,
            STRUCTURAL_REL_TOLERANCE,
            &mut differences,
        );
    }
    let direct_matrix = normalize_matrix(direct);
    let mps_matrix = normalize_matrix(from_mps);
    if direct_matrix.keys().collect::<Vec<_>>() != mps_matrix.keys().collect::<Vec<_>>() {
        differences.push(format!(
            "matrix coordinates differ: direct={:?}, mps={:?}",
            direct_matrix.keys().collect::<Vec<_>>(),
            mps_matrix.keys().collect::<Vec<_>>()
        ));
    }
    for (coordinate, direct_value) in &direct_matrix {
        if let Some(mps_value) = mps_matrix.get(coordinate) {
            compare_scalar(
                &format!("matrix coefficient {coordinate:?}"),
                *direct_value,
                *mps_value,
                STRUCTURAL_ABS_TOLERANCE,
                STRUCTURAL_REL_TOLERANCE,
                &mut differences,
            );
        }
    }
    differences
}

fn ordered_columns(summary: &HighsMpsSummary) -> Vec<&roml_highs::MpsColumnSemantics> {
    summary
        .column_order
        .iter()
        .map(|name| summary.column_semantics.get(name).unwrap())
        .collect()
}

fn ordered_rows(summary: &HighsMpsSummary) -> Vec<&roml_highs::MpsRowSemantics> {
    summary
        .row_order
        .iter()
        .map(|name| summary.row_semantics.get(name).unwrap())
        .collect()
}

fn normalize_matrix(summary: &HighsMpsSummary) -> BTreeMap<(usize, usize), f64> {
    let columns: BTreeMap<_, _> = summary
        .column_order
        .iter()
        .enumerate()
        .map(|(index, name)| (name, index))
        .collect();
    let rows: BTreeMap<_, _> = summary
        .row_order
        .iter()
        .enumerate()
        .map(|(index, name)| (name, index))
        .collect();
    summary
        .matrix
        .iter()
        .map(|((row, column), value)| ((rows[row], columns[column]), *value))
        .collect()
}

fn assert_close(label: &str, left: f64, right: f64, abs: f64, rel: f64, case_name: &str) {
    let equal = if left.is_infinite() || right.is_infinite() {
        left == right
    } else {
        (left - right).abs() <= abs + rel * left.abs().max(right.abs())
    };
    assert!(equal, "{case_name}: {label}: {left} != {right}");
}

fn compare_scalar(
    label: &str,
    left: f64,
    right: f64,
    abs: f64,
    rel: f64,
    differences: &mut Vec<String>,
) {
    let equal = if left.is_infinite() || right.is_infinite() {
        left == right
    } else {
        (left - right).abs() <= abs + rel * left.abs().max(right.abs())
    };
    if !equal {
        differences.push(format!("{label}: {left} != {right}"));
    }
}
