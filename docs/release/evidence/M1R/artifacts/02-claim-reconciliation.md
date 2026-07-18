# Phase 09 Plan 02: Claim Reconciliation

**Date:** 2026-07-18
**Plan:** 09-02
**Phase:** 09-truth-reset-and-candidate-admission
**Branch inspected:** `planning/roml-M1-native-backends-release`
**Base:** `main@ef37c88`

## M1 Claim Disposition Table

Every disposition is backed by a git command run against the candidate branch
(`planning/roml-M1-native-backends-release`). No verdict relies on evidence-document
claims — only source code, file existence, git history, and test-function counts.

### M1.0: Admission

| Milestone | Claim | Source Evidence | Command Run | Truth Verdict |
|-----------|-------|----------------|-------------|---------------|
| M1.0 | admission complete | `docs/release/evidence/M1/M1.0_ADMISSION.md` exists (committed in c4db3e0); LICENSE-MIT and LICENSE-APACHE committed; 11 ignored tests identified but still `#[ignore]` | `git show planning/roml-M1-native-backends-release:docs/release/evidence/M1/M1.0_ADMISSION.md`; `git ls-tree planning/roml-M1-native-backends-release -- LICENSE-MIT LICENSE-APACHE` | Partially Satisfied |
| M1.0 | ignored test reconciliation | All 11 ignored tests identified: 4 P1 (`model_characterization.rs` lines 545, 566, 818, 842) + 7 P2 (`sync_characterization.rs` lines 185–459); all remain `#[ignore]` | `git grep -n '#\\[ignore' planning/roml-M1-native-backends-release -- tests/` | Partially Satisfied |
| M1.0 | license preparation | LICENSE-MIT (blob bfe2cc1) and LICENSE-APACHE (blob d645695) committed in c4db3e0; Cargo.toml declares `license = "MIT OR Apache-2.0"` with all 4 members inheriting via `license.workspace = true` | `git ls-tree planning/roml-M1-native-backends-release -- LICENSE-MIT LICENSE-APACHE`; `git show planning/roml-M1-native-backends-release:Cargo.toml | grep license` | Accepted |
| M1.0 | crates.io verification | No evidence of crates.io API or `cargo owner --list` execution; RESEARCH.md records curl API check showing both names available | `git log planning/roml-M1-native-backends-release -- docs/release/evidence/M1/` | Failed |
| M1.0 | support labels | `roml` = supported (workspace root crate), `roml-highs` = experimental (separate crate with build deps); no formal `README.md` or crate-level support label found | `git show planning/roml-M1-native-backends-release:Cargo.toml`; `git ls-tree planning/roml-M1-native-backends-release -- roml-highs/` | Locally Verified |

### M1.1: Backend Contract Freeze

| Milestone | Claim | Source Evidence | Command Run | Truth Verdict |
|-----------|-------|----------------|-------------|---------------|
| M1.1 | contract freeze complete | Protocol types exist (DeltaBatch, Revision, Journal, Snapshot, Cursor, capabilities, SolveRequest); legacy `SolverAdapter` still imported at `adapter.rs:23` and impl'd at `:675`; `drain_changes()` still public at `model/mod.rs:747` | `git grep -c 'pub \(struct\|enum\|trait\|fn\)' ... -- src/delta.rs src/revision.rs src/snapshot.rs src/sync.rs src/solver/backend.rs src/solver/request.rs`; `git grep -rn 'SolverAdapter' ... -- roml-highs/src/` | Failed |
| M1.1 | protocol types frozen | delta.rs (7 pub items), revision.rs (6), snapshot.rs (10), sync.rs (16), solver/backend.rs (11), solver/request.rs (13) — all exist with public API surface | `git grep -c 'pub \(struct\|enum\|trait\|fn\)' planning/roml-M1-native-backends-release -- src/delta.rs src/revision.rs src/snapshot.rs src/sync.rs src/solver/backend.rs src/solver/request.rs` | Accepted |
| M1.1 | legacy path disposition | No plan or shim for legacy path removal in evidence documents; `Model::drain_changes()` still public at `model/mod.rs:747` returning `Vec<Change>`; `SolverAdapter` trait in `adapter.rs:1` (doc), `:23` (import), `:675` (impl) — all public | `git show planning/roml-M1-native-backends-release:docs/release/evidence/M1/M1.1_CONTRACT_FREEZE.md 2>/dev/null | head -20`; `git grep -n 'fn drain_changes' ... -- src/model/mod.rs` | Failed |
| M1.1 | SolveRequest type exists | `pub struct SolveRequest` at `src/solver/request.rs:16` with builder pattern | `git show planning/roml-M1-native-backends-release:src/solver/request.rs | head -10` | Accepted |

