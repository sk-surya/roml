//! MIR-02: packed parametric objectives, validated L2 dependency layouts,
//! and block-native transactional repricing.
//!
//! The fixture is a direct/core BESS-shaped objective: one price parameter
//! drives a charge and a discharge objective cell. The dependency witness is
//! hand-constructed and must be validated by core against the post-canonical
//! packed cells.

#![allow(deprecated)]

use roml::bulk::{ParamDepBlockWitness, ParamDepLayout, ParamSpan, StridedMap};
use roml::delta::ModelOp;
use roml::model::CoefficientTarget;
use roml::prelude::*;
use roml::{LinExpr, ModelError, ModelRevision, ObjId, ParamId, ValueExpr, VarId};

const DT: f64 = 0.25;

fn price_values(n: usize) -> Vec<f64> {
    (0..n).map(|i| 50.0 + i as f64 * 0.5).collect()
}

/// Build the BESS-shaped packed parametric objective with an eligible layout.
///
/// Canonical order is by variable: charge cells (scale `-DT`) then discharge
/// cells (scale `+DT`), each reading the matching price parameter.
fn build_block(n: usize) -> (Model, ParamSpan, ObjId, Vec<VarId>, Vec<VarId>) {
    let mut model = Model::new();
    let charge: Vec<VarId> = (0..n).map(|_| model.add_var()).collect();
    let discharge: Vec<VarId> = (0..n).map(|_| model.add_var()).collect();
    let span = model
        .add_parameter_block(&price_values(n))
        .expect("parameter block");
    let price: Vec<ParamId> = span.ids().collect();

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

    let layout = ParamDepLayout {
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
    };

    let obj = model
        .set_linear_objective_param_bulk_with_layout(
            Sense::Maximize,
            &vars,
            &params,
            &scales,
            0.0,
            &layout,
        )
        .expect("eligible layout");
    (model, span, obj, charge, discharge)
}

/// General symbolic oracle: one `scale * param` `ValueExpr` per cell.
fn build_general(n: usize, values: &[f64]) -> (Model, Vec<VarId>, Vec<VarId>, Vec<ParamId>) {
    let mut model = Model::new();
    let charge: Vec<VarId> = (0..n).map(|_| model.add_var()).collect();
    let discharge: Vec<VarId> = (0..n).map(|_| model.add_var()).collect();
    let price: Vec<ParamId> = values
        .iter()
        .map(|v| model.add_parameter(*v).expect("parameter"))
        .collect();
    let mut expr = LinExpr::new();
    for i in 0..n {
        expr = expr.term(ValueExpr::scaled_param(-DT, price[i]), charge[i]);
        expr = expr.term(ValueExpr::scaled_param(DT, price[i]), discharge[i]);
    }
    model.maximize(expr).expect("general objective");
    (model, charge, discharge, price)
}

#[test]
fn block_dependencies_are_complete_for_every_param() {
    let n = 32;
    let (model, span, _obj, _c, _d) = build_block(n);
    for param in span.ids() {
        assert_eq!(
            model.parameter_dependent_count(param),
            2,
            "charge + discharge cells are both reported for a block parameter"
        );
    }
}

#[test]
fn block_model_passes_invariant_audit() {
    let (model, span, _obj, _c, _d) = build_block(16);
    model.validate_invariants().expect("block invariants");

    // The per-cell baseline path still passes the same audit.
    let mut baseline = Model::new();
    let charge: Vec<VarId> = (0..4).map(|_| baseline.add_var()).collect();
    let discharge: Vec<VarId> = (0..4).map(|_| baseline.add_var()).collect();
    let span4 = baseline
        .add_parameter_block(&price_values(4))
        .expect("params");
    let price: Vec<ParamId> = span4.ids().collect();
    let mut vars = Vec::new();
    let mut params = Vec::new();
    let mut scales = Vec::new();
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
    baseline
        .set_linear_objective_param_bulk(Sense::Maximize, &vars, &params, &scales, 0.0)
        .expect("baseline objective");
    baseline.validate_invariants().expect("baseline invariants");
    assert_eq!(span.len(), 16);
}

// ── IR-09 / IR-11: storage without per-cell positions ─────────────────────

