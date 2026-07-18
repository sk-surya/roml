---
phase: 9
slug: truth-reset-and-candidate-admission
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-18
---

# Phase 9 — Validation Strategy

> Per-phase validation contract for governance/audit phase (M1R-00: Truth Reset and Candidate Admission).
> This phase produces evidence documents, not code — validation verifies evidence completeness and correctness.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | N/A — governance phase, no code produced |
| **Config file** | N/A |
| **Quick run command** | `echo "Phase 9 is audit-only; no automated tests"` |
| **Full suite command** | `echo "Verification is manual evidence review"` |
| **Estimated runtime** | Manual review only |

---

## Sampling Rate

- **After every task commit:** Review the evidence artifact against TRACEABILITY.md format requirements
- **After every plan wave:** Re-read the admission report for completeness against the phase gate
- **Before gate close:** Independent review of admission report + all evidence artifacts
- **Max feedback latency:** N/A (manual review)

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | What's Verified | Verification Method |
|---------|------|------|-------------|-----------------|---------------------|
| 09-01 | 01 | A | M1R-G1, M1R-G3 | Commit inventory matches git log output | Manual: compare RESEARCH.md inventory against `git log` |
| 09-02 | 01 | A | M1R-G3 | Claim reconciliation table | Manual: cross-reference claim vs source evidence |
| 09-03 | 01 | A | M1R-G2 | Ignored test classification | Manual: each test re-run and result recorded |
| 09-04 | 01 | A | M1R-G3 | License file evidence | Manual: verify LICENSE-MIT and LICENSE-APACHE-2.0 exist |
| 09-05 | 01 | A | M1R-G5 | Crates.io ownership result | Manual: `cargo owner --list` output recorded |
| 09-06 | 02 | A | M1R-G4 | Branch contamination analysis | Manual: verify commit classification |
| 09-07 | 02 | B | M1R-G2 | Test fix verification | Automated: `cargo test` — no ignored tests remain |
| 09-08 | 03 | B | M1R-G1–G5 | Admission report compiled | Manual: review M1R-00-ADMISSION.md against gate criteria |

*All verification is manual except 09-07 (cargo test).*

---

## Wave 0 Requirements

- Evidence directory `docs/release/evidence/M1R/` must be created
- TRACEABILITY.md evidence format verified

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Commit inventory accuracy | M1R-G1 | Git log is the source of truth | Run `git log origin/planning/roml-M1-native-backends-release` and compare |
| Claim reconciliation | M1R-G1 | Requires judgment about implementation completeness | Cross-reference each M1 claim against source files |
| Admission report completeness | M1R-G3 | Gate review is a manual process | Verify all 7 critical work items have evidence entries |

---

## Validation Sign-Off

- [ ] Evidence directory created at `docs/release/evidence/M1R/`
- [ ] M1R-00-ADMISSION.md contains requirement disposition for all 5 M1R-G requirements
- [ ] All 11 ignored tests re-run and disposition recorded
- [ ] License files verified in candidate branch
- [ ] Crates.io ownership verified
- [ ] Branch contamination analysis complete with replay plan
- [ ] Gate criteria: no contradiction between .planning state, source, tests, CI, and known skipped checks
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
