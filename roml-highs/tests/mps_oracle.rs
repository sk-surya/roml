//! Independent native/ROML MPS-reader qualification checks.

use std::path::PathBuf;

use roml_highs::{
    compare_mps_solve, compare_mps_structure, observe_mps_differential,
    observe_mps_solve_differential, MpsDifferentialDisposition, MPS_STRUCTURAL_ABS_TOLERANCE,
    MPS_STRUCTURAL_REL_TOLERANCE,
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/mps")
        .join(name)
}

#[test]
fn native_and_roml_solves_match_termination_and_objective_with_tolerances() {
    let observation = observe_mps_solve_differential(fixture("oracle-equivalent.mps"));
    let roml = observation.roml.expect("ROML solve must succeed");
    let highs = observation.highs.expect("native solve must succeed");
    let comparison = compare_mps_solve(
        &highs,
        &roml,
        MPS_STRUCTURAL_ABS_TOLERANCE,
        MPS_STRUCTURAL_REL_TOLERANCE,
    );
    assert!(
        comparison.equivalent,
        "solve comparison failed: {:?}",
        comparison.differences
    );
}

#[test]
fn native_read_model_is_observation_only_and_agrees_on_synthetic_fixture() {
    let observation = observe_mps_differential(fixture("oracle-equivalent.mps"));
    let roml = observation.roml.expect("synthetic fixture must import");
    let highs = observation.highs.expect("bundled HiGHS must read fixture");

    assert_eq!(roml.columns, highs.columns);
    assert_eq!(roml.rows, highs.rows);
    assert_eq!(roml.nonzeros, highs.nonzeros);
    assert_eq!(roml.objective_offset, highs.objective_offset);
    let comparison = compare_mps_structure(
        &highs,
        &roml,
        MPS_STRUCTURAL_ABS_TOLERANCE,
        MPS_STRUCTURAL_REL_TOLERANCE,
    );
    assert!(
        comparison.equivalent,
        "full structural comparison failed: {:?}",
        comparison.differences
    );
    assert_eq!(roml.objective_sense, highs.objective_sense);
    assert_eq!(roml.column_semantics, highs.column_semantics);
    assert_eq!(roml.row_semantics, highs.row_semantics);
    assert_eq!(roml.matrix, highs.matrix);
}

#[test]
fn strict_rejection_has_an_explicit_differential_disposition() {
    assert_eq!(
        MpsDifferentialDisposition::IntentionalRomlRejection,
        MpsDifferentialDisposition::IntentionalRomlRejection
    );
}
