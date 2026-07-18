# Phase 11 (M1R-02): HiGHS Projection/Session Rewrite — Research

**Researched:** 2026-07-18
**Domain:** HiGHS native solver integration, snapshot/delta projection, safe FFI, BackendSession implementation
**Confidence:** HIGH

## Summary

Phase 11 rewrites `roml-highs` from a legacy `SolverAdapter` (886-line monolithic adapter + 252-line handwritten FFI) into a safe `BackendSession` implementation using authoritative `highs-sys` 1.15.0 bindings. The current crate is fully broken (E0432 — it imports `SolverAdapter`, `SolverModelExt`, `SolverStatus` types removed in Phase 10). The candidate branch at `planning/roml-M1-native-backends-release` commit `c1d5e90` already demonstrates working `highs-sys` integration with the same constant aliasing pattern.

**Primary recommendation:** Execute the complete rewrite with TDD contract tests. Keep `index_map.rs` as-is. Adopt the candidate branch's `bindings.rs` pattern for highs-sys integration. Implement 8 focused modules with clear dependency order: bindings/error/lifecycle first, then projection, then solution/session/callback.

### What's Different From Old Adapter

| Aspect | Old (SolverAdapter) | New (BackendSession) |
|--------|---------------------|---------------------|
| Contract | Change-driven, monolithic trait | Snapshot/delta-driven, decomposed traits |
| Errors | String-typed SolverError | Typed BackendError with HealthEffect |
| Status | SolverStatus (4 variants, collapses InfeasibleOrUnbounded) | TerminationStatus (12 variants, preserves ambiguity) |
| FFI | Handwritten extern "C" (252 lines) | highs-sys 1.15.0 re-export |
| Construction | Panics on failure | Returns Result<Session, BackendError> |
| Callbacks | Unconditional Send | Send with safety comments, no Sync |
| Test approach | SolverAdapter characterization | Contract tests shared with ReferenceBackend |

### Verified highs-sys Status

`highs-sys` 1.15.0 is the latest published version on crates.io (verified via `cargo search highs-sys`). The crate is maintained by `rust-or/highs-sys` at `github.com/rust-or/highs-sys` (repo verified reachable via HTTP 200). The candidate branch already validates structural compatibility. All required BackendSession API symbols (`Highs_getRunStatus`, `Highs_getHighsVersion`, `Highs_getHighsVersionString`, `Highs_getHighsCompilationDate`, `Highs_getInfoValue`, `Highs_getBasicVariables`, `Highs_passModel`) are standard HiGHS C API functions available in highs-sys 1.15.0 since bindings are generated from the full `highs_c_api.h` header. [CITED: rust-or/highs-sys repo]

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Decision 1: Implementation Strategy — Complete Rewrite with Pattern Extraction

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

#### Decision 2: highs-sys Version Pin — 1.15.0

**Chosen: Option A (Pin to 1.15.0)**

`highs-sys = "1.15"` in Cargo.toml, with exact-version pin in Cargo.lock. Version 1.15.0 is the latest published version on crates.io.

#### Decision 3: Test Strategy — Contract Tests First (TDD)

**Chosen: Option A (Write Contract Tests First)**

11 contract test categories (C1–C11), with C1–C7 running against both ReferenceBackend and HiGHS, and C8–C11 requiring HiGHS.

#### Decision 4: Backend Crate Structure — Restructured Modules

**Chosen: Option B (Restructured with Clear Module Boundaries)**

8 modules replacing monolithic adapter: `bindings.rs`, `error.rs`, `lifecycle.rs`, `projection.rs`, `session.rs`, `solution.rs`, `callback.rs`, `index_map.rs`.

### Claude's Discretion

#### AD-1: Old Code Disposition
Entire `adapter.rs` and `ffi.rs` deleted. `lib.rs` rewritten. Public API: `pub use session::HighsSession`, `pub use error::HighsError`.

#### AD-2: Send/Sync Policy
Send implemented with documented justification. Sync NOT implemented. Every `unsafe` block has `// SAFETY:` comment.

#### AD-3: Feature Topology
```toml
[features]
default = ["bundled"]
bundled = []
system = []
```

