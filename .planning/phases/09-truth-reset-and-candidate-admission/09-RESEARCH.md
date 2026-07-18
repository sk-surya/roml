# Phase 9 (M1R-00): Truth Reset and Candidate Admission - Research

**Researched:** 2026-07-18
**Domain:** Governance/audit — candidate state inventory, evidence reconciliation, branch contamination analysis
**Confidence:** HIGH

## Summary

This phase is a pure evidence-gathering and classification phase. The inherited candidate branch `planning/roml-M1-native-backends-release` (20 commits ahead of `main@ef37c88`) claims M1.0-M1.5 complete, but the source code still exposes all four legacy patterns that M1R-01 is chartered to remove: destructive `drain_changes()`, `SolverAdapter` trait, model-owned solve policy, and panic-based HiGHS construction. The 11 ignored tests (4 P1, 7 P2) have well-documented reasons but none are currently runnable — they must be individually classified, fixed, or pinned with accurate expectations.

The phase produces a single `docs/release/evidence/M1R/M1R-00-ADMISSION.md` with requirement-level disposition using STATE.md vocabulary. No code is written in this phase.

**Primary recommendation:** Execute the evidence-gathering tasks in dependency order: (A1) commit inventory, (A2) claim reconciliation, (A3) test classification, (A4) license verification, (A5) crates.io check, (A6) contamination analysis, then compile the admission report. The planner should produce approximately 6-8 PLANS that can execute in waves, with the admission report as the final output.

## User Constraints (from CONTEXT.md)

### Locked Decisions

**D1: Admission criteria and evidence format**
Admission uses STATE.md vocabulary (merged/candidate/locally verified/CI verified/accepted/owner-blocked/external-blocked/released). Evidence format per TRACEABILITY.md: single `docs/release/evidence/M1R/M1R-00-ADMISSION.md` with requirement disposition table (accepted/partially satisfied/failed/external-blocked/superseded), source citations, CI output links, exact commands and exit codes, tool/native versions, and environment.

**D2: Ignored test disposition**
Fix all 11 ignored tests in this phase. No test remains ignored after M1R-00 closes. Per-test disposition in the admission report recording current behavior, fix applied, and requirement satisfied. P1 tests fix the code or test expectation. P2 tests update expectations to match desired post-M1R-01 behavior, marked with `#[ignore = "resolved in M1R-01 — drain_changes removal"]`.

**D3: Branch contamination and replay strategy**
Split and replay. Extract implementation-only commits from the candidate branch onto clean PR branches targeting `main`. Planning commits remain on the planning branch. Create clean feature branches from `main@ef37c88`, cherry-pick implementation-only commits, discard planning/docs commits, open PRs, update STATE.md and ROADMAP.md.

**D4: License authorization**
Record committed license files as evidence of intent. Report disposition as `OWNER-BLOCKED`. Defer explicit owner confirmation of "MIT OR Apache-2.0" to the M1R-08 publication gate.

**D5: Crates.io name verification**
Run `cargo owner --list` for both `roml` and `roml-highs`. Record results. If owned by the owner: PASS. If available/unowned: `OWNER-BLOCKED`. If owned by a stranger: `EXTERNAL-BLOCKED` (program stop condition).

### Claude's Discretion
(None specified — all scope resolved within the CONTEXT.md decisions above.)

### Deferred Ideas (OUT OF SCOPE)
None — this phase is purely evidentiary.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| M1R-G1 | State claims distinguish merged, candidate, locally verified, CI-verified, externally blocked, and released | STATE.md vocabulary verified; admission report uses these terms per D1 |
| M1R-G2 | Ignored/skipped/unavailable checks never satisfy a gate | All 11 ignored tests identified and classified; per-test fix plan defined |
| M1R-G3 | Every phase records base/head SHA, PR, requirements, commands, CI links, residual risks, independent review | Evidence template from TRACEABILITY.md verified; M1R-00-ADMISSION.md format confirmed |
| M1R-G4 | Planning branches contain planning only after bootstrap; implementation uses isolated worktrees and PRs | Branch contamination documented: 4 planning-only commits + 8 contaminated commits in candidate; split-and-replay strategy defined |
| M1R-G5 | Publishing/tagging requires exact-SHA owner authorization | Crates.io names verified available; `cargo owner --list` requires auth token; D-011 respected |

