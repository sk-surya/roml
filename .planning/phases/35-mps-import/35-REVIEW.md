---
phase: 35-mps-import
reviewed: 2026-08-08T05:51:12Z
depth: deep
files_reviewed: 4
files_reviewed_list:
  - src/io/mod.rs
  - src/io/mps/mod.rs
  - src/lib.rs
  - tests/mps_module_seam.rs
findings:
  critical: 1
  warning: 2
  info: 0
  total: 3
status: issues_found
---

# Phase 35: Code Review Report

**Reviewed:** 2026-08-08T05:51:12Z  
**Depth:** deep  
**Files Reviewed:** 4  
**Status:** issues_found  
**Scope:** Task 35-00 only, `origin/main@3bd0319518c27127a30bc53878f776e82f1ad822..62c3358`

## Summary

The new module is solver-free and correctly keeps file-format entry points outside `Model`. However, the public seam freezes an error/diagnostic representation that cannot satisfy the approved source-context contract, and the required malformed-input test is a self-fulfilling closure test rather than an importer regression harness. This task is **BLOCKED** until the P1 is fixed.

## Narrative Findings (AI reviewer)

## Critical Issues

### CR-01: The public error contract cannot preserve or render required source context

**Severity:** P1 — **BLOCKER**

**File:** `src/io/mps/mod.rs:135-157`, `src/io/mps/mod.rs:162-240`

**Issue:** The approved design requires errors to carry path, line/span, section, raw-field, and entity context where available. `MpsDiagnostic` only represents a span and free-form section; it has no path, raw field, or entity. `MpsErrorKind::Io` is a unit variant, and `MpsError` neither retains the underlying I/O failure nor exposes it through `Error::source`. Finally, `Display` prints only `Debug` for `kind`, discarding even the available line/section context. Consequently a future `read_path` failure and malformed-record failure cannot provide the required actionable typed diagnostics through the frozen public API.

**Fix:** Define an extensible diagnostic context before parser work lands, with accessors for input path/source label, span, typed section, raw field, and referenced entity. Preserve an I/O cause (or at least `io::ErrorKind` plus message) and implement `Error::source` when it is retained. Make `Display` include available location/section context. For example, keep fields private and add builder-style setters so stream reads can omit a path while `read_path` supplies one:

```rust
pub struct MpsDiagnostic {
    source: Option<MpsInputSource>,
    span: Option<MpsSourceSpan>,
    section: Option<MpsSection>,
    raw_field: Option<String>,
    entity: Option<String>,
}

// MpsError must retain and expose the I/O/model cause when applicable.
```

Add contract tests for a path-backed I/O error and a malformed field/entity error, asserting that all available context is accessible and appears in the rendered error.

## Warnings

### WR-01: `MpsSourceSpan::new` permits impossible and ambiguous locations

**Severity:** P2 — **WARNING**

**File:** `src/io/mps/mod.rs:105-132`

**Issue:** This public constructor accepts line zero and spans where `end < start`, and its documentation does not define whether line and offsets are zero- or one-based or whether `end` is inclusive. Those invalid/ambiguous locations can be stored in errors and source maps, undermining the exact provenance promised to later resolution/IIS tasks.

**Fix:** Document one coordinate convention (for example, one-based line plus zero-based half-open byte columns) and make invalid ranges unrepresentable: use a checked constructor returning a typed error, or make construction crate-private and expose validated parser-created spans. Add boundary tests for the first byte, empty span policy, and reversed ranges.

### WR-02: The malformed-input “harness” never invokes importer code

**Severity:** P2 — **WARNING**

**File:** `tests/mps_module_seam.rs:74-92`

**Issue:** The test passes a closure that is written to return `Err` and then asserts that this closure returned `Err`. It does not call `MpsReader` or any lexer/staging entry point, so it will stay green if a later parser panics on NUL bytes, invalid UTF-8, oversized records, or any other malformed input. The helper is also private to this integration-test crate, so subsequent lexer/staging tests cannot reuse it as the Task 35-00 synthetic fixture harness.

**Fix:** Put a reusable helper under `tests/common/` and, once the read entry point exists, execute `MpsReader::read(BufReader::new(input))` inside `catch_unwind`, asserting `Ok(Err(_))` for malformed fixtures. Add representative binary/NUL, invalid-number, bad fixed-column, truncated-section, and limit-exceeded cases as the relevant stages are implemented.

---

_Reviewed: 2026-08-08T05:51:12Z_  
_Reviewer: the agent (gsd-code-reviewer)_  
_Depth: deep_
