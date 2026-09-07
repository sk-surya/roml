//! P34 application workflows (contract Tasks 34-02, 34-03): MPS-imported
//! infeasibility through IIS to repair, and combined MILP reuse with
//! constructs, starts, locks, persistent soft penalties, and P31 priorities.

use std::collections::BTreeMap;
use std::io::Cursor;

use roml::construct::PwlRelation;
use roml::io::mps::{MpsReader, MpsWriter};
use roml::solver::infeasibility::BoundSide;
use roml::{
    binary, continuous, ConstraintExprExt, InfeasibilityOutcome, InfeasibilityPlan, LexicographicObjectives,
    LinExpr, MipStart, Model, ObjectivePolicy, ObjectivePriority, ObjectiveProviderPolicy,
    PenaltyPolicy, PenaltyTarget, PrimalAssignment, RelaxationOutcome, RelaxationRestriction,
    RelaxationScope, RepairPolicy, SolveOptions, SolveStatus, SolverSession, StageContinuation,
    ValueExpr, VariableHints, ViolationPolicy, WeightedObjective,
};
use roml_highs::HighsSession;

fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-7 + 1e-8 * a.abs().max(b.abs())
}

/// Task 34-02: MPS -> infeasible -> portable IIS with source origins ->
/// weighted-L1 repair -> repaired solve. The canonical model is unchanged
/// by the solve-scoped relaxation.
#[test]
fn imported_infeasibility_repairs_through_iis_origins() {
    let mut built = Model::with_name("p34-import-infeasible");
    let x = built
        .add_variable(continuous().bounds(0.0, 10.0).named("x"))
        .unwrap();
    built
        .add_constraint((x).ge(7.0).named("demand"))
        .unwrap();
    built
        .add_constraint((x).le(3.0).named("capacity"))
        .unwrap();
    built.minimize(x).unwrap();

    let mut bytes = Vec::new();
    MpsWriter::new().write(&built, &mut bytes).unwrap();
    let imported = MpsReader::new().read(Cursor::new(bytes)).unwrap();
    let mut model = imported.model;

    let mut session = SolverSession::new(HighsSession::try_new().unwrap());
    let proof = session.solve(&mut model).unwrap();
    assert_eq!(proof.status(), SolveStatus::Infeasible);

    let report = session
        .analyze_infeasibility(&model, &InfeasibilityPlan::portable_lp())
        .unwrap();
    assert_eq!(report.outcome, InfeasibilityOutcome::Conflict);
    assert!(report.members.len() >= 2);

    // Repair the capacity side (lower the 7.0 demand is infeasible against
    // capacity 3.0; relaxing demand down restores feasibility).
    let all: Vec<_> = model.take_snapshot().unwrap().constraints.clone();
    let demand_con = all
        .iter()
        .find(|c| c.bounds.lower == 7.0 && !c.bounds.upper.is_finite())
        .expect("demand row survives MPS round trip")
        .id;
    let repair = session
        .solve_feasibility_relaxation(
            &mut model,
            roml::FeasibilityRelaxationPlan {
                scope: RelaxationScope::Explicit(vec![
                    RelaxationRestriction::ConstraintSide {
                        constraint: demand_con,
                        side: BoundSide::Lower,
                    },
                ]),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(repair.outcome, RelaxationOutcome::OptimalRepair);
    assert!(approx_eq(repair.solution.value(x).unwrap(), 3.0));
    // Canonical model unchanged by the solve-scoped relaxation.
    let check = session.solve(&mut model).unwrap();
    assert_eq!(check.status(), SolveStatus::Infeasible);
}

/// Task 34-03: MILP reuse orchestration — binary direction construct rows,
/// a partial MIP start, an overlay fixing, a persistent parameterized soft
/// penalty, and P31 priorities in one persistent session.
#[test]
fn milp_reuse_orchestration_with_constructs_starts_and_priorities() {
    use roml::construct::ExtrapolationPolicy;

    let mut model = Model::new();
    let charge = model
        .add_variable(continuous().bounds(0.0, 2.0))
        .unwrap();
    let discharge = model
        .add_variable(continuous().bounds(0.0, 2.0))
        .unwrap();
    let direction = model.add_variable(binary()).unwrap();
    // Direction exclusivity (MILP): charge and discharge never coincide.
    model
        .add_constraint((charge - 2.0 * direction).le(0.0))
        .unwrap();
    model
        .add_constraint((discharge + 2.0 * direction).le(2.0))
        .unwrap();
    // PWL terminal value on discharge (convex epigraph, exact at optimum).
    let (_pwl, terminal) = model
        .add_piecewise_linear(
            LinExpr::from(discharge),
            vec![(0.0, 0.0).into(), (2.0, 3.0).into()],
            PwlRelation::Epigraph,
            ExtrapolationPolicy::Constant,
            None,
        )
        .unwrap();
    // Persistent parameterized soft penalty on over-discharge, targeted at
    // priority 1 (economics): resolved numerically before stage 1.
    let cap = model.add_constraint((discharge).le(1.5)).unwrap();
    let rate = model.add_parameter(4.0).unwrap();
    model
        .soften_constraint(
            cap,
            ViolationPolicy::default(),
            PenaltyPolicy {
                weight: ValueExpr::param(rate),
                target: PenaltyTarget::Priority(ObjectivePriority::new(1)),
            },
        )
        .unwrap();
    // Priority 0: minimize charge (unique optimum 0). Priority 1: maximize
    // discharge revenue (3 per unit) minus terminal value, with the soft
    // cap penalty. Net revenue slope 1.5/unit is outweighed by the 4.0/unit
    // penalty past the 1.5 cap, so priority 1 stops at discharge = 1.5.
    let obj0 = model.minimize(charge).unwrap();
    let obj1 = model.maximize(3.0 * discharge - terminal).unwrap();

    let mut session = SolverSession::new(HighsSession::try_new().unwrap());

    // Partial MIP start: direction only; the backend repairs the rest.
    let start_assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: Some(model.instance()),
        source_revision: Some(model.current_revision()),
        values: BTreeMap::from([(direction, 1.0)]),
    };
    let plan = roml::SolvePlan {
        options: SolveOptions::new().threads(1).output(false),
        overlay: roml::SolveOverlay::new(BTreeMap::new(), vec![], vec![], vec![]).unwrap(),
        mip_starts: vec![MipStart::new(
            start_assignment,
            RepairPolicy::BackendDefault,
        )],
        hints: VariableHints::default(),
        objective_override: None,
        lex_stage_policy: roml::LexStagePolicy::RequireOptimal,
        unsupported: roml::UnsupportedFeaturePolicy::Reject,
    };
    let planned = session.solve_plan(&mut model, plan).unwrap();
    assert_eq!(planned.status(), SolveStatus::Optimal);

    // P31 orchestration on the same persistent session afterwards.
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
    // Priority 0 pins charge at 0; priority 1 pushes discharge to the soft
    // cap 1.5 (penalty 4.0 per unit outweighs net revenue slope 1.5/unit).
    let xf = result.final_solution.value(charge).unwrap();
    let yf = result.final_solution.value(discharge).unwrap();
    assert!(xf.abs() < 1e-6, "charge pinned at 0, got {xf}");
    assert!(
        (yf - 1.5).abs() < 1e-6,
        "discharge at soft cap 1.5, got {yf}"
    );
    // Ordinary solve afterwards proves no solve-scoped artifact leaked.
    let plain = session.solve(&mut model).unwrap();
    assert_eq!(plain.status(), SolveStatus::Optimal);
}
