//! MIR-06 M6-2A: shared `GeneralLinArray` fallback.

use roml::modeling::{GeneralAffine, GeneralLinArray, GeneralTerm};
use roml::{Model, ValueExpr};

#[test]
fn linarray_expands_to_general_cells() {
    let mut model = Model::new();
    let x = model.var("x", [2, 3]).bounds(0.0, 1.0).build().unwrap();
    let prices: Vec<f64> = (1..=6).map(|k| k as f64).collect();
    let price = model.param("price", [2, 3], &prices).unwrap();

    let lin = price.try_mul(&x.expr().unwrap()).unwrap().unwrap();
    let general = lin.to_general().unwrap();
    assert_eq!(general.shape(), &[2, 3]);
    assert_eq!(general.owner(), model.instance());
    assert_eq!(general.len(), 6);

    let cell = general.cell(0).unwrap();
    assert_eq!(cell.terms.len(), 1);
    assert_eq!(cell.terms[0].var, x.get(0).unwrap());
    assert_eq!(
        cell.terms[0].coeff,
        ValueExpr::scaled_param(1.0, price.get(0).unwrap())
    );
    assert_eq!(cell.constant, ValueExpr::constant(0.0));
}

#[test]
fn to_general_commutes_with_add_and_scale() {
    use std::collections::BTreeMap;

    // Canonicalize to evaluated (variable -> coefficient, constant), so the
    // check is insensitive to `ValueExpr` tree shape / simplification.
    fn canon(array: &GeneralLinArray) -> Vec<(BTreeMap<u32, f64>, f64)> {
        array
            .cells()
            .iter()
            .map(|cell| {
                let mut coeffs: BTreeMap<u32, f64> = BTreeMap::new();
                for term in &cell.terms {
                    *coeffs.entry(term.var.index()).or_insert(0.0) += term.coeff.eval(|_| 0.0);
                }
                (coeffs, cell.constant.eval(|_| 0.0))
            })
            .collect()
    }

    let mut model = Model::new();
    let x = model.var("x", 4).bounds(0.0, 1.0).build().unwrap();
    let a = x.expr().unwrap();
    let b = x.expr().unwrap().scaled(2.0);

    let lhs = a.clone().try_add(b.clone()).unwrap().to_general().unwrap();
    let rhs = a
        .to_general()
        .unwrap()
        .try_add(b.to_general().unwrap())
        .unwrap();
    assert_eq!(canon(&lhs), canon(&rhs));

    let scaled = a.clone().scaled(3.0).to_general().unwrap();
    assert_eq!(canon(&scaled), canon(&a.to_general().unwrap().scaled(3.0)));
}

#[test]
fn general_array_holds_uncovered_forms_and_validates_shape() {
    let mut model = Model::new();
    let x = model.var("x", 2).bounds(0.0, 1.0).build().unwrap();
    let p = model.param("p", 2, &[1.0, 2.0]).unwrap();
    let q = model.param("q", 2, &[3.0, 4.0]).unwrap();

    // A parameter x parameter coefficient is not representable as a compact
    // `LinArray` family; the general fallback holds it.
    let cells: Vec<GeneralAffine> = (0..2)
        .map(|i| {
            GeneralAffine::new(
                vec![GeneralTerm {
                    var: x.get(i).unwrap(),
                    coeff: ValueExpr::mul(
                        ValueExpr::param(p.get(i).unwrap()),
                        ValueExpr::param(q.get(i).unwrap()),
                    ),
                }],
                ValueExpr::constant(0.0),
            )
        })
        .collect();
    let general = model.general_lin_array([2], cells.clone()).unwrap();
    assert_eq!(general.len(), 2);
    assert_eq!(general.cell(1).unwrap().terms.len(), 1);

    // Shape product must match the cell count.
    assert!(model.general_lin_array([3], cells).is_err());
}

#[test]
fn general_rows_commit_and_reject_stale_or_foreign() {
    let mut model = Model::new();
    let x = model.var("x", 2).bounds(0.0, 1.0).build().unwrap();
    let cells = (0..2)
        .map(|i| {
            GeneralAffine::new(
                vec![GeneralTerm {
                    var: x.get(i).unwrap(),
                    coeff: ValueExpr::constant(2.0),
                }],
                ValueExpr::constant(1.0),
            )
        })
        .collect();
    let array = model.general_lin_array([2], cells).unwrap();
    let cons = model
        .add_general_rows(&array, &[(0.0, 10.0); 2])
        .expect("general rows");
    assert_eq!(cons.len(), 2);
    assert_eq!(model.num_constraints(), 2);

    // A stale variable rejects atomically at the sink.
    let before = model.num_constraints();
    model.remove_variable(x.get(0).unwrap()).unwrap();
    let result = model.add_general_rows(&array, &[(0.0, 1.0); 2]);
    assert!(matches!(result, Err(roml::ModelError::VariableNotFound(_))));
    assert_eq!(model.num_constraints(), before, "atomic rejection");

    // A foreign owner rejects with a typed cross-model error.
    let mut other = Model::new();
    let y = other.var("y", 1).bounds(0.0, 1.0).build().unwrap();
    let foreign = other
        .general_lin_array(
            [1],
            vec![GeneralAffine::new(
                vec![GeneralTerm {
                    var: y.get(0).unwrap(),
                    coeff: ValueExpr::constant(1.0),
                }],
                ValueExpr::constant(0.0),
            )],
        )
        .unwrap();
    assert!(matches!(
        model.add_general_rows(&foreign, &[(0.0, 1.0)]),
        Err(roml::ModelError::View(
            roml::modeling::ViewError::CrossModel { .. }
        ))
    ));
}

#[test]
fn general_rows_reject_overflowing_coefficient_atomically() {
    let mut model = Model::new();
    let x = model.var("x", 1).bounds(0.0, 1.0).build().unwrap();
    let p = model.add_parameter_array_block([1], &[1e150]).unwrap();
    let pid = p.get(0).unwrap();
    let coeff = ValueExpr::mul(ValueExpr::param(pid), ValueExpr::param(pid));
    let array = model
        .general_lin_array(
            [1],
            vec![GeneralAffine::new(
                vec![GeneralTerm {
                    var: x.get(0).unwrap(),
                    coeff,
                }],
                ValueExpr::constant(0.0),
            )],
        )
        .unwrap();

    // Both parameter values are finite, but p*p overflows.
    model.set_parameter(pid, 1e308).unwrap();
    model.commit().unwrap();
    let revision = model.current_revision();
    let before = model.num_constraints();

    let result = model.add_general_rows(&array, &[(0.0, 1.0)]);
    assert!(matches!(result, Err(roml::ModelError::NonFiniteValue(_))));
    assert_eq!(model.num_constraints(), before, "no row allocated");
    assert_eq!(
        model.current_revision(),
        revision,
        "no journal/revision change"
    );
}
