//! End-to-end MPS writer edge cases that exercise the library pipeline's
//! ranged-row, mixed-domain, extreme-magnitude and objective-less branches.

use std::io::Cursor;

use roml::io::mps::{MpsReader, MpsWriter};
use roml::model::{ConstraintBounds, Sense};
use roml::{binary, continuous, integer, Model};

#[test]
fn ranged_rows_mixed_domains_and_extreme_values_round_trip() {
    let mut model = Model::with_name("edge");
    let cont = model
        .add_variable(continuous().bounds(-1e30, 1e30).named("wide"))
        .expect("wide continuous");
    let intg = model
        .add_variable(integer().bounds(2.0, 8.0).named("i"))
        .expect("integer");
    let bin = model.add_variable(binary().named("b")).expect("binary");
    let unb = model
        .add_variable(continuous().named("u"))
        .expect("non-negative");

    // A ranged row (both bounds finite, not an equality) plus extreme
    // magnitudes on both ends of the formatter's range.
    let row = model.add_empty_constraint(ConstraintBounds::range(1.0, 3.0));
    model.add_coeff(row, cont, 1e25).expect("large coeff");
    model.add_coeff(row, intg, 1e-9).expect("small coeff");
    model.add_coeff(row, bin, 1.0).expect("binary coeff");
    model.add_coeff(row, unb, 1.0).expect("coeff");

    let objective = model.add_objective_named(Sense::Minimize, "cost");
    model
        .add_objective_coeff(objective, cont, 1.0)
        .expect("objective coeff");
    model
        .set_active_objective(objective)
        .expect("active objective");

    let mut bytes = Vec::new();
    let report = MpsWriter::new()
        .write(&model, &mut bytes)
        .expect("edge model is representable");
    assert_eq!(report.rows, 1);
    assert_eq!(report.columns, 4);

    let text = String::from_utf8(bytes.clone()).expect("UTF-8 MPS");
    assert!(text.contains("RANGES"), "ranged row emits a RANGES section");
    assert!(
        text.contains("e+") || text.contains("e-"),
        "scientific value"
    );
    assert!(text.contains("MARKER"), "integer/binary markers emitted");

    // Round-trips through the reader.
    let imported = MpsReader::new()
        .read(Cursor::new(bytes))
        .expect("reader accepts the emitted MPS");
    assert_eq!(imported.model.num_variables(), 4);
    assert_eq!(imported.model.num_constraints(), 1);
}

#[test]
fn objective_less_model_still_writes_and_round_trips() {
    let mut model = Model::with_name("noobj");
    let x = model
        .add_variable(continuous().bounds(0.0, 4.0).named("x"))
        .expect("x");
    let row = model.add_empty_constraint(ConstraintBounds::ge(-5.0));
    model.add_coeff(row, x, 1.0).expect("coeff");

    let mut bytes = Vec::new();
    MpsWriter::new()
        .write(&model, &mut bytes)
        .expect("objective-less model is representable");
    let imported = MpsReader::new()
        .read(Cursor::new(bytes))
        .expect("reader accepts objective-less MPS");
    assert_eq!(imported.model.num_variables(), 1);
    assert_eq!(imported.model.num_constraints(), 1);
}

#[test]
fn free_variable_bounds_are_emitted() {
    let mut model = Model::with_name("free");
    let x = model
        .add_variable(
            continuous()
                .bounds(f64::NEG_INFINITY, f64::INFINITY)
                .named("fv"),
        )
        .expect("free variable");
    let row = model.add_empty_constraint(ConstraintBounds::le(1.0));
    model.add_coeff(row, x, 1.0).expect("coeff");

    let mut bytes = Vec::new();
    MpsWriter::new()
        .write(&model, &mut bytes)
        .expect("free variable is representable");
    let text = String::from_utf8(bytes).expect("UTF-8 MPS");
    assert!(text.contains("FR"), "free bound record emitted");
}
