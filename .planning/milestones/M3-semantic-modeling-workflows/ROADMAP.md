# M3 Semantic Modeling and Solve Workflows Roadmap

> **Historical/superseded routing notice:** This original milestone roadmap is retained for the historical M3 phase contract. It is not an active routing or authorization authority. Current execution is governed by root `.planning/STATE.md`, milestone `STATE.md`, and `COMPLETION-ROADMAP.md`: `PR #45 merge -> P36 -> P30 -> P31 -> P34`. While PR #45 is unmerged, production authorization is false.

## Dependency graph

```text
P25 Semantic IR, lineage, instance identity, metadata
  |
  v
P26 Compiler boundary, backend IR, capabilities, origins, compilation identity
  |
  +-------------------------------+
  |                               |
  v                               v
P27 Fixing, locks, overlays       P32 Common semantic constructs
  |                               |
  v                               v
P28 Starts, hints, SolvePlan      P33 PWL and bound analysis
  |                                 |
  +------------+------------+       |
  |            |            |       |
  v            v            v       |
P29 IIS        P30 Soft     P31 Objective policies
  \            |            /       |
   +-----------+-----------+        |
               |                    |
               +--------------------+
                                    |
                                    v
P34 Qualification, migration, docs, NLP-readiness
```

Historical original dependency note (superseded): P32/P29/P30/P31 were once allowed to overlap after prerequisite phases. The completion program replaces that route with one serialized production path: P36 -> P30 -> P31 -> P34. Research/review may proceed ahead, but production implementation may not overlap.

---

## P25 — Canonical semantic IR, identities, and metadata

**Goal:** establish semantic canonical state before adding workflows.

**Requirements:** SM-01, SM-02, foundations of SM-15.

**Deliverables:**

- `ModelLineageId` and clone-preserved lineage;
- unique `ModelInstanceId` for every live model clone;
- entity metadata store/accessors while preserving existing name authority;
- `ScalarFunction::Linear`, `ScalarSet`, and `FunctionConstraint`;
- generation-safe construct arena with activity and derived dependencies;
- semantic snapshot/delta entries;
- invariant updates;
- M2 ordinary API characterization and public surface review.

**Gate:** existing linear models remain observationally equivalent; independent models reject cross-lineage assignments; clones share lineage but never instance identity; every construct fixture survives clone/snapshot/delta/activity/remove/rebuild.

**Branch:** `phase-roml-P25-semantic-ir-foundation`

---

## P26 — Compiler boundary, backend IR, capabilities, origins, and exact compilation identity

**Goal:** insert deterministic semantic compilation without regressing primitive incremental behavior.

**Requirements:** SM-03, SM-04, SM-13 foundations.

**Deliverables:**

- `BackendSnapshot`, `BackendDeltaBatch`, compiled IDs, normalized backend primitives, and compiled objective policy;
- unique checked `CompilationId` for exact backend state;
- non-authoritative deterministic recipe fingerprints;
- `CompilationSession`, policies, recipes, and reports;
- typed version-aware `BackendFeature` registry;
- mandatory `OriginMap` and completeness validator;
- identity compiler for primitive linear models;
- compiled synchronization contract;
- ReferenceBackend then HiGHS migration;
- exact from/to compilation IDs on deltas;
- coefficient-removal and objective-policy operations;
- differential compile/rebuild and stale-state tests;
- backend-author migration guide.

**Gate:** M2 solve/recovery passes through backend IR; primitive random deltas equal rebuild; divergent clones with equal revision cannot share exact compiled artifacts; hashes are never accepted as exact authority; all generated entities have origins; backends consume no mutable model internals.

**Branch:** `phase-roml-P26-compiler-backend-ir`

---

## P27 — Persistent fixing, assignments, locks, and overlays

**Goal:** support hard solution reuse while protecting canonical history and backend state.

**Requirements:** SM-05, SM-06, SM-07.3–SM-07.6.

