# M2 — Public API Ergonomics

**Branch:** `docs/public-api-ergonomics-gsd-ultra`  
**Baseline:** `main@ac473911bc2239e940b8c2019dee3e01a445701e`  
**Planning date:** 2026-08-02  
**Status:** planning complete; implementation not started

This packet is the authoritative execution plan for making ROML's external API intuitive, coherent, and complete. It does not replace the historical release-hardening documents; it narrows the next body of work to the user-facing modeling and solve experience on the current qualified core and HiGHS backend.

## Outcome

A new user can copy one short example, build a named LP or MILP, solve it with HiGHS, inspect a single solution type, update parameters, and re-solve incrementally without learning revisions, snapshots, delta batches, cursors, or backend synchronization.

Canonical target:

```rust
use roml::prelude::*;
use roml_highs::Highs;

let mut model = Model::named("production");
let x = model.add_variable(continuous().named("x"))?;
let y = model.add_variable(integer().bounds(0.0, 10.0).named("y"))?;

model.add_constraint((x + y).le(4.0).named("capacity"))?;
model.maximize(3.0 * x + y)?;

let mut highs = Highs::new()?;
let solution = highs.solve(&mut model)?;
assert!(solution.status().is_optimal());
```

## Packet index

1. `PROJECT.md` — objective, boundaries, product promise, acceptance criteria.
2. `REQUIREMENTS.md` — stable requirement IDs for implementation and review.
3. `ROADMAP.md` — phase dependency graph and gates.
4. `STATE.md` — current execution state and next gate.
5. `DECISIONS.md` — accepted API and architecture decisions.
6. `RESEARCH.md` — current-main findings and evidence.
7. `TRACEABILITY.md` — requirements mapped to phases and evidence.
8. `RISKS.md` — failure modes, mitigations, and reversal triggers.
9. `EXECUTION.md` — branch, testing, review, and integration protocol.
10. `.planning/phases/20-*` through `24-*` — implementation-ready phase plans.

## Execution rule

Execute phases in order. Phase 22 may begin only after Phase 21's public solve path is accepted. Do not start deprecation or documentation cleanup before the replacement API compiles end-to-end. Keep at most two implementation branches active: one coding branch and one review/fix branch.

## Stopping condition

M2 is complete when every requirement in `REQUIREMENTS.md` is closed with compiled examples, tests, public-API evidence, fresh-consumer verification, and independent review.