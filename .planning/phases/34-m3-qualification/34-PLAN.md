# Phase 34 — M3 Qualification, Migration, Documentation, and NLP-Readiness Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** close M3 only after the combined LP/MILP semantic model, compiler, solve workflows, constructs, IIS/relaxation, objective policies, and MPS I/O satisfy the executable final qualification contract.

**Architecture:** P34 is qualification/integration, not a feature phase. `34-QUALIFICATION-CONTRACT.md` freezes the leaf ledger schema, fault matrix, native/portable corpus, backend/version/OS matrix, numerical rules, performance fixture/baseline, packed-consumer commands, NLP readiness shapes/verdicts, and positive closure predicate.

**Requirements:** SM-15.1–SM-15.8 plus final evidence closure for every leaf of SM-01–SM-14, MPS-W01–W14, and M3-C01–C05.

## Global constraints

- P30/P31/P36 must be accepted/merged; earlier required M3 phases remain accepted.
- No publication/tag/release.
- No new modeling/file-format/NLP feature added to make qualification pass.
- Correctness fixes discovered by P34 are allowed through normal TDD/review; adjacent capability expansion is not.
- Shared identity/overlay/error/objective-lock contracts remain binding.
- M3 does not close by absence of blockers; every positive gate in `34-QUALIFICATION-CONTRACT.md` must pass.

---

## Task 34-00: Exact baseline + leaf-level requirement ledger

**Create:** `.planning/phases/34-m3-qualification/34-REQUIREMENT-LEDGER.md`, baseline evidence.

- [ ] Record exact post-P31 `main` SHA, Rust/MSRV, Cargo, OS, bundled HiGHS 1.15.0 identity, system HiGHS 1.9.0 floor, dependency tree, public APIs, package lists.
- [ ] Enumerate **every** `SM-xx.y`, `MPS-Wxx`, and `M3-Cxx` row using the required columns from the qualification contract.
- [ ] Link prior exact evidence but mark missing integration/review data `BLOCKED`, never “implicitly covered.”
- [ ] Run untouched core/HiGHS/docs/policy/coverage baseline and record outputs.
- [ ] Independent ledger review checks no leaf ID is absent or aggregated away.

## Task 34-01: Parameterized primitive + MPS interchange workflow

Fixture `Q01_primitive_parameter_delta` + `Q14_mps_parameterized_snapshot`:

```text
canonical model
 -> solve
 -> parameter mutation
 -> incremental solve
 -> P36 evaluated MPS write
 -> P35 read
 -> native HiGHS readModel
```

- [ ] Assert exact `(ModelInstanceId, revision)` progression and expected CompilationId changes.
- [ ] Export report must identify the current evaluated parameter environment; no stale compiled-state export.
- [ ] Independent ROML/native structure and solve comparisons obey P36/P34 frozen tolerances.
- [ ] Parameter symbolic identity is not claimed to survive MPS.

## Task 34-02: Imported infeasibility -> IIS -> repair workflow

Use synthetic + reviewed Chinneck case:

```text
MPS -> infeasible -> P29 IIS/source origins
    -> P30 supported-origin mapping
    -> weighted-L1 repair
    -> repaired relaxed solve/report
```

- [ ] Exact supported origin mapping; unsupported members fail all-or-error.
- [ ] Stale report after model mutation rejects before mutation.
- [ ] `NoRepairFound`, `Unknown`, and operational error are exercised separately.
- [ ] Canonical model remains unchanged by solve-scoped relaxation.
- [ ] Rendered report maps back to original MPS row/bound/fixing origins without minimum-repair overclaim.

## Task 34-03: MILP reuse/construct/P31 orchestration workflow

Use Q02–Q09 + Q12/Q13 as appropriate:

- [ ] Combine exact construct/PWL, MIP start, overlay fixing/lock, persistent P30 soft constraint, parameterized penalty, P31 priorities.
- [ ] Prove P31 parameter weight resolution happens before stage construction/backend mutation.
- [ ] Exercise positive, zero, and negative stage optima and assert `scale = |z*|` lock reports.
- [ ] Verify every stage/objective/provider/CompilationId field.
- [ ] Ordinary solve afterward proves no solve-scoped artifact leakage.
- [ ] P36 semantic export either succeeds under its representability matrix or rejects active constructs with the exact typed reason; rejection is expected only for this deliberately out-of-P36 fixture and is not confused with the frozen Netlib gate.

## Task 34-04: Execute the frozen fault-injection matrix

- [ ] Implement/aggregate one named fault point for every row in `34-QUALIFICATION-CONTRACT.md` §2.
- [ ] For each assert revision, adapter health, CompilationId trust, returned error/result, and cleanup artifacts exactly as specified.
- [ ] Add combined primary+rollback and rollback+rebuild failure cases.
- [ ] Produce machine-readable matrix output: boundary, injected fault, expected state, actual state, PASS/FAIL.
- [ ] Zero mandatory row may be skipped because a backend happens not to fail naturally; use fault seams/reference backends.

## Task 34-05: Native/portable/backend-version corpus

