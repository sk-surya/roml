//! MIR-02 remediation regressions (owner review 2026-09-11).
//!
//! Each test here targets a specific rejected behavior. They are written to
//! fail on the pre-remediation head and pass after the fix.

#![allow(deprecated)]

use roml::bulk::{ParamDepBlockWitness, ParamDepLayout, ParamSpan, StridedMap};
use roml::prelude::*;
use roml::{LinExpr, ModelError, ModelRevision, ObjId, ParamId, ValueExpr, VarId};

const DT: f64 = 0.25;

fn price_values(n: usize) -> Vec<f64> {
    (0..n).map(|i| 50.0 + i as f64 * 0.5).collect()
}

fn build_vars(model: &mut Model, n: usize) -> (Vec<VarId>, Vec<VarId>) {
    let charge = (0..n).map(|_| model.add_var()).collect();
    let discharge = (0..n).map(|_| model.add_var()).collect();
    (charge, discharge)
}

fn objective_slices(
    n: usize,
    charge: &[VarId],
    discharge: &[VarId],
    price: &[ParamId],
) -> (Vec<VarId>, Vec<ParamId>, Vec<f64>) {
    let mut vars = Vec::with_capacity(2 * n);
    let mut params = Vec::with_capacity(2 * n);
    let mut scales = Vec::with_capacity(2 * n);
    for (var, param) in charge.iter().zip(price.iter()) {
        vars.push(*var);
        params.push(*param);
        scales.push(-DT);
    }
    for (var, param) in discharge.iter().zip(price.iter()) {
        vars.push(*var);
        params.push(*param);
        scales.push(DT);
    }
    (vars, params, scales)
}

/// Packed objective built through the **ordinary** (non-eligible) path, so its
/// cells live in `param_positions`.
fn build_baseline(n: usize) -> (Model, ParamSpan, Vec<VarId>, Vec<VarId>) {
    let mut model = Model::new();
    let (charge, discharge) = build_vars(&mut model, n);
    let span = model
        .add_parameter_block(&price_values(n))
        .expect("parameter block");
    let price: Vec<ParamId> = span.ids().collect();
    let (vars, params, scales) = objective_slices(n, &charge, &discharge, &price);
    model
        .set_linear_objective_param_bulk(Sense::Maximize, &vars, &params, &scales, 0.0)
        .expect("baseline packed objective");
    (model, span, charge, discharge)
}

fn build_general(n: usize) -> (Model, Vec<ParamId>) {
    let mut model = Model::new();
    let (charge, discharge) = build_vars(&mut model, n);
    let price: Vec<ParamId> = price_values(n)
        .iter()
        .map(|v| model.add_parameter(*v).expect("parameter"))
        .collect();
    let mut expr = LinExpr::new();
    for i in 0..n {
        expr = expr.term(ValueExpr::scaled_param(-DT, price[i]), charge[i]);
        expr = expr.term(ValueExpr::scaled_param(DT, price[i]), discharge[i]);
    }
    model.maximize(expr).expect("general objective");
    (model, price)
}

/// Build an objective with a layout derived from the freshly created
/// parameter span.
fn build_with_layout(
    n: usize,
    make_layout: impl Fn(ParamSpan) -> ParamDepLayout,
) -> (Model, ParamSpan, ObjId) {
    let mut model = Model::new();
    let (charge, discharge) = build_vars(&mut model, n);
    let span = model
        .add_parameter_block(&price_values(n))
        .expect("parameter block");
    let price: Vec<ParamId> = span.ids().collect();
    let (vars, params, scales) = objective_slices(n, &charge, &discharge, &price);
    let layout = make_layout(span);
    let obj = model
        .set_linear_objective_param_bulk_with_layout(
            Sense::Maximize,
            &vars,
            &params,
            &scales,
            0.0,
            &layout,
        )
        .expect("valid layout");
    (model, span, obj)
}

fn full_layout(n: usize, span: ParamSpan) -> ParamDepLayout {
    ParamDepLayout {
        blocks: vec![
            ParamDepBlockWitness {
                params: span,
                param_map: StridedMap::contiguous(n),
                cell_offset: 0,
                cell_map: StridedMap::contiguous(n),
                scale: -DT,
                row: None,
            },
            ParamDepBlockWitness {
                params: span,
                param_map: StridedMap::contiguous(n),
                cell_offset: n as u32,
                cell_map: StridedMap::contiguous(n),
                scale: DT,
                row: None,
            },
        ],
    }
}

// ── P1: bulk update preserves the non-eligible packed fallback ────────────

#[test]
fn bulk_update_propagates_non_eligible_packed_positions() {
    let n = 16;
    let (mut packed, span, _c, _d) = build_baseline(n);
    let (mut general, gprice) = build_general(n);
    let new: Vec<f64> = (0..n).map(|i| 11.0 + i as f64).collect();

    packed
        .set_parameters_bulk(span, &new)
        .expect("queue bulk update");
    packed.commit().expect("commit bulk update");
    for (param, value) in gprice.iter().zip(new.iter()) {
        general.set_parameter(*param, *value).expect("scalar");
    }
    general.commit().expect("commit general");

    assert_eq!(
        packed.take_snapshot().expect("packed snapshot"),
        general.take_snapshot().expect("general snapshot"),
        "bulk update must reach param_positions cells"
    );
}

// ── P1: ownership/coverage validation ─────────────────────────────────────