**Deliverables:**

- declared/effective domains and first-class fixing;
- `PrimalAssignment` with lineage compatibility and instance/revision provenance;
- solution-lock selectors and continuous bands;
- `SolveOverlay`, compiled overlay operations, IDs, and origins;
- overlay compilation against exact `CompilationId`;
- explicit transactional apply/rollback receipts;
- `RequiresRebuild` on rollback uncertainty;
- failure injection across apply/solve/extraction/rollback;
- HiGHS temporary bounds/rows.

**Gate:** fix/unfix survives rebuild; locks never advance model revision; exact compilation mismatches reject before mutation; no overlay leaks after any injected failure.

**Branch:** `phase-roml-P27-fixing-locks-overlays`

---

## P28 — SolvePlan, starts, hints, and effective-plan reporting

**Goal:** expose one explicit solve-attempt contract.

**Requirements:** SM-07.1–SM-07.2, SM-07.7, SM-08.

**Deliverables:**

- `SolvePlan` over options, overlay, starts, hints, objective override, continuation, and unsupported policy;
- `MipStart`, repair and multiple-start policy;
- `VariableHints` and priorities;
- explicit conversion policy;
- optional backend start/hint support;
- official pinned HiGHS start/hint audit;
- runtime/version capability declarations;
- `EffectiveSolvePlan` plus lineage/instance/revision/compilation identity in metadata;
- conformance proving starts/hints do not change feasibility;
- compiled examples.

**Gate:** default unsupported behavior rejects; all applications/conversions/rejections are recorded; HiGHS behavior is qualified; `solve` and `solve_with` remain compatible.

**Branch:** `phase-roml-P28-solve-plan-warm-starts`

---

## P29 — IIS/conflict analysis and origin-aware reports

**Goal:** diagnose infeasibility accurately in original ROML terms.

**Requirements:** SM-09 and IIS-related SM-02.6/SM-04.3.

**Deliverables:**

- optional `InfeasibilityAnalysisSession`;
- normalized request/result/report types;
- kind, scope, minimality, completion, model instance/revision, and exact compilation identity;
- mapping of compiled row sides, bounds, fixings, locks, and construct roles;
- text, Markdown, and structured renderers;
- official bundled/minimum-system HiGHS API audit;
- native implementation or precise version-gated unsupported behavior;
- deterministic fixtures and stable report tests.

**Gate:** conflict/origin compilation IDs match exactly; stale mappings reject; reports identify original entities and precise guarantees; unsupported versions fail safely.

**Branch:** `phase-roml-P29-iis-conflict-reports`

---

## P30 — Soft constraints, slacks, penalties, and feasibility relaxation

**Goal:** make controlled violation an ordinary modeling operation.

**Requirements:** SM-10.

**Deliverables:**

- soft-constraint construct/builder;
- stable lower/upper nonnegative violation variables and origins;
- correct upper/lower/equality/ranged algebra;
- validated violation bounds;
- parameter-dependent penalty weights;
- objective/lexicographic/no-target penalty modes;
- signed correction with L1 positive/negative parts;
- solution violation accessors;
- separate solve-scoped feasibility-relaxation contract;
- qualified HiGHS native path;
- public examples.

**Gate:** algebra and objective sign are proven; generated variables map to constructs; persistent softening and solve-scoped relaxation remain distinct.

**Branch:** `phase-roml-P30-soft-constraints`

---

## P31 — Objective policies and lexicographic orchestration

**Goal:** support multi-criteria optimization through one semantic policy.

**Requirements:** SM-11.

**Deliverables:**

- canonical single, weighted, and lexicographic policies;
- finite nonnegative weights with per-objective sense normalization;
- tolerance/stage validation;
- backend IR objective policy;
- native capability negotiation;
- sequential portable executor through overlay objective locks;
- correct degradation formulas for positive/zero/negative optima;
- stage and all-objective results;
- qualified HiGHS native path;
- native/portable differential corpus.

