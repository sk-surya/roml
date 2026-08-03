# M3 Semantic Modeling and Solve Workflows Roadmap

## Dependency graph

```text
P25 Semantic IR, lineage, metadata
  |
  v
P26 Compiler boundary, backend IR, capabilities, origin maps
  |
  +-------------------------------+
  |                               |
  v                               v
P27 Fixing, assignments, locks    P32 Common semantic constructs
  |                               |
  v                               v
P28 Starts, hints, SolvePlan      P33 PWL and bound analysis
  |
  +------------+------------+
  |            |            |
  v            v            v
P29 IIS        P30 Soft     P31 Objective policies
  \            |            /
   +-----------+-----------+
               |
               v
P34 Qualification, migration, docs, NLP-readiness
```

P32 may begin after P26 while P27–P31 proceed, but only if the compiler contract is frozen and review capacity exists. P29, P30, and P31 may be implemented independently after P28; integration into P34 remains sequential.

---

## Phase 25 — Canonical semantic IR, lineage, and metadata

**Goal:** establish semantic canonical state before adding new workflows.

**Requirements:** SM-01, SM-02, foundations of SM-15.

**Deliverables:**

- `ModelLineageId` allocation and clone semantics;
- entity metadata store and accessors;
- `ScalarFunction::Linear` and `ScalarSet`;
- canonical `FunctionConstraint` representation behind existing constraint APIs;
- `ConstructId`, construct store, activity, metadata, and dependency hooks;
- semantic construct entries in snapshots/deltas without implementing concrete construct kinds beyond test fixtures;
- invariant checker updates;
- public/advanced surface disposition;
- migration characterization proving M2 ordinary API source compatibility.

**Gate:** existing linear models produce observationally identical snapshots/backend results; lineages reject cross-model artifacts; every canonical construct fixture survives clone, snapshot, delta, remove, and rebuild.

**Branch:** `phase-roml-P25-semantic-ir-foundation`

---

## Phase 26 — Compiler boundary, backend IR, capabilities, and origin maps

**Goal:** insert a deterministic semantic-to-backend compilation layer without regressing primitive incremental behavior.

**Requirements:** SM-03, SM-04, SM-13 foundations.

**Deliverables:**

- `BackendSnapshot`, `BackendDeltaBatch`, compiled IDs, normalized backend primitives;
- `CompilationSession`, policies, recipes, fingerprints, and reports;
- typed `BackendFeature` capability registry with limitations;
- mandatory `OriginMap` and completeness validator;
- identity compiler for primitive linear models;
- `SolverSession` orchestration updated to compile before backend synchronization;
- `BackendSession` migration to compiled synchronization;
- ReferenceBackend and HiGHS migration;
- conformance tests for compile-rebuild equivalence and primitive incremental deltas;
- backend-author migration guide and ADR amendment.

**Gate:** all M2 solve/recovery tests pass through backend IR; primitive random delta sequences equal compiled rebuild; no generated entity lacks origin; HiGHS and ReferenceBackend consume no mutable `Model` internals.

**Branch:** `phase-roml-P26-compiler-backend-ir`

---

## Phase 27 — Persistent fixing, primal assignments, locks, and overlays

**Goal:** support hard solution reuse while protecting canonical model history and persistent backend state.

**Requirements:** SM-05, SM-06, SM-07.3–SM-07.6.

**Deliverables:**

- declared versus effective variable domains;
- `VariableFixing`, provenance, `fix`, `unfix`, and validation;
- `PrimalAssignment` and `Solution::primal_assignment`;
- lock selectors and continuous lock bands;
- `SolveOverlay`, compiled overlay ops, IDs, and origin mapping;
- transactional overlay apply/rollback protocol;
- backend health transition to `RequiresRebuild` on uncertain rollback;
- failure-injection tests across apply, solve, extraction, and rollback;
- HiGHS temporary-bound implementation.

**Gate:** persistent fixings survive rebuild and unfix correctly; temporary locks never advance model revision and never leak into subsequent solves; injected failures preserve recoverability.

