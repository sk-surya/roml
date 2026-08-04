---
phase: 27-fixing-locks-overlays
plan: 01
type: execute
wave: 1
depends_on:
  - "26-01"
files_modified:
  - src/model/variable.rs
  - src/model/mod.rs
  - src/model/changelog.rs
  - src/model/validation.rs
  - src/snapshot.rs
  - src/delta.rs
  - src/compiler/session.rs
  - src/compiler/origin.rs
  - src/assignment.rs
  - src/solution/mod.rs
  - src/solution/metadata.rs
  - src/solver/overlay.rs
  - src/solver/session.rs
  - src/solver/facade.rs
  - src/solver/error.rs
  - src/lib.rs
  - src/advanced.rs
  - roml-highs/src/session.rs
  - roml-highs/src/compiler.rs
  - src/solver/request.rs
  - src/solver/reference.rs
  - tests/fixing_assignment.rs
  - tests/solve_overlay.rs
  - docs/release/evidence/M3_P27_FIXING_LOCKS_OVERLAYS.md
autonomous: false
requirements:
  - SM-05
  - SM-06
  - SM-07.3
  - SM-07.4
  - SM-07.5
  - SM-07.6
must_haves:
  truths:
    - fix/unfix survives rebuild
    - locks never advance model revision
    - exact compilation mismatches reject before mutation
    - no overlay leaks after any injected failure
  artifacts:
    - src/model/variable.rs, src/model/mod.rs, src/snapshot.rs, src/delta.rs — declared domain + fixing state machine
    - src/compiler/session.rs — fixing compiles to effective-bound deltas; overlay compile entry
    - src/compiler/origin.rs — GeneratedRole overlay variants; OverlayId in use
    - src/assignment.rs — PrimalAssignment, SolutionLock, LockSelector, ContinuousLock
    - src/solver/overlay.rs — SolveOverlay, CompiledOverlay, OverlayOp, apply/rollback receipts
    - src/solver/facade.rs, src/solver/session.rs — overlay lifecycle orchestration; OverlaySession trait
    - roml-highs/src/session.rs, roml-highs/src/compiler.rs — temporary bounds/rows apply/rollback
    - tests/fixing_assignment.rs, tests/solve_overlay.rs
    - docs/release/evidence/M3_P27_FIXING_LOCKS_OVERLAYS.md
  key_links:
    - Overlay compile ↔ base CompilationId — stale overlay rejected before mutation (D28, SM-03.9)
    - CompiledOverlay fresh CompilationId ↔ result tagging — an override solve is recorded by a distinct id
    - Assignment lineage/generation/domain validation ↔ backend-mutation gate (SM-02.2, SM-06.6)
    - Fixing effective bounds ↔ compiled SetVariableBounds — incremental when IncrementalBounds, rebuild otherwise (SM-05.7, D22)
    - Rollback uncertainty ↔ RequiresRebuild — session forced to rebuild before reuse (SM-07.5, D7)
    - Overlay never advances ModelRevision — SM-07.3 revision-invariance assertion
---

# Phase 27 — Persistent Fixing, Assignments, Locks, and Reversible Overlays

> **For agentic workers:** this phase builds hard solution reuse (persistent fixing, primal assignments, solution locks) and the reversible solve overlay on the P26 compiled-IR foundation. Execute test-first per `EXECUTION.md`: write a focused failing test, record the expected failure, implement the smallest correct behavior, run focused then phase tests, commit one coherent unit, update evidence and traceability. Tasks 8 → 9 → 10 are strictly serial (see "Waves and parallelization" for the shared-file and semantic-dependency rationale). Do NOT run `roml-mosek`/`roml-xpress` — they are known-broken against the current facade and out of scope (M2 convention). Verification is per-crate only; never `--workspace`. Stop after Task 10, append the phase gate evidence, and request the two-stage independent review before marking the phase done.

**Goal:** support hard solution reuse while protecting canonical history and backend state.

**Requirements:** SM-05 (all clauses), SM-06 (all clauses), SM-07.3–SM-07.6. Secondary closure: SM-02.2 (the P25 lineage identity + the P27 assignment-validation mechanism — per `TRACEABILITY.md`, "SM-02.2 foundations only (validation mechanism P27)").

## Requirements

- **SM-05.1** — variable state separates declared domain from optional persistent fixing. Closed by Task 8 (`VariableDomain` + `Option<VariableFixing>` in one variable record).
- **SM-05.2** — `Model::fix` and `Model::unfix` are typed atomic canonical mutations. Closed by Task 8.
- **SM-05.3** — the default compiled representation of fixing is equal lower/upper bounds. Closed by Task 8 (compiler).
- **SM-05.4** — `unfix` restores the current declared bounds, not the bounds at the time of fixing. Closed by Task 8 (state-machine test).
- **SM-05.5** — continuous, integer, and binary fixing validation is explicit and tolerance-aware. Closed by Task 8 (named integrality tolerance).
- **SM-05.6** — declared-bound changes that exclude an active fixing fail atomically. Closed by Task 8 (`set_variable_bounds` guard).
- **SM-05.7** — persistent fixing changes synchronize incrementally when the backend supports bound changes. Closed by Task 8 (compiled `SetVariableBounds`; `roml-highs` regression).
- **SM-06.1** — `PrimalAssignment` stores a partial mapping without claiming feasibility or optimality. Closed by Task 9.
- **SM-06.2** — `Solution` can produce a lineage-bound primal assignment and selected subsets. Closed by Task 9 (`Solution::primal_assignment`, `PrimalAssignment::subset`).
- **SM-06.3** — solution locks, MIP starts, hints, and persistent fixings are distinct public types. Closed by Task 9 for locks/fixings; `MipStart`/`VariableHints` land as distinct types in P28 (Task 10) — P27 documents the distinction (D8) and must not conflate them.
- **SM-06.4** — solution-lock selectors support all assigned, integer assigned, binary assigned, explicit variables, and exclusions. Closed by Task 9.
- **SM-06.5** — continuous locks support exact or absolute-band semantics. Closed by Task 9.
- **SM-06.6** — stale or unrelated assignments fail before backend mutation. Closed by Task 9 (`PrimalAssignment::validate_for` gating overlay compilation).
- **SM-07.3** — temporary fixings, solution locks, objective-lock rows, and cutoffs do not mutate the canonical model revision. Closed by Task 10.
- **SM-07.4** — overlay application and rollback are transactional from the caller's perspective. Closed by Task 10 (explicit apply/rollback receipts; rollback always attempted on failure).
- **SM-07.5** — rollback uncertainty marks the backend `RequiresRebuild`. Closed by Task 10.
- **SM-07.6** — failure injection proves that no overlay leaks into a later solve. Closed by Task 10 (failure matrix + subsequent-solve leak tests).
- **SM-02.2 (secondary)** — reusable assignments validate lineage and entity generations. The P25 lineage identity + the P27 validation mechanism (`PrimalAssignment::validate_for`: lineage match + live generation + value/domain compatibility) closes this clause. Per `TRACEABILITY.md`, the evidence must state this clause-level closure explicitly.

## SolveOverlay contract (issue #26 item 1 — pinned in this phase)

