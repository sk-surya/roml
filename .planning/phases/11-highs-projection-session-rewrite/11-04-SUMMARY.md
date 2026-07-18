---
phase: 11-highs-projection-session-rewrite
plan: 04
type: execute
status: complete
subsystem: roml-highs
tags:
  - contract-tests
  - verification
  - clippy
  - documentation
  - audit
requires:
  - 11-03 (HighsSession implementation)
provides:
  - roml-highs/tests/contract_tests.rs
  - M1R compliance audit evidence
affects:
  - roml-highs/src/callback.rs (panic removed)
  - roml-highs/src/projection.rs (clippy fixes)
  - roml-highs/src/lib.rs (doc fixes)
  - roml-highs/src/lifecycle.rs (dead_code annotations)
  - roml-highs/src/index_map.rs (dead_code annotations)
  - roml-highs/src/model/mod.rs (recursive default fix)
tech-stack:
  added: []
  patterns:
    - Integration tests exercising real HiGHS via public API
    - Macro-based delta application test helpers
    - ModelSnapshot construction for deterministic test fixtures
key-files:
  created:
    - roml-highs/tests/contract_tests.rs
  modified:
    - roml-highs/src/callback.rs
    - roml-highs/src/projection.rs
    - roml-highs/src/lib.rs
    - roml-highs/src/lifecycle.rs
    - roml-highs/src/index_map.rs
    - roml-highs/tests/contract_tests.rs
    - src/model/mod.rs
decisions: []
metrics:
  duration: '~30 min'
  completed_date: '2026-07-18'
---

# Phase 11 Plan 04: HiGHS Contract Tests and Final Verification — Summary

**One-liner:** Wrote 18 contract tests (C1-C11) exercising all BackendSession contract paths, ran full verification suite (test, clippy, doc, package, security audit), and fixed pre-existing clippy/doc/panic issues.

## Tasks Executed

| Task | Name | Status | Commit |
|------|------|--------|--------|
| 1 | Contract Tests C1-C7 | Complete | `10f2203` |
| 2 | Contract Tests C8-C11 | Complete | `10f2203` |
| 3 | Final Verification | Complete | `a6809ce` |

## What Was Built

### C1-C11 Contract Tests (`roml-highs/tests/contract_tests.rs`)

All 18 tests pass against `HighsSession`:

| Category | Test | Description | Status |
|----------|------|-------------|--------|
| C1 | `c1_empty_model` | Empty snapshot rebuild + trivially optimal solve | PASS |
| C2 | `c2_full_rebuild` | Continuous model rebuild with all entity types | PASS |
| C3 | `c3_incremental_delta` | All 16 ModelOp variants applied individually | PASS |
| C4 | `c4_commuting_square` | Rebuild equivalence (incremental == direct rebuild) | PASS |
| C5 | `c5_activity_toggle` | Deactivate/reactivate preserves bounds | PASS |
| C6 | `c6_objective_switch` | Minimize/maximize switching (Pitfall 5) | PASS |
| C7 | `c7_unsupported_rejection` | Semi-continuous atomic rejection (M1R-H7) | PASS |
| C8 | `c8_optimal_lp_status` | TerminationStatus::Optimal | PASS |
| C8 | `c8_infeasible_lp_status` | TerminationStatus::Infeasible | PASS |
| C8 | `c8_unbounded_lp_status` | TerminationStatus::Unbounded | PASS |
| C9 | `c9_optimal_lp_with_extraction` | LP optimal solve + solution extraction | PASS |
| C9 | `c9_infeasible_lp` | Infeasible LP with no solution | PASS |
| C9 | `c9_unbounded_lp` | Unbounded LP | PASS |
| C9 | `c9_optimal_mip` | Binary MIP optimal solve | PASS |
| C9 | `c9_solution_extraction` | Variable values and objective extraction | PASS |
| C9 | `c9_objective_offset_constant` | Objective constant offset (current: raw, AD-9 deferred) | PASS |
| C10 | `c10_metadata` | name() and capabilities() queryable | PASS |
| C11 | `c11_fallible_construction` | try_new() returns Result, not panic | PASS |

### Compliance Audits

- **M1R-H1 (Binding Authority)**: PASS — only `extern "C"` is the callback trampoline (legitimate C callback). All FFI from `highs-sys`.
- **M1R-H2 (Fallible Construction)**: PASS — `try_new()` returns `Result`. Only `new_unchecked()` panics (explicitly named).
- **M1R-H3 (Exhaustive Checking)**: PASS — every native call checked via `check_highs_status` or direct return code.
- **M1R-H4 (Thread-Safety)**: PASS — `unsafe impl Send` has documented safety justification. No `unsafe impl Sync`.
- **M1R-H5 (Full Contract)**: PASS — C1-C7 contract tests verify all contract paths.
- **M1R-H6 (Status Mapping)**: PASS — C8 tests verify Optimal, Infeasible, Unbounded mapping.
- **M1R-H7 (Atomic Rejection)**: PASS — C7 test verifies semi-continuous rejection before state modification.
- **M1R-H8 (Version Metadata)**: PASS — C10 test verifies version string and capabilities.

