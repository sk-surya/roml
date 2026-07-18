# ROML-M1R Execution Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Audit and repair the current M1 candidate, complete the revisioned backend contract migration, qualify HiGHS natively across supported platforms, and prepare an exact-evidence v0.1 release of `roml` and `roml-highs`.

**Architecture:** Canonical `Model` state emits immutable snapshots and replayable typed deltas. Independent backend sessions synchronize through cursors and explicit acknowledgements, negotiate immutable solve requests, and expose faithful results. HiGHS is the mandatory reference native backend; MOSEK and Xpress remain separately gated.

**Tech Stack:** Rust 2021, MSRV 1.85, Cargo workspace, `highs-sys`, GitHub Actions, property/differential testing, cargo-semver-checks, cargo-deny, cargo-audit, cargo-machete, packed-crate consumers, Criterion or equivalent.

## Global Constraints
- Planning authority: `planning/roml-ultra-mega-roadmap-v2`.
- Merged baseline at packet creation: `main@ef37c88a6d80775ea69d2ccb986655edeb5789ec`.
- Inherited M1 candidate is untrusted until M1R-00 admission.
- No supported path destructively drains model changes before backend acknowledgement.
- Canonical `Model` contains no transient solve policy.
- Unsupported options/domains/callbacks are rejected explicitly.
- No handwritten native ABI where maintained generated/vendor bindings exist.
- No expected environment failure panics.
- Commercial adapters do not block core + HiGHS.
- No publish/tag action without exact-SHA owner authorization.
- Every implementation task is test-first, independently reviewable, and committed separately.

---

## Execution waves

```text
Wave 0: M1R-00 truth reset
Wave 1: M1R-01 contract closure (single integration owner)
Wave 2: H1 lifecycle/bindings | H2 projection | H3 solve/extraction | C1 CI prep | P1 benchmark prep | M/X research
Wave 3: integrate M1R-02, then M1R-03 semantic/fault campaigns
Wave 4: M1R-04 hosted/package matrix | M1R-05 performance/UX | optional commercial implementation
Wave 5: M1R-08 release candidate and independent audits
Wave 6: owner-authorized publication, then M1R-09 operations
```

Do not start Wave 2 implementation against an unfrozen contract. CI/benchmark scaffolding and vendor research may proceed but cannot freeze obsolete APIs.

### Task 1: Candidate manifest and truth reset

**Files:**
- Create: `docs/release/evidence/M1R/M1R-00-ADMISSION.md`
- Modify: `.planning/STATE-V2.md`
- Reference: `.planning/phases/00-truth-reset-and-candidate-admission/phase.md`

**Produces:** exact candidate head, commit/file/requirement manifest, ignored-test disposition, integration strategy.

- [ ] Fetch all refs and record main, candidate, planning heads and merge base.
- [ ] Export candidate commits and changed files; classify each as planning, contract, HiGHS, tests, CI/package, benchmark, MOSEK, Xpress, or unrelated.
- [ ] Run the full exact-candidate verification command set and record exit codes, environment, versions, ignored/skipped counts, and package lists.
- [ ] Temporarily unignore each of the 11 tests and run it individually; record requirement and current outcome.
- [ ] Audit source claims around legacy synchronization, solve policy, status/error types, HiGHS lifecycle, and common-harness instantiation.
- [ ] Verify license authorization and crates.io name state as separate external gates.
- [ ] Select merge-after-repair, split/replay, or replace for every candidate component.
- [ ] Request independent admission review.
- [ ] Commit evidence and state update.

**Gate:** M1R-G1–G4 and M1R-C8 have complete disposition; M1R-01 exact base is frozen.

### Task 2: Public backend API and compatibility design

**Files:**
- Modify: `src/solver/backend.rs`, `src/solver/request.rs`, `src/solver/mod.rs`, `src/sync.rs` or focused successors
- Modify: `src/lib.rs`
- Test: `tests/backend_contract.rs`, `tests/sync_characterization.rs`, `tests/status_negotiation_tests.rs`
- Create: `docs/release/evidence/M1R/M1R-01-CONTRACT-CLOSURE.md`

**Produces:** frozen exact interfaces for backend factory/session, synchronization, request negotiation, result/error/status, capabilities, and solution views.

- [ ] Inventory the current public API and write a compatibility table for every legacy symbol.
- [ ] Write failing tests proving no delta/request loss, independent cursors, explicit option outcomes, deterministic rebuild, and faithful statuses.
- [ ] Define concrete signatures for backend creation, synchronization, rebuild, solve negotiation, solve execution, and solution access.
- [ ] Review design for bounded responsibilities rather than one universal trait.
- [ ] Obtain architecture/API approval before implementation.
- [ ] Commit contract tests and approved interface skeleton.

**Gate:** exact interfaces are frozen and all backend workers can implement without editing shared semantics.

### Task 3: Revisioned synchronization integration