The approved design §12 names `SolveOverlay` in the overlay lifecycle but never enumerates its contents. **This section is the authority**; Task 9 and Task 10 implement exactly these shapes and identifiers. A later phase may change them only through the amendment protocol.

### Contents — what a `SolveOverlay` carries

```rust
pub struct SolveOverlay {
    pub id: OverlayId,
    pub temporary_fixings: std::collections::BTreeMap<Variable, f64>,
    pub locks: Vec<SolutionLock>,
    pub objective_locks: Vec<ObjectiveLock>,
    pub cutoffs: Vec<ObjectiveCutoff>,
}
```

- `id: OverlayId` — opaque overlay identity (design §4.4), allocated at overlay construction through the checked atomic counter in `src/compiler/origin.rs` (zero reserved, typed `IdentityOverflow`). Every overlay-generated entity and every apply/rollback receipt references this id.
- `temporary_fixings` — solve-scoped variable fixings (SM-07.3), distinct from persistent `VariableFixing` (SM-05.1). Apply as equal lower/upper compiled bounds; rollback restores the pre-overlay compiled bounds. P28's `MipStart`→temporary-fixing conversion (SM-08.5) writes here.
- `locks` — `SolutionLock` instances; each applies its selector over its `PrimalAssignment` values (SM-06.4) and restricts the selected variables per `ContinuousLock` (SM-06.5): `Exact` fixes each selected variable to its assigned value; `Within { absolute }` sets a band `[v - absolute, v + absolute]` and is valid for continuous variables only (a `Within` band on an integer/binary variable is a typed error — an integer band cannot round-trip exactly).
- `objective_locks` — temporary degradation rows for lexicographic stages (design §15.2): `f(x) <= z + abs_tol + rel_tol*|z|` for minimization, `>=` for maximization. P31 builds these and executes stages through the P27 overlay mechanism; P27 declares the type and compiles/executes it as a temporary row.
- `cutoffs` — temporary objective cutoffs (`f(x) <= limit` or `>= limit`). Compiled and executed as temporary rows like objective locks.

### How `SolvePlan.objective_override` changes the active `CompiledObjectivePolicy`

- The overlay compiler accepts an optional objective override alongside the overlay. In P27 the override is `Option<ObjId>` (an objective handle — no canonical `ObjectivePolicy` type exists until P31). P28 wraps it as `SolvePlan.objective_override: Option<ObjectivePolicy>` (design §12) and the mapping below is identical for `ObjectivePolicy::Single(Objective)`; P31 extends the same path to `Weighted`/`Lexicographic`.
- Mapping: `override: Some(obj)` → `OverlayOp::SetObjectivePolicy(CompiledObjectivePolicy::Single(compiled_id))`, where `compiled_id` is resolved through the `CompilationSession` user→compiled objective map. A compiled override referencing an objective absent from the base compiled state is a typed `CompileError::InvalidReference`/`InvalidObjectivePolicy`.
- The override is solve-scoped: it does not advance canonical revision and does not mutate the compiled canonical snapshot; it is carried by the overlay state and rolled back with it.

### How the result's `CompilationId` records an override solve

- Base compiled state (after `commit → compile/synchronize`): `C_base = compiler.current_compilation()`.
- Overlay compilation against the **exact `C_base`** produces a distinct compiled overlay state with a **fresh `CompilationId`** `C_overlay` (D28: every distinct compiled backend state receives a distinct opaque `CompilationId`). `CompiledOverlay { base_compilation: C_base, compilation_id: C_overlay, overlay_id, operations, origin_additions, objective_policy_override }`.
- Apply transitions the backend `C_base → C_overlay`; the solve result is tagged `compilation_id = C_overlay`. An override/overlay solve is therefore recorded by a `CompilationId` distinct from the plain model's `C_base`; a result tagged with anything else is a `SolveError::CompilationMismatch`. Plain solves keep recording `C_base`.
- Stale-state safety (the phase gate "exact compilation mismatches reject before mutation"): the overlay is compiled and applied **only** against `C_base`. A backend whose current `CompilationId` is not `C_base` rejects the overlay with typed `CompileError::StaleCompilation` before any native mutation (SM-03.9). After apply, a canonical delta against the overlay state is rejected until rollback (structural interleaving protection).
- Rollback transitions `C_overlay → C_base`; a `Clean` outcome verifies `backend.current_compilation == C_base`. An uncertain rollback marks the session `RequiresRebuild` (SM-07.5, D7 invariant).
- Note: `compiler.current_compilation()` stays `C_base` throughout the overlay solve — the overlay is not compiled *into* the `CompilationSession`. The overlay-aware solve therefore validates `result.compilation_id == compiled_overlay.compilation_id`, not against `compiler.current_compilation()` (which would false-fail). See Task 10.

### Overlay lifecycle against exact `CompilationId` (design §12)

```text
commit canonical model
-> compile/synchronize canonical state          (C_base established)
-> validate plan / overlay                       (typed errors, before any mutation)
-> compile overlay against exact C_base          (produces CompiledOverlay with fresh C_overlay)
-> apply overlay                                 (backend C_base -> C_overlay; OverlayApplyReceipt)
-> apply starts/hints                            (P28; declared here, not in P27)
-> execute objective stages                      (P31 lexicographic; P27 declares the mechanism)
-> extract result tagged with C_overlay
-> rollback overlay                              (backend C_overlay -> C_base; explicit receipt)
-> verify backend canonical state                (C_base restored, else RequiresRebuild)
```

### Overlay-to-compiled-IR mapping (`src/solver/overlay.rs` + `roml-highs/src/compiler.rs`)

```rust
pub struct CompiledOverlay {
    pub base_compilation: CompilationId,   // exact state the overlay applies on top of
    pub compilation_id: CompilationId,     // fresh id for the overlay-compounded state (D28)
    pub overlay_id: OverlayId,
    pub operations: Vec<OverlayOp>,
    pub origin_additions: OriginMap,       // every added temp row has a SolveOverlay origin (D5)
    pub objective_policy_override: Option<CompiledObjectivePolicy>,
}

#[non_exhaustive]
pub enum OverlayOp {
    SetTemporaryVariableBounds { variable: CompiledVariableId, bounds: Bounds },
    AddTemporaryRow { row: CompiledLinearRow },
    RemoveTemporaryRow { row: CompiledConstraintId },
    SetObjectivePolicy(CompiledObjectivePolicy),
}
```

