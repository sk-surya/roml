# Phase 10 CONTEXT: Backend contract migration closure

**Date:** 2026-07-18
**Phase:** 10 (M1R-01) — Backend contract migration closure
**Goal:** Make the revisioned snapshot/delta/session contract the ONLY supported semantic execution path and retire destructive legacy behavior.
**Requirements:** M1R-C1–C8
**Depends on:** M1R-00 (Phase 9) — complete, gate PASS

## Domain

The ROML core currently exposes a legacy backend contract (`SolverAdapter` + `SolverModelExt`) that uses destructive `drain_changes()`, model-owned transient `SolveOptions`, best-effort silently ignored options, cloned `HashMap` solution access, and a single-adapter synchronous pipeline. The target contract — prototyped in the worktree `phase-roml-P0-release-baseline` — uses revisioned `DeltaBatch`/`ModelOp` sync, `Journal`-based non-destructive replay, independent `AdapterCursor` per session, explicit `AdapterHealth`, immutable `SolveRequest` with explicit negotiation, and a `SolveResult` containing typed sub-structures.

Phase 9 truth reset confirmed that M1.1 is FAILED — the legacy path remains the public API despite protocol types being frozen. This phase MUST bridge that gap by making the revisioned protocol the ONLY supported path.

The worktree already contains the key types: `ReferenceBackend`, `DeltaBatch`, `ModelOp`, `ApplyOutcome`, `AdapterCursor`, `AdapterHealth`, `ModelRevision`, `ModelSnapshot`, `Journal`, `SyncCoordinator`, `SolveRequest`, `SolveResult`, `TerminationStatus`, `EffectiveConfig`, `SolveSolution`, `BackendError`, `ErrorCategory`, `BackendCapabilities`, `HealthEffect`. What's missing: a `BackendSession` trait, a `SolutionView` type, and integration wiring from `Model` to the session protocol.

## Canonical refs (MUST read before planning)

- `.planning/ROADMAP.md` — M1R-01 section, M1R phase graph, parallel execution policy
- `.planning/STATE.md` — Phase ledger, M1R base SHA (main@ef37c88, candidate@649c635), state vocabulary
- `.planning/REQUIREMENTS.md` — M1R-C1–C8 backend contract requirements
- `.planning/DECISIONS.md` — D-002 (revisioned execution), D-003 (compatibility shim constraints), D-006 (no false callback uniformity), D-008 (layered evidence), D-010 (performance follows correctness)
- `.planning/TRACEABILITY.md` — Evidence directory convention, phase evidence rule
- `.planning/phases/10-backend-contract-migration-closure/phase.md` — Full phase packet with 7 tasks, target flow, verification commands, gate criteria
- `docs/release/evidence/M1R/M1R-00-ADMISSION.md` — Phase 9 admission: legacy source patterns, pinned test inventory, residual risks
- **Worktree types (the new protocol foundation):**
  - `.claude/worktrees/phase-roml-P0-release-baseline/src/delta.rs` — `ModelOp` (16 variants), `DeltaBatch`
  - `.claude/worktrees/phase-roml-P0-release-baseline/src/revision.rs` — `ModelRevision`
  - `.claude/worktrees/phase-roml-P0-release-baseline/src/sync.rs` — `AdapterCursor`, `AdapterHealth`, `ApplyOutcome`, `SyncCoordinator`
  - `.claude/worktrees/phase-roml-P0-release-baseline/src/journal.rs` — `Journal`
  - `.claude/worktrees/phase-roml-P0-release-baseline/src/snapshot.rs` — `ModelSnapshot`
  - `.claude/worktrees/phase-roml-P0-release-baseline/src/solver/reference.rs` — `ReferenceBackend`, `NormalizedView`
  - `.claude/worktrees/phase-roml-P0-release-baseline/src/solver/request.rs` — `SolveRequest`, `SolveResult`, `EffectiveConfig`, `SolveSolution`
  - `.claude/worktrees/phase-roml-P0-release-baseline/src/solver/backend.rs` — `TerminationStatus`, `BackendError`, `BackendCapabilities`, `ErrorCategory`, `HealthEffect`
