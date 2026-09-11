//! MIR-02 edge-case coverage: canonicalization, layout rejection, transaction
//! interactions, reference/compiler projections, and API surfaces.

#![allow(deprecated)]

use std::sync::Arc;

use roml::bulk::{BlockBounds, ParamDepBlockWitness, ParamDepLayout, ParamSpan, StridedMap};
use roml::delta::{CoefficientPatch, ModelOp};
use roml::id::Generation;
use roml::model::coefficient::CoefficientTarget;
use roml::prelude::*;
use roml::{ConstraintBounds, ModelError, ModelRevision, ObjId, ParamId, ValueExpr, VarId};

const DT: f64 = 0.25;

fn price_values(n: usize) -> Vec<f64> {
    (0..n).map(|i| 50.0 + i as f64 * 0.5).collect()
}

fn vars(model: &mut Model, n: usize) -> (Vec<VarId>, Vec<VarId>) {
    (
        (0..n).map(|_| model.add_var()).collect(),
        (0..n).map(|_| model.add_var()).collect(),
    )
}

fn objective_slices(
    _n: usize,
    charge: &[VarId],
    discharge: &[VarId],
    price: &[ParamId],
) -> (Vec<VarId>, Vec<ParamId>, Vec<f64>) {
    let mut v = Vec::new();
    let mut p = Vec::new();
    let mut s = Vec::new();
    for (var, param) in charge.iter().zip(price.iter()) {
        v.push(*var);
        p.push(*param);
        s.push(-DT);
    }
    for (var, param) in discharge.iter().zip(price.iter()) {
        v.push(*var);
        p.push(*param);
        s.push(DT);
    }
    (v, p, s)
}

#[test]
fn bulk_reprice_matches_general_oracle() {
    let n = 3;
    let (mut packed, span) = eligible_model(n);
    let mut general = Model::new();
    let (gcharge, gdischarge) = vars(&mut general, n);
    let gprice: Vec<ParamId> = price_values(n)
        .iter()
        .map(|v| general.add_parameter(*v).expect("parameter"))
        .collect();
    let mut expr = LinExpr::new();
    for i in 0..n {
        expr = expr.term(ValueExpr::scaled_param(-DT, gprice[i]), gcharge[i]);
        expr = expr.term(ValueExpr::scaled_param(DT, gprice[i]), gdischarge[i]);
    }
    general.maximize(expr).expect("general objective");

    let new: Vec<f64> = (0..n).map(|i| 2.0 + i as f64).collect();
    packed.set_parameters_bulk(span, &new).expect("queue");
    packed.commit().expect("commit");
    for (param, value) in gprice.iter().zip(new.iter()) {
        general.set_parameter(*param, *value).expect("scalar");
    }
    general.commit().expect("commit general");
    assert_eq!(
        packed.take_snapshot().expect("packed"),
        general.take_snapshot().expect("general")
    );
}

// ── ParamSpan / span surfaces ─────────────────────────────────────────────

#[test]
fn parameter_block_supports_empty_and_iterates_ids() {
    let mut model = Model::new();
    let empty = model.add_parameter_block(&[]).expect("empty block");
    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);
    assert_eq!(empty.ids().count(), 0);

    let span = model.add_parameter_block(&[1.0, 2.0, 3.0]).expect("block");
    assert_eq!(span.len(), 3);
    assert!(!span.is_empty());
    let ids: Vec<ParamId> = span.ids().collect();
    assert_eq!(ids.len(), 3);
    assert_eq!(model.parameter_value(ids[2]), Some(3.0));
}

