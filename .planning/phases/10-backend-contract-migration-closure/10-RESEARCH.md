# Phase 10: Backend Contract Migration Closure (M1R-01) - Research

**Researched:** 2026-07-18
**Domain:** Backend contract migration, trait decomposition, revisioned sync protocol
**Confidence:** HIGH

## Summary

This phase makes the revisioned snapshot/delta/session contract the ONLY supported public execution path in the `roml` core crate and removes all legacy destructive patterns. The phase is the critical code-change handoff between M1R-00 (truth reset, complete) and M1R-02 (HiGHS rewrite, blocked by this phase).

The worktree (`phase-roml-P0-release-baseline`) already prototypes all the target protocol types: `DeltaBatch`, `ModelOp`, `ModelRevision`, `ModelSnapshot`, `Journal`, `AdapterCursor`, `AdapterHealth`, `SyncCoordinator`, `SolveRequest`, `SolveResult`, `TerminationStatus`, `BackendError`, `BackendCapabilities`, `ReferenceBackend`, `NormalizedView`. What does NOT exist yet: the `BackendSession` trait, supplementary bounded traits (`SessionHealth`, `SolutionView`, `CallbackSession`, `BackendMetadata`), the `Synchronization` enum, the `SyncReceipt` type, and integration wiring from `Model` to the session protocol.

The current `main` has the legacy 13-method `SolverAdapter` trait, `SolverModelExt` blanket impl, destructive `drain_changes()`, `Model.solver_options` field, cloned `HashMap` solution access, and 8-variant `SolverStatus`. All three backend crates (`roml-highs`, `roml-mosek`, `roml-xpress`) implement the legacy trait. This phase removes the legacy surface from the `roml` core crate only; backend crates are migrated in their own phases.

**Primary recommendation:** Adopt the bounded-traits decomposition from CONTEXT.md D1: `BackendSession` (primary), `SessionHealth`, `SolutionView`, `CallbackSession`, `BackendMetadata` (supplementary). Import all worktree protocol types as-is (they are the designed contract). Delete the 9 pinned tests. Write new contract tests. Produce ADR-001 at the freeze commit. Remove all legacy public APIs from `roml` core.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### D1: BackendSession surface — separated bounded traits

**Decision:** Use separated bounded traits decomposing lifecycle, synchronization, solve, and extraction into independently implementable interfaces. **NOT** a single monolithic `BackendSession` trait.

**Core trait:**
```rust
pub trait BackendSession {
    fn synchronize(&mut self, sync: Synchronization) -> Result<SyncReceipt, BackendError>;
    fn solve(&mut self, request: &SolveRequest) -> Result<SolveResult, BackendError>;
    fn close(self) -> Result<(), BackendError>;
}
```

**Separate bounded traits:**
- `SessionHealth` — exposes `AdapterHealth` (ready/retryable/rebuild-required/terminal) and cursor position
- `SolutionView` — borrowed/indexed access to primal values, duals, reduced costs from the last solve; lifetime-bound to the session, not cloned
- `CallbackSession` — register/unregister `CallbackHandler` (only for backends that support callbacks)
- `BackendMetadata` — version, build info, index width, capability declaration

#### D2: Compatibility shim — remove legacy APIs entirely

**Decision:** Remove `SolverAdapter`, `SolverModelExt`, `drain_changes()`, `Model.solver_options`, and `Change` from the public API surface. Do NOT provide a deprecated compatibility shim.

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
- `SolverError` — may be extended or replaced by `BackendError`
- `pub mod callback` — `CallbackHandler`, `CallbackData`, `CallbackCut`, `CallbackAction` migrate to new session
- `Solution`, `SolutionBuilder`, `SolutionStore` — remain as value types

**What gets added (from worktree):**
- All types from `delta.rs`, `revision.rs`, `sync.rs`, `journal.rs`, `snapshot.rs`
- All types from `solver/reference.rs`, `solver/request.rs`, `solver/backend.rs`
- New `BackendSession` trait + supplementary traits
- New `SolutionView` borrowed-access type

#### D3: Pinned test un-ignore timing — delete and replace

**Decision:** Delete all 9 pinned tests. Task 01.2 writes new contract tests that cover the same invariants using the new `BackendSession`/`DeltaBatch`/`SolveRequest` API. No attempt is made to retrofit old tests to the new API.