#### AD-4: Callback Disposition
Legal callbacks only: kCallbackMipLogging, kCallbackMipInterrupt, kCallbackMipSolution, kCallbackMipImprovingSolution (informational), kCallbackMipDefineLazyConstraints. User cuts/incumbent injection rejected.

#### AD-5: Semi-Continuous Handling (H7 Compliance)
Rejection before rebuild/delta apply, not partial state. `BackendError::unsupported("semi-continuous variables")` with `HealthEffect::RequiresRebuild`.

#### AD-6: Status Mapping (H6 Compliance)
Comprehensive table mapping all 15 HiGHS model statuses to TerminationStatus. InfeasibleOrUnbounded preserved. Run status checked separately before model status.

#### AD-7: Node Limit
Node limit not natively supported in HiGHS C API. `SolveRequest` negotiation will reject node-limit requests.

#### AD-8: Interrupted Status
User interruption via callback maps to `TerminationStatus::Interrupted`.

#### AD-9: Objective Offset Semantics
Use `Highs_getObjectiveValue` (includes offset in HiGHS 1.14+). Verify with test.

### Deferred Ideas (OUT OF SCOPE)

1. Bulk model loading (`Highs_passModel`) — M1R-05
2. Basis/warm-start — M2-05
3. SOS1/SOS2 — M2-04
4. CSR/CSC matrix ingestion — M2-02
5. Multi-threaded solve — covered by SolveRequest.threads (basic support)
6. Crossover after barrier — defer
7. `Highs_run` async/background — M3-03
8. 64-bit index support — defer until demand

</user_constraints>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| M1R-H1 | Binding Authority: highs-sys sole ABI owner | Candidate branch c1d5e90 verifies the pattern. HIGH confidence |
| M1R-H2 | Fallible Construction: returns Result, never panics | Current adapter panics in `new()`. New `lifecycle.rs` must return `Result<Session, BackendError>`. |
| M1R-H3 | Exhaustive Native Checking: every return code checked | Current adapter ignores option-set failures. Every `Highs_set*OptionValue`, `Highs_getSolution`, `Highs_addVar`/`addRow` must be checked. |
| M1R-H4 | Thread-Safety Audit: documented Send, no Sync | Current adapter has `unsafe impl Send for HighsAdapter {}` without safety comment. Must replace with documented Send, no Sync. |
| M1R-H5 | Full Contract Implementation: all 16 ModelOp variants | ReferenceBackend implements all 16. HiGHS must support or explicitly reject. |
| M1R-H6 | Correct Status Mapping: InfeasibleOrUnbounded preserved | Current adapter collapses it into Infeasible. New mapping expands to 12 TerminationStatus variants. |
| M1R-H7 | Atomic Unsupported-Domain Rejection: no partial apply | Semi-continuous rejection before state modification. |
| M1R-H8 | Version/Configuration Metadata queryable via BackendMetadata | Uses `Highs_getHighsVersion`, `Highs_getHighsVersionString`, `Highs_getHighsCompilationDate`. |

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Model rebuild from snapshot | Backend (HighsSession) | — | Snapshot is API-side; projection is backend-side |
| Delta application | Backend (HighsSession) | — | 16 ModelOp variants map to HiGHS C API calls |
| Solve negotiation | Backend (HighsSession) | — | Translate SolveRequest to Highs_set*OptionValue |
| Solve execution | Backend (HighsSession) | — | Highs_run is inherently native |
| Solution extraction | Backend (HighsSession) | — | Highs_getSolution results mapped to SolveSolution |
| Status mapping | Backend (HighsSession) | — | HiGHS model status → TerminationStatus |
| Callback bridge | Backend (HighsSession) | — | HiGHS C callback → rust CallbackHandler |
| Health tracking | Backend (HighsSession) | — | AdapterCursor managed internally |
| Version metadata | Backend (HighsSession) | — | Highs_getHighsVersion |
| Index bookkeeping | Backend (HighsSession) | — | IndexMap maintains VarId/ConId → HiGHS index |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `highs-sys` | 1.15.0 | HiGHS C API Rust bindings | Sole maintained ABI owner; replaces 252 lines handwritten FFI |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `log` | 0.4 (workspace) | Logging during solve | Everywhere (workspace dep) |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| highs-sys (generated) | Handwritten FFI | Adversarial: 252 lines prone to struct layout drift, missing symbol exposure |
| highs-sys (generated) | Orphaned forks (ferrox-highs-sys) | Not maintained; rust-or/highs-sys is the canonical fork |

