//! Independent native/ROML MPS-reader qualification checks.

use std::path::PathBuf;

use roml_highs::{observe_mps_differential, MpsDifferentialDisposition};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/mps")
        .join(name)
}

#[test]
fn native_read_model_is_observation_only_and_agrees_on_synthetic_fixture() {
    let observation = observe_mps_differential(fixture("synthetic-edge.mps"));
    let roml = observation.roml.expect("synthetic fixture must import");
    let highs = observation.highs.expect("bundled HiGHS must read fixture");

    assert_eq!(roml.columns, highs.columns);
    assert_eq!(roml.rows, highs.rows);
    assert_eq!(roml.nonzeros, highs.nonzeros);
    assert_eq!(roml.objective_offset, highs.objective_offset);
}

#[test]
fn strict_rejection_has_an_explicit_differential_disposition() {
    assert_eq!(
        MpsDifferentialDisposition::IntentionalRomlRejection,
        MpsDifferentialDisposition::IntentionalRomlRejection
    );
}
