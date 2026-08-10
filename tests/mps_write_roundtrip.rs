#[path = "support/mps_write_oracle.rs"]
mod mps_write_oracle;

use std::{
    io::Cursor,
    panic::{catch_unwind, AssertUnwindSafe},
};

use roml::{
    binary, continuous, integer,
    io::mps::{MpsReader, MpsWriter},
    model::{ConstraintBounds, Sense},
    parameter, ConstraintExprExt, ConstraintSpec, LinExpr, Model, ObjectiveSpec, ValueExpr,
};

#[test]
fn named_lp_round_trips_to_the_same_normalized_mathematics() {
    let mut model = Model::with_name("hand-lp");
    let x = model
        .add_variable(continuous().bounds(-2.0, 7.0).named("x"))
        .expect("valid variable");
    let row = model
        .add_constraint((x).between(1.0, 5.0).named("capacity"))
        .expect("valid row");
    model.add_coeff(row, x, 2.5).expect("valid matrix cell");
    let (objective, _) = model
        .add_objective_spec(ObjectiveSpec::new(Sense::Maximize, 3.25 * x + 4.5).named("profit"))
        .expect("valid objective");
    model
        .set_active_objective(objective)
        .expect("objective exists");

    let before = mps_write_oracle::normalize(&model);
    let mut bytes = Vec::new();
    MpsWriter::new()
        .write(&model, &mut bytes)
        .expect("hand LP is representable");
    let imported = MpsReader::new()
        .read(Cursor::new(bytes))
        .expect("writer output is readable");
    let after = mps_write_oracle::normalize(&imported.model);

    mps_write_oracle::assert_equivalent(&before, &after);
}

#[test]
fn generated_names_do_not_change_unnamed_duplicate_or_invalid_math() {
    let mut model = Model::with_name("invalid model name");
    let unnamed = model
        .add_variable(continuous().bounds(0.0, 10.0))
        .expect("valid unnamed variable");
    let duplicate_a = model
        .add_variable(continuous().named("duplicate"))
        .expect("valid first duplicate variable");
    let duplicate_b = model
        .add_variable(integer().bounds(0.0, 4.0).named("duplicate"))
        .expect("valid second duplicate variable");
    let invalid = model
        .add_variable(binary().named("invalid variable name"))
        .expect("valid invalid-name variable");

    let unnamed_row = model.add_empty_constraint(ConstraintBounds::le(20.0));
    let duplicate_row_a = model
        .add_constraint(
            ConstraintSpec::new(LinExpr::new(), ConstraintBounds::ge(-3.0))
                .named("duplicate"),
        )
        .expect("valid first duplicate row");
    let duplicate_row_b = model
        .add_constraint(
            ConstraintSpec::new(LinExpr::new(), ConstraintBounds::eq(5.0)).named("duplicate"),
        )
        .expect("valid second duplicate row");
    let invalid_row = model
        .add_constraint(
            ConstraintSpec::new(LinExpr::new(), ConstraintBounds::range(-2.0, 8.0))
                .named("invalid row name"),
        )
        .expect("valid invalid-name row");

    model
        .add_coeff(unnamed_row, unnamed, 1.0)
        .expect("unnamed matrix cell");
    model
        .add_coeff(duplicate_row_a, duplicate_a, 2.0)
        .expect("first duplicate matrix cell");
    model
        .add_coeff(duplicate_row_b, duplicate_b, 3.0)
        .expect("second duplicate matrix cell");
    model
        .add_coeff(invalid_row, invalid, 4.0)
        .expect("invalid-name matrix cell");
    let objective = model.add_objective_named(Sense::Maximize, "invalid objective name");
    model
        .add_objective_coeff(objective, unnamed, 1.5)
        .expect("objective cell");
    model
        .set_active_objective(objective)
        .expect("objective exists");

    assert_round_trip(&model, "generated-names");
}

#[test]
fn no_objective_and_free_variable_round_trip() {
    let mut model = Model::with_name("hand-no-objective");
    let x = model
        .add_variable(
            continuous()
                .bounds(f64::NEG_INFINITY, f64::INFINITY)
                .named("free_x"),
        )
        .expect("valid free variable");
    let row = model
        .add_constraint(
            ConstraintSpec::new(LinExpr::new(), ConstraintBounds::ge(-3.0)).named("floor"),
        )
        .expect("valid row");
    model.add_coeff(row, x, -1.0).expect("valid matrix cell");

    assert_round_trip(&model, "hand-no-objective");
}

