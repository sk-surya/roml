# Phase 09 Plan 03: Test Classification and Branch Contamination Analysis

**Date:** 2026-07-18
**Plan:** 09-truth-reset-and-candidate-admission / 03
**Requirement:** M1R-G2, M1R-G3, M1R-G4
**Evidence for:** D2 (Ignored test disposition), D3 (Branch contamination and replay strategy)

---

## Ignored Test Existence Confirmation

All 11 ignored test annotations confirmed on candidate branch `planning/roml-M1-native-backends-release` via:

```
$ git grep -n '#\[ignore' planning/roml-M1-native-backends-release -- tests/
planning/roml-M1-native-backends-release:tests/model_characterization.rs:4://! semantic refactoring (Phase 1). Tests marked with `#[ignore]` document
planning/roml-M1-native-backends-release:tests/model_characterization.rs:545:#[ignore = "P1: last-write-wins coefficient semantics"]
planning/roml-M1-native-backends-release:tests/model_characterization.rs:566:#[ignore = "P1: last-write-wins coefficient semantics"]
planning/roml-M1-native-backends-release:tests/model_characterization.rs:818:#[ignore = "P1: semicontinuous partial apply"]
planning/roml-M1-native-backends-release:tests/model_characterization.rs:842:#[ignore = "P1: solve options should move to solve request"]
planning/roml-M1-native-backends-release:tests/sync_characterization.rs:18://! All tests are marked `#[ignore = "P2: destructive changelog — fixed by revisioned sync"]`
planning/roml-M1-native-backends-release:tests/sync_characterization.rs:185:#[ignore = "P2: destructive changelog — fixed by revisioned sync"]
planning/roml-M1-native-backends-release:tests/sync_characterization.rs:223:#[ignore = "P2: destructive changelog — fixed by revisioned sync"]
planning/roml-M1-native-backends-release:tests/sync_characterization.rs:281:#[ignore = "P2: destructive changelog — fixed by revisioned sync"]
planning/roml-M1-native-backends-release:tests/sync_characterization.rs:313:#[ignore = "P2: destructive changelog — fixed by revisioned sync"]
planning/roml-M1-native-backends-release:tests/sync_characterization.rs:356:#[ignore = "P2: destructive changelog — fixed by revisioned sync"]
planning/roml-M1-native-backends-release:tests/sync_characterization.rs:411:#[ignore = "P2: destructive changelog — fixed by revisioned sync"]
planning/roml-M1-native-backends-release:tests/sync_characterization.rs:459:#[ignore = "P2: destructive changelog — fixed by revisioned sync"]
```

**Count:** 11 test `#[ignore]` annotations (excluding 2 doc-comment references at lines 4 and 18).

---

## Ignored Test Disposition Table

