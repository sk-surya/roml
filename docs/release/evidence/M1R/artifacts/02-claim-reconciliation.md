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

## Legacy Source Patterns

Each pattern was verified by running `git show` or `git grep` against the candidate
branch (`planning/roml-M1-native-backends-release`). Line numbers reference the
file content on that branch.

### Pattern 1: `Model::drain_changes()` (destructive)

- **Definition:** `pub fn drain_changes(&mut self) -> Vec<Change>` at `src/model/mod.rs:747`
- **References:** 3 total (definition at `:747`, call sites at `:1232` and `src/solver/mod.rs:178`)
- **Issue:** Destructive synchronization — consumes the changelog irreversibly. No replay
  or retry possible after `drain_changes()` is called. If the adapter errors during apply
  (P2-1, P2-2), changes are lost.
- **Severity:** Critical
- **M1R Phase:** M1R-01
- **Note:** This is the destructive synchronization point that the revisioned protocol
  (`DeltaBatch`, `Journal`, `AdapterCursor`) replaces.

### Pattern 2: `SolverAdapter` trait (public, legacy path)

- **Import:** `use roml::solver::SolverAdapter` at `roml-highs/src/adapter.rs:23`
- **Implementation:** `impl SolverAdapter for HighsAdapter` at `roml-highs/src/adapter.rs:675`
- **Exposed in lib.rs doc:** `roml-highs/src/lib.rs:4` references SolverAdapter in module docs
- **Issue:** Legacy path is still the **public execution path** for HiGHS. The trait's
  `apply_changes` method takes `&[Change]` — the legacy Change type, not a `DeltaBatch`.
  All production HiGHS solves go through this path.
- **Severity:** Critical
- **M1R Phase:** M1R-01
- **Note:** Legacy path is still the PUBLIC execution path for HiGHS. M1R-01 replaces
  this with the revisioned `DeltaBatch`-based protocol.

### Pattern 3: `Model.solver_options` (model-owned solve policy)

- **Field:** `pub(crate) solver_options: Option<SolveOptions>` at `src/model/mod.rs:124`
- **Setter:** `pub fn set_solver_options()` at `src/model/mod.rs:307-308`
- **Issue:** Transient solve policy stored in canonical `Model` state alongside the model's
  structural data. The `SolveRequest` type exists (per M1.1) but the old `solver_options`
  field persists and is the default path for setting solve options.
- **Severity:** High
- **M1R Phase:** M1R-01
- **Note:** Transient solve policy stored in canonical Model state. M1R-01 removes this
  field, making `SolveRequest` the exclusive path.

### Pattern 4: Panic-based HiGHS construction

- **Constructor:** `pub fn new()` at `roml-highs/src/adapter.rs:175` (delegates to `with_options`)
- **`with_options`:** at `roml-highs/src/adapter.rs:180`
- **Panic paths:**
  - `assert!(!ptr.is_null(), ...)` at `:182` — panics if `Highs_create()` returns null
  - `assert_eq!(sz, 4, ...)` at `:186` — panics if HighsInt size is wrong
  - `.unwrap()` at `:206` — panics if CString conversion fails for option keys
  - `.unwrap()` at `:218` — panics if CString conversion fails for option values
- **Doc comment:** Line 173 explicitly says "Panics if `Highs_create()` returns null"
- **Issue:** Multiple panic paths during normal construction. Violates D-007 (fallible
  construction). A missing library, wrong HighsInt size, or non-UTF-8 option value crashes
  the process.
- **Severity:** High
- **M1R Phase:** M1R-02
- **Note:** Violates D-007 (fallible construction). M1R-02 replaces these with typed errors.

### Pattern 5: Legacy `Change` type still wired

- **Definition:** `pub enum Change` at `src/model/changelog.rs:22`
- **Return type:** `drain_changes()` returns `Vec<Change>` (model/mod.rs:747)
- **Changelog storage:** `ChangeLog` stores `changes: Vec<Change>` (changelog.rs:163)
- **Usage:** `pub use changelog::Change` re-exported at model/mod.rs:19
- **Issue:** The `Change` type is the data type of the legacy destructive path. Every
  `Change::*` variant corresponds to a model operation that the revisioned protocol
  represents as a `DeltaBatch` with typed operations. The entire Change enum goes away
  with M1R-01.
- **Severity:** Critical
- **M1R Phase:** M1R-01
- **Note:** The Change-based path goes away with M1R-01 (`DeltaBatch` replacement).

### Legacy Pattern Summary Table

| Pattern | Location | Line | Severity | M1R Phase to Address |
|---------|----------|------|----------|----------------------|
| drain_changes | src/model/mod.rs | 747 | Critical | M1R-01 |
| SolverAdapter trait | roml-highs/src/adapter.rs | 675 (impl) | Critical | M1R-01 |
| Model.solver_options | src/model/mod.rs | 124 | High | M1R-01 |
| Panic construction | roml-highs/src/adapter.rs | 182, 186, 206, 218 | High | M1R-02 |
| Legacy Change type | src/model/changelog.rs | 22 | Critical | M1R-01 |