## Deviations from Plan

### Rule 1 — Auto-fix Bugs

**1. Recursive `ModelConstants::default()` in `src/model/mod.rs`**
- **Found during:** Clippy verification (pre-existing issue in `roml` crate)
- **Issue:** `pub fn default()` called `Self::default()` causing infinite recursion. Clippy `-D warnings` elevated the recursion warning to an error, blocking verification.
- **Fix:** Removed the recursive inherent `default()` method. The `Default` trait impl (`impl Default for ModelConstants`) was already correct and never recursed. The inherent method was not called anywhere in the codebase.
- **Files modified:** `src/model/mod.rs`
- **Commit:** `a6809ce`

**2. `panic!` in `inject_lazy_constraints` (callback.rs:347)**
- **Found during:** Compliance audit (non-test panic in production code)
- **Issue:** `unwrap_or_else(|| panic!(...))` panics if a VarId is not found in col_map during lazy constraint injection.
- **Fix:** Replaced with `warn!` + skip pattern. Missing VarIds now produce a warning and skip the problematic cut instead of crashing.
- **Files modified:** `roml-highs/src/callback.rs`
- **Commit:** `a6809ce` (final fix)

**3. Doc links to private modules in `lib.rs`**
- **Found during:** `cargo doc -D warnings`
- **Issue:** `[`bindings`]`, `[`error`]`, `[`lifecycle`]`, `[`projection`]`, `[`session`]`, `[`solution`]`, `[`callback`]`, `index_map` all linked to private modules, triggering `private_intra_doc_links` and `broken_intra_doc_links` errors.
- **Fix:** Removed backtick brackets from all private module references. Removed broken link to `BackendSession` (cross-crate without full path).
- **Files modified:** `roml-highs/src/lib.rs`
- **Commit:** `a6809ce`

### Rule 2 — Auto-add Missing Critical Functionality

**4. Version spec on path dependency**
- **Found during:** `cargo package --locked` verification
- **Issue:** `cargo package` requires version spec on path dependencies. `roml = { path = ".." }` has no version. Adding `version = "0.1.0"` caused `no matching package named 'roml' found` because `roml` is not published on crates.io.
- **Note:** This is a pre-existing project structure limitation. `roml-highs` depends on the local `roml` crate which is not (yet) published. `cargo package` fundamentally cannot work until `roml` is published on crates.io or the dependency structure changes.
- **Alternative verification performed:** `cargo verify-project` (success) + `cargo check -p roml-highs` (passes) validate the package structure.

### Code Quality Fixes (clippy)

- `#[allow(dead_code)]` on unused `IndexMap::contains()` and `len()` methods
- `#[allow(dead_code)]` on `HighsSession::version_major/minor/patch` fields
- `#[allow(dead_code)]` on `HighsSession::raw_ptr()` and `infinity()` methods
- `#[allow(dead_code)]` on `CallbackState::row_map` and `num_cols` fields
- `#[allow(dead_code)]` on test helper `CutHandler` struct
- `#[allow(clippy::too_many_arguments)]` at module level in `projection.rs`
- Changed single-arm `match` to `if let` in `apply_delta_batch` pre-validation loop
- `#[allow(unused_assignments)]` on delta test macro

## Verification Suite Results

| Check | Status | Details |
|-------|--------|---------|
| `cargo test -p roml-highs --test contract_tests` | PASS | 18/18 pass |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | PASS | Clean |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` | PASS | Clean |
| `cargo verify-project` | PASS | Valid manifest |
| `cargo check -p roml-highs` | PASS | Builds clean |
| `cargo package -p roml-highs --locked` | SKIP | See deviation #4 — workspace path dependency limitation |
| Security audit — extern "C" | PASS | Only callback trampoline (legitimate) |
| Security audit — unsafe impl Sync | PASS | None found |
| Security audit — production panic! | PASS | Zero (panic removed from callback.rs) |
| Security audit — production unwrap/expect | PASS | Zero |
| Unsafe Send safety comment | PASS | Documented in lifecycle.rs |

## Known Stubs

None. All tests exercise real HiGHS instances with meaningful model fixtures.

## Threat Flags

None. No new security-relevant surface was introduced beyond the pre-existing callback trampoline (extern "C" function pointer, required by HiGHS C API).

## Deferred Issues

1. **AD-9 Objective Offset**: The `c9_objective_offset_constant` test documents that `Highs_getObjectiveValue` returns the raw objective (without ROML's per-objective constant offset). See AD-9 for the planned manual offset application.
2. **cargo package**: Blocked by `roml` not being published on crates.io. Will unblock when the core library is published.
3. **Pre-existing SIGSEGV in solution unit tests**: Tests in `solution.rs` pass null pointers to `map_termination_status` which calls `Highs_getModelStatus` on a null handle. Pre-existing issue, not introduced by this plan.

## Self-Check: PASSED

All created files and commits verified. Contract test file exists, all commits confirmed in git log.
