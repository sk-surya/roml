---
phase: 23-surface-curation
verified: 2026-08-02T21:45:00Z
status: human_needed
score: 11/11 must-haves verified
behavior_unverified: 0
overrides_applied: 0
human_verification:
  - test: "Independent API coherence review (API-10.6): a separate human reviewer assesses protocol preservation, error semantics, and the prelude/advanced split, per docs/release/M2_P23_PUBLIC_API_REVIEW.md lines 91-96."
    expected: "A reviewer signs off on the curated surface; any adjustments are tracked before merge."
    why_human: "Requires a separate human reviewer; not performable by the executing agent. The orchestrator's PR review serves this item (noted, not a failure)."
---

# Phase 23: Surface Curation, Validation, and Migration Verification Report

**Phase Goal:** reduce cognitive load and make misuse explicit without stranding current users.
**Verified:** 2026-08-02
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | The default prelude is small and intentional — only common model/expression/definition/solver/solution/error types (API-07.1) | ✓ VERIFIED | `src/lib.rs:238-245` curated `pub mod prelude` (25 items); `tests/prelude_contract.rs` 3/3 pass importing only `roml::prelude::*` |
| 2 | The 11 API-07.2 protocol types (`Change`, `CoeffId`, `DeltaBatch`, `ModelOp`, `ModelRevision`, `ModelSnapshot`, `AdapterCursor`, `AdapterHealth`, `Synchronization`, `BackendSession`, `SyncReceipt`) are absent from the prelude | ✓ VERIFIED | `src/lib.rs:217-237` `compile_fail` doctest passes; public-api evidence `docs/release/evidence/M2_P23_public_api_roml.txt` shows 0 matches for each type under `roml::prelude::` |
| 3 | `roml::advanced` groups backend contract/revisions/snapshots/deltas/cursors/capabilities/callbacks/raw IDs with stability docs (API-07.3) | ✓ VERIFIED | `src/advanced.rs` (90 lines) grouped re-exports + stability/semver docs (lines 15-27); `tests/advanced_surface.rs` 2/2 pass, including `backend_contract_implementable_from_advanced` proving a backend author can implement `BackendSession` from `roml::advanced` alone |
| 4 | Implementation stores and raw arena internals remain private (API-07.4) | ✓ VERIFIED | `src/id/mod.rs:16` `pub(crate) use arena::IdArena`; `IdArena` has 0 occurrences in `M2_P23_public_api_roml.txt` |
| 5 | Validation is release-safe: `set_variable_bounds`, `set_constraint_bounds`, `set_semicontinuous`, and raw `*_coefficient` reject NaN/inverted/non-finite; NaN term coefficients rejected before row insertion; failed mutations atomic (API-06.1/06.2/06.5) | ✓ VERIFIED | `src/model/mod.rs:337-364,416-433,462-485,911+,993+,1708-1716`; `tests/validation_consistency.rs` 13/13 pass in debug AND release with `assert_unchanged` atomicity snapshot |
| 6 | `VarId - VarId` expression operator (P22 deferred item 3) | ✓ VERIFIED | `src/expr/linear.rs:413-419`; `tests/validation_consistency.rs:269-288` `var_id_sub_var_id_produces_expression` passes |
| 7 | Deprecations carry actionable replacement notes; the deprecated surface still works and remains tested (API-08.3) | ✓ VERIFIED | `#[deprecated]` on `Model::constrain`/`constraint`/`set_objective` (`src/expr/linear.rs:591,608,681`), `add_var`/`add_binary`/`add_integer` (`src/model/mod.rs:247,261,281`), `constrain!`/`set_objective!` (`src/lib.rs:156,192`); `tests/compatibility_api.rs` 5/5 pass with `#![allow(deprecated)]` |
| 8 | `MIGRATION.md` has the 10 plan-listed before/after sections; `CHANGELOG.md` has Unreleased entries (API-08.2) | ✓ VERIFIED | `MIGRATION.md` sections: Variable/parameter creation, Constraints, Objectives, HiGHS solve path, Parameter update, Solve options, Solution/status access, Advanced backend sessions, Coefficient-cell ops, Imports/prelude changes; `CHANGELOG.md:14-25,82-100` Unreleased Deprecated/Changed P23 entries |
| 9 | Every P23 `#[deprecated]` note references its MIGRATION.md section | ✓ VERIFIED | All 8 P23-deprecated entry points name the section (`MIGRATION.md -> Variable and parameter creation` / `Constraints` / `Objectives`). `Model::drain_changes` (pre-P23 deprecation, commit `797591b`) lacks the link but is documented in `MIGRATION.md` deprecation summary table line 26 — pre-existing, not a P23 regression |
| 10 | `cargo public-api` evidence stored and compared to P20 baseline; every removal classified as intentional pre-1.0 break with migration coverage (API-07.5) | ✓ VERIFIED | `docs/release/M2_P23_PUBLIC_API_REVIEW.md` (removals table lines 26-37, API-07.2 negative-inventory grep lines 53-67); evidence files present (`M2_P23_public_api_roml.txt` 10,737 lines, `roml_highs.txt` 106 lines vs P20 7,431/80) |
| 11 | Existing suites stay green (API-10.1) | ✓ VERIFIED | Independent re-run: `cargo test -p roml --all-targets` 551/0; `cargo test -p roml-highs --all-targets` 89/0; fmt clean; clippy `-D warnings` clean; `cargo test --release -p roml --test validation_consistency` 13/0; `--test modeling_ergonomics` 38/0; doctests 1 passed/10 ignored |