#[test]
fn empty_layout_keeps_every_cell_on_the_position_fallback() {
    let n = 8;
    let (mut model, span, _obj) = build_with_layout(n, |_| ParamDepLayout::default());
    let lowering = model.lowering_stats();
    assert_eq!(lowering.param_dep_blocks, 0);
    assert_eq!(
        lowering.param_positions_cells as usize,
        2 * n,
        "uncovered cells retain param_positions"
    );
    model.validate_invariants().expect("invariants");

    let (mut general, gprice) = build_general(n);
    let new: Vec<f64> = (0..n).map(|i| 3.0 + i as f64).collect();
    model.set_parameters_bulk(span, &new).expect("queue");
    model.commit().expect("commit");
    for (param, value) in gprice.iter().zip(new.iter()) {
        general.set_parameter(*param, *value).expect("scalar");
    }
    general.commit().expect("commit general");
    assert_eq!(
        model.take_snapshot().expect("layout"),
        general.take_snapshot().expect("general")
    );
}

#[test]
fn partial_layout_uses_blocks_for_covered_and_positions_for_uncovered() {
    let n = 8;
    // Cover only the charge family (first n canonical cells).
    let (mut model, span, _obj) = build_with_layout(n, |span| ParamDepLayout {
        blocks: vec![ParamDepBlockWitness {
            params: span,
            param_map: StridedMap::contiguous(n),
            cell_offset: 0,
            cell_map: StridedMap::contiguous(n),
            scale: -DT,
            row: None,
        }],
    });
    let lowering = model.lowering_stats();
    assert_eq!(lowering.param_dep_blocks, 1);
    assert_eq!(model.parameter_dependency_block_count(), 1);
    assert_eq!(
        lowering.param_positions_cells as usize, n,
        "only the uncovered discharge cells retain positions"
    );
    model.validate_invariants().expect("invariants");

    let (mut general, gprice) = build_general(n);
    let new: Vec<f64> = (0..n).map(|i| 9.0 + i as f64).collect();
    model.set_parameters_bulk(span, &new).expect("queue");
    model.commit().expect("commit");
    for (param, value) in gprice.iter().zip(new.iter()) {
        general.set_parameter(*param, *value).expect("scalar");
    }
    general.commit().expect("commit general");
    assert_eq!(
        model.take_snapshot().expect("partial"),
        general.take_snapshot().expect("general"),
        "covered and uncovered families both reprice"
    );
}

#[test]
fn overlapping_witnesses_are_rejected_atomically() {
    let n = 4;
    let mut model = Model::new();
    let (charge, discharge) = build_vars(&mut model, n);
    let span = model
        .add_parameter_block(&price_values(n))
        .expect("parameter block");
    let price: Vec<ParamId> = span.ids().collect();
    let (vars, params, scales) = objective_slices(n, &charge, &discharge, &price);
    // Both witnesses claim the same first n canonical cells.
    let overlap = ParamDepLayout {
        blocks: vec![
            ParamDepBlockWitness {
                params: span,
                param_map: StridedMap::contiguous(n),
                cell_offset: 0,
                cell_map: StridedMap::contiguous(n),
                scale: -DT,
                row: None,
            },
            ParamDepBlockWitness {
                params: span,
                param_map: StridedMap::contiguous(n),
                cell_offset: 0,
                cell_map: StridedMap::contiguous(n),
                scale: -DT,
                row: None,
            },
        ],
    };
    let error = model
        .set_linear_objective_param_bulk_with_layout(
            Sense::Maximize,
            &vars,
            &params,
            &scales,
            0.0,
            &overlap,
        )
        .expect_err("double-owned canonical cell must reject");
    assert!(matches!(error, ModelError::InvalidParamDepLayout(_)));
    assert!(model.active_objective().is_none());
    assert_eq!(model.num_coefficients(), 0);
}

// ── P2: reference backend preserves symbolic cells ────────────────────────

#[test]
fn reference_replay_preserves_symbolic_patch_cells() {
    use roml::solver::reference::ReferenceBackend;
    use roml::sync::{AdapterCursor, ApplyOutcome};

    let n = 8;
    let (mut model, span, _obj) = build_with_layout(n, |span| full_layout(n, span));
    model.commit().expect("construction");
    let new: Vec<f64> = (0..n).map(|i| 6.0 + i as f64).collect();
    model.set_parameters_bulk(span, &new).expect("queue");
    model.commit().expect("reprice");

    let mut incremental = ReferenceBackend::new();
    let mut cursor = AdapterCursor::new();
    for batch in model.deltas_since(ModelRevision::ZERO).expect("deltas") {
        let outcome = incremental.apply_batch(batch, &mut cursor).expect("apply");
        assert_eq!(
            outcome,
            ApplyOutcome::Applied {
                new_revision: batch.to
            }
        );
    }
    let mut rebuilt = ReferenceBackend::new();
    let mut rebuild_cursor = AdapterCursor::new();
    rebuilt.rebuild(
        &model.take_snapshot().expect("snapshot"),
        &mut rebuild_cursor,
    );

    // Symbolic shapes must agree, not merely numeric normalized values.
    assert_eq!(
        incremental.symbolic_objective_cells(),
        rebuilt.symbolic_objective_cells(),
        "patches must preserve the parameterized expression form"
    );
    assert_eq!(incremental.normalized_view(), rebuilt.normalized_view());
}