#[test]
fn variable_block_packed_op_exposes_bounds_and_type() {
    let mut model = Model::new();
    let bounds = [
        Bounds::new(0.0, 1.0),
        Bounds::new(-2.0, 3.0),
        Bounds::new(4.0, 4.0),
    ];
    model
        .add_variable_block(3, VarType::Integer, BlockBounds::PerElement(&bounds))
        .expect("block");
    model.commit().expect("commit");

    let batches = model.deltas_since(ModelRevision::ZERO).expect("delta");
    match &batches[0].operations[0] {
        ModelOp::AddVariableBlock { block } => {
            assert_eq!(block.len(), 3);
            assert!(!block.is_empty());
            assert_eq!(block.var_type(), VarType::Integer);
            assert_eq!(block.ids().count(), 3);
            for (i, expected) in bounds.iter().enumerate() {
                assert_eq!(block.bounds_for(i), Some(*expected));
            }
            assert_eq!(block.bounds_for(3), None);
            assert_eq!(block.span().len(), 3);
        }
        other => panic!("expected AddVariableBlock, got {other:?}"),
    }
}

#[test]
fn empty_variable_block_is_a_journaled_noop() {
    let mut model = Model::new();
    let span = model
        .add_variable_block(
            0,
            VarType::Continuous,
            BlockBounds::Uniform(Bounds::NON_NEGATIVE),
        )
        .expect("empty block");
    assert!(span.is_empty());
    assert_eq!(model.num_variables(), 0);
    assert_eq!(model.journal_len(), 0, "empty block records nothing");
}

// ── set_parameters_bulk edge cases ────────────────────────────────────────

fn eligible_model(n: usize) -> (Model, ParamSpan) {
    let mut model = Model::new();
    let (charge, discharge) = vars(&mut model, n);
    let span = model.add_parameter_block(&price_values(n)).expect("params");
    let price: Vec<ParamId> = span.ids().collect();
    let (v, p, s) = objective_slices(n, &charge, &discharge, &price);
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
    model
        .set_linear_objective_param_bulk_with_layout(Sense::Maximize, &v, &p, &s, 0.0, &layout)
        .expect("objective");
    (model, span)
}

#[test]
fn bulk_update_length_mismatch_is_typed() {
    let (mut model, span) = eligible_model(4);
    assert!(matches!(
        model.set_parameters_bulk(span, &[1.0, 2.0]),
        Err(ModelError::MismatchedBulkLengths { .. })
    ));
    assert!(!model.has_uncommitted());
}

#[test]
fn bulk_update_unchanged_values_do_not_advance_revision() {
    let (mut model, span) = eligible_model(4);
    model.commit().expect("flush construction");
    let revision = model.current_revision();
    let journal = model.journal_len();
    let same = price_values(4);
    model.set_parameters_bulk(span, &same).expect("queue no-op");
    model.commit().expect("commit no-op");
    assert_eq!(
        model.current_revision(),
        revision,
        "no change -> no revision"
    );
    assert_eq!(model.journal_len(), journal, "no new delta batch");
}

#[test]
fn bulk_update_empty_span_is_a_noop() {
    let mut model = Model::new();
    let empty = model.add_parameter_block(&[]).expect("empty");
    model.set_parameters_bulk(empty, &[]).expect("queue");
    model.commit().expect("commit");
    assert_eq!(model.current_revision(), ModelRevision::ZERO);
}

#[test]
fn scalar_pending_write_overrides_bulk_block_on_commit() {
    let (mut model, span) = eligible_model(4);
    let first = span.ids().next().expect("member");
    let bulk: Vec<f64> = (0..4).map(|i| 5.0 + i as f64).collect();
    model.set_parameters_bulk(span, &bulk).expect("queue bulk");
    model.set_parameter(first, 99.0).expect("queue scalar");
    model.commit().expect("commit");
    assert_eq!(model.parameter_value(first), Some(99.0), "scalar wins");
    assert_eq!(
        model.parameter_value(span.ids().nth(1).unwrap()),
        Some(bulk[1])
    );
}

// ── add_linear_rows_param_bulk edge cases ─────────────────────────────────

fn param_row_ids(n: usize) -> (Model, Vec<VarId>, ParamSpan) {
    let mut model = Model::new();
    let vars: Vec<VarId> = (0..n).map(|_| model.add_var()).collect();
    let span = model.add_parameter_block(&price_values(n)).expect("params");
    (model, vars, span)
}