Mapping rules (implemented in Task 9's `compile_overlay`, executed in Task 10):

- `temporary_fixings` → `SetTemporaryVariableBounds { variable: compiled_id(v), bounds: Bounds::fixed(value) }` per entry.
- `locks` → selector resolution over the lock's assignment values → one `SetTemporaryVariableBounds` per selected variable (`Exact` → `Bounds::fixed(value)`; `Within { absolute }` → band `[v - absolute, v + absolute]`). `IntegerAssigned`/`BinaryAssigned` select only the integer/binary assignment entries; `Variables(set)` selects exactly the set; `Except(set)` selects all assigned minus the set. Each selected variable's value is validated against the model's declared domain before any op is produced.
- `objective_locks` → `AddTemporaryRow { row: CompiledLinearRow }` — a row over the compiled objective's coefficients (the compiled row for `f(x)`) with the degradation RHS (design §15.2).
- `cutoffs` → `AddTemporaryRow { row: CompiledLinearRow }` — the cutoff row.
- objective override → `SetObjectivePolicy(CompiledObjectivePolicy::Single(...))`.
- Every `AddTemporaryRow` receives `EntityOrigin::SolveOverlay { overlay, role }` in `origin_additions`, with `GeneratedRole` gaining overlay variants in P27 (D5, SM-02.5: no generated entity without an origin).
- Backends translate `OverlayOp` in `roml-highs/src/compiler.rs`: `SetTemporaryVariableBounds` → `Highs_changeColBounds`; `AddTemporaryRow` → `Highs_addRows`; `RemoveTemporaryRow` → `Highs_deletionRow`; `SetObjectivePolicy` → the same compiled-policy projection `BackendOp::SetObjectivePolicy` already uses.

## Files

Create:

- `src/assignment.rs` — `PrimalAssignment`, `SolutionLock`, `LockSelector`, `ContinuousLock`, `AssignmentError`, `validate_for`/`subset`.
- `src/solver/overlay.rs` — `SolveOverlay`, `ObjectiveLock`, `ObjectiveCutoff`, `CompiledOverlay`, `OverlayOp`, `OverlayError`, `OverlayApplyReceipt`, `OverlayRollbackOutcome`, and the overlay compiler `compile_overlay`.
- `tests/fixing_assignment.rs` — domain/fixing state machine, fix/unfix, atomicity, tolerance, rebuild survival, incremental sync.
- `tests/solve_overlay.rs` — assignment compatibility, lock selectors/bands, overlay compile/apply/rollback, failure injection, leak tests.
- `docs/release/evidence/M3_P27_FIXING_LOCKS_OVERLAYS.md` — created empty (scope + baseline) before implementation per `EXECUTION.md`, appended as work proceeds.

Modify:

- `src/model/variable.rs` — `VariableDomain { bounds, var_type, semi }`, `SemiDomain`, `VariableFixing`, `FixingProvenance`; the internal variable record gains a declared `VariableDomain` and `Option<VariableFixing>`.
- `src/model/mod.rs` — `Model::fix`, `Model::unfix`, `Model::declared_bounds`, `Model::effective_bounds`, named integrality tolerance; `set_variable_bounds` atomicity guard.
- `src/model/changelog.rs` — canonical `Change::VariableFixingChanged`.
- `src/model/validation.rs` — invariant: a live fixing's value lies inside the declared bounds.
- `src/snapshot.rs` — `VariableEntry` carries `Option<VariableFixing>`.
- `src/delta.rs` — `ModelOp::SetVariableFixing { var, fixing: Option<VariableFixing>, effective_bounds: Bounds }` (self-contained: carries the bounds to apply, mirroring `SetConstraintBounds`).
- `src/compiler/session.rs` — `compile_delta` lowers `SetVariableFixing` to `BackendOp::SetVariableBounds` (or `RebuildRequired`); forward user→compiled accessors for overlay id resolution (`compiled_variable_id`, `compiled_objective_id`).
- `src/compiler/origin.rs` — `GeneratedRole` gains overlay variants (`ObjectiveLockRow`, `CutoffRow`); `OverlayId::allocate` exercised.
- `src/solution/mod.rs` — `Solution::primal_assignment`.
- `src/solution/metadata.rs` — `SolveMetadata.overlay_id: Option<OverlayId>` (pre-release additive field; `Default` → `None`).
- `src/solver/session.rs` — optional `OverlaySession` trait (apply/rollback/verify).
- `src/solver/facade.rs` — `SolverSession::solve_with_overlay` orchestration (lifecycle above).
- `src/solver/request.rs` — `SolveResult.overlay_id: Option<OverlayId>` (pre-release additive; `None` for plain solves).
- `src/solver/error.rs` — `SolveError::Overlay(...)`/`SolveError::Rollback(...)` wrappers (pre-release additive).
- `src/lib.rs`, `src/advanced.rs` — export the new public surface (assignment/lock/overlay types through the reviewed public surface; compiler-facing pieces through `advanced`).
- `roml-highs/src/session.rs` — `OverlaySession` impl: temporary bounds via `Highs_changeColBounds`, temporary rows via `Highs_addRows`/`Highs_deletionRow`, base-state capture and restore, `current_compilation` `C_base → C_overlay → C_base` transitions.
- `roml-highs/src/compiler.rs` — `OverlayOp` → HiGHS translation helper.
- `tests/fixing_assignment.rs`, `tests/solve_overlay.rs` — see above.

## Task 8 — Unify declared domains and add persistent fixing

**Phase:** P27  **Requirements:** SM-05 (all clauses)

**Read first:**
- `docs/superpowers/specs/2026-08-02-semantic-modeling-and-solve-workflows-design.md` §10 "Persistent fixing", §18 "Incremental semantics" (fixing becomes an effective bound delta where supported; any uncertainty selects rebuild).
- `docs/superpowers/plans/2026-08-02-semantic-modeling-and-solve-workflows.md` — Task 8 and the "Domains and fixing" interface contract (authoritative shapes: `VariableDomain`, `SemiDomain`, `VariableFixing`, `FixingProvenance`).
- `.planning/milestones/M3-semantic-modeling-workflows/DECISIONS.md` — D6 (fixing compiles as bounds), D7 (persistent vs solve-scoped), D22 (rebuild-on-uncertainty), D28.
- `src/model/variable.rs`, `src/model/mod.rs` (`add_variable`, `variable_bounds`, `set_variable_bounds`, `remove_variable`, `commit`), `src/snapshot.rs` (`VariableEntry`), `src/delta.rs` (`ModelOp::SetVariableBounds`), `src/model/changelog.rs` (`Change`), `src/compiler/session.rs` (`compile_snapshot`/`compile_delta` — the effective-bound compile and `RebuildRequired` path).

**TDD order** (per `EXECUTION.md`):

1. Write a reference **state-machine test** in `tests/fixing_assignment.rs` over continuous/integer/binary/semi domains, bound changes, fixing, and unfix. Fixture cases: fix→effective bounds equal; unfix→declared bounds restored (not fix-time bounds — SM-05.4); fix value outside declared bounds rejected (SM-05.5); non-integer value on an integer variable rejected within/outside the named integrality tolerance; `set_variable_bounds` excluding an active fixing fails atomically and leaves revision/state unchanged (SM-05.6); `remove_variable` clears the fixing; fixing survives `commit`→`take_snapshot`→rebuild (the phase gate "fix/unfix survives rebuild").
2. Run the tests and record the expected failures (no `fix`/`unfix`/`declared_bounds`/`effective_bounds` on `Model`, no `VariableFixing`).
3. Implement:
   - `src/model/variable.rs`: public `VariableDomain { bounds: Bounds, var_type: VarType, semi: Option<SemiDomain> }`, `#[derive(Clone, Copy, Debug, PartialEq)]` per packet; `SemiDomain { Continuous { nonzero_lower: f64 }, Integer { nonzero_lower: f64 } }`; `VariableFixing { value: f64, provenance: FixingProvenance }`; `FixingProvenance { User, Imported { source: String } }`. Rebuild the internal variable record as one struct containing the declared `VariableDomain`, `Option<VariableFixing>`, activity, and name (Task 8's "replace fragmented variable state" — the P26 `VariableData { bounds, var_type, active, name }` becomes `{ domain: VariableDomain, fixing: Option<VariableFixing>, active, name }`).
   - `src/model/mod.rs`: `pub fn fix(&mut self, variable: Variable, value: f64) -> Result<(), ModelError>` and `pub fn unfix(&mut self, variable: Variable) -> Result<(), ModelError>` as typed atomic canonical mutations (SM-05.2): validate finite value, integrality within the named tolerance (SM-05.5), value inside declared bounds, live variable id; record `VariableFixing { value, provenance: FixingProvenance::User }`; emit `Change::VariableFixingChanged` and `ModelOp::SetVariableFixing { var, fixing, effective_bounds }`; commit advances revision exactly once. `pub fn declared_bounds(&self, variable: Variable) -> Option<Bounds>` (SM-05.1) and `pub fn effective_bounds(&self, variable: Variable) -> Option<Bounds>` (declared ∩ fixing). Add a named integrality tolerance (`Model::integrality_tolerance()`, setter; documented default consistent with the existing feasibility-tolerance convention) used by the fix validation.
   - Atomicity guard (SM-05.6): `set_variable_bounds` computes `effective_bounds(declared ∪ new) ∩ fixing`; if the new declared bounds exclude the active fixing value, return a typed `ModelError` with no state change (validate before commit).
   - `src/model/changelog.rs`: `Change::VariableFixingChanged { var, fixing }`.
   - `src/model/validation.rs`: invariant — every live variable with `Some(fixing)` satisfies `declared.lower <= fixing.value <= declared.upper`.
   - `src/snapshot.rs`: `VariableEntry` gains `fixing: Option<VariableFixing>`; snapshot build/take/reconstruct threads it (rebuild survival).
   - `src/delta.rs`: `ModelOp::SetVariableFixing { var: VarId, fixing: Option<VariableFixing>, effective_bounds: Bounds }` — self-contained (carries the bounds to apply; `None` fixing = unfix restores the current declared bounds). `DeltaBatch::new` accepts the variant; `ReferenceBackend::apply_op` (canonical path) handles it as a bound update.
   - `src/compiler/session.rs`: `compile_delta` lowers `SetVariableFixing` → `BackendOp::SetVariableBounds { variable: compiled_id(var), bounds: effective_bounds }` when the backend declares `BackendFeature::IncrementalBounds` (SM-05.7), else returns `CompileError::RebuildRequired` (D22). `compile_snapshot` continues to fold fixing into the compiled variable bounds (fix → `Bounds::fixed(value)`; SM-05.3). Semi-continuous domain remains rejected at the compile boundary (P26 behavior unchanged — the compiled IR has no semi-continuous representation; `VariableDomain.semi` is declared canonical state only).
   - Wire the public surface through `src/lib.rs`/`src/advanced.rs`.
4. Run `cargo test -p roml --test fixing_assignment` (must pass), then `cargo test -p roml --all-targets`, `cargo test -p roml-highs --all-targets` (HiGHS applies the fix/unfix `SetVariableBounds` deltas incrementally — SM-05.7 regression), `cargo clippy -p roml --all-targets -- -D warnings`, `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps`, `cargo fmt --all -- --check`.
5. Update evidence and traceability; stop; commit.

- [ ] Write a reference state-machine test over continuous/integer/binary/semi domains, bound changes, fixing, and unfix.
- [ ] Replace fragmented variable state with one record containing declared domain, fixing, activity, and name.
- [ ] Add named integrality tolerance and atomic fixing validation.
- [ ] Add canonical `SetVariableFixing` operation (op + change + snapshot entry).
- [ ] Compile effective bound deltas when supported; rebuild otherwise.
- [ ] Assert fix/unfix survives snapshot rebuild; declared-bound exclusion fails atomically.

**Stopping condition:** the domain/fixing state-machine test passes; `fix`/`unfix`/`declared_bounds`/`effective_bounds` behave per SM-05.1–05.6; a fixed variable's effective bounds equal `Bounds::fixed(value)`; `unfix` restores the current declared bounds; declared-bound exclusion of an active fixing returns a typed error with no revision/state change; the compiled delta emits `SetVariableBounds` under `IncrementalBounds` and `RebuildRequired` otherwise; `cargo test -p roml --all-targets` and `cargo test -p roml-highs --all-targets` are green; no `roml-mosek`/`roml-xpress` command is run.

**Commit:** `feat(model): add first-class variable fixing`

**Verification:**

```bash
cargo test -p roml --test fixing_assignment
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
cargo clippy -p roml --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
```

**Acceptance criteria:**
- `Model::fix`/`Model::unfix`/`Model::declared_bounds`/`Model::effective_bounds` exist and are typed; fixing advances the canonical revision exactly once (SM-05.2).
- A reference state-machine test covers continuous/integer/binary/semi domains, bound changes, fixing, and unfix, and asserts `unfix` restores current (not fix-time) declared bounds (SM-05.4).
- Non-finite, out-of-domain, and non-integral (beyond the named tolerance) fix values are typed errors (SM-05.5).
- `set_variable_bounds` excluding an active fixing returns a typed error with no state change (SM-05.6).
- The compiled representation of a fixing is equal lower/upper bounds (SM-05.3); the fix/unfix delta applies incrementally as `SetVariableBounds` when `IncrementalBounds` is supported, and the `roml-highs` suite stays green (SM-05.7).
- Fixing survives `commit` → snapshot → rebuild (phase gate "fix/unfix survives rebuild") — asserted by a rebuild-equivalence test.

## Task 9 — Add assignments, locks, and the SolveOverlay contract

**Phase:** P27  **Requirements:** SM-06 (all clauses), SM-02.2 (secondary), the "SolveOverlay contract" section above (issue #26 item 1)

**Read first:**
- `docs/superpowers/specs/2026-08-02-semantic-modeling-and-solve-workflows-design.md` §11 "Assignments, starts, hints, and locks", §12 "SolvePlan and reversible overlays", §4.4/§5 "Overlay identity and origins", §19 "Failure semantics".
- `docs/superpowers/plans/2026-08-02-semantic-modeling-and-solve-workflows.md` — Task 9 and the "Assignments and solve intent" interface contract (authoritative shapes: `PrimalAssignment`, `SolutionLock`, `LockSelector`, `ContinuousLock`).
- `.planning/milestones/M3-semantic-modeling-workflows/DECISIONS.md` — D4 (lineage governs assignment compatibility; instance/revision are provenance, not compatibility authority), D5 (generated entities need origins), D7, D8, D28.
- `src/compiler/origin.rs` (`OverlayId`, `GeneratedRole`, `OriginMap`), `src/compiler/backend_ir.rs` (`CompilationId`, `CompiledVariableId`, `CompiledLinearRow`), `src/compiler/session.rs` (current-compilation + id maps), `src/solution/mod.rs` (`Solution`/`SolutionBuilder`/value storage keyed by user `Variable`), `src/solution/metadata.rs` (`SolveMetadata`), `src/model/mod.rs` (`declared_bounds`, `active_objective` from Task 8).

**TDD order** (per `EXECUTION.md`):

1. Write failing tests in `tests/solve_overlay.rs`:
   - **Assignment compatibility (SM-02.2, SM-06.1):** `PrimalAssignment` stores a partial value map with `lineage` plus optional `source_instance`/`source_revision`; it makes no feasibility/optimality claim. `validate_for(&model)` passes for a same-lineage live variable set with in-domain values; fails with `LineageMismatch` for an independent lineage; fails with `StaleVariable` for a removed/stale-generation variable; fails with `ValueOutOfBounds` for an out-of-domain value. **Instance/revision are provenance, not compatibility authority**: two clones at the same revision with the same lineage both validate (D4).
   - **`Solution::primal_assignment` (SM-06.2):** a solved `Solution` produces an assignment whose `lineage` equals the solved model's lineage and whose values are the user-variable solution values, excluding any compiler-only variable; `PrimalAssignment::subset(&[...])` restricts the value map to the named variables.
   - **Lock selectors and continuous bands (SM-06.3, SM-06.4, SM-06.5):** `LockSelector::{AllAssigned, IntegerAssigned, BinaryAssigned, Variables(set), Except(set)}` each resolve to the expected variable subset over a fixture assignment; `ContinuousLock::Exact` and `ContinuousLock::Within { absolute }` produce the expected bounds; `Within` on an integer/binary variable is a typed error.
   - **Overlay contract compile (issue #26 item 1):** `compile_overlay` maps a `SolveOverlay` (`temporary_fixings`/`locks`/`objective_locks`/`cutoffs`) + optional objective override against the exact current `CompilationId` into a `CompiledOverlay` with a fresh `compilation_id != base_compilation`, the correct `OverlayOp` sequence, and a `SolveOverlay` origin for every added temporary row; a stale base id returns a typed error; an invalid lock/assignment fails before any op is produced (SM-06.6 — "fail before backend mutation").
2. Run the tests and record the expected failures (missing types/methods).
3. Implement:
   - `src/assignment.rs`: `PrimalAssignment { lineage: ModelLineageId, source_instance: Option<ModelInstanceId>, source_revision: Option<ModelRevision>, values: std::collections::BTreeMap<Variable, f64> }` (packet shape); `pub fn validate_for(&self, model: &Model) -> Result<(), AssignmentError>` (lineage equality, live generation via the model's id validation, value within declared bounds — tolerance-aware for integrality); `pub fn subset(&self, variables: &[Variable]) -> PrimalAssignment`; `pub fn value(&self, variable: Variable) -> Option<f64>`; `AssignmentError { LineageMismatch { expected, actual }, StaleVariable { variable }, ValueOutOfBounds { variable, value, bounds } }`. `SolutionLock { assignment: PrimalAssignment, selector: LockSelector, continuous: ContinuousLock }`; `LockSelector { AllAssigned, IntegerAssigned, BinaryAssigned, Variables(std::collections::BTreeSet<Variable>), Except(std::collections::BTreeSet<Variable>) }`; `ContinuousLock { Exact, Within { absolute: f64 } }` — all packet shapes.
   - `src/solution/mod.rs`: `pub fn primal_assignment(&self) -> PrimalAssignment` — binds `self.metadata().model_lineage` (CR-02 pattern: real solved lineage, never `SolveMetadata::default()`), `source_instance: self.metadata().model_instance`, `source_revision: self.metadata().model_revision`, and the user-variable values already stored on the `Solution` (compiler-only variables are excluded structurally at extraction — the values are keyed by user `Variable`).
   - `src/solver/overlay.rs` (this task defines the types and the compiler; execution lands in Task 10): `SolveOverlay`, `ObjectiveLock { objective: Objective, absolute_tolerance: f64, relative_tolerance: f64 }`, `ObjectiveCutoff { objective: Objective, limit: f64, direction: CutoffDirection }` with `CutoffDirection { Upper, Lower }`, `CompiledOverlay`, `#[non_exhaustive] OverlayOp`, `OverlayError { StaleCompilation { expected, actual }, Assignment(AssignmentError), ObjectiveNotFound(Objective), IdentityOverflow }`, and `pub fn compile_overlay(model: &Model, compiler: &CompilationSession, overlay: &SolveOverlay, objective_override: Option<ObjId>) -> Result<CompiledOverlay, OverlayError>` implementing the "Overlay-to-compiled-IR mapping" section exactly (selector resolution, band validation, row construction, `SetObjectivePolicy` for the override, `SolveOverlay` origins in `origin_additions`, fresh `CompilationId::allocate` for the overlay state, stale-`base_compilation` rejection).
   - `src/compiler/origin.rs`: `GeneratedRole` gains `ObjectiveLockRow` and `CutoffRow` (enum stays `#[non_exhaustive]`); `OverlayId::allocate` used by `SolveOverlay` construction.
   - `src/compiler/session.rs`: add `pub(crate) fn compiled_variable_id(&self, v: VarId) -> Option<CompiledVariableId>` and `pub(crate) fn compiled_objective_id(&self, o: ObjId) -> Option<CompiledObjectiveId>` (forward id resolution for the overlay compiler; additive, no behavior change).
   - `src/lib.rs`/`src/advanced.rs`: export `PrimalAssignment`, `SolutionLock`, `LockSelector`, `ContinuousLock`, `SolveOverlay`, `ObjectiveLock`, `ObjectiveCutoff`, `OverlayError` through the reviewed public surface (SM-06.3: these are public distinct types, never conflated with `MipStart`/`VariableHints` which remain P28).
4. Run `cargo test -p roml --test solve_overlay -- --nocapture` (must pass), then `cargo test -p roml --all-targets`, `cargo clippy -p roml --all-targets -- -D warnings`, `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps`, `cargo fmt --all -- --check`.
5. Update evidence and traceability; stop; commit.

- [ ] Test assignment compatibility by lineage, generation, and value/domain validation; instance/revision are provenance, not compatibility authority.
- [ ] Implement `Solution::primal_assignment()` excluding compiler-only variables.
- [ ] Test all lock selectors and continuous bands.
- [ ] Define the `SolveOverlay` contract: contents, objective-override policy interaction, compilation-ID tagging, overlay lifecycle, overlay-to-compiled-IR mapping (this plan's contract section is authoritative).
- [ ] Implement `compile_overlay` against one exact `CompilationId` with stale rejection and before-mutation validation.

**Stopping condition:** `PrimalAssignment`/`SolutionLock`/`LockSelector`/`ContinuousLock` are public packet-shaped types; `Solution::primal_assignment()` returns a lineage-bound partial assignment over user variables; `validate_for` gates on lineage + generation + value/domain only (instance/revision recorded as provenance, never authority); all five selectors and both continuous locks compile and validate; `compile_overlay` produces the enumerated `CompiledOverlay`/`OverlayOp` mapping against the exact base `CompilationId`, rejects a stale base and an invalid assignment **before** producing any op (SM-06.6), and tags every added row with a `SolveOverlay` origin (D5).

**Commit:** `feat(solve): add assignments and solution locks` (the authoritative Task 9's first half — the contract/types; the second half, `feat(solve): add reversible solve overlays`, is Task 10)

**Verification:**

```bash
cargo test -p roml --test solve_overlay -- --nocapture
cargo test -p roml --all-targets
cargo clippy -p roml --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
```

**Acceptance criteria:**
- `PrimalAssignment` is a partial value map with lineage plus instance/revision provenance and makes no feasibility/optimality claim (SM-06.1).
- `Solution::primal_assignment()` returns a lineage-bound assignment whose values are the solution's user-variable values, excluding compiler-only variables; `PrimalAssignment::subset` produces selected subsets (SM-06.2).
- `SolutionLock`, `MipStart`, `VariableHints`, and persistent `VariableFixing` are distinct public types (SM-06.3; starts/hints land as types in P28 — the distinction is documented here per D8, and no conversion exists without explicit policy).
- All five `LockSelector` variants and both `ContinuousLock` variants behave as specified; `Within` on a non-continuous variable is a typed error (SM-06.4, SM-06.5).
- Stale or unrelated assignments fail before backend mutation via `validate_for` gating (SM-06.6).
- `compile_overlay` implements the pinned `SolveOverlay` contract: contents enumerated, objective override maps to `SetObjectivePolicy`, the result tags the override solve with a fresh `CompilationId` distinct from the base, and the lifecycle runs against the exact base `CompilationId` (issue #26 item 1).

## Task 10 — Execute reversible overlays transactionally

**Phase:** P27  **Requirements:** SM-07.3, SM-07.4, SM-07.5, SM-07.6

**Read first:**
- `docs/superpowers/specs/2026-08-02-semantic-modeling-and-solve-workflows-design.md` §12 "SolvePlan and reversible overlays" (the lifecycle), §18, §19 "Failure semantics", §2 binding invariant 9 (failed overlay rollback forces backend rebuild before reuse).
- `docs/superpowers/plans/2026-08-02-semantic-modeling-and-solve-workflows.md` — Task 9 (second half) and the "Solve plan and exact result identity" contract.
- `.planning/milestones/M3-semantic-modeling-workflows/DECISIONS.md` — D7 invariant ("rollback uncertainty forces backend rebuild"), D22, D28.
- `src/solver/facade.rs` (`solve_with`: the sync → solve → normalize → CompilationMismatch path and where the overlay lifecycle hooks), `src/solver/session.rs` (`Synchronization`, the optional-trait pattern of `SessionHealth`/`SolutionView`), `src/solver/request.rs` (`SolveResult.compilation_id`), `src/solver/reference.rs` (`ReferenceBackend` compiled projection — the correctness reference for apply/rollback), `roml-highs/src/session.rs` (`synchronize` compiled path, `current_compilation`, `var_bounds` tracking), `roml-highs/src/compiler.rs` (`BackendOp` → native translation), `roml-highs/src/bindings.rs` (native `Highs_changeColBounds`/`Highs_addRows`/`Highs_deletionRow` availability per the pinned `highs-sys`).

**TDD order** (per `EXECUTION.md`):

1. Write failing tests in `tests/solve_overlay.rs`:
   - **Transactional apply/rollback (SM-07.4):** apply a `CompiledOverlay` (fixings + locks + a cutoff row) to a `ReferenceBackend`, solve, roll back, verify the backend's compiled state and `current_compilation` return exactly to `C_base`; the backend is solveable again with the base objective. Apply → solve with an **objective override**, assert the result's `compilation_id == C_overlay` (the override solve is recorded by the distinct id) and that `result.compilation_id != compiler.current_compilation()` is handled correctly by the facade (no false mismatch).
   - **Revision invariance (SM-07.3):** after the full overlay solve, `model.current_revision()` is unchanged; no `Change`/`ModelOp` records an overlay effect.
   - **Stale rejection before mutation (phase gate):** a `CompiledOverlay` whose `base_compilation` does not match the backend's current compiled state is rejected with typed `OverlayError::StaleCompilation` and the backend's native/compiled state and `current_compilation` are unchanged.
   - **Rollback uncertainty → RequiresRebuild (SM-07.5):** a fault-injecting backend that fails rollback mid-way returns `OverlayRollbackOutcome::RequiresRebuild` and the session health becomes `RequiresRebuild`; the next solve forces a snapshot rebuild.
   - **Failure injection (SM-07.6):** inject a backend fault at each of validation, compile, apply, solve, extraction (result `CompilationId` mismatch), rollback, and post-rollback verification; for every stage assert (a) the canonical revision is unchanged and (b) a subsequent clean solve equals a fresh rebuild (no overlay leaks into a later solve). For apply-stage failure assert the backend is not left partially overlaid (rebuild-required or rolled back).
2. Run the tests and record the expected failures (no overlay execution, no `OverlaySession`).
3. Implement:
   - `src/solver/session.rs`: optional `pub trait OverlaySession { fn apply_overlay(&mut self, overlay: &CompiledOverlay) -> Result<OverlayApplyReceipt, BackendError>; fn rollback_overlay(&mut self, receipt: &OverlayApplyReceipt) -> Result<OverlayRollbackOutcome, BackendError>; fn verify_overlay_clean(&mut self) -> Result<(), BackendError>; }` — a bounded optional trait alongside `SessionHealth`/`SolutionView` (design §12: fallible rollback is explicit, not delegated solely to `Drop`).
   - `src/solver/overlay.rs`: `OverlayApplyReceipt { overlay_id: OverlayId, base_compilation: CompilationId, applied_compilation: CompilationId }` and `OverlayRollbackOutcome { Clean { restored_compilation: CompilationId }, RequiresRebuild { reason: String } }`.
   - `src/solver/reference.rs`: `OverlaySession` for `ReferenceBackend` — apply records prior compiled bounds, applies `SetTemporaryVariableBounds`, adds temporary rows (dense temporary compiled row ids distinct from compiled rows), sets the overlay objective policy; `current_compilation` → `C_overlay`; rollback restores prior bounds, removes temporary rows, restores the base policy, `current_compilation` → `C_base`; verify asserts the compiled maps equal the base.
   - `roml-highs/src/compiler.rs` + `roml-highs/src/session.rs`: `OverlaySession` for the HiGHS session — apply translates each `OverlayOp`: `SetTemporaryVariableBounds` → `Highs_changeColBounds` (recording the prior bounds from `self.var_bounds`), `AddTemporaryRow` → `Highs_addRows` (recording the native row index in an overlay-row map keyed by a temporary compiled row id), `SetObjectivePolicy` → the compiled-policy projection already used for `BackendOp::SetObjectivePolicy`; `current_compilation` → `C_overlay`. Rollback: restore each prior bound, `Highs_deletionRow` each added row, restore the base policy, `current_compilation` → `C_base`; any failing native call returns `RequiresRebuild`. Verify: the native row count and the session's compiled-state maps match `C_base`.
   - `src/solver/facade.rs`: `pub fn solve_with_overlay(&mut self, model: &mut Model, options: SolveOptions, overlay: &SolveOverlay, objective_override: Option<ObjId>) -> Result<Solution, SolveError>` implementing the pinned lifecycle: validate options → commit → synchronize exactly as `solve_with` (so `C_base` is established) → `compile_overlay` against `compiler.current_compilation()` → `apply_overlay` → solve → validate `result.compilation_id == compiled_overlay.compilation_id` (NOT `compiler.current_compilation()`, which stays `C_base`) → rollback (always attempted, even on solve/extraction failure) → on `RequiresRebuild` mark the backend `RequiresRebuild` → verify `C_base` restored → normalize the result with `compilation_id = C_overlay` and `overlay_id = Some(overlay.id)` in the metadata. A panic-safe best-effort guard may additionally roll back, but the explicit rollback call is the mechanism (SM-07.4).
   - `src/solver/request.rs`/`src/solution/metadata.rs`: `overlay_id: Option<OverlayId>` on `SolveResult`/`SolveMetadata` (`Default` → `None`; the facade sets `Some(overlay.id)` on overlay solves), so overlay artifacts and results agree on the exact `CompilationId`/`OverlayId` (D28, SM-03.9 overlay artifacts).
   - `src/solver/error.rs`: map `OverlayError`/apply/rollback `BackendError`s into `SolveError` variants; the extraction-mismatch path reuses `SolveError::CompilationMismatch`.
   - Wire `solve_with_overlay` through the public surface. P28 folds this entry into `SolvePlan` and routes `solve`/`solve_with` through one executor; P27 does not change `solve`/`solve_with` signatures (D27).
4. Run `cargo test -p roml --test solve_overlay -- --nocapture`, `cargo test -p roml --all-targets`, `cargo test -p roml-highs --all-targets`, `cargo clippy -p roml --all-targets -- -D warnings`, `cargo clippy -p roml-highs --all-targets -- -D warnings`, `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps`, `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps`, `cargo fmt --all -- --check`.
5. Update evidence and traceability (append the phase gate: failure matrix, leak checks, revision-invariance, HiGHS incremental evidence); stop; request P27 review.

- [ ] Implement explicit fallible apply/rollback receipts; do not rely only on `Drop`.
- [ ] Compile overlays against one exact `CompilationId`; reject a stale base before mutation.
- [ ] Inject failure during validation, apply, solve, extraction, rollback, and post-rollback verification.
- [ ] Assert every later clean solve equals fresh rebuild and no canonical revision changed.
- [ ] Qualify HiGHS temporary bounds/rows through pinned `highs-sys` bindings with typed unsupported behavior.

**Stopping condition:** overlay apply/rollback is transactional from the caller's perspective (SM-07.4); a full overlay solve (fixings + locks + cutoff + objective override) leaves `ModelRevision` unchanged (SM-07.3); a stale base `CompilationId` rejects before mutation (phase gate); rollback uncertainty marks `RequiresRebuild` (SM-07.5); the injected-failure matrix proves no overlay leaks into any later solve and every later clean solve equals a fresh rebuild (SM-07.6); the override solve's result is tagged with the fresh `C_overlay` and `overlay_id`; the HiGHS temporary bounds/rows path is implemented through pinned `highs-sys` APIs and version-qualified.

**Commit:** `feat(solve): add reversible solve overlays` (the authoritative Task 9's second half; requests the P27 phase review)

**Verification:**

```bash
cargo test -p roml --test solve_overlay -- --nocapture
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
cargo clippy -p roml --all-targets -- -D warnings
cargo clippy -p roml-highs --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps
```

**Acceptance criteria:**
- Temporary fixings, solution locks, objective-lock rows, and cutoffs never mutate the canonical model revision (SM-07.3) — asserted by a revision-invariance test.
- Overlay application and rollback are transactional from the caller's perspective, with explicit `OverlayApplyReceipt`/`OverlayRollbackOutcome` and rollback always attempted on failure (SM-07.4).
- Rollback uncertainty marks the backend `RequiresRebuild` and the next solve rebuilds (SM-07.5).
- The failure-injection matrix (validation/compile/apply/solve/extraction/rollback/post-rollback) proves no overlay leaks into a later solve; each later clean solve equals a fresh rebuild (SM-07.6) — the phase gate "no overlay leaks after any injected failure".
- Exact `CompilationId` mismatches reject before mutation; the override solve's result records the fresh overlay `CompilationId` and `overlay_id` (phase gate "exact compilation mismatches reject before mutation"; SM-03.9 overlay artifacts).
- Locks never advance the model revision (phase gate "locks never advance model revision").

## Waves and parallelization

**This is a single-plan phase; all three tasks are strictly serial (waves 1 → 2 → 3).**

- **Task 8 (wave 1)** owns canonical domain/fixing state: `src/model/variable.rs`, `src/model/mod.rs`, `src/model/changelog.rs`, `src/model/validation.rs`, `src/snapshot.rs`, `src/delta.rs`, `src/compiler/session.rs` (effective-bound compile), `tests/fixing_assignment.rs`.
- **Task 9 (wave 2)** owns `src/assignment.rs`, `src/solution/mod.rs`, `src/solver/overlay.rs` (types + compiler), `src/compiler/origin.rs`, and forward id-resolution accessors in `src/compiler/session.rs`, `tests/solve_overlay.rs`.
- **Task 10 (wave 3)** owns the overlay execution: `src/solver/overlay.rs` (apply/rollback/receipts on the SAME module Task 9 authors), `src/solver/session.rs`, `src/solver/facade.rs`, `src/solver/error.rs`, `src/solver/request.rs`, `src/solution/metadata.rs`, `roml-highs/src/session.rs`, `roml-highs/src/compiler.rs`, `tests/solve_overlay.rs` (same test file Task 9 authors).

**Merge-conflict / dependency rationale for serial execution (Task 8 ∥ Task 9 analysis):**

- **Shared file:** both Task 8 and Task 9 modify `src/lib.rs`/`src/advanced.rs` (Task 8 re-exports `VariableDomain`/`VariableFixing`; Task 9 re-exports `PrimalAssignment`/`SolutionLock`/overlay types) and Task 8's `src/compiler/session.rs` is extended again by Task 9 (id-resolution accessors). A shared module-wiring file forces serialization under the wave rule.
- **Semantic dependency:** Task 9's `validate_for` value/domain check requires Task 8's `Model::declared_bounds`/`VariableDomain`; Task 9's `compile_overlay` temp-fixing/lock validation requires Task 8's declared-domain API (SM-02.2/SM-06.6 "fail before backend mutation"). Task 9 is not buildable or correct before Task 8.
- **Same-file authorship:** Task 9 and Task 10 both author `src/solver/overlay.rs` and `tests/solve_overlay.rs` (Task 9 defines types/compiler; Task 10 implements execution on the same module). This is the strongest serialization constraint and matches the D26/M3 one-coding-phase WIP rule.
- Parallelizing Task 8 and Task 9 would save little (types are cheap) while forcing a manual merge of the shared `lib.rs`/`advanced.rs` wiring and risking a fix/temp-fixing domain mismatch. The P26 precedent of parallel content-independent tasks (5/6) does not apply: those shared only a one-line `mod.rs` wiring, whereas P27 shares semantics.

Per `DECISIONS.md` D26 and `EXECUTION.md` § "WIP and parallelism", default WIP is one coding branch and one review/fix branch; this phase uses one worktree with the three serial tasks.

## Verification

Phase gate commands (per-crate only; `roml-mosek`/`roml-xpress` are out of scope and never run):

```bash
cargo fmt --all -- --check
cargo check -p roml --all-targets
cargo check -p roml-highs --all-targets
cargo clippy -p roml --all-targets -- -D warnings
cargo clippy -p roml-highs --all-targets -- -D warnings
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps
```

Per-`EXECUTION.md` "Phase-specific mandatory checks — P27–P28":

- overlay failure injection (validation/compile/apply/solve/extraction/rollback/post-rollback);
- model revision invariance under temporary operations (overlay solve leaves `ModelRevision` unchanged);
- subsequent-solve leak tests (clean solve after overlay equals fresh rebuild);
- capability/effective-plan assertions (overlay requires `IncrementalBounds`; unsupported features typed, never silent).

### Safety and failure model (the phase's threat register)

The overlay is the phase's attack surface. The register, with the mitigation each gate enforces:

| Threat | Component | Severity | Disposition | Mitigation |
|--------|-----------|----------|-------------|------------|
| Stale overlay/result applied against a different compiled state | `compile_overlay`, façade extraction | high | mitigate | Exact `CompilationId` authority (D28): stale base rejected with typed `OverlayError::StaleCompilation`/`CompileError::StaleCompilation` before any mutation; result must equal the overlay state's `C_overlay` (phase gate "exact compilation mismatches reject before mutation") |
| Overlay leak into a later solve | `solve_with_overlay`/rollback | critical | mitigate | Explicit transactional rollback always attempted; post-rollback verification restores `C_base`; failure-injection matrix + subsequent-solve equality (phase gate "no overlay leaks after any injected failure") |
| Overlay mutates canonical history | canonical change/commit path | high | mitigate | Overlay ops never emit `Change`/`ModelOp`/revision advance; revision-invariance test (SM-07.3, phase gate "locks never advance model revision") |
| Rollback uncertainty leaves inconsistent backend | backend rollback path | high | mitigate | `OverlayRollbackOutcome::RequiresRebuild` marks the session `RequiresRebuild`; next solve forces snapshot rebuild before reuse (SM-07.5, D7) |
| Partially applied overlay on apply failure | `apply_overlay` | high | mitigate | Backends preflight against the exact base and record prior bounds/rows before mutating; a failed apply returns `RequiresRebuild` (never a half-overlaid state that is silently reused) |
| Invalid assignment/lock mutating backend | `validate_for`/`compile_overlay` | medium | mitigate | Lineage + generation + value/domain validated before any `OverlayOp` is produced (SM-06.6, SM-02.2) |
| Unsupported native API misuse | `roml-highs` overlay path | medium | mitigate | Temporary bounds/rows only through pinned `highs-sys` official bindings; unqualified/unsupported behavior is a typed error, never a guess (EXECUTION native-API protocol) |

## Review gates

Two-stage independent review at the P27 boundary per `EXECUTION.md` § "Review gates". P0/P1 findings block merge; P2 findings may merge only when explicitly accepted and scheduled.

- **Pass 1 — Specification and correctness:** requirement coverage (SM-05, SM-06, SM-07.3–07.6, SM-02.2); `SolveOverlay` contract fidelity (issue #26 item 1 — contents, objective-override policy interaction, compilation-ID tagging, lifecycle); semantic correctness of fixing state machine and lock selectors/bands; assignment lineage/generation/domain validation; origin completeness for overlay-generated rows; unsupported/error behavior; API coherence and public-surface placement.
- **Pass 2 — Integration and operations:** incremental/rebuild behavior of fixing deltas; overlay failure recovery and leak matrix; rollback-uncertainty → `RequiresRebuild`; exact-`CompilationId` stale rejection across both backends; public API diff and package impact; HiGHS temporary bounds/rows version-qualification evidence; revision-invariance evidence.

## Artifacts this phase produces

Public/semantic types:

- `VariableDomain`, `SemiDomain`, `VariableFixing`, `FixingProvenance` (declared-domain/fixing separation, SM-05.1).
- `PrimalAssignment`, `SolutionLock`, `LockSelector`, `ContinuousLock` (SM-06.1–06.5).
- `SolveOverlay` (contents enumerated in this plan), `ObjectiveLock`, `ObjectiveCutoff`, `OverlayId`, `CompiledOverlay`, `OverlayOp`, `OverlayApplyReceipt`, `OverlayRollbackOutcome`, `OverlayError` (issue #26 item 1).
- `AssignmentError`; `Model::fix`/`Model::unfix`/`declared_bounds`/`effective_bounds`; `Solution::primal_assignment`; `SolveMetadata.overlay_id`.

Module paths:

- `src/model/variable.rs`, `src/model/mod.rs`, `src/model/changelog.rs`, `src/model/validation.rs`, `src/snapshot.rs`, `src/delta.rs`, `src/compiler/session.rs`, `src/compiler/origin.rs`
- `src/assignment.rs`, `src/solver/overlay.rs`, `src/solver/session.rs`, `src/solver/facade.rs`, `src/solver/request.rs`, `src/solver/error.rs`, `src/solution/mod.rs`, `src/solution/metadata.rs`
- `roml-highs/src/session.rs`, `roml-highs/src/compiler.rs`

Test files:

- `tests/fixing_assignment.rs` (domain/fixing state machine, tolerance, atomicity, rebuild survival, incremental sync)
- `tests/solve_overlay.rs` (assignment compatibility, lock selectors/bands, overlay compile/apply/rollback, failure injection, leak tests, revision invariance)

Evidence:

- `docs/release/evidence/M3_P27_FIXING_LOCKS_OVERLAYS.md` — declared/effective domain tests; continuous/integer/binary fixing matrix; fix/unfix bound-update traces; assignment lineage/stale-handle tests; overlay exact-compilation-ID validation; overlay apply/rollback failure matrix; subsequent-solve leak checks; HiGHS incremental bound evidence. Closes SM-05, SM-06, SM-07.3–SM-07.6 (and SM-02.2), per `TRACEABILITY.md`'s P27 evidence requirements.

## Gate

P27 passes when all four ROADMAP gate clauses hold and are evidenced:

1. **fix/unfix survives rebuild** — a fixed/unfixed model's compiled effective bounds after `commit` → snapshot rebuild equal the incremental path.
2. **locks never advance model revision** — a full overlay solve leaves `ModelRevision` unchanged.
3. **exact compilation mismatches reject before mutation** — a stale overlay/result `CompilationId` is a typed error with no backend mutation.
4. **no overlay leaks after any injected failure** — after failure at any lifecycle stage and rollback, every later clean solve equals a fresh rebuild.

Plus: per-crate `roml`/`roml-highs` gate commands exit 0; `docs/release/evidence/M3_P27_FIXING_LOCKS_OVERLAYS.md` records the full failure matrix and evidence; two-stage independent review resolves all P0/P1 findings; `TRACEABILITY.md` clause-level closure is updated; M3 `STATE.md` is updated only with verified facts.
