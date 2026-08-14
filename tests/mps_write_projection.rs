//! Focused semantic projection tests for P36 Task 36-01A.

use roml::{
    binary, continuous, integer,
    io::mps::write::{MpsEntityKind, MpsNamePolicy, MpsWriteErrorKind, MpsWriteLowering},
    model::CoefficientTarget,
    model::{Bounds, ConstraintBounds, Model, Sense},
    parameter,
    snapshot::CellEntry,
    value_expr::ValueExpr,
};

// The serial Wave 1 integrator will wire this module through write/mod.rs.
// Keep this task-local inclusion independent of that shared integration file.
mod id {
    pub use roml::id::*;
}
mod snapshot {
    pub use roml::snapshot::*;
}
mod construct {
    pub use roml::construct::*;
}
mod model {
    pub use roml::model::*;
}
mod io {
    pub mod mps {
        pub mod write {
            pub use roml::io::mps::write::*;
        }
    }
}
pub use roml::{expr, function, value_expr};

#[path = "../src/io/mps/write/projection.rs"]
mod projection;

fn project(
    model: &Model,
) -> Result<projection::MpsWriteDocument, roml::io::mps::write::MpsWriteError> {
    projection::project(model, MpsNamePolicy::PreserveOrGenerate)
}

#[test]
fn projects_active_lp_and_milp_cells_in_deterministic_order() {
    let mut model = Model::new();
    let x = model
        .add_variable(continuous().named("x").bounds(-2.0, 8.0))
        .unwrap();
    let y = model
        .add_variable(integer().named("y").bounds(0.0, 3.0))
        .unwrap();
    let _row = model
        .add_constraint(
            roml::expr::ConstraintSpec::new(
                roml::expr::LinExpr::new().term(2.0, x).term(-1.0, y),
                ConstraintBounds::range(1.0, 4.0),
            )
            .named("demand"),
        )
        .unwrap();
    let objective = model.add_objective_named(Sense::Maximize, "profit");
    model.add_objective_coefficient(objective, x, 3.5).unwrap();
    model.add_objective_coefficient(objective, y, 1.0).unwrap();
    model.set_active_objective(objective).unwrap();

    let document = project(&model).unwrap();

    assert_eq!(
        document
            .variables
            .iter()
            .map(|v| v.name.as_str())
            .collect::<Vec<_>>(),
        ["x", "y"]
    );
    assert_eq!(
        document
            .rows
            .iter()
            .map(|r| r.name.as_str())
            .collect::<Vec<_>>(),
        ["demand"]
    );
    assert_eq!(document.rows[0].cells.len(), 2);
    assert_eq!(document.objective.as_ref().unwrap().cells.len(), 2);
    assert_eq!(document.report.columns, 2);
    assert_eq!(document.report.rows, 1);
    assert_eq!(document.report.nonzeros, 4);
    assert_eq!(document.report.integer_columns, 1);
    assert!(document.report.objective_present);
    assert_eq!(document.report.model_instance, model.instance());
    assert_eq!(document.report.model_revision, model.current_revision());
    assert_eq!(
        document.report.name_policy,
        MpsNamePolicy::PreserveOrGenerate
    );
    assert_eq!(document.rows[0].bounds, ConstraintBounds::range(1.0, 4.0));
    assert_eq!(document.rows[0].cells[0].variable, x);
    assert_eq!(document.rows[0].cells[0].value, 2.0);
}

#[test]
fn omits_inactive_primitives_and_records_the_omission() {
    let mut model = Model::new();
    let active = model.add_variable(continuous()).unwrap();
    let inactive = model.add_variable(continuous()).unwrap();
    model.set_variable_active(inactive, false).unwrap();
    let row = model.add_empty_constraint(ConstraintBounds::le(4.0));
    model.set_constraint_active(row, false).unwrap();

    let document = project(&model).unwrap();

    assert_eq!(document.variables.len(), 1);
    assert_eq!(document.variables[0].source_id, active);
    assert!(document.rows.is_empty());
    assert!(document.objective.is_none());
    assert_eq!(document.report.omitted_inactive_entities, 2);
}

#[test]
fn persistent_fixing_is_lowered_to_exact_equal_effective_bounds() {
    let mut model = Model::new();
    let x = model
        .add_variable(continuous().named("fixed-x").bounds(0.0, 10.0))
        .unwrap();
    model.fix(x, 3.25).unwrap();

    let document = project(&model).unwrap();
    let variable = &document.variables[0];

    assert_eq!(variable.effective_bounds, Bounds::new(3.25, 3.25));
    assert_eq!(
        document.report.lowerings,
        vec![MpsWriteLowering::PersistentFixingAsBound {
            variable: x,
            value: 3.25,
        }]
    );
}

