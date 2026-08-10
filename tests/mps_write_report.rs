//! Focused accounting tests for the P36 MPS write report.

use roml::{
    continuous,
    io::mps::MpsWriter,
    model::{ConstraintBounds, Sense},
    Model,
};

#[test]
fn nonzeros_counts_normalized_columns_entries_including_zero_cells() {
    let mut model = Model::with_name("report-accounting");
    let _synthetic = model
        .add_variable(continuous().named("synthetic"))
        .expect("synthetic column variable is valid");
    let explicit_row_zero = model
        .add_variable(continuous().named("explicit-row-zero"))
        .expect("row-zero variable is valid");
    let explicit_objective_zero = model
        .add_variable(continuous().named("explicit-objective-zero"))
        .expect("objective-zero variable is valid");

    let row = model.add_empty_constraint(ConstraintBounds::le(1.0));
    model
        .add_coeff(row, explicit_row_zero, 0.0)
        .expect("explicit row zero is a valid canonical cell");

    let objective = model.add_objective_named(Sense::Minimize, "objective");
    model
        .add_objective_coeff(objective, explicit_objective_zero, 0.0)
        .expect("explicit objective zero is a valid canonical cell");
    model
        .set_active_objective(objective)
        .expect("objective is active");

    let mut bytes = Vec::new();
    let report = MpsWriter::new()
        .write(&model, &mut bytes)
        .expect("zero-valued mathematical entries are representable");

    assert_eq!(report.columns, 3);
    assert_eq!(report.rows, 1);
    assert_eq!(report.nonzeros, 3);

    let output = String::from_utf8(bytes).expect("MPS output is UTF-8");
    assert!(output.contains("synthetic objective 0"));
    assert!(output.contains("explicit-row-zero R000001 0"));
    assert!(output.contains("explicit-objective-zero objective 0"));
}
