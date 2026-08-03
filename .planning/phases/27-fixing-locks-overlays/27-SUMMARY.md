---
phase: 27-fixing-locks-overlays
plan: 01
subsystem: model
tags: [variable-domain, variable-fixing, bounds, snapshot, delta, compiler]
dependency_graph:
  requires: [P26 Task 7 identity compiler / backend IR (SetVariableBounds, IncrementalBounds, RebuildRequired), P25 snapshot/delta canonical state]
  provides: [VariableDomain/SemiDomain/VariableFixing/FixingProvenance, Model::{fix,unfix,declared_bounds,effective_bounds}, ModelOp::SetVariableFixing, Change::VariableFixingChanged, VariableEntry::fixing]
  affects: [P27 Task 9 (assignments/locks use declared_bounds/effective_bounds), P27 Task 10 (overlay fixings), P28 (MipStart→temporary fixing), P29 (bound diagnostics)]
tech-stack:
  added: []
  patterns:
    - fixing represented as bound tightening (D6): effective bounds = [value, value], no separate fixing authority
    - self-contained ModelOp carries the effective bounds to apply (mirrors SetConstraintBounds)
    - atomic validation-before-mutation for fixing/bounds (SM-05.5, SM-05.6)
    - snapshot carries declared bounds + fixing so rebuild reconstructs both (SM-05.1)
key-files:
  created:
    - tests/fixing_assignment.rs
    - docs/release/evidence/M3_P27_FIXING_LOCKS_OVERLAYS.md
  modified:
    - src/model/variable.rs
    - src/model/mod.rs
    - src/model/changelog.rs
    - src/model/validation.rs
    - src/snapshot.rs
    - src/delta.rs
    - src/compiler/session.rs
    - src/solver/reference.rs
    - src/solver/conformance.rs
    - src/lib.rs
    - src/advanced.rs
    - tests/backend_contract.rs
    - tests/differential_harness.rs
    - tests/semicontinuous_recovery.rs
    - roml-highs/tests/behavior_tests.rs
    - roml-highs/tests/contract_tests.rs
    - roml-highs/tests/solve_observables_tests.rs
decisions:
  - Fixing is first-class and compiles as equal lower/upper bounds (D6): no separate fixing authority; effective = declared ∩ fixing.
  - ModelOp::SetVariableFixing is self-contained and carries effective_bounds; Change::VariableFixingChanged carries the same so the pure compile_change can produce the self-contained op.
  - Missing IncrementalBounds yields a typed UnsupportedFeature capability gate (consistent with the sibling SetVariableBounds op, SM-04.4), not RebuildRequired; the façade recovers with a rebuild (D22 behavior preserved).
  - integrality_tolerance is a named model-level scalar defaulting to 1e-9 (consistent with feasibility tolerance).
metrics:
  duration: ~90 min
  completed: 2026-08-03
  tasks: 1 (Task 8 of a 3-task phase)
  commits: 1 (task commit) + 1 (summary commit)
status: complete
actuals:
  tokens: 22923   # chars/4 over the realized Task 8 diff (91,693 bytes)
  tasks: 1
  commits: 2
---

# Phase [27] Task [8]: Unify declared domains and add persistent fixing

Declared/effective domain separation and first-class persistent fixing: `VariableDomain { bounds, var_type, semi }` + `Option<VariableFixing>` in one variable record (SM-05.1); typed atomic `Model::fix`/`Model::unfix` (SM-05.2); fixing compiled as equal lower/upper bounds (SM-05.3, D6); `unfix` restores the current declared bounds, not fix-time bounds (SM-05.4); tolerance-aware integrality validation with a named integrality tolerance (SM-05.5); `set_variable_bounds` excluding an active fixing fails atomically (SM-05.6); the fix/unfix delta compiles to `BackendOp::SetVariableBounds` under `IncrementalBounds` (SM-05.7) with a typed capability gate otherwise.

## What was built

