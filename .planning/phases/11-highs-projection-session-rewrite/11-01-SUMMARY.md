---
phase: 11-highs-projection-session-rewrite
plan: 01
subsystem: solver-backend
tags: highs, highs-sys, ffi, lifecycle, safety

requires:
  - phase: 10-backend-contract-migration-closure
    provides: BackendContract traits (BackendSession, BackendError, AdapterCursor), frozen types
provides:
  - Authoritative highs-sys 1.15.0 bindings replacing 252-line handwritten extern C
  - Fallible lifecycle (HighsSession::try_new -> Result) instead of panicking constructor
  - Typed error helpers (check_highs_status, from_native_status) for HiGHS return codes
  - Documented thread-safety discipline (Send with invariant comment, no Sync)
  - Module scaffold for 8-module crate with projection/session/solution/callback stubs
affects:
  - 11-02-projection
  - 11-03-session
  - 11-04-solution-callback

tech-stack:
  added:
    - highs-sys = "1.15" (sole ABI owner)
  patterns:
    - pub use highs_sys::* for bindings (no handwritten extern C)
    - Fallible construction returning Result<Session, BackendError>
    - Documented unsafe impl Send with safety invariants, no impl Sync
    - Minimal build.rs delegating to highs-sys

key-files:
  created:
    - roml-highs/src/bindings.rs
    - roml-highs/src/error.rs
    - roml-highs/src/lifecycle.rs
    - roml-highs/src/projection.rs (stub)
    - roml-highs/src/session.rs (stub)
    - roml-highs/src/solution.rs (stub)
    - roml-highs/src/callback.rs (stub)
  modified:
    - roml-highs/Cargo.toml
    - roml-highs/build.rs
    - roml-highs/src/lib.rs
  deleted:
    - roml-highs/src/ffi.rs
    - roml-highs/src/adapter.rs
    - roml-highs/tests/integration.rs
key-decisions:
  - "D-01: Complete rewrite — ffi.rs and adapter.rs deleted entirely"
  - "D-02: highs-sys 1.15.0 pinned as sole ABI owner"
  - "D-05: bindings.rs re-exports highs_sys with ROML constant aliases"
  - "AD-2: unsafe impl Send with documented justification; Sync not implemented"

patterns-established:
  - "Bindings: pub use highs_sys::* + ROML constant aliases"
  - "Lifecycle: try_new() returns Result, Drop with null-pointer guard"
  - "Error: from_native_status and check_highs_status for idiomatic HiGHS error handling"
  - "Thread-safety: Unsafe impl Send with multi-paragraph SAFETY comment documenting invariants"

requirements-completed:
  - M1R-H1
  - M1R-H2
  - M1R-H3
  - M1R-H4

coverage:
  - id: D1
    description: "Binding authority — highs-sys 1.15.0 replaces handwritten extern C; ffi.rs deleted"
    requirement: M1R-H1
    verification:
      - kind: unit
        ref: "test ! -f roml-highs/src/ffi.rs"
        status: pass
      - kind: unit
        ref: "! grep -rn 'extern \"C\"' roml-highs/src/ (actual extern blocks, not comments)"
        status: pass
      - kind: unit
        ref: "cargo check -p roml-highs"
        status: pass
    human_judgment: false
  - id: D2
    description: "Fallible construction — HighsSession::try_new returns Result, no panicking fn new()"
    requirement: M1R-H2
    verification:
      - kind: unit
        ref: "grep -n 'fn new()' roml-highs/src/lifecycle.rs (exit 1 = no panicking new)"
        status: pass
    human_judgment: false
  - id: D3
    description: "Exhaustive native checking — error helpers (check_highs_status, from_native_status) return typed BackendError"
    requirement: M1R-H3
    verification:
      - kind: unit
        ref: "cargo check -p roml-highs"
        status: pass
    human_judgment: false
  - id: D4
    description: "Thread-safety audit — Send with documented justification, no Sync, every unsafe block has SAFETY comment"
    requirement: M1R-H4
    verification:
      - kind: unit
        ref: "grep -c '// SAFETY:' roml-highs/src/lifecycle.rs (3+)"
        status: pass
      - kind: unit
        ref: "! grep -rn 'unsafe impl Sync' roml-highs/src/"
        status: pass
    human_judgment: false

