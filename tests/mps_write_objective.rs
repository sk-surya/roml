//! Focused tests for the isolated P36 objective/RHS/RANGES lowering unit.
//!
//! Wave 2 integration is intentionally serial.  The production module is
//! included directly here so these tests do not edit `write/mod.rs` while the
//! shared formatter/projection integration is still owned by the integrator.

use std::io::Cursor;

use roml::{
    continuous,
    io::mps::{
        MpsEntityKind, MpsReader, MpsWriteContext, MpsWriteError, MpsWriteErrorKind,
        MpsWriteReport, MpsWriter,
    },
    model::{ConstraintBounds, Model, Sense},
};

mod model {
    pub use roml::model::*;
}

#[path = "../src/io/mps/write/format.rs"]
#[allow(dead_code)]
mod format;

#[path = "../src/io/mps/write/objective.rs"]
mod objective;

use format::{MpsObjectiveSense, MpsRowKind};
use objective::{encode_objective, encode_row_bounds};

fn report() -> roml::io::mps::MpsWriteReport {
    let model = Model::new();
    roml::io::mps::MpsWriteReport {
        model_lineage: model.lineage(),
        model_instance: model.instance(),
        model_revision: model.current_revision(),
        name_policy: Default::default(),
        evaluated_parameters: Vec::new(),
        columns: 0,
        rows: 0,
        nonzeros: 0,
        integer_columns: 0,
        objective_present: true,
        rhs_vector: None,
        ranges_vector: None,
        bounds_vector: None,
        name_map: Default::default(),
        lowerings: Vec::new(),
        omitted_inactive_entities: 0,
    }
}

#[test]
fn objective_encoding_table_preserves_sense_and_inverse_offset_for_all_signs() {
    let cases = [
        (
            "min-positive",
            Some(Sense::Minimize),
            Some(7.5),
            MpsObjectiveSense::Minimize,
            Some(-7.5),
        ),
        (
            "min-zero",
            Some(Sense::Minimize),
            Some(0.0),
            MpsObjectiveSense::Minimize,
            Some(0.0),
        ),
        (
            "min-negative",
            Some(Sense::Minimize),
            Some(-3.25),
            MpsObjectiveSense::Minimize,
            Some(3.25),
        ),
        (
            "max-positive",
            Some(Sense::Maximize),
            Some(7.5),
            MpsObjectiveSense::Maximize,
            Some(-7.5),
        ),
        (
            "max-zero",
            Some(Sense::Maximize),
            Some(0.0),
            MpsObjectiveSense::Maximize,
            Some(0.0),
        ),
        (
            "max-negative",
            Some(Sense::Maximize),
            Some(-3.25),
            MpsObjectiveSense::Maximize,
            Some(3.25),
        ),
        (
            "no-objective",
            None,
            None,
            MpsObjectiveSense::Minimize,
            None,
        ),
    ];

    for (name, sense, constant, expected_sense, expected_rhs) in cases {
        let encoded = encode_objective(sense, constant, "OBJ", &report())
            .unwrap_or_else(|error| panic!("{name} must encode: {error}"));
        assert_eq!(encoded.sense, expected_sense, "{name} sense");
        assert_eq!(
            encoded.rhs.as_ref().map(|entry| entry.value),
            expected_rhs,
            "{name} RHS"
        );
        assert_eq!(
            encoded.rhs.as_ref().map(|entry| entry.row.as_str()),
            expected_rhs.map(|_| "OBJ"),
            "{name} row"
        );
    }
}

#[test]
fn row_encoding_table_emits_one_semantic_row_and_the_minimal_rim_vectors() {
    let cases = [
        (
            "equality",
            ConstraintBounds::eq(2.0),
            MpsRowKind::Equal,
            2.0,
            None,
        ),
        (
            "lower",
            ConstraintBounds::ge(-4.0),
            MpsRowKind::GreaterThan,
            -4.0,
            None,
        ),
        (
            "upper",
            ConstraintBounds::le(9.0),
            MpsRowKind::LessThan,
            9.0,
            None,
        ),
        (
            "ranged",
            ConstraintBounds::range(-2.0, 5.5),
            MpsRowKind::GreaterThan,
            -2.0,
            Some(7.5),
        ),
    ];

    for (name, bounds, expected_kind, expected_rhs, expected_range) in cases {
        let encoded = encode_row_bounds(bounds, "R1", &report())
            .unwrap_or_else(|error| panic!("{name} must encode: {error}"));
        assert_eq!(encoded.kind, expected_kind, "{name} sense");
        assert_eq!(encoded.rhs.value, expected_rhs, "{name} RHS");
        assert_eq!(encoded.rhs.row, "R1", "{name} RHS row");
        assert_eq!(
            encoded.range.as_ref().map(|entry| entry.value),
            expected_range,
            "{name} RANGES"
        );
        assert_eq!(
            encoded.range.as_ref().map(|entry| entry.row.as_str()),
            expected_range.map(|_| "R1"),
            "{name} RANGES row"
        );
    }
}