| ID | Test Name | File | Line | Current Behavior | Ignore Reason | Fix Category | Fix Action | Requirement |
|----|-----------|------|------|-----------------|---------------|--------------|------------|-------------|
| P1-1 | `duplicate_coefficient_for_same_cell` | `tests/model_characterization.rs` | 545 | Last-write-wins: two coefficients for same (con, var) cell produce 2 entries in model index and expression. Assertions expect `num_coefficients() == 2` and `num_terms() == 2`. | `P1: last-write-wins coefficient semantics` | **fix-now** | Remove `#[ignore]`. Update assertions to expect 1 combined coefficient. The canonical cell implementation (commit c1fe456) combines duplicate coefficients algebraically. | M1R-C5 |
| P1-2 | `duplicate_coefficient_in_objective` | `tests/model_characterization.rs` | 566 | Same as P1-1 but for objective coefficients: two coefficients for same (objective, var) pair produce 2 entries. | `P1: last-write-wins coefficient semantics` | **fix-now** | Remove `#[ignore]`. Update assertions to expect 1 combined coefficient. Same canonical fix as P1-1. | M1R-C5 |
| P1-3 | `set_semicontinuous_low_lower_emits_change_without_bounds_update` | `tests/model_characterization.rs` | 818 | Semi-continuous lower (3.0) <= current lower (5.0), so bounds unchanged, but a `SemiContinuousBoundChanged` change IS emitted via `drain_changes()`. | `P1: semicontinuous partial apply` | **pin-m1r1** | Update expectation to assert desired post-M1R-01 behavior. Re-mark with `#[ignore = "resolved in M1R-01 — drain_changes removal"]`. The Change-based emission path is being removed by M1R-01; DeltaBatch path will handle correctly. | M1R-H7 |
| P1-4 | `solve_options_stored_on_model_and_consumed_during_solve` | `tests/model_characterization.rs` | 842 | SolveOptions set on Model via `set_solver_options()`. No public getter. Options consumed during solve via `SolverAdapter::apply_options`. | `P1: solve options should move to solve request` | **pin-m1r1** | Update assertion to document expected behavior via SolveRequest path. Re-mark with `#[ignore = "resolved in M1R-01 — solve policy removal from Model"]`. The SolveRequest type exists but `Model.solver_options` field persists — M1R-01 removes it. | M1R-C2 |
| P2-1 | `drained_changes_are_lost_on_adapter_error` | `tests/sync_characterization.rs` | 185 | `drain_changes()` is destructive. After drain, model has no pending changes and a second drain returns empty Vec. Adapter receives drained changes, but if adapter had failed, changes would be lost. | `P2: destructive changelog — fixed by revisioned sync` | **pin-m1r1** | Update expectation to assert desired post-M1R-01 behavior: journal retains batch, retry possible. Re-mark with `#[ignore = "resolved in M1R-01 — drain_changes removal"]`. | M1R-C1, M1R-C4 |
| P2-2 | `error_during_apply_loses_changes_from_model` | `tests/sync_characterization.rs` | 223 | Adapter configured to fail after 2 ops. `sync_model` drains 3 changes, adapter applies 2, then fails. After error: no changes remain on model, adapter partially applied, no way to determine subset. | `P2: destructive changelog — fixed by revisioned sync` | **pin-m1r1** | Same as P2-1. Update expectation to assert journal-based recovery. Re-mark. | M1R-C1, M1R-C4 |
| P2-3 | `two_adapters_cannot_both_sync_same_changes` | `tests/sync_characterization.rs` | 281 | Single-consumer changelog. After adapter A drains and applies 3 changes, adapter B's drain returns empty. B applies 0 changes. | `P2: destructive changelog — fixed by revisioned sync` | **pin-m1r1** | Update expectation: SyncCoordinator supports independent cursors. Both adapters should receive same changes. Re-mark. | M1R-C1, M1R-C4 |
| P2-4 | `sync_model_leaves_nothing_for_second_adapter` | `tests/sync_characterization.rs` | 313 | Same as P2-3 but via `sync_model` convenience method. After adapter A syncs, adapter B gets 0 changes. | `P2: destructive changelog — fixed by revisioned sync` | **pin-m1r1** | Same as P2-3. Update expectation to assert both adapters receive changes via independent cursors. Re-mark. | M1R-C1, M1R-C4 |
| P2-5 | `no_recovery_path_after_partial_apply` | `tests/sync_characterization.rs` | 356 | Partial apply leaves model in undefined state. After adapter fails mid-batch: no recoverable changes on model, adapter partially mutated. Attempted recovery approaches (call sync_model again, reset adapter and sync) all get nothing. | `P2: destructive changelog — fixed by revisioned sync` | **pin-m1r1** | Update expectation: `ApplyOutcome::RequiresRebuild` provides recovery path. Re-mark. | M1R-C1, M1R-C4 |
| P2-6 | `reset_has_no_revision_check` | `tests/sync_characterization.rs` | 411 | After reset, adapter clears state. No way to check if adapter is synchronized with model: no revision counter accessible to adapter, no `is_synchronized` method on SolverAdapter. | `P2: destructive changelog — fixed by revisioned sync` | **pin-m1r1** | Update expectation: `AdapterCursor` tracks `applied_revision`. Re-mark. | M1R-C1, M1R-C4 |
| P2-7 | `no_staleness_detection_after_mutation` | `tests/sync_characterization.rs` | 459 | After adapter syncs (2 changes), model mutates further (adds variable, 1 change). No adapter-aware mechanism signals staleness — caller must re-sync manually. No compile-time or runtime guard against stale reads. | `P2: destructive changelog — fixed by revisioned sync` | **pin-m1r1** | Update expectation: cursor revision comparison detects staleness. Re-mark. | M1R-C1, M1R-C4 |
| **Total** | **11** | | | | | **2 fix-now + 9 pin-m1r1** | | |

