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

---

# Task 9 — assignments, solution locks, and the SolveOverlay contract

**Requirements:** SM-06 (all clauses), SM-02.2 (secondary), the pinned SolveOverlay contract resolving issue #26 item 1
**Plan:** `27-PLAN.md` Task 9
**Commit:** `29ccf95` `feat(solve): add assignments and solution locks`
**Status:** Task 9 complete — assignments, locks, and the overlay contract (types + compiler) implemented and verified. Execution (apply/rollback/receipts) is Task 10.

## Scope

Task 9 adds the public packet types for hard solution reuse and the pinned SolveOverlay contract:

- `PrimalAssignment { lineage, source_instance, source_revision, values }` — a neutral partial value map; no feasibility/optimality claim (SM-06.1).
- `Solution::primal_assignment` binds the SOLVED model's real lineage/instance/revision and the solution's user-variable values; compiler-only variables are excluded structurally (SM-06.2).
- `SolutionLock` / `LockSelector { AllAssigned, IntegerAssigned, BinaryAssigned, Variables, Except }` / `ContinuousLock { Exact, Within { absolute } }` — distinct packet types (SM-06.3–06.5).
- `SolveOverlay` contract (issue #26 item 1): contents (`temporary_fixings`/`locks`/`objective_locks`/`cutoffs`/`id: OverlayId`), objective override → `SetObjectivePolicy(CompiledObjectivePolicy::Single)`, fresh `CompilationId` `C_overlay` distinct from `C_base`, exact-base staleness rejection, before-mutation assignment/band/value validation (SM-06.6), and a `SolveOverlay` origin on every added temporary row (D5).

## TDD — RED failures (recorded before implementation)

`cargo test -p roml --test solve_overlay --no-run` failed to compile against the Task 8 tree — the new test referenced the intended public surface (`PrimalAssignment`, `SolutionLock`, `LockSelector`, `ContinuousLock`, `SolveOverlay`, `ObjectiveLock`, `ObjectiveCutoff`, `OverlayError`, `CutoffDirection`, `OverlayOp`, `compile_overlay`, `Solution::primal_assignment`), none of which existed. Expected failures, recorded verbatim (excerpt):

```text
error[E0432]: unresolved imports `roml::advanced::CutoffDirection`, `roml::advanced::OverlayOp`
error[E0432]: unresolved imports `roml::AssignmentError`, `roml::ContinuousLock`, `roml::LockSelector`, `roml::ObjectiveCutoff`, `roml::ObjectiveLock`, `roml::OverlayError`, `roml::PrimalAssignment`, `roml::SolutionLock`, `roml::SolveOverlay`
error[E0433]: cannot find `Generation` in `roml`
```

No production source existed for the assignment/lock/overlay surface; every new test failed at compile time.

## Implementation

### `src/assignment.rs` (new)
- `PrimalAssignment { lineage, source_instance, source_revision, values: BTreeMap<Variable, f64> }` — public packet shape, `Clone, Debug, PartialEq`.
- `PrimalAssignment::validate_for(&Model) -> Result<(), AssignmentError>` gates on lineage equality (D4), live generation via `variable_domain` (stale/removed → `StaleVariable`), and value/domain compatibility (`ValueOutOfBounds`, tolerance-aware for integrality on integer/binary variables). Instance/revision are provenance, never authority.
- `PrimalAssignment::subset(&[Variable])` filters the value map preserving lineage/provenance; `PrimalAssignment::value(Variable)`.
- `SolutionLock { assignment, selector, continuous }`, `LockSelector { AllAssigned, IntegerAssigned, BinaryAssigned, Variables(BTreeSet), Except(BTreeSet) }`, `ContinuousLock { Exact, Within { absolute } }` — packet shapes; `SolutionLock::resolve` (crate-private) validates the assignment then resolves the selected `(Variable, f64)` pairs deterministically.
- `AssignmentError { LineageMismatch, StaleVariable, ValueOutOfBounds }`.

### `src/solution/mod.rs`
- `Solution::primal_assignment()` — binds `metadata.model_lineage`/`model_instance`/`model_revision` (CR-02: real solved identity, never `SolveMetadata::default()`) and the user-variable values already stored on the `Solution`.

### `src/solver/overlay.rs` (new — types + compiler; execution is Task 10)
- `SolveOverlay { id: OverlayId, temporary_fixings, locks, objective_locks, cutoffs }` with `SolveOverlay::new` allocating the overlay id through the checked counter.
- `ObjectiveLock { objective, absolute_tolerance, relative_tolerance }`, `ObjectiveCutoff { objective, limit, direction }`, `CutoffDirection { Upper, Lower }`.
- `CompiledOverlay { base_compilation, compilation_id, overlay_id, operations, origin_additions, objective_policy_override }`, `#[non_exhaustive] OverlayOp { SetTemporaryVariableBounds, AddTemporaryRow, RemoveTemporaryRow, SetObjectivePolicy }`.
- `compile_overlay(model, compiler, overlay, objective_override) -> Result<CompiledOverlay, OverlayError>` implements the pinned mapping: temporary fixings → equal bounds; locks → selector-resolved `SetTemporaryVariableBounds` (`Exact` → `[v,v]`, `Within` → band, continuous-only); objective locks/cutoffs → `AddTemporaryRow` over the compiled objective's coefficients with a `SolveOverlay` origin; override → `SetObjectivePolicy(Single)`. Fresh `CompilationId` allocated for `C_overlay` (D28). Stale base (absent or compiler bound to another model instance) and invalid assignments/bands fail before any op (SM-06.6).

### `src/compiler/origin.rs`
- `GeneratedRole` gains `ObjectiveLockRow` and `CutoffRow` (enum stays `#[non_exhaustive]`); `OverlayId::allocate` is now used (the `#[allow(dead_code)]` removed).

### `src/compiler/session.rs`
- Additive `pub(crate)` accessors: `source_instance`, `compiled_variable_id`, `compiled_objective_id`, `next_row_index` (forward id resolution for the overlay compiler; no behavior change).

### `src/model/mod.rs`
- Additive read-only `Model::objective_sense(ObjId) -> Option<Sense>` (objective-lock degradation direction, design §15.2).

### `src/lib.rs` / `src/advanced.rs`
- Public surface: `PrimalAssignment`, `SolutionLock`, `LockSelector`, `ContinuousLock`, `SolveOverlay`, `ObjectiveLock`, `ObjectiveCutoff`, `OverlayError`, `CutoffDirection`, `AssignmentError` through the reviewed public root; `CompiledOverlay`, `OverlayOp`, `compile_overlay` through `advanced` (compiler-facing).

### `tests/solve_overlay.rs` (new — 23 tests)
- Assignment packet/provenance/validate_for (lineage, stale generation, out-of-domain, non-integral, clone reuse per D4); `Solution::primal_assignment` + `subset`; all five selectors and both continuous locks (including the `Within`-on-integer typed error); the pinned overlay contract compile (contents, override, fresh id, origins, stale rejection, before-mutation validation); revision invariance (overlay compile never advances the model).

## Focused verification

| Command | Result |
|---|---|
| `cargo test -p roml --test solve_overlay` | 0 — **23 passed; 0 failed** |

## Full verification

| Command | Result |
|---|---|
| `cargo test -p roml --all-targets` | 0 — **718 passed; 0 failed; 2 ignored** (baseline 695 + 23 new) |
| `cargo test -p roml-highs --all-targets` | 0 — **114 passed; 0 failed** |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 — clean |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 — clean |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 — docs generated, no warnings |
| `cargo test -p roml --doc` | 0 — doctests pass |
| `cargo fmt --all -- --check` | 0 — formatting clean |

No `roml-mosek`/`roml-xpress` command was run (M2 convention, out of scope).

## Acceptance criteria

- `PrimalAssignment` is a partial value map with lineage plus instance/revision provenance and makes no feasibility/optimality claim (SM-06.1) — **met**.
- `Solution::primal_assignment()` returns a lineage-bound assignment whose values are the solution's user-variable values, excluding compiler-only variables; `PrimalAssignment::subset` produces selected subsets (SM-06.2) — **met**.
- `SolutionLock`, `MipStart`, `VariableHints`, and persistent `VariableFixing` are distinct public types; starts/hints land as types in P28 and no conversion exists without explicit policy (SM-06.3, D8) — **met** for locks/fixings; the distinction is documented.
- All five `LockSelector` variants and both `ContinuousLock` variants behave as specified; `Within` on a non-continuous variable is a typed error (SM-06.4, SM-06.5) — **met**.
- Stale or unrelated assignments fail before backend mutation via `validate_for` gating (SM-06.6) — **met** (`compile_overlay_rejects_invalid_assignment_before_any_op`; a failed compile produces no ops).
- `compile_overlay` implements the pinned `SolveOverlay` contract: contents enumerated, objective override maps to `SetObjectivePolicy`, the result tags the override solve with a fresh `CompilationId` distinct from the base, and the lifecycle runs against the exact base `CompilationId` (issue #26 item 1) — **met**.
- SM-02.2 (secondary) closure: reusable assignments validate lineage + entity generations — **met** at the mechanism level (`PrimalAssignment::validate_for`); clause-level closure will be stated in `TRACEABILITY.md`.

## Deviations from the plan

1. **D9-1 — `OverlayError` gains two typed-error variants beyond the plan's enumerated list**: `WithinBandOnNonContinuous { variable }` and `InvalidLockBand { variable, absolute }`. The pinned contract REQUIRES a typed error for a `Within` band on an integer/binary variable ("an integer band cannot round-trip exactly"); the plan's enumerated `OverlayError { StaleCompilation, Assignment, ObjectiveNotFound, IdentityOverflow }` omits it. Correctness also requires rejecting a non-finite/negative `absolute`. Added to complete the pinned contract (Rule 2).
2. **D9-2 — `Model::objective_sense` added** (additive read-only accessor, no behavior change). The objective-lock degradation row direction follows the objective's sense (design §15.2); no sense accessor existed.
3. **D9-3 — objective-lock degradation rows are compiled with a zero reference optimum** in P27: `f(x) <= absolute_tolerance` (min) / `f(x) >= -absolute_tolerance` (max), with the objective's constant folded into the row bound. P31 supplies the real stage optimum `z` (`f(x) <= z + abs + rel*|z|`, design §15.2); the `relative_tolerance` therefore does not affect the P27 placeholder row RHS.
4. **D9-4 — compile-time staleness rejects both an absent compiled base and a compiler bound to a different model instance** via the new `CompilationSession::source_instance` accessor, both as typed `OverlayError::StaleCompilation` before any op.

## Reviewer findings

Pending the P27 two-stage independent review at the phase boundary (plan §"Review gates").

## Gate result

Task 9 gate: **PASSED** — `cargo test -p roml --all-targets` (718) and `cargo test -p roml-highs --all-targets` (114) green; clippy `-D warnings` clean on both crates; `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` clean; `cargo fmt --all -- --check` clean; no `roml-mosek`/`roml-xpress` command run. The phase gates "locks never advance model revision" and "exact compilation mismatches reject before mutation" are structurally satisfied by Task 9's read-only compiler and before-mutation validation; their full end-to-end assertion (revision-invariance after a full overlay solve, apply-time stale rejection) is Task 10.

## Task 9 commit trail

| # | SHA | Message |
|---|---|---|
| 1 | `29ccf95` | `feat(solve): add assignments and solution locks` |

---

# Task 10 — Execute reversible overlays transactionally

**Requirements:** SM-07.3, SM-07.4, SM-07.5, SM-07.6
**Plan:** `27-PLAN.md` Task 10
**Commit:** `9d7a597` `feat(solve): add reversible solve overlays`
**Status:** Task 10 complete — transactional reversible overlay execution implemented and verified on the reference backend and the HiGHS session.

## Scope

Task 10 executes the pinned overlay lifecycle (design §12) transactionally from the caller's perspective:

```text
commit canonical model
-> compile/synchronize canonical state          (C_base established)
-> compile overlay against the exact C_base     (fresh C_overlay, D28)
-> apply overlay                                (C_base -> C_overlay; OverlayApplyReceipt)
-> solve                                        (result tagged C_overlay)
-> validate result.compilation_id == C_overlay  (NOT C_base — the compiler stays at C_base)
-> rollback overlay (always attempted)          (C_overlay -> C_base; explicit receipt)
-> on RequiresRebuild mark the backend RequiresRebuild (D7, D22)
-> verify C_base restored
-> normalize with compilation_id = C_overlay and overlay_id = Some(overlay.id)
```

It closes:
- **SM-07.3** — temporary fixings, solution locks, objective-lock rows, and cutoffs never mutate the canonical model revision (revision-invariance assertion after a full overlay solve);
- **SM-07.4** — explicit `OverlayApplyReceipt`/`OverlayRollbackOutcome`, rollback always attempted on failure;
- **SM-07.5** — rollback uncertainty marks the backend `RequiresRebuild` and the next solve forces a snapshot rebuild;
- **SM-07.6** — the failure-injection matrix (validation/compile/apply/solve/extraction/rollback/post-rollback) proves no overlay leaks into any later solve; every later clean solve equals a fresh rebuild (phase gate 4).

## TDD — RED failures (recorded before implementation)

`cargo test -p roml --test solve_overlay --no-run` failed to compile against the Task 9 tree — the new Task 10 tests referenced the intended new surface, none of which existed. Expected failures, recorded verbatim (excerpt):

```text
error[E0432]: unresolved imports `roml::advanced::OverlayApplyReceipt`,
              `roml::advanced::OverlayRollbackOutcome`, `roml::advanced::OverlaySession`
```

`OverlayApplyReceipt`/`OverlayRollbackOutcome`/`OverlaySession`/`solve_with_overlay`/
`SolveError::Overlay`/`SolveError::Rollback`/`SolveMetadata.overlay_id`/
`SolveResult.overlay_id` were all absent; every new test failed at compile time.

## Implementation

### `src/solver/overlay.rs`
- `OverlayApplyReceipt { overlay_id, base_compilation, applied_compilation }` and `OverlayRollbackOutcome { Clean { restored_compilation }, RequiresRebuild { reason } }` — the explicit transactional receipts (SM-07.4).

### `src/solver/session.rs`
- New bounded optional `pub trait OverlaySession { apply_overlay, rollback_overlay, verify_overlay_clean }` alongside `SessionHealth`/`SolutionView` (design §12: a fallible rollback is explicit, not delegated solely to `Drop`).

### `src/solver/reference.rs`
- `OverlaySession for ReferenceBackend`: apply records prior compiled bounds + added temporary rows + prior policy + a deterministic base view, then applies the ops and transitions `current_compilation` → `C_overlay`; a stale base is rejected before mutation. Rollback restores the prior bounds, removes the temporary rows, restores the policy, and transitions → `C_base`; verify asserts the compiled state equals the base view exactly.

### `src/solver/facade.rs`
- `solve_with` refactored to share a private `synchronize_base` helper (validate → commit → synchronize → bind) with `solve_with_overlay`, so the plain solve path is unchanged (D27).
- New `pub fn solve_with_overlay(&mut self, model, options, overlay, objective_override) -> Result<Solution, SolveError>` (in an `impl` block bounded on `OverlaySession`): compile against `compiler.current_compilation()` (`C_base`), apply, solve, validate `result.compilation_id == compiled_overlay.compilation_id` (NOT `compiler.current_compilation()`, which stays `C_base` — the false-fail trap the plan flags), rollback always attempted, mark `RequiresRebuild` on uncertainty, verify `C_base` restored, normalize with `compilation_id = C_overlay` and `overlay_id = Some(overlay.id)`.
- `normalize_result` gains an `overlay_id: Option<OverlayId>` parameter (pre-release additive) so the overlay solve's metadata records the exact `OverlayId` (D28 overlay artifacts).

### `src/solver/error.rs`
- `SolveError::Overlay(OverlayError)` — the overlay failed to compile before any backend mutation (SM-06.6); `SolveError::Rollback(BackendError)` — a backend error during the apply/rollback/verify lifecycle (backend marked `RequiresRebuild`). The extraction-mismatch path reuses `SolveError::CompilationMismatch`.

### `src/solver/request.rs` / `src/solution/metadata.rs`
- `SolveResult.overlay_id: Option<OverlayId>` and `SolveMetadata.overlay_id: Option<OverlayId>` (default `None`); the façade sets `Some(overlay.id)` on overlay solves (D28/SM-03.9 overlay artifacts).

### `roml-highs/src/compiler.rs` / `roml-highs/src/session.rs` / `roml-highs/src/lifecycle.rs`
- New `project_objective_policy` helper (extracted from the `BackendOp::SetObjectivePolicy` arm) shared by the delta path and the overlay apply/rollback.
- `OverlaySession for HighsSession`: apply translates `SetTemporaryVariableBounds` → `Highs_changeColBounds` (recording prior bounds from `self.var_bounds`), `AddTemporaryRow` → `Highs_addRows` (recording the native row index), `SetObjectivePolicy` → the compiled-policy projection; `current_compilation` → `C_overlay`. Rollback restores each prior bound, deletes each added row via `Highs_deleteRowsBySet` (the temp rows live at the END of the native model, so base row indices are unchanged), restores the base policy, → `C_base`; any failing native call returns `RequiresRebuild` and marks the cursor. Verify checks the native row/column counts and `current_compilation` match `C_base`.
- `HighsSession` gains `compiled_objective_policy` (maintained by `synchronize`) and `overlay_state: Option<HighsOverlayState>`.

### `src/advanced.rs`
- `OverlaySession` exported through the advanced backend-authoring surface; `OverlayApplyReceipt`/`OverlayRollbackOutcome` through the overlay re-export.

### `tests/solve_overlay.rs` (13 new Task 10 tests, 36 total)
- Reference-backend apply → rollback → verify round-trip; stale-apply rejection before mutation.
- Façade-level `solve_with_overlay`: result tagged `C_overlay` + `overlay_id`, revision invariance, C_base restored, clean solve equals fresh rebuild; overlay solve validates against `C_overlay` not `C_base`; invalid overlay rejected before mutation.
- Rollback uncertainty → `RequiresRebuild`; the next solve forces a snapshot rebuild and equals a fresh rebuild.
- Failure-injection matrix at validation/compile/apply/solve/extraction/rollback/post-rollback, each asserting canonical-revision invariance and clean-solve-equals-fresh-rebuild.

### `roml-highs/src/session.rs` (2 new HiGHS overlay tests, 116 total)
- Full overlay apply → solve → rollback → verify round-trip: temp bound + cutoff change the solved objective (max x+y = 10 base; temp fix x=4 + cutoff x+y<=6 → objective 6), rollback restores C_base and the native row count, a subsequent solve equals the base solve (no leak).
- Stale overlay apply rejected before native mutation (column count and `current_compilation` unchanged).

## Focused verification

| Command | Result |
|---|---|
| `cargo test -p roml --test solve_overlay -- --nocapture` | 0 — **36 passed; 0 failed** (23 Task 9 + 13 Task 10) |

## Full verification

| Command | Result |
|---|---|
| `cargo test -p roml --all-targets` | 0 — **731 passed; 0 failed; 2 ignored** (baseline 718 + 13 new) |
| `cargo test -p roml-highs --all-targets` | 0 — **116 passed; 0 failed** (baseline 114 + 2 new) |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 — clean |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 — clean |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 — docs generated, no warnings |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` | 0 — docs generated, no warnings |
| `cargo test -p roml --doc` | 0 — doctests pass |
| `cargo fmt --all -- --check` | 0 — formatting clean |

No `roml-mosek`/`roml-xpress` command was run (M2 convention, out of scope).

## Acceptance criteria

- Temporary fixings, solution locks, objective-lock rows, and cutoffs never mutate the canonical model revision (SM-07.3) — **met** (`solve_with_overlay_tags_result_with_overlay_compilation_and_restores_base` asserts `model.current_revision()` unchanged and no pending changes after a full overlay solve).
- Overlay application and rollback are transactional from the caller's perspective, with explicit `OverlayApplyReceipt`/`OverlayRollbackOutcome` and rollback always attempted on failure (SM-07.4) — **met** (facade `rollback_and_verify` runs on solve/extraction failure; `OverlayRollbackOutcome::Clean`/`RequiresRebuild`).
- Rollback uncertainty marks the backend `RequiresRebuild` and the next solve rebuilds (SM-07.5) — **met** (`rollback_uncertainty_marks_requires_rebuild_and_forces_rebuild` observes the rebuild counter increase and clean-solve-equals-fresh).
- The failure-injection matrix (validation/compile/apply/solve/extraction/rollback/post-rollback) proves no overlay leaks into a later solve; each later clean solve equals a fresh rebuild (SM-07.6) — **met** (the phase gate "no overlay leaks after any injected failure").
- Exact `CompilationId` mismatches reject before mutation; the override solve's result records the fresh overlay `CompilationId` and `overlay_id` (phase gate "exact compilation mismatches reject before mutation"; SM-03.9 overlay artifacts) — **met** (stale-apply rejection tests on both backends; `SolveError::CompilationMismatch` on extraction; metadata records `C_overlay` + `overlay_id`).
- Locks never advance the model revision (phase gate "locks never advance model revision") — **met** (revision-invariance assertion).
- HiGHS temporary bounds/rows qualified through pinned `highs-sys` (`Highs_changeColBounds`/`Highs_addRows`/`Highs_deleteRowsBySet`); unqualified behavior is a typed error — **met** (unknown `OverlayOp` variants and missing compiled variables are typed errors; `Highs_addRows` path asserted end-to-end).

## Deviations from the plan

1. **D10-1 — `Highs_deletionRow` does not exist in the pinned `highs-sys` 1.15.0 bindings.** The plan's rollback bullet names `Highs_deletionRow`; the pinned bindings expose `Highs_deleteRowsBySet` (already used by `RemoveLinearRow`). The HiGHS overlay rollback deletes the added temporary rows via `Highs_deleteRowsBySet`. Functional intent unchanged (remove the added rows).
2. **D10-2 — apply-time stale rejection is a typed `BackendError`, not an `OverlayError::StaleCompilation`.** `OverlaySession::apply_overlay` returns `Result<_, BackendError>` (the plan's pinned trait signature), so a backend cannot return `OverlayError::StaleCompilation`. The backend returns a `BackendError` with `ErrorCategory::InvalidInput` whose message names the expected/actual compilations, BEFORE any mutation. The phase gate "exact compilation mismatches reject before mutation" is preserved.
3. **D10-3 — apply-stage failures map to `SolveError::Rollback`.** The plan names exactly two new `SolveError` variants (`Overlay(OverlayError)` and `Rollback(BackendError)`) and lists apply among the backend errors to map. A failed `apply_overlay` is a `BackendError`; it maps to `SolveError::Rollback` (the only BackendError-carrying overlay variant) and the backend marks itself `RequiresRebuild`.
4. **D10-4 — `normalize_result` gains an `overlay_id: Option<OverlayId>` parameter** (pre-release additive). The plan requires the overlay solve's metadata to record `overlay_id = Some(overlay.id)`; the shared normalization helper is the single place that builds `SolveMetadata`. Plain-solve call sites pass `None`.

## Native/backend evidence

`roml-highs` applies temporary bounds via `Highs_changeColBounds`, temporary rows via `Highs_addRows`, and removes them via `Highs_deleteRowsBySet` — all through the pinned `highs-sys` 1.15.0 bindings (`roml-highs/src/bindings.rs`). The overlay round-trip test proves the native model returns to `C_base` exactly (row/column counts, objective value, `current_compilation`), and the stale-apply test proves no native mutation on rejection.

## Phase-gate status for Task 10

All four ROADMAP gate clauses are now evidenced end-to-end:
1. **fix/unfix survives rebuild** — closed by Task 8 (`fixing_survives_snapshot_rebuild`).
2. **locks never advance model revision** — closed by Task 10 (`solve_with_overlay_tags_result_with_overlay_compilation_and_restores_base`).
3. **exact compilation mismatches reject before mutation** — closed by Tasks 9-10 (compile-time stale rejection + apply-time stale rejection on both backends).
4. **no overlay leaks after any injected failure** — closed by Task 10 (failure-injection matrix + subsequent-solve leak checks on both the façade test backend and HiGHS).

## Task 10 commit trail

| # | SHA | Message |
|---|---|---|
| 1 | `9d7a597` | `feat(solve): add reversible solve overlays` |