- **Current source (the legacy surface to replace):**
  - `src/solver/mod.rs` — `SolverAdapter` trait (13 methods), `SolverModelExt` trait, `SolverStatus`, `SolverError`, `SolveOptions`, `LpAlgorithm`
  - `src/solver/callback.rs` — `CallbackHandler`, `CallbackData`, `CallbackCut`, `CallbackAction`
  - `src/model/mod.rs` — `Model` struct (with `solver_options` field line 112, `drain_changes()` line 625)
  - `src/model/changelog.rs` — `Change` (16 variants), `ChangeLog` (destructive `drain()`)
  - `src/solution/mod.rs` — `Solution`, `SolutionBuilder`, `SolutionStore` (HashMap-based)
  - `src/lib.rs` — Crate root, prelude (re-exports legacy types)
- **Pinned tests (to be deleted and replaced):**
  - `.claude/worktrees/phase-roml-P0-release-baseline/tests/sync_characterization.rs` — 7 tests proving destructive drain_changes brokenness (all `#[ignore]`)
  - `.claude/worktrees/phase-roml-P0-release-baseline/tests/model_characterization.rs` — 2 tests: semicontinuous partial apply + solve options on model (`#[ignore]`)

## Locked requirements (M1R-C1–C8)

| ID | Requirement | Evidence expected |
|---|---|---|
| M1R-C1 | Supported synchronization consumes `DeltaBatch` through independent adapter cursors; no supported path destructively drains before acknowledgement | Journal-based sync, AdapterCursor advancement, no `drain_changes()` in public API |
| M1R-C2 | Canonical `Model` contains no transient solve policy | Remove `Model.solver_options` field; SolveRequest owns all solve policy |
| M1R-C3 | Every requested option/capability is applied, adjusted with reason, or rejected | SolveResult.effective_configuration with adjustments/rejections |
| M1R-C4 | Adapter health is explicit: ready, retryable, rebuild-required, terminal | AdapterHealth enum on AdapterCursor |
| M1R-C5 | Snapshot rebuild and complete incremental application are observationally equivalent | NormalizedView comparison (commuting square proven by ReferenceBackend tests) |
| M1R-C6 | Public status/error/solution contracts preserve incumbent, proof, limits, interruption, ambiguity, native code, operation, and recoverability | TerminationStatus (11 variants vs current 8), BackendError with ErrorCategory, SolveSolution with optional primal/dual/reduced costs |
| M1R-C7 | Legacy `SolverAdapter`/`SolverModelExt` is removed; no destructive shim remains | Remove both traits from `src/solver/mod.rs`; remove from prelude; no `#[deprecated]` wrapper needed (see D2) |
| M1R-C8 | All P1/P2 characterization tests execute or are deleted with requirement-backed disposition; none remain ignored | 9 pinned tests deleted; new contract tests (Task 01.2) cover the same invariants |

## Codebase context