### Category Summary

| Category | Count | Tests | Action |
|----------|-------|-------|--------|
| fix-now | 2 | P1-1, P1-2 | Remove `#[ignore]`, update assertion to match canonical cell behavior |
| pin-m1r1 | 9 | P1-3, P1-4, P2-1 through P2-7 | Update expectation to match desired post-M1R-01 behavior, re-mark with `#[ignore = "resolved in M1R-01 — drain_changes removal"]` |

### Requirement Mapping

| Requirement ID | Description | Tests |
|---------------|-------------|-------|
| M1R-C5 | Canonical cell behavior | P1-1, P1-2 |
| M1R-H7 | Semi-continuous recovery | P1-3 |
| M1R-C2 | Solve policy outside Model | P1-4 |
| M1R-C1 | Destructive drain removal | P2-1 through P2-7 |
| M1R-C4 | Health model | P2-1 through P2-7 |

### IMPORTANT: Test File Location

Test files exist only on branch `planning/roml-M1-native-backends-release`, not on this planning branch. Plan 04 (wave 2) uses git worktree to check out the candidate branch, fix the tests, and commit to a feature branch.

---

## Branch Contamination Analysis

### Confirmed Topology

```
82e2ed95 (main) → ... → ef37c88 (origin/main) ← merge of P0-P6 worktree branch
                                ↓
                          073106f (planning docs start)
                                ↓
                           ... (17 more commits)
                                ↓
                          649c635 (tip of planning/roml-M1-native-backends-release)
```

**Merge base:** `82e2ed95` (`git merge-base main planning/roml-M1-native-backends-release`)
**Documented base in RESEARCH.md:** `ef37c88`
**Discrepancy:** main has advanced with `82e2ed9 feat: SolveOptions plumbing for per-solve LP algorithm override`. The 20-commit candidate branch is built on `ef37c88`, not `82e2ed95`. Cherry-pick base should use `main@ef37c88` (the point where the candidate branch forked), not `main@82e2ed95` (current main HEAD).

### Commits that Touch .planning/ (8 commits)

Verified with:
```
$ git log --oneline --name-only planning/roml-M1-native-backends-release --not main -- .planning/
```

