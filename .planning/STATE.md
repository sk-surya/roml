---
gsd_state_version: 1.0
milestone: v0.1
milestone_name: milestone
current_phase: M1R-03 — Native differential and fault qualification
status: in_progress
stopped_at: Completed 12-02-PLAN.md
last_updated: "2026-07-18T15:11:47.000Z"
progress:
  total_phases: 9
  completed_phases: 2
  total_plans: 12
  completed_plans: 12
  percent: 25
---

# ROML Program State v2

**Canonical planning branch:** `planning/roml-ultra-mega-roadmap-v2`  
**Merged implementation baseline:** `main@ef37c88a6d80775ea69d2ccb986655edeb5789ec`  
**Inherited candidate:** `planning/roml-M1-native-backends-release`  
**Current milestone:** ROML-M1R — Truth Reset, Native HiGHS Qualification, and v0.1 Release
**Current phase:** M1R-02 — HiGHS projection/session rewrite

## State vocabulary

- **Merged:** present on `main`.
- **Candidate:** implemented on an unmerged branch.
- **Locally verified:** commands passed on a named machine/commit.
- **CI verified:** required hosted/protected jobs passed for the exact commit.
- **Accepted:** requirement evidence and independent review passed.
- **Owner-blocked:** technically ready but awaiting an explicit owner decision.
- **External-blocked:** legal/vendor/infrastructure condition is unresolved.
- **Released:** published artifact and tag verified against exact evidence.

## Established predecessor

PR #3 merged solver-free hardening: canonical cells, validation/invariants, revision/snapshot/delta foundations, reference synchronization, contract characterization, solver-free CI, package hygiene, and release documentation. This does not establish native backend correctness.

## Candidate facts requiring admission

The inherited candidate is 20 commits ahead of main and contains licenses, `highs-sys` migration, HiGHS CI, differential/recovery/status tests, benchmarks, and commercial qualification plans. Its M1 state ledger marks M1.0–M1.5 complete.

The candidate source still visibly exposes the legacy `SolverAdapter` path, destructive `drain_changes()` synchronization, best-effort silently ignored options, model-owned one-shot solve options, and a HiGHS implementation of the legacy trait with panic-based construction. These are M1R-00 truth-reset findings, not accepted completion.

## Phase ledger

| Phase | State | Admission/exit evidence |
|---|---|---|
| M1R-00 Truth reset | Complete | M1R-00-ADMISSION.md produced, 5/5 M1R-G requirements dispositioned. Gate: PASS (G5 OWNER-BLOCKED deferred to M1R-08). All 11 ignored tests resolved. Base SHA frozen.
| M1R-01 Contract closure | Complete | Gate: PASS. Freeze SHA: bf3ba70. ADR-001. |
| M1R-02 HiGHS rewrite | In progress | 11-01 foundation complete: highs-sys bindings, fallible lifecycle, thread-safety audit, module scaffold |
| M1R-03 Native qualification | Blocked by M1R-02 | common conformance, differential traces, fault recovery |
| M1R-04 Platform/package | Infrastructure may prepare; gate blocked by M1R-03 | clean matrix and packed consumers |
| M1R-05 Performance/UX | Harness may prepare; acceptance blocked by M1R-03 | reproducible evidence and user journeys |
| M1R-06 MOSEK | Non-blocking, external/license gated | official API, legal callbacks, protected CI |
| M1R-07 Xpress | Non-blocking, legal gated | binding decision, lifecycle, protected CI |
| M1R-08 Release | Blocked | exact-SHA evidence and owner authorization |
| M1R-09 Operations | Blocked by release | patch/security/compatibility machinery active |

## Owner decisions

1. Dual license appears committed in the candidate; record explicit owner authorization in M1R-00 evidence.
2. Verify/control crates.io names `roml` and `roml-highs`.
3. Approve publication only after M1R-08 evidence.
4. Approve protected commercial-solver runners only if M1R-06/07 execution is desired.

## Immediate sequence

1. Audit every inherited candidate commit and changed file.
2. Re-run and classify all required commands at the exact candidate head.
3. Reconcile the 11 ignored tests individually.
4. Produce requirement disposition and integration/replay plan.
5. Freeze the public backend contract and remove legacy semantic contradictions.
6. Only then admit HiGHS implementation work.

## Non-negotiables

- Candidate completion labels are not inherited as facts.
- No ignored test closes a requirement.
- Commercial backends do not block core + HiGHS.
- Planning state changes only with exact evidence.
- No publication or tag without owner authorization.

## Session

**Last session:** 2026-07-18T15:11:47.000Z
**Stopped at:** Completed 12-02-PLAN.md
**Resume file:** None

## Performance Metrics

| Plan | Duration | Tasks | Files |
|------|----------|-------|-------|
| Phase 09 P04 | ~15 min | 4 tasks | 3 files |
| Phase 09 P05 | ~5 min | 2 tasks | 2 files |
| Phase 10 P01 | ~30 min | 2 tasks | 13 files |
| Phase 10 P02 | ~4 min | 3 tasks | 7 files |
| Phase 10 P03 | ~10 min | 2 tasks | 8 files |
| Phase 11 P01 | ~5 min | 3 tasks | 16 files |
| Phase 11-highs-projection-session-rewrite P02 | 12m | 3 tasks | 2 files |
| Phase 11-highs-projection-session-rewrite P03 | 45m | 3 tasks | 3 files |
| Phase 12 P01 | ~31 min | 3 tasks | 8 files |
| Phase 11-highs-projection-session-rewrite P03 | 45m | 3 tasks | 3 files |
| Phase 12 P01 | ~31 min | 3 tasks | 8 files |
| Phase 12 P02 | ~16 min | 3 tasks | 2 files |

## M1R Base

**Frozen at:** M1R-00 admission
**Date:** 2026-07-18
**Main SHA:** ef37c88a6d80775ea69d2ccb986655edeb5789ec
**Candidate SHA:** 649c635974cae1d6716bbc19429833a0135df22f
**Planning branch SHA:** a372df227d0e6988ea56f653808931696e86433c
**Evidence:** docs/release/evidence/M1R/M1R-00-ADMISSION.md
**Test fix branch:** fix/m1r-00-ignored-tests@629ccd3ba5ec06b1569f8320a2a803e6325223eb
**Phase ledger for M1R-00:** evidence complete -- see M1R-00-ADMISSION.md for requirement disposition

## Decisions

- [Phase ?]: Highs_getRunStatus not available in highs-sys 1.15.0 — pass run_status parameter directly from Highs_run() return value
- [Phase ?]: Highs_getSolution uses 5-param signature (col_value, col_dual, row_value, row_dual) not 3-param as originally planned
- [Phase ?]: Version metadata uses Highs_version(), Highs_versionMajor/Minor/Patch instead of Highs_getHighsVersion* family
- [Phase ?]: Callback handler consumed per solve; user must call set_callback_handler before each solve needing callbacks
