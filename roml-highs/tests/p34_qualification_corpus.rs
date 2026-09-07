//! P34 native qualification corpus Q01–Q14 (contract §3).
//!
//! Each fixture is a frozen deterministic model with a hand-verified
//! mathematical result, solved through the full `SolverSession<HighsSession>`
//! stack (bundled HiGHS). Objective comparisons obey the frozen rule
//! `|a-b| <= 1e-7 + 1e-8*max(|a|,|b|)`; primal residuals `1e-7`;
//! integrality residuals exact-committed `1e-6`.
//! All fixtures use one solver thread and no output.

use std::collections::BTreeMap;
use std::io::Cursor;

use roml::construct::{
    AbsoluteValueVariant, IndicatorDirection, MinMaxRelation, MinMaxSense, PwlRelation,
};
use roml::io::mps::{MpsReader, MpsWriter};
use roml::solver::infeasibility::BoundSide;
use roml::{
    binary, continuous, integer, ConstraintExprExt, InfeasibilityOutcome, LexicographicObjectives,
    LinExpr, MipStart, Model, ObjectivePolicy, ObjectivePriority, ObjectiveProviderPolicy,
    Parameter, PrimalAssignment, RelaxationOutcome, RepairPolicy, SolveOptions, SolveStatus,
    SolverSession, StageContinuation, UnsupportedFeaturePolicy, ValueExpr, VariableHints,
    WeightedObjective,
};
use roml_highs::HighsSession;

fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-7 + 1e-8 * a.abs().max(b.abs())
}

fn highs_session() -> SolverSession<HighsSession> {
    SolverSession::new(HighsSession::try_new().expect("bundled HiGHS available"))
}

fn quiet_options() -> SolveOptions {
    SolveOptions::new()
        .threads(1)
        .output(false)
        .random_seed(20260907)
}

/// Q01: primitive parameter delta. `min (1+p)*x, x >= 2`, `p = 3` → 8.0;
/// `p = 5` → 12.0. Revision advances; both optima exact.
#[test]
fn q01_primitive_parameter_delta() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.add_constraint((x).ge(2.0)).unwrap();
    let obj = model.minimize(x).unwrap();
    let p: Parameter = model.add_parameter(3.0).unwrap();
    model
        .add_objective_coefficient(obj, x, ValueExpr::param(p))
        .unwrap();

    let mut session = highs_session();
    let rev0 = model.current_revision();
    let first = session.solve(&mut model).unwrap();
    assert_eq!(first.status(), SolveStatus::Optimal);
    assert!(approx_eq(first.objective_value().unwrap(), 8.0));
    assert!(approx_eq(first.value(x).unwrap(), 2.0));
    let rev1 = model.current_revision();
    assert!(rev1 != rev0);

    model.set_parameter(p, 5.0).unwrap();
    let second = session.solve(&mut model).unwrap();
    assert_eq!(second.status(), SolveStatus::Optimal);
    assert!(approx_eq(second.objective_value().unwrap(), 12.0));
    assert!(approx_eq(second.value(x).unwrap(), 2.0));
    assert!(model.current_revision() != rev1);
}

/// Q02: indicator with tight bounds. `z = 1 → x <= 2`, `max x`:
/// fixed `z = 1` → 2.0 (bound tight); `z = 0` → 5.0 (inactive).
#[test]
fn q02_indicator_tight_bounds() {
    for (z_fix, expected) in [(1.0, 2.0), (0.0, 5.0)] {
        let mut model = Model::new();
        let z = model.add_variable(binary()).unwrap();
        let x = model.add_variable(continuous().bounds(0.0, 5.0)).unwrap();
        model
            .add_indicator(z, IndicatorDirection::WhenOne, (x).le(2.0), None)
            .unwrap();
        model.add_constraint((z).eq(z_fix)).unwrap();
        model.maximize(x).unwrap();
        let mut session = highs_session();
        let solution = session.solve(&mut model).unwrap();
        assert_eq!(solution.status(), SolveStatus::Optimal);
        assert!(
            approx_eq(solution.objective_value().unwrap(), expected),
            "z = {z_fix}: expected {expected}"
        );
        assert!(approx_eq(solution.value(x).unwrap(), expected));
    }
}

/// Q03: exact minmax. `out == max(x, y)`, `x <= 3`, `y <= 7`,
/// `max out` → 7.0.
#[test]
fn q03_exact_minmax() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 3.0)).unwrap();
    let y = model.add_variable(continuous().bounds(0.0, 7.0)).unwrap();
    let (_construct, out) = model
        .add_minmax(
            vec![LinExpr::from(x), LinExpr::from(y)],
            MinMaxSense::Max,
            MinMaxRelation::Exact,
            None,
        )
        .unwrap();
    model.maximize(out).unwrap();
    let mut session = highs_session();
    let solution = session.solve(&mut model).unwrap();
    assert_eq!(solution.status(), SolveStatus::Optimal);
    assert!(approx_eq(solution.objective_value().unwrap(), 7.0));
    assert!(approx_eq(solution.value(y).unwrap(), 7.0));
}

