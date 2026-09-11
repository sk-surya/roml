//! MIR-02 remediation: persistent-HiGHS direct BESS fixture.
//!
//! Proves the eligible objective reprice uses one bulk native cost call (no
//! per-cell scalar calls), and exercises the IR-17 sequence with a real
//! prior solve: build -> solve -> append fresh packed cells -> solve ->
//! mutate an existing cell, with rebuild/incremental equivalence.

#![allow(deprecated)]

use std::time::Instant;

use roml::bulk::{ParamDepBlockWitness, ParamDepLayout, ParamSpan, StridedMap};
use roml::model::coefficient::CoefficientTarget;
use roml::model::continuous;
use roml::prelude::*;
use roml::{ConstraintBounds, Model, ObjId, ParamId, VarId};
use roml_highs::Highs;

fn full_capabilities() -> roml::advanced::BackendCapabilitySet {
    use roml::advanced::{BackendCapabilitySet, BackendFeature, FeatureSupport, SupportLevel};
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

const DT: f64 = 0.25;

/// BESS-shaped eligible objective with `n` price parameters driving `2n`
/// objective cells, over unit-capacity bounded storage variables.
fn build_eligible(n: usize) -> (Model, ParamSpan, ObjId, Vec<VarId>, Vec<VarId>) {
    let mut model = Model::new();
    let charge: Vec<VarId> = (0..n)
        .map(|_| model.add_variable(continuous().bounds(0.0, 1.0)).unwrap())
        .collect();
    let discharge: Vec<VarId> = (0..n)
        .map(|_| model.add_variable(continuous().bounds(0.0, 1.0)).unwrap())
        .collect();
    // Bounded: sum(charge) + sum(discharge) <= n.
    let mut row_vars = charge.clone();
    row_vars.extend(discharge.iter().copied());
    let row_vals = vec![1.0; row_vars.len()];
    let row_ptr = [0u32, row_vars.len() as u32];
    model
        .add_linear_rows_bulk(
            &row_ptr,
            &row_vars,
            &row_vals,
            &[ConstraintBounds::le(n as f64)],
        )
        .expect("bounding row");

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
    let obj = model
        .set_linear_objective_param_bulk_with_layout(
            Sense::Maximize,
            &vars,
            &params,
            &scales,
            0.0,
            &layout,
        )
        .expect("eligible objective");
    (model, span, obj, charge, discharge)
}

#[test]
fn eligible_reprice_uses_one_bulk_highs_cost_call() {
    let n = 300 * 96; // flagship cardinality: 28,800 params / 57,600 cells
    let (mut model, span, _obj, _c, _d) = build_eligible(n);
    let mut solver = Highs::new().expect("highs");
    let t_build = Instant::now();
    solver.solve(&mut model).expect("initial solve");
    let initial = t_build.elapsed();

    let t_noop = Instant::now();
    solver.solve(&mut model).expect("no-change solve");
    let noop = t_noop.elapsed();

    // Measure only the reprice + synchronization, not the solve.
    let new: Vec<f64> = (0..n).map(|i| 51.0 + (i % 17) as f64).collect();
    roml_highs::cost_call_stats::reset();

    let t0 = Instant::now();
    model
        .set_parameters_bulk(span, &new)
        .expect("queue reprice");
    model.commit().expect("commit reprice");
    let reprice = t0.elapsed();

    let t1 = Instant::now();
    solver
        .solve(&mut model)
        .expect("reprice synchronize + solve");
    let apply_solve = t1.elapsed();

    let t_post = Instant::now();
    solver
        .solve(&mut model)
        .expect("post-reprice no-change solve");
    let post_noop = t_post.elapsed();

    let (bulk, scalar) = roml_highs::cost_call_stats::snapshot();
    let bulk_nanos = roml_highs::cost_call_stats::bulk_cost_nanos();
    let (rebuilds, delta_batches) = roml_highs::sync_stats::snapshot();
    println!(
        "MIR-02 eligible reprice: params={n} initial={initial:?} noop={noop:?} \
         reprice={reprice:?} apply+solve={apply_solve:?} post_noop={post_noop:?} \
         bulk_cost_calls={bulk} scalar_cost_calls={scalar} \
         bulk_native={bulk_nanos}ns rebuilds={rebuilds} delta_batches={delta_batches}"
    );

    assert!(
        (1..=4).contains(&bulk),
        "expected one/few bulk HiGHS cost calls, got {bulk}"
    );
    assert_eq!(
        scalar, 0,
        "pure eligible objective reprice must issue zero scalar cost calls"
    );
    assert_eq!(
        model.propagation_stats().coefficient_patch_batches,
        1,
        "one canonical coefficient-patch batch"
    );
}

#[test]
fn eligible_reprice_apply_is_separated_from_lp_solve() {
    use roml::advanced::CompilationSession;
    use roml::compiler::capability::CompilationPolicy;
    use roml::solver::request::SolveRequest;
    use roml::solver::session::{BackendSession, Synchronization};
    use roml_highs::HighsSession;

    let n = 300 * 96;
    let t_build = Instant::now();
    let (mut model, span, _obj, _c, _d) = build_eligible(n);
    let build = t_build.elapsed();
    let t_commit = Instant::now();
    model.commit().expect("commit");
    let commit = t_commit.elapsed();
    let r0 = model.current_revision();
    let t_snap = Instant::now();
    let snapshot = model.take_snapshot().expect("snapshot");
    let take_snapshot = t_snap.elapsed();
    let capabilities = full_capabilities();

    let mut compiler = CompilationSession::new();
    let t_compile = Instant::now();
    let base = compiler
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Auto,
            &capabilities,
        )
        .expect("compile snapshot");
    let compile_snapshot = t_compile.elapsed();

    let mut session = HighsSession::try_new().expect("highs");
    let t_rebuild = Instant::now();
    session
        .synchronize(Synchronization::CompiledRebuild(base))
        .expect("rebuild");
    let rebuild = t_rebuild.elapsed();
    let t_init_solve = Instant::now();
    session.solve(&SolveRequest::new()).expect("initial solve");
    let init_solve = t_init_solve.elapsed();

    let new: Vec<f64> = (0..n).map(|i| 51.0 + (i % 17) as f64).collect();
    model
        .set_parameters_bulk(span, &new)
        .expect("queue reprice");
    model.commit().expect("commit reprice");
    let deltas = model.deltas_since(r0).expect("deltas");
    let delta = deltas.last().expect("reprice delta");
    let from = compiler.current_compilation().expect("base compilation");
    let backend = compiler
        .compile_delta(
            delta,
            from,
            model.instance(),
            &CompilationPolicy::Auto,
            &capabilities,
        )
        .expect("compile reprice");

    roml_highs::cost_call_stats::reset();
    let apply_started = Instant::now();
    session
        .synchronize(Synchronization::CompiledDeltaBatch(backend))
        .expect("apply reprice");
    let apply = apply_started.elapsed();

    let solve_started = Instant::now();
    session.solve(&SolveRequest::new()).expect("solve reprice");
    let solve = solve_started.elapsed();

    let (bulk, scalar) = roml_highs::cost_call_stats::snapshot();
    println!(
        "MIR-02 apply/solve split: build={build:?} commit={commit:?} take_snapshot={take_snapshot:?} \
         compile_snapshot={compile_snapshot:?} rebuild={rebuild:?} \
         init_solve={init_solve:?} apply={apply:?} solve={solve:?} \
         bulk_cost_calls={bulk} scalar_cost_calls={scalar}"
    );
    assert_eq!(scalar, 0, "zero scalar cost calls");
    assert!(bulk >= 1, "at least one bulk cost call");
}