**Score:** 11/11 truths verified (0 present, behavior-unverified)

### Required Artifacts

| Artifact | Expected | Status | Details |
| -------- | --------- | ------ | ------- |
| `src/advanced.rs` | `roml::advanced` namespace | ✓ VERIFIED | 90 lines; grouped re-exports + stability/semver docs; wired via `pub mod advanced` (`src/lib.rs:10`) |
| `src/lib.rs` | curated prelude + negative-inventory doctest | ✓ VERIFIED | prelude (lines 238-245) + `compile_fail` doctest (lines 217-237) passing |
| `src/model/mod.rs` | release-safe validation + deprecations | ✓ VERIFIED | validation at lines 337-364, 416-433, 462-485, 573-583, 911+, 993+, 1708-1716; `#[deprecated]` at 247, 261, 281, 1255 |
| `src/expr/linear.rs` | `VarId - VarId` + deprecated aliases + NaN bound rejection | ✓ VERIFIED | Sub impl at 413-419; deprecations at 591, 608, 681; `add_constraint_expr` validation at 631-650 |
| `src/id/mod.rs` / `src/id/arena.rs` | arena internals crate-private | ✓ VERIFIED | `pub(crate) use arena::IdArena` (`src/id/mod.rs:16`) |
| `tests/prelude_contract.rs` | intended prelude compile checks | ✓ VERIFIED | 3/3 pass |
| `tests/advanced_surface.rs` | backend-author compile example | ✓ VERIFIED | 2/2 pass |
| `tests/validation_consistency.rs` | release-safe validation + atomicity | ✓ VERIFIED | 13/13 pass (debug + release) |
| `tests/compatibility_api.rs` | deprecation compatibility coverage | ✓ VERIFIED | 5/5 pass |
| `MIGRATION.md` | 10-section migration guide | ✓ VERIFIED | all 10 plan-listed sections present |
| `docs/release/M2_P23_PUBLIC_API_REVIEW.md` + evidence | public API review + inventories | ✓ VERIFIED | review classifies every removal as intentional pre-1.0 break |
| `CHANGELOG.md` | Unreleased entries | ✓ VERIFIED | Unreleased Deprecated/Changed (lines 14-25, 82-100) |

### Key Link Verification