#[test]
fn param_rows_reject_shape_mismatch_and_bad_inputs() {
    let n = 3;
    let (mut model, vars, span) = param_row_ids(n);
    let params: Vec<ParamId> = span.ids().collect();
    let scales = vec![1.0; n];
    let bounds = [ConstraintBounds::le(1.0)];

    // row_ptr shape mismatch.
    assert!(matches!(
        model.add_linear_rows_param_bulk(&[0, 1], &vars, &params, &scales, &bounds),
        Err(ModelError::MismatchedRowBlock { .. })
    ));
    // Invalid bounds.
    assert!(model
        .add_linear_rows_param_bulk(
            &[0, n as u32],
            &vars,
            &params,
            &scales,
            &[ConstraintBounds::le(f64::NAN)],
        )
        .is_err());
    // Non-finite scale.
    let mut bad_scales = scales.clone();
    bad_scales[1] = f64::INFINITY;
    assert!(matches!(
        model.add_linear_rows_param_bulk(&[0, n as u32], &vars, &params, &bad_scales, &bounds),
        Err(ModelError::NonFiniteValue(_))
    ));
    // Stale variable.
    let stale = VarId::new(9_999, Generation::new());
    assert!(matches!(
        model.add_linear_rows_param_bulk(
            &[0, n as u32],
            &[vars[0], stale, vars[2]],
            &params,
            &scales,
            &bounds
        ),
        Err(ModelError::VariableNotFound(_))
    ));
    // Stale parameter.
    let stale_param = ParamId::new(9_999, Generation::new());
    assert!(matches!(
        model.add_linear_rows_param_bulk(
            &[0, n as u32],
            &vars,
            &[params[0], stale_param, params[2]],
            &scales,
            &bounds
        ),
        Err(ModelError::ParameterNotFound(_))
    ));
    assert_eq!(model.num_constraints(), 0, "all rejections atomic");
    assert_eq!(model.num_coefficients(), 0);
}

#[test]
fn param_rows_drop_zero_merged_scales() {
    let (mut model, vars, span) = param_row_ids(1);
    let p = span.ids().next().unwrap();
    model
        .add_linear_rows_param_bulk(
            &[0, 2],
            &[vars[0], vars[0]],
            &[p, p],
            &[1.0, -1.0],
            &[ConstraintBounds::le(1.0)],
        )
        .expect("zero merged scale drops");
    assert_eq!(model.num_constraints(), 1);
    assert_eq!(model.num_coefficients(), 0, "zero cell dropped");
}

#[test]
fn param_rows_multi_row_snapshot_matches_general() {
    let n = 4;
    let (mut packed, vars, span) = param_row_ids(n);
    let params: Vec<ParamId> = span.ids().collect();
    let scales: Vec<f64> = (0..n).map(|i| 1.0 + i as f64).collect();
    let bounds = [ConstraintBounds::le(10.0), ConstraintBounds::ge(-3.0)];
    let cons = packed
        .add_linear_rows_param_bulk(&[0, 2, 4], &vars, &params, &scales, &bounds)
        .expect("two param rows");
    assert_eq!(cons.len(), 2);

    let mut general = Model::new();
    let gvars: Vec<VarId> = (0..n).map(|_| general.add_var()).collect();
    let gparams: Vec<ParamId> = price_values(n)
        .iter()
        .map(|v| general.add_parameter(*v).unwrap())
        .collect();
    for (r, bound) in bounds.iter().enumerate() {
        let row = general.add_constraint(*bound).expect("row");
        for k in (r * 2)..(r * 2 + 2) {
            general
                .add_constraint_coefficient(
                    row,
                    gvars[k],
                    ValueExpr::scaled_param(scales[k], gparams[k]),
                )
                .expect("cell");
        }
    }
    assert_eq!(
        packed.take_snapshot().expect("packed"),
        general.take_snapshot().expect("general")
    );
}

// ── Layout rejection edge cases ───────────────────────────────────────────