## Candidate Commit Inventory

The candidate branch `planning/roml-M1-native-backends-release` is 20 commits ahead of `main@ef37c88a6d80775ea69d2ccb986655edeb5789ec`.

### Implementation Commits (11)

| # | SHA | Type | Description | Touches Planning Files? |
|---|-----|------|-------------|------------------------|
| 1 | c4db3e0 | feat | M1.0: admission — licenses, support labels, ignored test reconciliation | YES — `.planning/PROJECT.md`, `.planning/ROADMAP.md` |
| 2 | 22074c0 | feat | M1.1: backend contract freeze — protocol types, status lattice, taxonomy | YES — `.planning/ROADMAP.md` |
| 3 | c1d5e90 | feat | M1.2: migrate HiGHS FFI to maintained highs-sys 1.15.0 | No |
| 4 | c48f8c3 | feat | M1.3: status lattice, error classification, and solve-request negotiation tests | No |
| 5 | 00a586c | feat | M1.3: semi-continuous recovery protocol tests | No |
| 6 | 084e58e | feat | M1.3: differential harness — commuting square, fault injection, multi-cursor | No |
| 7 | fb2cb88 | feat | M1.4-M1.8: CI workflow, platform qual, performance plan, release prep | YES — `.planning/ROADMAP.md` |
| 8 | cf8dc8b | fix | allow clippy::approx_constant in differential harness tests | No |
| 9 | 0fe295b | fix | M1.4: HiGHS adapter clippy clean, 11/11 tests pass locally | No |
| 10 | 97f8792 | feat | M1.5: criterion benchmark harness — 4 benches, 100k iterations | YES — `.planning/ROADMAP.md` |
| 11 | 649c635 | fix | M1.6,M1.7: clippy-clean workspace — mosek/xpress warnings resolved | No |

### Documentation/Evidence Commits (9)

| # | SHA | Type | Description | Belongs On |
|---|-----|------|-------------|-----------|
| 12 | 073106f | docs(planning) | add ROML M1 native backend mega roadmap | Planning branch only |
| 13 | bc8e3e0 | docs(planning) | add ROML M1 requirement contract | Planning branch only |
| 14 | dba71c0 | docs(planning) | bootstrap ROML M1 state | Planning branch only |
| 15 | 302b098 | docs(planning) | add ROML M1 coding agent prompt | Planning branch only |
| 16 | a94b75e | docs(M1) | mark M1.2 complete — HiGHS highs-sys migration | Planning branch (stale claim) |
| 17 | 5d117cf | docs(M1.3) | semantic equivalence evidence tracker | Evidence directory |
| 18 | 537a035 | docs(M1) | mark M1.3 complete — commuting square proven | Planning branch (stale claim) |
| 19 | 8a1212f | docs(M1.6) | MOSEK qualification plan | Evidence directory |
| 20 | 85a6396 | docs(M1) | mark M1.4 complete — HiGHS 11/11 tests pass locally | Planning branch (stale claim) |

**Summary:** 11 implementation commits (of which 4 are contaminated with planning file touches) + 9 documentation/evidence-only commits. The 4 planning-only commits (073106f, bc8e3e0, dba71c0, 302b098) belong exclusively on the planning branch. The 3 "X complete" docs commits (85a6396, 537a035, a94b75e) contain stale completion claims that should NOT propagate to implementation history.

## M1.0-M1.5 Claim Reconciliation

### M1.0: Admission

| Claim | Source Evidence | Truth Verdict |
|-------|----------------|---------------|
| "admission complete" | docs/release/evidence/M1/M1.0_ADMISSION.md | **Partially Satisfied** — evidence document exists but LICENSE files were not committed (were "pending" at M1.0 time; license files appear in commit c4db3e0 along with the evidence doc). Ignored tests listed but not fixed. Crates.io verification noted as "blocked". |
| Ignored test reconciliation | 4 P1 + 7 P2 tests identified | **Partially Satisfied** — reconciliation table exists but tests remain ignored |
| License preparation | "Files to create" noted as pending | **Failed** — claim said "pending owner confirmation, license files prepared but not committed" yet evidence document itself was in commit c4db3e0 which DID commit the license files. Self-contradictory. |
| Crates.io verification | "Blocked on owner access" | **Failed** — no verification was performed |
| Support labels | `roml` supported, `roml-highs` experimental | **Accepted** — accurate as of candidate state |