#[test]
fn rejects_active_constructs_and_semi_domains_before_projection() {
    let mut construct_model = Model::new();
    let b = construct_model.add_variable(binary()).unwrap();
    let x = construct_model
        .add_variable(continuous().bounds(0.0, 5.0))
        .unwrap();
    construct_model
        .add_indicator(
            b,
            roml::construct::IndicatorDirection::WhenOne,
            roml::function::FunctionConstraint {
                function: roml::function::ScalarFunction::Linear(
                    roml::expr::LinExpr::new().term(1.0, x),
                ),
                set: roml::function::ScalarSet::LessEqual(ValueExpr::constant(2.0)),
            },
            None,
        )
        .expect("indicator fixture should be valid");

    let construct_error = project(&construct_model).unwrap_err();
    assert_eq!(construct_error.kind(), &MpsWriteErrorKind::Unrepresentable);
    assert_eq!(
        construct_error.context().entity_kind,
        Some(MpsEntityKind::Construct)
    );

    let mut semi_model = Model::new();
    let semi = semi_model
        .add_variable(continuous().bounds(2.0, 5.0))
        .unwrap();
    semi_model.set_semicontinuous(semi, 2.0).unwrap();

    let semi_error = project(&semi_model).unwrap_err();
    assert_eq!(semi_error.kind(), &MpsWriteErrorKind::Unrepresentable);
    assert_eq!(
        semi_error.context().entity_kind,
        Some(MpsEntityKind::Variable)
    );
    assert_eq!(semi_error.context().entity_name.as_deref(), Some("X000001"));

    // Keep the construct fixture's input variables used so the test remains
    // explicit about the primitive/semantic boundary.
    assert_ne!(b, x);
}

#[test]
fn evaluates_parameterized_cells_once_and_reports_consumed_values() {
    let mut model = Model::new();
    let p = model
        .add_parameter(parameter(4.0).named("capacity"))
        .unwrap();
    let x = model.add_variable(continuous().named("x")).unwrap();
    let row = model.add_empty_constraint(ConstraintBounds::le(20.0));
    model
        .add_constraint_coefficient(
            row,
            x,
            ValueExpr::mul(ValueExpr::param(p), ValueExpr::constant(2.0)),
        )
        .unwrap();

    let document = project(&model).unwrap();

    assert_eq!(document.rows[0].cells[0].value, 8.0);
    assert_eq!(document.report.evaluated_parameters.len(), 1);
    assert_eq!(document.report.evaluated_parameters[0].id, p);
    assert_eq!(
        document.report.evaluated_parameters[0].name.as_deref(),
        Some("capacity")
    );
    assert_eq!(document.report.evaluated_parameters[0].value, 4.0);
}

#[test]
fn projects_the_captured_cell_value_without_re_evaluating_its_expression() {
    let mut model = Model::new();
    let p = model.add_parameter(parameter(4.0)).unwrap();
    let x = model.add_variable(continuous()).unwrap();
    let row = model.add_empty_constraint(ConstraintBounds::le(20.0));
    model
        .add_constraint_coefficient(
            row,
            x,
            ValueExpr::mul(ValueExpr::param(p), ValueExpr::constant(2.0)),
        )
        .unwrap();

    let mut snapshot = model.take_snapshot().unwrap();
    snapshot.cells[0].value_expr = ValueExpr::constant(f64::NAN);

    let document =
        projection::project_snapshot(&model, &snapshot, MpsNamePolicy::PreserveOrGenerate).unwrap();

    assert_eq!(document.rows[0].cells[0].value, 8.0);
    assert_eq!(document.report.evaluated_parameters[0].id, p);
}

#[test]
fn supports_no_active_objective_and_allocates_names_without_raw_ids() {
    let mut model = Model::new();
    let first = model.add_variable(continuous().named("duplicate")).unwrap();
    let second = model.add_variable(continuous().named("duplicate")).unwrap();
    let unnamed = model.add_variable(continuous()).unwrap();
    let row = model.add_empty_constraint(ConstraintBounds::ge(0.0));
    model.add_coeff(row, first, 1.0).unwrap();
    model.add_coeff(row, second, 2.0).unwrap();

    let document = project(&model).unwrap();
    let names: Vec<_> = document
        .report
        .name_map
        .variables
        .iter()
        .map(|entry| entry.emitted_name.as_str())
        .collect();

    assert_eq!(names, ["X000001", "X000002", "X000003"]);
    assert_eq!(document.variables[0].source_id, first);
    assert_eq!(document.variables[1].source_id, second);
    assert_eq!(document.variables[2].source_id, unnamed);
    assert!(!document.report.objective_present);
}

