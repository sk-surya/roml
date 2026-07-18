# Phase 11 Context: HiGHS Projection/Session Rewrite

**Phase:** M1R-02 (Phase 11)
**Date:** 2026-07-18
**Mode:** Autonomous discuss-phase (headless)
**Milestone:** ROML-M1R — Truth Reset, Native HiGHS Qualification, and v0.1 Release
**Depends on:** M1R-01 (Phase 10 — Backend Contract Migration Closure) — COMPLETE

## Domain

`roml-highs` currently implements the **removed** `SolverAdapter` trait, producing E0432 compilation errors. The adapter has a handwritten FFI layer (252 lines of `extern "C"`, manually-copied callback struct layouts, and hardcoded constant values) and a monolithic 886-line `adapter.rs` that owns ABI, model projection, solve policy, callbacks, and extraction in one file.

Phase 11 converts `roml-highs` into the mandatory v0.1 implementation of the **frozen backend contract** (ADR-001) using authoritative `highs-sys` bindings. The key insight: this is not a port or migration of the old adapter — the new contract is fundamentally different (snapshot/delta driven instead of raw Change events; explicit solve negotiation instead of stored options; typed errors instead of stringly-typed statuses).

## Canonical References

| Ref | Path | Purpose |
|-----|------|---------|
| ADR-001 | `.planning/adr/ADR-001-backend-contract-freeze.md` | Frozen trait/type surface; change process |
| BackendSession trait | `src/solver/session.rs` | Required contract: `synchronize()`, `solve()`, `close()` |
| SessionHealth trait | `src/solver/session.rs` | Optional: `health()`, `revision()` |
| SolutionView trait | `src/solver/session.rs` | Optional: `value()`, `dual()`, `reduced_cost()`, `objective_value()` |
| CallbackSession trait | `src/solver/session.rs` | Optional: `set_callback_handler()`, `clear_callback_handler()` |
| BackendMetadata trait | `src/solver/session.rs` | Optional: `name()`, `capabilities()` |
| ModelOp enum | `src/delta.rs` | 16 typed operation variants for delta application |
| DeltaBatch | `src/delta.rs` | Immutable batch of ModelOps with from→to revision pair |
| ModelSnapshot | `src/snapshot.rs` | Canonical model state for deterministic rebuild |
| SolveRequest/Result | `src/solver/request.rs` | Immutable solve policy; effective config + termination + solution |
| TerminationStatus | `src/solver/backend.rs` | 12-variant precise termination (includes InfeasibleOrUnbounded) |
| BackendError | `src/solver/backend.rs` | Categorised error with HealthEffect |
| BackendCapabilities | `src/solver/backend.rs` | 15 capability flags |
| ReferenceBackend | `src/solver/reference.rs` | Solver-neutral contract reference implementation |
| AdapterCursor | `src/sync.rs` | Revision tracking with health transitions |
| Current ffi.rs | `roml-highs/src/ffi.rs` | Handwritten FFI (252 lines) — TO BE REPLACED |
| Current adapter.rs | `roml-highs/src/adapter.rs` | SolverAdapter impl (886 lines) — TO BE REPLACED |
| Current index_map.rs | `roml-highs/src/index_map.rs` | Dense index bookkeeping — KEPT AND REUSED |
| Candidate highs-sys migration | `c1d5e90` on `planning/roml-M1-native-backends-release` | Evidence: highs-sys 1.15.0 works with adapter patterns |
| Phase packet | `.planning/phases/11-highs-projection-session-rewrite/phase.md` | Target modules, tasks 02.1–02.9, gate criteria |
| ROADMAP.md | `.planning/ROADMAP.md` | Program-level phase graph, stop conditions |

## Locked Requirements

### M1R-H1: Binding Authority
`highs-sys` (maintained `rust-or/highs-sys` crate, generated from official `highs_c_api.h`) is the sole ABI owner. No handwritten `extern "C"` declarations, no manually-copied struct layouts, no duplicate constant definitions, no duplicate `links` ownership. ROML-specific constant aliases are allowed as thin wrappers bridging naming conventions.

