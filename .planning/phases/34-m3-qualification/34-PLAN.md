# Phase 34 — M3 Qualification, Migration, Documentation, and NLP-Readiness Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** close M3 by independently proving the combined LP/MILP semantic model, compiler, solve workflows, constructs, IIS/relaxation, objective policies, and MPS I/O are coherent, performant, recoverable, and extensible.

**Architecture:** P34 is qualification/integration, not a feature phase. It may fix correctness/API/documentation defects discovered by integration, but it may not add adjacent modeling capabilities to make the milestone look broader.

**Requirements:** SM-15.1–SM-15.8 plus residual evidence obligations from SM-01–SM-14.

## Global constraints

- P29, P30, P31, P32, P33, P35, P36 must all be accepted before P34 implementation begins.
- No crate publication, tag, or release.
- No new parser format or nonlinear production feature.
- Any P0/P1 finding blocks milestone closure.
- Public claims are capped at exact evidence; backend/version limitations remain explicit.

---

### Task 34-00: Freeze M3 exact baseline and requirement ledger

- [ ] Record exact `main` SHA after P31 merge.
- [ ] Enumerate every SM-01..SM-15 requirement and link existing phase evidence/tests.
- [ ] Mark each as `evidenced`, `needs_integration_evidence`, or `gap`; no vague partial state.
- [ ] Capture `cargo public-api` for `roml` and `roml-highs`.
- [ ] Capture package lists, dependency tree, toolchain/backend versions.
- [ ] Run untouched full baseline CI/local matrix and store raw summary.
- [ ] Commit evidence only.

### Task 34-01: Integrated workflow — parameterized production LP + MPS interchange

Construct one named parameterized LP fixture that exercises:

```text
Model -> compile -> HiGHS solve
 -> parameter update -> incremental solve
 -> MPS write -> ROML read
 -> native HiGHS readModel
 -> structural + solve equivalence
```

- [ ] Assert exact revision/CompilationId progression and no unnecessary rebuild where incremental support applies.
- [ ] Assert exported model reflects current canonical parameter value, not stale compiled state.
- [ ] Assert semantic and native structures/objectives agree.
- [ ] Record timings as evidence.

### Task 34-02: Integrated workflow — infeasible imported model -> IIS -> repair

Use synthetic fixture plus selected Chinneck model where practical:

```text
MPS read
 -> prove infeasible
 -> P29 semantic IIS
 -> exact source provenance
 -> P30 relaxation seeded/scoped from report
 -> repaired feasible solve
 -> violation report in original MPS terms
```

- [ ] Verify stale IIS report rejects after model mutation.
- [ ] Verify relaxation does not mutate canonical model unless user explicitly applies a later canonical edit.
- [ ] Verify IIS/repair guarantee wording remains distinct.

### Task 34-03: Integrated workflow — MILP reuse + constructs + lexicographic solve

Build a compact operational MILP combining:

- semantic indicator/min/max or PWL construct;
- MIP start;
- temporary fixing/solution lock;
- P30 soft penalty priority;
- P31 economics/deviation priorities.

- [ ] Solve portable path.
- [ ] Verify stage results, objective locks, overlays, generated origins, compilation reports.
- [ ] Re-solve ordinary model afterward and prove no solve-scoped artifact leaked.
- [ ] Export classification through P36: semantic export succeeds only if representable; otherwise typed `Unrepresentable` and optional compiled-formulation export if that feature was accepted.

### Task 34-04: Failure/recovery matrix

Create one consolidated fault harness spanning:

```text
canonical mutation
compile
backend delta apply
rebuild
SolvePlan overlay apply
solve
solution extraction
overlay rollback
IIS oracle/reducer/verify
feasibility relaxation
lexicographic stage lock/rollback
MPS path write
```

For each injectable boundary record expected state:

- model revision unchanged/advanced as appropriate;
- adapter cursor `Ready` or `RequiresRebuild`;
- exact compilation identity retained/invalidated correctly;
- no stale solution/analysis accepted;
- no temp row/bound/objective leak.

### Task 34-05: Native/portable and cross-version capability matrix