**Files:**
- Modify: `src/model.rs` or current model modules, `src/journal.rs`, `src/sync.rs`, `src/transaction.rs`, solver session modules
- Test: contract/synchronization suites

**Consumes:** Task 2 interfaces.  
**Produces:** supported synchronization path using snapshots, delta batches, cursor acknowledgement, health, and rebuild.

- [ ] Make the failing pre-ack failure test reproducible.
- [ ] Connect model revision/journal to independent backend cursors.
- [ ] Implement gap detection and snapshot rebuild.
- [ ] Advance cursor only after complete acknowledgement.
- [ ] Preserve replay after retryable/dirty failure.
- [ ] Verify atomic transaction revision boundaries.
- [ ] Run focused tests, then all core tests.
- [ ] Commit independently.

### Task 4: Solve policy, status, error, and solution closure

**Files:**
- Modify: solver request/status/error/solution modules, `src/model/*`, crate exports/prelude
- Test: `tests/status_negotiation_tests.rs`, backend contract tests

- [ ] Write failing source/API test proving canonical Model owns no transient solver policy.
- [ ] Remove model-owned options and legacy consumption.
- [ ] Implement applied/adjusted/rejected negotiation and effective configuration.
- [ ] Finalize status lattice and backend error recoverability.
- [ ] Implement non-mandatory-cloning solution access.
- [ ] Update exports and tests.
- [ ] Commit independently.

### Task 5: Legacy API disposition and documentation

**Files:**
- Modify: `src/solver/mod.rs`, `src/lib.rs`, `README.md`, `MODELING_API.md`, examples, `CHANGELOG.md`
- Create/modify: migration guide

- [ ] Choose removal or safe deprecated shim from M1R-00 evidence.
- [ ] Prove any shim delegates to safe synchronization and explicit request negotiation.
- [ ] Remove all supported examples of destructive drain/best-effort behavior.
- [ ] Compile examples/doctests and run semver review.
- [ ] Obtain independent public-API review.
- [ ] Complete M1R-01 evidence and commit.

**Gate:** M1R-C1–C8 pass; mandatory ignored count is zero.

### Task 6: HiGHS binding and lifecycle boundary

**Files:**
- Modify/split: `roml-highs/src/ffi.rs`, `adapter.rs`, `lib.rs`, `Cargo.toml`, build configuration
- Create as needed: `error.rs`, `lifecycle.rs`, focused tests

**Consumes:** frozen M1R-01 interfaces.  
**Produces:** fallible safe native handle owner backed solely by maintained bindings.

- [ ] Write failing construction tests for native create/index/version/configuration errors.
- [ ] Verify pinned `highs-sys` symbol/header coverage.
- [ ] Remove copied ABI declarations/constants and duplicate links/build ownership.
- [ ] Implement typed construction and deterministic Drop/cleanup.
- [ ] Audit/limit Send/Sync and every unsafe block.
- [ ] Check every option-set/native return code.
- [ ] Add backend metadata query.
- [ ] Run focused native tests and commit.

### Task 7: HiGHS snapshot projection

**Files:**
- Create/modify: projection and index-map modules
- Test: HiGHS projection tests and common harness

- [ ] Write failing projection cases for empty, objectiveless, LP, MILP, ranged, activity, domains, coefficients, objective offset/switching.
- [ ] Implement deterministic clean rebuild from snapshot.
- [ ] Verify index maps and cached state after rebuild.
- [ ] Classify partial rebuild failure correctly.
- [ ] Run focused tests and commit.

### Task 8: HiGHS typed delta application

**Files:**
- Create/modify: session/projection modules
- Test: operation matrix and recovery cases

- [ ] Write one failing case per admitted ModelOp.
- [ ] Implement prevalidation and typed native mappings.
- [ ] Add failure sites around every multi-call operation.
- [ ] Advance cursor only after success; classify dirty states.
- [ ] Verify add/remove/reindex/deactivate/reactivate/objective transitions.
- [ ] Run common focused tests and commit.

### Task 9: HiGHS request, solve, and extraction

**Files:**
- Create/modify: request/session/solution/callback modules
- Test: request/status/solution native suites

- [ ] Write failing applied/adjusted/rejected request cases.
- [ ] Map admitted options and return effective configuration.
- [ ] Implement faithful native status mapping, including ambiguous and incumbent states.
- [ ] Extract objective offset once, primal, valid dual/reduced-cost/basis data.
- [ ] Invalidate stale solution state after mutation/failure.
- [ ] Implement only officially supported callbacks with panic containment.
- [ ] Run focused tests and commit.

**Gate:** M1R-H1–H8 pass and native-safety review is clean.

### Task 10: Common native conformance harness

**Files:**
- Refactor: `tests/backend_contract.rs`, `tests/differential_harness.rs`, associated support modules