**Installation:**
```toml
[dependencies]
highs-sys = "1.15"
```

**Version verification:** Verified `highs-sys` 1.15.0 at `crates.io/crates/highs-sys/1.15.0`, repository `github.com/rust-or/highs-sys`.

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| highs-sys 1.15.0 | crates.io | >2 years | High | github.com/rust-or/highs-sys | OK | Approved |
| cmake (build-dep) | crates.io | >10 years | Very high | github.com/rust-lang/cmake-rs | OK | Approved |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

## Architecture Patterns

### System Architecture Data Flow

```
Snapshot/Delta (roml API)
         │
         ▼
  HighsSession::synchronize()
         │
         ├── Rebuild from ModelSnapshot ──► projection.rs
         │      ├── Scan for semicontinuous ──► reject early (H7)
         │      ├── Clear HiGHS model
         │      ├── AddVar for each variable
         │      ├── AddRow for each constraint
         │      ├── ChangeCoeff for each cell
         │      ├── Set objective/sense/costs
         │      └── IndexMap updated throughout
         │
         └── Apply DeltaBatch ──► projection.rs
                ├── Prevalidate each ModelOp
                ├── Map to HiGHS C API operation
                ├── Check every return code
                ├── Update IndexMap (reindex_after_delete for remove ops)
                └── Acknowledge only on complete success

  HighsSession::solve(&SolveRequest)
         │
         ├── Negotiate config ──► Highs_set*OptionValue
         ├── Register callback (if MIP + handler set) ──► callback.rs
         ├── Highs_run
         ├── Check Highs_getRunStatus ──► solution.rs
         ├── Map Highs_getModelStatus to TerminationStatus ──► solution.rs
         └── Extract solution (Highs_getSolution) ──► solution.rs

  HighsSession::close() ──► Highs_destroy

  BackendMetadata:
         ├── name() ──► "HiGHS {version}"
         └── capabilities() ──► BackendCapabilities with lp/mip/solution/callbacks
```

### Recommended Project Structure
```
roml-highs/src/
├── lib.rs              # Module declarations; pub use HighsSession, HighsError
├── bindings.rs         # pub use highs_sys::* + ROML constant aliases
├── error.rs            # BackendError construction helpers; native→category mapping
├── lifecycle.rs        # Session construction, ownership, Drop, version checks
├── projection.rs       # Snapshot→HiGHS rebuild; ModelOp→HiGHS apply
├── session.rs          # BackendSession impl (thin delegation to projection/solution)
├── solution.rs         # Status mapping (Highs→TerminationStatus); solution extraction
├── callback.rs         # Callback bridge (only officially supported callbacks)
└── index_map.rs        # KEPT AS-IS: dense index bookkeeping
```

### Pattern 1: Binding Layer — Re-export from highs-sys

**What:** `bindings.rs` uses `pub use highs_sys::*` for all types/functions, adding ROML-specific constant aliases with numeric values verified against highs-sys.

**When to use:** Always. No handwritten extern "C".

**Example (from candidate c1d5e90):**
```rust
// bindings.rs
#![allow(non_snake_case)]

pub use highs_sys::*;

// ROML constant aliases (verified against highs-sys 1.15.0 values)
pub const STATUS_OK: HighsInt = 0;
pub const MODEL_STATUS_OPTIMAL: HighsInt = 7;
pub const MODEL_STATUS_INFEASIBLE: HighsInt = 8;
pub const MODEL_STATUS_UNBOUNDED_OR_INFEASIBLE: HighsInt = 9;
// ... all 15 model statuses, 4 var types, 2 senses, 6 callback types
```

### Pattern 2: Fallible Lifecycle with Typed Errors

