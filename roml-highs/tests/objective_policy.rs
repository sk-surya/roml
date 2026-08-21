//! P31 — portable weighted/lexicographic objective execution against the
//! production HiGHS backend (SM-11.3, SM-11.4, SM-11.5, SM-11.7).
//!
//! HiGHS does not natively accept `CompiledObjectivePolicy::Weighted` /
//! `Lexicographic` policy ops, so the portable executor reduces each weighted
//! stage to a single normalized minimization scalar and applies exact
//! degradation-lock rows as solve-scoped overlays (Task 31-08 portable path).

use roml::{
    ConstraintExprExt, Model, MultiObjectiveResult, ObjectiveExecutionProvider, ObjectivePolicy,
    ObjectivePriority, ObjectiveProviderPolicy, ObjectiveStageResult, SolverSession,
    StageContinuation, StageContinuationDecision, WeightedObjective, WeightedObjectives,
};
use roml_highs::HighsSession;

/// Build `minimize x` and `maximize y` over `x,y in [0,10]` with `x+y <= 10`.
fn two_objective_model() -> (Model, roml::VarId, roml::VarId, roml::ObjId, roml::ObjId) {
    let mut model = Model::new();
    let x = model
        .add_variable(roml::continuous().bounds(0.0, 10.0))
        .unwrap();
    let y = model
        .add_variable(roml::continuous().bounds(0.0, 10.0))
        .unwrap();
    model.add_constraint((x + y).le(10.0)).unwrap();
    let obj1 = model.minimize(x).unwrap();
    let obj2 = model.maximize(y).unwrap();
    (model, x, y, obj1, obj2)
}

/// A single weighted stage executes to an Optimal portable solve.
#[test]
fn weighted_policy_solves_portably_on_highs() {
    let (mut model, _x, _y, obj1, obj2) = two_objective_model();
    let mut session = SolverSession::new(HighsSession::try_new().expect("HiGHS available"));

    // Normalized scalar = +1*min(x) - 2*max(y) => minimize x - 2y.
    let objectives = vec![
        WeightedObjective {
            objective: obj1,
            weight: 1.0,
        },
        WeightedObjective {
            objective: obj2,
            weight: 2.0,
        },
    ];
    let policy = ObjectivePolicy::Weighted(WeightedObjectives { objectives });
    let result = session
        .solve_objective_policy(
            &mut model,
            policy,
            ObjectiveProviderPolicy::PortableOnly,
            StageContinuation::RequireOptimal,
        )
        .expect("weighted portable solve must succeed");

    assert_eq!(result.stages.len(), 1);
    assert_eq!(
        result.stages[0].continuation,
        StageContinuationDecision::ContinueOptimal
    );
    assert_eq!(
        result.provider,
        ObjectiveExecutionProvider::PortableSequential
    );
}

/// A two-level lexicographic policy runs both stages in priority order.
#[test]
fn lexicographic_policy_runs_stages_in_priority_order() {
    let (mut model, _x, _y, obj1, obj2) = two_objective_model();
    let mut session = SolverSession::new(HighsSession::try_new().expect("HiGHS available"));

    let level0 = roml::WeightedObjectiveLevel {
        priority: ObjectivePriority::new(0),
        objectives: vec![WeightedObjective {
            objective: obj1,
            weight: 1.0,
        }],
        absolute_tolerance: 1e-6,
        relative_tolerance: 1e-9,
    };
    let level1 = roml::WeightedObjectiveLevel {
        priority: ObjectivePriority::new(1),
        objectives: vec![WeightedObjective {
            objective: obj2,
            weight: 1.0,
        }],
        absolute_tolerance: 1e-6,
        relative_tolerance: 1e-9,
    };
    let policy = ObjectivePolicy::Lexicographic(roml::LexicographicObjectives {
        levels: vec![level0, level1],
    });
    let result = session
        .solve_objective_policy(
            &mut model,
            policy,
            ObjectiveProviderPolicy::PortableOnly,
            StageContinuation::RequireOptimal,
        )
        .expect("lexicographic portable solve must succeed");

    assert_eq!(result.stages.len(), 2);
    assert_eq!(result.stages[0].priority, ObjectivePriority::new(0));
    assert_eq!(result.stages[1].priority, ObjectivePriority::new(1));
    // Both stages prove optimality and descend.
    assert!(result.stages.iter().all(|s: &ObjectiveStageResult| {
        s.continuation == StageContinuationDecision::ContinueOptimal
    }));
    // Task 31-06: the last (final-point) stage exposes BOTH distinct canonical
    // objectives evaluated at the final solution, not only its own objective.
    let last = result.stages.last().unwrap();
    let ids: Vec<_> = last.objective_values.iter().map(|v| v.objective).collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&obj1));
    assert!(ids.contains(&obj2));
    let _ = MultiObjectiveResult {
        final_solution: result.final_solution.clone(),
        stages: result.stages.clone(),
        provider: result.provider.clone(),
    };
}