#[test]
fn mixed_integer_domains_and_fixed_bounds_round_trip() {
    let mut model = Model::with_name("hand-mixed-domains");
    let integer = model
        .add_variable(integer().bounds(-2.0, 5.0).named("integer_x"))
        .expect("valid integer variable");
    let binary = model
        .add_variable(binary().named("binary_y"))
        .expect("valid binary variable");
    let fixed = model
        .add_variable(continuous().bounds(3.0, 3.0).named("fixed_z"))
        .expect("valid fixed variable");
    let row = model
        .add_constraint(
            ConstraintSpec::new(LinExpr::new(), ConstraintBounds::le(10.0)).named("limit"),
        )
        .expect("valid row");
    model.add_coeff(row, integer, 2.0).expect("integer cell");
    model.add_coeff(row, binary, 3.0).expect("binary cell");
    model.add_coeff(row, fixed, -1.0).expect("fixed cell");
    let objective = model.add_objective_named(Sense::Minimize, "cost");
    model
        .add_objective_coeff(objective, integer, -4.0)
        .expect("objective cell");
    model
        .add_objective_coeff(objective, binary, 1.5)
        .expect("objective cell");
    model
        .set_active_objective(objective)
        .expect("objective exists");

    assert_round_trip(&model, "hand-mixed-domains");
}

#[test]
fn parameterized_cells_compare_against_the_evaluated_pre_write_snapshot() {
    let mut model = Model::with_name("hand-parameterized");
    let scale = model
        .add_parameter(parameter(2.0).named("scale"))
        .expect("valid parameter");
    let x = model
        .add_variable(continuous().bounds(0.0, 20.0).named("x"))
        .expect("valid variable");
    let row = model
        .add_constraint(
            ConstraintSpec::new(LinExpr::new(), ConstraintBounds::le(15.0)).named("capacity"),
        )
        .expect("valid row");
    model
        .add_constraint_coefficient(
            row,
            x,
            ValueExpr::mul(ValueExpr::param(scale), ValueExpr::constant(1.5)),
        )
        .expect("parameterized row cell");
    let objective = model.add_objective_named(Sense::Maximize, "profit");
    model
        .add_objective_coefficient(objective, x, ValueExpr::param(scale))
        .expect("parameterized objective cell");
    model
        .set_active_objective(objective)
        .expect("objective exists");
    model
        .set_parameter(scale, 3.5)
        .expect("pending parameter value");
    model.commit().expect("parameter update commits");

    let evaluated_snapshot = model.take_snapshot().expect("evaluated snapshot");
    let before = mps_write_oracle::normalize_snapshot(&model, &evaluated_snapshot);
    let mut bytes = Vec::new();
    MpsWriter::new()
        .write(&model, &mut bytes)
        .expect("parameterized model is representable");
    let imported = MpsReader::new()
        .read(Cursor::new(bytes))
        .expect("writer output is readable");
    let after = mps_write_oracle::normalize(&imported.model);

    mps_write_oracle::assert_equivalent(&before, &after);
}

#[test]
fn fixed_seed_randomized_primitive_lp_milp_round_trips() {
    const RANDOM_CASE_COUNT: usize = 256;
    let mut rng = FixedSeed::new(0x3602_C0DE_5EED);
    for case in 0..RANDOM_CASE_COUNT {
        let model = randomized_primitive_model(case, &mut rng);
        assert_round_trip(&model, &format!("random-{case:03}"));
    }
}

fn assert_round_trip(model: &Model, case: &str) {
    let snapshot = model.take_snapshot().expect("legal model snapshot");
    let before = mps_write_oracle::normalize_snapshot(model, &snapshot);
    let mut bytes = Vec::new();
    MpsWriter::new()
        .write(model, &mut bytes)
        .unwrap_or_else(|error| panic!("{case}: writer rejected legal primitive model: {error}"));
    let imported = MpsReader::new()
        .read(Cursor::new(bytes))
        .unwrap_or_else(|error| panic!("{case}: reader rejected writer output: {error}"));
    let after = mps_write_oracle::normalize(&imported.model);

    let comparison = catch_unwind(AssertUnwindSafe(|| {
        mps_write_oracle::assert_equivalent(&before, &after);
    }));
    if let Err(payload) = comparison {
        panic!("{case}: normalized mathematics differ: {payload:?}");
    }
}

#[derive(Debug)]
struct FixedSeed {
    state: u64,
}

impl FixedSeed {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }

    fn index(&mut self, upper: usize) -> usize {
        (self.next() as usize) % upper
    }

    fn bool(&mut self) -> bool {
        self.next() & 1 == 0
    }

    fn coefficient(&mut self) -> f64 {
        const VALUES: [f64; 11] = [-7.0, -4.0, -2.5, -1.0, -0.5, 0.5, 1.0, 2.0, 3.5, 5.0, 8.0];
        VALUES[self.index(VALUES.len())]
    }

    fn bound_value(&mut self) -> f64 {
        (self.index(13) as f64) - 6.0
    }
}