- [ ] Build a generated table from actual capability declarations for bundled HiGHS and qualified system HiGHS versions.
- [ ] Differential-test every feature with both native and portable paths where both exist.
- [ ] Confirm `Native`, `Bridge`, `Unsupported` claims match runtime behavior.
- [ ] Explicitly retain known limits such as system-native IIS versions if not qualified.
- [ ] No documentation hand-maintained table may contradict generated/evidence truth.

### Task 34-06: Performance regression gate

The original M3 gate remains binding:

> ordinary primitive parameter-update median overhead attributable to M3 remains below 5% or 50 microseconds per solve attempt, whichever is larger, on the fixture named before benchmarking.

- [ ] Freeze benchmark fixture and machine/toolchain/backend metadata before running.
- [ ] Compare current M3 path to appropriate accepted baseline/reference path.
- [ ] Run enough repetitions/warmups to report median and dispersion.
- [ ] Profile any breach before optimizing.
- [ ] Add targeted non-gating measurements for P29 IIS, P30 relaxation, P31 stage orchestration, and P36 writer throughput/size.
- [ ] Exceptions require profiling evidence and owner approval recorded in evidence.

### Task 34-07: Public API coherence and migration review

- [ ] Review golden path from a fresh user's perspective: model, solve, plan, analysis, I/O.
- [ ] Identify duplicate concepts/names introduced across phases; remove pre-1.0 accidental redundancy only with migration notes and compile guards.
- [ ] Verify `prelude` remains curated and `advanced` contains framework/backend concepts.
- [ ] Update `MIGRATION.md`, `MODELING_API.md`, README capability table, CHANGELOG, support matrix.
- [ ] Every public example compiles/runs in CI.

### Task 34-08: Fresh packed consumers

Create clean external consumers from packed crates, not workspace paths:

1. core modeling only;
2. default bundled HiGHS quickstart/incremental;
3. MPS read/write round trip;
4. IIS + relaxation;
5. lexicographic solve.

- [ ] Validate package content has no corpus/worktree/planning leakage beyond intended docs/licenses.
- [ ] Validate no undeclared workspace dependency is required.

### Task 34-09: NLP-readiness principal architecture review

This is a required evidence review, not implementation.

Explicitly certify whether each existing component can be extended without replacement:

- `ScalarFunction`;
- scalar/set constraint representation;
- parameter dependencies;
- canonical snapshots/deltas;
- backend IR;
- compiler recipes/reports;
- typed capabilities;
- origin maps;
- `CompilationId` exact-state semantics;
- SolvePlan/overlays;
- starts/hints/assignments;
- objective policies;
- IIS/relaxation reporting;
- file-I/O boundary.

The review must test concrete proposed quadratic/NLP shapes rather than saying “non-exhaustive enum means extensible.” Record findings in `docs/release/evidence/M3_NLP_READINESS.md`.

Any architectural replacement requirement is a P1 M3 closure blocker or an explicitly owner-approved M4 migration debt with bounded blast radius.

### Task 34-10: Independent review gauntlet

Request at least these review perspectives, sequentially or in bounded parallel if independent:

- principal software architecture/API;
- operations-research formulation correctness;
- backend/native/unsafe boundary;
- testing/qualification evidence;
- NLP extension readiness.

- [ ] Consolidate findings by severity.
- [ ] Resolve all P0/P1.
- [ ] Re-run affected focused tests plus full exact-head matrix.
- [ ] Record review IDs and dispositions.

### Task 34-11: Milestone closure

- [ ] Produce `docs/release/evidence/M3_FINAL_QUALIFICATION.md` with requirement traceability, exact SHA, CI, performance, corpus, review, residuals.
- [ ] Produce `.planning/milestones/M3-semantic-modeling-workflows/SUMMARY.md` describing what M3 actually guarantees.
- [ ] Mark P34 and M3 complete only after independent CLEAR TO MERGE and owner merge.
- [ ] Route active planning to M4 design gate; do not create M4 implementation branch automatically.

## P34 stop conditions

Stop milestone closure if any mandatory SM requirement lacks evidence, any public capability claim exceeds runtime qualification, primitive incremental performance breaches without approved disposition, rollback/stale-state safety is uncertain, or NLP-readiness requires replacing core identity/compiler boundaries.