/// A priority-targeted P30 penalty is folded into the matching lexicographic
/// stage (Task 31-07; SM-10.6), and the live parameterized weight drives the
/// stage result.
#[test]
fn priority_target_penalty_folds_into_stage() {
    use roml::{ObjectivePriority, PenaltyPolicy, PenaltyTarget, ValueExpr, ViolationPolicy};

    let mut model = Model::new();
    let x = model
        .add_variable(roml::continuous().bounds(0.0, 10.0))
        .unwrap();
    // Soften `x >= 6` with a priority-0-targeted penalty weight of 5.
    let con = model.add_constraint((x).ge(6.0)).unwrap();
    model
        .soften_constraint(
            con,
            ViolationPolicy {
                max_violation: None,
            },
            PenaltyPolicy {
                weight: ValueExpr::constant(5.0),
                target: PenaltyTarget::Priority(ObjectivePriority::new(0)),
            },
        )
        .expect("softening a priority-targeted constraint must succeed");
    let obj = model.minimize(x).unwrap();

    let mut session = SolverSession::new(HighsSession::try_new().expect("HiGHS available"));
    let policy = ObjectivePolicy::Lexicographic(roml::LexicographicObjectives {
        levels: vec![roml::WeightedObjectiveLevel {
            priority: ObjectivePriority::new(0),
            objectives: vec![WeightedObjective {
                objective: obj,
                weight: 1.0,
            }],
            absolute_tolerance: 1e-6,
            relative_tolerance: 1e-9,
        }],
    });
    let result = session
        .solve_objective_policy(
            &mut model,
            policy,
            ObjectiveProviderPolicy::PortableOnly,
            StageContinuation::RequireOptimal,
        )
        .expect("priority-penalty portable solve must succeed");

    assert_eq!(result.stages.len(), 1);
    // Without folding, minimizing x would land at 0; the weight-5 penalty on
    // violating x >= 6 forces the stage to x = 6.
    assert_eq!(result.final_solution.value(x), Some(6.0));
}

/// When no qualified native provider exists, `PreferNative` must fall back to
/// the portable path rather than erroring or returning a native provider
/// (Task 31-08). HiGHS has no qualified native lexicographic path in the
/// normalized `|z*|` contract, so the observable provider is PortableSequential.
#[test]
fn prefer_native_falls_back_to_portable_with_observable_provider() {
    let (mut model, _x, _y, obj1, _obj2) = two_objective_model();
    let mut session = SolverSession::new(HighsSession::try_new().expect("HiGHS available"));

    let policy = ObjectivePolicy::Weighted(WeightedObjectives {
        objectives: vec![WeightedObjective {
            objective: obj1,
            weight: 1.0,
        }],
    });
    let result = session
        .solve_objective_policy(
            &mut model,
            policy,
            ObjectiveProviderPolicy::PreferNative,
            StageContinuation::RequireOptimal,
        )
        .expect("PreferNative must fall back to portable");
    assert_eq!(
        result.provider,
        ObjectiveExecutionProvider::PortableSequential
    );
    assert_eq!(result.stages.len(), 1);
    assert!(result.stages[0]
        .objective_values
        .iter()
        .any(|v| v.objective == obj1));
}

/// `NativeRequired` rejects before mutation when no qualified native provider
/// is available (Task 31-08). HiGHS has no such provider, so this must not
/// mutate the canonical model or backend.
#[test]
fn native_required_rejects_on_highs_without_mutation() {
    let (mut model, _x, _y, obj1, _obj2) = two_objective_model();
    let mut session = SolverSession::new(HighsSession::try_new().expect("HiGHS available"));

    let policy = ObjectivePolicy::Weighted(WeightedObjectives {
        objectives: vec![WeightedObjective {
            objective: obj1,
            weight: 1.0,
        }],
    });
    let result = session.solve_objective_policy(
        &mut model,
        policy,
        ObjectiveProviderPolicy::NativeRequired,
        StageContinuation::RequireOptimal,
    );
    assert!(result.is_err());
    // Canonical model remains usable by an ordinary solve afterwards.
    assert!(session.solve(&mut model).is_ok());
}

