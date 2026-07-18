# Phase 11 Discussion Log: HiGHS Projection/Session Rewrite

**Date:** 2026-07-18
**Mode:** Autonomous discuss-phase (headless — no user interaction)
**Artifacts:** `11-CONTEXT.md` (domain, refs, requirements, decisions, deferred ideas)

---

## Decision 1: Implementation Strategy

**Question:** How should the HiGHS adapter be converted from the removed `SolverAdapter` trait to the frozen `BackendSession` contract?

### Options Evaluated

| Criteria | Option A: Complete Rewrite | Option B: Incremental Migration | Option C: Cherry-pick Candidate |
|----------|---------------------------|--------------------------------|-------------------------------|
| Contract alignment | Direct — written against frozen contract | Indirect — adapter shaped for old trait | Partial — only FFI layer |
| Risk of legacy contamination | None | High — structural coupling to SolverAdapter | Medium — FFI clean, adapter not |
| Reuse of tested logic | Extracted patterns (IndexMap, bounds, objectives) | Full reuse of apply_one, batch logic | FFI layer only |
| Time to working code | Longer initial, faster completion | Shorter start, longer tail (dead code removal) | Medium |
| Testability | Clean module boundaries, TDD | Tests tied to monolith | Tests need rewrite either way |

### Decision: Option A — Complete Rewrite with Pattern Extraction

**Rationale:**
The structural gap between `SolverAdapter` and `BackendSession` is too large for incremental migration. Key differences:

1. **Input type:** `&[Change]` (raw events, needs batch consolidation) vs `Synchronization` (either `DeltaBatch<ModelOp>` or `ModelSnapshot`)
2. **Output type:** `SolverStatus` (5 variants, flattened) vs `SolveResult` (effective config + `TerminationStatus` (12 variants) + optional solution)
3. **Error type:** `SolverError` (stringly-typed) vs `BackendError` (categorised with `HealthEffect`)
4. **Lifecycle:** Implicit (`new()` panics) vs explicit (`close(self) -> Result`)
5. **Options:** Stored in model (`solver_options`) vs explicit per-request (`SolveRequest`)

The internal patterns worth keeping (IndexMap, bound/objective caching, infinity normalization, callback trampoline) are extracted as focused modules. The `apply_one` logic per Change variant is adapted to the 16 ModelOp variants, but the surrounding structure (SolverAdapter trait impl, `apply_changes` batch consolidation, `solve()` without request) is discarded.

**Evidence:** The frozen contract (ADR-001) was designed after studying the legacy adapter's patterns. The new `BackendSession` contract addresses every structural issue in `SolverAdapter`. A rewrite ensures alignment without compromise.

---

## Decision 2: highs-sys Version Pin

**Question:** Which version of `highs-sys` should `roml-highs` depend on?

### Options Evaluated

| Criteria | Option A: 1.15.0 | Option B: Latest | Option C: Git Revision |
|----------|-----------------|------------------|------------------------|
| API coverage | Complete — all needed symbols present | Same as A (1.15.0 = latest) | Potentially newer APIs |
| Stability | Published, semver-compatible | Same | Unstable, may rebase |
| CI reproducibility | Deterministic | Deterministic | Fragile (force-push risk) |
| Candidate validation | Verified in c1d5e90 | N/A | Not validated |
| Callback struct compatibility | Confirmed (structs match HiGHS 1.14) | Same | Unknown |

### Decision: Option A — Pin to 1.15.0

**Rationale:**
Version 1.15.0 is the latest published version on crates.io. The candidate branch (`c1d5e90` on `planning/roml-M1-native-backends-release`) validated structural compatibility: replacing 252 lines of handwritten FFI with `pub use highs_sys::*` compiled and linked successfully. All constants had matching numerical values, all struct layouts were correct, all function signatures aligned.

**Verified APIs needed beyond current usage:**
- `Highs_getRunStatus` — needed for H6 (distinguish run errors from model outcomes)
- `Highs_getHighsVersion` / `Highs_getHighsVersionString` — needed for H8 (version metadata)
- `Highs_getHighsCompilationDate` — needed for H8 (build info)
- `Highs_getInfoValue` — needed for solve result details (iterations, nodes)
- `Highs_getBasicVariables` — needed for M1R-03 (basis status)

All are standard HiGHS C API functions present in the official `highs_c_api.h` header that highs-sys 1.15.0 is generated from.

**Fallback plan:** If a needed API is discovered missing during implementation, evaluate the latest highs-sys release at that point and record the version change as a decision update.

---

## Decision 3: Test Strategy

**Question:** What is the test strategy for the HiGHS BackendSession implementation?

### Options Evaluated