fn layout_case(model: &mut Model, n: usize, witness_row: Option<u32>) -> ModelError {
    let (charge, discharge) = vars(model, n);
    let span = model.add_parameter_block(&price_values(n)).expect("params");
    let price: Vec<ParamId> = span.ids().collect();
    let (v, p, s) = objective_slices(n, &charge, &discharge, &price);
    let layout = ParamDepLayout {
        blocks: vec![ParamDepBlockWitness {
            params: span,
            param_map: StridedMap::contiguous(n),
            cell_offset: 0,
            cell_map: StridedMap::contiguous(n),
            scale: -DT,
            row: witness_row,
        }],
    };
    model
        .set_linear_objective_param_bulk_with_layout(Sense::Maximize, &v, &p, &s, 0.0, &layout)
        .expect_err("must reject")
}

#[test]
fn objective_layout_may_not_reference_a_row() {
    let mut model = Model::new();
    let error = layout_case(&mut model, 4, Some(0));
    assert!(matches!(error, ModelError::InvalidParamDepLayout(_)));
}

#[test]
fn objective_layout_rejects_param_offset_out_of_range() {
    let n = 4;
    let mut model = Model::new();
    let (charge, discharge) = vars(&mut model, n);
    let span = model.add_parameter_block(&price_values(n)).expect("params");
    let price: Vec<ParamId> = span.ids().collect();
    let (v, p, s) = objective_slices(n, &charge, &discharge, &price);
    // Maps claim n+1 ordinals but the parameter span has only n.
    let layout = ParamDepLayout {
        blocks: vec![ParamDepBlockWitness {
            params: span,
            param_map: StridedMap::contiguous(n + 1),
            cell_offset: 0,
            cell_map: StridedMap::contiguous(n + 1),
            scale: -DT,
            row: None,
        }],
    };
    let error = model
        .set_linear_objective_param_bulk_with_layout(Sense::Maximize, &v, &p, &s, 0.0, &layout)
        .expect_err("param offset out of range");
    assert!(matches!(error, ModelError::InvalidParamDepLayout(_)));
    assert_eq!(model.num_coefficients(), 0);
}

#[test]
fn objective_layout_rejects_cell_offset_outside_run() {
    let n = 4;
    let mut model = Model::new();
    let (charge, discharge) = vars(&mut model, n);
    let span = model.add_parameter_block(&price_values(n)).expect("params");
    let price: Vec<ParamId> = span.ids().collect();
    let (v, p, s) = objective_slices(n, &charge, &discharge, &price);
    let layout = ParamDepLayout {
        blocks: vec![ParamDepBlockWitness {
            params: span,
            param_map: StridedMap::contiguous(n),
            cell_offset: 10_000,
            cell_map: StridedMap::contiguous(n),
            scale: -DT,
            row: None,
        }],
    };
    let error = model
        .set_linear_objective_param_bulk_with_layout(Sense::Maximize, &v, &p, &s, 0.0, &layout)
        .expect_err("cell offset outside run");
    assert!(matches!(error, ModelError::InvalidParamDepLayout(_)));
}

// ── Reference backend projection ──────────────────────────────────────────

#[test]
fn reference_patch_missing_cell_is_a_typed_error() {
    use roml::solver::reference::ReferenceBackend;
    let mut backend = ReferenceBackend::new();
    let patches: Arc<[CoefficientPatch]> = Arc::from(vec![CoefficientPatch {
        target: CoefficientTarget::Objective(ObjId::new(0, Generation::new())),
        var: VarId::new(0, Generation::new()),
        old: 0.0,
        new: 1.0,
    }]);
    let error = backend
        .apply_op(&ModelOp::SetCoefficientPatchBatch { patches })
        .expect_err("missing target cell rejects");
    assert!(error.contains("missing"));
}