**Branch:** `phase-roml-P27-fixing-locks-overlays`

---

## Phase 28 — SolvePlan, MIP starts, hints, and effective-plan reporting

**Goal:** expose one explicit solve-attempt contract for options, overlays, starts, hints, and objective overrides.

**Requirements:** SM-07, SM-08.

**Deliverables:**

- `SolvePlan`, builders, validation, and convenience conversion from existing `SolveOptions`;
- `MipStart`, repair policy, multiple-start policy;
- `VariableHints` and priorities;
- explicit unsupported/conversion policy;
- backend optional traits or bounded methods for starts/hints;
- HiGHS start API derived from pinned official headers;
- runtime/version capability declaration;
- `EffectiveSolvePlan` in `SolveMetadata`;
- conformance tests proving starts/hints do not alter feasibility;
- compiled examples for partial start, integer lock, and unsupported hint behavior.

**Gate:** default rejection is explicit; all applied/converted/rejected features appear in metadata; HiGHS start behavior is qualified; existing `solve` and `solve_with` remain source-compatible.

**Branch:** `phase-roml-P28-solve-plan-warm-starts`

---

## Phase 29 — IIS/conflict analysis and origin-aware reports

**Goal:** provide accurate infeasibility diagnosis in original ROML terms.

**Requirements:** SM-09 plus SM-02.5–SM-02.6.

**Deliverables:**

- optional `InfeasibilityAnalysisSession` contract;
- normalized request/result/report types;
- conflict kinds, scopes, minimality/completion semantics;
- mapping of compiled row sides, bounds, fixings, locks, and construct roles;
- text, Markdown, and structured renderers;
- official-header audit for bundled and minimum system HiGHS versions;
- HiGHS native IIS implementation or explicit version-gated unsupported behavior;
- deterministic infeasible fixtures covering rows, bounds, fixing, overlay lock, and bridged construct;
- report golden tests without brittle backend-generated prose.

**Gate:** reports contain only valid origin references, state exact guarantees, and identify original names/provenance; unsupported versions fail before unsafe/native misuse.

**Branch:** `phase-roml-P29-iis-conflict-reports`

---

## Phase 30 — Soft constraints, slacks, penalties, and feasibility relaxation

**Goal:** make controlled constraint violation an ordinary modeling operation.

**Requirements:** SM-10.

**Deliverables:**

- soft-constraint construct and builder;
- generated lower/upper nonnegative violation variables with stable handles;
- correct upper/lower/equality/ranged algebra;
- bounded and unbounded-above violation policy with validation;
- parameter-dependent penalty weights;
- objective and lexicographic-priority penalty targets;
- signed-correction API with L1 positive/negative parts;
- violation accessors on `Solution`;
- separate backend feasibility-relaxation request/report contract;
- HiGHS native feasibility relaxation where official support is qualified;
- examples for demand shedding, capacity overage, and diagnostic no-penalty slack.

**Gate:** symbolic and numerical tests confirm violation algebra and objective sign; generated variables map to the soft construct; persistent softening and solve-scoped feasibility relaxation remain distinct.

**Branch:** `phase-roml-P30-soft-constraints`

---

## Phase 31 — Objective policies and lexicographic orchestration

**Goal:** support multi-criteria optimization through one semantic policy and equivalent native/portable execution.

**Requirements:** SM-11.

**Deliverables:**

- objective-policy store and canonical changes;
- single, weighted, and lexicographic builders;
- tolerance and stage-policy validation;
- native capability negotiation;
- sequential portable executor using overlay objective locks;
- exact degradation-bound formulas for minimize/maximize objectives;
- stage result and all-objective-value storage in `Solution`;
- HiGHS native path if official semantics match;
- differential corpus comparing native and portable paths;
- examples for service-first/cost-second and feasibility-first optimization.

**Gate:** native and portable paths agree within declared tolerances; failed/limited stages obey explicit continuation policy; temporary stage rows never persist.

**Branch:** `phase-roml-P31-lexicographic-objectives`

---

## Phase 32 — Common semantic modeling constructs

