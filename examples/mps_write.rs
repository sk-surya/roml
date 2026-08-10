//! Write a small solver-free LP/MILP model as deterministic free MPS.

use roml::{continuous, integer, io::mps::MpsWriter, model::ConstraintBounds, Model};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::with_name("mps-example");
    let x = model.add_variable(continuous().named("production"))?;
    let y = model.add_variable(integer().bounds(0.0, 10.0).named("batches"))?;
    let capacity = model.add_empty_constraint(ConstraintBounds::le(40.0));
    model.add_coeff(capacity, x, 2.0)?;
    model.add_coeff(capacity, y, 5.0)?;
    model.maximize(3.0 * x + 8.0 * y)?;

    let mut bytes = Vec::new();
    let report = MpsWriter::new().write(&model, &mut bytes)?;
    println!("wrote {} columns and {} rows", report.columns, report.rows);
    print!("{}", String::from_utf8(bytes)?);
    Ok(())
}