#[test]
fn reference_patch_preserves_symbolic_expression_and_updates_cache() {
    use roml::solver::reference::ReferenceBackend;
    let mut backend = ReferenceBackend::new();
    let obj = ObjId::new(0, Generation::new());
    let var = VarId::new(0, Generation::new());
    let param = ParamId::new(0, Generation::new());
    backend
        .apply_op(&ModelOp::AddObjective {
            obj,
            sense: Sense::Maximize,
        })
        .expect("objective");
    backend
        .apply_op(&ModelOp::SetObjectiveCell {
            cell_key: (CoefficientTarget::Objective(obj), var),
            value_expr: ValueExpr::scaled_param(2.5, param),
            evaluated_value: 2.5,
            constant: 0.0,
        })
        .expect("cell");
    let before = backend.symbolic_objective_cells();

    let patches: Arc<[CoefficientPatch]> = Arc::from(vec![CoefficientPatch {
        target: CoefficientTarget::Objective(obj),
        var,
        old: 2.5,
        new: 7.5,
    }]);
    backend
        .apply_op(&ModelOp::SetCoefficientPatchBatch { patches })
        .expect("patch");

    let after = backend.symbolic_objective_cells();
    assert_eq!(after, before, "symbolic expression preserved by a patch");
}

// ── Compiler projection: constraint patches stay per cell ─────────────────

#[test]
fn compiler_compiles_constraint_patch_to_scalar_linear_op() {
    use roml::advanced::{
        BackendCapabilitySet, BackendFeature, BackendOp, CompilationSession, FeatureSupport,
        SupportLevel,
    };
    use roml::compiler::capability::CompilationPolicy;
    use roml::{DeltaBatch, ModelRevision};

    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 1.0)).unwrap();
    let c = model.add_constraint((x).le(5.0)).unwrap();
    model.minimize(x).unwrap();
    model.commit().unwrap();

    let capabilities = {
        let mut set = BackendCapabilitySet::new();
        for feature in [
            BackendFeature::Lp,
            BackendFeature::IncrementalBounds,
            BackendFeature::IncrementalRows,
            BackendFeature::IncrementalCoefficients,
        ] {
            set.set(
                feature,
                FeatureSupport {
                    level: SupportLevel::Native,
                    limitations: Default::default(),
                },
            );
        }
        set
    };
    let snapshot = model.take_snapshot().unwrap();
    let mut session = CompilationSession::new();
    let base = session
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Auto,
            &capabilities,
        )
        .unwrap();

    let patches: Arc<[CoefficientPatch]> = Arc::from(vec![CoefficientPatch {
        target: CoefficientTarget::Constraint(c),
        var: x,
        old: 1.0,
        new: 3.0,
    }]);
    let from = model.current_revision();
    let to = from.next().unwrap();
    let delta = DeltaBatch::new(
        from,
        to,
        vec![ModelOp::SetCoefficientPatchBatch { patches }],
    )
    .unwrap();
    let compiled = session
        .compile_delta(
            &delta,
            base.compilation_id,
            model.instance(),
            &CompilationPolicy::Auto,
            &capabilities,
        )
        .expect("compile constraint patch");
    assert!(matches!(
        compiled.operations.as_slice(),
        [BackendOp::SetLinearCoefficient { .. }]
    ));
    let _ = ModelRevision::ZERO;
}

// ── Multiple blocks, partial scalar updates, shadow dependencies ──────────

#[test]
fn two_block_updates_in_one_commit_emit_two_pairs() {
    let (mut model, span) = eligible_model(4);
    model.commit().expect("flush");
    let base = model.current_revision();
    let a: Vec<f64> = (0..4).map(|i| 1.0 + i as f64).collect();
    let b: Vec<f64> = (0..4).map(|i| 9.0 + i as f64).collect();
    model.set_parameters_bulk(span, &a).expect("first");
    model.set_parameters_bulk(span, &b).expect("second");
    model.commit().expect("commit");

    let batches = model.deltas_since(base).expect("delta");
    assert_eq!(batches.len(), 1);
    assert_eq!(
        batches[0].operations.len(),
        4,
        "two committed blocks -> two parameter ops + two patch batches"
    );
    assert_eq!(model.propagation_stats().coefficient_patch_batches, 2);
    // Last write wins.
    assert_eq!(
        model.parameter_value(span.ids().next().unwrap()),
        Some(b[0])
    );
}

