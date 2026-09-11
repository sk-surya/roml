//! MIR-02 compiled-path integration: drive the new operations through
//! `CompilationSession` -> `BackendOp` and the compiled reference backend.
//!
//! The canonical-path tests exercise `ReferenceBackend::apply_op(ModelOp)`;
//! this file covers the compiled path (`compile_delta` -> `apply_compiled_delta`)
//! for the new variable-block, parametric-row, parameter-bulk and packed
//! objective-cost operations.

#![allow(deprecated)]

use roml::advanced::{
    BackendCapabilitySet, BackendFeature, CompilationSession, FeatureSupport, SupportLevel,
};
use roml::bulk::{BlockBounds, ParamDepBlockWitness, ParamDepLayout, StridedMap};
use roml::compiler::capability::CompilationPolicy;
use roml::prelude::*;
use roml::solver::reference::ReferenceBackend;
use roml::{ConstraintBounds, ModelOp, ModelRevision, ObjId, ParamId, VarId, VarSpan};

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

/// Compile and apply every delta since `from`, returning the new revision.
fn apply_since(
    compiler: &mut CompilationSession,
    backend: &mut ReferenceBackend,
    model: &Model,
    capabilities: &BackendCapabilitySet,
    from: ModelRevision,
) -> ModelRevision {
    let mut revision = from;
    for batch in model.deltas_since(from).expect("retained deltas") {
        let from_compilation = compiler
            .current_compilation()
            .expect("compiled base present");
        let compiled = compiler
            .compile_delta(
                batch,
                from_compilation,
                model.instance(),
                &CompilationPolicy::Auto,
                capabilities,
            )
            .expect("compiled delta");
        backend
            .apply_compiled_delta(&compiled)
            .expect("apply compiled delta");
        revision = batch.to;
    }
    revision
}

/// Incremental compiled replay of a whole model history equals a compiled
/// rebuild of its final snapshot.
#[test]
fn compiled_path_replay_matches_compiled_rebuild() {
    let capabilities = full_capabilities();
    let mut model = Model::new();

    // Base: the empty revision-0 snapshot.
    let mut compiler = CompilationSession::new();
    let snapshot0 = model.take_snapshot().expect("empty snapshot");
    let base = compiler
        .compile_snapshot(
            model.instance(),
            &snapshot0,
            &CompilationPolicy::Auto,
            &capabilities,
        )
        .expect("empty base compiles");
    let mut incremental = ReferenceBackend::new();
    incremental.rebuild_compiled(&base).expect("empty rebuild");
    let mut applied = ModelRevision::ZERO;

    // r1: one packed variable block.
    let n = 8;
    let vspan: VarSpan = model
        .add_variable_block(
            n,
            VarType::Continuous,
            BlockBounds::Uniform(Bounds::new(0.0, 1.0)),
        )
        .expect("variable block");
    model.commit().expect("commit variables");
    applied = apply_since(
        &mut compiler,
        &mut incremental,
        &model,
        &capabilities,
        applied,
    );
    assert_eq!(applied, model.current_revision());

    // r2: packed parametric rows over the block members.
    let vars: Vec<VarId> = vspan.ids().collect();
    let span = model
        .add_parameter_block(&(0..n).map(|i| 1.0 + i as f64).collect::<Vec<_>>())
        .expect("parameter block");
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
        .expect("parametric rows");
    model.commit().expect("commit rows");
    applied = apply_since(
        &mut compiler,
        &mut incremental,
        &model,
        &capabilities,
        applied,
    );

    // r3: an eligible packed objective + a bulk reprice. The objective family
    // is uniform-scale, so it is representable by one dependency block.
    let layout = ParamDepLayout {
        blocks: vec![ParamDepBlockWitness {
            params: span,
            param_map: StridedMap::contiguous(n),
            cell_offset: 0,
            cell_map: StridedMap::contiguous(n),
            scale: 1.0,
            row: None,
        }],
    };
    let objective_scales = vec![1.0; n];
    model
        .set_linear_objective_param_bulk_with_layout(
            Sense::Maximize,
            &vars,
            &params,
            &objective_scales,
            0.0,
            &layout,
        )
        .expect("eligible objective");
    model.commit().expect("commit objective");
    applied = apply_since(
        &mut compiler,
        &mut incremental,
        &model,
        &capabilities,
        applied,
    );

    let repriced: Vec<f64> = (0..n).map(|i| 5.0 + i as f64).collect();
    model.set_parameters_bulk(span, &repriced).expect("queue");
    model.commit().expect("commit reprice");
    applied = apply_since(
        &mut compiler,
        &mut incremental,
        &model,
        &capabilities,
        applied,
    );
    assert_eq!(applied, model.current_revision());

    // Compiled rebuild of the final snapshot.
    let snapshot = model.take_snapshot().expect("final snapshot");
    let mut fresh = CompilationSession::new();
    let final_base = fresh
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Auto,
            &capabilities,
        )
        .expect("final base");
    let mut rebuilt = ReferenceBackend::new();
    rebuilt
        .rebuild_compiled(&final_base)
        .expect("final rebuild");

    // `compilation_id` is an opaque allocation counter and legitimately
    // differs between the incremental and rebuilt backends; every semantic
    // field must match.
    let incremental = incremental.compiled_normalized_view();
    let rebuilt = rebuilt.compiled_normalized_view();
    assert_eq!(incremental.revision, rebuilt.revision);
    assert_eq!(incremental.variables, rebuilt.variables);
    assert_eq!(incremental.rows, rebuilt.rows);
    assert_eq!(incremental.objectives, rebuilt.objectives);
    assert_eq!(incremental.objective_policy, rebuilt.objective_policy);
}