| SHA | Message | Files Touched Outside .planning/ | Classification |
|-----|---------|---------------------------------|----------------|
| 073106f | docs(planning): add ROML M1 native backend mega roadmap | (none) | Planning-only |
| bc8e3e0 | docs(planning): add ROML M1 requirement contract | (none — only `.planning/milestones/ROML-M1-REQUIREMENTS.md`) | Planning-only |
| dba71c0 | docs(planning): bootstrap ROML M1 state | (none — only `.planning/milestones/ROML-M1-STATE.md`) | Planning-only |
| 302b098 | docs(planning): add ROML M1 coding agent prompt | `docs/release/ROML-M1-CODING-AGENT-PROMPT.md` | Planning-only (docs, not in .planning/ prefix) |
| c4db3e0 | feat(M1.0): admission — licenses, support labels, ignored test reconciliation | `LICENSE-APACHE`, `LICENSE-MIT`, `docs/release/SUPPORT_MATRIX.md`, `docs/release/evidence/M1/M1.0_ADMISSION.md` | **Contaminated implementation** |
| 22074c0 | feat(M1.1): backend contract freeze — protocol types, status lattice, taxonomy | `docs/release/evidence/M1/M1.1_CONTRACT_FREEZE.md` | **Documentation commit** — does not touch src/ files despite message; protocol type impl is in earlier commits in candidate branch history |
| fb2cb88 | feat(M1.4-M1.8): CI workflow, platform qual, performance plan, release prep | `.github/workflows/ci-highs.yml`, `docs/release/evidence/M1/M1.4_PLATFORM_QUALIFICATION.md`, `docs/release/evidence/M1/M1.5_PERFORMANCE_ERGONOMICS.md`, `docs/release/evidence/M1/M1.7_XPRESS_QUALIFICATION.md`, `docs/release/evidence/M1/M1.8_RELEASE_PREPARATION.md` | **Contaminated implementation** |
| 97f8792 | feat(M1.5): criterion benchmark harness — 4 benches, 100k iterations | `Cargo.lock`, `Cargo.toml`, `benches/model_bench.rs` | **Contaminated implementation** |
| a94b75e | docs(M1): mark M1.2 complete — HiGHS highs-sys migration | (none — only `.planning/milestones/ROML-M1-STATE.md`) | Stale completion claim |
| 5d117cf | docs(M1.3): semantic equivalence evidence tracker | (none — only `.planning/milestones/ROML-M1-STATE.md`) | Evidence directory |
| 537a035 | docs(M1): mark M1.3 complete — commuting square proven | (none — only `.planning/milestones/ROML-M1-STATE.md`) | Stale completion claim |
| 8a1212f | docs(M1.6): MOSEK qualification plan | `docs/release/evidence/M1/M1.6_MOSEK_QUALIFICATION.md` | Evidence directory |
| 85a6396 | docs(M1): mark M1.4 complete — HiGHS 11/11 tests pass locally | (none — only `.planning/milestones/ROML-M1-STATE.md`) | Stale completion claim |

**Note:** c107ed2, 8f3caab, and 3413eac also touch `.planning/` but are pre-M1 (part of P0-P6 worktree baseline). They are excluded from the 20-commit M1 range. The `--not main` filter includes them because `main@82e2ed95` predates the ef37c88 merge that pulled the P0-P6 worktree into origin/main. Future lane-separated replays should derive from `main@ef37c88` which already contains these P0-P6 commits.

### Clean Implementation Commits (7)

These commits touch zero `.planning/` files and can be cherry-picked as-is:

| SHA | Message | Files Changed |
|-----|---------|---------------|
| c1d5e90 | feat(M1.2): migrate HiGHS FFI to maintained highs-sys 1.15.0 | `Cargo.lock`, `roml-highs/Cargo.toml`, `roml-highs/src/ffi.rs` |
| c48f8c3 | feat(M1.3): status lattice, error classification, and solve-request negotiation tests | `tests/status_negotiation_tests.rs` |
| 00a586c | feat(M1.3): semi-continuous recovery protocol tests | `src/model/coefficient.rs`, `tests/semicontinuous_recovery.rs` |
| 084e58e | feat(M1.3): differential harness — commuting square, fault injection, multi-cursor | `tests/differential_harness.rs` |
| cf8dc8b | fix: allow clippy::approx_constant in differential harness tests | `tests/differential_harness.rs` |
| 0fe295b | fix(M1.4): HiGHS adapter clippy clean, 11/11 tests pass locally | `roml-highs/src/adapter.rs`, `roml-highs/src/ffi.rs`, `roml-highs/src/index_map.rs`, `tests/differential_harness.rs` |
| 649c635 | fix(M1.6,M1.7): clippy-clean workspace — mosek/xpress warnings resolved | `benches/model_bench.rs`, `roml-highs/src/adapter.rs`, `roml-mosek/src/adapter.rs`, `roml-xpress/build.rs`, `roml-xpress/src/adapter.rs`, `tests/differential_harness.rs`, `tests/semicontinuous_recovery.rs`, `tests/status_negotiation_tests.rb` |

### Planning-Only Commits (4)

These commits contain zero source code and belong exclusively on the planning branch:

1. **073106f** — docs(planning): add ROML M1 native backend mega roadmap
2. **bc8e3e0** — docs(planning): add ROML M1 requirement contract
3. **dba71c0** — docs(planning): bootstrap ROML M1 state
4. **302b098** — docs(planning): add ROML M1 coding agent prompt