fn randomized_primitive_model(case: usize, rng: &mut FixedSeed) -> Model {
    let mut model = Model::with_name(format!("CASE{case:03}"));
    let is_milp = case % 2 == 0;
    let parameterized = case % 11 == 0;
    // Deliberately stay inside the frozen primitive representability matrix:
    // no semi-domains, active semantic constructs, non-finite numerics, or
    // fractional integer bounds are generated. Those are rejection tests,
    // not legal round-trip cases, and belong to the writer contract suites.
    let parameter_id = if parameterized {
        Some(
            model
                .add_parameter(parameter((rng.index(6) as f64) + 1.0).named(format!("P{case:03}")))
                .expect("random parameter is valid"),
        )
    } else {
        None
    };

    let variable_count = 1 + rng.index(6);
    let mut variables = Vec::with_capacity(variable_count);
    for variable_index in 0..variable_count {
        let name = format!("V{case:03}_{variable_index:02}");
        let variable = if is_milp && (variable_index == 0 || rng.bool()) {
            if rng.bool() {
                model
                    .add_variable(binary().named(name))
                    .expect("random binary variable is valid")
            } else {
                let lower = (rng.index(5) as f64) - 3.0;
                let upper = lower + 1.0 + rng.index(6) as f64;
                model
                    .add_variable(integer().bounds(lower, upper).named(name))
                    .expect("random integer variable is valid")
            }
        } else {
            let definition = match rng.index(6) {
                0 => continuous(),
                1 => continuous().bounds(-4.0, 6.0),
                2 => continuous().bounds(-3.0, f64::INFINITY),
                3 => continuous().bounds(0.0, 5.0),
                4 => continuous().bounds(f64::NEG_INFINITY, 8.0),
                _ => continuous().bounds(f64::NEG_INFINITY, f64::INFINITY),
            };
            model
                .add_variable(definition.named(name))
                .expect("random continuous variable is valid")
        };
        variables.push(variable);
    }

    let row_count = 1 + rng.index(5);
    let mut rows = Vec::with_capacity(row_count);
    for row_index in 0..row_count {
        let bounds = match rng.index(4) {
            0 => ConstraintBounds::le(rng.bound_value() + 7.0),
            1 => ConstraintBounds::ge(rng.bound_value() - 7.0),
            2 => ConstraintBounds::eq(rng.bound_value()),
            _ => {
                let lower = rng.bound_value() - 4.0;
                ConstraintBounds::range(lower, lower + 1.0 + rng.index(7) as f64)
            }
        };
        let row = model
            .add_constraint(
                ConstraintSpec::new(LinExpr::new(), bounds)
                    .named(format!("R{case:03}_{row_index:02}")),
            )
            .expect("random row is valid");
        rows.push(row);
    }

    for (row_index, row) in rows.iter().copied().enumerate() {
        let mut inserted = false;
        for (variable_index, variable) in variables.iter().copied().enumerate() {
            if variable_index == row_index % variables.len() || rng.bool() {
                let expression = if parameterized && row_index == 0 && variable_index == 0 {
                    ValueExpr::mul(
                        ValueExpr::param(parameter_id.expect("parameterized case")),
                        ValueExpr::constant(1.25),
                    )
                } else {
                    ValueExpr::constant(rng.coefficient())
                };
                model
                    .add_constraint_coefficient(row, variable, expression)
                    .expect("random row cell is valid");
                inserted = true;
            }
        }
        if !inserted {
            model
                .add_coeff(row, variables[0], rng.coefficient())
                .expect("fallback row cell is valid");
        }
    }

    if rng.index(5) != 0 {
        let sense = if rng.bool() {
            Sense::Minimize
        } else {
            Sense::Maximize
        };
        let objective = model.add_objective_named(sense, format!("O{case:03}"));
        for (variable_index, variable) in variables.iter().copied().enumerate() {
            if variable_index == 0 || rng.bool() {
                let expression = if parameterized && variable_index == 0 {
                    ValueExpr::param(parameter_id.expect("parameterized case"))
                } else {
                    ValueExpr::constant(rng.coefficient())
                };
                model
                    .add_objective_coefficient(objective, variable, expression)
                    .expect("random objective cell is valid");
            }
        }
        model
            .set_active_objective(objective)
            .expect("random objective exists");
    }

    if let Some(parameter) = parameter_id {
        model
            .set_parameter(parameter, (rng.index(8) as f64) + 0.5)
            .expect("random parameter update is valid");
        model.commit().expect("random parameter update commits");
    }

    model
}