#[test]
fn strict_preserve_rejects_missing_invalid_and_duplicate_names() {
    let mut missing = Model::new();
    missing.add_variable(continuous()).unwrap();
    let error = projection::project_model(&missing, MpsNamePolicy::StrictPreserve).unwrap_err();
    assert_eq!(error.kind(), &MpsWriteErrorKind::NameAllocation);
    assert_eq!(error.context().entity_kind, Some(MpsEntityKind::Variable));

    let mut invalid = Model::new();
    invalid
        .add_variable(continuous().named("not valid"))
        .unwrap();
    let error = projection::project_model(&invalid, MpsNamePolicy::StrictPreserve).unwrap_err();
    assert_eq!(error.kind(), &MpsWriteErrorKind::NameAllocation);
    assert_eq!(error.context().entity_name.as_deref(), Some("not valid"));

    let mut duplicate = Model::new();
    duplicate.add_variable(continuous().named("same")).unwrap();
    duplicate.add_variable(continuous().named("same")).unwrap();
    let error = projection::project_model(&duplicate, MpsNamePolicy::StrictPreserve).unwrap_err();
    assert_eq!(error.kind(), &MpsWriteErrorKind::NameAllocation);
    assert_eq!(error.context().entity_name.as_deref(), Some("same"));
}

#[test]
fn name_policy_is_recorded_for_strict_preserve_success() {
    let mut model = Model::new();
    model.add_variable(continuous().named("x")).unwrap();

    let document = projection::project_model(&model, MpsNamePolicy::StrictPreserve).unwrap();

    assert_eq!(document.report.name_policy, MpsNamePolicy::StrictPreserve);
    assert_eq!(document.variables[0].name, "x");
}

#[test]
fn marker_control_tokens_are_rejected_or_replaced_without_changing_generation() {
    for marker_token in ["'MARKER'", "'INTORG'", "'INTEND'"] {
        let mut strict = Model::new();
        strict
            .add_variable(continuous().named(marker_token))
            .unwrap();
        let error = projection::project_model(&strict, MpsNamePolicy::StrictPreserve)
            .expect_err("marker control tokens cannot be preserved as entity names");
        assert_eq!(error.kind(), &MpsWriteErrorKind::NameAllocation);
        assert_eq!(error.context().entity_kind, Some(MpsEntityKind::Variable));
        assert_eq!(error.context().entity_name.as_deref(), Some(marker_token));

        let mut generated = Model::new();
        generated
            .add_variable(continuous().named(marker_token))
            .unwrap();
        let document =
            projection::project_model(&generated, MpsNamePolicy::PreserveOrGenerate).unwrap();
        assert_eq!(document.variables[0].name, "X000001");
        assert_eq!(
            document.report.name_policy,
            MpsNamePolicy::PreserveOrGenerate
        );
    }
}

#[test]
fn duplicate_canonical_cells_are_projected_as_one_cell() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let row = model.add_empty_constraint(ConstraintBounds::le(10.0));
    model.add_constraint_coefficient(row, x, 2.0).unwrap();
    model.add_constraint_coefficient(row, x, 3.0).unwrap();

    let document = project(&model).unwrap();

    assert_eq!(document.rows[0].cells.len(), 1);
    assert_eq!(document.rows[0].cells[0].value, 5.0);
}

#[test]
fn projection_report_parameter_order_follows_semantic_cell_order() {
    let mut model = Model::new();
    let p1 = model.add_parameter(parameter(1.0).named("first")).unwrap();
    let p2 = model.add_parameter(parameter(2.0).named("second")).unwrap();
    let x = model.add_variable(continuous()).unwrap();
    let first_row = model.add_empty_constraint(ConstraintBounds::le(10.0));
    let second_row = model.add_empty_constraint(ConstraintBounds::le(10.0));
    model
        .add_constraint_coefficient(first_row, x, ValueExpr::param(p2))
        .unwrap();
    model
        .add_constraint_coefficient(second_row, x, ValueExpr::param(p1))
        .unwrap();

    let document = project(&model).unwrap();
    let parameter_order: Vec<_> = document
        .report
        .evaluated_parameters
        .iter()
        .map(|entry| (entry.id, entry.name.as_deref(), entry.value))
        .collect();
    assert_eq!(
        parameter_order,
        [(p2, Some("second"), 2.0), (p1, Some("first"), 1.0)]
    );
}

