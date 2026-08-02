---
status: testing
phase: 20-public-api-contract
source: [20-VERIFICATION.md]
started: 2026-08-02
updated: 2026-08-02
---

## Current Test

number: 1
name: Review the baseline evidence in docs/release/evidence/M2_P20_BASELINE.md against EXECUTION.md evidence standards
expected: |
  Baseline commands, item counts, and skipped-check reasons are acceptable and
  the recorded counts match what a clean re-run at the base commit produces.
awaiting: user response

## Tests

### 1. Review baseline evidence

Review `docs/release/evidence/M2_P20_BASELINE.md` against `EXECUTION.md` evidence standards: base SHA `d1391fb`, toolchain (rustc 1.97.1, aarch64-apple-darwin), core matrix (399 passed) and HiGHS matrix (73 passed) at base, and the clean-worktree `cargo package --list -p roml` capture (exit 0, 99 files — previously exit 101 from a dirty primary tree, now resolved).

expected: Baseline commands, item counts, and the clean-worktree package list are acceptable and the recorded counts match what a clean re-run at the base commit produces.
result: [pending]

### 2. Review drift characterization

Review `tests/ui/current_readme_drift.rs` and the captured E0432/E0599 compile failures recorded in `M2_P20_BASELINE.md` against the actual README.md (lines 60, 78-79) and MODELING_API.md (lines 26, 55, 146, 160-161, 179, 188) usage.

expected: The captured failures match the documented `HighsAdapter`/`solve_model` API and freezing the fixture (kept out of the default build) is the correct characterization approach.
result: [pending]

### 3. Review frozen target signatures

Review `tests/ui/target_quickstart.rs` and `tests/ui/target_incremental.rs` against DECISIONS.md D1/D3/D4/D7 and the M2 packet (`continuous`/`integer`/`binary`/`parameter` builders, `Model::named`, `add_constraint(spec)`, fallible `set_parameter`, `Highs::new`/`solve`/`solve_with`, `Solution::status().is_optimal()`).

expected: The target signatures are exactly as specified and have NOT been weakened; they remain non-compiling by design until P21/P22.
result: [pending]

### 4. Review disposition table

Review the per-item disposition table in `docs/release/PUBLIC_API_M2_DISPOSITION.md` (91 items across the 8 required categories: root/prelude, model constructors/mutators, expression/operator traits, all four macros, IDs and coefficients, solution/status/result, backend session/sync, callback/capability).

expected: Each item's disposition among exactly one of the five (golden path / optional syntax sugar / advanced backend extension / compatibility-deprecated / internal exposure to remove) is coherent, the replacement signatures match D7/D3, and the deprecation order honors D12 (replacement before deprecation).
result: [pending]

## Summary

total: 4
passed: 0
issues: 0
pending: 4
skipped: 0
blocked: 0

## Gaps