#[test]
fn layout_stores_blocks_without_param_positions() {
    let (model, _span, _obj, _c, _d) = build_block(16);
    let lowering = model.lowering_stats();
    assert_eq!(lowering.parametric_bulk, 1);
    assert_eq!(lowering.general_affine, 0);
    assert_eq!(lowering.param_dep_blocks, 2, "charge + discharge families");
    assert_eq!(
        model.parameter_dependency_block_count(),
        2,
        "dependency-block diagnostic matches"
    );
    assert_eq!(
        lowering.param_positions_cells, 0,
        "eligible families do not populate per-cell reverse positions"
    );
}

// ── IR-08 / IR-13 / IR-14: block reprice equivalence and packed delta ─────

#[test]
fn bulk_reprice_matches_general_path() {
    let n = 16;
    let (mut block, span, _obj, _c, _d) = build_block(n);
    let (mut general, _gc, _gd, gprice) = build_general(n, &price_values(n));

    let new: Vec<f64> = (0..n).map(|i| 10.0 + i as f64).collect();
    block.set_parameters_bulk(span, &new).expect("queue block");
    block.commit().expect("commit block");
    for (param, value) in gprice.iter().zip(new.iter()) {
        general.set_parameter(*param, *value).expect("scalar");
    }
    general.commit().expect("commit general");

    assert_eq!(
        block.take_snapshot().expect("block snapshot"),
        general.take_snapshot().expect("general snapshot")
    );
}

#[test]
fn bulk_reprice_emits_one_parameter_change_and_one_patch_batch() {
    let n = 1024;
    let (mut model, span, _obj, _c, _d) = build_block(n);
    model.commit().expect("flush construction");
    let base = model.current_revision();

    let new: Vec<f64> = (0..n).map(|i| 1.0 + i as f64).collect();
    model.reset_diagnostics();
    model.set_parameters_bulk(span, &new).expect("queue");
    model.commit().expect("commit");

    let propagation = model.propagation_stats();
    assert_eq!(propagation.param_position_lookups, 0);
    assert_eq!(propagation.overlay_lookups, 0);
    assert_eq!(propagation.value_expr_evals, 0);
    assert_eq!(propagation.coefficient_patch_batches, 1);

    let batches = model.deltas_since(base).expect("retained reprice delta");
    assert_eq!(batches.len(), 1);
    assert_eq!(
        batches[0].operations.len(),
        2,
        "one packed parameter-value change + one packed coefficient-patch batch"
    );
    assert!(matches!(
        batches[0].operations[0],
        ModelOp::SetParametersBulk { .. }
    ));
    assert!(matches!(
        batches[0].operations[1],
        ModelOp::SetCoefficientPatchBatch { .. }
    ));
}

// ── IR-08: canonicalization and not-packable ──────────────────────────────

#[test]
fn distinct_params_one_cell_is_typed_not_packable() {
    let mut model = Model::new();
    let v = model.add_var();
    let p1 = model.add_parameter(1.0).expect("p1");
    let p2 = model.add_parameter(2.0).expect("p2");

    let error = model
        .set_linear_objective_param_bulk_with_layout(
            Sense::Minimize,
            &[v, v],
            &[p1, p2],
            &[1.0, 1.0],
            0.0,
            &ParamDepLayout::default(),
        )
        .expect_err("distinct params in one canonical cell are not packable");
    assert!(matches!(error, ModelError::NotPackable(_)));
    assert!(model.active_objective().is_none(), "no objective created");
    assert_eq!(model.num_coefficients(), 0, "no partial mutation");
}

#[test]
fn duplicate_same_param_scales_merge_into_one_cell() {
    let mut model = Model::new();
    let v = model.add_var();
    let span = model.add_parameter_block(&[2.0]).expect("param block");
    let p = span.ids().next().expect("member");

    let layout = ParamDepLayout {
        blocks: vec![ParamDepBlockWitness {
            params: span,
            param_map: StridedMap::contiguous(1),
            cell_offset: 0,
            cell_map: StridedMap::contiguous(1),
            scale: 3.0,
            row: None,
        }],
    };
    model
        .set_linear_objective_param_bulk_with_layout(
            Sense::Minimize,
            &[v, v],
            &[p, p],
            &[1.0, 2.0],
            0.0,
            &layout,
        )
        .expect("merged scale 3 packs");
    assert_eq!(model.num_coefficients(), 1);
    assert_eq!(model.lowering_stats().param_dep_blocks, 1);
}

