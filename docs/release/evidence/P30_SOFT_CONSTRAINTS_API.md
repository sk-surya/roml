# P30 Persistent Soft-Constraint API Evidence

## Candidate

- Execution base before implementation: `5fe8f5b0f60831b438364a73300b4306dfa6d195`
- Phase: P30 — soft constraints and feasibility relaxation
- Scope: Task 30-00 API characterization

## Frozen inventory

The first P30 slice exposes `SoftConstraint`, `ViolationPolicy`,
`PenaltyPolicy`, `PenaltyTarget::{None,Objective}`, `ViolationRole`, and
`ViolationSide`. `Model::soften_constraint` validates the original constraint,
caps, penalty parameters, weight value, and objective target before reserving a
construct identity. The result is represented by the existing canonical
construct arena and ordinary `ConstructAdded` delta operation.

The role contract is stable and source-aware: both lower and upper roles retain
the owning construct identity and the original constraint handle. A later
compiler emits only finite sides, while the semantic roles remain available for
solution and P29 reporting.

## SM-10.6 ownership split

P30 owns only `PenaltyTarget::{None,Objective}` and portable weighted-L1
repair. P31 owns `PenaltyTarget::Priority`, the shared `ObjectivePriority`,
canonical objective policy, and lexicographic execution. No priority field,
second priority type, or generic unsupported-provider escape hatch is added.

## Verification

```text
cargo test -p roml --test soft_constraints_contract -- --nocapture
```

Result at task completion: 2 passed, 0 failed.

