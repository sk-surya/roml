//! Phase 29 planted-IIS comparison harness.
//!
//! Run with `cargo bench -p roml-highs --bench iis`. The output is evidence,
//! not a release performance claim: record machine, HiGHS, and Rust metadata
//! with the printed timings and oracle-call counts.

use std::time::Instant;

use roml::prelude::*;
use roml::{
    ConflictGuarantee, InfeasibilityMode, InfeasibilityOutcome, InfeasibilityPlan, ReductionPolicy,
    SeedPolicy, SolverSession,
};
use roml_highs::HighsSession;

fn planted_lp(size: usize) -> Model {
    let mut model = Model::new();
    let mut variables = Vec::with_capacity(size);
    for _ in 0..size {
        variables.push(model.add_variable(continuous()).expect("variable"));
    }
    for variable in variables.iter().skip(2) {
        model
            .add_constraint((*variable).ge(-100.0))
            .expect("loose planted row");
    }
    model
        .add_constraint(variables[0].ge(1.0))
        .expect("planted lower row");
    model
        .add_constraint(variables[0].le(0.0))
        .expect("planted upper row");
    model
}

fn run_case(size: usize, label: &str, plan: InfeasibilityPlan) {
    let model = planted_lp(size);
    let mut session = SolverSession::new(HighsSession::try_new().expect("bundled HiGHS"));
    let started = Instant::now();
    let report = session
        .analyze_infeasibility(&model, &plan)
        .expect("planted IIS analysis");
    let elapsed = started.elapsed();
    println!(
        "size={size:4} case={label:20} elapsed_ms={:8.3} oracle_calls={:4} members={:3} outcome={:?} guarantee={:?}",
        elapsed.as_secs_f64() * 1_000.0,
        report.statistics.oracle_calls,
        report.members.len(),
        report.outcome,
        report.guarantee
    );
    assert_eq!(report.outcome, InfeasibilityOutcome::Conflict);
    assert!(matches!(
        report.guarantee,
        ConflictGuarantee::Irreducible | ConflictGuarantee::NativeReported
    ));
}

fn main() {
    println!("Phase 29 planted IIS comparison; bundled HiGHS 1.15.0");
    for size in [32, 128, 512] {
        let mut portable_adaptive = InfeasibilityPlan::portable_lp();
        portable_adaptive.mode = InfeasibilityMode::RomlPortable;
        run_case(size, "portable-adaptive", portable_adaptive);

        let mut portable_naive = InfeasibilityPlan::portable_lp();
        portable_naive.mode = InfeasibilityMode::RomlPortable;
        portable_naive.reduction = ReductionPolicy::SingleAtom;
        run_case(size, "portable-single-atom", portable_naive);

        let mut native_seeded = InfeasibilityPlan::portable_lp();
        native_seeded.mode = InfeasibilityMode::Auto;
        native_seeded.seed_policy = SeedPolicy::Adaptive;
        run_case(size, "native-seeded-roml", native_seeded);

        let mut native_only = InfeasibilityPlan::portable_lp();
        native_only.mode = InfeasibilityMode::NativeOnly;
        run_case(size, "native-only", native_only);
    }
}