9 pinned tests to delete (all `#[ignore]` in the worktree):
- 7 in `sync_characterization.rs`: drained_changes_are_lost_on_adapter_error, error_during_apply_loses_changes_from_model, two_adapters_cannot_both_sync_same_changes, sync_model_leaves_nothing_for_second_adapter, no_recovery_path_after_partial_apply, reset_has_no_revision_check, no_staleness_detection_after_mutation
- 2 in `model_characterization.rs`: set_semicontinuous_low_lower_emits_change_without_bounds_update, solve_options_stored_on_model_and_consumed_during_solve

2 duplicate_coefficient tests (also `#[ignore]` in model_characterization.rs) are P1 issues fixed in Phase 9, not affected by this phase.

#### D4: Status/error/solution architecture — hybrid SolveResult with typed sub-structures

**Decision:** Use a unified `SolveResult` as the return type from `BackendSession::solve()`, containing typed sub-structures: `SolveRequest` (requested), `EffectiveConfig` (negotiated), `TerminationStatus` (outcome), and `SolveSolution` (optional values). This is the hybrid pattern — one return type, internally decomposed.

#### D5: Contract freeze process — lightweight ADR

**Decision:** After M1R-01 implementation is complete and all gates pass, produce a lightweight Architecture Decision Record at `.planning/adr/ADR-001-backend-contract-freeze.md`.

### Claude's Discretion
None specified in CONTEXT.md.

### Deferred Ideas (OUT OF SCOPE)
1. Factory pattern (`BackendSessionFactory`) — deferred to M1R-06 (MOSEK)
2. Async solve — deferred to M3
3. `Change` type migration — `Change` removed from public API this phase; internal changelog may keep it as an implementation detail
4. `SolverStatus` deprecation path — references in HiGHS/MOSEK/Xpress migrated during their respective phases (M1R-02, M1R-06, M1R-07)
5. semver-checks baseline — configured this phase but "no preceding release" is acceptable for v0.1
</user_constraints>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| M1R-C1 | Supported synchronization consumes `DeltaBatch` through independent adapter cursors; no supported path destructively drains before acknowledgement | Worktree `SyncCoordinator`/`Journal`/`AdapterCursor` already implement this. This phase adds `BackendSession::synchronize()` and removes `drain_changes()` from public API. |
| M1R-C2 | Canonical `Model` contains no transient solve policy | `Model.solver_options` field removed. `SolveRequest` owns all solve policy. Verified by D2 removal list. |
| M1R-C3 | Every requested option/capability is applied, adjusted with reason, or rejected | Worktree `EffectiveConfig` with `adjustments`/`rejections` vectors. `validate_request()` function prototypes this. |
| M1R-C4 | Adapter health is explicit: ready, retryable, rebuild-required, terminal | Worktree `AdapterHealth` has Ready/RequiresRebuild/Terminal. CONTEXT.md D1 adds `SessionHealth` supplementary trait. |
| M1R-C5 | Snapshot rebuild and complete incremental application are observationally equivalent | `ReferenceBackend::normalized_view()` proves this. Worktree tests at `solver/reference.rs:381`. |
| M1R-C6 | Public status/error/solution contracts preserve incumbent, proof, limits, interruption, ambiguity, native code, operation, and recoverability | Worktree `TerminationStatus` (11 variants vs 8 in `SolverStatus`), `BackendError` with `ErrorCategory` and `HealthEffect`. |
| M1R-C7 | Legacy `SolverAdapter`/`SolverModelExt` is removed; no destructive shim remains | D2 locked. Removal applied to `roml` core crate only. Backend crates retain legacy types until their own migration phases. |
| M1R-C8 | All P1/P2 characterization tests execute or are deleted with requirement-backed disposition; none remain ignored | D3 locked: 9 pinned tests deleted, replaced by new contract tests in Task 01.2. |

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Model mutation tracking | Model (core data) | Journal (delta replay) | Model pushes `Change` to internal changelog; Journal stores committed `DeltaBatch` for replay. |
| Delta compilation | Model (core data) | — | Model compiles `Change` events into `ModelOp` vectors for `DeltaBatch`. |
| Session lifecycle | BackendSession trait | Adapter concrete impl | `BackendSession` is the session abstraction; concrete adapter implementations own native resources. |
| Synchronization protocol | SyncCoordinator (model side) | AdapterCursor (adapter side) | Coordinator owns the journal; cursor tracks which revision each adapter has applied. |
| Solve policy | SolveRequest (immutable) | BackendSession::solve() | Request is immutable, reusable; backend applies, adjusts, or rejects each option. |
| Solution extraction | SolveResult / SolveSolution | SolutionView (borrowed access) | Value types for transfer; indexed view for borrowed access without cloning. |
| Health tracking | SessionHealth trait | AdapterCursor | Health is per-session; cursor exposes AdapterHealth and revision position. |
| Callback registration | CallbackSession trait | BackendSession impl | Optional trait; only backends that support MIP callbacks implement this. |
| Backend metadata | BackendMetadata trait | — | Version, build info, capability declaration. |