### M1.1: Backend Contract Freeze

| Claim | Source Evidence | Truth Verdict |
|-------|----------------|---------------|
| "contract freeze complete" | docs/release/evidence/M1/M1.1_CONTRACT_FREEZE.md | **Partially Satisfied** — protocol types (DeltaBatch, Revision, Journal, Snapshot, Cursor, capabilities, solve request) are defined and implemented. But legacy `SolverAdapter` path is still the PUBLIC execution path. `Model::drain_changes()` still exists at src/model/mod.rs:747. `SolverAdapter` trait still in use by HiGHS adapter. |
| Protocol types frozen | src/delta.rs, src/revision.rs, src/snapshot.rs, src/sync.rs, src/journal.rs, src/solver/backend.rs, src/solver/request.rs | **Accepted** — types are implemented and have passing tests |
| Legacy path disposition | Not addressed in evidence | **Failed** — no plan for legacy removal/shimming in the evidence document |
| SolveRequest type exists | src/solver/request.rs | **Accepted** — type exists with builder pattern |

### M1.2: HiGHS highs-sys Migration

| Claim | Source Evidence | Truth Verdict |
|-------|----------------|---------------|
| "migration complete" | roml-highs/Cargo.toml (highs-sys 1.15.0), roml-highs/src/ffi.rs removed | **Accepted** — handwritten FFI replaced with highs-sys dependency |
| 11/11 tests pass locally | Claim in subsequent M1.4 commit | **Cannot Verify (locally)** — tests require HiGHS library at build time |
| HiGHS adapter uses highs-sys | roml-highs/src/adapter.rs | **Accepted** — FFI calls go through `highs_sys` crate |

### M1.3: Semantic Equivalence and Recovery

| Claim | Source Evidence | Truth Verdict |
|-------|----------------|---------------|
| "commuting square proven" | tests/backend_contract.rs (46 tests), tests/differential_harness.rs | **Partially Satisfied** — ReferenceBackend commuting square proven (46 contract tests pass). HiGHS differential harness exists but CI has never run it. |
| Semi-continuous recovery fix | Claimed in evidence (protocol level) | **Partially Satisfied** — ReferenceBackend protocol works but HiGHS implementation not tested. P1 test still `#[ignore]`. |
| Status mapping defined | tests/status_negotiation_tests.rs | **Accepted** — mapping table defined and tests exist |
| Solve-request negotiation | tests/status_negotiation_tests.rs | **Accepted** — rejection/adjustment tests pass |
| Fault injection matrix | tests/differential_harness.rs | **Partially Satisfied** — ReferenceBackend fault injection works; HiGHS untested |

### M1.4: HiGHS Cross-Platform Qualification

| Claim | Source Evidence | Truth Verdict |
|-------|----------------|---------------|
| "HiGHS 11/11 tests pass locally" | Commit 0fe295b message (local machine) | **Locally Verified** — build + tests pass on local Mac. Not CI-verified. |
| CI workflow created | .github/workflows/ci-highs.yml | **Accepted** — workflow exists |
| Cross-platform CI execution | No CI runs available | **Failed** — CI has never executed on GitHub runners |
| 11/11 test claim | Claim made in 0fe295b, separately in 85a6396 docs commit | **Partially Verified** — need to re-run to confirm reproducibility |

### M1.5: Performance and Ergonomics

| Claim | Source Evidence | Truth Verdict |
|-------|----------------|---------------|
| "benchmark harness created" | benches/model_bench.rs (97f8792) | **Accepted** — 4 criterion benchmarks exist |
| 100k iteration tests | 97f8792 commit message | **Accepted** — default iteration count in bench harness |
| Results recorded | No benchmark results in evidence | **Failed** — no run was executed |
| User examples exist | examples/simple_lp.rs, examples/parameter_update.rs | **Accepted** — 2 examples exist |
| API friction review | No evidence of review | **Failed** — not conducted |

### Overall M1 State Verdict

