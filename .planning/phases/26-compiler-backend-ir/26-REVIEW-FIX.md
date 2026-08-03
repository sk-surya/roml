---
phase: 26-compiler-backend-ir
fixed_at: 2026-08-03T07:36:30Z
review_path: .planning/phases/26-compiler-backend-ir/26-REVIEW.md
iteration: 1
findings_in_scope: 7
fixed: 7
skipped: 0
status: all_fixed
---

# Phase 26: Code Review Fix Report

**Fixed at:** 2026-08-03T07:36:30Z
**Source review:** `.planning/phases/26-compiler-backend-ir/26-REVIEW.md`
**Iteration:** 1

**Summary:**
- Findings in scope: 7 (2 critical, 5 warning; the 4 info findings are out of the
  critical/warning fix scope)
- Fixed: 7
- Skipped: 0

## Fixed Issues

### CR-01: Compiled `RemoveVariable` leaves stale coefficients in rows/objectives

**Files modified:** `src/solver/reference.rs`, `tests/differential_harness.rs`
**Commit:** `9af0e4c`
**Applied fix:** `ReferenceBackend::apply_compiled_op(BackendOp::RemoveVariable(id))`
now mirrors the canonical `apply_op(ModelOp::RemoveVariable)` cleanup: after
removing the variable from `compiled_variables`, it retain-prunes every
`(CompiledVariableId, f64)` entry referencing `id` from all `compiled_rows` and
`compiled_objectives` coefficient vectors. Added the end-to-end removal test
`dx_compiled_remove_variable_purges_coefficients_and_holds_square` (Section 8 of
`tests/differential_harness.rs`) asserting the compiled rows/objectives no longer
reference the removed variable and that the compiled commuting square
(`compiled_rebuild(rN) == apply(compiled_deltas)`) holds for a removal path.

### CR-02: Removing the active objective leaves the compiled policy dangling

**Files modified:** `src/compiler/session.rs`, `src/solver/reference.rs`,
`tests/differential_harness.rs`
**Commit:** `5675a99`
**Applied fix:** (a) `CompilationSession::compile_delta` now tracks the working
compiled objective policy on `CurrentCompilation` and, when `RemoveObjective`
removes the currently-active objective, ALSO emits
`BackendOp::SetObjectivePolicy(CompiledObjectivePolicy::None)` after
`RemoveObjective` so the batch is self-contained (A31); (b)
`ReferenceBackend::apply_compiled_op(BackendOp::RemoveObjective(id))`
defensively clears `compiled_objective_policy` when it references the removed
id (matching the canonical `active_objective = None`). Added the end-to-end test
`dx_compiled_remove_active_objective_clears_policy_and_holds_square` asserting
`objective_policy == CompiledObjectivePolicy::None` after application and that
the compiled commuting square holds.

### WR-1: HiGHS session never validates `CompilationId` on `CompiledDeltaBatch`

**Files modified:** `roml-highs/src/lifecycle.rs`, `roml-highs/src/session.rs`,
`roml-highs/tests/contract_tests.rs`
**Commit:** `49df760`
**Applied fix:** `HighsSession` now tracks `current_compilation: Option<CompilationId>`,
set from a `CompiledRebuild`'s `snapshot.compilation_id` and each accepted
batch's `to_compilation`. `synchronize(CompiledDeltaBatch)` rejects a batch whose
`from_compilation` does not match the session's current compiled state (a typed
`CompileError::StaleCompilation` surfaced as an `InvalidInput` `BackendError`,
health `Recoverable` + cursor `RequiresRebuild`) before any op is applied —
mirroring the reference backend's D28/SM-03.9 check. The nine `contract_tests`
that previously compiled the base and the delta in separate one-shot sessions
(so `from_compilation` could never match the session base) were updated to chain
the base and its deltas in one `CompilationSession` (new
`compile_base_and_delta` / `rebuild_then_apply_delta` helpers), which is exactly
the D28 discipline. Added
`compiled_delta_rejects_stale_from_compilation` (unit test in
`roml-highs/src/session.rs`).

### WR-2: Façade first-solve delta path never delivers the compiled empty base

