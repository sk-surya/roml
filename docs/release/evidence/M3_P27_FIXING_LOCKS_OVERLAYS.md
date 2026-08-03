# P27 Evidence — Persistent Fixing, Assignments, Locks, and Reversible Overlays

**Phase:** 27-fixing-locks-overlays
**Plan:** `27-PLAN.md` — Task 8 (unify declared domains and add persistent fixing)
**Requirements (Task 8):** SM-05 (all clauses)
**Branch:** `phase-roml-P27-fixing-locks-overlays`
**Base:** `main@192cd00` (P26 merged, PR #28)
**Status:** Task 8 complete — declared domains and first-class persistent fixing implemented and verified.

This document records the P27 Task 8 deliverables per `EXECUTION.md` § "Evidence file structure": the untouched baseline matrix, the RED failures recorded before implementation, the focused and full verification matrices, and the acceptance criteria. Later tasks (9, 10) append their per-task evidence.

## Scope and requirements

Task 8 unifies variable state into one declared-domain record (`VariableDomain` + optional `VariableFixing`) and adds first-class persistent fixing (`Model::fix`/`Model::unfix`, `declared_bounds`/`effective_bounds`, named integrality tolerance) with fixing represented as bound tightening (D6: `fix x = v => lower(x) = upper(x) = v`). It closes SM-05.1–SM-05.7: declared/effective separation, typed atomic fix/unfix, equal-bound compiled representation, unfix-restores-current-declared-bounds, tolerance-aware integrality validation, atomic declared-bound exclusion of an active fixing, and incremental `SetVariableBounds` compilation with rebuild-on-uncertainty.

## Baseline and environment

| Item | Value |
|---|---|
| Base commit (`main`) | `192cd00` (P26 merged, PR #28) |
| HEAD at baseline capture | `192cd00` (worktree `agent-a7d9549eb22ccd7fb`) |
| Branch | `phase-roml-P27-fixing-locks-overlays` (worktree branch `worktree-agent-a7d9549eb22ccd7fb`) |
| Toolchain | rustc/cargo 1.97.x, `aarch64-apple-darwin`, Darwin 25.4.0 |
| HiGHS build | bundled via `highs-sys 1.15.0` (cmake) |
| `roml` baseline | 675 passed; 0 failed; 2 ignored (P26 fifth-review head) |
| `roml-highs` baseline | 114 passed; 0 failed |

All commands ran on the untouched `roml` tree (P26 state) with no P27 source modification.

### Untouched baseline matrix — `roml`

| Command | Exit | Result |
|---|---|---|
| `cargo test -p roml --all-targets` | 0 | 675 passed; 0 failed; 2 ignored |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 | clean |
| `cargo fmt --all -- --check` | 0 | clean |

### Untouched baseline matrix — `roml-highs`

| Command | Exit | Result |
|---|---|---|
| `cargo test -p roml-highs --all-targets` | 0 | 114 passed; 0 failed |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 | clean |

## TDD — RED failures (recorded before implementation)

`cargo test -p roml --test fixing_assignment` failed to compile against the untouched tree — the new test referenced the intended public surface (`fix`/`unfix`/`declared_bounds`/`effective_bounds`, `VariableDomain`/`SemiDomain`/`VariableFixing`/`FixingProvenance`, `VariableEntry.fixing`, and the typed `ModelError` variants). Expected failures, recorded verbatim (excerpt):

```text
error[E0432]: unresolved imports `roml::FixingProvenance`, `roml::SemiDomain`,
              `roml::VariableDomain`, `roml::VariableFixing`
error[E0599]: no method named `fix` found for struct `Model` in the current scope
error[E0599]: no method named `declared_bounds` found for struct `Model` in the current scope
error[E0599]: no method named `effective_bounds` found for struct `Model` in the current scope
error[E0599]: no method named `unfix` found for struct `Model` in the current scope
error[E0609]: no field `fixing` on type `&VariableEntry`
error[E0599]: no variant named `ValueOutOfBounds` found for enum `ModelError`
error[E0599]: no variant named `NonIntegralValue` found for enum `ModelError`
error[E0599]: no method named `integrality_tolerance` found for struct `Model`
```

No production source existed for the fixing surface; every new test failed at compile time.

## Implementation

### `src/model/variable.rs`
- New public packet-shaped types (interface contract): `VariableDomain { bounds, var_type, semi }` (`Clone, Copy, Debug, PartialEq`), `SemiDomain { Continuous { nonzero_lower }, Integer { nonzero_lower } }` (`Clone, Copy, Debug, PartialEq`), `VariableFixing { value, provenance }` (`Clone, Debug, PartialEq`), `FixingProvenance { User, Imported { source } }`.
- `VariableData` rebuilt from `{ bounds, var_type, active, name }` into `{ domain: VariableDomain, fixing: Option<VariableFixing>, active, name }` — one record for declared domain, fixing, activity, and name (SM-05.1).

### `src/model/mod.rs`
- `Model::fix(&mut self, Variable, f64) -> Result<(), ModelError>` and `Model::unfix(&mut self, Variable) -> Result<(), ModelError>` — typed atomic canonical mutations (SM-05.2): validate finiteness, in-domain (`ValueOutOfBounds`), integrality within the named tolerance (`NonIntegralValue`); record `VariableFixing { value, provenance: User }`; emit `Change::VariableFixingChanged` and (via `compile_change`) `ModelOp::SetVariableFixing`; `commit` advances the revision exactly once.
- `Model::declared_bounds` (SM-05.1) and `Model::effective_bounds` (declared ∩ fixing; a fixed variable returns `[value, value]`, SM-05.3). `variable_bounds` remains the declared view (backward compatible). `Model::variable_domain` exposes the full declared domain.
- Named integrality tolerance: `Model::integrality_tolerance()`/`Model::set_integrality_tolerance` (default 1e-9, consistent with the feasibility-tolerance convention), stored on `ModelConstants`.
- Atomicity guard (SM-05.6): `set_variable_bounds` validates the requested bounds against any active fixing before mutation — bounds that exclude the fixing value return `ModelError::BoundsExcludeFixing` with no state change.
- `Model::validate_invariants` gains invariant 10: a live variable with an active fixing satisfies `declared.lower <= fixing.value <= declared.upper` (via the `validation.rs` helper).

### `src/model/changelog.rs`
- `Change::VariableFixingChanged { var, fixing, effective_bounds }` — carries the effective bounds so the compiled delta is self-contained (deviation D8-1 below: an extra field vs the plan's `{ var, fixing }`, required for the self-contained `ModelOp` contract).

### `src/model/validation.rs`
- `fixing_within_declared(fixing, declared) -> bool` invariant helper.

### `src/snapshot.rs`
- `VariableEntry` gains `fixing: Option<VariableFixing>` (SM-05.1); `take_snapshot` signature extended to carry `(bounds, var_type, active, semi, fixing)` per variable (type alias `SnapshotVariableRecord`); `Model::take_snapshot` threads the fixing through (rebuild survival).

### `src/delta.rs`
- `ModelOp::SetVariableFixing { var, fixing: Option<VariableFixing>, effective_bounds: Bounds }` — self-contained (mirrors `SetConstraintBounds`); `None` fixing = unfix restores the current declared bounds (SM-05.4).

### `src/compiler/session.rs`
- `compile_snapshot` folds fixing into the compiled variable bounds (`fix -> Bounds::new(value, value)`, SM-05.3); semi-continuous remains rejected at the compile boundary (P26 behavior unchanged).
- `compile_delta` lowers `SetVariableFixing` to `BackendOp::SetVariableBounds` under `BackendFeature::IncrementalBounds` (SM-05.7); without the feature, a typed capability gate rejects the op (SM-04.4; the façade recovers with a deterministic rebuild — deviation D8-2 below).

### `src/solver/reference.rs`
- `ReferenceBackend::apply_op` handles `ModelOp::SetVariableFixing` as a self-contained bound update.

### `src/lib.rs` / `src/advanced.rs`
- `VariableDomain`, `SemiDomain`, `VariableFixing`, `FixingProvenance` exported through the public surface (`lib.rs` root + `advanced`).

## Public interfaces

New public surface (pre-release additive, per the interface contract and D6):

- `roml::VariableDomain`, `roml::SemiDomain`, `roml::VariableFixing`, `roml::FixingProvenance`.
- `Model::fix`, `Model::unfix`, `Model::declared_bounds`, `Model::effective_bounds`, `Model::variable_domain`, `Model::integrality_tolerance`, `Model::set_integrality_tolerance`.
- `ModelError::{ValueOutOfBounds, NonIntegralValue, BoundsExcludeFixing, InvalidIntegralityTolerance}`.
- `ModelOp::SetVariableFixing`, `Change::VariableFixingChanged`, `VariableEntry::fixing`.

## Focused verification

| Command | Result |
|---|---|
| `cargo test -p roml --test fixing_assignment` | 0 — **20 passed; 0 failed** |

## Full verification

| Command | Result |
|---|---|
| `cargo test -p roml --all-targets` | 0 — **695 passed; 0 failed; 2 ignored** (baseline 675 + 20 new) |
| `cargo test -p roml-highs --all-targets` | 0 — **114 passed; 0 failed** (incremental `SetVariableBounds` path unchanged — SM-05.7 regression) |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 — clean |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 — clean |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 — docs generated, no warnings |
| `cargo test -p roml --doc` | 0 — doctests pass (incl. the prelude negative-inventory compile_fail) |
| `cargo fmt --all -- --check` | 0 — formatting clean |

Baseline comparison: `roml` grew from 675 to 695 passing tests (+20: all in `tests/fixing_assignment.rs`). `roml-highs` is unchanged at 114 (the incremental bound path already exists from P26; no HiGHS test was weakened or deleted).

## Acceptance criteria

- `Model::fix`/`Model::unfix`/`Model::declared_bounds`/`Model::effective_bounds` exist and are typed; fixing advances the canonical revision exactly once (SM-05.2) — **met**.
- A reference state-machine test covers continuous/integer/binary/semi domains, bound changes, fixing, and unfix, and asserts `unfix` restores current (not fix-time) declared bounds (SM-05.4) — **met**.
- Non-finite, out-of-domain, and non-integral (beyond the named tolerance) fix values are typed errors (SM-05.5) — **met**.
- `set_variable_bounds` excluding an active fixing returns a typed error with no state change (SM-05.6) — **met**.
- The compiled representation of a fixing is equal lower/upper bounds (SM-05.3); the fix/unfix delta applies incrementally as `SetVariableBounds` when `IncrementalBounds` is supported, and the `roml-highs` suite stays green (SM-05.7) — **met**.
- Fixing survives `commit` → snapshot → rebuild (phase gate "fix/unfix survives rebuild") — **met** (`fixing_survives_snapshot_rebuild`).
- `remove_variable` clears the fixing; semi-continuous declared domain is carried as `VariableDomain.semi` (canonical state only, still rejected at the compile boundary) — **met**.

## Deviations and decisions

1. **D8-1 — `Change::VariableFixingChanged` carries `effective_bounds`** in addition to `{ var, fixing }`. The plan's `src/model/changelog.rs` bullet names `{ var, fixing }`, but the plan's own `src/delta.rs` contract requires `ModelOp::SetVariableFixing` to be **self-contained** ("carries the bounds to apply; `None` fixing = unfix restores the current declared bounds"). `compile_change` is a pure function with no model access, so the Change must carry the effective bounds for the compile to produce the self-contained op. An unfix's effective bounds (current declared bounds) cannot be derived at compile time.
2. **D8-2 — missing `IncrementalBounds` yields `CompileError::UnsupportedFeature`, not `RebuildRequired`.** The plan's Task 8 prose says "else returns `CompileError::RebuildRequired` (D22)". The P26 capability-gate convention (SM-04.4, WR-3 — asserted by `compiled_delta_gates_bounds_ops_on_incremental_bounds`) returns `UnsupportedFeature` for the identical sibling `SetVariableBounds` op when the backend lacks `IncrementalBounds`. Since `SetVariableFixing` lowers to `SetVariableBounds`, it follows the same gate for consistency; the façade maps any compile-delta error to `Synchronization(RequiresRebuild)`, so the D22 rebuild-on-uncertainty behavior is preserved at the caller.
3. **D8-3 — `take_snapshot` signature change** (the variable tuple gains a 5th `Option<VariableFixing>` element; new `SnapshotVariableRecord` alias). Required for snapshot-carrying-fixing (SM-05.1, rebuild survival). Callers in `src/solver/reference.rs`, `tests/backend_contract.rs`, `tests/differential_harness.rs`, `tests/semicontinuous_recovery.rs` updated mechanically.

## Native/backend evidence

`roml-highs` applies fix/unfix deltas through the P26 incremental `BackendOp::SetVariableBounds` path (`Highs_changeColBounds`) — verified by the full `cargo test -p roml-highs --all-targets` suite (114 passed) staying green with the new canonical fixing state. No native API change was required for Task 8.

## Public API and packaging

The `roml` public surface grows by the types/methods listed under "Public interfaces". `cargo package --list -p roml` and `cargo public-api -p roml` deltas are recorded at the P27 phase end (per the M2/M3 phase-end diff convention). M2 guarded surface (`Model::new`/`Default`/`Clone`, `solve`/`solve_with`, `Highs::solve`) is unchanged (D27).

## Reviewer findings

Pending the P27 two-stage independent review at the phase boundary (plan §"Review gates").

## Residual risks

- The integrality tolerance is a model-level scalar, not per-variable; a per-variable tolerance would be a later amendment.
- `VariableDomain.semi` is declared canonical state only — the compiled IR still has no semi-continuous representation, and a semi-continuous snapshot remains rejected at the compile boundary (P26 behavior preserved).

## Gate result

Task 8 gate: **PASSED** — `cargo test -p roml --all-targets` (695) and `cargo test -p roml-highs --all-targets` (114) green; clippy `-D warnings` clean on both crates; `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` clean; `cargo fmt --all -- --check` clean; no `roml-mosek`/`roml-xpress` command run.

## Commit trail

| # | SHA | Message |
|---|---|---|
| 1 | `d19b54c` | `feat(model): add first-class variable fixing` |