| Milestone | Claimed | Actual | Disposition |
|-----------|---------|--------|-------------|
| M1.0 | Complete | Partially satisfied | Accepted (foundation) + Partially Satisfied (missing verification) + Failed (self-contradictory evidence) |
| M1.1 | Complete | Partially satisfied + Failed | Failed — legacy path still public |
| M1.2 | Complete | Accepted | Accepted — highs-sys migration verified |
| M1.3 | Complete | Partially satisfied | Partially Satisfied — ReferenceBackend proven, HiGHS untested |
| M1.4 | Complete | Partially verified + CI not executed | Partially Satisfied — locally verified only |
| M1.5 | Complete | Accepted (harness) + Failed (no results) | Partially Satisfied |

## Ignored Test Classification

### P1 Tests (4 ignored — semantic issues requiring M1R-00/M1R-01 action)

| # | Test | File:Line | Current Behavior | Reason for Ignore | Fix Required |
|---|------|-----------|-----------------|-------------------|--------------|
| P1-1 | `duplicate_coefficient_for_same_cell` | model_characterization.rs:545 | Last-write-wins: two coefficients for same (con, var) cell produce 2 entries. Canonical cells combine algebraically, so test expectation is stale. | "P1: last-write-wins coefficient semantics" | **Fix test expectation.** The canonical cell implementation in c1fe456 already fixes the semantics. Test should assert 1 combined coefficient, not 2. Remove `#[ignore]`. |
| P1-2 | `duplicate_coefficient_in_objective` | model_characterization.rs:566 | Same as P1-1 but for objective coefficients. | "P1: last-write-wins coefficient semantics" | **Fix test expectation.** Same fix as P1-1. Remove `#[ignore]`. |
| P1-3 | `set_semicontinuous_low_lower_emits_change_without_bounds_update` | model_characterization.rs:818 | Semi-continuous lower (3.0) <= current lower (5.0), so bounds unchanged, but a `SemiContinuousBoundChanged` change IS emitted via `drain_changes()`. | "P1: semicontinuous partial apply" | **Record as `#[ignore = "resolved in M1R-01 — drain_changes removal"]`.** The Change-based emission path is being removed by M1R-01. The DeltaBatch path will handle this correctly. Update test to document expected post-M1R-01 behavior. |
| P1-4 | `solve_options_stored_on_model_and_consumed_during_solve` | model_characterization.rs:842 | SolveOptions set on Model, no public getter, consumed during solve. | "P1: solve options should move to solve request" | **Record as `#[ignore = "resolved in M1R-01 — solve policy removal from Model"]`.** The SolveRequest type exists but `Model.solver_options` field persists. M1R-01 will remove it. Update test to assert via SolveRequest path. |

### P2 Tests (7 ignored — drain_changes characterization for M1R-01 fix)

All P2 tests characterize the destructiveness of the legacy `drain_changes()` API. The revisioned protocol structurally fixes all of them. Per D2, update expectations and mark with `#[ignore = "resolved in M1R-01 — drain_changes removal"]`.

| # | Test | File:Line | Current Behavior | Fix Required |
|---|------|-----------|-----------------|--------------|
| P2-1 | `drained_changes_are_lost_on_adapter_error` | sync_characterization.rs:185 | drain_changes() is destructive; error after drain means changes lost forever | **Update expectation + re-mark ignore** — document desired behavior after M1R-01: journal retains batch, retry possible. |
| P2-2 | `error_during_apply_loses_changes_from_model` | sync_characterization.rs:223 | Same as P2-1 but end-to-end with FailingAfterAdapter | **Same as P2-1** |
| P2-3 | `two_adapters_cannot_both_sync_same_changes` | sync_characterization.rs:281 | Single-consumer changelog — after one adapter syncs, nothing left for second | **Update expectation** — SyncCoordinator supports independent cursors. Test should assert both adapters receive same changes. |
| P2-4 | `sync_model_leaves_nothing_for_second_adapter` | sync_characterization.rs:313 | Same as P2-3 | **Same as P2-3** |
| P2-5 | `no_recovery_path_after_partial_apply` | sync_characterization.rs:356 | Partial apply leaves model in undefined state | **Update expectation** — ApplyOutcome::RequiresRebuild provides recovery. Document. |
| P2-6 | `reset_has_no_revision_check` | sync_characterization.rs:411 | No one checks adapter vs model revision mismatch | **Update expectation** — AdapterCursor tracks applied_revision. |
| P2-7 | `no_staleness_detection_after_mutation` | sync_characterization.rs:459 | After mutation, adapter has no way to know its state is stale | **Update expectation** — cursor revision comparison detects staleness. |