// ── IR-10: forged layout atomic rejection ─────────────────────────────────

#[test]
fn forged_layout_is_rejected_atomically() {
    let n = 4;
    // Wrong scale: witness claims -2*DT for the charge family.
    let mut model = Model::new();
    let charge: Vec<VarId> = (0..n).map(|_| model.add_var()).collect();
    let discharge: Vec<VarId> = (0..n).map(|_| model.add_var()).collect();
    let span = model
        .add_parameter_block(&price_values(n))
        .expect("param block");
    let price: Vec<ParamId> = span.ids().collect();
    let mut vars = Vec::new();
    let mut params = Vec::new();
    let mut scales = Vec::new();
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
    let forged = ParamDepLayout {
        blocks: vec![ParamDepBlockWitness {
            params: span,
            param_map: StridedMap::contiguous(n),
            cell_offset: 0,
            cell_map: StridedMap::contiguous(n),
            scale: -2.0 * DT,
            row: None,
        }],
    };
    let error = model
        .set_linear_objective_param_bulk_with_layout(
            Sense::Maximize,
            &vars,
            &params,
            &scales,
            0.0,
            &forged,
        )
        .expect_err("forged scale rejects");
    assert!(matches!(error, ModelError::InvalidParamDepLayout(_)));
    assert!(model.active_objective().is_none());
    assert_eq!(model.num_coefficients(), 0);

    // Out-of-range cell offset also rejects atomically.
    let out_of_range = ParamDepLayout {
        blocks: vec![ParamDepBlockWitness {
            params: span,
            param_map: StridedMap::contiguous(n),
            cell_offset: n as u32 + 1,
            cell_map: StridedMap::contiguous(n),
            scale: -DT,
            row: None,
        }],
    };
    assert!(model
        .set_linear_objective_param_bulk_with_layout(
            Sense::Maximize,
            &vars,
            &params,
            &scales,
            0.0,
            &out_of_range,
        )
        .is_err());
    assert_eq!(model.num_coefficients(), 0);
}

// ── IR-12: transaction queue / rollback / commit ──────────────────────────

#[test]
fn bulk_update_queues_rolls_back_and_commits() {
    let (mut model, span, _obj, _c, _d) = build_block(8);
    let first = span.ids().next().expect("member");
    let original = model.parameter_value(first).expect("value");
    let new: Vec<f64> = (0..8).map(|i| 5.0 + i as f64).collect();

    model.set_parameters_bulk(span, &new).expect("queue");
    assert!(model.has_uncommitted());
    model.rollback();
    assert!(!model.has_uncommitted());
    assert_eq!(model.parameter_value(first), Some(original));

    model.set_parameters_bulk(span, &new).expect("queue");
    model.commit().expect("commit");
    assert_eq!(model.parameter_value(first), Some(new[0]));
}

#[test]
fn bulk_update_rejects_non_finite_atomically() {
    let (mut model, span, _obj, _c, _d) = build_block(4);
    let mut bad = vec![1.0; 4];
    bad[2] = f64::NAN;
    assert!(matches!(
        model.set_parameters_bulk(span, &bad),
        Err(ModelError::NonFiniteValue(_))
    ));
    assert!(!model.has_uncommitted());
}

// ── IR-15: scalar update on a block-created parameter ─────────────────────

#[test]
fn scalar_update_on_block_param_matches_general_path() {
    let n = 8;
    let (mut block, span, _obj, _c, _d) = build_block(n);
    let (mut general, _gc, _gd, gprice) = build_general(n, &price_values(n));
    let target = 123.0;

    block
        .set_parameter(span.ids().nth(3).expect("member"), target)
        .expect("scalar on block param");
    block.commit().expect("commit block");
    general.set_parameter(gprice[3], target).expect("scalar");
    general.commit().expect("commit general");

    assert_eq!(
        block.take_snapshot().expect("block"),
        general.take_snapshot().expect("general")
    );
}