- **`src/model/variable.rs`** — packet-shaped public types `VariableDomain`, `SemiDomain`, `VariableFixing`, `FixingProvenance`; `VariableData` rebuilt from `{ bounds, var_type, active, name }` into `{ domain, fixing, active, name }` — one declared-domain + fixing record (SM-05.1).
- **`src/model/mod.rs`** — `Model::fix`/`Model::unfix` (typed atomic canonical mutations), `declared_bounds`/`effective_bounds`/`variable_domain`, named `integrality_tolerance` (+ setter), the SM-05.6 atomicity guard in `set_variable_bounds`, the SM-05.5 invariant in `validate_invariants`, `compile_change` mapping `VariableFixingChanged` → `SetVariableFixing`, and `take_snapshot` threading the fixing.
- **`src/model/changelog.rs`** — `Change::VariableFixingChanged { var, fixing, effective_bounds }`.
- **`src/model/validation.rs`** — `fixing_within_declared` invariant helper.
- **`src/snapshot.rs`** — `VariableEntry::fixing`; `take_snapshot` carries `(bounds, type, active, semi, fixing)` via the new `SnapshotVariableRecord` alias (rebuild survival).
- **`src/delta.rs`** — self-contained `ModelOp::SetVariableFixing { var, fixing, effective_bounds }`.
- **`src/compiler/session.rs`** — `compile_snapshot` folds fixing into effective compiled bounds; `compile_delta` lowers `SetVariableFixing` → `BackendOp::SetVariableBounds` under `IncrementalBounds`.
- **`src/solver/reference.rs`** — `apply_op` handles `SetVariableFixing` as a self-contained bound update.
- **`src/lib.rs` / `src/advanced.rs`** — public-surface wiring of the new domain/fixing types.
- **`tests/fixing_assignment.rs`** — 20-test reference state machine (declared/effective distinction, fix/unfix, tolerance, atomicity, semi domain, snapshot-rebuild survival, compiler lowering + capability gate).

## Verification

| Command | Result |
|---|---|
| `cargo test -p roml --test fixing_assignment` | 0 — 20 passed |
| `cargo test -p roml --all-targets` | 0 — 695 passed; 0 failed; 2 ignored (baseline 675 + 20 new) |
| `cargo test -p roml-highs --all-targets` | 0 — 114 passed (SM-05.7 incremental-bounds regression green) |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 — clean |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 — clean |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 — clean |
| `cargo test -p roml --doc` | 0 — doctests pass (incl. prelude negative-inventory compile_fail) |
| `cargo fmt --all -- --check` | 0 — clean |

## Deviations from plan