- [ ] Materialize exact Q01–Q14 fixture inventory from qualification contract.
- [ ] Run ReferenceBackend semantic/recovery checks.
- [ ] Run bundled HiGHS 1.15.0 required Linux/macOS/Windows matrix.
- [ ] Run system HiGHS 1.9.0 required Linux compatibility floor; optional other-OS system lanes remain explicit.
- [ ] Compare observables under frozen structural/objective tolerances.
- [ ] Every mismatch receives one allowed disposition; owner/reviewer approval required for bounded residual exceptions.
- [ ] Generate capability truth table from actual declarations/results; docs must consume/agree with it.

## Task 34-06: `P34_PRIMITIVE_PARAMETER_UPDATE_V1` performance gate

- [ ] Commit deterministic external fixture/generator under `tools/p34-perf/` exactly matching §4 of qualification contract.
- [ ] Benchmark historical pre-M3 implementation `4d111cceafce17aea44a6e396a838d1cc9ef255d` and exact P34 candidate on same machine/configuration.
- [ ] 20 warmups + 200 measured release-mode update+solve attempts, HiGHS bundled, output off, threads=1.
- [ ] Record median, p25, p75 and available sync classification.
- [ ] Apply exact gate: candidate excess median <= `max(5% baseline median, 50us)`.
- [ ] Any breach: profile before changes; exception requires written owner approval.
- [ ] Record non-gating P29/P30/P31/P36 telemetry separately.

## Task 34-07: Public API/coherence/migration review

- [ ] Review end-user flow: model -> solve/SolvePlan -> I/O -> IIS -> relaxation -> objective policy.
- [ ] Verify one identity vocabulary, one ObjectivePolicy, one ObjectivePriority, one provider policy per workflow, and no decorative dead fields.
- [ ] `prelude` stays ordinary-user focused; framework/backend internals stay advanced.
- [ ] Update README/MODELING_API/MIGRATION/CHANGELOG/support matrix from generated/evidence truth.
- [ ] Compile/run every public example in required CI lanes.

## Task 34-08: Exact packed-consumer/package protocol

- [ ] Create `scripts/p34-packed-consumers.sh` implementing §5 exactly.
- [ ] Run exact package-list and `cargo package -p roml --locked` commands in a clean exact-head worktree.
- [ ] Attempt `cargo package -p roml-highs --locked`; accept only the already documented unpublished-`roml` resolution limitation, otherwise fail.
- [ ] Assert no `.planning`, `.worktrees`, corpora, git metadata, target output, machine paths, or solver logs in packed artifacts.
- [ ] Create/run `/tmp/p34-consumer-{core,highs,mps,iis-relax,lexicographic}` from packed/extracted sources only.
- [ ] Record exact Cargo.toml/source hashes, commands, outputs, and expected objective/report assertions.

## Task 34-09: Concrete quadratic/NLP readiness review

For N1 convex QP, N2 convex QCQP, N3 nonconvex bilinear, N4 smooth nonlinear parameterized shape:

- [ ] Trace each through every component listed in qualification contract §6.
- [ ] Assign each component one verdict: `READY_ADDITIVE`, `READY_WITH_BOUNDED_M4_AMENDMENT`, or `BLOCKED_REPLACEMENT_REQUIRED`.
- [ ] Bounded amendment must state exact type/module/API owner and blast radius.
- [ ] No `BLOCKED_REPLACEMENT_REQUIRED` may remain at M3 closure without an explicit owner decision changing the architectural goal.
- [ ] Record full matrix in `docs/release/evidence/M3_NLP_READINESS.md`; “enum is non-exhaustive” alone is not evidence.

## Task 34-10: Independent review gauntlet

Request independent perspectives:

```text
principal architecture/API
OR formulation correctness
backend/native/unsafe boundary
testing/qualification evidence
quadratic/NLP extension readiness
```

- [ ] Each reviewer receives exact head, frozen contracts, and assigned evidence, not session history.
- [ ] Consolidate all P0/P1/P2 findings with disposition.
- [ ] Resolve all P0/P1 and rerun affected focused + full gates.
- [ ] Leaf ledger records review IDs and residual risk for every impacted requirement.

## Task 34-11: Exact-head positive closure

- [ ] Produce `M3_FINAL_QUALIFICATION.md`, milestone `SUMMARY.md`, final capability matrix, leaf ledger, fault matrix, perf report, packed-consumer report, NLP-readiness report.
- [ ] Evaluate the **entire** positive predicate in `34-QUALIFICATION-CONTRACT.md` §7; every conjunction must have affirmative evidence.
- [ ] Verify exact-head hosted Core/MSRV, HiGHS, Coverage, Quality, Policy and any P34-specific workflow gates.
- [ ] Independent final review: zero unresolved P0/P1.
- [ ] Move P34 to `pending_merge`; owner-authorized merge only.
- [ ] After merge mark M3 complete and route to **M4 design gate only**; do not create M4 production branch automatically.

## Stop conditions

Stop closure if a mandatory leaf lacks evidence, any fault-matrix row cannot prove its expected state, public capability exceeds runtime qualification, native/portable mismatch is unresolved, performance breaches without approved profiled disposition, packed consumer uses live workspace paths, or any N1–N4 readiness path requires replacement of a core M3 identity/compiler boundary.