duration: 5min
completed: 2026-07-18
status: complete
---

# Phase 11 Plan 01: HiGHS Foundation Summary

**highs-sys 1.15.0 as sole ABI owner with fallible lifecycle, typed error helpers, module scaffold, and documented thread-safety discipline**

## Performance

- **Duration:** 5 min
- **Started:** 2026-07-18T07:47:54Z
- **Completed:** 2026-07-18T07:52:04Z
- **Tasks:** 3
- **Files modified:** 16 (7 created, 3 modified, 3 deleted, 3 regenerated)

## Accomplishments

- Deleted 252-line handwritten extern C (ffi.rs) and 886-line monolithic adapter (adapter.rs) — replaced with authoritative highs-sys 1.15.0 bindings.
- Fallible construction: `HighsSession::try_new()` returns `Result<Self, BackendError>` — no panicking default constructor.
- Lifecycle safety: `Drop` with null-pointer guard prevents double-free; `unsafe impl Send` with comprehensive safety comment; `Sync` deliberately not implemented.
- Typed error helpers: `check_highs_status()` and `from_native_status()` convert HiGHS return codes into `BackendError` with `ErrorCategory::Internal` and `HealthEffect::Recoverable`.
- Module scaffold: lib.rs declares all 8 target modules (bindings, error, index_map, lifecycle, projection, session, solution, callback) with pub use HighsSession, HighsError, HighsInt.
- Cargo.toml: bundled/system features, highs-sys = "1.15", reduced dependencies.
- build.rs: minimal passthrough (rerun-if-changed only).

## Task Commits

Each task was committed atomically:

1. **Task 1: Binding Authority** - `458b644` (feat: binding authority -- rewrite Cargo.toml, build.rs, create bindings.rs, delete ffi.rs/adapter.rs/integration.rs)
2. **Task 2: Fallible Lifecycle and Error Module** - `25daaef` (feat: fallible lifecycle and error module)
3. **Task 3: Module Wiring** - `deee883` (feat: module wiring -- rewrite lib.rs, thread-safety audit, create stub files)

## Files Created/Modified

### Created
- `roml-highs/src/bindings.rs` - pub use highs_sys::* + 15 MODEL_STATUS aliases, 6 callback type aliases, 3 status aliases, kHighsInfinity
- `roml-highs/src/error.rs` - check_highs_status(), from_native_status(), pub type HighsError = BackendError
- `roml-highs/src/lifecycle.rs` - HighsSession struct, try_new(), new_unchecked(), Drop, unsafe impl Send, pub(crate) accessors
- `roml-highs/src/projection.rs` - stub for Plan 02
- `roml-highs/src/session.rs` - stub for Plan 03
- `roml-highs/src/solution.rs` - stub for Plan 03
- `roml-highs/src/callback.rs` - stub for Plan 03

### Modified
- `roml-highs/Cargo.toml` - highs-sys = "1.15", bundled/system features, removed log4rs/serde_yaml/cmake
- `roml-highs/build.rs` - minimal passthrough (174 lines -> 3 lines)
- `roml-highs/src/lib.rs` - rewritten: declares 8 modules, pub use HighsSession/HighsError/HighsInt

### Deleted
- `roml-highs/src/ffi.rs` - 252-line handwritten extern C (replaced by highs-sys)
- `roml-highs/src/adapter.rs` - 886-line SolverAdapter implementation (replaced by new modules)
- `roml-highs/tests/integration.rs` - integration tests referencing deleted HighsAdapter

### Regenerated
- `Cargo.lock` - workspace lockfile with highs-sys 1.15.0 dependency tree

## Decisions Made

- **Implemented D-01 (Complete Rewrite):** ffi.rs and adapter.rs deleted entirely. No incremental migration.
- **Implemented D-02 (highs-sys 1.15.0 pinned):** Added as sole FFI dependency; removed cmake build-dep.
- **Implemented D-05 (Authoritative Bindings):** bindings.rs uses pub use highs_sys::* with ROML constant aliases per AD-6.
- **Implemented AD-2 (Send/Sync Policy):** unsafe impl Send with 3-paragraph safety comment; no impl Sync.
- **Feature topology per AD-3:** default = ["bundled"], bundled = [], system = [].
- **Module structure per AD-4:** 8 focused modules replacing monolithic adapter.

