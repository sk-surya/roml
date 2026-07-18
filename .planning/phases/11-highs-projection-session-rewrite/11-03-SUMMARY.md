---
phase: 11-highs-projection-session-rewrite
plan: 03
subsystem: highs-backend
tags: ffi, highs, solution-extraction, session, backend-session
requires:
  - phase: 11-02
    provides: projection module, delta application
provides:
  - BackendSession trait implementation for HiGHS
  - Solution extraction and status mapping
  - Solve request negotiation
  - Supplementary traits (SessionHealth, SolutionView, BackendMetadata, CallbackSession)
affects: 11-04
tech-stack:
  added: []
  patterns:
    - map_termination_status: run status before model status (Pitfall 3)
    - negotiate_options: explicit option application/rejection per field
    - Session-wide solution invalidation after model mutation (T-11-18)
key-files:
  created:
    - roml-highs/src/solution.rs
    - roml-highs/src/session.rs
  modified:
    - roml-highs/src/lifecycle.rs
key-decisions:
  - "Highs_getRunStatus not available in highs-sys 1.15.0 — pass run_status parameter directly from Highs_run() return value"
  - "Highs_getSolution uses 5-param signature (col_value, col_dual, row_value, row_dual) not 3-param as originally planned"
  - "Version metadata uses Highs_version(), Highs_versionMajor/Minor/Patch instead of Highs_getHighsVersion* family"
  - "Callback handler consumed per solve; user must call set_callback_handler before each solve needing callbacks"
requirements-completed: [M1R-H3, M1R-H5, M1R-H6, M1R-H8]
status: complete
---

# Phase 11 Plan 03: HiGHS Solution Extraction, Session, and Solve Negotiation

**BackendSession trait fully implemented for HiGHS with solution extraction, status mapping (15 model statuses + run status), solve request negotiation, and four supplementary traits**

## Performance

- **Duration:** ~45 min
- **Started:** 2026-07-18
- **Completed:** 2026-07-18
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments

- solution.rs: `map_termination_status` checks run status BEFORE model status (Pitfall 3), maps all 15 HiGHS model statuses per AD-6. `InfeasibleOrUnbounded` preserved (H6). Feasible incumbent check for `OBJECTIVE_BOUND`/`OBJECTIVE_TARGET` MIP outcomes.
- solution.rs: `extract_solution` extracts variable_values, reduced_costs, dual_values, objective_value from HiGHS using col_map/row_map reverse maps. Buffer overrun protection via `Highs_getNumCol` validation (T-11-16). `CString::new` error handling (T-11-15).
- session.rs: `BackendSession::synchronize` handles Rebuild and DeltaBatch, invalidates cached solution after model mutation (T-11-18).
- session.rs: `BackendSession::solve` negotiates options, registers callbacks, runs `Highs_run`, maps status, extracts solution, cleans up callback state. Fatal run errors (negative return) return error with cleanup.
- session.rs: `SessionHealth`, `SolutionView`, `BackendMetadata`, `CallbackSession` all implemented.
- session.rs: `negotiate_options` maps every `SolveRequest` field explicitly — algorithm, time limit, MIP gaps, threads, output, random_seed, extra_options. Core options return errors; extra options collect rejections (M1R-H3, M1R-H5).
- lifecycle.rs: Added version fields (version_string, version_major/minor/patch), solution state (current_solution, last_status), callback state (callback_state, callback_handler). Drop impl now cleans up callback state before `Highs_destroy`.

## Task Commits

Each task was committed atomically:

1. **Task 1: Solution extraction and status mapping** - `0ab52f2` (feat)
2. **Tasks 2+3: Session implementation and solve negotiation** - `f5210fa` (feat)

## Files Created/Modified

- `roml-highs/src/solution.rs` — Created: status mapping (map_termination_status), solution extraction (extract_solution), feasible check (has_feasible_solution)
- `roml-highs/src/session.rs` — Created: BackendSession impl, SessionHealth, SolutionView, BackendMetadata, CallbackSession, negotiate_options, set_option
- `roml-highs/src/lifecycle.rs` — Modified: struct fields (pub(crate) + new fields), Drop impl (callback cleanup), try_new (version caching)