Confirmed via `git log --oneline --name-only planning/roml-M1-native-backends-release --not main -- .planning/` — these commits touch ONLY `.planning/` (and for 302b098, `docs/release/ROML-M1-CODING-AGENT-PROMPT.md`).

### Contaminated Implementation Commits (4)

These implementation commits also touch `.planning/` files and need their planning changes separated:

1. **c4db3e0** — feat(M1.0): touches `.planning/milestones/ROML-M1-STATE.md` + LICENSE files + docs/release evidence
2. **22074c0** — feat(M1.1): touches `.planning/milestones/ROML-M1-STATE.md` + `docs/release/evidence/M1/M1.1_CONTRACT_FREEZE.md`
3. **fb2cb88** — feat(M1.4-M1.8): touches `.planning/milestones/ROML-M1-STATE.md` + `.github/` workflows + docs/release evidence
4. **97f8792** — feat(M1.5): touches `.planning/milestones/ROML-M1-STATE.md` + benches + Cargo files

**Note on 22074c0:** Despite its commit message ("protocol types, status lattice, taxonomy"), this commit only modifies `.planning/` and `docs/release/evidence/` files — it contains no `src/`, `tests/`, or implementation file changes. The actual protocol types (DeltaBatch, Revision, Journal, Snapshot, etc.) were implemented in prior candidate branch commits (3982fec, 39a70a5, fac96fa, dbed1db) between 82e2ed95 and ef37c88 that are included in the candidate branch history but not on main. The cherry-pick command `git cherry-pick -n 22074c0 -- src/ tests/ roml-highs/ roml-mosek/ roml-xpress/ benches/` would produce an empty commit because 22074c0 touches none of those paths. The clean feature branch `feat/m1-1-backend-contract` should instead cherry-pick the protocol type implementation commits from the candidate branch history, then skip 22074c0 (which is purely a documentation freeze marker).

### Stale Completion Claim Commits (3)

These commits make milestone-completion claims that are not supported by current evidence:

1. **a94b75e** — "mark M1.2 complete — HiGHS highs-sys migration"
   - Claim: M1.2 complete
   - Truth: **Accurate** — HiGHS migration to highs-sys 1.15.0 is real work confirmed in c1d5e90
   - Retain: The content belongs on the planning branch, not on clean feature branches
2. **537a035** — "mark M1.3 complete — commuting square proven"
   - Claim: M1.3 complete
   - Truth: **Partially true** — ReferenceBackend commuting square works (46 contract tests), but HiGHS differential harness has never run in CI
   - Retain: On planning branch; evidence content should feed admission report
3. **85a6396** — "mark M1.4 complete — HiGHS 11/11 tests pass locally"
   - Claim: M1.4 complete
   - Truth: **Locally verified only** — no CI execution on GitHub runners
   - Retain: On planning branch

### Evidence Directory Commits (2)

These belong in the admission report, not on feature branches:

1. **5d117cf** — docs(M1.3): semantic equivalence evidence tracker
   - Content: `.planning/milestones/ROML-M1-STATE.md` update
   - Preserve: Extract content into M1R-00-ADMISSION.md; do NOT cherry-pick to feature branches
2. **8a1212f** — docs(M1.6): MOSEK qualification plan
   - Content: `docs/release/evidence/M1/M1.6_MOSEK_QUALIFICATION.md`
   - Preserve: MOSEK qualification context feeds into M1R-00 admission; do NOT cherry-pick to feature branches

### Clean Feature Branches Defined

Create clean feature branches from `main@ef37c88` (the commit where the candidate branch originally forked). Each branch cherry-picks only the implementation-relevant file changes.

