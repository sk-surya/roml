//! Portable weighted-L1 feasibility repair with the HiGHS backend.

use roml::{
    continuous, BoundSide, ConstraintExprExt, FeasibilityRelaxationPlan, Model,
    RelaxationAcceptance, RelaxationOutcome, RelaxationRestriction, RelaxationScope, SolverSession,
};
use roml_highs::HighsSession;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::named("feasibility-repair");
    let x = model.add_variable(continuous().bounds(0.0, 10.0).named("x"))?;

    // Persistent softening is canonical model state and is separate from the
    // temporary feasibility-repair overlay below.
    let service = model.add_constraint(x.ge(2.0).named("service"))?;
    let soft = model.soften_constraint(service, Default::default(), Default::default())?;
    assert_eq!(soft.original_constraint(), service);

    let lower = model.add_constraint(x.ge(5.0).named("minimum"))?;
    let upper = model.add_constraint(x.le(3.0).named("capacity"))?;
    model.commit()?;
    let revision = model.current_revision();

    // P30's repair API is an advanced SolverSession workflow. It emits a
    // portable weighted-L1 overlay and rolls it back before returning.
    let mut session = SolverSession::new(HighsSession::try_new()?);
    let report = session.solve_feasibility_relaxation(
        &mut model,
        FeasibilityRelaxationPlan {
            scope: RelaxationScope::Explicit(vec![
                RelaxationRestriction::ConstraintSide {
                    constraint: lower,
                    side: BoundSide::Lower,
                },
                RelaxationRestriction::ConstraintSide {
                    constraint: upper,
                    side: BoundSide::Upper,
                },
            ]),
            acceptance: RelaxationAcceptance::RequireOptimal,
            ..Default::default()
        },
    )?;

    assert!(matches!(
        report.outcome,
        RelaxationOutcome::OptimalRepair
            | RelaxationOutcome::FeasibleRepair
            | RelaxationOutcome::NoRepairFound
            | RelaxationOutcome::Unknown(_)
    ));
    assert_eq!(model.current_revision(), revision);

    println!("outcome: {:?}", report.outcome);
    println!("provider: {:?}", report.metadata.provider);
    println!("weighted violation: {}", report.total_weighted_violation);
    for member in report.members {
        println!("  {:?}: violation={}", member.restriction, member.violation);
    }

    Ok(())
}