**When to use:** Construction returns `Result<Session, BackendError>` with typed errors.
```rust
// lifecycle.rs
pub struct HighsSession {
    raw: *mut c_void,  // Highs_create handle
    cursor: AdapterCursor,
    // ... IndexMaps, caches, etc.
}

impl HighsSession {
    pub fn try_new() -> Result<Self, BackendError> {
        let raw = unsafe { highs_sys::Highs_create() };
        if raw.is_null() {
            return Err(BackendError::library_not_found("Highs_create returned null"));
        }
        // Validate index width
        let sz = unsafe { highs_sys::Highs_getSizeofHighsInt(raw) };
        if sz != 4 {
            unsafe { highs_sys::Highs_destroy(raw); }
            return Err(BackendError::unsupported("64-bit HighsInt not supported"));
        }
        // ...
    }
}
```

### Anti-Patterns to Avoid
- **Panicky construction:** The old adapter `HighsAdapter::new()` panics on null handle. New code returns `Result`.
- **Ignored option-set return codes:** The old adapter ignores `Highs_setBoolOptionValue` returns. New code checks every return code.
- **Unconditional unsafe impl Send:** The old adapter has `unsafe impl Send for HighsAdapter {}` without any safety comment. New code requires documented justification.
- **Collapsed status mapping:** The old adapter maps `MODEL_STATUS_UNBOUNDED_OR_INFEASIBLE` to `SolverStatus::Infeasible`. New code maps to `TerminationStatus::InfeasibleOrUnbounded`.
- **Partial delta application:** The old adapter applies changes one at a time without atomicity guarantees. New code acknowledges only on complete success or rolls back.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HiGHS C API bindings | Handwritten `extern "C"` (252 lines deleted) | `highs-sys` | Struct/layout drift, missing functions, ABI maintenance burden |
| Callback struct layout | Manual struct layout with size assertions | `highs_sys::HighsCallbackDataOut` | Layout shifts with HighsInt width; highs-sys matches the header |
| MODEL_STATUS constants | Hardcoded integers | ROML aliases wrapping highs-sys generated enums | Value correctness verified at build time |
| Index management after delete | Manual HashMap maintenance | `index_map.rs` with `reindex_after_delete` | Already tested and correct; kept as-is |

**Key insight:** The handwritten FFI is the source of the most dangerous class of bugs (silent memory corruption from struct layout mismatch). Using `highs-sys` eliminates this entire category.

## Runtime State Inventory

> Not applicable — this is a greenfield rewrite of a broken crate, not a rename/refactor. The existing `adapter.rs` and `ffi.rs` are deleted entirely; no runtime state carries forward.

## Common Pitfalls

### Pitfall 1: Struct Layout Mismatch in Callback Data

**What goes wrong:** `HighsCallbackDataOut` layout depends on `HighsInt` width (4 or 8 bytes). The handwritten struct has size assertions but cannot adapt to different builds.

**Why it happens:** HiGHS can be compiled with `-DHIGHSINT64`. When this happens, every `HighsInt` field doubles in size, shifting all subsequent fields.

**How to avoid:** Use `highs-sys` generated bindings, which match the compiled header exactly. Validate `Highs_getSizeofHighsInt` at construction.

**Warning signs:** Wrong dual/reduced cost values reading from wrong offsets; callback crashes.

### Pitfall 2: Ignored Option Return Codes

**What goes wrong:** `Highs_setBoolOptionValue` returns non-zero for invalid option names. The old adapter discards this, silently failing to apply the user's configuration.

**Why it happens:** The old adapter treats option-setting as best-effort.

**How to avoid:** Check every `Highs_set*OptionValue` return code. Return `BackendError` on failure.

**Warning signs:** Configuration doesn't take effect (output still on when turned off, wrong solver algorithm).

### Pitfall 3: Missing Highs_getRunStatus Check

**What goes wrong:** `Highs_getModelStatus` returns a valid model status even when `Highs_run` failed. The old adapter checks `Highs_getModelStatus` immediately after `Highs_run` without checking run status.

**Why it happens:** HiGHS can partially process a solve before a fatal error, leaving a stale model status.

**How to avoid:** Check `Highs_getRunStatus` separately. If `kError`, return `TerminationStatus::Error` regardless of model status.

**Warning signs:** Spurious "Optimal" result on a failed solve.

### Pitfall 4: Reindex After Delete

