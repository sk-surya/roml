//! MIR-02 remediation: packed objective-cost batching through the backend IR.
//!
//! Proves a direct eligible reprice compiles to a packed objective-cost op and
//! does **not** expand into one scalar `SetObjectiveCoefficient` per cell.

#![allow(deprecated)]

use roml::advanced::{
    BackendCapabilitySet, BackendFeature, BackendOp, CompilationSession, FeatureSupport,
    SupportLevel,
};
use roml::bulk::{ParamDepBlockWitness, ParamDepLayout, ParamSpan, StridedMap};
use roml::compiler::capability::CompilationPolicy;
use roml::model::continuous;
use roml::prelude::*;
use roml::{ModelError, ModelOp, ObjId, ParamId, VarId};

const DT: f64 = 0.25;

fn full_capabilities() -> BackendCapabilitySet {
    let mut set = BackendCapabilitySet::new();
    for feature in [
        BackendFeature::Lp,
        BackendFeature::Mip,
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
}

fn build_eligible(n: usize) -> Result<(Model, ParamSpan, ObjId), ModelError> {
    let mut model = Model::new();
    let charge: Vec<VarId> = (0..n)
        .map(|_| model.add_variable(continuous().bounds(0.0, 1.0)).unwrap())
        .collect();
    let discharge: Vec<VarId> = (0..n)
        .map(|_| model.add_variable(continuous().bounds(0.0, 1.0)).unwrap())
        .collect();
    let span = model
        .add_parameter_block(&(0..n).map(|i| 50.0 + i as f64).collect::<Vec<_>>())
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
    let obj = model.set_linear_objective_param_bulk_with_layout(
        Sense::Maximize,
        &vars,
        &params,
        &scales,
        0.0,
        &layout,
    )?;
    Ok((model, span, obj))
}

#[test]
fn eligible_reprice_compiles_to_one_packed_cost_op() {
    let n = 1024;
    let (mut model, span, _obj) = build_eligible(n).expect("eligible objective");
    model.commit().expect("construction commit");
    let base_revision = model.current_revision();
    let snapshot = model.take_snapshot().expect("snapshot");

    let mut session = CompilationSession::new();
    let base = session
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .expect("compile snapshot");

    // Reprice: canonical delta is one parameter-value change + one packed
    // coefficient-patch batch.
    let new: Vec<f64> = (0..n).map(|i| 1.0 + i as f64).collect();
    model.set_parameters_bulk(span, &new).expect("queue");
    model.commit().expect("reprice commit");
    let deltas = model.deltas_since(base_revision).expect("retained delta");
    assert_eq!(deltas.len(), 1);
    assert_eq!(
        deltas[0].operations.len(),
        2,
        "canonical delta stays two packed ops"
    );

    let compiled = session
        .compile_delta(
            deltas[0],
            base.compilation_id,
            model.instance(),
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .expect("compile reprice delta");

    let packed_cost_ops = compiled
        .operations
        .iter()
        .filter(|op| matches!(op, BackendOp::SetObjectiveCosts { .. }))
        .count();
    let scalar_cost_ops = compiled
        .operations
        .iter()
        .filter(|op| matches!(op, BackendOp::SetObjectiveCoefficient { .. }))
        .count();
    let scalar_linear_ops = compiled
        .operations
        .iter()
        .filter(|op| matches!(op, BackendOp::SetLinearCoefficient { .. }))
        .count();

    assert_eq!(packed_cost_ops, 1, "one packed objective-cost op");
    assert_eq!(scalar_cost_ops, 0, "no per-cell scalar cost ops");
    assert_eq!(scalar_linear_ops, 0, "no scalar linear ops in this fixture");

    // The packed op carries all 2n affected objective cells.
    if let BackendOp::SetObjectiveCosts { costs, .. } = &compiled.operations[0] {
        assert_eq!(costs.len(), 2 * n);
    } else {
        panic!(
            "expected SetObjectiveCosts first, got {:?}",
            compiled.operations
        );
    }
}

// ── Mixed and multi-objective patch projection ────────────────────────────

#[test]
fn mixed_objective_and_constraint_patches_compile_to_packed_and_scalar() {
    use std::sync::Arc;

    use roml::delta::{CoefficientPatch, DeltaBatch};
    use roml::model::coefficient::CoefficientTarget;

    let mut model = Model::new();
    let x = model
        .add_variable(continuous().bounds(0.0, 10.0))
        .expect("x");
    let y = model
        .add_variable(continuous().bounds(0.0, 10.0))
        .expect("y");
    let con = model.add_constraint((x + y).le(20.0)).expect("row");
    let obj = model
        .set_linear_objective_bulk(Sense::Minimize, &[x, y], &[1.0, 2.0], 0.0)
        .expect("objective");
    model.commit().expect("commit");

    let snapshot = model.take_snapshot().expect("snapshot");
    let mut session = CompilationSession::new();
    let base = session
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .expect("compile snapshot");

    let patches: Arc<[CoefficientPatch]> = Arc::from(vec![
        CoefficientPatch {
            target: CoefficientTarget::Objective(obj),
            var: x,
            old: 1.0,
            new: 5.0,
        },
        CoefficientPatch {
            target: CoefficientTarget::Objective(obj),
            var: y,
            old: 2.0,
            new: 6.0,
        },
        CoefficientPatch {
            target: CoefficientTarget::Constraint(con),
            var: x,
            old: 1.0,
            new: 7.0,
        },
    ]);
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
            &full_capabilities(),
        )
        .expect("compile mixed patch");

    let packed = compiled
        .operations
        .iter()
        .filter(|op| matches!(op, BackendOp::SetObjectiveCosts { .. }))
        .count();
    let linear = compiled
        .operations
        .iter()
        .filter(|op| matches!(op, BackendOp::SetLinearCoefficient { .. }))
        .count();
    assert_eq!(packed, 1, "one packed objective-cost op");
    assert_eq!(linear, 1, "constraint patch stays per-cell");
}

#[test]
fn patches_to_two_objectives_compile_to_two_packed_ops() {
    use std::sync::Arc;

    use roml::delta::{CoefficientPatch, DeltaBatch};
    use roml::model::coefficient::CoefficientTarget;

    let mut model = Model::new();
    let x = model
        .add_variable(continuous().bounds(0.0, 10.0))
        .expect("x");
    let obj1 = model
        .set_linear_objective_bulk(Sense::Minimize, &[x], &[1.0], 0.0)
        .expect("obj1");
    let obj2 = model
        .set_linear_objective_bulk(Sense::Maximize, &[x], &[2.0], 0.0)
        .expect("obj2");
    // The second objective is active; patch both.
    model.commit().expect("commit");

    let snapshot = model.take_snapshot().expect("snapshot");
    let mut session = CompilationSession::new();
    let base = session
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Auto,
            &full_capabilities(),
        )
        .expect("compile snapshot");

    let patches: Arc<[CoefficientPatch]> = Arc::from(vec![
        CoefficientPatch {
            target: CoefficientTarget::Objective(obj1),
            var: x,
            old: 1.0,
            new: 3.0,
        },
        CoefficientPatch {
            target: CoefficientTarget::Objective(obj2),
            var: x,
            old: 2.0,
            new: 4.0,
        },
    ]);
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
            &full_capabilities(),
        )
        .expect("compile two-objective patch");
    let packed = compiled
        .operations
        .iter()
        .filter(|op| matches!(op, BackendOp::SetObjectiveCosts { .. }))
        .count();
    assert_eq!(packed, 2, "one packed op per objective");
}
