//! Public integration coverage for the serial P36 writer pipeline.

use std::{
    error::Error,
    fs,
    io::{self, Cursor, Write},
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use roml::{
    continuous, integer,
    io::mps::{MpsDestinationPolicy, MpsReader, MpsWriteErrorKind, MpsWriteOptions, MpsWriter},
    model::{ConstraintBounds, Sense},
    parameter, ConstraintExprExt, Model, ValueExpr,
};

#[test]
fn writes_a_simple_named_lp_to_a_public_stream() {
    let mut model = Model::with_name("simple");
    let x = model
        .add_variable(continuous().named("x"))
        .expect("variable definition is valid");
    let row = model.add_empty_constraint(ConstraintBounds::le(4.0));
    model
        .add_coeff(row, x, 1.0)
        .expect("constraint coefficient is valid");

    let objective = model.add_objective_named(Sense::Minimize, "cost");
    model
        .add_objective_coeff(objective, x, 2.0)
        .expect("objective coefficient is valid");
    model
        .set_active_objective(objective)
        .expect("objective is present");

    let mut bytes = Vec::new();
    let report = MpsWriter::new()
        .write(&model, &mut bytes)
        .expect("a primitive LP is representable as free MPS");

    assert_eq!(report.columns, 1);
    assert_eq!(report.rows, 1);
    assert_eq!(report.nonzeros, 2);
    assert_eq!(report.model_instance, model.instance());
    assert!(String::from_utf8(bytes)
        .expect("writer emits UTF-8 MPS bytes")
        .contains("NAME simple\n"));
}

fn simple_model() -> Model {
    let mut model = Model::with_name("simple");
    let x = model
        .add_variable(continuous().named("x"))
        .expect("variable definition is valid");
    let row = model.add_empty_constraint(ConstraintBounds::le(4.0));
    model
        .add_coeff(row, x, 1.0)
        .expect("constraint coefficient is valid");
    let objective = model.add_objective_named(Sense::Minimize, "cost");
    model
        .add_objective_coeff(objective, x, 2.0)
        .expect("objective coefficient is valid");
    model
        .set_active_objective(objective)
        .expect("objective is present");
    model
}

#[test]
fn parameterized_coefficients_use_the_captured_evaluated_snapshot() {
    let mut model = Model::with_name("parameterized");
    let rate = model
        .add_parameter(parameter(3.0).named("rate"))
        .expect("parameter definition is valid");
    let x = model.add_variable(continuous().named("x")).unwrap();
    let row = model.add_empty_constraint(ConstraintBounds::le(12.0));
    model
        .add_constraint_coefficient(
            row,
            x,
            ValueExpr::mul(ValueExpr::param(rate), ValueExpr::constant(2.0)),
        )
        .unwrap();

    let mut bytes = Vec::new();
    let report = MpsWriter::new().write(&model, &mut bytes).unwrap();
    let text = String::from_utf8(bytes).unwrap();

    assert!(text.contains("R000001 6\n"));
    assert_eq!(report.evaluated_parameters.len(), 1);
    assert_eq!(report.evaluated_parameters[0].id, rate);
    assert_eq!(report.evaluated_parameters[0].value, 3.0);
    assert_eq!(report.model_revision, model.current_revision());
}

#[test]
fn repeated_writes_are_byte_identical_and_reports_retain_identity() {
    let model = simple_model();
    let writer = MpsWriter::new();
    let mut first = Vec::new();
    let mut second = Vec::new();

    let first_report = writer.write(&model, &mut first).unwrap();
    let second_report = writer.write(&model, &mut second).unwrap();

    assert_eq!(first, second);
    assert_eq!(first_report, second_report);
    assert!(first.windows(2).all(|pair| pair != b"\r\n"));
}

#[derive(Debug)]
struct PartialWriter {
    bytes: Vec<u8>,
    remaining: usize,
}

impl Write for PartialWriter {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "injected stream failure",
            ));
        }
        let count = input.len().min(self.remaining);
        self.bytes.extend_from_slice(&input[..count]);
        self.remaining -= count;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn stream_failures_preserve_partial_bytes_and_the_io_source() {
    let model = simple_model();
    let mut output = PartialWriter {
        bytes: Vec::new(),
        remaining: 9,
    };

    let error = MpsWriter::new()
        .write(&model, &mut output)
        .expect_err("the injected stream must fail after a partial write");

    assert_eq!(error.kind(), &MpsWriteErrorKind::Serialization);
    assert_eq!(error.io_kind(), Some(io::ErrorKind::BrokenPipe));
    assert!(error.source().is_some());
    assert_eq!(error.context().model_instance, Some(model.instance()));
    assert!(!output.bytes.is_empty());
}

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