1. **`Change::VariableFixingChanged` carries `effective_bounds`** (in addition to `{ var, fixing }`). Required so the pure `compile_change` can produce the plan's self-contained `ModelOp::SetVariableFixing` contract ("carries the bounds to apply; `None` fixing = unfix restores the current declared bounds"). Without the field, an unfix's effective bounds (current declared bounds) are unknowable at compile time.
2. **Missing `IncrementalBounds` yields `CompileError::UnsupportedFeature`, not `RebuildRequired`.** The plan's Task 8 prose says "else returns `CompileError::RebuildRequired` (D22)". The P26 capability-gate convention (SM-04.4/WR-3, asserted for the identical sibling `SetVariableBounds` op) returns `UnsupportedFeature`; `SetVariableFixing` follows the same gate for consistency. The façade maps any compile-delta error to `Synchronization(RequiresRebuild)`, so the D22 rebuild-on-uncertainty behavior is preserved at the caller.
3. **`take_snapshot` signature change** — the per-variable tuple gains a 5th `Option<VariableFixing>` element (new `SnapshotVariableRecord` alias); mechanical updates to in-crate/tests callers.
4. **`Model::variable_domain` accessor added** (not in the plan's method list) so the public `VariableDomain` (including `semi`) is reachable.

## Known Stubs

- `VariableDomain.semi` is declared canonical state only — the compiled IR still has no semi-continuous representation and a semi-continuous snapshot remains rejected at the compile boundary (P26 behavior preserved; semi-native support is out of P27 scope).
- `FixingProvenance::Imported` is declared but only `User` is produced by `Model::fix`; the `Imported` variant is the forward contract for P28+ import paths.

## Self-Check: PASSED

Created files exist (`tests/fixing_assignment.rs`, `docs/release/evidence/M3_P27_FIXING_LOCKS_OVERLAYS.md`); commit `d19b54c` exists in `git log`; all verification commands exit 0; no deletions or untracked files left behind.

---

# Task 9 — assignments, solution locks, and the SolveOverlay contract

**Phase:** 27-fixing-locks-overlays  **Plan:** 01 (Task 9 of 3)
**Requirements:** SM-06 (all clauses), SM-02.2 (secondary), the pinned SolveOverlay contract (issue #26 item 1)
**Status:** complete
**Commits:** `29ccf95` `feat(solve): add assignments and solution locks`
**actuals:**
```yaml
tokens: 32000   # chars/4 over the realized Task 9 diff (≈128 KB)
tasks: 1
commits: 1
```

## One-liner

Assignments, solution locks, and the pinned `SolveOverlay` contract (issue #26 item 1): `PrimalAssignment` (lineage + provenance, no feasibility/optimality claim, `validate_for` gating on lineage + generation + value/domain per SM-02.2/SM-06.6), `Solution::primal_assignment`, `SolutionLock`/`LockSelector`/`ContinuousLock`, and the overlay types + `compile_overlay` compiler (fresh `CompilationId`, `SetObjectivePolicy(Single)` override mapping, `SolveOverlay` origins on every added row, stale-base rejection before any op).

## What was built

- **`src/assignment.rs`** (new) — `PrimalAssignment { lineage, source_instance, source_revision, values: BTreeMap<Variable, f64> }` (SM-06.1), `validate_for` (lineage equality D4 + live generation + value/domain, tolerance-aware for integrality), `subset`, `value`, `AssignmentError { LineageMismatch, StaleVariable, ValueOutOfBounds }`. `SolutionLock`/`LockSelector` (AllAssigned/IntegerAssigned/BinaryAssigned/Variables/Except)/`ContinuousLock` (Exact/Within{absolute}) packet shapes; crate-private `resolve` selects the (variable, value) pairs deterministically.
- **`src/solution/mod.rs`** — `Solution::primal_assignment` binds the SOLVED model's real lineage/instance/revision (CR-02) and the solution's user-variable values (SM-06.2).
- **`src/solver/overlay.rs`** (new, types + compiler; execution is Task 10) — `SolveOverlay` (contents + `id: OverlayId` allocated at construction), `ObjectiveLock`, `ObjectiveCutoff`, `CutoffDirection`, `CompiledOverlay`, `#[non_exhaustive] OverlayOp`, `OverlayError`, and `compile_overlay` implementing the pinned overlay-to-compiled-IR mapping.
- **`src/compiler/origin.rs`** — `GeneratedRole` gains `ObjectiveLockRow`/`CutoffRow`; `OverlayId::allocate` now used.
- **`src/compiler/session.rs`** — additive `pub(crate)` forward-id accessors: `source_instance`, `compiled_variable_id`, `compiled_objective_id`, `next_row_index`.
- **`src/model/mod.rs`** — additive read-only `Model::objective_sense` (objective-lock degradation direction, design §15.2).
- **`src/lib.rs` / `src/advanced.rs`** — public-surface wiring: root exports the assignment/lock/overlay packet types; `advanced` exports `compile_overlay`/`CompiledOverlay`/`OverlayOp`.
- **`tests/solve_overlay.rs`** (new) — 23 tests covering the four Task 9 test groups.

## Verification

| Command | Result |
|---|---|
| `cargo test -p roml --test solve_overlay` | 0 — 23 passed |
| `cargo test -p roml --all-targets` | 0 — 718 passed; 0 failed; 2 ignored (baseline 695 + 23 new) |
| `cargo test -p roml-highs --all-targets` | 0 — 114 passed (no HiGHS surface touched by Task 9) |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 — clean |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 — clean |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 — clean |
| `cargo test -p roml --doc` | 0 — doctests pass |
| `cargo fmt --all -- --check` | 0 — clean |

## Deviations from plan

1. **`OverlayError` gains `WithinBandOnNonContinuous { variable }` and `InvalidLockBand { variable, absolute }`** beyond the plan's enumerated list. The pinned contract requires a typed error for a `Within` band on an integer/binary variable; correctness requires rejecting a non-finite/negative half-width (Rule 2).
2. **`Model::objective_sense` added** (additive read-only accessor) — required for the objective-lock degradation row direction (design §15.2).
3. **Objective-lock degradation rows compile with a zero reference optimum** in P27 (`f(x) <= abs_tol` min / `f(x) >= -abs_tol` max, constant folded into the bound). P31 supplies the real stage optimum `z` (design §15.2).
4. **Compile-time staleness rejects an absent base AND a cross-model base** via the new `source_instance` accessor, both as typed `OverlayError::StaleCompilation` before any op.

## Known Stubs

- **Objective-lock row RHS** is a zero-reference placeholder in P27 (`relative_tolerance` unused at compile time); P31 materializes the stage-optimum RHS. Documented in `docs/release/evidence/M3_P27_FIXING_LOCKS_OVERLAYS.md` (deviation D9-3).
- **`OverlayApplyReceipt` / `OverlayRollbackOutcome`** and the `OverlaySession` trait are intentionally NOT part of Task 9 — the plan places them in Task 10 (execution).

## Phase-gate status for Task 9

- "locks never advance model revision" — structurally satisfied: `validate_for`/`compile_overlay` are read-only on the model (no `Change`/`ModelOp`/revision advance); asserted by `temporary_fixings_and_locks_never_advance_the_model_revision`. The end-to-end overlay-solve revision-invariance assertion is Task 10.
- "exact compilation mismatches reject before mutation" — structurally satisfied at compile time (stale-base rejection before any op); the apply-time stale rejection is Task 10.

## Self-Check: PASSED

Created files exist (`src/assignment.rs`, `src/solver/overlay.rs`, `tests/solve_overlay.rs`); commit `29ccf95` exists in `git log`; all verification commands exit 0; no deletions or untracked files left behind.

---

# Task 10 — Execute reversible overlays transactionally

**Phase:** 27-fixing-locks-overlays  **Plan:** 01 (Task 10 of 3)
**Requirements:** SM-07.3, SM-07.4, SM-07.5, SM-07.6
**Status:** complete
**Commits:** `9d7a597` `feat(solve): add reversible solve overlays`
**actuals:**
```yaml
tokens: 30710   # chars/4 over the realized Task 10 implementation diff (122,842 bytes)
tasks: 1
commits: 1
```

## One-liner

Transactional reversible overlay execution: explicit `OverlayApplyReceipt`/`OverlayRollbackOutcome` + the `OverlaySession` trait (apply/rollback/verify), `SolverSession::solve_with_overlay` running the pinned lifecycle (synchronize to `C_base` → compile overlay → apply → solve → validate against `C_overlay` → always-attempted rollback → `RequiresRebuild` on uncertainty → verify `C_base` → normalize with `C_overlay` + `overlay_id`), reference-backend and HiGHS implementations (temp bounds/rows via `Highs_changeColBounds`/`Highs_addRows`/`Highs_deleteRowsBySet`), and a failure-injection matrix proving no overlay leaks into any later solve.

## What was built

- **`src/solver/overlay.rs`** — `OverlayApplyReceipt` / `OverlayRollbackOutcome` (explicit transactional receipts, SM-07.4).
- **`src/solver/session.rs`** — new bounded `OverlaySession` trait (`apply_overlay`/`rollback_overlay`/`verify_overlay_clean`); fallible rollback is explicit, never Drop-only (design §12).
- **`src/solver/reference.rs`** — `OverlaySession for ReferenceBackend`: prior-bounds/row/policy capture, apply → `C_overlay`, rollback → exact `C_base`, verify equals the base view; stale apply rejected before mutation.
- **`src/solver/facade.rs`** — `solve_with` refactored onto a shared `synchronize_base` helper (plain path unchanged, D27); `solve_with_overlay` implements the pinned lifecycle; `normalize_result` gains `overlay_id`.
- **`src/solver/error.rs`** — `SolveError::Overlay(OverlayError)` and `SolveError::Rollback(BackendError)`; extraction reuses `CompilationMismatch`.
- **`src/solver/request.rs` / `src/solution/metadata.rs`** — `overlay_id: Option<OverlayId>` on `SolveResult`/`SolveMetadata`.
- **`roml-highs`** — `OverlaySession for HighsSession` (temp bounds/rows/policy via pinned `highs-sys`), `project_objective_policy` helper shared with the delta path, `compiled_objective_policy` + `overlay_state` tracking, `Highs_deleteRowsBySet` for row removal.
- **`tests/solve_overlay.rs`** — 13 new tests (36 total); **`roml-highs/src/session.rs`** — 2 HiGHS overlay tests (116 total).

## Verification

| Command | Result |
|---|---|
| `cargo test -p roml --test solve_overlay` | 0 — 36 passed |
| `cargo test -p roml --all-targets` | 0 — 731 passed; 0 failed; 2 ignored (baseline 718 + 13 new) |
| `cargo test -p roml-highs --all-targets` | 0 — 116 passed (baseline 114 + 2 new) |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 — clean |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 — clean |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 — clean |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` | 0 — clean |
| `cargo test -p roml --doc` | 0 — doctests pass |
| `cargo fmt --all -- --check` | 0 — clean |

## Deviations from plan

1. **`Highs_deletionRow` does not exist in pinned `highs-sys` 1.15.0** — the HiGHS overlay rollback deletes added rows via `Highs_deleteRowsBySet` (the existing `RemoveLinearRow` path).
2. **Apply-time stale rejection is a typed `BackendError`, not `OverlayError::StaleCompilation`** — the pinned `OverlaySession::apply_overlay` returns `BackendError`; the backend returns a `BackendError` (InvalidInput) naming the expected/actual compilations before any mutation.
3. **Apply-stage failures map to `SolveError::Rollback`** — the plan names exactly two new `SolveError` variants; a failed apply (a `BackendError`) maps to the `Rollback` variant and the backend marks itself `RequiresRebuild`.
4. **`normalize_result` gains an `overlay_id` parameter** (pre-release additive) so the overlay solve's metadata records `Some(overlay.id)`.

## Phase-gate status for Task 10

- "locks never advance model revision" — **closed end-to-end** (revision-invariance after a full overlay solve).
- "exact compilation mismatches reject before mutation" — **closed end-to-end** (compile-time + apply-time stale rejection on both backends).
- "no overlay leaks after any injected failure" — **closed** (failure-injection matrix at validation/compile/apply/solve/extraction/rollback/post-rollback; every later clean solve equals a fresh rebuild).

## Self-Check: PASSED

All verification commands exit 0; Task 10 tests (13) and HiGHS overlay tests (2) pass; commit recorded below.