#[test]
fn partial_layout_scalar_update_reprices_covered_and_uncovered() {
    let n = 6;
    let (mut model, charge, discharge, span) = {
        let mut model = Model::new();
        let (charge, discharge) = vars(&mut model, n);
        let span = model.add_parameter_block(&price_values(n)).unwrap();
        let price: Vec<ParamId> = span.ids().collect();
        let (v, p, s) = objective_slices(n, &charge, &discharge, &price);
        // Cover only the charge family.
        let layout = ParamDepLayout {
            blocks: vec![ParamDepBlockWitness {
                params: span,
                param_map: StridedMap::contiguous(n),
                cell_offset: 0,
                cell_map: StridedMap::contiguous(n),
                scale: -DT,
                row: None,
            }],
        };
        model
            .set_linear_objective_param_bulk_with_layout(Sense::Maximize, &v, &p, &s, 0.0, &layout)
            .unwrap();
        (model, charge, discharge, span)
    };
    assert_eq!(model.lowering_stats().param_positions_cells as usize, n);

    let mut general = Model::new();
    let (gcharge, gdischarge) = vars(&mut general, n);
    let gprice: Vec<ParamId> = price_values(n)
        .iter()
        .map(|v| general.add_parameter(*v).unwrap())
        .collect();
    let mut expr = LinExpr::new();
    for i in 0..n {
        expr = expr.term(ValueExpr::scaled_param(-DT, gprice[i]), gcharge[i]);
        expr = expr.term(ValueExpr::scaled_param(DT, gprice[i]), gdischarge[i]);
    }
    general.maximize(expr).unwrap();

    // A single parameter drives one covered and one uncovered cell.
    let p0 = span.ids().next().unwrap();
    model.set_parameter(p0, 77.0).unwrap();
    model.commit().unwrap();
    general.set_parameter(gprice[0], 77.0).unwrap();
    general.commit().unwrap();

    assert_eq!(
        model.take_snapshot().unwrap(),
        general.take_snapshot().unwrap()
    );
    let _ = (&charge, &discharge);
}

#[test]
fn shadowed_block_cell_keeps_overlay_dependency() {
    let n = 4;
    let (model, charge, _discharge, span) = {
        let mut model = Model::new();
        let (charge, discharge) = vars(&mut model, n);
        let span = model.add_parameter_block(&price_values(n)).unwrap();
        let price: Vec<ParamId> = span.ids().collect();
        let (v, p, s) = objective_slices(n, &charge, &discharge, &price);
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
            .set_linear_objective_param_bulk_with_layout(Sense::Maximize, &v, &p, &s, 0.0, &layout)
            .unwrap();
        model
            .set_coefficient(CoefficientTarget::Objective(obj), charge[0], 1.0)
            .unwrap();
        (model, charge, discharge, span)
    };
    // The shadowed charge cell moved to overlay with a constant expression
    // (replacement drops its parameter dependency); the live discharge block
    // cell remains, so the parameter reports exactly one dependent cell.
    let p0 = span.ids().next().unwrap();
    assert_eq!(model.parameter_dependent_count(p0), 1);
    assert!(model.variable_bounds(charge[0]).is_some());
}

// ── Diagnostics and block parameter naming ────────────────────────────────

#[test]
fn diagnostics_reset_clears_all_counters() {
    let (mut model, _span) = eligible_model(4);
    assert!(model.lowering_stats().param_dep_blocks > 0);
    model.reset_diagnostics();
    assert_eq!(
        model.lowering_stats(),
        roml::diagnostics::LoweringStats::default()
    );
    assert_eq!(
        model.propagation_stats(),
        roml::diagnostics::PropagationStats::default()
    );
}

#[test]
fn block_parameters_have_no_materialized_names() {
    let mut model = Model::new();
    let span = model.add_parameter_block(&[1.0, 2.0, 3.0]).expect("block");
    for param in span.ids() {
        assert_eq!(model.parameter_name(param).expect("live"), None);
    }
}