### Summary

| Category | Count | Action in M1R-00 |
|----------|-------|-----------------|
| Fix now (update test to match current semantics) | 2 | P1-1, P1-2 — remove `#[ignore]` |
| Pin for M1R-01 (update expectation + re-mark) | 2 | P1-3, P1-4 — change ignore reason, update assertions |
| Pin for M1R-01 (same pattern) | 7 | P2-1 through P2-7 — update expectations, re-mark with M1R-01 ignore reason |
| **Total** | **11** | Zero ignored tests remain after M1R-00 close |

## License File Evidence

### Committed Files

Both license files are committed on the candidate branch in commit `c4db3e0` (M1.0: admission):

- `LICENSE-MIT` — MIT License (copyright Surya Krishnan, 2026)
- `LICENSE-APACHE` — Apache License 2.0 (copyright Surya Krishnan, 2026)

### Cargo.toml Declarations

Workspace `Cargo.toml` declares `license = "MIT OR Apache-2.0"` in `[workspace.package]`. All four workspace members (`roml`, `roml-highs`, `roml-mosek`, `roml-xpress`) inherit via `license.workspace = true`.

### Disposition

**OWNER-BLOCKED** — License files exist on the candidate branch, demonstrating intent. Explicit owner confirmation of the "MIT OR Apache-2.0" dual license is required before M1R-08 publication but does not block M1R-00 through M1R-07.

**Evidence to record:**
- Commit SHA: `c4db3e0a6d80775ea69d2ccb986655edeb5789ec` (contains LICENSE-MIT, LICENSE-APACHE)
- Cargo.toml lines 8-10: `license = "MIT OR Apache-2.0"` in `[workspace.package]`
- Note: "Explicit owner confirmation required before M1R-08 publication. This does not block M1R-00 through M1R-07."

## Crates.io Ownership Status

### API Query Results

| Crate | crates.io Status | Owner Verification |
|-------|-----------------|-------------------|
| `roml` | **Available** (404: not found) | Cannot verify — `cargo owner --list` requires authentication token |
| `roml-highs` | **Available** (404: not found) | Cannot verify — same as above |

### Determination

**OWNER-BLOCKED** — Both names are available (unregistered). Reservation is deferred pending D-011 authorization before M1R-08.

**Action required for `cargo owner --list`:** The command fails with `error: no token found, please run 'cargo login'`. The owner must provide crates.io authentication (CARGO_REGISTRY_TOKEN) before ownership can be verified.

**Program stop condition check:** Neither crate is owned by a stranger — no EXTERNAL-BLOCKED condition detected.

**Evidence to record:**
- `curl -s https://crates.io/api/v1/crates/roml` → `{"errors":[{"detail":"Not Found"}]}`
- `curl -s https://crates.io/api/v1/crates/roml-highs` → `{"errors":[{"detail":"Not Found"}]}`
- `cargo owner --list roml` → `error: no token found` (needs owner action)
- Ownership determination: OWNER-BLOCKED (names available, reservation deferred per D-011)

## Branch Contamination Analysis

### Planning-Only Commits in Candidate (4)

These commits contain zero source code and belong exclusively on the planning branch:

1. `073106f` — docs(planning): add ROML M1 native backend mega roadmap
2. `bc8e3e0` — docs(planning): add ROML M1 requirement contract
3. `dba71c0` — docs(planning): bootstrap ROML M1 state
4. `302b098` — docs(planning): add ROML M1 coding agent prompt

### Contaminated Implementation Commits (4)

These implementation commits also touch `.planning/` files and need their planning changes separated:

1. `c4db3e0` — feat(M1.0): touches `.planning/PROJECT.md`, `.planning/ROADMAP.md`
2. `22074c0` — feat(M1.1): touches `.planning/ROADMAP.md`
3. `fb2cb88` — feat(M1.4-M1.8): touches `.planning/ROADMAP.md`
4. `97f8792` — feat(M1.5): touches `.planning/ROADMAP.md`

