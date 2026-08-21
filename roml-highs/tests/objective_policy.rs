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