| From | To | Via | Status | Details |
| ---- | --- | --- | ------ | ------- |
| `roml::prelude` | source re-exports | `pub use crate::{...}` (`src/lib.rs:239-244`) | WIRED | all 25 items resolve; prelude_contract tests compile/run |
| `roml::advanced` | grouped modules | `pub use crate::solver::...` / `delta` / `revision` / `snapshot` / `sync` / `id` / `model` (`src/advanced.rs:33-89`) | WIRED | advanced_surface tests implement contract from this namespace |
| `Model` mutations | validation gates | `validate_constraint_bounds` + `validate_expression_entities` called before insertion (`src/model/mod.rs:462-485, 496-497, 635-636`) | WIRED | atomicity verified by `assert_unchanged` in tests |
| `#[deprecated]` notes | MIGRATION.md sections | note text (`see MIGRATION.md -> <Section>`) | WIRED | all 8 P23-deprecated items name a section |
| prelude negative inventory | API-07.2 | `compile_fail` doctest (`src/lib.rs:217-237`) | WIRED | doctest passes; evidence file confirms 0 matches |
| `BackendSession` contract | `roml::advanced` | re-export unchanged (`src/advanced.rs:34`) | WIRED | contract preserved (API-08.4); no ADR amendment needed |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
| -------- | ------------- | ------ | ------------------ | ------ |
| `prelude_contract.rs` | model state (vars/cons/objs/params) | real `Model::add_variable`/`add_constraint`/`maximize` calls | Yes — assertions on actual counts/names/bounds | ✓ FLOWING |
| `validation_consistency.rs` | `State` snapshot (counts/seq/rev) | real model state before/after failed mutation | Yes — `assert_unchanged` compares actual post-mutation state | ✓ FLOWING |
| `advanced_surface.rs` | `DeltaBatch`/`SyncReceipt` | real `DeltaBatch::new` + `Synchronization::DeltaBatch` | Yes — assert applied revision/health after sync | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| -------- | ------- | ------ | ------ |
| Release-safe validation (atomicity + NaN/non-finite rejection) | `cargo test -p roml --test validation_consistency` | 13 passed | ✓ PASS |
| Release-safe validation in release profile | `cargo test --release -p roml --test validation_consistency` | 13 passed | ✓ PASS |
| Curated prelude supports ordinary workflow | `cargo test -p roml --test prelude_contract` | 3 passed | ✓ PASS |
| Backend contract implementable from `roml::advanced` | `cargo test -p roml --test advanced_surface` | 2 passed | ✓ PASS |
| Deprecated surface still works | `cargo test -p roml --test compatibility_api` | 5 passed | ✓ PASS |
| Prelude negative-inventory guard | `cargo test -p roml --doc` | 1 passed / 10 ignored (compile_fail prelude) | ✓ PASS |
| Release-mode ergonomics (golden path) | `cargo test --release -p roml --test modeling_ergonomics` | 38 passed | ✓ PASS |
| Full core suite green | `cargo test -p roml --all-targets` | 551 passed / 0 failed | ✓ PASS |
| Full HiGHS suite green | `cargo test -p roml-highs --all-targets` | 89 passed / 0 failed | ✓ PASS |
| fmt / clippy `-D warnings` | `cargo fmt --all -- --check`; `cargo clippy -p roml -p roml-highs --all-targets -- -D warnings` | clean | ✓ PASS |

### Probe Execution