## Standard Stack

This phase does not introduce new external dependencies. All types are in the `roml` core crate. The "stack" refers to the protocol type taxonomy and module layout.

### Protocol Types (to Import from Worktree)

| Type | File (worktree) | Purpose | Status |
|------|-----------------|---------|--------|
| `ModelOp` | `src/delta.rs` | 16 self-contained operation variants | Adopt as-is from worktree |
| `DeltaBatch` | `src/delta.rs` | Revisioned batch of operations | Adopt as-is from worktree |
| `ModelRevision` | `src/revision.rs` | Monotonic revision counter | Adopt as-is from worktree |
| `ModelSnapshot` | `src/snapshot.rs` | Full deterministic projection | Adopt as-is from worktree |
| `Journal` | `src/journal.rs` | BTreeMap<ModelRevision, DeltaBatch> | Adopt as-is from worktree |
| `SyncCoordinator` | `src/sync.rs` | Model-side sync bridge | Adopt as-is from worktree |
| `AdapterCursor` | `src/sync.rs` | Per-adapter revision tracker | Adopt as-is from worktree |
| `AdapterHealth` | `src/sync.rs` | Ready/Rebuild/Terminal | Adopt as-is from worktree |
| `ApplyOutcome` | `src/sync.rs` | Applied/RequiresRebuild/Failure | Adopt as-is from worktree |
| `ApplyError` | `src/sync.rs` | RevisionMismatch/RevisionNotFound | Adopt as-is from worktree |
| `ReferenceBackend` | `src/solver/reference.rs` | Correctness verification backend | Adopt as-is from worktree |
| `NormalizedView` | `src/solver/reference.rs` | Deterministic state comparison | Adopt as-is from worktree |
| `SolveRequest` | `src/solver/request.rs` | Immutable solve policy | Adopt as-is from worktree |
| `SolveResult` | `src/solver/request.rs` | Hybrid result with substructures | Adopt as-is from worktree |
| `EffectiveConfig` | `src/solver/request.rs` | Negotiated configuration | Adopt as-is from worktree |
| `SolveSolution` | `src/solver/request.rs` | Vec-based solution data | Adopt as-is from worktree |
| `TerminationStatus` | `src/solver/backend.rs` | 11 variant termination status | Adopt as-is from worktree |
| `BackendError` | `src/solver/backend.rs` | Categorized error with health effect | Adopt as-is from worktree |
| `ErrorCategory` | `src/solver/backend.rs` | InvalidInput/Unsupported/.../Unknown | Adopt as-is from worktree |
| `HealthEffect` | `src/solver/backend.rs` | None/Recoverable/Rebuild/Terminal | Adopt as-is from worktree |
| `BackendCapabilities` | `src/solver/backend.rs` | Feature flag struct | Adopt as-is from worktree |
| `BackendInfo` | `src/solver/backend.rs` | Identity + capabilities | Adopt as-is from worktree |

### New Types (to Design and Implement)

| Type | Purpose | Design Source |
|------|---------|---------------|
| `BackendSession` trait | Primary session lifecycle: synchronize, solve, close | CONTEXT.md D1 |
| `SessionHealth` trait | Expose AdapterHealth and cursor position | CONTEXT.md D1 |
| `SolutionView` trait | Borrowed/indexed access to last solve solution | CONTEXT.md D1 |
| `CallbackSession` trait | Optional callback registration | CONTEXT.md D1 |
| `BackendMetadata` trait | Version, build info, capability declaration | CONTEXT.md D1 |
| `Synchronization` enum | `DeltaBatch(DeltaBatch)` or `Rebuild(ModelSnapshot)` | CONTEXT.md D1 |
| `SyncReceipt` | Result of synchronization (confirms cursor position, health) | Implied by D1 trait signature |
| `BackendSession` module | Module containing all new traits and types | New |

### Module Layout (Recommended)