/// Q04: absolute positive clamp. `out = clamp(x, 0, 4)`, `x <= 10`,
/// `max out` → 4.0.
#[test]
fn q04_absolute_positive_clamp() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let (_construct, out) = model
        .add_absolute_value(
            LinExpr::from(x),
            AbsoluteValueVariant::Clamp {
                lower: 0.0,
                upper: 4.0,
            },
            None,
        )
        .unwrap();
    model.maximize(out).unwrap();
    let mut session = highs_session();
    let solution = session.solve(&mut model).unwrap();
    assert_eq!(solution.status(), SolveStatus::Optimal);
    assert!(approx_eq(solution.objective_value().unwrap(), 4.0));
}

/// Q05: binary product. `out = b*x`, `b binary`, `x <= 5`,
/// `max out` → 5.0 at `b = 1`, `x = 5`.
#[test]
fn q05_binary_product() {
    let mut model = Model::new();
    let b = model.add_variable(binary()).unwrap();
    let x = model.add_variable(continuous().bounds(0.0, 5.0)).unwrap();
    let (_construct, out) = model
        .add_binary_times_linear(b, LinExpr::from(x), None)
        .unwrap();
    model.maximize(out).unwrap();
    let mut session = highs_session();
    let solution = session.solve(&mut model).unwrap();
    assert_eq!(solution.status(), SolveStatus::Optimal);
    assert!(approx_eq(solution.objective_value().unwrap(), 5.0));
    let bv = solution.value(b).unwrap();
    assert!((bv - 0.0).abs() < 1e-6 || (bv - 1.0).abs() < 1e-6);
    assert!(approx_eq(solution.value(x).unwrap(), 5.0));
}

/// Q06: convex PWL epigraph. `f = {(0,0),(2,1),(4,4)}` (slopes 0.5, 1.5:
/// convex), `out >= f(x)`, `x = 3` → `out = 2.5`.
#[test]
fn q06_pwl_convex_epigraph() {
    use roml::construct::ExtrapolationPolicy;

    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(3.0, 3.0)).unwrap();
    let (_construct, out) = model
        .add_piecewise_linear(
            LinExpr::from(x),
            vec![(0.0, 0.0).into(), (2.0, 1.0).into(), (4.0, 4.0).into()],
            PwlRelation::Epigraph,
            ExtrapolationPolicy::Constant,
            None,
        )
        .unwrap();
    model.minimize(out).unwrap();
    let mut session = highs_session();
    let solution = session.solve(&mut model).unwrap();
    assert_eq!(solution.status(), SolveStatus::Optimal);
    assert!(approx_eq(solution.objective_value().unwrap(), 2.5));
}

/// Q07: nonconvex PWL exact graph. `f = {(0,0),(1,1),(2,0)}`,
/// `out == f(x)`, `x = 0.5` → `out = 0.5`.
#[test]
fn q07_pwl_nonconvex_exact_graph() {
    use roml::construct::ExtrapolationPolicy;

    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.5, 0.5)).unwrap();
    let (_construct, out) = model
        .add_piecewise_linear(
            LinExpr::from(x),
            vec![(0.0, 0.0).into(), (1.0, 1.0).into(), (2.0, 0.0).into()],
            PwlRelation::ExactGraph,
            ExtrapolationPolicy::Constant,
            None,
        )
        .unwrap();
    model.minimize(out).unwrap();
    let mut session = highs_session();
    let solution = session.solve(&mut model).unwrap();
    assert_eq!(solution.status(), SolveStatus::Optimal);
    assert!(approx_eq(solution.objective_value().unwrap(), 0.5));
}

/// Q08: overlay fix lock. `min x, x >= 2` → 2.0; overlay fixing `x = 5`
/// solves to 5.0; the base is unchanged afterwards (2.0 again).
#[test]
fn q08_overlay_fix_lock() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.add_constraint((x).ge(2.0)).unwrap();
    let obj = model.minimize(x).unwrap();
    let mut session = highs_session();
    let base = session.solve(&mut model).unwrap();
    assert!(approx_eq(base.objective_value().unwrap(), 2.0));

    let overlay =
        roml::SolveOverlay::new(BTreeMap::from([(x, 5.0)]), vec![], vec![], vec![]).unwrap();
    let overlaid = session
        .solve_with_overlay(&mut model, SolveOptions::default(), &overlay, Some(obj))
        .unwrap();
    assert!(approx_eq(overlaid.objective_value().unwrap(), 5.0));

    let again = session.solve(&mut model).unwrap();
    assert!(approx_eq(again.objective_value().unwrap(), 2.0));
}