#[test]
fn p35_ranges_sign_and_sense_table_round_trips_to_one_normalized_row() {
    let cases = [
        ("E", 10.0, 3.0, ConstraintBounds::range(10.0, 13.0)),
        ("E", 10.0, -3.0, ConstraintBounds::range(7.0, 10.0)),
        ("G", 10.0, 3.0, ConstraintBounds::range(10.0, 13.0)),
        ("G", 10.0, -3.0, ConstraintBounds::range(10.0, 13.0)),
        ("L", 10.0, 3.0, ConstraintBounds::range(7.0, 10.0)),
        ("L", 10.0, -3.0, ConstraintBounds::range(7.0, 10.0)),
    ];

    for (sense, rhs, range, expected_bounds) in cases {
        let input = format!(
            "NAME RANGES-{sense}-{range}\nROWS\n N OBJ\n {sense} R1\nCOLUMNS\n X R1 1\nRHS\n RHS1 R1 {rhs}\nRANGES\n RNG1 R1 {range}\nENDATA\n"
        );
        let imported = MpsReader::new()
            .read(Cursor::new(input.into_bytes()))
            .unwrap_or_else(|error| panic!("{sense} {range} must import: {error}"));
        assert_eq!(imported.model.num_constraints(), 1);

        let mut bytes = Vec::new();
        MpsWriter::new()
            .write(&imported.model, &mut bytes)
            .unwrap_or_else(|error| panic!("{sense} {range} must export: {error}"));
        let round_trip = MpsReader::new()
            .read(Cursor::new(bytes))
            .unwrap_or_else(|error| panic!("{sense} {range} must re-import: {error}"));
        let snapshot = round_trip.model.take_snapshot().expect("snapshot is valid");
        assert_eq!(snapshot.constraints.len(), 1);
        assert_eq!(
            snapshot.constraints[0].bounds, expected_bounds,
            "{sense} {range}"
        );
    }
}

#[test]
fn nonfinite_objective_and_row_values_are_rejected_before_serialization() {
    let objective_error = encode_objective(Some(Sense::Minimize), Some(f64::NAN), "OBJ", &report())
        .expect_err("NaN objective offsets must be rejected");
    assert_eq!(objective_error.kind(), &MpsWriteErrorKind::NonFiniteValue);
    assert_eq!(
        objective_error.context().numeric_field.as_deref(),
        Some("objective offset")
    );

    let row_error = encode_row_bounds(ConstraintBounds::range(f64::NAN, 2.0), "R1", &report())
        .expect_err("NaN row bounds must be rejected");
    assert_eq!(row_error.kind(), &MpsWriteErrorKind::NonFiniteValue);
    assert_eq!(
        row_error.context().numeric_field.as_deref(),
        Some("row lower bound")
    );
}

#[test]
fn active_empty_objective_round_trips_with_zero_offset_and_preserved_sense() {
    let mut model = Model::with_name("empty-objective");
    model.add_variable(continuous().named("x")).unwrap();
    let objective = model.add_objective(Sense::Maximize);
    model.set_active_objective(objective).unwrap();

    let mut bytes = Vec::new();
    let report = MpsWriter::new().write(&model, &mut bytes).unwrap();
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(report.objective_present);
    assert!(text.contains("OBJSENSE MAX\n"));
    assert!(text.contains("N OBJ\n"));
    assert!(text.contains("RHS1 OBJ 0\n"));

    let imported = MpsReader::new().read(Cursor::new(bytes)).unwrap();
    let imported_objective = imported.model.active_objective().unwrap();
    assert_eq!(
        imported.model.objective_sense(imported_objective),
        Some(Sense::Maximize)
    );
    assert_eq!(
        imported.model.objective_constant(imported_objective),
        Some(0.0)
    );
}