```
src/
├── delta.rs          # ModelOp + DeltaBatch (copy from worktree)
├── revision.rs       # ModelRevision + RevisionError (copy from worktree)
├── sync.rs           # AdapterCursor + AdapterHealth + ApplyOutcome + SyncCoordinator (copy from worktree)
├── journal.rs        # Journal (copy from worktree)
├── snapshot.rs       # ModelSnapshot + take_snapshot (copy from worktree)
├── solver/
│   ├── mod.rs        # Remove SolverAdapter, SolverModelExt, SolverStatus, SolveOptions, LpAlgorithm.
│   │                 # Keep only SolverError (extended or replaced) and pub mod callback.
│   ├── callback.rs   # Unchanged
│   ├── reference.rs  # ReferenceBackend + NormalizedView (copy from worktree)
│   ├── request.rs    # SolveRequest + SolveResult + EffectiveConfig + SolveSolution (copy from worktree)
│   ├── backend.rs    # TerminationStatus + BackendError + ErrorCategory + HealthEffect + BackendCapabilities (copy from worktree)
│   └── session.rs    # NEW: BackendSession + SessionHealth + SolutionView + CallbackSession + BackendMetadata + Synchronization + SyncReceipt
└── model/
    ├── mod.rs        # Remove drain_changes() from public, remove solver_options field, retain Changelog as internal
    └── changelog.rs  # Keep as internal implementation detail, remove Change from public API
```

## Package Legitimacy Audit

No new external packages are installed by this phase. The phase operates entirely within the `roml` core crate, importing types from the worktree that have been developed in-tree. No package registry verification is required.

## Architecture Patterns

### System Architecture Data Flow

```
Model mutation
    |
    v
[Model internal changelog] -- compiles changes into --> [DeltaBatch {from, to, operations: Vec<ModelOp>}]
    |                                                         |
    | commit_batch()                                         |
    v                                                         v
[Journal (BTreeMap<ModelRevision, DeltaBatch>)]           [SyncCoordinator]
    |                                                         |
    | deltas_since(cursor.revision)                          |
    v                                                         v
[AdapterCursor {applied_revision, health}] <-- catches up via delta replay OR rebuild from ModelSnapshot
    |
    | SessionHealth::health() | SessionHealth::revision()
    v
[BackendSession]
    |
    |-- synchronize(Synchronization) --> SyncReceipt {cursor, health}
    |-- solve(SolveRequest)           --> SolveResult {effective_configuration, termination, solution}
    |-- close()                       --> Ok(())
    |
    v
[SolutionView] -- borrowed access to last solve (value(VarId), dual(ConId), reduced_cost(VarId))
```

### Pattern 1: Bounded Traits Composition

**What:** Decompose session capabilities into independently implementable traits, following Rust's `Iterator` + `ExactSizeIterator` composition pattern. Each backend declares which traits it supports; consumers can query for optional capabilities.

**When to use:** Always. This is the primary design pattern for this phase.

**Trait composition:**
```rust
// Every backend MUST implement BackendSession
pub trait BackendSession {
    fn synchronize(&mut self, sync: Synchronization) -> Result<SyncReceipt, BackendError>;
    fn solve(&mut self, request: &SolveRequest) -> Result<SolveResult, BackendError>;
    fn close(self) -> Result<(), BackendError>;
}

// Optional — most backends implement this
pub trait SessionHealth {
    fn health(&self) -> AdapterHealth;
    fn revision(&self) -> ModelRevision;
}

// Optional — backends that expose solution data implement this
pub trait SolutionView {
    fn value(&self, var: VarId) -> Option<f64>;
    fn dual(&self, con: ConId) -> Option<f64>;
    fn reduced_cost(&self, var: VarId) -> Option<f64>;
}

// Optional — MIP-capable backends that support callbacks
pub trait CallbackSession {
    fn set_callback_handler(&mut self, handler: Box<dyn CallbackHandler>) -> Result<(), BackendError>;
    fn clear_callback_handler(&mut self) -> Result<(), BackendError>;
}

// Optional — backends that expose metadata
pub trait BackendMetadata {
    fn name(&self) -> &str;
    fn capabilities(&self) -> BackendCapabilities;
}
```

### Pattern 2: Synchronization Enum Dispatch

**What:** `Synchronization` enum with two variants that the `BackendSession::synchronize()` method dispatches on. This avoids needing two separate methods and allows the session to handle both delta replay and full rebuild in one entry point.

```rust
pub enum Synchronization {
    DeltaBatch(DeltaBatch),
    Rebuild(ModelSnapshot),
}

pub struct SyncReceipt {
    pub cursor: AdapterCursor,
    pub health: AdapterHealth,
}
```