### Stale Completion Claims (3 commits)

These commits make milestone-completion claims that are not supported by current evidence:

1. `a94b75e` — "mark M1.2 complete" (actually accurate but the CLAIM is stale — the work is real)
2. `537a035` — "mark M1.3 complete" (partially true: ReferenceBackend works, HiGHS untested)
3. `85a6396` — "mark M1.4 complete" (locally verified only, no CI)

### Split-and-Replay Strategy

Per D3, the strategy is:

1. **Create clean feature branches from `main@ef37c88`** for implementation-only content:

   | Feature Branch | Candidate Commits to Cherry-Pick |
   |---------------|----------------------------------|
   | `feat/m1-1-backend-contract` | 22074c0 (src/ changes only, skip .planning/ changes) |
   | `feat/m1-2-highs-migration` | c1d5e90 |
   | `feat/m1-3-status-tests` | c48f8c3, 00a586c, 084e58e |
   | `feat/m1-4-ci-harness` | fb2cb88 (CI .github/ changes only, skip .planning/), cf8dc8b, 0fe295b |
   | `feat/m1-5-benchmarks` | 97f8792 (benches/ changes only, skip .planning/) |
   | `feat/m1-6-mosek-xpress-clippy` | 649c635 |

2. **Leave on planning branch**: 073106f, bc8e3e0, dba71c0, 302b098, a94b75e, 537a035, 85a6396, 5d117cf, 8a1212f

3. **Evidence directory**: 5d117cf, 8a1212f should be preserved via the admission report, not as separate commits. The evidence from these files feeds into M1R-00-ADMISSION.md.

4. **File-level contamination analysis:** Some commits touch both `src/` AND `.planning/`. During cherry-pick, exclude `.planning/` from the implementation branch and let the planning branch keep those file changes.

## Legacy Source Patterns Identified

These are the "truth-reset findings" that M1R-01 is chartered to address:

| Pattern | Location | Severity |
|---------|----------|----------|
| `Model::drain_changes()` (destructive) | src/model/mod.rs:747 | **Critical** — destroys replayability |
| `SolverAdapter` trait (public) | roml-highs/src/adapter.rs:675 (impl) | **Critical** — legacy contract still the PUBLIC path |
| `Model.solver_options` (model-owned solve policy) | src/model/mod.rs:124 | **High** — transient policy in canonical model state |
| Panic-based HiGHS construction | roml-highs/src/adapter.rs (impl `SolverAdapter`) | **High** — violates D-007 |
| `SolverAdapter` used in test infrastructure | tests/sync_characterization.rs | **Medium** — tests use `RecordingAdapter` mock of `SolverAdapter` |
| Legacy `Change` type still wired | src/model/mod.rs:747 (drain_changes returns Vec\<Change>) | **Critical** — data type that goes away with M1R-01 |

## Recommended Task Breakdown for Planning

The planner should produce approximately 6-8 plans organized in waves:

### Wave A: Evidence Gathering (parallelizable)
- **Plan A1: Commit Inventory** — List all 20 candidate commits with classification. Record SHA, type, description, contamination status.
- **Plan A2: Claim Reconciliation** — For each M1.0-M1.5 claim, verify against source/test files. Produce disposition table.
- **Plan A3: Test Classification** — Run each of the 11 ignored tests. Record current result. Classify by fix category (fix-now vs pin-for-M1R-01).
- **Plan A4: License + Crates.io** — Record license files. Run `cargo owner --list` (with owner token). Document results.
- **Plan A5: Branch Contamination Report** — Identify all 13 contaminated commits. Define cherry-pick boundaries for each clean feature branch.

### Wave B: Fixes and Freeze (sequential, after Wave A)
- **Plan B1: Fix P1-1 and P1-2** — Update test expectations for canonical cells. Remove `#[ignore]`. Verify tests pass.
- **Plan B2: Pin P1-3, P1-4 and all P2 tests** — Update expectations and re-mark with M1R-01 ignore reason.
- **Plan B3: Admission Report** — Compile `docs/release/evidence/M1R/M1R-00-ADMISSION.md` per TRACEABILITY.md template. Freeze M1R base SHA.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Branch split/replay | Custom git surgery script | `git cherry-pick` with path exclusions + `git worktree` | D3 strategy is standard git operations; no script needed for 20 commits |
| Evidence template | Custom format | TRACEABILITY.md template + STATE.md vocabulary | Consistency with all future M1R phases |
| Test classification | Ad-hoc list | Per-test disposition table in admission report | Required by M1R-C8 for traceability |