| Criteria | Option A: Contract Tests First (TDD) | Option B: Implement Then Test | Option C: Live at Heads |
|----------|--------------------------------------|------------------------------|-------------------------|
| Early contract validation | Tests written before code | Tests after code | Tests evolve with code |
| Coverage confidence | High — tests define expected behavior | Medium — tests may miss edge cases | Low — unstructured |
| Regressions caught | Immediately | May be discovered late | Ad-hoc |
| M1R-03 readiness | Front-loads differential tests | Extra work in M1R-03 | Extra work in M1R-03 |
| Development velocity | Slower initial, faster verification | Faster initial, slower verification | Variable |

### Decision: Option A — Write Contract Tests First

**Rationale:**
The contract is FROZEN (ADR-001). There will be no more changes to `BackendSession` trait signatures. The `ReferenceBackend` already implements the contract correctly and can serve as the oracle for solver-agnostic tests. This makes TDD natural:

1. **Write contract test** → fails against HiGHS (not implemented)
2. **Implement the feature** → test passes against HiGHS
3. **Run against ReferenceBackend** → confirms both agree

The phase packet lists 7 categories of solver-agnostic contract tests (C1–C7: empty model, full rebuild, incremental delta, commuting square, activity toggling, objective switching, unsupported rejection) that can be written and verified against ReferenceBackend before a single line of HiGHS code.

Execution-dependent tests (C8–C11: status mapping, solve, metadata, fallible construction) require a working HiGHS instance and are written alongside the implementation.

**Test categories with 11 contract groups specified in CONTEXT.md.**

---

## Decision 4: Backend Crate Structure

**Question:** How should `roml-highs/src/` be organized?

### Options Evaluated

| Criteria | Option A: Keep As-Is | Option B: Restructured Modules |
|----------|---------------------|-------------------------------|
| Module cohesion | Low — 886-line monolith | High — one responsibility per file |
| Independent testability | Poor — everything coupled | Good — most modules testable in isolation |
| Parallel development | Impossible — single file | 4 lanes (L1–L4) can run in parallel |
| SRP compliance | Violates — ABI + projection + solve + callbacks + extraction | Compliant — each module has one job |
| Phase packet alignment | Contradicts explicit guidance | Matches target file boundaries |

### Decision: Option B — Restructured with Clear Module Boundaries

**Rationale:**
The phase packet explicitly calls for avoiding "a monolithic adapter owning ABI, model projection, solve policy, callbacks, and extraction." The proposed structure:

```
roml-highs/src/
├── lib.rs              # Public API
├── bindings.rs         # pub use highs_sys::* + ROML aliases (≈30 lines)
├── error.rs            # BackendError construction (≈50 lines)
├── lifecycle.rs        # Construction, ownership, Drop, version (≈150 lines)
├── projection.rs       # Snapshot rebuild + ModelOp apply (≈500 lines)
├── session.rs          # BackendSession impl (≈150 lines)
├── solution.rs         # Status mapping + extraction (≈200 lines)
├── callback.rs         # Callback bridge (≈150 lines)
└── index_map.rs        # KEPT AS-IS (122 lines)
```

Each module has a single, clearly-defined responsibility. `session.rs` is thin — it delegates to `projection.rs` for `synchronize()`, to `solution.rs` for `solve()` extraction, and to `lifecycle.rs` for `close()`. This keeps the trait implementation readable.

The existing `index_map.rs` stays as-is — it's already clean, tested, and has no dependencies on the adapter.

**Backward compatibility:** The old `HighsAdapter` name is NOT retained. The new public type is `HighsSession`, clearly signaling it implements `BackendSession`. Old code using `HighsAdapter` will fail to compile with a clear error pointing to the new type.

---

## Autonomous Decisions (No Viable Alternatives)

### AD-1: Old Code Disposition

**Decision:** Delete `adapter.rs` and `ffi.rs` entirely. Rewrite `lib.rs` to export `HighsSession` instead of `HighsAdapter`.

**Reasoning:** The old types (`SolverAdapter`, `SolverError`, `SolverStatus`, `SolverModelExt`) have been removed from `roml` in Phase 10. The old adapter cannot even compile. There is no value in preserving code that targets a removed trait. The proven internal patterns (IndexMap, bound/objective caching, callback trampoline) are extracted to new modules — they don't need the old file structure.

### AD-2: Send/Sync Policy

**Decision:** Implement `Send` with documented justification. Do NOT implement `Sync`.