/// Q09: partial MIP start. Binary/integer model with a start assigning only
/// the integer variable; the plan solves optimally and records the start.
#[test]
fn q09_mip_start_partial() {
    let mut model = Model::new();
    let n = model.add_variable(integer().bounds(0.0, 4.0)).unwrap();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.add_constraint((x + n).le(6.0)).unwrap();
    model.maximize(x + 2.0 * n).unwrap();

    let assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: Some(model.instance()),
        source_revision: Some(model.current_revision()),
        values: BTreeMap::from([(n, 2.0)]),
    };
    let plan = roml::SolvePlan {
        options: quiet_options(),
        overlay: roml::SolveOverlay::new(BTreeMap::new(), vec![], vec![], vec![]).unwrap(),
        mip_starts: vec![MipStart::new(assignment, RepairPolicy::BackendDefault)],
        hints: VariableHints::default(),
        objective_override: None,
        lex_stage_policy: roml::LexStagePolicy::RequireOptimal,
        unsupported: UnsupportedFeaturePolicy::Reject,
    };
    let mut session = highs_session();
    let solution = session.solve_plan(&mut model, plan).unwrap();
    assert_eq!(solution.status(), SolveStatus::Optimal);
    // max x + 2n s.t. x + n <= 6, n <= 4 integer: n = 4, x = 2 → 10.0.
    assert!(approx_eq(solution.objective_value().unwrap(), 10.0));
    let nv = solution.value(n).unwrap();
    assert!((nv - nv.round()).abs() < 1e-6);
}

/// Q10: IIS row and bound. `x >= 1` and `x <= 0` → infeasible; the portable
/// report names both rows and proves irreducibility.
#[test]
fn q10_iis_row_and_bound() {
    use roml::{ConflictGuarantee, InfeasibilityPlan};

    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    model.add_constraint((x).ge(1.0)).unwrap();
    model.add_constraint((x).le(0.0)).unwrap();
    let mut session = highs_session();
    let report = session
        .analyze_infeasibility(&model, &InfeasibilityPlan::portable_lp())
        .unwrap();
    assert_eq!(report.outcome, InfeasibilityOutcome::Conflict);
    assert_eq!(report.guarantee, ConflictGuarantee::Irreducible);
    assert_eq!(report.members.len(), 2);
}