### M1R-H2: Fallible Construction
Construction returns `Result<Session, BackendError>`. Typed errors for: native create failure, unsupported index width, ABI/version mismatch, option validation failure, unavailable system library in discovery mode. A convenience `fn new()` that panics is NOT permitted; if retained at all, it must be explicitly named (e.g., `fn new_unchecked()`) and documented as panicking.

### M1R-H3: Exhaustive Native Checking
Every native return code, pointer, length, index width, callback userdata, and lifecycle transition is checked. No ignored `Highs_set*OptionValue` failures, no silent `Highs_getSolution` errors, no unvalidated `Highs_addVar`/`Highs_addRow` return codes.

### M1R-H4: Thread-Safety Audit
- `Send`: Implement ONLY with documented justification (exclusive handle ownership, no shared mutable state across threads). Remove unconditional `unsafe impl Send`.
- `Sync`: NEVER implement without explicit proof. HiGHS C API is not thread-safe — calling `Highs_run` on the same handle from multiple threads is UB.
- Every `unsafe` block paired with a safety comment documenting the invariants it upholds.

### M1R-H5: Full Contract Implementation
Snapshot rebuild, every admitted delta operation, solve negotiation, solve, and extraction implement the frozen contract. The session supports all 16 `ModelOp` variants (or explicitly rejects unsupported ones with clear errors).

### M1R-H6: Correct Status Mapping
`MODEL_STATUS_UNBOUNDED_OR_INFEASIBLE` maps to `TerminationStatus::InfeasibleOrUnbounded` (NOT collapsed into `Infeasible`). `Highs_getRunStatus` is checked separately from `Highs_getModelStatus` to distinguish run errors from model outcomes. Feasible-but-not-proven (MIP with gap) maps to `TerminationStatus::Feasible`.

### M1R-H7: Atomic Unsupported-Domain Rejection
Semi-continuous variables and unsupported-domain paths are rejected BEFORE any state modification. The rejection does not leave the session in a partially-applied state. Replayability is preserved: the source delta/snapshot is intact and the session can retry after correction.

### M1R-H8: Version and Configuration Metadata
Queryable via `BackendMetadata`: HiGHS version (major.minor.patch), build mode (bundled vs system), index width (32-bit or 64-bit), effective solve configuration. Uses `Highs_getHighsVersion`, `Highs_getHighsVersionString`, `Highs_getHighsCompilationDate`.

## Decisions

### Decision 1: Implementation Strategy — Complete Rewrite with Pattern Extraction

**Chosen: Option A (Complete Rewrite)**

The existing adapter is discarded as a structural unit. The new session implementation is written from scratch against `BackendSession`. However, proven internal patterns are extracted and reused:

| Pattern | Source | Destination |
|---------|--------|-------------|
| IndexMap + reindex_after_delete | `index_map.rs` | KEPT as-is (already a clean module) |
| HiGHS infinity normalization (highs_bounds) | `adapter.rs:247-251` | Extracted to `projection.rs` |
| Bound caching (var_bounds, con_bounds) | `adapter.rs:64-68` | Extracted to `projection.rs` |
| Objective cost/sense caching | `adapter.rs:69-76` | Extracted to `projection.rs` |
| Status mapping logic (expanded) | `adapter.rs:234-245` | Extracted to `solution.rs` |
| Callback trampoline + state management | `adapter.rs:571-650` | Extracted to `callback.rs` |
| highs-sys migration (candidate c1d5e90) | candidate branch | Adopted for `bindings.rs` |

**Alternatives considered:**
- **Option B (Incremental Migration):** Rejected because the old adapter is structurally tied to `SolverAdapter` (Change-based, not ModelOp-based; solves without explicit request; uses removed `SolverStatus`/`SolverError` types). Incremental migration would mean maintaining two parallel code paths during transition, increasing risk.
- **Option C (Cherry-pick from candidate):** Rejected as a standalone strategy because the candidate only migrated FFI — it didn't touch the adapter. Cherry-picking gains the FFI migration but the adapter still needs a complete rewrite.

