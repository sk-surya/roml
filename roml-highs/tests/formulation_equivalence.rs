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
use roml::construct::{
    AbsoluteValueVariant, BooleanKind, CardinalityKind, IndicatorDirection, MinMaxRelation,
    MinMaxSense, ProductOperand,
};
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

/// Whether HiGHS finds the model with the given (var, value) fixes feasible
/// under `policy` (each fix is an ordinary equality constraint on a clone).
fn highs_feasible_for_fixes(
    base: &Model,
    fixes: &[(VarId, f64)],
    policy: CompilationPolicy,
) -> bool {
    let mut probe = base.clone();
    for &(var, value) in fixes {
        probe
            .add_constraint((LinExpr::from(var)).eq(value))
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
    matches!(result.termination, TerminationStatus::Optimal)
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

#[test]
fn minmax_highs_exact_feasible_sets_match_semantic() {
    // Exact max over two binary operands: HiGHS finds the probe feasible iff
    // y == max(x1,x2). Enumerates the (x1,x2,y) domain via fixed probes.
    let mut model = Model::new();
    let x1 = model.add_variable(binary()).unwrap();
    let x2 = model.add_variable(binary()).unwrap();
    let (_, y) = model
        .add_minmax(
            vec![x1.into(), x2.into()],
            MinMaxSense::Max,
            MinMaxRelation::Exact,
            None,
        )
        .unwrap();

    let mut mismatches = Vec::new();
    for a in 0..2 {
        for b in 0..2 {
            for yv in 0..2 {
                let semantic = (yv as f64) == (a.max(b) as f64);
                let auto = highs_feasible_for_fixes(
                    &model,
                    &[(x1, a as f64), (x2, b as f64), (y, yv as f64)],
                    CompilationPolicy::Auto,
                );
                let portable = highs_feasible_for_fixes(
                    &model,
                    &[(x1, a as f64), (x2, b as f64), (y, yv as f64)],
                    CompilationPolicy::Portable,
                );
                if auto != semantic || portable != semantic {
                    mismatches.push((a, b, yv, semantic, auto, portable));
                }
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "exact-max HiGHS feasible set differs from semantic: {mismatches:?}"
    );

    // Exact min mirror.
    let mut model = Model::new();
    let x1 = model.add_variable(binary()).unwrap();
    let x2 = model.add_variable(binary()).unwrap();
    let (_, y) = model
        .add_minmax(
            vec![x1.into(), x2.into()],
            MinMaxSense::Min,
            MinMaxRelation::Exact,
            None,
        )
        .unwrap();

    let mut mismatches = Vec::new();
    for a in 0..2 {
        for b in 0..2 {
            for yv in 0..2 {
                let semantic = (yv as f64) == (a.min(b) as f64);
                let auto = highs_feasible_for_fixes(
                    &model,
                    &[(x1, a as f64), (x2, b as f64), (y, yv as f64)],
                    CompilationPolicy::Auto,
                );
                let portable = highs_feasible_for_fixes(
                    &model,
                    &[(x1, a as f64), (x2, b as f64), (y, yv as f64)],
                    CompilationPolicy::Portable,
                );
                if auto != semantic || portable != semantic {
                    mismatches.push((a, b, yv, semantic, auto, portable));
                }
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "exact-min HiGHS feasible set differs from semantic: {mismatches:?}"
    );
}

#[test]
fn absolute_value_highs_feasible_sets_match_semantic() {
    // Exact abs over a bounded integer domain: HiGHS finds the probe feasible
    // iff z == |x|.
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(-2.0, 2.0)).unwrap();
    let (_, z) = model
        .add_absolute_value(x.into(), AbsoluteValueVariant::Absolute, None)
        .unwrap();

    let mut mismatches = Vec::new();
    for xv in -2..=2 {
        for zv in -2..=2 {
            let semantic = (zv as f64) == (xv as f64).abs();
            let auto = highs_feasible_for_fixes(
                &model,
                &[(x, xv as f64), (z, zv as f64)],
                CompilationPolicy::Auto,
            );
            let portable = highs_feasible_for_fixes(
                &model,
                &[(x, xv as f64), (z, zv as f64)],
                CompilationPolicy::Portable,
            );
            if auto != semantic || portable != semantic {
                mismatches.push((xv, zv, semantic, auto, portable));
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "exact abs HiGHS feasible set differs from semantic: {mismatches:?}"
    );

    // Clamp over a bounded domain: HiGHS finds the probe feasible iff
    // z == clamp(x, 1, 3).
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(-3.0, 5.0)).unwrap();
    let (_, z) = model
        .add_absolute_value(
            x.into(),
            AbsoluteValueVariant::Clamp {
                lower: 1.0,
                upper: 3.0,
            },
            None,
        )
        .unwrap();

    let mut mismatches = Vec::new();
    for xv in -3..=5 {
        for zv in 0..=4 {
            let semantic = (zv as f64) == (xv as f64).clamp(1.0, 3.0);
            let auto = highs_feasible_for_fixes(
                &model,
                &[(x, xv as f64), (z, zv as f64)],
                CompilationPolicy::Auto,
            );
            let portable = highs_feasible_for_fixes(
                &model,
                &[(x, xv as f64), (z, zv as f64)],
                CompilationPolicy::Portable,
            );
            if auto != semantic || portable != semantic {
                mismatches.push((xv, zv, semantic, auto, portable));
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "clamp HiGHS feasible set differs from semantic: {mismatches:?}"
    );
}

#[test]
fn binary_product_highs_feasible_sets_match_semantic() {
    // Binary-binary: HiGHS finds the probe feasible iff w == a·b.
    let mut model = Model::new();
    let a = model.add_variable(binary()).unwrap();
    let b = model.add_variable(binary()).unwrap();
    let (_, w) = model
        .add_binary_product(ProductOperand::Binary(a), ProductOperand::Binary(b), None)
        .unwrap();

    let mut mismatches = Vec::new();
    for av in 0..2 {
        for bv in 0..2 {
            for wv in 0..2 {
                let semantic = (wv as f64) == ((av * bv) as f64);
                let auto = highs_feasible_for_fixes(
                    &model,
                    &[(a, av as f64), (b, bv as f64), (w, wv as f64)],
                    CompilationPolicy::Auto,
                );
                let portable = highs_feasible_for_fixes(
                    &model,
                    &[(a, av as f64), (b, bv as f64), (w, wv as f64)],
                    CompilationPolicy::Portable,
                );
                if auto != semantic || portable != semantic {
                    mismatches.push((av, bv, wv, semantic, auto, portable));
                }
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "binary-binary HiGHS feasible set differs from semantic: {mismatches:?}"
    );

    // Binary-times-bounded-linear: HiGHS finds the probe feasible iff w == b·f.
    let mut model = Model::new();
    let b = model.add_variable(binary()).unwrap();
    let f = model.add_variable(continuous().bounds(-2.0, 2.0)).unwrap();
    let (_, w) = model.add_binary_times_linear(b, f.into(), None).unwrap();

    let mut mismatches = Vec::new();
    for bv in 0..2 {
        for fv in -2..=2 {
            for wv in -2..=2 {
                let semantic = (wv as f64) == (bv as f64 * fv as f64);
                let auto = highs_feasible_for_fixes(
                    &model,
                    &[(b, bv as f64), (f, fv as f64), (w, wv as f64)],
                    CompilationPolicy::Auto,
                );
                let portable = highs_feasible_for_fixes(
                    &model,
                    &[(b, bv as f64), (f, fv as f64), (w, wv as f64)],
                    CompilationPolicy::Portable,
                );
                if auto != semantic || portable != semantic {
                    mismatches.push((bv, fv, wv, semantic, auto, portable));
                }
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "binary-times-linear HiGHS feasible set differs from semantic: {mismatches:?}"
    );
}
