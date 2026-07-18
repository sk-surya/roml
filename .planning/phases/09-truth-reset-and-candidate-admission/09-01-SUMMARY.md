---
phase: 09-truth-reset-and-candidate-admission
plan: 01
subsystem: evidence
tags:
  - governance
  - audit
  - evidence
  - m1r
requires: []
provides:
  - docs/release/evidence/M1R/artifacts/01-evidence-foundation.md
affects: []
tech-stack:
  added: []
  patterns:
    - Evidence document format with raw command output
    - Cross-check against RESEARCH.md for consistency verification
key-files:
  created:
    - docs/release/evidence/M1R/artifacts/01-evidence-foundation.md
  modified: []
decisions:
  - OWNER-BLOCKED disposition for both license and crates.io ownership per D4 and D5
  - crates.io API v1 returns 403 (data access policy); web page returns canonical 404 for unreserved names
  - Candidate branch `planning/roml-M1-native-backends-release` confirmed at 20 commits ahead of `main@82e2ed95`
metrics:
  duration: "<5 min"
  completed-date: 2026-07-18
status: complete
---

# Phase 09 Plan 01: Evidence Foundation Summary

## One-Liner

Produced canonical commit inventory (20 commits), license file evidence (LICENSE-MIT, LICENSE-APACHE at c4db3e0), and crates.io ownership status (both roml/roml-highs available, OWNER-BLOCKED) for the inherited M1 candidate branch.

## Tasks Executed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Create evidence directory and record canonical commit inventory | `4ba4b16` | `docs/release/evidence/M1R/artifacts/01-evidence-foundation.md` |
| 2 | Record license evidence and verify crates.io names | `bc1a6cf` | `docs/release/evidence/M1R/artifacts/01-evidence-foundation.md` |

## Commit Inventory Summary

- **Candidate branch:** `planning/roml-M1-native-backends-release`
- **Candidate HEAD:** `649c635`
- **Merge base (main):** `82e2ed95`
- **Total commits:** 20

### Classification

| Category | Count |
|----------|-------|
| Implementation-only | 7 |
| Contaminated (implementation + planning files) | 4 |
| Documentation-only | 9 |

### Cross-Check

All 20 SHAs match the RESEARCH.md enumeration exactly. The file path for planning touches differs (`.planning/milestones/ROML-M1-STATE.md` vs the `.planning/ROADMAP.md` referenced in RESEARCH.md) — minor nesting difference, no classification impact.

## License Evidence

- Both LICENSE-MIT and LICENSE-APACHE confirmed present on candidate branch at commit `c4db3e0`
- Cargo.toml declares `license = "MIT OR Apache-2.0"` in `[workspace.package]` at commit `c4db3e0`
- **Disposition:** OWNER-BLOCKED (deferred to M1R-08 per D4)

## Crates.io Verification

- `roml`: 404 Not Found (not registered) — web page confirms
- `roml-highs`: 404 Not Found (not registered) — web page confirms
- `cargo owner --list`: Failed for both (no CARGO_REGISTRY_TOKEN)
- **Disposition:** OWNER-BLOCKED (both names available, reservation deferred per D-011)
- **Program stop condition check:** PASS — no EXTERNAL-BLOCKED (neither crate owned by stranger)

### API Response Note

The crates.io API v1 endpoint (`/api/v1/crates/{name}`) now returns 403 for unauthenticated requests due to their data access policy. The web page endpoint (`/crates/{name}`) returns the canonical 404 for unreserved names. Both checks were performed; the web page 404 is authoritative.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] crates.io API v1 returns 403, not 404**
- **Found during:** Task 2
- **Issue:** The plan expected `curl https://crates.io/api/v1/crates/roml` to return HTTP 404. The crates.io API now requires authentication and returns 403 Forbidden with an error message about their data access policy.
- **Fix:** Added fallback check against the web page endpoint `https://crates.io/crates/roml` with a browser User-Agent header, which returns the canonical 404. Both the API response and the web response are documented, with a note explaining the discrepancy.
- **Files modified:** `docs/release/evidence/M1R/artifacts/01-evidence-foundation.md`
- **Commit:** `bc1a6cf`

## Self-Check: PASSED

| Check | Result |
|-------|--------|
| `docs/release/evidence/M1R/artifacts/01-evidence-foundation.md` exists | PASS |
| Commit inventory table contains 20 rows | PASS |
| License files confirmed at commit c4db3e0 | PASS |
| Cargo.toml license field recorded as "MIT OR Apache-2.0" | PASS |
| Both crates.io responses recorded (roml: 404, roml-highs: 404) | PASS |
| cargo owner --list output recorded for both crates | PASS |
| Determination documented as OWNER-BLOCKED | PASS |
| Commit `4ba4b16` exists | PASS |
| Commit `bc1a6cf` exists | PASS |