**Rationale:** The frozen contract represents a clean break from the legacy `SolverAdapter`. A rewrite ensures alignment without legacy compromise. Extracting proven patterns preserves the tested domain logic (index maintenance, bound caching, objective switching) without carrying forward the structural debt.

---

### Decision 2: highs-sys Version Pin — 1.15.0

**Chosen: Option A (Pin to 1.15.0)**

`highs-sys = "1.15"` in Cargo.toml, with exact-version pin in Cargo.lock. Version 1.15.0 is the latest published version on crates.io.

**API coverage verification** (symbols needed by BackendSession that the old adapter didn't use):

| API | Purpose | In 1.15.0? |
|-----|---------|-------------|
| `Highs_getRunStatus` | Distinguish run errors from model outcomes | Verified in candidate |
| `Highs_getHighsVersion` | H8: version major/minor/patch | Standard C API |
| `Highs_getHighsVersionString` | H8: human-readable version | Standard C API |
| `Highs_getHighsCompilationDate` | H8: build metadata | Standard C API |
| `Highs_getInfoValue` | Iteration/node count for result | Standard C API |
| `Highs_getBasicVariables` | Basis status (M1R-03) | Standard C API |
| `Highs_passModel` | Bulk model load (performance) | Standard C API |

All standard HiGHS C API functions are available in highs-sys 1.15.0 since the bindings are generated from the full `highs_c_api.h` header.

**Alternatives considered:**
- **Option B (Latest release):** Equivalent to Option A — 1.15.0 is latest.
- **Option C (Git revision):** Rejected. No callback-specific fixes are needed beyond what 1.15.0 provides. Git pin would complicate CI and reproducible builds.

**Rationale:** The candidate branch already validated structural compatibility. 1.15.0 provides all APIs needed for the full BackendSession contract including the new requirements (run status, version queries, info queries).

---

### Decision 3: Test Strategy — Contract Tests First (TDD)

**Chosen: Option A (Write Contract Tests First)**

Contract tests are written against the frozen `BackendSession` trait, verified against `ReferenceBackend` (already correct), then run against the HiGHS implementation as it's built.

**Contract test categories:**

| Category | Tests | Runs against ReferenceBackend? | Runs against HiGHS? |
|----------|-------|-------------------------------|---------------------|
| C1: Empty model | Rebuild from empty snapshot; solve empty model | Yes | Yes |
| C2: Full rebuild | All entity types (var, con, obj, param, cell) | Yes | Yes |
| C3: Incremental delta | All 16 ModelOp variants individually | Yes | Yes |
| C4: Commuting square | snapshot(r1) == apply(snapshot(r0), deltas r0→r1) | Yes | Yes |
| C5: Activity toggling | Deactivate/reactivate preserves bounds | Yes | Yes |
| C6: Objective switching | Switch active objective; costs/sense update | Yes | Yes |
| C7: Unsupported rejection | Semi-continuous rejected atomically | Yes | Yes |
| C8: Status mapping | All 15 HiGHS model statuses → TerminationStatus | No | Yes |
| C9: Solve | Optimal, infeasible, unbounded, MIP feasible | No | Yes |
| C10: Metadata | Version, build mode, capabilities queryable | No | Yes |
| C11: Fallible construction | Missing library, wrong index width | No | Yes |

Categories C1–C7 are solver-agnostic and run against both backends. Categories C8–C11 require an actual HiGHS instance.

**Alternatives considered:**
- **Option B (Implement then test):** Rejected. Higher risk of missing contract requirements discovered late.
- **Option C (Live at heads):** Rejected. Single evolving test file doesn't provide the coverage granularity needed for the phase gate.

**Rationale:** The contract is FROZEN — it won't change. The ReferenceBackend already proves the contract is implementable. Writing tests first provides an executable specification and catches misunderstandings before implementation. This also front-loads work for M1R-03 (differential/fault qualification).

---

### Decision 4: Backend Crate Structure — Restructured Modules

**Chosen: Option B (Restructured with Clear Module Boundaries)**

The monolithic adapter is replaced with focused modules aligned to the phase packet's target file boundaries:

```
roml-highs/src/
├── lib.rs              # Public API: re-exports Session, Error
├── bindings.rs         # pub use highs_sys::* + ROML constant aliases
├── error.rs            # BackendError construction helpers; native→category mapping
├── lifecycle.rs        # Session construction, ownership, Drop, version checks
├── projection.rs       # Snapshot→HiGHS rebuild; ModelOp→HiGHS apply
├── session.rs          # BackendSession impl (thin delegation to projection/solution)
├── solution.rs         # Status mapping (Highs→TerminationStatus); solution extraction
├── callback.rs         # Callback bridge (only officially supported callbacks)
└── index_map.rs        # KEPT AS-IS: dense index bookkeeping
```

**Module responsibilities and dependencies:**

```
bindings.rs   → depends on: highs-sys (external)
error.rs      → depends on: roml::solver::backend (BackendError, ErrorCategory)
lifecycle.rs  → depends on: bindings, error
projection.rs → depends on: bindings, error, index_map, roml (ModelOp, ModelSnapshot)
session.rs    → depends on: lifecycle, projection, solution, error, roml (BackendSession)
solution.rs   → depends on: bindings, roml (TerminationStatus, SolveSolution)
callback.rs   → depends on: bindings, roml (CallbackHandler, CallbackData)
index_map.rs  → depends on: nothing (pure data structure)
```

**Alternatives considered:**
- **Option A (Keep as-is):** Rejected. The monolithic adapter.rs (886 lines) violates SRP and makes independent testing difficult. The phase packet explicitly calls for avoiding a "monolithic adapter owning ABI, model projection, solve policy, callbacks, and extraction."

**Rationale:** Each module has a single responsibility with clear dependencies. Bindings/lifecycle, projection, and solve/extraction lanes can be developed in parallel (per the phase packet's parallelism note). `index_map.rs` stays as-is — it's already well-tested and clean.

---

## Additional Autonomous Decisions

### AD-1: Old Code Disposition

The entire existing `adapter.rs` and `ffi.rs` are deleted. `lib.rs` is rewritten to export the new session type instead of `HighsAdapter`/`SolverAdapter`/`SolverError`/`SolverStatus`/`SolverModelExt`. The new public API is:

```rust
pub mod bindings;
mod error;
mod lifecycle;
mod projection;
mod session;
mod solution;
mod callback;
mod index_map;

pub use session::HighsSession;
pub use error::HighsError;
```

The old `HighsAdapter` name is NOT retained. The new session type is `HighsSession`, clearly signaling it implements `BackendSession`, not `SolverAdapter`.

### AD-2: Send/Sync Policy

- **Send:** Implemented with documented justification. The `HighsSession` owns the `*mut c_void` handle exclusively. Moving the session to another thread is safe because no other thread holds a reference to the handle. The safety comment must explicitly state: (1) the handle is created and destroyed within the session's lifecycle, (2) no internal references escape the session, (3) callbacks are torn down before the session is sent.
- **Sync:** NOT implemented. HiGHS is not thread-safe. `Highs_run` cannot be called concurrently on the same handle. The phase packet says "Never implement Sync without explicit proof" — no such proof exists.
- Every `unsafe` block has a `// SAFETY:` comment documenting the invariants.

### AD-3: Feature Topology

Per task 02.1: "Define bundled/static default and optional system-discovery feature behavior."

```toml
[features]
default = ["bundled"]
bundled = []       # Build HiGHS from source via cmake (default)
system = []        # Discover system HiGHS via HIGHS_ROOT/pkg-config
```

The `bundled` feature keeps the existing `build.rs` cmake path but makes it the default. The `system` feature uses pkg-config or `HIGHS_ROOT`/`HIGHS_LIB_DIR` environment variables. Both features use `highs-sys` for the bindings — the feature only controls how the native library is linked.

`docs.rs` builds use `bundled` so documentation generation doesn't require a system HiGHS installation (per task 02.1 requirement).

### AD-4: Callback Disposition

Per task 02.8: "Implement only legal progress/interruption/incumbent behavior. Reject lazy constraints/user cuts/incumbent injection unless officially supported and tested."

HiGHS 1.14+ officially supports these callback types:
- `kCallbackMipLogging` (1) — progress logging → **supported**
- `kCallbackMipInterrupt` (2) — user interrupt → **supported**
- `kCallbackMipSolution` (3) — candidate solution found → **informational only**
- `kCallbackMipImprovingSolution` (4) — improving incumbent → **informational only**
- `kCallbackMipGetCutPool` (7) — cut pool access → **not implemented (read-only diagnostic)**
- `kCallbackMipDefineLazyConstraints` (8) — lazy constraint checking → **officially supported, implemented**

The existing adapter's lazy constraint callback implementation is extracted and adapted. User cuts and incumbent injection are NOT in the official callback type list for HiGHS 1.14+ and are REJECTED.

The `CallbackSession` trait is implemented. `CallbackHandler` trait usage matches the existing pattern (boxed trait object, `on_candidate` method).

### AD-5: Semi-Continuous Handling (H7 Compliance)

`ModelOp` has no semi-continuous-specific variant. Instead, `VariableEntry` in `ModelSnapshot` carries `semicontinuous_lower: Option<f64>`. Semi-continuous rejection happens:

1. **At rebuild time:** Before applying any state to HiGHS, scan the snapshot's variables. If any `VariableEntry` has `semicontinuous_lower: Some(_)`, return `BackendError::unsupported("semi-continuous variables")` with `HealthEffect::RequiresRebuild`. The session state is unchanged.

2. **At delta apply time:** `ModelOp::AddVariable` could carry semi-continuous info (though the current `ModelOp` doesn't have a field for it). If this is added later, the same pre-validation applies.

No partial application occurs. The source snapshot/delta is preserved for replay.

### AD-6: Status Mapping (H6 Compliance)

Mapping from HiGHS model status to `TerminationStatus`:

| HiGHS Constant | TerminationStatus | Notes |
|---------------|-------------------|-------|
| `MODEL_STATUS_OPTIMAL` (7) | `Optimal` | Proven optimal |
| `MODEL_STATUS_INFEASIBLE` (8) | `Infeasible` | Proven infeasible |
| `MODEL_STATUS_UNBOUNDED_OR_INFEASIBLE` (9) | `InfeasibleOrUnbounded` | PRESERVES AMBIGUITY |
| `MODEL_STATUS_UNBOUNDED` (10) | `Unbounded` | Proven unbounded |
| `MODEL_STATUS_OBJECTIVE_BOUND` (11) | `Feasible` | MIP: objective bound reached |
| `MODEL_STATUS_OBJECTIVE_TARGET` (12) | `Feasible` | MIP: objective target reached |
| `MODEL_STATUS_TIME_LIMIT` (13) | `TimeLimit` | Time limit reached |
| `MODEL_STATUS_ITERATION_LIMIT` (14) | `IterationLimit` | Iteration limit reached |
| `MODEL_STATUS_MODEL_EMPTY` (6) | `Optimal` | Empty model is trivially optimal |
| `MODEL_STATUS_LOAD_ERROR` (1) | `Error` | Model loading failed |
| `MODEL_STATUS_MODEL_ERROR` (2) | `Error` | Model structure error |
| `MODEL_STATUS_PRESOLVE_ERROR` (3) | `NumericalIssue` | Presolve failure |
| `MODEL_STATUS_SOLVE_ERROR` (4) | `Error` | Solver error during run |
| `MODEL_STATUS_POSTSOLVE_ERROR` (5) | `Error` | Postsolve failure |
| `MODEL_STATUS_UNKNOWN` (15) | `Unknown` | Unknown status |

**Run status check:** Before mapping model status, check `Highs_getRunStatus`:
- `kOk` → normal, proceed to model status mapping
- `kWarning` → log warning, proceed to model status mapping
- `kError` → return `TerminationStatus::Error` regardless of model status

**Feasible-but-not-proven:** When model status is `OBJECTIVE_BOUND`, `OBJECTIVE_TARGET`, `TIME_LIMIT`, or `ITERATION_LIMIT`, check if a feasible solution exists via `Highs_getSolution`. If primal values are available, the termination is the mapped status. If no solution is available, return `TerminationStatus::Error`.

### AD-7: Node Limit

HiGHS doesn't have a direct node-limit control in the standard C API. However, `TerminationStatus::NodeLimit` exists in the contract. For HiGHS, node limits are not natively supported — the `SolveRequest` negotiation will reject node-limit requests with a `ConfigRejection`. If a future HiGHS version adds node limits, the mapping can be added.

### AD-8: Interrupted Status

HiGHS supports user interruption via the callback mechanism (`kCallbackMipInterrupt` setting `user_interrupt` in `HighsCallbackDataIn`). When the user interrupts, `Highs_run` returns. The model status may be anything — check for feasible incumbent. Map to `TerminationStatus::Interrupted`.

### AD-9: Objective Offset Semantics

ROML carries objective constants via `ObjectiveEntry.constant` in snapshots and `ModelOp::SetObjectiveCell.constant` in deltas. HiGHS has `Highs_getObjectiveValue` which returns the objective value including any constant offset. Additionally, HiGHS supports `Highs_getHighsModelObjectiveValue` or `Highs_getObjectiveOffset` — need to verify. If HiGHS applies the offset internally, the extraction is straightforward. If not, ROML must add the constant to the extracted objective value. The contract at `SolveSolution.objective_value` expects the full objective value including offset.

**Decision:** Use `Highs_getObjectiveValue` (which includes offset in HiGHS 1.14+) and verify with a test. If HiGHS doesn't include the offset, add it manually.

## Execution Lanes (Parallelism)

Per the phase packet: "binding/lifecycle, projection, solve/extraction, and documentation lanes may run in parallel after interface freeze."

| Lane | Modules | Depends On | Independent? |
|------|---------|------------|--------------|
| L1: Binding + Lifecycle | `bindings.rs`, `error.rs`, `lifecycle.rs` | highs-sys | Yes (pure FFI + construction) |
| L2: Projection | `projection.rs`, `index_map.rs` | L1 (needs bindings) | Partially (can start after L1 has type definitions) |
| L3: Solve + Extraction | `session.rs`, `solution.rs` | L2 (needs projection) | No (needs model in HiGHS) |
| L4: Callbacks | `callback.rs` | L1, L2 | Yes (can test with mock callbacks) |
| L5: Contract Tests | Test files | L1 (for construction tests), L2+L3 (for behavioral tests) | Staggered |

## Deferred Ideas

These are explicitly deferred to later phases or future work:

1. **Bulk model loading (`Highs_passModel`):** Performance optimization. Deferred to M1R-05 (performance acceptance). The initial implementation uses incremental `Highs_addVar`/`Highs_addRow`/`Highs_changeCoeff` for correctness.

2. **Basis/warm-start save/restore:** The contract has basis support in `BackendCapabilities`. HiGHS supports `Highs_getBasicVariables` and `Highs_setBasis`. Deferred to M2-05 (basis and warm starts).

3. **SOS1/SOS2 constraints:** Not in the current contract. Deferred to M2-04 (advanced constructs).

4. **CSR/CSC matrix ingestion:** Performance optimization for large models. Deferred to M2-02.

5. **Multi-threaded solve configuration:** HiGHS supports `threads` option. The session will respect it via `SolveRequest.threads`. Thread safety of the session itself (Send but not Sync) is covered in AD-2.

6. **Crossover after barrier:** HiGHS supports barrier+crossover. The `SolveRequest.lp_algorithm` → `LpAlgorithm::Barrier` maps to HiGHS `solver = "ipm"`. Crossover is a HiGHS option (`run_crossover`) that can be set via `extra_options`. No special ROML abstraction needed at this stage.

7. **`Highs_run` async/background execution:** Long-running solves could benefit from async. Deferred to M3-03 (structured cancellation and event streams).

8. **HiGHS 64-bit index support:** The current adapter validates 32-bit indexing and panics on 64-bit. The new lifecycle module will return `Err` instead. Supporting 64-bit requires a separate feature flag and type alias — deferred until demand exists.