/// A priority-0 penalty must be part of the degradation lock for ALL later
/// stages. Priority 0 minimizes `x` with a priority-targeted penalty forcing
/// `x >= 6`; priority 1 also minimizes `x` (an incentive to drive it toward 0,
/// i.e. to reintroduce the violation). With zero tolerance, the later stage
/// must NOT be allowed to push `x` below 6.
///
/// This is the two-level real-HiGHS regression for the P1 "priority penalties
/// not in z*/degradation lock" defect: before the fix the lock contained only
/// the canonical objective (`x <= 6`), and priority 1 minimizing `x` would
/// drive it to 0, reintroducing the violation.
#[test]
fn two_level_priority_penalty_lock_prevents_reintroduction() {
    use roml::{ObjectivePriority, PenaltyPolicy, PenaltyTarget, ValueExpr, ViolationPolicy};

    let mut model = Model::new();
    let x = model
        .add_variable(roml::continuous().bounds(0.0, 10.0))
        .unwrap();
    // Soften `x >= 6` with a priority-0-targeted penalty weight of 5.
    let con = model.add_constraint((x).ge(6.0)).unwrap();
    model
        .soften_constraint(
            con,
            ViolationPolicy {
                max_violation: None,
            },
            PenaltyPolicy {
                weight: ValueExpr::constant(5.0),
                target: PenaltyTarget::Priority(ObjectivePriority::new(0)),
            },
        )
        .expect("softening a priority-targeted constraint must succeed");
    // Priority 1 also minimizes x: it wants the smallest x and therefore has a
    // direct incentive to reintroduce the priority-0 violation (x below 6).
    let obj0 = model.minimize(x).unwrap();
    // Same canonical objective at a different priority is legal (design §15).
    let obj1 = model.minimize(x).unwrap();

    let mut session = SolverSession::new(HighsSession::try_new().expect("HiGHS available"));
    let policy = ObjectivePolicy::Lexicographic(roml::LexicographicObjectives {
        levels: vec![
            roml::WeightedObjectiveLevel {
                priority: ObjectivePriority::new(0),
                objectives: vec![WeightedObjective {
                    objective: obj0,
                    weight: 1.0,
                }],
                // Zero tolerance: priority 0's scalar is locked exactly.
                absolute_tolerance: 0.0,
                relative_tolerance: 0.0,
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
    });
    let result = session
        .solve_objective_policy(
            &mut model,
            policy,
            ObjectiveProviderPolicy::PortableOnly,
            StageContinuation::RequireOptimal,
        )
        .expect("two-level priority-penalty lexicographic solve must succeed");

    assert_eq!(result.stages.len(), 2);
    // The priority-0 penalty lock must keep x pinned at 6; priority 1 cannot
    // reintroduce the violation.
    for stage in &result.stages {
        assert!(
            stage.continuation == StageContinuationDecision::ContinueOptimal,
            "stage {:?} not optimal: {stage:?}",
            stage.priority
        );
    }
    assert_eq!(result.final_solution.value(x), Some(6.0));
}

/// `z* = 0` with only a relative tolerance must allow ZERO degradation, so the
/// priority-0 feasibility-penalty lock is exact. Priority 0 carries only a
/// zero-weight objective so its canonical contribution is 0, plus a
/// priority-targeted penalty on `x <= 2` which is satisfiable in `[0,2]`,
/// giving a normalized `z* = 0 + 0 = 0`. Priority 1 then maximizes `x`
/// (incentive to re-violate by pushing `x > 2`). With relative-only tolerance
/// (`abs=0, rel>0`), `allowed_degradation = rel*|0| = 0`, so the lock is exact
/// and priority 1 must keep `x <= 2`.
#[test]
fn zero_star_relative_only_penalty_lock_is_exact() {
    use roml::{ObjectivePriority, PenaltyPolicy, PenaltyTarget, ValueExpr, ViolationPolicy};

    let mut model = Model::new();
    let x = model
        .add_variable(roml::continuous().bounds(0.0, 10.0))
        .unwrap();
    let con = model.add_constraint((x).le(2.0)).unwrap();
    model
        .soften_constraint(
            con,
            ViolationPolicy {
                max_violation: None,
            },
            PenaltyPolicy {
                weight: ValueExpr::constant(5.0),
                target: PenaltyTarget::Priority(ObjectivePriority::new(0)),
            },
        )
        .expect("softening a priority-targeted constraint must succeed");
    // Zero-weight objective: canonical contribution is 0, so z* = penalty
    // violation = 0 whenever `x <= 2` is satisfied.
    let obj0 = model.minimize(roml::LinExpr::new()).unwrap();
    let obj1 = model.maximize(x).unwrap();

    let mut session = SolverSession::new(HighsSession::try_new().expect("HiGHS available"));
    let policy = ObjectivePolicy::Lexicographic(roml::LexicographicObjectives {
        levels: vec![
            roml::WeightedObjectiveLevel {
                priority: ObjectivePriority::new(0),
                objectives: vec![WeightedObjective {
                    objective: obj0,
                    weight: 1.0,
                }],
                // z* = 0 here; RELATIVE-only tolerance must still allow no
                // degradation (rel * |0| = 0) and leave the lock exact.
                absolute_tolerance: 0.0,
                relative_tolerance: 0.5,
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
    });
    let result = session
        .solve_objective_policy(
            &mut model,
            policy,
            ObjectiveProviderPolicy::PortableOnly,
            StageContinuation::RequireOptimal,
        )
        .expect("zero-star relative-only penalty-lock solve must succeed");

    assert_eq!(result.stages.len(), 2);
    // z*=0 with relative-only tolerance allows no degradation: priority 1 must
    // not reintroduce the `x <= 2` violation by driving x above 2.
    assert!(result.final_solution.value(x).unwrap() <= 2.0 + 1e-6);
}

/// A P30 `PenaltyTarget::Objective` penalty is folded by the compiler directly
/// into the targeted canonical objective as a coefficient on a generated
/// soft-constraint violation variable. Those generated variables are NOT
/// exposed in `SolveSolution.variable_values`, so P31 must use the exact solved
/// objective value (which includes them) for the stage scalar and degradation
/// lock — never a primal recomputation that silently drops them.
///
/// Here weight w = 0.5 on `x >= 6` (targeting `obj0 = min x`) makes the
/// priority-0 minimum land at x = 0 with a NONZERO violation, so the true
/// penalized scalar is g = x + 0.5*max(0, 6-x) = 3.0 at x = 0. A primal
/// recomputation that skips the generated violation term would report
/// z* = 0, making the zero-tolerance lock `g <= 0` infeasible for the
/// stage-0 point and blocking stage 1. Priority 1 maximizes x (an incentive to
/// move off the penalized optimum), and the zero-tolerance lock must keep the
/// penalized scalar pinned at exactly 3.0 while both stages remain feasible.
#[test]
fn two_level_objective_target_penalty_lock_uses_exact_solved_scalar() {
    use roml::{ObjectivePriority, PenaltyPolicy, PenaltyTarget, ValueExpr, ViolationPolicy};

    let mut model = Model::new();
    let x = model
        .add_variable(roml::continuous().bounds(0.0, 10.0))
        .unwrap();
    // Soften `x >= 6` with a small weight (0.5) targeting the canonical
    // objective `obj0`. The penalty is small enough that the priority-0
    // minimum violates the constraint: minimizing `x + 0.5*max(0,6-x)` lands
    // at x=0 with g = 3.0.
    let con = model.add_constraint((x).ge(6.0)).unwrap();
    let obj0 = model.minimize(x).unwrap();
    model
        .soften_constraint(
            con,
            ViolationPolicy::default(),
            PenaltyPolicy {
                weight: ValueExpr::constant(0.5),
                target: PenaltyTarget::Objective(obj0),
            },
        )
        .expect("softening an objective-targeted constraint must succeed");
    // Priority 1 maximizes x: a direct incentive to move x away from the
    // priority-0 penalized optimum at x = 0 (raising the scalar toward 1.5x).
    let obj1 = model.maximize(x).unwrap();

    let mut session = SolverSession::new(HighsSession::try_new().expect("HiGHS available"));
    let policy = ObjectivePolicy::Lexicographic(roml::LexicographicObjectives {
        levels: vec![
            roml::WeightedObjectiveLevel {
                priority: ObjectivePriority::new(0),
                objectives: vec![WeightedObjective {
                    objective: obj0,
                    weight: 1.0,
                }],
                // Zero tolerance: priority 0's solved scalar is locked exactly.
                absolute_tolerance: 0.0,
                relative_tolerance: 0.0,
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
    });
    let result = session
        .solve_objective_policy(
            &mut model,
            policy,
            ObjectiveProviderPolicy::PortableOnly,
            StageContinuation::RequireOptimal,
        )
        .expect("objective-target penalty lexicographic solve must succeed");

    assert_eq!(result.stages.len(), 2, "stage 1 must remain feasible");
    let s0 = &result.stages[0];
    assert_eq!(s0.continuation, StageContinuationDecision::ContinueOptimal);
    // The locked scalar is the EXACT solved penalized value 3.0 (x=0 plus the
    // 0.5*6 = 3 violation term), not a primal value that drops the penalty.
    let z = s0.scalar_stage_value.expect("stage-0 scalar");
    assert!((z - 3.0).abs() < 1e-6, "expected z*=3.0, got {z}");
    let lock = s0.lock.expect("stage-0 lock");
    assert!((lock.reference_value - 3.0).abs() < 1e-6);
    // Both stages prove optimality and the zero-tolerance lock keeps the
    // penalized scalar preserved at x = 0.
    assert!(result
        .stages
        .iter()
        .all(|s| { s.continuation == StageContinuationDecision::ContinueOptimal }));
    let xf = result.final_solution.value(x).expect("final x");
    assert!(
        (xf - 0.0).abs() < 1e-6,
        "penalized scalar must be preserved at x=0, got {xf}"
    );
}