fn test_directory() -> PathBuf {
    loop {
        let path = std::env::temp_dir().join(format!(
            "roml-mps-write-integration-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        if fs::create_dir(&path).is_ok() {
            return path;
        }
    }
}

#[test]
fn write_path_atomic_replace_publishes_new_bytes_over_old_destination() {
    let directory = test_directory();
    let destination = directory.join("model.mps");
    fs::write(&destination, b"old bytes").unwrap();

    MpsWriter::new()
        .write_path(&simple_model(), &destination)
        .unwrap();

    let bytes = fs::read(&destination).unwrap();
    assert!(bytes.starts_with(b"NAME simple\n"));
    assert_ne!(bytes, b"old bytes");
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn write_path_create_new_preserves_an_existing_destination() {
    let directory = test_directory();
    let destination = directory.join("model.mps");
    fs::write(&destination, b"old bytes").unwrap();
    let writer = MpsWriter::with_options(MpsWriteOptions {
        name_policy: Default::default(),
        destination_policy: MpsDestinationPolicy::CreateNew,
    });

    let error = writer
        .write_path(&simple_model(), &destination)
        .expect_err("CreateNew must reject an existing destination");

    assert_eq!(error.kind(), &MpsWriteErrorKind::DestinationExists);
    assert_eq!(fs::read(&destination).unwrap(), b"old bytes");
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn objective_sense_and_offset_round_trip_through_p35() {
    let mut model = Model::with_name("objective");
    let x = model.add_variable(continuous().named("x")).unwrap();
    let row = model.add_empty_constraint(ConstraintBounds::le(10.0));
    model.add_coeff(row, x, 1.0).unwrap();
    model.maximize(2.0 * x + 3.5).unwrap();

    let mut bytes = Vec::new();
    MpsWriter::new().write(&model, &mut bytes).unwrap();
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(text.contains("OBJSENSE MAX\n"));
    assert!(text.contains("RHS1 OBJ -3.5"));

    let imported = MpsReader::new()
        .read(Cursor::new(bytes))
        .expect("writer output is accepted by P35");
    let objective = imported.model.active_objective().unwrap();
    assert_eq!(
        imported.model.objective_sense(objective),
        Some(Sense::Maximize)
    );
    assert_eq!(imported.model.objective_constant(objective), Some(3.5));
}

#[test]
fn ranged_rows_use_one_rhs_and_one_range_record() {
    let mut model = Model::with_name("ranged");
    let x = model.add_variable(continuous().named("x")).unwrap();
    model.add_constraint((x).between(1.0, 4.0)).unwrap();

    let mut bytes = Vec::new();
    let report = MpsWriter::new().write(&model, &mut bytes).unwrap();
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert_eq!(report.ranges_vector.as_deref(), Some("RNG1"));
    assert!(text.contains("RHS1 R000001 1\n"));
    assert!(text.contains("RNG1 R000001 3\n"));

    let imported = MpsReader::new()
        .read(Cursor::new(bytes))
        .expect("ranged writer output is accepted by P35");
    let snapshot = imported.model.take_snapshot().unwrap();
    assert_eq!(snapshot.constraints.len(), 1);
    assert_eq!(
        snapshot.constraints[0].bounds,
        ConstraintBounds::range(1.0, 4.0)
    );
}

#[test]
fn integer_and_binary_domains_emit_markers_and_bounds() {
    let mut model = Model::with_name("domains");
    let integer = model
        .add_variable(integer().named("integer").bounds(-2.0, 5.0))
        .unwrap();
    let binary = model.add_variable(roml::binary().named("binary")).unwrap();
    let row = model.add_empty_constraint(ConstraintBounds::le(10.0));
    model.add_coeff(row, integer, 1.0).unwrap();
    model.add_coeff(row, binary, 2.0).unwrap();

    let mut bytes = Vec::new();
    let report = MpsWriter::new().write(&model, &mut bytes).unwrap();
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert_eq!(report.integer_columns, 2);
    assert!(text.contains("'INTORG'"));
    assert!(text.contains("'INTEND'"));
    assert!(text.contains("BV BND1 binary"));
    assert!(text.contains("LI BND1 integer -2"));
    assert!(text.contains("UI BND1 integer 5"));

    let imported = MpsReader::new()
        .read(Cursor::new(bytes))
        .expect("domain writer output is accepted by P35");
    let imported_snapshot = imported.model.take_snapshot().unwrap();
    assert_eq!(imported_snapshot.variables.len(), 2);
    assert_eq!(
        imported_snapshot
            .variables
            .iter()
            .filter(|variable| variable.var_type == roml::VarType::Integer)
            .count(),
        1
    );
    assert_eq!(
        imported_snapshot
            .variables
            .iter()
            .filter(|variable| variable.var_type == roml::VarType::Binary)
            .count(),
        1
    );
}

#[test]
fn free_integer_domains_preserve_both_infinite_sides() {
    let mut model = Model::with_name("free-integer");
    let integer = model
        .add_variable(
            integer()
                .named("integer")
                .bounds(f64::NEG_INFINITY, f64::INFINITY),
        )
        .unwrap();
    let row = model.add_empty_constraint(ConstraintBounds::le(1.0));
    model.add_coeff(row, integer, 1.0).unwrap();

    let mut bytes = Vec::new();
    MpsWriter::new().write(&model, &mut bytes).unwrap();
    let imported = MpsReader::new()
        .read(Cursor::new(bytes))
        .expect("free integer output is accepted by P35");
    let snapshot = imported.model.take_snapshot().unwrap();
    assert_eq!(snapshot.variables.len(), 1);
    assert_eq!(snapshot.variables[0].var_type, roml::VarType::Integer);
    assert_eq!(snapshot.variables[0].bounds, roml::Bounds::UNBOUNDED);
}

#[test]
fn no_active_objective_has_a_deterministic_zero_objective_encoding() {
    let mut model = Model::with_name("feasibility");
    let x = model.add_variable(continuous().named("x")).unwrap();
    let row = model.add_empty_constraint(ConstraintBounds::le(4.0));
    model.add_coeff(row, x, 1.0).unwrap();

    let mut bytes = Vec::new();
    let report = MpsWriter::new().write(&model, &mut bytes).unwrap();
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(!report.objective_present);
    assert_eq!(
        report.name_map.objective.as_ref().unwrap().emitted_name,
        "OBJ"
    );
    assert!(text.contains("N OBJ\n"));

    let imported = MpsReader::new()
        .read(Cursor::new(bytes))
        .expect("zero-objective writer output is accepted by P35");
    assert_eq!(imported.model.num_variables(), 1);
    assert_eq!(imported.model.active_objective_constant(), Some(0.0));
    assert_eq!(imported.metadata.objective_row.as_deref(), Some("OBJ"));
}

#[test]
fn synthetic_zero_objective_name_avoids_a_preserved_row_name() {
    let mut model = Model::with_name("zero-objective-name");
    let x = model.add_variable(continuous().named("x")).unwrap();
    let row = model
        .add_constraint((x).le(4.0).named("OBJ"))
        .expect("named row is valid");
    model.add_coeff(row, x, 1.0).unwrap();

    let mut bytes = Vec::new();
    MpsWriter::new().write(&model, &mut bytes).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("N OBJ000001\n"));
    assert!(text.contains("L OBJ\n"));
}