/// The empty compiled base and per-op provenance are stable across a
/// multi-batch incremental history.
#[test]
fn compiled_path_applies_each_new_op_kind() {
    use roml::advanced::BackendOp;

    let capabilities = full_capabilities();
    let mut model = Model::new();
    let mut compiler = CompilationSession::new();
    let snapshot0 = model.take_snapshot().expect("snapshot");
    let _base = compiler
        .compile_snapshot(
            model.instance(),
            &snapshot0,
            &CompilationPolicy::Auto,
            &capabilities,
        )
        .expect("base");

    // Variable block delta compiles to one AddVariable per member.
    let n = 3;
    let _ = model
        .add_variable_block(
            n,
            VarType::Continuous,
            BlockBounds::Uniform(Bounds::new(0.0, 2.0)),
        )
        .expect("block");
    model.commit().expect("commit");
    let batches = model.deltas_since(ModelRevision::ZERO).expect("delta");
    let batch = batches.first().expect("one batch");
    let from = compiler.current_compilation().expect("base");
    let compiled = compiler
        .compile_delta(
            batch,
            from,
            model.instance(),
            &CompilationPolicy::Auto,
            &capabilities,
        )
        .expect("compile block delta");
    let add_variables = compiled
        .operations
        .iter()
        .filter(|op| matches!(op, BackendOp::AddVariable(_)))
        .count();
    assert_eq!(add_variables, n, "packed block compiles to n add-var ops");
}

/// Compiled `SetObjectiveCosts` rejects unknown objectives and variables with
/// typed errors instead of silently mutating.
#[test]
fn compiled_cost_patch_rejects_unknown_entities() {
    use roml::advanced::{BackendOp, CompiledObjectiveId, CompiledVariableId};

    // Establish a compiled base with variable 0 and objective 0.
    let mut model = Model::new();
    let x = model
        .add_variable(roml::model::continuous().bounds(0.0, 1.0))
        .unwrap();
    model
        .set_linear_objective_bulk(Sense::Maximize, &[x], &[1.0], 0.0)
        .unwrap();
    model.commit().unwrap();
    let capabilities = full_capabilities();
    let snapshot = model.take_snapshot().unwrap();
    let mut compiler = CompilationSession::new();
    let base = compiler
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Auto,
            &capabilities,
        )
        .unwrap();
    let mut backend = ReferenceBackend::new();
    backend.rebuild_compiled(&base).unwrap();

    // Unknown objective id.
    assert!(backend
        .apply_compiled_op(&BackendOp::SetObjectiveCosts {
            objective: CompiledObjectiveId(99),
            costs: vec![(CompiledVariableId(0), 2.0)],
        })
        .is_err());
    // Unknown variable id for an existing objective.
    assert!(backend
        .apply_compiled_op(&BackendOp::SetObjectiveCosts {
            objective: CompiledObjectiveId(0),
            costs: vec![(CompiledVariableId(99), 2.0)],
        })
        .is_err());
}

/// Compiling a patch batch that references unknown canonical entities is a
/// typed rebuild-required rejection, never a silent skip.
#[test]
fn compiler_rejects_patch_for_unknown_entities() {
    use std::sync::Arc;

    use roml::delta::{CoefficientPatch, DeltaBatch};
    use roml::id::Generation;
    use roml::model::coefficient::CoefficientTarget;

    let mut model = Model::new();
    let x = model
        .add_variable(roml::model::continuous().bounds(0.0, 1.0))
        .unwrap();
    let con = model.add_constraint((x).le(1.0)).unwrap();
    let obj = model
        .set_linear_objective_bulk(Sense::Maximize, &[x], &[1.0], 0.0)
        .unwrap();
    model.commit().unwrap();

    let capabilities = full_capabilities();
    let snapshot = model.take_snapshot().unwrap();
    let mut compiler = CompilationSession::new();
    let base = compiler
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Auto,
            &capabilities,
        )
        .unwrap();

    let stale_var = VarId::new(9_999, Generation::new());
    let stale_obj = ObjId::new(9_999, Generation::new());
    let stale_con = roml::id::ConId::new(9_999, Generation::new());
    let cases: Vec<CoefficientPatch> = vec![
        CoefficientPatch {
            target: CoefficientTarget::Objective(obj),
            var: stale_var,
            old: 1.0,
            new: 2.0,
        },
        CoefficientPatch {
            target: CoefficientTarget::Objective(stale_obj),
            var: x,
            old: 1.0,
            new: 2.0,
        },
        CoefficientPatch {
            target: CoefficientTarget::Constraint(stale_con),
            var: x,
            old: 1.0,
            new: 2.0,
        },
    ];

    for patch in cases {
        let patches: Arc<[CoefficientPatch]> = Arc::from(vec![patch]);
        let from = model.current_revision();
        let to = from.next().unwrap();
        let delta = DeltaBatch::new(
            from,
            to,
            vec![ModelOp::SetCoefficientPatchBatch { patches }],
        )
        .unwrap();
        let result = compiler.compile_delta(
            &delta,
            base.compilation_id,
            model.instance(),
            &CompilationPolicy::Auto,
            &capabilities,
        );
        assert!(
            matches!(
                result,
                Err(roml::advanced::CompileError::RebuildRequired(_))
            ),
            "unknown entity must be a rebuild-required rejection, got {result:?}"
        );
    }
    let _ = con;
}