**Reasoning:**
- **Send:** The `HighsSession` owns the HiGHS handle exclusively. No internal references escape. Moving to another thread is safe because ownership transfer guarantees exclusive access. The current adapter has `unsafe impl Send` without documentation — the new implementation provides a `// SAFETY:` comment justifying the claim.
- **Sync:** HiGHS C API is NOT thread-safe. Calling `Highs_run` from multiple threads on the same handle is undefined behavior. The phase packet explicitly says "Never implement Sync without explicit proof." No such proof exists. The absence of `Sync` prevents `&HighsSession` from being shared across threads, which is the correct safety invariant.
- **Callback constraint:** If `Send` is implemented, callbacks must be torn down BEFORE the session is sent to another thread (the callback trampoline captures a raw pointer to session state). The `CallbackSession::clear_callback_handler()` method ensures this.

### AD-3: Feature Topology

**Decision:** Default `bundled` (build from source), optional `system` (discover installed library).

**Reasoning:** Per task 02.1: "Define bundled/static default and optional system-discovery feature behavior. Ensure docs/core builds do not require a system HiGHS installation." The `bundled` feature uses the existing cmake-based build.rs path. The `system` feature uses pkg-config or `HIGHS_ROOT`/`HIGHS_LIB_DIR` environment variables. Both features use `highs-sys` for bindings — only linking differs. `docs.rs` uses the `bundled` default.

### AD-4: Callback Disposition

**Decision:** Support only officially documented HiGHS callback types. Implement lazy constraints via `kCallbackMipDefineLazyConstraints`. Log and interrupt callbacks are informational. Reject user cuts and incumbent injection.

**Reasoning:** Per task 02.8: "Implement only legal progress/interruption/incumbent behavior. Reject lazy constraints/user cuts/incumbent injection unless officially supported and tested." HiGHS 1.14+ officially documents `kCallbackMipDefineLazyConstraints` as the mechanism for lazy constraint checking. This is the only mutation-capable callback type. User cuts and incumbent injection are not in the official callback type list and are rejected.

### AD-5: Semi-Continuous Handling

**Decision:** Reject semi-continuous variables at rebuild/delta-apply time before any state mutation. Return `BackendError::unsupported()` with `HealthEffect::RequiresRebuild`.

**Reasoning:** Per H7: "Semi-continuous and unsupported-domain paths cannot partially apply then lose replayability." The rejection happens as a pre-validation step — scan the snapshot's `VariableEntry` list for any `semicontinuous_lower: Some(_)` before touching HiGHS state. If found, return error immediately. The session remains unchanged, the source delta/snapshot is preserved, and the caller can retry after removing the unsupported variable.

### AD-6: Status Mapping Details

**Decision:** Preserve infeasible-or-unbounded ambiguity. Check run status before model status. Return `Feasible` for MIP with gap but incumbent.

**Reasoning:** Per H6: "Status mapping preserves infeasible-or-unbounded ambiguity and feasible-but-not-proven outcomes." The old adapter collapsed `MODEL_STATUS_UNBOUNDED_OR_INFEASIBLE` into `SolverStatus::Infeasible` — this was incorrect. The new mapping preserves the distinction using `TerminationStatus::InfeasibleOrUnbounded`. Run status (`Highs_getRunStatus`) is checked first — if the run itself failed, the model status is not meaningful and `TerminationStatus::Error` is returned.

### AD-7: Objective Offset Semantics

**Decision:** Use `Highs_getObjectiveValue` which includes the objective constant in HiGHS 1.14+. Verify with a test.

**Reasoning:** The ROML contract expects `SolveSolution.objective_value` to include any objective constant offset. HiGHS 1.14+ includes the constant in `Highs_getObjectiveValue`. This behavior is verified with a contract test that sets an objective constant and checks the extracted value matches.

### AD-8: Interrupted Status

**Decision:** Map user-initiated interruption (via callback or external signal) to `TerminationStatus::Interrupted`.

**Reasoning:** HiGHS supports interruption via `kCallbackMipInterrupt` setting `user_interrupt` in callback data, or by external mechanisms. When interrupted, `Highs_run` returns. The session checks if a feasible incumbent exists and maps to `Interrupted` with any available solution data.

---

## Evidence Reviewed

