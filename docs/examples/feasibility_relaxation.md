# Portable feasibility relaxation

Persistent softening and feasibility repair are separate operations. Attach a
`SoftConstraint` when the model's canonical semantics should retain a
violation role across revisions:

```rust
use roml::{
    continuous, ConstraintExprExt, Model, PenaltyPolicy, PenaltyTarget, ValueExpr,
    ViolationPolicy,
};

let mut model = Model::named("repair");
let x = model.add_variable(continuous().bounds(0.0, 10.0).named("x"))?;
let demand = model.add_constraint(x.ge(2.0).named("demand"))?;
let soft = model.soften_constraint(
    demand,
    ViolationPolicy { max_violation: Some(10.0) },
    PenaltyPolicy {
        weight: ValueExpr::constant(4.0),
        target: PenaltyTarget::None,
    },
)?;
assert_eq!(soft.original_constraint(), demand);
```

For one isolated repair attempt, use an advanced solver session's portable
weighted-L1 workflow. It adds no persistent constructs and does not advance the
canonical revision:

```rust
use roml::{
    BoundSide, FeasibilityRelaxationPlan, RelaxationOutcome, RelaxationRestriction,
    RelaxationScope, SolverSession,
};
use roml_highs::HighsSession;

let lower = model.add_constraint(x.ge(5.0).named("minimum"))?;
let upper = model.add_constraint(x.le(3.0).named("capacity"))?;
model.commit()?;
let before = model.current_revision();
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
        ..Default::default()
    },
)?;
assert_eq!(model.current_revision(), before);
assert!(matches!(
    report.outcome,
    RelaxationOutcome::OptimalRepair
        | RelaxationOutcome::FeasibleRepair
        | RelaxationOutcome::NoRepairFound
        | RelaxationOutcome::Unknown(_)
));
println!(
    "provider: {:?}, base: {:?}",
    report.metadata.provider,
    report.metadata.base_compilation_id
);
```

`PortableOnly` is the default provider policy. `PreferNative` records a
portable fallback when no qualified native provider is available, while
`NativeRequired` rejects before synchronization. `AcceptFeasible` accepts a
valid feasible incumbent without an optimality proof; `RequireOptimal` reports
`Unknown` for an unproven feasible termination. IIS membership is diagnostic
scope input only; it does not claim minimum-cardinality or minimum-weight
repair. Objective-priority and lexicographic execution belong to P31.

The HiGHS showcase uses the public advanced `SolverSession<HighsSession>` path;
the compact `Highs` façade remains the recommended entry point for ordinary
solves.
