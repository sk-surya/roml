# Portable feasibility relaxation

Persistent softening and feasibility repair are separate operations. Attach a
`SoftConstraint` when the model's canonical semantics should retain a
violation role across revisions:

```rust
let soft = model.soften_constraint(
    demand,
    roml::ViolationPolicy { max_violation: Some(10.0) },
    roml::PenaltyPolicy {
        weight: roml::ValueExpr::constant(4.0),
        target: roml::PenaltyTarget::None,
    },
)?;
```

For one isolated repair attempt, use a solver session's portable weighted-L1
workflow. It does not add persistent constructs or advance the canonical
revision:

```rust
let before = model.current_revision();
let report = session.solve_feasibility_relaxation(
    &mut model,
    roml::FeasibilityRelaxationPlan {
        scope: roml::RelaxationScope::Explicit(vec![
            roml::RelaxationRestriction::ConstraintSide {
                constraint: demand,
                side: roml::solver::infeasibility::BoundSide::Upper,
            },
        ]),
        ..Default::default()
    },
)?;
assert_eq!(model.current_revision(), before);
assert!(matches!(
    report.outcome,
    roml::RelaxationOutcome::OptimalRepair
        | roml::RelaxationOutcome::FeasibleRepair
        | roml::RelaxationOutcome::NoRepairFound
        | roml::RelaxationOutcome::Unknown(_)
));
println!("provider: {:?}, base: {:?}", report.metadata.provider, report.metadata.base_compilation_id);
```

`PreferNative` records a portable fallback when no qualified native provider is
available. `NativeRequired` rejects before synchronization. IIS membership is
diagnostic scope input only; it does not claim minimum-cardinality or
minimum-weight repair. Objective-priority and lexicographic execution belong to
P31 and are intentionally absent here. Native feasibility-relaxation calls are
not claimed by this phase.