No probes declared in the P23 plan; not a migration/tooling phase in the probe-script sense. Step 7c: SKIPPED (no `scripts/*/tests/probe-*.sh` referenced by the plan).

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
| ----------- | ---------- | ----------- | ------ | -------- |
| API-06.1 | P23 Task 3 | Invalid bounds rejected before mutation | ✓ SATISFIED | `set_variable_bounds` (model/mod.rs:342-353), `validate_constraint_bounds` (1708-1716); tests |
| API-06.2 | P23 Task 3 | NaN/non-finite rejected in debug + release | ✓ SATISFIED | validation_consistency tests pass in both profiles; `validate_expression_entities` (462-485) |
| API-06.3 | P23 Task 3 | Parameter mutation fallible, rejects stale IDs | ✓ SATISFIED | `add_parameter` fallible (744-757); `set_variable_bounds_stale_id_is_atomic` test |
| API-06.4 | P23 Task 3 | Builders validate binary/integer bounds | ✓ SATISFIED | `set_variable_bounds_rejects_binary_outside_unit_interval` test |
| API-06.5 | P23 Task 3 | Public mutations atomic on validation failure | ✓ SATISFIED | `assert_unchanged` snapshot across 13 tests incl. NaN term-coefficient rejection |
| API-06.6 | P23 Task 3 | Errors identify entity/invariant | ✓ SATISFIED | `NonFiniteValue("coefficient value")` etc.; typed `ModelError` variants |
| API-07.1 | P23 Task 1 | Prelude limited to common types | ✓ SATISFIED | prelude (lib.rs:238-245); prelude_contract 3/3 |
| API-07.2 | P23 Task 1 | 11 protocol types absent from prelude | ✓ SATISFIED | compile_fail doctest + 0 matches in evidence |
| API-07.3 | P23 Task 2 | Backend extensions under `roml::advanced` with stability docs | ✓ SATISFIED | advanced.rs + advanced_surface 2/2 |
| API-07.4 | P23 Task 2 | Implementation stores/arena private | ✓ SATISFIED | `IdArena` `pub(crate)`; 0 public-api occurrences |
| API-07.5 | P23 Task 6 | `cargo public-api` reviewed and stored | ✓ SATISFIED | review doc + evidence files |
| API-08.1 | P23 Task 4 | Replacements land before deprecations | ✓ SATISFIED | P22 canonical surface (add_constraint/minimize/maximize/definitions) precedes P23 deprecation commits; deprecation commits are later in history |
| API-08.2 | P23 Task 5 | Breaking changes in MIGRATION.md + CHANGELOG.md | ✓ SATISFIED | both files present with all entries |
| API-08.3 | P23 Task 4 | Deprecated APIs have actionable notes + remain tested | ✓ SATISFIED | compatibility_api 5/5 + `#[deprecated]` notes referencing MIGRATION.md |
| API-08.4 | P23 Task 2 | Backend contract unchanged | ✓ SATISFIED | `BackendSession` re-exported unchanged via `roml::advanced`; no ADR amendment |
| API-10.1 | P23 Task 6 | Existing suites remain green | ✓ SATISFIED | 551/0 + 89/0 independently confirmed |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| `src/id/arena.rs` | 39 | `TODO - maybe never` doc-comment design note | ℹ️ Info | Pre-existing (introduced in commit `bede54d`, not P23). Doc note about a potential future compaction optimization, not an unfinished implementation. No action required. |

No TBD/FIXME/XXX, no placeholder/stub, no hardcoded-empty patterns, no console.log-only implementations found in any P23-created or P23-modified file.

### Deviations (evaluated, not failures)

1. **`add_parameter(f64)` documented rather than `#[deprecated]`** — Verified sound. `add_parameter<P: Into<ParameterDef>>` (`src/model/mod.rs:744-747`) is generic; rustc rejects `#[deprecated]` on the `From<f64> for ParameterDef` trait impl, so the attribute cannot be applied to just the f64-form. The `Into` bridge preserves the call shape (`add_parameter(1.0)` still compiles). Documented in MIGRATION.md (lines 57-58) and CHANGELOG.md (line 22). Acceptable.
2. **`deltas_since(rev)` deferred to P24** — Verified recorded. `docs/release/M2_P23_PUBLIC_API_REVIEW.md` lines 34-37 classify it as a deferred removal (P20 disposition "internal exposure to remove") so P21/P22 suites stay green. Remains public at `src/model/mod.rs:1693`. Not a P23 gate item; recorded, suites green (551/0).
3. **`cargo-semver-checks` skipped** — Plan says "if configured"; not installed, no pre-1.0 baseline. Reason recorded in the review doc (lines 69-75) rather than claimed as passing. Consistent with plan.

### Human Verification Required

1. **Independent API coherence review (API-10.6)**
   **Test:** A separate human reviewer assesses protocol preservation, error semantics, and the prelude/advanced split.
   **Expected:** Reviewer signoff; any adjustments tracked before merge.
   **Why human:** Not performable by the executing agent. Recorded in `docs/release/M2_P23_PUBLIC_API_REVIEW.md` lines 91-96 as recommended before merge. Per orchestrator instruction, the PR review serves this item — noted, not a failure.

### Gaps Summary

No gaps found. All 11 must-have truths verified, all artifacts exist and are substantive and wired, all key links confirmed, all requirements (API-06, API-07, API-08) satisfied with evidence, and the full test matrix independently re-confirmed green (roml 551/0, roml-highs 89/0, release validation 13/0, release ergonomics 38/0, doctests, fmt, clippy). The P23 gate is met: default surface small and intentional; advanced authors retain a documented path; validation is release-safe; every breaking/deprecated change has a tested replacement and a migration entry. The single outstanding item is the human independent API coherence review, which the merge/PR review process serves.

---

_Verified: 2026-08-02_
_Verifier: Claude (gsd-verifier)_