#[test]
fn preserves_valid_objective_name_when_generated_row_would_collide() {
    let mut model = Model::new();
    let row = model.add_empty_constraint(ConstraintBounds::le(10.0));
    let objective = model.add_objective_named(Sense::Minimize, "R000001");
    model.set_active_objective(objective).unwrap();

    let document = project(&model).unwrap();

    assert_eq!(document.rows[0].name, "R000002");
    assert_eq!(document.objective.as_ref().unwrap().name, "R000001");
    assert_eq!(
        document
            .report
            .name_map
            .objective
            .as_ref()
            .unwrap()
            .emitted_name,
        "R000001"
    );
    assert_eq!(document.rows[0].source_id, row);
}

#[test]
fn reports_numeric_field_for_nonfinite_captured_cell_value() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let row = model.add_empty_constraint(ConstraintBounds::le(10.0));
    model.add_constraint_coefficient(row, x, 2.0).unwrap();
    let mut snapshot = model.take_snapshot().unwrap();
    snapshot.cells[0].evaluated_value = f64::INFINITY;

    let error = projection::project_snapshot(&model, &snapshot, MpsNamePolicy::PreserveOrGenerate)
        .unwrap_err();

    assert_eq!(error.kind(), &MpsWriteErrorKind::NonFiniteValue);
    assert_eq!(
        error.context().numeric_field.as_deref(),
        Some("evaluated coefficient")
    );
}

#[test]
fn rejects_snapshot_cell_with_missing_parameter_dependency() {
    let mut model = Model::new();
    let p = model.add_parameter(parameter(4.0)).unwrap();
    let x = model.add_variable(continuous()).unwrap();
    let row = model.add_empty_constraint(ConstraintBounds::le(20.0));
    model
        .add_constraint_coefficient(row, x, ValueExpr::param(p))
        .unwrap();
    let mut snapshot = model.take_snapshot().unwrap();
    snapshot.parameters.clear();

    let error = projection::project_snapshot(&model, &snapshot, MpsNamePolicy::PreserveOrGenerate)
        .unwrap_err();

    assert_eq!(error.kind(), &MpsWriteErrorKind::ParameterEvaluation);
    assert_eq!(error.context().entity_kind, Some(MpsEntityKind::MatrixCell));
    assert_eq!(error.context().parameter_dependencies, vec![p]);
}

#[test]
fn omits_inactive_constructs_and_records_the_omission() {
    let mut model = Model::new();
    let b = model.add_variable(binary()).unwrap();
    let x = model.add_variable(continuous()).unwrap();
    model
        .add_indicator(
            b,
            roml::construct::IndicatorDirection::WhenOne,
            roml::function::FunctionConstraint {
                function: roml::function::ScalarFunction::Linear(
                    roml::expr::LinExpr::new().term(1.0, x),
                ),
                set: roml::function::ScalarSet::LessEqual(ValueExpr::constant(2.0)),
            },
            None,
        )
        .unwrap();
    let mut snapshot = model.take_snapshot().unwrap();
    snapshot.constructs[0].active = false;

    let document =
        projection::project_snapshot(&model, &snapshot, MpsNamePolicy::PreserveOrGenerate).unwrap();

    assert_eq!(document.report.omitted_inactive_entities, 1);
    assert!(document.rows.is_empty());
}

#[test]
fn rejects_snapshot_cells_that_reference_stale_entities() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let row = model.add_empty_constraint(ConstraintBounds::le(10.0));
    let mut snapshot = model.take_snapshot().unwrap();
    let stale = roml::id::VarId::new(99, roml::id::Generation::new());
    snapshot.cells.push(CellEntry {
        cell_key: (CoefficientTarget::Constraint(row), stale),
        value_expr: ValueExpr::constant(1.0),
        evaluated_value: 1.0,
        dependencies: Vec::new(),
    });

    let error = projection::project_snapshot(&model, &snapshot, MpsNamePolicy::PreserveOrGenerate)
        .unwrap_err();
    assert_eq!(error.kind(), &MpsWriteErrorKind::StaleEntity);
    assert_eq!(error.context().entity_kind, Some(MpsEntityKind::MatrixCell));
    assert_eq!(error.context().entity_name.as_deref(), Some("matrix cell"));
    assert_ne!(x, stale);
}