- **Main branch (`main@ef37c88`):** Solver-free core hardening (PR #3). Has `Model` with destructive `drain_changes()`, `SolverAdapter` trait, `Change` type. No revisioned protocol types in `src/`.
- **Candidate branch (`649c635`):** 20 commits ahead of main. Has worktree-level prototyped types (ReferenceBackend, DeltaBatch, etc.) AND still exposes the legacy path. M1.1 claims "contract freeze" but the legacy surface contradicts this.
- **Worktree (`phase-roml-P0-release-baseline`):** Contains the full revisioned protocol types at `/Users/skrishnan/repos/roml/.claude/worktrees/phase-roml-P0-release-baseline/src/`. These are the foundation for M1R-01. Key files:
  - `src/delta.rs` (151 lines) — `ModelOp` + `DeltaBatch`
  - `src/revision.rs` (93 lines) — `ModelRevision` + `RevisionError`
  - `src/sync.rs` (198 lines) — `AdapterCursor`, `AdapterHealth`, `ApplyOutcome`, `SyncCoordinator`
  - `src/journal.rs` (94 lines) — `Journal` (BTreeMap<ModelRevision, DeltaBatch>)
  - `src/snapshot.rs` (210 lines) — `ModelSnapshot`
  - `src/solver/reference.rs` (351 lines) — `ReferenceBackend` + `NormalizedView`
  - `src/solver/request.rs` (175 lines) — `SolveRequest`, `SolveResult`, `EffectiveConfig`, `SolveSolution`
  - `src/solver/backend.rs` (222+ lines) — `TerminationStatus`, `BackendError`, `BackendCapabilities`
- **9 pinned tests:** All use the legacy API (`SolverAdapter`, `drain_changes()`, `Model.solver_options`). They test broken behavior of the destructive changelog. They are not salvageable — they import types being removed.
- **No `BackendSession` trait exists.** The worktree has the concrete `ReferenceBackend` struct and the synchronization primitives, but no trait that unifies lifecycle, synchronization, solve, and extraction. This phase must design and implement that trait.
- **No `SolutionView` exists.** The current `Solution` uses cloned `HashMap<VarId, f64>`. The worktree's `SolveSolution` has `variable_values: Vec<(VarId, f64)>` but no borrowed/indexed view abstraction.
- **Callback infrastructure is stable.** `CallbackHandler`, `CallbackData`, `CallbackCut`, `CallbackAction` are solver-agnostic types that don't need to change. The migration only affects how they're registered (through the new session trait rather than `SolverAdapter::set_callback_handler`).

## Decisions

### D1: BackendSession surface — separated bounded traits

**Decision:** Use separated bounded traits decomposing lifecycle, synchronization, solve, and extraction into independently implementable interfaces. **NOT** a single monolithic `BackendSession` trait.

**Core trait:**
```rust
pub trait BackendSession {
    /// Apply a delta batch or rebuild from a snapshot to synchronize state.
    fn synchronize(&mut self, sync: Synchronization) -> Result<SyncReceipt, BackendError>;
    
    /// Solve with the given request, returning structured result.
    fn solve(&mut self, request: &SolveRequest) -> Result<SolveResult, BackendError>;
    
    /// Close the session, releasing native resources.
    fn close(self) -> Result<(), BackendError>;
}
```

**Separate bounded traits:**
- `SessionHealth` — exposes `AdapterHealth` (ready/retryable/rebuild-required/terminal) and cursor position
- `SolutionView` — borrowed/indexed access to primal values, duals, reduced costs from the last solve; lifetime-bound to the session, not cloned
- `CallbackSession` — register/unregister `CallbackHandler` (only for backends that support callbacks)
- `BackendMetadata` — version, build info, index width, capability declaration

**Why:** 
1. The phase packet explicitly says: "Avoid one giant trait when lifecycle, synchronization, solve, and extraction can be separate bounded interfaces."
2. The existing `SolverAdapter` is a 13-method monolith — this is the anti-pattern we're replacing.
3. The target flow naturally decomposes: synchronize → solve → extract. Different backends can implement different subsets (e.g., a lint backend may not implement `CallbackSession`).
4. The worktree's `ReferenceBackend` already decomposes this way — it has `apply_batch()`, `rebuild()`, and `normalized_view()` as separate methods on a concrete struct.
5. Rust trait composition (e.g., `Iterator` + `ExactSizeIterator`) is the idiomatic pattern for optional capabilities.

**How:**
- `BackendSession` is the primary trait — every backend MUST implement it
- `SessionHealth`, `SolutionView`, `CallbackSession`, `BackendMetadata` are supplementary traits
- Blanket impls or default methods aren't forced — each backend declares its capabilities explicitly
- The `Synchronization` enum has two variants: `DeltaBatch(DeltaBatch)` or `Rebuild(ModelSnapshot)`

---

### D2: Compatibility shim — remove legacy APIs entirely

**Decision:** Remove `SolverAdapter`, `SolverModelExt`, `drain_changes()`, `Model.solver_options`, and `Change` from the public API surface. Do NOT provide a deprecated compatibility shim.

**Why:**
1. This is pre-v0.1 — no stability promise exists. Breaking changes are expected.
2. The legacy API is actively harmful: `drain_changes()` is destructive and `SolverModelExt::solve_model()` silently ignores unsupported options.
3. Every backend will be rewritten against the new contract:
   - HiGHS: M1R-02 rewrites `roml-highs` to implement `BackendSession` directly
   - MOSEK: M1R-06 migrates to the new contract
   - Xpress: M1R-07 migrates to the new contract
4. The 9 pinned tests that use the legacy API are being deleted (D3) — no test code depends on the shim.
5. A shim adds maintenance burden for a bridge nobody needs. Per D-003, a shim may exist "only if it delegates to the safe protocol, cannot lose replayability, rejects unsupported policy explicitly, and is loudly deprecated." Since there's no consumer that benefits from it, the simplest correct decision is removal.
6. M1R-C7 explicitly allows removal: "Legacy SolverAdapter/SolverModelExt is removed or retained only as a safe, loudly deprecated shim."

**What gets removed:**
- `pub trait SolverAdapter` — all 13 methods
- `pub trait SolverModelExt` — blanket impl on SolverAdapter
- `Model::drain_changes()` — replaced by Journal-based sync
- `Model::has_pending_changes()` — replaced by revision comparison
- `Model::changelog_sequence()` — replaced by ModelRevision
- `Model.solver_options` field — replaced by SolveRequest
- `Model::set_solver_options()` — replaced by SolveRequest builder
- `pub struct SolveOptions` — replaced by SolveRequest
- `pub enum LpAlgorithm` — moved into SolveRequest or EffectiveConfig
- `pub enum SolverStatus` — replaced by TerminationStatus
- `pub enum Change` — replaced by ModelOp (different enum, not a rename)
- `pub struct ChangeLog` — replaced by Journal

**What stays:**
- `SolverError` — may be extended or replaced by `BackendError` (worktree already has a richer version)
- `pub mod callback` — `CallbackHandler`, `CallbackData`, `CallbackCut`, `CallbackAction` migrate to new session
- `Solution`, `SolutionBuilder`, `SolutionStore` — remain as value types; `SolutionView` complements them for borrowed access

**What gets added (from worktree):**
- All types from `delta.rs`, `revision.rs`, `sync.rs`, `journal.rs`, `snapshot.rs`
- All types from `solver/reference.rs`, `solver/request.rs`, `solver/backend.rs`
- New `BackendSession` trait + supplementary traits
- New `SolutionView` borrowed-access type

---

### D3: Pinned test un-ignore timing — delete and replace

**Decision:** Delete all 9 pinned tests. Task 01.2 writes new contract tests that cover the same invariants using the new `BackendSession`/`DeltaBatch`/`SolveRequest` API. No attempt is made to retrofit old tests to the new API.

**Why:**
1. **The old tests literally won't compile** after this phase removes `SolverAdapter`, `SolverModelExt`, `drain_changes()`, and `Model.solver_options`. They import types being deleted.
2. **The old tests test the wrong API.** All 9 tests exercise the destructive changelog protocol — they prove WHY the legacy approach is broken. The new contract tests prove that the revisioned protocol DOES NOT have those defects.
3. **The test scenarios are preserved in requirements.** M1R-C1–C6 encode the same invariants. Task 01.2 explicitly mandates tests for: backend failure cannot consume unacknowledged deltas, two sessions independently catch up, failed request validation consumes neither request nor model history, requested option is applied/adjusted/rejected, rebuild resets cursor and health deterministically, status preserves incumbent/proof/ambiguous states.
4. **Incremental approach (Option A) is infeasible** — each task would need to keep the old API alive for its tests, then tear it down later. This creates circular dependencies.
5. **Final batch approach (Option B) wastes time** — keeping tests ignored until the end means no regression protection during implementation.

**Inventory of tests being deleted:**

| Test file | Test name | What it proves | Replaced by (Task 01.2) |
|---|---|---|---|
| sync_characterization.rs | `drained_changes_are_lost_on_adapter_error` | destructive drain loses changes | Contract test: error preserves journal entry |
| sync_characterization.rs | `error_during_apply_loses_changes_from_model` | partial apply = unrecoverable | Contract test: DeltaBatch is atomic or fully recoverable |
| sync_characterization.rs | `two_adapters_cannot_both_sync_same_changes` | single changelog serves one adapter | Contract test: two sessions independently catch up |
| sync_characterization.rs | `sync_model_leaves_nothing_for_second_adapter` | same as above, via sync_model | Same replacement |
| sync_characterization.rs | `no_recovery_path_after_partial_apply` | no deterministic recovery | Contract test: ApplyOutcome distinguishes recoverable/terminal |
| sync_characterization.rs | `reset_has_no_revision_check` | no staleness detection | Contract test: revision mismatch is detected and reported |
| sync_characterization.rs | `no_staleness_detection_after_mutation` | no way to know if stale | Contract test: AdapterCursor exposes stale/current state |
| model_characterization.rs | `set_semicontinuous_low_lower_emits_change_without_bounds_update` | Change event emitted without bounds update | Contract test: ModelOp for semicontinuous is self-consistent |
| model_characterization.rs | `solve_options_stored_on_model_and_consumed_during_solve` | options live on Model, consumed destructively | Contract test: SolveRequest is immutable and reusable |

**The 2 duplicate_coefficient tests** (`duplicate_coefficient_for_same_cell`, `duplicate_coefficient_in_objective`) are P1 issues (last-write-wins) but are NOT pinned for M1R-01. They were fixed in Phase 9 (P1-1, P1-2 in the admission report). They remain as passing tests, unaffected by this phase.

---

### D4: Status/error/solution architecture — hybrid SolveResult with typed sub-structures

**Decision:** Use a unified `SolveResult` as the return type from `BackendSession::solve()`, containing typed sub-structures: `SolveRequest` (requested), `EffectiveConfig` (negotiated), `TerminationStatus` (outcome), and `SolveSolution` (optional values). This is the hybrid pattern — one return type, internally decomposed.

**The worktree already defines this structure.** M1R-01 adopts and enriches it:

**SolveResult (adopted from worktree `request.rs:83-94`):**
```rust
pub struct SolveResult {
    /// The request as submitted (immutable, reusable reference).
    pub requested: SolveRequest,
    /// The configuration actually applied by the backend.
    pub effective: EffectiveConfig,
    /// Why the solve terminated.
    pub termination: TerminationStatus,
    /// Solution data, if any was produced.
    pub solution: Option<SolveSolution>,
}
```

**TerminationStatus (adopted from worktree `backend.rs:198-222`, replacing SolverStatus):**
```rust
pub enum TerminationStatus {
    Optimal,           // proven optimal
    Infeasible,        // proven infeasible
    Unbounded,         // proven unbounded
    Feasible,          // incumbent found but not proven optimal
    InfeasibleOrUnbounded, // ambiguity (HiGHS can return this)
    TimeLimit,         // hit time limit
    IterationLimit,    // hit iteration limit
    NodeLimit,         // hit node limit
    SolutionLimit,     // hit solution limit
    Interrupted,       // user cancelled
    NumericalIssue,    // numerical difficulties
    Error,             // backend error
}
```

**Key improvements over current SolverStatus:**
1. `Feasible` captures "incumbent found, not proven" (M1R-C6: preserve incumbent)
2. `InfeasibleOrUnbounded` captures ambiguity (M1R-C6: preserve ambiguity)
3. `NodeLimit` and `SolutionLimit` are distinct from IterationLimit (M1R-C6: preserve limits)
4. `Interrupted` is distinct from `TimeLimit` (M1R-C6: preserve interruption)
5. `NumericalIssue` is distinct from generic `Error` (M1R-C6: preserve recoverability)

**EffectiveConfig (adopted from worktree `request.rs:97-113`):**
Tracks `adjustments: Vec<ConfigAdjustment>` and `rejections: Vec<ConfigRejection>` — each option is applied, adjusted with reason, or explicitly rejected (M1R-C3).

**SolveSolution (adopted from worktree `request.rs:138-148`):**
Contains `variable_values: Vec<(VarId, f64)>`, `objective_value: Option<f64>`, `dual_values`, `reduced_costs`. 

**New: SolutionView** — a borrowed/indexed accessor that wraps `SolveSolution` and provides `value(VarId) -> Option<f64>`, `dual(ConId) -> Option<f64>`, `reduced_cost(VarId) -> Option<f64>` without cloning HashMaps. Lifetime-bound to the session.

**BackendError (adopted from worktree `backend.rs`):**
Enriched with `ErrorCategory` (NotSupported, InvalidInput, Internal, ResourceExhausted, License, Native, Io, Timeout, State) and recoverability flag.

**Why:**
1. The target flow explicitly shows `SolveResult { requested, effective, termination, solution_view }` — this is the designed shape.
2. The worktree already prototypes this exact structure — no redesign needed, just adoption and enrichment.
3. The hybrid pattern keeps the API surface simple (one return type) while keeping internal concerns properly separated.
4. Option A (single unified type) would be a flat struct with 30 fields — unreadable and hard to evolve.
5. Option B (separate type hierarchy) would force consumers to pattern-match across multiple return types.
6. The `SolutionView` borrowed-access pattern replaces the current `HashMap<VarId, f64>` cloning, which is expensive for large models.

---

### D5: Contract freeze process — lightweight ADR

**Decision:** After M1R-01 implementation is complete and all gates pass, produce a lightweight Architecture Decision Record at `.planning/adr/ADR-001-backend-contract-freeze.md`. The ADR records:

1. **The frozen trait signatures** — exact `BackendSession`, `SessionHealth`, `SolutionView`, `CallbackSession`, `BackendMetadata` trait definitions at the freeze commit SHA.
2. **The frozen type signatures** — `SolveRequest`, `SolveResult`, `TerminationStatus`, `EffectiveConfig`, `SolveSolution`, `BackendError`, `Synchronization`, `SyncReceipt`, `DeltaBatch`, `ModelOp`, `ModelRevision`, `AdapterCursor`, `AdapterHealth`, `ApplyOutcome`, `Journal`, `ModelSnapshot`.
3. **The freeze commit SHA** — exact git commit where the contract implementation lands.
4. **The change process** — any post-freeze edit to a frozen trait/type signature requires:
   - A recorded decision (update to this ADR or a new ADR).
   - Notification to all downstream workers (HiGHS, MOSEK, Xpress).
   - Coordinated rebase of all downstream branches.
5. **A cross-reference** in `STATE.md` Phase M1R-01 row pointing to the ADR and freeze SHA.

This is NOT a heavyweight process. The ADR is a single markdown file recording a specific commit as the contract boundary.

**Why:**
1. The gate explicitly says: "Contract edits after freeze require a recorded decision and coordinated rebase."
2. M1R-02 (HiGHS rewrite), M1R-06 (MOSEK), and M1R-07 (Xpress) all depend on a stable contract. If the contract drifts after they start implementing, it creates cascading rework.
3. Option B (STATE.md checkpoint only) is too lightweight — STATE.md tracks phase state, not interface contracts. An ADR gives the contract its own canonical location.
4. Option C (no formal freeze) would create drift risk — the shared contract files have one integration owner but multiple backend workers. Without a freeze, a worker starts implementing against a moving target.
5. The ADR format is already established by `DECISIONS.md` (D-001 through D-012). The contract freeze ADR follows the same pattern.
6. This is consistent with D-009 (lane separation): "Shared contract files have one integration owner."

**How:**
- Create `.planning/adr/` directory if it doesn't exist.
- Write `ADR-001-backend-contract-freeze.md` after M1R-01 gates pass.
- Add Phase M1R-01 contract freeze SHA to STATE.md.
- Downstream phases (M1R-02, M1R-06, M1R-07) reference this ADR as their contract baseline.

---

## Deferred ideas

1. **Factory pattern (Decision 1, Option C):** A `BackendSessionFactory` that produces sessions could be useful for backends that require per-session native resource initialization (e.g., MOSEK environments). This is deferred to M1R-06 (MOSEK) since HiGHS doesn't need it. The `BackendSession` trait should be designed to NOT preclude a factory pattern later — a separate `BackendSessionFactory` trait can be added without modifying `BackendSession`.

2. **Async solve:** The current `solve()` is synchronous. For long-running solves, an async or streaming API may be needed. This is deferred to M3 (persistent incremental runtime). The `BackendSession::solve()` signature should return `Result<SolveResult, BackendError>` synchronously for now, with the understanding that M3 may add `solve_async() -> impl Future` or a progress-streaming variant.

3. **Change type migration:** `Change` (16 variants, carries old+new values) vs `ModelOp` (16 variants, self-contained). The worktree uses `ModelOp` exclusively. `Change` is removed from the public API in this phase. The internal changelog mechanism may still use `Change` as an implementation detail if `ModelOp` construction is more expensive — but this is an internal optimization, not a contract decision. The external contract only sees `ModelOp` through `DeltaBatch`.

4. **SolverStatus deprecation path:** `SolverStatus` is replaced by `TerminationStatus` in the public API. If any internal code or ROML-HIGHS code references `SolverStatus`, those references should be migrated during M1R-02 (HiGHS rewrite), not during M1R-01. M1R-01 only concerns the `roml` core crate contract.

5. **semver-checks baseline:** After the contract is frozen, `cargo semver-checks check-release` should establish a baseline. The first release after M1R-01 (v0.1) will have no prior version to compare against, but the check should still be configured and passing (it will report "no preceding release" which is acceptable for v0.1).