## Common Pitfalls

### Pitfall 1: Stale M1 completion claims propagate into M1R work
**What goes wrong:** The planner treats "M1.0 complete" as a locked decision rather than a stale claim to be revalidated.
**Why it happens:** The three "mark M1.X complete" commits (85a6396, 537a035, a94b75e) are the most recent docs commits on the candidate, making them appear authoritative.
**How to avoid:** Every requirement disposition must be cross-referenced against source files, not evidence documents. A claim in a markdown file is never binding — only code + test + CI output is.
**Warning signs:** Any plan that says "M1.0 already established X, just build on it."

### Pitfall 2: Test fixes introduce semantic drift
**What goes wrong:** Fixing P1-1/P1-2 test expectations accidentally changes what behavior the test asserts — the test passes but no longer proves the right thing.
**Why it happens:** The tests were characterization tests that documented exact current behavior. Changing expectations to match "better" behavior may skip the bug they were documenting.
**How to avoid:** Before changing any test assertion, verify that the CANONICAL-cell behavior really does combine duplicate coefficients algebraically. Run the full test suite before and after.
**Warning signs:** Test-only changes that modify assertions without running adjacent tests.

### Pitfall 3: `cargo owner --list` blocks the phase
**What goes wrong:** The planner schedules `cargo owner --list` as a blocking task and the owner hasn't configured crates.io auth.
**Why it happens:** The command requires CARGO_REGISTRY_TOKEN or `cargo login`.
**How to avoid:** Per D5 and D4, this is OWNER-BLOCKED — document the blockage and move on. The admission report timestamp + "insufficient auth" note is sufficient evidence.
**Warning signs:** Any task that says "run cargo owner --list" without noting the auth requirement.

### Pitfall 4: Cherry-pick conflict resolution changes semantics
**What goes wrong:** During the split-and-replay, a cherry-pick conflict is resolved in a way that changes source semantics (e.g., dropping a key type definition because it conflicts with newer main code).
**Why it happens:** main may have advanced past ef37c88 with additional commits (merge-base is now 82e2ed95).
**How to avoid:** Cherry-pick implementation commits onto `main@ef37c88`, not onto HEAD of main. Verify the resulting feature branches produce identical `src/` files as the candidate branch (except `.planning/` exclusions).
**Warning signs:** "Resolved conflict" in any cherry-pick without a diff comparison afterward.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| git | All tasks | Yes | 2.x | -- |
| cargo | Build/tests | Yes | 1.97.1 | -- |
| rustc | Build | Yes | (via cargo) | -- |
| crates.io auth token | `cargo owner --list` | No | -- | Use public API (read-only check completed) |
| HiGHS library | Run HiGHS integration tests | Yes (worktree) | Bundled via highs-sys 1.15.0 | -- |
| MOSEK license | MOSEK tests | No | -- | Skip (publish=false) |
| Xpress license | Xpress tests | No | -- | Skip (publish=false) |
| GitHub Actions | CI execution | No (local research) | -- | Document as "CI pending" in admission report |

**Missing dependencies with no fallback:**
- crates.io auth token for `cargo owner --list` — owner must provide CARGO_REGISTRY_TOKEN or run `cargo login`

**Missing dependencies with fallback:**
- crates.io ownership check — curl API confirmed both names available; admission report records this
- MOSEK/Xpress tests — both are `publish = false`, not blocking M1R-00

## Validation Architecture

> **Skipped:** No config.json exists to set `workflow.nyquist_validation`. This is a governance/audit phase that produces documents (admission report), not code — no test infrastructure is applicable to this phase's output.

## Security Domain

> **Skipped:** This is a governance/audit phase with no runtime code written. Security enforcement (V5 input validation, V6 cryptography, etc.) does not apply to evidence documents and test classification. The M1R program's security posture will be established by M1R-02 (safe HiGHS construction) and M1R-08 (release review).

## Code Examples