#[test]
fn ir17_solve_then_append_then_shadow_matches_rebuild() {
    let n = 64;
    let (mut model, span, _obj, charge, _d) = build_eligible(n);

    let mut incremental = Highs::new().expect("highs");
    let first = incremental.solve(&mut model).expect("first solve");

    // Append fresh packed cells after a real solve: a second eligible
    // objective over the same parameters, active after append.
    let (vars2, params2, scales2) = {
        let price: Vec<ParamId> = span.ids().collect();
        let mut vars = Vec::new();
        let mut params = Vec::new();
        let mut scales = Vec::new();
        for (var, param) in charge.iter().zip(price.iter()) {
            vars.push(*var);
            params.push(*param);
            scales.push(DT);
        }
        (vars, params, scales)
    };
    let layout2 = ParamDepLayout {
        blocks: vec![ParamDepBlockWitness {
            params: span,
            param_map: StridedMap::contiguous(n),
            cell_offset: 0,
            cell_map: StridedMap::contiguous(n),
            scale: DT,
            row: None,
        }],
    };
    let obj2 = model
        .set_linear_objective_param_bulk_with_layout(
            Sense::Maximize,
            &vars2,
            &params2,
            &scales2,
            0.0,
            &layout2,
        )
        .expect("append fresh packed objective");
    let second = incremental
        .solve(&mut model)
        .expect("solve after fresh append");

    // Mutate an existing logical cell: shadows the packed cell into overlay.
    model
        .set_coefficient(CoefficientTarget::Objective(obj2), charge[0], -7.0)
        .expect("shadow existing cell");
    let third = incremental.solve(&mut model).expect("solve after shadow");

    // Rebuild equivalence: a fresh session rebuilds from the model snapshot.
    let mut rebuilt = Highs::new().expect("highs");
    let rebuild = rebuilt.solve(&mut model).expect("rebuild solve");

    let close = |a: f64, b: f64| (a - b).abs() <= 1e-6 * (1.0 + a.abs().max(b.abs()));
    assert!(
        close(
            third.objective_value().unwrap_or(f64::NAN),
            rebuild.objective_value().unwrap_or(f64::NAN)
        ),
        "incremental {:?} != rebuild {:?}",
        third.objective_value(),
        rebuild.objective_value()
    );
    // Sanity: the sequence produced finite objective values.
    assert!(first.objective_value().is_some() && second.objective_value().is_some());
}