/// Q11: relaxation weighted L1. `x >= 5` softened against `x <= 3` with
/// weight 2: repair accepts with the declared weight and optimal outcome.
#[test]
fn q11_relaxation_weighted_l1() {
    use roml::{PenaltyPolicy, PenaltyTarget, RelaxationRestriction, RelaxationScope};

    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let constraint = model.add_constraint((x).ge(5.0)).unwrap();
    model.add_constraint((x).le(3.0)).unwrap();
    model.minimize(x).unwrap();
    model
        .soften_constraint(
            constraint,
            roml::ViolationPolicy::default(),
            PenaltyPolicy {
                weight: ValueExpr::constant(2.0),
                target: PenaltyTarget::None,
            },
        )
        .unwrap();
    let mut session = highs_session();
    let report = session
        .solve_feasibility_relaxation(
            &mut model,
            roml::FeasibilityRelaxationPlan {
                scope: RelaxationScope::Explicit(vec![RelaxationRestriction::ConstraintSide {
                    constraint,
                    side: BoundSide::Lower,
                }]),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(report.outcome, RelaxationOutcome::OptimalRepair);
    // Repair relaxes x >= 5 down to x >= 3 (violation 2 at weight 2):
    // the repaired primal is x = 3.0 and the repair objective is 4.0.
    assert!(approx_eq(report.total_weighted_violation, 4.0));
    assert!(approx_eq(report.solution.value(x).unwrap(), 3.0));
}

/// Q12: lexicographic mixed sense. Priority 0 `min x` (→ 0), priority 1
/// `max y` with `x + y <= 10` (→ 10): both stages optimal, exact locks.
#[test]
fn q12_lexicographic_mixed_sense() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let y = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.add_constraint((x + y).le(10.0)).unwrap();
    let obj0 = model.minimize(x).unwrap();
    let obj1 = model.maximize(y).unwrap();
    let mut session = highs_session();
    let result = session
        .solve_objective_policy(
            &mut model,
            ObjectivePolicy::Lexicographic(LexicographicObjectives {
                levels: vec![
                    roml::WeightedObjectiveLevel {
                        priority: ObjectivePriority::new(0),
                        objectives: vec![WeightedObjective {
                            objective: obj0,
                            weight: 1.0,
                        }],
                        absolute_tolerance: 1e-9,
                        relative_tolerance: 0.0,
                    },
                    roml::WeightedObjectiveLevel {
                        priority: ObjectivePriority::new(1),
                        objectives: vec![WeightedObjective {
                            objective: obj1,
                            weight: 1.0,
                        }],
                        absolute_tolerance: 1e-9,
                        relative_tolerance: 0.0,
                    },
                ],
            }),
            ObjectiveProviderPolicy::PortableOnly,
            StageContinuation::RequireOptimal,
        )
        .unwrap();
    assert_eq!(result.stages.len(), 2);
    assert!(approx_eq(result.stages[0].scalar_stage_value.unwrap(), 0.0));
    // Stage scalars are normalized minimization values: max(y) becomes -y.
    assert!(approx_eq(
        result.stages[1].scalar_stage_value.unwrap(),
        -10.0
    ));
    for stage in &result.stages {
        assert_eq!(
            stage.continuation,
            roml::StageContinuationDecision::ContinueOptimal
        );
        stage.lock.expect("each stage locks");
    }
    let last = result.stages.last().unwrap();
    let v0 = last
        .objective_values
        .iter()
        .find(|v| v.objective == obj0)
        .unwrap();
    let v1 = last
        .objective_values
        .iter()
        .find(|v| v.objective == obj1)
        .unwrap();
    assert!(approx_eq(v0.value, 0.0));
    assert!(approx_eq(v1.value, 10.0));
}

/// Q13: lexicographic zero optimum. `min x, x >= 0` → `z* = 0` with
/// relative-only tolerance: the lock is exact (`allowed_degradation = 0`).
#[test]
fn q13_lexicographic_zero_optimum() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.add_constraint((x).ge(0.0)).unwrap();
    let y = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let obj0 = model.minimize(x).unwrap();
    let obj1 = model.minimize(y).unwrap();
    let mut session = highs_session();
    let result = session
        .solve_objective_policy(
            &mut model,
            ObjectivePolicy::Lexicographic(LexicographicObjectives {
                levels: vec![
                    roml::WeightedObjectiveLevel {
                        priority: ObjectivePriority::new(0),
                        objectives: vec![WeightedObjective {
                            objective: obj0,
                            weight: 1.0,
                        }],
                        absolute_tolerance: 0.0,
                        relative_tolerance: 1e-6,
                    },
                    roml::WeightedObjectiveLevel {
                        priority: ObjectivePriority::new(1),
                        objectives: vec![WeightedObjective {
                            objective: obj1,
                            weight: 1.0,
                        }],
                        absolute_tolerance: 0.0,
                        relative_tolerance: 0.0,
                    },
                ],
            }),
            ObjectiveProviderPolicy::PortableOnly,
            StageContinuation::RequireOptimal,
        )
        .unwrap();
    assert_eq!(result.stages.len(), 2);
    let lock = result.stages[0].lock.expect("stage-0 lock");
    assert!(approx_eq(lock.reference_value, 0.0));
    assert!(approx_eq(lock.relative_scale, 0.0));
    assert!(approx_eq(lock.allowed_degradation, 0.0));
}

/// Q14: MPS parameterized snapshot. A parameterized model writes to MPS,
/// reads back, and solves to the same objective as the native model.
#[test]
fn q14_mps_parameterized_snapshot() {
    let mut model = Model::new();
    let x = model
        .add_variable(continuous().bounds(0.0, 10.0).named("x"))
        .unwrap();
    let y = model
        .add_variable(continuous().bounds(0.0, 10.0).named("y"))
        .unwrap();
    model
        .add_constraint((x + 2.0 * y).le(8.0).named("capacity"))
        .unwrap();
    let p: Parameter = model.add_parameter(3.0).unwrap();
    let obj = model.minimize(x).unwrap();
    model
        .add_objective_coefficient(obj, x, ValueExpr::param(p))
        .unwrap();
    // min (1+3)x = 4x s.t. capacity, x >= 0 → 0.0. Pin x >= 1 for a
    // nonzero frozen optimum: min 4x s.t. x in [1,10] → 4.0.
    model.add_constraint((x).ge(1.0).named("floor")).unwrap();

    let mut session = highs_session();
    let native = session.solve(&mut model).unwrap();
    assert!(approx_eq(native.objective_value().unwrap(), 4.0));

    let mut bytes = Vec::new();
    MpsWriter::new().write(&model, &mut bytes).unwrap();
    let imported = MpsReader::new()
        .read(Cursor::new(bytes))
        .expect("writer output is readable");
    let mut reread = imported.model;
    let mut session2 = highs_session();
    let round_tripped = session2.solve(&mut reread).unwrap();
    assert!(approx_eq(round_tripped.objective_value().unwrap(), 4.0));
    let _ = y;
}
