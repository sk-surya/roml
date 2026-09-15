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
    let general = GeneralLinArray::new(model.instance(), [2], cells.clone()).unwrap();
    assert_eq!(general.len(), 2);
    assert_eq!(general.cell(1).unwrap().terms.len(), 1);

    // Shape product must match the cell count.
    assert!(GeneralLinArray::new(model.instance(), [3], cells).is_err());
}