- [ ] Define a fixture/factory abstraction shared by ReferenceBackend and HiGHS.
- [ ] Prove the test binary actually instantiates both implementations.
- [ ] Port focused operation scenarios without duplicating semantic expectations.
- [ ] Add exact backend/version metadata to failures.
- [ ] Commit harness refactor before broad random campaigns.

### Task 11: Differential trace campaign

**Files:**
- Modify: differential generator/harness
- Create: deterministic regression corpus and M1R-03 evidence

- [ ] Build legal state-aware operation generator.
- [ ] Run deterministic seeds across every operation family.
- [ ] Compare incremental versus fresh rebuild at each revision.
- [ ] Shrink and persist every failure before repair.
- [ ] Promote minimized traces to regressions.
- [ ] Commit test/fix cycles separately.

### Task 12: Fault, cursor, status, and solution campaigns

- [ ] Enumerate native multi-call failure points.
- [ ] Inject before/during/after apply, rebuild, request, solve, extraction.
- [ ] Assert cursor, health, replay, stale solution, recovery.
- [ ] Run multi-session lag/catch-up and failure isolation.
- [ ] Run domain/semi-continuous, status/error, request, objective, dual/reduced-cost, basis cases.
- [ ] Independent verifier adds seeds/sites and signs M1R-03 evidence.

**Gate:** M1R-Q1–Q5 pass.

### Task 13: Hosted native matrix and package consumers

**Files:**
- Modify/add: `.github/workflows/*`, package scripts/fixtures, support matrix
- Create: M1R-04 evidence

- [ ] Implement exact workflow topology and least-privilege permissions.
- [ ] Test bundled HiGHS on required OSes and MSRV smoke.
- [ ] Test explicit system-discovery mode and clean failure diagnostics.
- [ ] Build `roml`/`roml-highs` archives and consume them from fresh projects.
- [ ] Run docs, semver, audit, deny, machete, package list/license checks.
- [ ] Add scheduled property/fuzz/sanitizer lanes.
- [ ] Archive exact-SHA run links/artifact hashes.
- [ ] Commit and obtain CI/package review.

**Gate:** M1R-P1–P6 pass on one exact SHA.

### Task 14: Performance and user-journey acceptance

**Files:**
- Modify/add: `benches/*`, examples, performance docs, migration guide
- Create: M1R-05 evidence

- [ ] Define workload manifests and metadata schema.
- [ ] Benchmark each decomposed stage separately.
- [ ] Measure incremental/rebuild crossover and basis reuse.
- [ ] Prove and measure any bulk path.
- [ ] Profile memory/allocation and identify release blockers.
- [ ] Run all public user journeys from packed crates.
- [ ] Scope claims and document unsupported features.
- [ ] Commit and review.

**Gate:** M1R-E1–E5 pass.

### Task 15: Commercial tracks (optional, non-blocking)

- [ ] Execute MOSEK tasks only after official API and protected runner admission.
- [ ] Execute Xpress tasks only after legal/binding and protected runner admission.
- [ ] Keep separate PRs/evidence and `publish = false` until independent gates.
- [ ] Never delay M1R-08 for unfinished commercial work.

### Task 16: Release candidate evidence and audits

**Files:**
- Create: `docs/release/evidence/M1R/M1R-08-RELEASE-CANDIDATE.md`, release manifest, SBOM/checksums
- Modify: versions, changelog, migration/support/release docs only as admitted

- [ ] Freeze exact release SHA/scope/features/MSRV/support matrix.
- [ ] Rerun all mandatory local/hosted/package evidence at that SHA.
- [ ] Conduct four independent reviews and resolve every blocker.
- [ ] Rehearse publication and released-core consumer ordering.
- [ ] Prepare exact owner authorization record.
- [ ] Commit evidence-only finalization if no source changes; otherwise rerun affected gates.

### Task 17: Owner-authorized publication and operations

- [ ] After explicit owner authorization, publish `roml`.
- [ ] Verify crates.io/docs.rs and build backend against released core.
- [ ] Publish `roml-highs`.
- [ ] Verify end-to-end consumer and tag exact commit.
- [ ] Activate compatibility, patch, security, diagnostics, and deprecation processes.
- [ ] Archive milestone retrospective and evaluate M2 admission.

## Per-task report
```text
TASK / PHASE:
BASE SHA:
HEAD SHA:
WORKTREE / BRANCH:
PR:
REQUIREMENTS:
TESTS (pass/fail/ignored/skipped):
CI / ARTIFACTS:
REVIEW FINDINGS:
RISKS / DEVIATIONS:
GATE: PASS | FAIL | OWNER-BLOCKED | EXTERNAL-BLOCKED
NEXT ADMITTED TASKS:
```

## Completion rule
M1R is complete only after published `roml` and `roml-highs` artifacts, their tag, package hashes, SBOM, hosted CI, packed consumers, and release evidence all identify the same exact source commit—or after an explicitly documented owner/external stop disposition. Candidate code, local tests, or a planning ledger alone cannot complete the milestone.
