# M2 Roadmap — Public API Ergonomics

## Dependency graph

```text
P20 Contract baseline and API characterization
  -> P21 Solver façade and unified result
       -> P22 Modeling ergonomics and names
            -> P23 Surface curation, validation, migration
                 -> P24 Documentation and consumer qualification
```

P21 is the critical path. P22 must target the accepted P21 solve/result contract. P23 must not remove or deprecate an API until its replacement has compile-pass coverage. P24 is the integration and release-evidence gate for this milestone, not a crates.io publication gate.

## Phase 20 — Contract baseline and API characterization

**Goal:** establish current behavior and freeze the M2 public contract before implementation.

**Deliverables:**

- exact current-main API inventory;
- compile characterization of documented examples;
- tests proving the current missing end-to-end solve path;
- golden-path API compile tests written against target signatures;
- explicit mapping from old entry points to replacements;
- performance and protocol baselines for repeated solve synchronization;
- approved M2 decisions and no unresolved naming/signature ambiguity.

**Gate P20:** every target public signature is written, every old API has a disposition, current documentation drift is captured by tests/evidence, and no backend contract change is required.

**Detailed plan:** `.planning/phases/20-public-api-contract/20-PLAN.md`

## Phase 21 — Solver façade and unified result

**Goal:** close the complete build-synchronize-solve-result loop without exposing protocol internals.

**Deliverables:**

- generic `SolverSession<B>` orchestration in core;
- controlled model access to revision batches and snapshots inside core;
- normalized `Solution`, `SolveStatus`, `SolveMetadata`, and `SolveError`;
- `roml_highs::Highs` façade;
- automatic commit/delta/rebuild synchronization;
- one-rebuild retry limit and stale-solution invalidation;
- repeated-solve and failure-recovery tests.

**Gate P21:** the target quickstart compiles and solves; parameter updates re-solve through one `Highs`; no user-side synchronization calls are required; all existing backend contract tests remain green.

**Detailed plan:** `.planning/phases/21-solver-facade/21-PLAN.md`

## Phase 22 — Modeling ergonomics and entity names

**Goal:** establish one discoverable method-first modeling language over existing canonical semantics.

**Deliverables:**

- validated variable and parameter definition builders;
- semantic entity aliases;
- names for variables, parameters, constraints, and objectives;
- canonical `add_constraint`, `minimize`, and `maximize` workflows;
- clear sparse cell update operations;
- model formatting and diagnostic name lookup;
- representative LP, MILP, sparse, and parameterized compile tests.

**Gate P22:** all ordinary examples use one consistent style; names survive model lifecycle operations; builders reject invalid input atomically; no canonical-cell or expression behavior regresses.

**Detailed plan:** `.planning/phases/22-modeling-ergonomics/22-PLAN.md`

## Phase 23 — Surface curation, validation, and migration

**Goal:** reduce cognitive load and make misuse explicit without stranding current users.

**Deliverables:**

- minimal prelude;
- advanced/backend namespace for protocol types;
- root export audit;
- release-mode validation consistency;
- deprecations for duplicate aliases and effectful macros;
- migration guide and changelog entries;
- public API baseline and semver review.

**Gate P23:** public inventory contains only intentional items; old-to-new migration is mechanical and documented; validation behavior is consistent; all replacements are tested before deprecations.

**Detailed plan:** `.planning/phases/23-surface-curation/23-PLAN.md`

## Phase 24 — Documentation and consumer qualification

**Goal:** prove a new user can install and use the intended API from packaged crates without repository knowledge.

**Deliverables:**

- rewritten README and modeling guide;
- working simple, MILP, incremental, solve-options, and sparse examples;
- rustdoc and doctest closure;
- fresh packed consumers for core and HiGHS;
- full CI, package, public API, and independent review evidence;
- final M2 evidence report.

**Gate P24:** all API-01 through API-10 requirements are closed with evidence; fresh consumers pass; docs and code agree; no unresolved blocker remains.

**Detailed plan:** `.planning/phases/24-consumer-qualification/24-PLAN.md`

## Concurrency policy

- Maximum active implementation phases: one.
- Maximum active review/fix branch: one.
- Documentation inventory may occur during P21/P22, but published wording waits for accepted interfaces.
- Public API removals and docs rewrites are integration work and remain on the critical path after replacement APIs land.

## Program stop conditions

Stop and escalate when:

- automatic synchronization would require exposing mutable model internals to backend crates;
- a backend failure cannot be classified without weakening recovery semantics;
- objective constants cannot be proven to appear exactly once;
- unified status mapping loses a backend distinction required by the current contract;
- trait coherence or operator overload changes would silently alter existing model algebra;
- a compatibility shim requires permanent duplication of two contradictory semantics;
- packed-consumer tests expose a dependency or native-discovery issue outside M2 scope.