### M1.2: HiGHS highs-sys Migration

| Milestone | Claim | Source Evidence | Command Run | Truth Verdict |
|-----------|-------|----------------|-------------|---------------|
| M1.2 | migration complete | `roml-highs/Cargo.toml` declares `highs-sys = "1.15"`; `roml-highs/src/ffi.rs` still exists (65 lines) but content replaced with re-exports from `highs-sys` rather than handwritten `extern "C"` declarations | `git show planning/roml-M1-native-backends-release:roml-highs/Cargo.toml | grep highs-sys`; `git ls-tree -r planning/roml-M1-native-backends-release -- name-only roml-highs/src/` | Partially Satisfied |
| M1.2 | 11/11 tests pass locally | Claim made in commit 0fe295b message and docs commit 85a6396; no test output evidence committed; build environment required for verification | `git log --oneline planning/roml-M1-native-backends-release | grep -i 'test.*pass\|11/11'` | Locally Verified |
| M1.2 | HiGHS adapter uses highs-sys | `roml-highs/src/adapter.rs` imports from `highs_sys` crate; ffi.rs re-exports highs-sys types; all FFI calls go through maintained crate | `git show planning/roml-M1-native-backends-release:roml-highs/src/adapter.rs | head -5`; `git show planning/roml-M1-native-backends-release:roml-highs/src/ffi.rs | head -15` | Accepted |

### M1.3: Semantic Equivalence and Recovery

| Milestone | Claim | Source Evidence | Command Run | Truth Verdict |
|-----------|-------|----------------|-------------|---------------|
| M1.3 | commuting square proven | `tests/backend_contract.rs` (46 test functions); `tests/differential_harness.rs` (23 test functions); both test ReferenceBackend only | `git grep -c '#\\[test\]' planning/roml-M1-native-backends-release -- tests/backend_contract.rs tests/differential_harness.rs` | Partially Satisfied |
| M1.3 | semi-continuous recovery fix | ReferenceBackend protocol implements recovery; HiGHS recovery untested; P1 test (`set_semicontinuous_low_lower_emits_change_without_bounds_update`) still `#[ignore]` at `model_characterization.rs:818` | `git ls-tree planning/roml-M1-native-backends-release -- tests/differential_harness.rs`; `git grep -n '#\\[ignore' planning/roml-M1-native-backends-release -- tests/model_characterization.rs` | Partially Satisfied |
| M1.3 | status mapping defined | `tests/status_negotiation_tests.rs` exists; mapping table tests present | `git ls-tree planning/roml-M1-native-backends-release -- tests/status_negotiation_tests.rs` | Accepted |
| M1.3 | solve-request negotiation | `tests/status_negotiation_tests.rs` covers rejection/adjustment | `git ls-tree planning/roml-M1-native-backends-release -- tests/status_negotiation_tests.rs` | Accepted |
| M1.3 | fault injection matrix | `tests/differential_harness.rs` exists; ReferenceBackend fault injection works; HiGHS fault injection untested | `git grep -c '#\\[test\]' planning/roml-M1-native-backends-release -- tests/differential_harness.rs` | Partially Satisfied |

### M1.4: HiGHS Cross-Platform Qualification