**Gate:** native and portable paths agree within declared tolerances; limited stages follow explicit continuation; stage artifacts never persist.

**Branch:** `phase-roml-P31-lexicographic-objectives`

---

## P32 — Common semantic modeling constructs

**Goal:** deliver a bounded high-value exact MILP construct library over the frozen compiler contract.

**Requirements:** SM-12 and relevant SM-13.

**Deliverables:**

- indicator and one-way implication;
- reification with separation/integrality proof;
- exact versus epigraph/hypograph min/max;
- exact absolute value, positive part, and clamp;
- Boolean implication/equivalence/any/all;
- exactly/at-most/at-least cardinality;
- binary-binary and binary-times-bounded-linear products;
- native and portable bridge recipes;
- deterministic generated roles;
- randomized reference-formulation equivalence;
- public examples and reports.

**Gate:** every construct has one semantic definition, complete origins, exact portable formulation, and explicit failure when bounds/support are insufficient.

**Branch:** `phase-roml-P32-common-constructs`

---

## P33 — Piecewise-linear functions and bound analysis

**Goal:** provide safe convexity-aware PWL modeling and Big-M evidence.

**Requirements:** SM-13, SM-14.

**Deliverables:**

- interval analyzer and bound-source traces;
- PWL validation, interpolation/extrapolation, and curvature classification;
- convex epigraph/concave hypograph rows with zero binaries;
- exact native/SOS2/segment-binary graph bridges;
- per-construct formulation override;
- randomized direct-evaluation/solver equivalence;
- numerical scaling diagnostics;
- public examples.

**Gate:** no unproven Big-M; one-sided convex/concave formulations introduce zero binaries; exact/nonconvex graphs remain exact; reports explain every representation.

**Branch:** `phase-roml-P33-piecewise-linear-bounds`

---

## P34 — Qualification, documentation, migration, and NLP-readiness

**Goal:** integrate and independently verify M3 as one coherent capability set.

**Requirements:** SM-15 and residual requirements.

**Deliverables:**

- complete requirement/evidence bundle;
- full core/HiGHS OS/version matrix;
- native/portable formulation corpus;
- overlay/rebuild and exact-identity failure matrix;
- public API/semver review;
- rustdoc, guides, examples, and migration;
- fresh packed consumers;
- package inspection;
- primitive/compile/report benchmarks;
- principal engineering, OR formulation, native/unsafe, and NLP-readiness reviews;
- milestone state update; no publication.

**Performance gate:** ordinary primitive parameter-update median overhead attributable to M3 remains below 5% or 50 microseconds per solve attempt, whichever is larger, on the fixture named in the P34 plan before any benchmarking (the same fixture is cited by SM-15.5 evidence). Exceptions require profiling evidence and owner approval.

**Gate:** all mandatory checks pass; no P0/P1 finding remains; all requirements are evidenced; exact-state safety is qualified; public claims match behavior; NLP extension review passes.

**Branch:** `phase-roml-P34-m3-qualification`

---

## Critical path

```text
P25 -> P26 -> P27 -> P28 -> integration of P29/P30/P31 -> P34
                 \-> P32 -> P33 ----------------------/
```

P26 correctness and review are the primary bottleneck. No later phase may duplicate or bypass compiler, origin, capability, objective-policy, or exact-identity boundaries.

## Program stop conditions

Stop and escalate when:

- official backend semantics contradict the generic feature contract;
- an exact bridge lacks proof;
- a convenience would silently alter exactness;
- overlay rollback cannot be made safe;
- an artifact is mapped without exact `CompilationId` agreement;
- IIS scope/minimality cannot be stated accurately;
- compilation optimization weakens rebuild equivalence;
- NLP extension would replace rather than extend the function/backend IR;
- parallel work exceeds review/integration capacity;
- any task proposes publication within M3.