**Goal:** deliver a small, high-value library of exact MILP constructs over the frozen compiler contract.

**Requirements:** SM-12 and relevant SM-13 requirements.

**Deliverables:**

- indicator and one-way implication;
- reification with continuous separation/integrality proof;
- exact/epigraph/hypograph min/max;
- exact absolute value, positive part, and clamp;
- Boolean implication/equivalence/any/all;
- exactly/at-most/at-least cardinality;
- binary-binary and binary-times-bounded-linear products;
- native and portable bridge recipes;
- deterministic auxiliary naming/roles;
- randomized equivalence tests against explicit reference formulations;
- public examples and formulation reports.

**Gate:** every construct has one semantic definition, complete origin mapping, exact portable formulation, and explicit failure when bounds/native support are insufficient.

**Branch:** `phase-roml-P32-common-constructs`

---

## Phase 33 — Piecewise-linear functions and bound analysis

**Goal:** provide safe, convexity-aware PWL modeling and production-grade Big-M derivation evidence.

**Requirements:** SM-13, SM-14.

**Deliverables:**

- interval bound analyzer for linear scalar functions;
- bound-source trace and compile diagnostics;
- PWL point validation, continuity, convexity, concavity, and extrapolation policy;
- convex epigraph and concave hypograph row bridges without binaries;
- exact native PWL/SOS2/binary graph bridges;
- formulation-selection policy and per-construct override;
- randomized direct-evaluation and solver-equivalence tests;
- numerical scaling diagnostics for extreme slopes/breakpoints;
- examples for production cost, tiered penalty, ReLU/positive part, and nonconvex efficiency curve.

**Gate:** no unproven Big-M; convex one-sided formulations introduce zero binaries; exact/nonconvex formulations remain exact; all representation decisions appear in reports.

**Branch:** `phase-roml-P33-piecewise-linear-bounds`

---

## Phase 34 — Qualification, documentation, migration, and NLP-readiness audit

**Goal:** integrate and independently verify M3 as a coherent production/research capability set.

**Requirements:** SM-15 and all remaining requirements.

**Deliverables:**

- requirement traceability and evidence bundle;
- full core/HiGHS test matrix on supported OS/version modes;
- native versus portable formulation corpus;
- overlay/rebuild failure-injection matrix;
- public API and semver review;
- rustdoc, README, modeling guide, diagnostics guide, compiler/formulation guide, and migration guide;
- fresh packed-consumer projects for every golden workflow;
- package-content inspection;
- benchmark baselines for primitive incremental solves, compile/rebuild, construct compilation, and report generation;
- independent principal-engineer review;
- independent OR formulation review;
- NLP-readiness architecture review with explicit pass/fail findings;
- milestone state update; no publication.

**Performance gate:** P34 records current M2 baseline before implementation. For ordinary primitive parameter-update solves, median wall-clock overhead attributable to M3 orchestration must remain below 5% or 50 microseconds per solve attempt, whichever is larger, on the benchmark fixture. Any exception requires profiling evidence and owner approval.

**Gate:** all mandatory checks pass, no unresolved P0/P1 findings remain, all requirements are evidenced, public claims match qualified behavior, and the NLP-readiness review passes.

**Branch:** `phase-roml-P34-m3-qualification`

---

## Critical path

```text
P25 -> P26 -> P27 -> P28 -> integration of P29/P30/P31 -> P34
                 \-> P32 -> P33 ----------------------/
```

The actual bottleneck is P26 correctness and review. No downstream phase may work around or duplicate the compiler/origin/capability boundary.

## Program stop conditions

Stop and escalate when:

- a backend's official semantics contradict the generic feature contract;
- an exact bridge cannot be proven from declared bounds;
- a proposed convenience would silently change exactness;
- overlay rollback cannot be made safe;
- native IIS scope/minimality cannot be represented accurately;
- a compiler optimization weakens rebuild equivalence;
- an NLP extension would require replacing rather than extending the function/backend IR;
- review capacity is exceeded by parallel branches;
- any agent proposes publication as part of M3.