## Deviations from Plan

None - plan executed exactly as written with minor adaptive fixes:

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed unused imports and clippy warnings in new files**
- **Found during:** Task 2 (compilation)
- **Issue:** Unused imports (warn, HighsInt, check_highs_status, bindings::self) and unnecessary cast
- **Fix:** Removed unused imports, used ret.into() -> ret directly for native_code parameter
- **Files modified:** roml-highs/src/error.rs, roml-highs/src/lifecycle.rs
- **Committed in:** 25daaef (Task 2 commit), deee883 (Task 3 commit)

**2. [Rule 1 - Bug] Symbol mismatch between plan and highs-sys API**
- **Found during:** Task 2 (first cargo check)
- **Issue:** `Highs_getRunStatus` and `Highs_getHighsVersionString` do not exist in highs-sys 1.15.0 bindings
- **Fix:** Replaced `from_native_status` to use operation return code directly instead of querying run status; replaced `Highs_getHighsVersionString(raw)` with static `Highs_version()` for version logging
- **Files modified:** roml-highs/src/error.rs, roml-highs/src/lifecycle.rs
- **Committed in:** 25daaef (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (2 Rule 1 bugs)
**Impact on plan:** Both fixes were necessary for compilation against the actual highs-sys 1.15.0 API. No scope creep.

## Issues Encountered

- `cargo clippy -p roml-highs -- -D warnings` fails due to pre-existing unconditional recursion warning in the parent `roml` crate's `src/model/mod.rs` (not in our code). Our crate passes `cargo clippy -p roml-highs` cleanly with only expected dead_code warnings.
- `Highs_getRunStatus` is not available in highs-sys 1.15.0 — the FROM_NATIVE_STATUS helper uses the operation's return code directly instead of querying run status. This simplification is sufficient for the current phase; run status inspection can be added when the solution module is implemented in Plan 03.

## Known Stubs

- `roml-highs/src/projection.rs` - placeholder for Plan 02 (snapshot rebuild / delta apply)
- `roml-highs/src/session.rs` - placeholder for Plan 03 (BackendSession impl)
- `roml-highs/src/solution.rs` - placeholder for Plan 03 (status mapping / extraction)
- `roml-highs/src/callback.rs` - placeholder for Plan 03 (MIP callback bridge)

These are intentional structural stubs declared in lib.rs to prevent module churn when implementation starts. They do not block the current plan's goals (binding authority, fallible lifecycle, thread-safety audit).

## Threat Flags

None — all new surface (network endpoints, auth paths, file access patterns, schema changes at trust boundaries) is within the plan's threat model. The three unsafe blocks in lifecycle.rs (Highs_create, Highs_getSizeofHighsInt, Highs_getInfinity, Highs_destroy) all have SAFETY comments documenting their invariants.

## Self-Check: PASSED

- [x] `cargo check -p roml-highs` passes
- [x] ffi.rs, adapter.rs, integration.rs deleted
- [x] No handwritten extern "C" blocks remain
- [x] No legacy type references (HighsAdapter, SolverAdapter, SolverStatus) in lib.rs
- [x] 3+ SAFETY comments in lifecycle.rs (confirmed: 7)
- [x] No `unsafe impl Sync` in crate
- [x] No panicking `fn new()` — only `fn try_new()` and `fn new_unchecked()`
- [x] lib.rs declares all 8 modules
- [x] 4 stub files exist (projection, session, solution, callback)
- [x] Cargo.toml has highs-sys = "1.15" with bundled/system features
- [x] build.rs is minimal (3 lines)

## Next Phase Readiness

- Foundation layer complete — bindings, error helpers, lifecycle, and module structure are ready.
- Plan 02 can proceed with projection.rs implementation (snapshot rebuild, delta apply).
- Plan 03 can proceed with session.rs, solution.rs, and callback.rs implementation.
- The pub(crate) accessors on HighsSession (raw_ptr, infinity, cursor_mut, etc.) provide the interface needed by projection/session modules.