// ── Parametric row scalar update and reference replay ─────────────────────

fn build_param_rows(n: usize) -> (Model, ParamSpan, Vec<ParamId>, Vec<VarId>, Vec<f64>) {
    let mut model = Model::new();
    let vars: Vec<VarId> = (0..n).map(|_| model.add_var()).collect();
    let span = model.add_parameter_block(&price_values(n)).unwrap();
    let params: Vec<ParamId> = span.ids().collect();
    let scales: Vec<f64> = (0..n).map(|i| 1.0 + i as f64).collect();
    model
        .add_linear_rows_param_bulk(
            &[0, n as u32],
            &vars,
            &params,
            &scales,
            &[ConstraintBounds::le(100.0)],
        )
        .expect("param rows");
    (model, span, params, vars, scales)
}

#[test]
fn scalar_update_reprices_parametric_row() {
    let n = 5;
    let (mut packed, _span, params, _vars, scales) = build_param_rows(n);
    let mut general = Model::new();
    let gvars: Vec<VarId> = (0..n).map(|_| general.add_var()).collect();
    let gparams: Vec<ParamId> = price_values(n)
        .iter()
        .map(|v| general.add_parameter(*v).unwrap())
        .collect();
    let gcon = general.add_constraint(ConstraintBounds::le(100.0)).unwrap();
    for i in 0..n {
        general
            .add_constraint_coefficient(
                gcon,
                gvars[i],
                ValueExpr::scaled_param(scales[i], gparams[i]),
            )
            .unwrap();
    }

    packed.set_parameter(params[2], 41.0).unwrap();
    packed.commit().unwrap();
    general.set_parameter(gparams[2], 41.0).unwrap();
    general.commit().unwrap();
    assert_eq!(
        packed.take_snapshot().unwrap(),
        general.take_snapshot().unwrap()
    );
}