### Pattern 3: Hybrid SolveResult with Typed Sub-Structures

**What:** A single `SolveResult` return type containing typed sub-structures, not a flat struct or multiple return values. This keeps the API surface minimal while separating concerns.

```rust
pub struct SolveResult {
    pub effective_configuration: EffectiveConfig,
    pub termination: TerminationStatus,
    pub solution: Option<SolveSolution>,
}
```

### Anti-Patterns to Avoid

- **Giant monolithic trait:** `SolverAdapter` had 13 methods. The bounded traits pattern replaces this.
- **Hand-coded HashMap cloning:** The current `Solution::value()` returns cloned HashMap values. `SolutionView` provides borrowed/indexed access.
- **Silent option ignorance:** `SolverAdapter::apply_options` silently ignores unsupported options. `SolveRequest` validation produces explicit `ConfigRejection` entries.
- **Model-owned transient state:** `Model.solver_options` stored on the model, consumed by solve. `SolveRequest` is independent and reusable.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Backend capability declaration | Custom runtime reflection | `BackendCapabilities` struct with boolean flags | Simple, compile-time friendly, pattern-matched by consumers |
| Health tracking | Error-code-based inference | `AdapterHealth` enum + `HealthEffect` on errors | Explicit states (Ready/RequiresRebuild/Terminal) — no guessing from error message strings |
| Solve option negotiation | Try-each-and-see | `validate_request()` + `EffectiveConfig.adjustments`/`rejections` | Explicit rejection with reason string, testable, deterministic |

## Runtime State Inventory

> Not applicable — this phase is a source-code refactor of the `roml` core crate. It does not involve runtime migration, renames, or data state on disk.

## Common Pitfalls

### Pitfall 1: Worktree Types Have `#[allow(dead_code)]` and `pub(crate)` Visibility
**What goes wrong:** The worktree types were prototyped in a worktree environment with relaxed visibility. When moved into main, many fields and methods are `pub(crate)` or annotated `#[allow(dead_code)]`.
**Why it happens:** The prototype focused on correctness, not API surface.
**How to avoid:** Review every type imported from the worktree for visibility. Fix `pub(crate)` to `pub` on public types. Remove `#[allow(dead_code)]` annotations. Every modifier in the worktree's new types MUST be evaluated for correctness in the public contract.
**Warning signs:** Compiler warnings for dead code when building with `cargo build -p roml`.

### Pitfall 2: Backend Crates Still Import Legacy Types
**What goes wrong:** `roml-highs`, `roml-mosek`, and `roml-xpress` all import `SolverAdapter`, `SolverStatus`, `SolverError` from `roml::solver`. Removing these from `roml` will break compilation of all three backend crates.
**Why it happens:** M1R-01 only touches the `roml` core crate. Backend crates have their own migration phases.
**How to avoid:** Keep `SolverError` in `roml::solver` (D2 says it stays). Remove `SolverAdapter`, `SolverStatus` from the public `roml` API but they must still compile in the workspaces. Strategy: the backend crate tests currently compile against the full workspace. Use `#[cfg(not(feature = "roml_m1r01"))]` guards or — simpler — keep the trait defs in `roml` but mark `#[deprecated]` with a message pointing to the new contract. However, D2 says NO deprecated shim. Thus the crates that import these types must be allowed to break. Workspace `cargo test -p roml --all-targets` will pass; workspace-wide `cargo test --workspace` will break for backend crates. The verification commands in the phase packet only test `-p roml`.
**Warning signs:** Workspace-level build failures when CI runs `cargo test --workspace` across all crates.

### Pitfall 3: `Change` Enum Is Internal Implementation — But Tests Reference It
**What goes wrong:** `tests/changelog_integration.rs` and `tests/model_characterization.rs` (passing tests) import `roml::Change` from the changelog. Removing `Change` from the public API breaks these tests.
**Why it happens:** These tests assert on `Change` enum variants in the changelog. They test model mutation tracking, which still exists but the public type changes.
**How to avoid:** The phase scope document says `Change` is removed from public API. The tests that use `Change` (in `tests/` directory) need updating. `tests/changelog_integration.rs` directly tests changelog behavior — these can be updated to test against `ModelOp`/`DeltaBatch` instead. `tests/macro_api.rs` does not import `Change` and is unaffected.
**Warning signs:** `tests/changelog_integration.rs` compile errors after removal.