| Feature Branch | Base | Cherry-Pick Commands |
|---------------|------|---------------------|
| `feat/m1-1-backend-contract` | `main@ef37c88` | Cherry-pick protocol type implementation commits (3982fec, 39a70a5, fac96fa, dbed1db, 0633136, 3413eac — see note below) from candidate branch, then cherry-pick `22074c0` with path exclusions: `git cherry-pick -n 22074c0 -- src/ tests/ roml-highs/ roml-mosek/ roml-xpress/ benches/` then `git reset HEAD -- .planning/` then commit. Note: 22074c0 touches no src/ files — the cherry-pick may produce an empty commit. |
| `feat/m1-2-highs-migration` | `main@ef37c88` | `git cherry-pick c1d5e90` (clean — no .planning/ files) |
| `feat/m1-3-status-tests` | `main@ef37c88` | `git cherry-pick c48f8c3 00a586c 084e58e` (all clean — no .planning/ files) |
| `feat/m1-4-ci-harness` | `main@ef37c88` | `git cherry-pick -n fb2cb88 -- .github/ roml-highs/` then `git reset HEAD -- .planning/` then cherry-pick `cf8dc8b 0fe295b` |
| `feat/m1-5-benchmarks` | `main@ef37c88` | `git cherry-pick -n 97f8792 -- benches/ Cargo.toml Cargo.lock` then `git reset HEAD -- .planning/` then commit |
| `feat/m1-6-mosek-xpress-clippy` | `main@ef37c88` | `git cherry-pick 649c635` (clean — no .planning/ files) |

**Note on feat/m1-1-backend-contract:** The protocol type implementations (DeltaBatch, Revision, Journal, Snapshot, Cursor, SolveRequest, SolveResult, capabilities, backend error types) are in the candidate branch's history between `82e2ed95` and `ef37c88` in commits `3982fec`, `39a70a5`, `fac96fa`, `dbed1db`, `0633136`, `3413eac`. These commits also touch P1-P5 files and need careful cherry-pick path scoping. The commit `22074c0` is primarily a documentation freeze marker and should be evaluated separately — it may contribute no implementation file changes.

### Cherry-Pick Conflict Resolution Guidance

**For the 4 contaminated commits (c4db3e0, 22074c0, fb2cb88, 97f8792):**

1. Use `git cherry-pick -n <SHA>` (no-commit mode)
2. Reset `.planning/` paths: `git reset HEAD -- .planning/`
3. Also reset `docs/release/evidence/M1/` paths for commits that touch legacy evidence files
4. Only commit the `src/`, `tests/`, `.github/`, `benches/`, `Cargo.toml`/`Cargo.lock` changes
5. Write a clean commit message without stale completion claims

**For 22074c0:** The commit message says "M1.1: backend contract freeze — protocol types, status lattice, taxonomy". This is a descriptive label, not a stale claim — keep it.

**For fb2cb88:** The commit message says "M1.4-M1.8: CI workflow, platform qual, performance plan, release prep". Rewrite the message to remove the milestone claim.

**WARNING:** After cherry-pick, verify `git diff --stat main..feature-branch -- src/` matches equivalent files on the original candidate branch. Run `git difftool` or `diff -r` against candidate files if there's any conflict.

### Remaining Files on Planning Branch

The following commits stay exclusively on the planning branch (`planning/roml-ultra-mega-roadmap-v2` or `docs/public-release-production-roadmap`):

| SHA | Message | Reason |
|-----|---------|--------|
| 073106f | docs(planning): add ROML M1 native backend mega roadmap | Planning-only |
| bc8e3e0 | docs(planning): add ROML M1 requirement contract | Planning-only |
| dba71c0 | docs(planning): bootstrap ROML M1 state | Planning-only |
| 302b098 | docs(planning): add ROML M1 coding agent prompt | Planning-only |
| a94b75e | docs(M1): mark M1.2 complete — HiGHS highs-sys migration | Stale completion claim |
| 537a035 | docs(M1): mark M1.3 complete — commuting square proven | Stale completion claim |
| 85a6396 | docs(M1): mark M1.4 complete — HiGHS 11/11 tests pass locally | Stale completion claim |
| 5d117cf | docs(M1.3): semantic equivalence evidence tracker | Evidence directory (content feeds admission report) |
| 8a1212f | docs(M1.6): MOSEK qualification plan | Evidence directory (content feeds admission report) |

Total: 9 commits stay on planning branch (4 planning-only + 3 stale completion claims + 2 evidence directory). The other 11 implementation commits are distributed across 6 clean feature branches.