**Files modified:** `src/solver/facade.rs`, `tests/solver_facade.rs`
**Commit:** `514f273`
**Applied fix:** `SolverSession::apply_deltas` now SENDs the newly compiled empty
base to the backend as a `CompiledRebuild` before the first `CompiledDeltaBatch`
(after all deltas are compiled, preserving compile-before-mutation). The uniform
explicit contract is documented and enforced: a backend always holds a compiled
base before receiving a `CompiledDeltaBatch`; there is no "un-sent empty base"
special case, and the base-establishment rebuild is NOT a rebuild retry
(API-02.3's one-retry bound counts only the error-recovery rebuild). This makes
the reference backend's actual capability (a base is required before deltas) and
the production path agree. The four `solver_facade.rs` tests whose `rebuilds`
counter is affected by the base-establishment rebuild were updated with
explanatory comments, and the first-solve test now locks the contract
(exactly one base-establishment rebuild + one delta on the first solve).

### WR-3: Incomplete capability gating in `compile_delta` (SM-04.4)

**Files modified:** `src/compiler/session.rs`, `tests/compiler_sync.rs`
**Commit:** `44541d7`
**Applied fix:** `compile_delta` now gates `SetVariableBounds` and
`SetConstraintBounds` (→ `SetLinearRowBounds`) on
`BackendFeature::IncrementalBounds`, and `SetCell`, `RemoveCell`, and
`SetObjectiveCell` on `BackendFeature::IncrementalCoefficients`; an unqualified
feature produces `CompileError::UnsupportedFeature`, never a silently compiled
delta, and the compiler does not advance on a rejected delta. Added the two
gated-path tests `compile_delta_gates_bounds_ops_on_incremental_bounds` and
`compile_delta_gates_cell_ops_on_incremental_coefficients`.

### WR-4: `CompilationSession` does not guard cross-model reuse

**Files modified:** `src/compiler/session.rs`, `src/solver/facade.rs`,
`src/solver/conformance.rs`, `tests/compiler_sync.rs`,
`tests/differential_harness.rs`, `roml-highs/src/session.rs`,
`roml-highs/tests/contract_tests.rs`, `roml-highs/tests/repeated_session_baseline.rs`
**Commit:** `cf40319`
**Applied fix:** `compile_snapshot` validates the incoming `ModelInstanceId`
against the session's recorded `source_instance` and returns a typed
`CompileError::RebuildRequired` on mismatch. `compile_delta` gained a
`source_instance` parameter (all callers updated) and performs the same check
before any compilation. Cross-model `SolverSession` reuse therefore surfaces as
a typed error instead of silently miscompiling the second model's deltas against
the first model's compiled base (D28). Added
`compile_snapshot_rejects_cross_model_source_instance` and
`compile_delta_rejects_cross_model_source_instance`.

### WR-5: HiGHS FFI projection panics via `.expect()` on origin-less entries

**Files modified:** `roml-highs/src/compiler.rs`
**Commit:** `f7e2c32`
**Applied fix:** `rebuild_from_backend_snapshot` replaced the three
`.expect("every compiled variable/row/objective has a recorded origin")` calls
with checked `match` lookups that return a typed
`BackendError::new(..., ErrorCategory::InvalidInput, HealthEffect::RequiresRebuild)`
on a missing origin, consistent with `synchronize`'s error surface. Added
`rebuild_from_snapshot_rejects_origin_less_entry_with_error_not_panic` (a
snapshot built through the builder then stripped of its origin map produces an
error, not a panic).

## Skipped Issues

None — all 7 in-scope findings were fixed.

## Verification

Gates ran in the isolated phase worktree at `.git/p26-impl` (branch
`phase-roml-P26-compiler-backend-ir`), NOT the main checkout — the numbers below
are reproducible from that tree.

- `cargo test -p roml --all-targets` — 647 passed, exit 0
- `cargo test -p roml-highs --all-targets` — 102 passed, exit 0
- `cargo clippy -p roml --all-targets -- -D warnings` — clean
- `cargo clippy -p roml-highs --all-targets -- -D warnings` — clean
- `cargo fmt --all -- --check` — clean

Per-fix commits (atomic):
- `9af0e4c` CR-01, `5675a99` CR-02, `58f5aff` (fixed-seed comment honesty,
  part of the CR-01/CR-02 removal-coverage deliverable), `49df760` WR-1,
  `514f273` WR-2, `44541d7` WR-3, `cf40319` WR-4, `f7e2c32` WR-5.

---

_Fixed: 2026-08-03T07:36:30Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