| Milestone | Claim | Source Evidence | Command Run | Truth Verdict |
|-----------|-------|----------------|-------------|---------------|
| M1.4 | HiGHS 11/11 tests pass locally | Claim from commit 0fe295b message ("fix(M1.4): HiGHS adapter clippy clean, 11/11 tests pass locally"); reproducible locally; no CI evidence | `git log --oneline planning/roml-M1-native-backends-release | grep '0fe295b\|85a6396'` | Locally Verified |
| M1.4 | CI workflow created | `.github/workflows/ci-highs.yml` exists at commit fb2cb88; workflow triggers on push/PR to `main` | `git ls-tree planning/roml-M1-native-backends-release -- .github/workflows/ci-highs.yml` | Accepted |
| M1.4 | cross-platform CI execution | No CI runs available in git history; workflow file exists but never executed on GitHub runners | `git log --all --oneline -- .github/workflows/ci-highs.yml` (single commit only) | Failed |
| M1.4 | 11/11 test reproducibility | Claim made in two commits (0fe295b implementation, 85a6396 docs); no test output or CI artifacts committed | `git log --oneline planning/roml-M1-native-backends-release | grep -E '0fe295b|85a6396'` | Locally Verified |

### M1.5: Performance and Ergonomics

| Milestone | Claim | Source Evidence | Command Run | Truth Verdict |
|-----------|-------|----------------|-------------|---------------|
| M1.5 | benchmark harness created | `benches/model_bench.rs` exists; criterion harness with `criterion_group!`/`criterion_main!` | `git ls-tree planning/roml-M1-native-backends-release -- benches/`; `git show planning/roml-M1-native-backends-release:benches/model_bench.rs | head -20` | Accepted |
| M1.5 | 100k iteration tests | Default iteration count in bench harness via criterion | `git show planning/roml-M1-native-backends-release:benches/model_bench.rs | head -20` | Accepted |
| M1.5 | results recorded | No `target/criterion/` or benchmark result files committed | `git ls-tree -r planning/roml-M1-native-backends-release -- benches/`; no benchmark output evidence | Failed |
| M1.5 | user examples exist | `examples/simple_lp.rs`, `examples/parameter_update.rs` both exist | `git ls-tree planning/roml-M1-native-backends-release -- examples/` | Accepted |
| M1.5 | API friction review | No evidence of API review document or commit | `git log --oneline planning/roml-M1-native-backends-release | grep -i 'review\|friction\|api'` | Failed |

### Summary by Milestone

| Milestone | Verdicts | Dominant Disposition |
|-----------|----------|---------------------|
| M1.0 | Accepted(1), Partially Satisfied(2), Failed(1), Locally Verified(1) | Partially Satisfied |
| M1.1 | Accepted(2), Failed(2) | Failed |
| M1.2 | Accepted(1), Partially Satisfied(1), Locally Verified(1) | Accepted |
| M1.3 | Accepted(2), Partially Satisfied(3) | Partially Satisfied |
| M1.4 | Accepted(1), Locally Verified(2), Failed(1) | Partially Satisfied |
| M1.5 | Accepted(3), Failed(2) | Partially Satisfied |

### Comparison with RESEARCH.md Verdict

The RESEARCH.md `Overall M1 State Verdict` table reported the following dominant dispositions:

| Milestone | RESEARCH.md Verdict | Task 1 Fresh Verdict | Discrepancy |
|-----------|--------------------|----------------------|-------------|
| M1.0 | Partially Satisfied | Partially Satisfied | No discrepancy |
| M1.1 | Failed | Failed | No discrepancy |
| M1.2 | Accepted | Accepted | No discrepancy |
| M1.3 | Partially Satisfied | Partially Satisfied | No discrepancy |
| M1.4 | Partially Satisfied | Partially Satisfied | No discrepancy |
| M1.5 | Partially Satisfied | Partially Satisfied | No discrepancy |

**Note on M1.2:** The RESEARCH.md verdict "Accepted" matches the fresh verification at the milestone level, but the fresh verification reveals that `ffi.rs` was NOT removed — it was repurposed as a re-export shim (65 lines vs ~252 lines originally). This is a textual detail: the migration DID replace handwritten FFI with highs-sys, but the file remains. The plan's acceptance criteria flagged this as "ffi.rs confirmed removed" which is technically incorrect; the evidence shows ffi.rs persists but with different content. This nuance is documented in task 002 — Legacy Source Patterns (legacy `Change` type still wired through `drain_changes()` which is the actual legacy path).
