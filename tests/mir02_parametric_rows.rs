//! MIR-02: parametric packed row insertion (IR-08).

#![allow(deprecated)]

use roml::delta::ModelOp;
use roml::prelude::*;
use roml::{ConstraintBounds, ModelError, ParamId, ValueExpr, VarId};

fn add_vars(model: &mut Model, n: usize) -> Vec<VarId> {
    (0..n).map(|_| model.add_var()).collect()
}

#[test]
fn param_rows_match_general_scalar_rows() {
    let n = 4;
    let values = [2.0, 3.0, 4.0, 5.0];
    let scales = [1.0, -0.5, 2.0, 0.25];

    let mut packed = Model::new();
    let pvars = add_vars(&mut packed, n);
    let span = packed.add_parameter_block(&values).expect("param block");
    let params: Vec<ParamId> = span.ids().collect();
    let bounds = [ConstraintBounds::le(100.0)];
    let cons = packed
        .add_linear_rows_param_bulk(&[0, n as u32], &pvars, &params, &scales, &bounds)
        .expect("packed param rows");
    assert_eq!(cons.len(), 1);

    let mut general = Model::new();
    let gvars = add_vars(&mut general, n);
    let gparams: Vec<ParamId> = values
        .iter()
        .map(|v| general.add_parameter(*v).expect("param"))
        .collect();
    let gcon = general
        .add_constraint(ConstraintBounds::le(100.0))
        .expect("row");
    for i in 0..n {
        general
            .add_constraint_coefficient(
                gcon,
                gvars[i],
                ValueExpr::scaled_param(scales[i], gparams[i]),
            )
            .expect("cell");
    }

    assert_eq!(
        packed.take_snapshot().expect("packed"),
        general.take_snapshot().expect("general")
    );
}

#[test]
fn param_rows_merge_same_param_duplicates() {
    let mut model = Model::new();
    let v = model.add_var();
    let span = model.add_parameter_block(&[2.0]).expect("param block");
    let p = span.ids().next().expect("member");

    let cons = model
        .add_linear_rows_param_bulk(
            &[0, 2],
            &[v, v],
            &[p, p],
            &[1.0, 2.0],
            &[ConstraintBounds::le(10.0)],
        )
        .expect("duplicate scales merge");
    assert_eq!(cons.len(), 1);
    assert_eq!(model.num_coefficients(), 1, "one merged canonical cell");
}

#[test]
fn param_rows_distinct_params_not_packable_and_atomic() {
    let mut model = Model::new();
    let v = model.add_var();
    let p1 = model.add_parameter(1.0).expect("p1");
    let p2 = model.add_parameter(2.0).expect("p2");

    let error = model
        .add_linear_rows_param_bulk(
            &[0, 2],
            &[v, v],
            &[p1, p2],
            &[1.0, 1.0],
            &[ConstraintBounds::le(10.0)],
        )
        .expect_err("distinct params in one canonical row cell are not packable");
    assert!(matches!(error, ModelError::NotPackable(_)));
    assert_eq!(model.num_constraints(), 0, "no partial row");
    assert_eq!(model.num_coefficients(), 0, "no partial cell");
    assert_eq!(model.journal_len(), 0, "no revision recorded");
}

#[test]
fn param_rows_journal_one_packed_op() {
    let n = 8;
    let mut model = Model::new();
    let vars = add_vars(&mut model, n);
    let span = model
        .add_parameter_block(&(0..n).map(|i| i as f64).collect::<Vec<_>>())
        .expect("param block");
    let params: Vec<ParamId> = span.ids().collect();
    let scales = vec![1.0; n];
    model.commit().expect("flush variable creation");
    let base = model.current_revision();
    model
        .add_linear_rows_param_bulk(
            &[0, n as u32],
            &vars,
            &params,
            &scales,
            &[ConstraintBounds::le(5.0)],
        )
        .expect("packed rows");
    model.commit().expect("commit");

    let batches = model.deltas_since(base).expect("deltas");
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].operations.len(), 1, "one packed row op");
    assert!(matches!(
        batches[0].operations[0],
        ModelOp::AddParametricRows { .. }
    ));
    assert_eq!(
        batches[0].functions.len(),
        1,
        "semantic function view covers the parametric row"
    );
}
