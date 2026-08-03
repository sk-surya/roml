//! P32 Task 16 — native/portable feasible-set equivalence on HiGHS
//! (packet: "Enumerate small binary domains and compare
//! semantic/reference/native/portable feasible sets").
//!
//! Each test builds a model with one logical construct over a small binary
//! domain, enumerates the semantic truth set, and verifies that HiGHS solving
//! the compiled formulation (Auto → bridge for HiGHS, and Portable) finds
//! exactly the semantic-feasible assignments.

use roml::compiler::capability::CompilationPolicy;
use roml::compiler::session::CompilationSession;
use roml::construct::{BooleanKind, CardinalityKind, IndicatorDirection};
use roml::id::VarId;
use roml::prelude::*;
use roml::solver::backend::TerminationStatus;
use roml::solver::request::SolveRequest;
use roml::solver::session::{BackendSession, Synchronization};
use roml_highs::HighsSession;

fn highs_caps() -> roml::compiler::capability::BackendCapabilitySet {
    roml_highs::highs_capability_set(1, 15, 0)
}

/// Enumerate the assignments of `binary_vars` that HiGHS finds feasible for
/// the compiled model (with each assignment fixed via ordinary constraints).
/// The construct is present in the model, so a feasible solve means the
/// construct's compiled rows hold for that assignment.
fn highs_feasible_assignments(
    base: &Model,
    binary_vars: &[VarId],
    policy: CompilationPolicy,
) -> Vec<Vec<u8>> {
    let n = binary_vars.len();
    let mut out = Vec::new();
    for mask in 0..(1 << n) {
        let assignment: Vec<u8> = (0..n).map(|i| ((mask >> i) & 1) as u8).collect();

        // Probe: clone the model and fix the binary domain to this assignment.
        let mut probe = base.clone();
        for (i, &var) in binary_vars.iter().enumerate() {
            probe
                .add_constraint((LinExpr::from(var)).eq(assignment[i] as f64))
                .unwrap();
        }
        let snapshot = probe.take_snapshot().unwrap();
        let mut session = CompilationSession::new();
        let compiled = session
            .compile_snapshot(probe.instance(), &snapshot, &policy, &highs_caps())
            .expect("snapshot must compile against HiGHS bridge capabilities");

        let mut highs = HighsSession::try_new().expect("bundled HiGHS available");
        highs
            .synchronize(Synchronization::CompiledRebuild(compiled))
            .expect("sync must succeed");
        let result = highs
            .solve(&SolveRequest::new())
            .expect("solve must succeed");
        if matches!(result.termination, TerminationStatus::Optimal) {
            out.push(assignment);
        }
    }
    out.sort();
    out
}

fn semantic_feasible(n: usize, predicate: impl Fn(&[u8]) -> bool) -> Vec<Vec<u8>> {
    let mut out = (0..(1 << n))
        .map(|mask| (0..n).map(|i| ((mask >> i) & 1) as u8).collect::<Vec<u8>>())
        .filter(|a| predicate(a))
        .collect::<Vec<_>>();
    out.sort();
    out
}

#[test]
fn indicator_highs_bridge_and_portable_feasible_sets_match_semantic() {
    let mut model = Model::new();
    let z = model.add_variable(binary()).unwrap();
    let x = model.add_variable(binary()).unwrap();
    let y = model.add_variable(binary()).unwrap();
    model
        .add_indicator(z, IndicatorDirection::WhenOne, (x + y).le(1.0), None)
        .unwrap();
    let vars = [z, x, y];

    let semantic = semantic_feasible(3, |a| !(a[0] == 1) || (a[1] + a[2] <= 1));

    // HiGHS declares bridge (not native) for Indicator: Auto → exact bridge.
    let auto = highs_feasible_assignments(&model, &vars, CompilationPolicy::Auto);
    let portable = highs_feasible_assignments(&model, &vars, CompilationPolicy::Portable);

    assert_eq!(
        semantic, auto,
        "Auto (bridge) feasible set must equal the semantic set"
    );
    assert_eq!(
        semantic, portable,
        "Portable feasible set must equal the semantic set"
    );
}

#[test]
fn reification_highs_feasible_sets_match_semantic() {
    let mut model = Model::new();
    let x = model.add_variable(binary()).unwrap();
    let y = model.add_variable(binary()).unwrap();
    // x+y is proven integer-valued → unit gap inferred. The reification result
    // binary variable is created by the builder (design §16.2).
    let k = model.add_reify((x + y).le(1.0), None, None).unwrap();
    let snap = model.take_snapshot().unwrap();
    let b = match &snap.constructs.iter().find(|e| e.id == k).unwrap().kind {
        roml::construct::ConstructKind::Reification(p) => p.activator,
        other => panic!("expected Reification payload, got {other:?}"),
    };
    let vars = [b, x, y];

    let semantic = semantic_feasible(3, |a| {
        let lhs = a[1] + a[2];
        (a[0] == 1) == (lhs <= 1)
    });

    let auto = highs_feasible_assignments(&model, &vars, CompilationPolicy::Auto);
    let portable = highs_feasible_assignments(&model, &vars, CompilationPolicy::Portable);

    assert_eq!(
        semantic, auto,
        "Auto reification feasible set must equal the semantic set"
    );
    assert_eq!(
        semantic, portable,
        "Portable reification feasible set must equal the semantic set"
    );
}

#[test]
fn boolean_and_cardinality_highs_feasible_sets_match_semantic() {
    let mut model = Model::new();
    let a = model.add_variable(binary()).unwrap();
    let b = model.add_variable(binary()).unwrap();
    let c = model.add_variable(binary()).unwrap();
    model
        .add_boolean(
            BooleanKind::Implication {
                antecedent: a,
                consequent: b,
            },
            None,
        )
        .unwrap();
    model
        .add_cardinality(vec![a, b, c], CardinalityKind::AtMost, 1.0, None)
        .unwrap();
    let vars = [a, b, c];

    let semantic = semantic_feasible(3, |a| a[0] <= a[1] && a[0] + a[1] + a[2] <= 1);

    let auto = highs_feasible_assignments(&model, &vars, CompilationPolicy::Auto);
    let portable = highs_feasible_assignments(&model, &vars, CompilationPolicy::Portable);

    assert_eq!(
        semantic, auto,
        "Auto feasible set must equal the semantic set"
    );
    assert_eq!(
        semantic, portable,
        "Portable feasible set must equal the semantic set"
    );
}