| Source | Type | Key Findings |
|--------|------|-------------|
| `roml-highs/src/adapter.rs` (886 lines) | Current code | Implements removed `SolverAdapter` trait → E0432 errors. Contains proven patterns: IndexMap, bound/objective caching, callback trampoline, batch consolidation. |
| `roml-highs/src/ffi.rs` (252 lines) | Current code | Handwritten `extern "C"`, manually-copied callback structs, hardcoded constants. Candidate (c1d5e90) proved `pub use highs_sys::*` replaces all of it. |
| `roml-highs/src/index_map.rs` (122 lines) | Current code | Clean, tested, reusable. No adapter dependency. |
| `roml-highs/src/lib.rs` | Current code | Re-exports removed types (`SolverAdapter`, `SolverError`, `SolverStatus`, `SolverModelExt`). |
| `roml-highs/Cargo.toml` | Current config | No highs-sys dependency. Manual cmake build via build.rs. |
| `roml-highs/build.rs` | Current build | Supports HIGHS_SOURCE_DIR, HIGHS_ROOT, HIGHS_LIB_DIR. Will be adapted for feature-gated linking. |
| Candidate commit `c1d5e90` | Prior art | Validated highs-sys 1.15.0 structural compatibility. ROML constant aliases preserve naming. |
| `src/solver/session.rs` | Frozen contract | 5 traits: BackendSession (required), 4 optional. Synchronization enum with DeltaBatch/Rebuild variants. |
| `src/solver/reference.rs` | Reference impl | Proves contract is implementable. Serves as oracle for contract tests. Commuting square test already exists. |
| `src/solver/backend.rs` | Frozen types | TerminationStatus (12 variants), BackendError (with HealthEffect), BackendCapabilities (15 flags), BackendInfo. |
| `src/solver/request.rs` | Frozen types | SolveRequest (immutable policy), SolveResult (effective config + termination + solution), ConfigAdjustment/Rejection. |
| `src/delta.rs` | Frozen types | ModelOp (16 variants, self-contained), DeltaBatch (from→to revision pair). |
| `src/snapshot.rs` | Frozen types | ModelSnapshot with VariableEntry (includes semicontinuous_lower), ConstraintEntry, ObjectiveEntry (includes constant). |
| `src/sync.rs` | Frozen types | AdapterCursor (revision tracking, health transitions), ApplyOutcome (4 variants), SyncCoordinator. |
| `src/solver/callback.rs` | Frozen types | CallbackHandler trait, CallbackData, CallbackCut, CallbackAction. |
| `src/revision.rs` | Frozen types | ModelRevision (monotonic, opaque u64). |
| `src/journal.rs` | Frozen types | Journal (BTreeMap of DeltaBatches), replay queries. |
| `.planning/phases/11-highs-projection-session-rewrite/phase.md` | Phase packet | 9 tasks (02.1–02.9), target modules, gate criteria, verification commands. |
| `.planning/adr/ADR-001-backend-contract-freeze.md` | Architecture decision | Frozen trait/type signatures, change process requiring notification to downstream workers. |
| `.planning/ROADMAP.md` | Program roadmap | Phase dependencies, parallel execution policy, stop conditions. |

---

## Hard Constraints Carried Forward

1. **No handwritten ABI survives** (H1 gate). Verified: candidate proved `pub use highs_sys::*` replaces all handwritten declarations.
2. **No panic-based normal construction** (H2 gate). Verified: new lifecycle module returns `Result`, panicking convenience retained only if explicitly named.
3. **Every native return code checked** (H3). Verified: new error module maps every HiGHS return code to `BackendError`.
4. **No unjustified Send/Sync** (H4). Verified: Send justified by exclusive ownership; Sync never implemented.
5. **Full contract implementation** (H5). Verified: 16 ModelOp variants mapped to HiGHS API calls.
6. **Correct status mapping** (H6). Verified: InfeasibleOrUnbounded preserved; run status checked separately.
7. **Atomic unsupported rejection** (H7). Verified: pre-validation before state mutation.
8. **Version/build metadata queryable** (H8). Verified: Highs_getHighsVersion family mapped to BackendMetadata.

---

## Open Questions for Plan Phase

These are implementation details that the plan phase should resolve:

1. **Exact `Highs_getObjectiveValue` offset behavior:** Does HiGHS 1.14+ include the constant in the objective value? Verify with a targeted test.
2. **`Highs_getRunStatus` availability in highs-sys 1.15.0:** Confirm the function is present in the generated bindings. If missing, file an issue and use a workaround (check `Highs_run` return value).
3. **Thread count option name:** HiGHS option for threads is `"threads"`. Confirm exact string and type.
4. **Presolve option name:** HiGHS option is `"presolve"` (string: "on"/"off"). The SolveRequest doesn't have an explicit presolve field — it goes through `extra_options`. Determine if this needs a dedicated field.
5. **Basis status extraction:** `Highs_getBasicVariables` returns `HighsInt` values (0=basic, -1=nonbasic at lower, etc.). The contract test for basis is deferred to M2-05.
6. **MIP gap extraction:** HiGHS reports MIP gap via `Highs_getInfoValue` with key `"mip_gap"`. The `SolveSolution` type doesn't currently include MIP gap — add to `EffectiveConfig` or `SolveResult`.