**What goes wrong:** When HiGHS deletes an element at index k, all higher indices shift down by 1. If the index map isn't updated, subsequent operations use wrong indices that may overlap or point to wrong entities.

**Why it happens:** HiGHS uses dense integer indexing. This is explicitly documented behavior but easy to miss.

**How to avoid:** Always call `index_map.reindex_after_delete(deleted_index)` after at every delete. This pattern is already correct in `index_map.rs` and must be preserved in the new projection code.

**Warning signs:** Model corruption after delete operations; wrong solutions referencing wrong variables.

### Pitfall 5: Objective Switching Without Zeroing Costs

**What goes wrong:** When switching active objective, old costs from the previously active objective remain in the HiGHS model alongside new costs.

**Why it happens:** HiGHS treats column costs as a single flat array. Switching requires zeroing all costs first, then loading the new objective's costs.

**How to avoid:** Follow the old adapter's pattern: call `Highs_changeColsCostByRange(0, num_cols-1, zeros)` to zero, then `Highs_changeColsCostBySet` for the new objective's non-zero costs.

**Warning signs:** Solve uses a blend of two objectives' costs.

### Pitfall 6: Infinity Normalization

**What goes wrong:** HiGHS uses `1e30` as its internal infinity, while ROML uses `f64::INFINITY` and `f64::NEG_INFINITY`. Passing `f64::INFINITY` as a bound value causes HiGHS to reject calls.

**Why it happens:** `Highs_addVar(lower, f64::INFINITY)` fails because HiGHS's `kHighsInfinity` is `1e30`.

**How to avoid:** Normalize bounds: `if lb == f64::NEG_INFINITY { -self.inf } else { lb }`, same for ub. The `highs_bounds` function in the old adapter already does this correctly.

**Warning signs:** "Invalid bound" errors from HiGHS for variables with infinite bounds.

## Code Examples

### Pattern: Callback Bridge (from old adapter, to be adapted)

```rust
// callback.rs
// SAFETY: Called from HiGHS C callback context. We only access data_out
// through verified pointers. user_data is a Box<CallbackState> we created.
unsafe extern "C" fn callback_trampoline(
    event_type: c_int,
    _message: *const c_char,
    data_out: *const highs_sys::HighsCallbackDataOut,
    _data_in: *mut highs_sys::HighsCallbackDataIn,
    user_data: *mut c_void,
) {
    // Only handle lazy-constraint callback type
    if event_type != CALLBACK_MIP_DEFINE_LAZY_CONSTRAINTS {
        return;
    }
    let state = &mut *(user_data as *mut CallbackState);
    let out = &*data_out;

    // Map HiGHS solution to ROML callback data
    let num_cols = highs_sys::Highs_getNumCol(state.highs_ptr) as usize;
    let sol_slice = std::slice::from_raw_parts(out.mip_solution, num_cols);
    // ... build var_values, invoke handler, inject cuts via Highs_addRow
}
```

### Pattern: Status Mapping (new, expanded from old)

```rust
// solution.rs
pub fn map_termination(highs: *mut c_void) -> TerminationStatus {
    // 1. Check run status first
    let run_status = unsafe { highs_sys::Highs_getRunStatus(highs) };
    if run_status != highs_sys::kHighsStatusOk
        && run_status != highs_sys::kHighsStatusWarning
    {
        return TerminationStatus::Error;
    }

    // 2. Map model status
    let model_status = unsafe { highs_sys::Highs_getModelStatus(highs) };
    match model_status {
        MODEL_STATUS_OPTIMAL => TerminationStatus::Optimal,
        MODEL_STATUS_INFEASIBLE => TerminationStatus::Infeasible,
        MODEL_STATUS_UNBOUNDED_OR_INFEASIBLE => TerminationStatus::InfeasibleOrUnbounded,
        MODEL_STATUS_UNBOUNDED => TerminationStatus::Unbounded,
        MODEL_STATUS_OBJECTIVE_BOUND | MODEL_STATUS_OBJECTIVE_TARGET => {
            // Check if feasible solution exists
            let has_solution = has_feasible_solution(highs);
            if has_solution { TerminationStatus::Feasible }
            else { TerminationStatus::Error }
        }
        MODEL_STATUS_TIME_LIMIT => TerminationStatus::TimeLimit,
        MODEL_STATUS_ITERATION_LIMIT => TerminationStatus::IterationLimit,
        // Error variants
        MODEL_STATUS_LOAD_ERROR | MODEL_STATUS_MODEL_ERROR
        | MODEL_STATUS_SOLVE_ERROR | MODEL_STATUS_POSTSOLVE_ERROR => {
            TerminationStatus::Error
        }
        MODEL_STATUS_PRESOLVE_ERROR => TerminationStatus::NumericalIssue,
        MODEL_STATUS_MODEL_EMPTY => TerminationStatus::Optimal,
        _ => TerminationStatus::Unknown,
    }
}
```