#[test]
fn parametric_row_delta_replays_on_reference_backend() {
    use roml::solver::reference::ReferenceBackend;
    use roml::sync::{AdapterCursor, ApplyOutcome};

    let (mut model, span, _params, _vars, _scales) = build_param_rows(6);
    model.commit().expect("construction commit");
    // A reprice also synchronizes parameter values (parameter existence alone
    // is intentionally not a solver-facing op).
    let new: Vec<f64> = (0..6).map(|i| 7.0 + i as f64).collect();
    model
        .set_parameters_bulk(span, &new)
        .expect("queue reprice");
    model.commit().expect("reprice commit");

    let mut incremental = ReferenceBackend::new();
    let mut cursor = AdapterCursor::new();
    for batch in model.deltas_since(ModelRevision::ZERO).expect("delta") {
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
    assert_eq!(incremental.normalized_view(), rebuilt.normalized_view());
}

#[test]
fn removed_overlay_cell_is_dropped_from_dependency_iteration() {
    let n = 4;
    let mut model = Model::new();
    let (charge, discharge) = vars(&mut model, n);
    let span = model.add_parameter_block(&price_values(n)).unwrap();
    let price: Vec<ParamId> = span.ids().collect();
    let (v, p, s) = objective_slices(n, &charge, &discharge, &price);
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
        .set_linear_objective_param_bulk_with_layout(Sense::Maximize, &v, &p, &s, 0.0, &layout)
        .unwrap();
    let p0 = span.ids().next().unwrap();
    assert_eq!(model.parameter_dependent_count(p0), 2, "charge + discharge");

    // Shadow, then remove the charge cell; its overlay slot tombstones.
    let target = CoefficientTarget::Objective(obj);
    model.set_coefficient(target, charge[0], 1.0).unwrap();
    model.remove_coefficient_at(target, charge[0]).unwrap();
    assert_eq!(
        model.parameter_dependent_count(p0),
        1,
        "a removed overlay cell is not a dependency"
    );
    model.validate_invariants().expect("invariants");
}

#[test]
fn param_rows_reject_non_monotone_and_allow_empty_rows() {
    let n = 2;
    let (mut model, vars, span) = param_row_ids(n);
    let params: Vec<ParamId> = span.ids().collect();
    let scales = vec![1.0; n];

    // Non-monotone row pointers.
    assert!(matches!(
        model.add_linear_rows_param_bulk(
            &[0, 2, 1],
            &vars,
            &params,
            &scales,
            &[ConstraintBounds::le(1.0), ConstraintBounds::le(2.0)]
        ),
        Err(ModelError::MismatchedRowBlock { .. })
    ));
    // row_ptr must start at zero.
    assert!(matches!(
        model.add_linear_rows_param_bulk(
            &[1, n as u32],
            &vars,
            &params,
            &scales,
            &[ConstraintBounds::le(1.0)]
        ),
        Err(ModelError::MismatchedRowBlock { .. })
    ));

    // An empty leading row is valid; the second row carries both cells.
    let cons = model
        .add_linear_rows_param_bulk(
            &[0, 0, n as u32],
            &vars,
            &params,
            &scales,
            &[ConstraintBounds::le(1.0), ConstraintBounds::le(2.0)],
        )
        .expect("empty leading row");
    assert_eq!(cons.len(), 2);
    assert_eq!(model.num_coefficients(), n);
}

#[test]
fn packed_delta_payloads_carry_expected_fields() {
    let n = 8;
    let (mut model, span) = eligible_model(n);
    model.commit().expect("flush");
    let base = model.current_revision();
    let new: Vec<f64> = (0..n).map(|i| 3.0 + i as f64).collect();
    model.set_parameters_bulk(span, &new).expect("queue");
    model.commit().expect("commit");

    let batches = model.deltas_since(base).expect("delta");
    assert_eq!(batches[0].operations.len(), 2);
    match &batches[0].operations[0] {
        ModelOp::SetParametersBulk { changes } => {
            assert_eq!(changes.len(), n);
            assert_eq!(changes[0].new, new[0]);
            assert_ne!(changes[0].old, changes[0].new);
        }
        other => panic!("expected SetParametersBulk, got {other:?}"),
    }
    let objective = model.active_objective().expect("active objective");
    match &batches[0].operations[1] {
        ModelOp::SetCoefficientPatchBatch { patches } => {
            assert_eq!(patches.len(), 2 * n, "charge + discharge cells");
            assert!(patches
                .iter()
                .all(|p| p.target == CoefficientTarget::Objective(objective)));
            assert!(patches.iter().all(|p| p.new.is_finite()));
        }
        other => panic!("expected SetCoefficientPatchBatch, got {other:?}"),
    }
}

#[test]
fn parametric_row_block_payload_is_well_formed() {
    let n = 3;
    let (mut model, vars, span) = param_row_ids(n);
    let params: Vec<ParamId> = span.ids().collect();
    let scales = vec![1.5, -2.0, 0.25];
    model
        .add_linear_rows_param_bulk(
            &[0, n as u32],
            &vars,
            &params,
            &scales,
            &[ConstraintBounds::le(100.0)],
        )
        .expect("row");
    model.commit().expect("commit");

    let batches = model.deltas_since(ModelRevision::ZERO).expect("delta");
    let op = batches
        .iter()
        .flat_map(|b| b.operations.iter())
        .find(|op| matches!(op, ModelOp::AddParametricRows { .. }))
        .expect("parametric row op");
    match op {
        ModelOp::AddParametricRows { block } => {
            assert_eq!(block.constraints.len(), 1);
            assert_eq!(block.row_ptr, vec![0, n as u32]);
            assert_eq!(block.bounds, vec![ConstraintBounds::le(100.0)]);
            assert_eq!(block.vars, vars);
            assert_eq!(block.params, params);
            assert_eq!(block.scales, scales);
            assert_eq!(block.values.len(), n);
            assert!(block.values.iter().all(|v| v.is_finite()));
        }
        other => panic!("expected AddParametricRows, got {other:?}"),
    }
}