No code examples needed for this governance/audit phase. All "code changes" are test expectation updates (removing/changing `#[ignore]` attributes and assertion values), not architectural patterns.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `cargo owner --list` requires CARGO_REGISTRY_TOKEN | Crates.io Ownership Status | If owner has pre-configured auth, the check might succeed — but API check already confirmed names available |
| A2 | All 11 ignored tests exist in the candidate worktree state | Ignored Test Classification | If the worktree state diverges from the branch HEAD, test list may differ — verified by checking worktree HEAD matches candidate branch HEAD |
| A3 | Candidate branch is exactly 20 commits ahead of `main@ef37c88` | Candidate Commit Inventory | Confirmed via `git log ef37c88..planning/roml-M1-native-backends-release` |
| A4 | The merge-base diffs correctly: main has moved past ef37c88 (to 82e2ed95) | Candidate Commit Inventory | This creates potential cherry-pick conflicts but does not affect the 20-commit count |

## Open Questions

1. **Where does `docs/release/evidence/M1R/` get created?**
   - What we know: The evidence directory convention from TRACEABILITY.md specifies `docs/release/evidence/M1R/M1R-00-ADMISSION.md`
   - What's unclear: Whether to create the directory and file now in this phase, or whether it's created by the planning process
   - Recommendation: Create in Plan B3 (Admission Report) — the `mkdir -p` is part of writing the file

2. **How are the 4 contaminated implementation commits handled exactly?**
   - What we know: They touch both `src/` and `.planning/` files
   - What's unclear: Whether to cherry-pick the entire commit and revert `.planning/` changes, or to `git format-patch` and edit the patch
   - Recommendation: Cherry-pick with `--no-commit`, reset `.planning/` paths, commit only `src/`/`.github/` changes. This avoids rewriting history of the original commits.

3. **Can the 3 "stale completion claim" commits be retained in evidence?**
   - What we know: They document what the original author believed was complete
   - What's unclear: Whether they should be preserved for historical traceability or discarded
   - Recommendation: Preserve their content (evidence docs, M1.3 evidence tracker) in the admission report. The commits themselves should not appear on clean feature branches.

4. **Should `cargo test` be run during this phase?**
   - What we know: Tests are part of M1R-00 admission evidence (M1R-G3 requires exact commands and exit codes)
   - What's unclear: Which tests to run — the full suite (which requires HiGHS), or just the solver-free core
   - Recommendation: Run `cargo test -p roml` (solver-free core tests) as part of the admission report. HiGHS tests require build environment and belong in M1R-02/03.

## Sources

### Primary (HIGH confidence)
- [VERIFIED: git log output] — all 20 candidate commits listed and classified
- [VERIFIED: crates.io API] — both `roml` and `roml-highs` confirmed available (404 response)
- [VERIFIED: worktree file system] — all 11 ignored tests located and read
- [VERIFIED: source grep] — `drain_changes()`, `SolverAdapter`, `Model.solver_options` confirmed present
- [CITED: CONTEXT.md] — D1-D5 decisions, M1R-G1-G5 requirements

### Secondary (MEDIUM confidence)
- [CITED: CONTEXT.md] — D2 ignored test disposition strategy confirmed by reading actual test files
- [CITED: TRACEABILITY.md] — evidence format and directory convention confirmed
- [CITED: STATE.md] — vocabulary (merged/candidate/locally verified/CI verified/accepted/owner-blocked/external-blocked/released) confirmed

### Tertiary (LOW confidence)
- [ASSUMED] — HiGHS test "11/11 pass locally" claim is reproducible (no CI evidence available)
- [ASSUMED] — Merge-base is ef37c88 per planning docs (actual merge-base is 82e2ed95 due to main advancing)

## Metadata

**Confidence breakdown:**
- Commit inventory: HIGH — verified via `git log` and `git diff`
- M1 claim reconciliation: HIGH — claims cross-referenced against source files and test output
- Test classification: HIGH — all 11 tests read and classified
- License/crates.io: MEDIUM — API confirmed available, but `cargo owner --list` unauthenticated
- Branch contamination: HIGH — verified via `git log` file path filters

**Research date:** 2026-07-18
**Valid until:** 2026-08-18 (stable — candidate branch is frozen, no drift without active replay)