### Pattern: Snapshot Rebuild (projection.rs)

```
For each snapshot component:
1. Variables:  Highs_addVar with normalized bounds, then Highs_changeColIntegrality
2. Constraints: Highs_addRow with all coefficients (batch into a single call)
3. Cells:       Highs_changeCoeff for each (row, col, value)
4. Objective:   Highs_changeObjectiveSense, then Highs_changeColsCostBySet for costs
5. Parameters:  (model parameters — set relevant HiGHS options)
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Handwritten `extern "C"` FFI | `highs-sys` generated bindings | M1.2 (candidate branch) | Eliminates struct layout drift and missing symbols |
| `SolverAdapter` (13-method monolith) | `BackendSession` (3 methods) + optional traits | Phase 10 (M1R-01) | Cleaner decomposition, optional capabilities |
| `SolverStatus` (4 variants) | `TerminationStatus` (12 variants) | Phase 10 | Preserves ambiguity (InfeasibleOrUnbounded), limits, interruption |
| Panic on construction failure | Typed `Result<Session, BackendError>` | This phase | No silent crashes in CI or production |
| `unsafe impl Send` without comment | Send with documented safety invariants | This phase | Audit trail for thread safety |

**Deprecated/outdated:**
- `SolverAdapter` trait: Removed in Phase 10 (M1R-01). Do not implement.
- `ffi.rs` with handwritten extern "C": Delete entire file. Replace with 20-line `bindings.rs`.
- `HighsAdapter` type: Delete. Replace with `HighsSession`.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `Highs_getRunStatus` is available in highs-sys 1.15.0 | Bindings | LOW — standard HiGHS C API function; verified in candidate |
| A2 | `Highs_getHighsVersion`/`String`/`CompilationDate` available | Metadata | LOW — standard HiGHS C API |
| A3 | `Highs_getInfoValue` for iteration/node count | Solution | LOW — standard function |
| A4 | `Highs_getBasicVariables` for basis access | Solution | LOW — standard function; basis access deferred anyway |
| A5 | `Highs_getObjectiveValue` includes constant offset (v1.14+) | Solution | MEDIUM — must verify with test per AD-9 |
| A6 | Bundled feature works via highs-sys `default` features | Features | LOW — verified in candidate |

## Open Questions (RESOLVED)

1. **[RESOLVED] Objective offset semantics in `Highs_getObjectiveValue`**
   - Resolution: Plan 04 (C9 contract test) tests objective extraction with non-zero constant and compares against manually computed sum. During projection implementation, AD-9 requires documenting exact semantics based on test outcome.
   - Plan reference: 11-04-PLAN.md C9 test group

2. **[RESOLVED] Bundled feature vs. build.rs conflict**
   - Resolution: Plan 01 Task 1 deletes the old custom build.rs. `highs-sys` features handle both `bundled` (default, CMake build) and `system` (via discover feature) modes. Cargo.toml feature flags delegate directly to highs-sys.
   - Plan reference: 11-01-PLAN.md Task 1

3. **[RESOLVED] Dual/reduced_cost availability for non-optimal outcomes**
   - Resolution: Plan 04 C9 test group covers feasible incumbent extraction across multiple termination statuses (optimal, time limit with incumbent, no incumbent). Solution extraction in Plan 03 checks run status before accessing solution data.
   - Plan reference: 11-03-PLAN.md, 11-04-PLAN.md C9

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | Compilation | Yes | stable | — |
| cargo | Build | Yes | — | — |
| cmake | highs-sys bundled build | Check | — | Use `system` feature |
| HiGHS system library | `system` feature | User-configured | — | Use `bundled` (default) |

## Validation Architecture

> Not yet configured — `.planning/config.json` not found. Treat validation as enabled by default.

### Test Framework (proposed)

| Property | Value |
|----------|-------|
| Framework | built-in `cargo test` |
| Quick run | `cargo test -p roml-highs --test contract_tests` |
| Full suite | `cargo test -p roml-highs --all-targets` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command |
|--------|----------|-----------|-------------------|
| M1R-H1 | Binding authority | build | `cargo check -p roml-highs` (should not reference extern "C" blocks) |
| M1R-H2 | Fallible construction | unit | C11: bad config options, missing library |
| M1R-H3 | Exhaustive checking | unit + integration | C8: status mapping; C9: solve |
| M1R-H4 | Thread-safety | static | `rg 'unsafe impl (Send|Sync)'` audit |
| M1R-H5 | Full contract | integration | C1-C7: rebuild, delta, commuting square |
| M1R-H6 | Status mapping | integration | C8: all model statuses |
| M1R-H7 | Unsupported rejection | integration | C7: semicontinuous rejected atomically |
| M1R-H8 | Version metadata | integration | C10: version/build/capabilities |

### Verification Commands
```bash
cargo test -p roml-highs --all-targets
cargo clippy -p roml-highs --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps
cargo package -p roml-highs --locked
rg -n 'extern "C"|unsafe impl (Send|Sync)|assert!|unwrap\(|expect\(' roml-highs
```

### Wave 0 Gaps
- [ ] `tests/contract_tests.rs` — covers C1-C11
- [ ] No existing test infrastructure to modify; all tests are new

## Security Domain

> `security_enforcement` is absent from config. Treat as enabled.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V5 Input Validation | yes | Check every return code, validate pointers, CString conversions |
| V6 Cryptography | no | No cryptographic operations in HiGHS integration |

### Known Threat Patterns

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Null pointer dereference | DoS | Check `Highs_create()` return, check all returned pointers |
| Buffer overrun in callback | Tampering | Validate slice lengths from `mip_solution_size` before accessing |
| Data race on HiGHS handle | DoS | Never implement Sync; document Send safety |
| CString creation panic | DoS | Handle CString::new failures for runtime option strings |
| Undefined behavior from stale pointer | Tampering | CallbackState lifecycle: create before Highs_run, destroy after |
| Forgotten close leaking native resources | Resource exhaustion | Drop implementation calls Highs_destroy; double-free prevention |

## Sources

### Primary (HIGH confidence)
- CONTEXT.md — locked decisions and autonomous decisions for Phase 11
- phase.md — phase packet with task definitions and verification commands
- ADR-001 — frozen BackendSession trait and change process
- Current `roml-highs/src/` — full inventory of existing code (1311 lines total)
- Candidate branch `c1d5e90` — verified highs-sys migration pattern
- Frozen contract source files: `src/solver/session.rs`, `src/solver/backend.rs`, `src/delta.rs`, `src/snapshot.rs`, `src/sync.rs`, `src/solver/request.rs`, `src/solver/callback.rs`, `src/solver/reference.rs`

### Secondary (MEDIUM confidence)
- `cargo search highs-sys` — verified 1.15.0 latest on crates.io
- `github.com/rust-or/highs-sys` — verified repository exists (HTTP 200)
- REQUIREMENTS.md — M1R-H1 through M1R-H8 requirement definitions

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — highs-sys 1.15.0 verified on crates.io; candidate branch validates pattern
- Architecture: HIGH — module structure per CONTEXT.md Decision 4; 8-module boundary from phase packet
- Pitfalls: HIGH — extracted from analyzing old adapter patterns, known HiGHS C API gotchas
- Feature topology: HIGH — verified candidate branch Cargo.toml has same pattern
- Status mapping: HIGH — AD-6 maps all 15 HiGHS values; confirmed in candidate

**Research date:** 2026-07-18
**Valid until:** 2026-08-18 (30 days — standard stability window)