// ── IR-16: shadowed eligible cell ─────────────────────────────────────────

#[test]
fn shadowed_cell_is_skipped_and_stays_correct() {
    let n = 8;
    let (mut block, span, objective, charge, _d) = build_block(n);
    let (mut general, gcharge, _gd, gprice) = build_general(n, &price_values(n));

    // Replace one charge cell with a constant on both models.
    block
        .set_coefficient(CoefficientTarget::Objective(objective), charge[0], 999.0)
        .expect("shadow block cell");
    let general_obj = general.active_objective().expect("objective");
    general
        .set_coefficient(CoefficientTarget::Objective(general_obj), gcharge[0], 999.0)
        .expect("shadow general cell");

    let new: Vec<f64> = (0..n).map(|i| 7.0 + i as f64).collect();
    block.set_parameters_bulk(span, &new).expect("queue");
    block.commit().expect("commit block");
    for (param, value) in gprice.iter().zip(new.iter()) {
        general.set_parameter(*param, *value).expect("scalar");
    }
    general.commit().expect("commit general");

    assert_eq!(
        block.take_snapshot().expect("block"),
        general.take_snapshot().expect("general")
    );
}

// ── IR-17: fresh cells append after prior revisions ───────────────────────

#[test]
fn fresh_block_appends_after_prior_revision() {
    let (mut model, span, _obj, _c, _d) = build_block(8);
    model.commit().expect("first revision");
    let first_blocks = model.lowering_stats().param_dep_blocks;
    assert_eq!(first_blocks, 2);

    // A second eligible objective appends fresh p-base cells and families.
    let charge: Vec<VarId> = (0..8).map(|_| model.add_var()).collect();
    let discharge: Vec<VarId> = (0..8).map(|_| model.add_var()).collect();
    let price: Vec<ParamId> = span.ids().collect();
    let mut vars = Vec::new();
    let mut params = Vec::new();
    let mut scales = Vec::new();
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
    let layout = ParamDepLayout {
        blocks: vec![
            ParamDepBlockWitness {
                params: span,
                param_map: StridedMap::contiguous(8),
                cell_offset: 0,
                cell_map: StridedMap::contiguous(8),
                scale: -DT,
                row: None,
            },
            ParamDepBlockWitness {
                params: span,
                param_map: StridedMap::contiguous(8),
                cell_offset: 8,
                cell_map: StridedMap::contiguous(8),
                scale: DT,
                row: None,
            },
        ],
    };
    model
        .set_linear_objective_param_bulk_with_layout(
            Sense::Maximize,
            &vars,
            &params,
            &scales,
            0.0,
            &layout,
        )
        .expect("second eligible objective");
    assert_eq!(model.lowering_stats().param_dep_blocks, 4);

    // Repricing the shared parameter span updates both resolutions.
    let new: Vec<f64> = (0..8).map(|i| 3.0 + i as f64).collect();
    model.set_parameters_bulk(span, &new).expect("queue");
    model.commit().expect("commit");
    assert_eq!(model.parameter_value(price[0]), Some(new[0]));
}

// ── IR-14: retained delta replays on the reference backend ────────────────

#[test]
fn packed_delta_replays_without_the_live_model() {
    use roml::solver::reference::ReferenceBackend;
    use roml::sync::{AdapterCursor, ApplyOutcome};

    let (mut model, span, _obj, _c, _d) = build_block(16);
    model.commit().expect("construction");
    let base = model.current_revision();

    let new: Vec<f64> = (0..16).map(|i| 4.0 + i as f64).collect();
    model.set_parameters_bulk(span, &new).expect("queue");
    model.commit().expect("commit reprice");

    let mut incremental = ReferenceBackend::new();
    let mut cursor = AdapterCursor::new();
    for batch in model.deltas_since(ModelRevision::ZERO).expect("deltas") {
        let outcome = incremental
            .apply_batch(batch, &mut cursor)
            .expect("apply batch");
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
    assert_eq!(incremental.normalized_view(), rebuilt.normalized_view());
    assert!(base < model.current_revision());
}
