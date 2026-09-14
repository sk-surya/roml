//! MIR-05 (IR-26): closure rules accumulate into CSR and bulk-commit once.

use roml::modeling::ViewError;
use roml::{Model, ModelSnapshot};

fn transport(use_rules: bool) -> ModelSnapshot {
    let (m, n) = (4usize, 3usize);
    let supply = [1.0, 2.0, 3.0, 4.0];
    let mut model = Model::new();
    let x = model.var("x", [m, n]).bounds(0.0, 1.0).build().unwrap();
    if use_rules {
        model
            .add_rules(|rules| {
                for (i, &s) in supply.iter().enumerate() {
                    rules.add_eq(x.row(i)?, s)?;
                }
                Ok(())
            })
            .unwrap();
    } else {
        model
            .add_rows(x.expr().unwrap().rows_eq(&supply).unwrap())
            .unwrap();
    }
    model.take_snapshot().unwrap()
}

#[test]
fn closure_rules_match_direct_rows() {
    assert_eq!(transport(true), transport(false));
}

#[test]
fn rule_batch_bulk_commits_once_for_many_rows() {
    let (m, n) = (4usize, 3usize);
    let mut model = Model::new();
    let x = model.var("x", [m, n]).bounds(0.0, 1.0).build().unwrap();
    let cons = model
        .add_rules(|rules| {
            for i in 0..m {
                rules.add_ge(x.row(i)?, 0.0)?;
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(cons.len(), m);
    assert_eq!(model.num_constraints(), m);
    let lowering = model.lowering_stats();
    assert_eq!(lowering.rule_rows_accumulated, m as u64);
    assert_eq!(lowering.rule_bulk_commits, 1);
    assert_eq!(lowering.numeric_bulk, 1, "one packed row block");
    assert_eq!(lowering.general_affine, 0);
}

#[test]
fn parametric_rule_rows_use_packed_families() {
    let (m, n) = (3usize, 4usize);
    let mut model = Model::new();
    let x = model.var("x", [m, n]).bounds(0.0, 1.0).build().unwrap();
    let price = model.param("price", [m, n], &[2.0; 12]).unwrap();
    let cons = model
        .add_rules(|rules| {
            for i in 0..m {
                let row = x.row(i)?;
                let coeffs = price
                    .row(i)?
                    .try_mul(&row.expr()?)?
                    .ok_or(ViewError::Unsupported("not representable"))?;
                rules.add_eq(coeffs, 1.0)?;
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(cons.len(), m);
    let lowering = model.lowering_stats();
    assert!(lowering.param_dep_blocks >= 1, "packed families");
    assert_eq!(lowering.general_affine, 0);
    assert_eq!(lowering.rule_bulk_commits, 1);
}

#[test]
fn indexed_rules_drive_iteration_and_match_direct_rows() {
    let (m, n) = (4usize, 3usize);
    let supply = [1.0, 2.0, 3.0, 4.0];
    let mut model = Model::new();
    let x = model.var("x", [m, n]).bounds(0.0, 1.0).build().unwrap();
    let cons = model
        .add_indexed_rules(0..m, |rules, i| {
            rules.add_eq(x.row(i)?, supply[i])?;
            Ok(())
        })
        .unwrap();
    assert_eq!(cons.len(), m);
    assert_eq!(model.lowering_stats().rule_bulk_commits, 1);
    assert_eq!(model.lowering_stats().rule_rows_accumulated, m as u64);

    let mut direct = Model::new();
    let y = direct.var("x", [m, n]).bounds(0.0, 1.0).build().unwrap();
    direct
        .add_rows(y.expr().unwrap().rows_eq(&supply).unwrap())
        .unwrap();
    assert_eq!(
        model.take_snapshot().unwrap(),
        direct.take_snapshot().unwrap()
    );
}

#[test]
fn rule_batch_preserves_detailed_view_errors() {
    use roml::modeling::ViewError;
    use roml::ModelError;

    // A foreign array preserves the exact `CrossModel` error.
    let mut a = Model::new();
    let mut b = Model::new();
    let bx = b.var("x", 3).bounds(0.0, 1.0).build().unwrap();
    let error = a
        .add_rules(|rules| {
            rules.add_eq(bx.clone(), 1.0)?;
            Ok(())
        })
        .unwrap_err();
    assert!(matches!(
        error,
        ModelError::View(ViewError::CrossModel { .. })
    ));

    // An out-of-range `row(i)` preserves the exact `SliceOutOfRange` error and
    // rejects atomically.
    let ax = a.var("x", [2, 3]).bounds(0.0, 1.0).build().unwrap();
    let before = a.num_constraints();
    let error = a
        .add_indexed_rules(0..3, |rules, i| {
            rules.add_eq(ax.row(i)?, 0.0)?;
            Ok(())
        })
        .unwrap_err();
    assert!(matches!(
        error,
        ModelError::View(ViewError::SliceOutOfRange { axis: 0, .. })
    ));
    assert_eq!(a.num_constraints(), before);
}

#[test]
fn failing_rule_closure_leaves_no_residue() {
    let mut model = Model::new();
    let x = model.var("x", [2, 3]).bounds(0.0, 1.0).build().unwrap();
    let before = model.num_constraints();
    let result = model.add_rules(|rules| {
        rules.add_eq(x.row(0)?, 1.0)?;
        rules.add_eq(x.row(1)?, 2.0)?;
        Err(ViewError::Unsupported("stop"))
    });
    assert!(result.is_err());
    assert_eq!(model.num_constraints(), before);
    assert_eq!(model.lowering_stats().rule_bulk_commits, 0);
    assert_eq!(model.lowering_stats().rule_rows_accumulated, 0);
}

#[test]
fn foreign_rule_row_is_rejected_atomically() {
    let mut a = Model::new();
    let ax = a.var("x", 3).bounds(0.0, 1.0).build().unwrap();
    let mut b = Model::new();
    let bx = b.var("x", 3).bounds(0.0, 1.0).build().unwrap();

    let before = a.num_constraints();
    let result = a.add_rules(|rules| {
        rules.add_eq(bx.clone(), 1.0)?;
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(a.num_constraints(), before);
    assert_eq!(a.lowering_stats().rule_bulk_commits, 0);

    // The model is still usable with its own arrays.
    a.add_rules(|rules| {
        rules.add_eq(ax.clone(), 1.0)?;
        Ok(())
    })
    .unwrap();
    assert_eq!(a.num_constraints(), 1);
}