## Deviations from Plan

### Auto-fixed Issues (Rule 2 — API Differences)

**1. [Rule 2 - API Not Available] Highs_getRunStatus missing from highs-sys 1.15.0**
- **Found during:** Task 1 (solution.rs)
- **Issue:** The plan specified calling `Highs_getRunStatus` inside `map_termination_status`, but this C API function is not exposed in highs-sys 1.15.0.
- **Fix:** `map_termination_status` accepts a `run_status: HighsInt` parameter derived from the `Highs_run()` return code. The caller in session.rs checks for negative return codes and returns `BackendError` before calling `map_termination_status`. Within `map_termination_status`, warning status codes (1) are logged and processing continues.
- **Files modified:** solution.rs, session.rs
- **Committed in:** 0ab52f2, f5210fa

**2. [Rule 2 - API Signature Differs] Highs_getSolution has 5 parameters, not 3**
- **Found during:** Task 1 (solution.rs)
- **Issue:** The plan assumed `Highs_getSolution(raw, primal_ptr, dual_ptr, ptr::null_mut())` (3 non-handle params). Actual signature is `Highs_getSolution(raw, col_value, col_dual, row_value, row_dual)` (4 output arrays).
- **Fix:** Updated calls to pass all 5 parameters. Mapped: `col_value` -> variable_values, `col_dual` -> reduced_costs, `row_dual` -> dual_values.
- **Files modified:** solution.rs
- **Committed in:** 0ab52f2

**3. [Rule 2 - API Not Available] Highs_getHighsVersion* family not available**
- **Found during:** Task 2 (lifecycle.rs)
- **Issue:** Plan specified `Highs_getHighsVersionString`, `Highs_getHighsVersion`, `Highs_getHighsCompilationDate`. Not available in highs-sys 1.15.0.
- **Fix:** Use `Highs_version()` (returns `*const c_char`), `Highs_versionMajor()`, `Highs_versionMinor()`, `Highs_versionPatch()` (static functions, no handle needed). Compilation date skipped.
- **Files modified:** lifecycle.rs
- **Committed in:** f5210fa

**4. [Rule 2 - API Usage Corrected] Highs_setOptionValue takes 3 args**
- **Found during:** Task 3 (session.rs, negotiation)
- **Issue:** The plan described constructing a combined "key=value" string for `Highs_setOptionValue`. The actual API takes separate `option` and `value` c_string parameters (same signature as `Highs_setStringOptionValue`).
- **Fix:** Both `Highs_setStringOptionValue` and `Highs_setOptionValue` are called with separate key/value. The fallback tries the other variant in case the option parser differs.
- **Files modified:** session.rs
- **Committed in:** f5210fa

---

**Total deviations:** 4 auto-fixed (all Rule 2 — alignment with actual highs-sys API)
**Impact on plan:** No scope creep. All deviations were adjustments to use the correct API signatures and names as documented in the highs-sys bindings. The behavioral intent (Pitfall 3 run status check, version caching, option setting) is preserved.

## Issues Encountered

- None during execution. The API differences were identified during reading and automatically handled.

## Known Stubs

- `version_major`, `version_minor`, `version_patch` fields in `HighsSession` are stored but not exposed through `BackendMetadata::name()` (which returns `version_string`). These fields are available for future structured version queries.
- `raw_ptr()` and `infinity()` methods on `HighsSession` are no longer called since session.rs accesses struct fields directly via `pub(crate)` visibility. Retained for API compatibility.
- `CallbackHandler` is consumed per solve (moved into `register_callback`, dropped by `clear_callback`). User must call `set_callback_handler` before each solve that needs callbacks.

## Threat Surface Scan

No new security-relevant surface introduced beyond what the plan's threat model covered. All T-11-14 through T-11-19 mitigations are implemented. No new network endpoints, auth paths, file access patterns, or schema changes at trust boundaries.

## Next Phase Readiness

- Plan 04 (test implementation and UAT) can proceed: all session traits are implemented against the frozen contract
- Solution extraction and status mapping are ready for integration testing
- Version metadata caching enables capability-based feature detection in downstream consumers

---
*Phase: 11-highs-projection-session-rewrite*
*Completed: 2026-07-18*