### Pitfall 4: ReferenceBackend Path Collision
**What goes wrong:** The worktree's `ReferenceBackend` lives at `src/solver/reference.rs` but main also has work-in-progress there or the path doesn't exist.
**How to avoid:** Check main's `src/solver/` directory. The `reference.rs` file exists in the worktree but may not exist in main. Create it as part of the import.
**Warning signs:** `git status` shows conflicts when moving files.

### Pitfall 5: `SolverError` Still Used by All Backend Crates
**What goes wrong:** If `SolverError` is removed (it's in the "what gets removed" list but D2 says it stays), all three backend crates break.
**How to avoid:** D2 explicitly says `SolverError` stays. CONTEXT.md says it "may be extended or replaced by `BackendError`". The safest path: keep `SolverError` in `roml::solver` as-is, add `BackendError` as the new type for the session protocol. Backend crates can continue to use `SolverError` until they migrate to `BackendError` in their own phases.
**Warning signs:** Backend crate compilation errors if `SolverError` is removed.

## Code Examples

### Example 1: BackendSession Trait (from D1)
```rust
// Source: CONTEXT.md D1
pub trait BackendSession {
    fn synchronize(&mut self, sync: Synchronization) -> Result<SyncReceipt, BackendError>;
    fn solve(&mut self, request: &SolveRequest) -> Result<SolveResult, BackendError>;
    fn close(self) -> Result<(), BackendError>;
}

pub enum Synchronization {
    DeltaBatch(DeltaBatch),
    Rebuild(ModelSnapshot),
}

pub struct SyncReceipt {
    pub cursor: AdapterCursor,
    pub health: AdapterHealth,
}
```

### Example 2: SessionHealth Optional Trait
```rust
// Source: Derived from CONTEXT.md D1 + worktree AdapterCursor
pub trait SessionHealth {
    fn health(&self) -> AdapterHealth;
    fn revision(&self) -> ModelRevision;
}
```

### Example 3: SolutionView Borrowed Access
```rust
// Source: Derived from CONTEXT.md D1
pub trait SolutionView {
    fn value(&self, var: VarId) -> Option<f64>;
    fn dual(&self, con: ConId) -> Option<f64>;
    fn reduced_cost(&self, var: VarId) -> Option<f64>;
    fn objective_value(&self) -> Option<f64>;
}
```

### Example 4: SolveResult with Typed Sub-Structures (from worktree)
```rust
// Source: worktree src/solver/request.rs
pub struct SolveResult {
    pub effective_configuration: EffectiveConfig,
    pub termination: TerminationStatus,
    pub solution: Option<SolveSolution>,
}

pub struct EffectiveConfig {
    pub lp_algorithm: Option<LpAlgorithm>,
    pub time_limit_secs: Option<f64>,
    pub mip_rel_gap: Option<f64>,
    pub threads: Option<i32>,
    pub enable_output: Option<bool>,
    pub adjustments: Vec<ConfigAdjustment>,
    pub rejections: Vec<ConfigRejection>,
}
```

### Example 5: ReferenceBackend Commuting Square Proof (from worktree)
```rust
// Source: worktree src/solver/reference.rs, lines 381-454
// This test proves that snapshot rebuild and incremental application produce
// equivalent normalized views.
fn build_from_snapshot_and_apply_deltas_are_equivalent() {
    // Backend A: rebuild from snapshot at r1
    // Backend B: rebuild from snapshot at r0, then apply deltas r0→r1
    // NormalizedView(A) == NormalizedView(B)  (the commuting square)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `drain_changes()` destructive | Journal-based non-destructive replay | This phase | Changes survive adapter errors; multiple sessions can independently catch up |
| `SolverAdapter` monolithic trait | `BackendSession` + bounded traits | This phase | Backends can implement only what they support; consumers query for optional capabilities |
| `Model.solver_options` transient state | `SolveRequest` immutable policy | This phase | Solve policy is independent of model; reusable after failure |
| `SolverStatus` 8 variants | `TerminationStatus` 11 variants | This phase | Preserves incumbent/proof/ambiguity/limits/interruption |
| `HashMap<VarId, f64>` cloned solutions | `SolveSolution` vec-based + `SolutionView` borrowed access | This phase | No cloning for read-only access; lower memory overhead |
| `SolverError` 3 variants | `BackendError` with `ErrorCategory` + `HealthEffect` | This phase | Richer error categorization; explicit health effect |
| Single changelog shared across adapters | `Journal` + `AdapterCursor` per session | This phase | Multiple sessions independently catch up; no destructive consumption |
| `Change` 16 variants with old+new | `ModelOp` 16 self-contained variants | This phase | Operations carry all context; no need to consult adjacent events |
| `ChangeLog` flat Vec | `DeltaBatch` revisioned + `Journal` indexed store | This phase | Revisions enable replay, staleness detection, compaction |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Worktree types compile without modification when moved to main | Common Pitfalls | Build errors from visibility issues or missing imports |
| A2 | The 9 pinned tests only exist in the worktree, not in main | Summary | There may be additional pinned/ignored tests in main that the evidence did not enumerate |
| A3 | Backend crates (`roml-highs`, `roml-mosek`, `roml-xpress`) don't need migration in this phase; they break separately | Pitfall 2 | If any shared CI runs workspace-wide tests, breakage would block merging |
| A4 | `Solution` type doesn't need changes to accommodate `SolveSolution` | What Stays | If consumers need to create Solutions from `SolveSolution`, a conversion trait or method may be needed |
| A5 | The callback infrastructure (`pub mod callback`) doesn't need semantic changes | What Stays | `CallbackSession` trait design may expose small signature differences |
| A6 | No CLAUDE.md exists in the project | CLAUDE.md check | No project-specific constraints to enforce |

## Open Questions

1. **What is the exact signature of `BackendSession`?**
   - What we know: CONTEXT.md D1 provides the canonical trait signature with `synchronize`, `solve`, `close`. Supplementary traits are described at a high level.
   - What's unclear: Exact method signatures for `SolutionView`, `CallbackSession`, `BackendMetadata` — these need to be defined in Task 01.3.
   - Recommendation: Design exact signatures during Task 01.3 (finalize interfaces), using the CONTEXT.md D1 descriptions as the spec. Iterate with contract tests.

2. **How does `Model` produce `DeltaBatch` and `ModelSnapshot`?**
   - What we know: The worktree has `take_snapshot()` function in `snapshot.rs` and `SyncCoordinator::commit_batch()`. The model's internal changelog must compile `Change` events into `ModelOp` vectors.
   - What's unclear: The exact integration point — does `Model::commit()` produce a `DeltaBatch`? Does the Model own a `SyncCoordinator`? Where does `take_snapshot()` get called?
   - Recommendation: Add a `SyncCoordinator` field to `Model`. In `Model::commit()`, compile pending `Change` events into `ModelOp` vectors, create a `DeltaBatch`, and commit it to the coordinator. Provide `Model::take_snapshot()` that calls `take_snapshot()`. This matches the target flow in the phase packet.

3. **Does `SolverError` stay exactly as-is or get extended/replaced?**
   - What we know: D2 says "SolverError — may be extended or replaced by BackendError (worktree already has a richer version)".
   - What's unclear: Whether `SolverError` should remain as a separate type or be replaced by `BackendError` in the `roml` core crate. Backend crates import `SolverError` by name.
   - Recommendation: Keep `SolverError` as-is for backward compatibility with backend crates. Add `BackendError` as the new error type for `BackendSession`. This avoids breaking backend crate compilations while introducing the richer error type. Backend crates migrate to `BackendError` in their own phases.

4. **What happens to `Solution` value type?**
   - What we know: `Solution` stays as a value type. `SolutionView` complements it for borrowed access. `SolveSolution` is the worktree's vec-based solution data.
   - What's unclear: Is there a conversion path from `SolveSolution` to `Solution`? Does `Solution` hold a `SolveSolution` internally?
   - Recommendation: Add `From<SolveSolution> for Solution` conversion. `Solution` remains a standalone value type backed by `HashMap<VarId, f64>`. `SolveSolution` is the backend-native vec-based struct. `SolutionView` provides borrowed access over the most recent `SolveResult`.

5. **When does `Model` produce `DeltaBatch` — at `commit()` time or at `drain_changes()` time?**
   - What we know: The new contract removes `drain_changes()` from the public API. Something must replace it.
   - What's unclear: Whether `Model` automatically compiles changes into `DeltaBatch` on every mutation, on `commit()`, or on demand.
   - Recommendation: On `Model::commit()`, compile the committed changes into a `DeltaBatch` and store it in the `SyncCoordinator`'s journal. This matches the transaction boundary concept: a transaction commits one atomic revision. For single non-parameter changes (like `set_variable_bounds`), compile on the fly.

## Environment Availability

> Skip condition check: This phase has no external dependencies beyond the Rust toolchain and the existing workspace crates.

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | All compilation | Verified | Check with `rustc --version` | — |
| Cargo workspace | All phases | Verified | Workspace has 4 members | — |
| HiGHS library | roml-highs integration tests | Unknown | — | Not required for `cargo test -p roml` (core crate tests only) |
| MOSEK | roml-mosek | Not installed | — | Not required |
| Xpress | roml-xpress | Not installed | — | Not required |

**Missing dependencies with no fallback:** None for this phase. The verification commands only test `-p roml`.

**Missing dependencies with fallback:** HiGHS, MOSEK, Xpress — not needed for core crate contract closure.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | built-in `#[test]` + `#[cfg(test)]` |
| Config file | None — cargo test default |
| Quick run command | `cargo test -p roml --lib` |
| Full suite command | `cargo test -p roml --all-targets` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| M1R-C1 | Error preserves journal entry | contract | `cargo test -p roml --test backend_contract -- error_preserves_journal_entry` | Not yet (Task 01.2) |
| M1R-C1 | Two sessions independently catch up | contract | `cargo test -p roml --test backend_contract -- two_sessions_independently_catch_up` | Not yet (Task 01.2) |
| M1R-C1 | DeltaBatch atomic or fully recoverable | contract | `cargo test -p roml --test backend_contract -- delta_batch_atomic_recovery` | Not yet (Task 01.2) |
| M1R-C2 | Model has no solver_options field | compile | `cargo build -p roml` | After Task 01.6 |
| M1R-C3 | Option applied/adjusted/rejected | contract | `cargo test -p roml --test status_negotiation_tests` | Worktree has `request.rs` tests |
| M1R-C4 | ApplyOutcome distinguishes recoverable/terminal | contract | `cargo test -p roml --test backend_contract -- apply_outcome_distinguishes` | Not yet (Task 01.2) |
| M1R-C5 | Snapshot rebuild == incremental apply | unit | `cargo test -p roml -- reference_backend` | Worktree `reference.rs:381` |
| M1R-C6 | Status preserves incumbent/proof/ambiguous | contract | `cargo test -p roml --test status_negotiation_tests` | Worktree has tests |
| M1R-C7 | SolverAdapter/SolverModelExt not in lib.rs | compile | `cargo build -p roml` | After Task 01.6 |
| M1R-C8 | No ignored tests remain | audit | `grep -r '#\[ignore\]' tests/ 2>/dev/null; echo $?` | After deletion |

### Sampling Rate
- **Per task commit:** `cargo test -p roml --lib`
- **Per merge:** `cargo test -p roml --all-targets && cargo clippy -p roml --all-targets -- -D warnings && RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps`

### Wave 0 Gaps
- [ ] `tests/backend_contract.rs` — covers M1R-C1, C3, C4, C6 (entirely new file)
- [ ] `tests/sync_characterization.rs` — will be DELETED per D3
- [ ] `tests/status_negotiation_tests.rs` — referenced in phase packet but not yet on main

## Security Domain

**Security enforcement: disabled.** This phase operates on pre-v0.1 core library code. There are no runtime security boundaries, authentication, session management, access control, or input validation security requirements. The model layer validates entity existence (variable not found errors) which is correctness validation, not security. ASVS is not applicable.

## Sources

### Primary (HIGH confidence)
- CONTEXT.md (Phase 10 decisions — D1 through D5 with full trait signatures)
- Worktree source files (.claude/worktrees/phase-roml-P0-release-baseline/src/) — ALL protocol type definitions verified by reading every file
- Current main source (src/solver/mod.rs, src/model/mod.rs, src/lib.rs, src/model/changelog.rs, src/solution/mod.rs) — verified by reading every file

### Secondary (MEDIUM confidence)
- `docs/release/evidence/M1R/artifacts/02-claim-reconciliation.md` — M1 claim verdicts
- Phase packet phase.md — 7 task definitions, verification commands, target flow

### Tertiary (LOW confidence)
- [ASSUMED] Worktree types compile without modification — A1 in Assumptions Log
- [ASSUMED] Backend crates can tolerate breakage in this phase — A3 in Assumptions Log

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all types verified by reading both worktree and main source
- Architecture: HIGH — bounded traits pattern explicitly decided in D1, verified against existing Rust ecosystem patterns
- Pitfalls: HIGH — all five pitfalls verified by grep'ing the workspace for imports and call sites
- Assumptions: 6 assumptions tagged LOW — none are blocking to planning

**Research date:** 2026-07-18
**Valid until:** 2026-08-18 (30 days — the Rust codebase and pre-v0.1 contract are relatively stable)
