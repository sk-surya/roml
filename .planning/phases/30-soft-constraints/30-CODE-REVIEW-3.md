---
phase: 30-soft-constraints
reviewed: 2026-08-14T17:05:30Z
depth: deep
files_reviewed: 23
files_reviewed_list:
  - roml-highs/src/compiler.rs
  - roml-highs/src/session.rs
  - roml-highs/src/solution.rs
  - roml-highs/tests/conformance.rs
  - roml-highs/tests/soft_constraints_differential.rs
  - src/compiler/bridge/mod.rs
  - src/compiler/bridge/soft_constraint.rs
  - src/compiler/session.rs
  - src/construct/mod.rs
  - src/construct/soft_constraint.rs
  - src/model/mod.rs
  - src/solution/mod.rs
  - src/solver/facade.rs
  - src/solver/overlay.rs
  - src/solver/relaxation.rs
  - tests/feasibility_relaxation.rs
  - tests/feasibility_relaxation_faults.rs
  - tests/feasibility_relaxation_p29.rs
  - tests/relaxation_provider_policy.rs
  - tests/soft_constraint_solution.rs
  - tests/soft_constraints_algebra.rs
  - tests/soft_constraints_lifecycle.rs
  - tests/soft_constraints_qualification.rs
findings:
  critical: 1
  warning: 1
  info: 0
  total: 2
status: issues_found
---

# Phase 30: Code Review Report

**Reviewed:** 2026-08-14T17:05:30Z  
**Depth:** deep  
**Files Reviewed:** 23  
**Status:** issues_found  
**Verdict:** NOT ACCEPTED — a P1 blocker remains.

## Summary

This is the final independent review of HEAD `b22acb8`, including fixes
`e653e19`, `7144fe5`, `eae2788`, and the current persistent-fixing domain fix.
The prior P0 findings and the previously reported persistent-fixing declared-domain
finding are closed by inspection and focused tests. The real HiGHS differential
and rollback checks also pass. However, a valid model containing an inactive
variable still causes portable feasibility-relaxation synchronization to reject
the backend's candidate as “unknown or inactive”; therefore the phase cannot be
accepted.

The residual documentation/qualification issue is P2. No additional P3 defect
was identified in this pass.

## Narrative Findings (AI reviewer)

## Critical Issues

### CR-01: Inactive compiled variables make valid relaxation candidates fail

**Classification:** P1 (BLOCKER)  
**File:** `src/solver/relaxation.rs:746-753`; cross-module evidence in `roml-highs/src/solution.rs:259-271` and `src/compiler/session.rs:360-387`

**Issue:** `report_members` accepts a candidate only when the corresponding
snapshot variable is both present and `active` (`relaxation.rs:746-750`). But
inactive variables remain in the compiled snapshot and are deliberately compiled
as fixed `[0, 0]` columns (`session.rs:360-387`). The HiGHS solution extractor
maps every finite compiled column back to a user variable without filtering
inactive variables (`solution.rs:259-271`). Consequently, a normal HiGHS solve
of a model with at least one inactive variable supplies a finite candidate for
that inactive variable, and portable feasibility relaxation returns a numerical
error before evaluating the valid active candidates. This violates the model's
valid inactive-variable state and makes the advertised relaxation path fail for
otherwise valid models.

**Fix:** Keep rejecting unknown and duplicate variables, but either filter
inactive variables from backend candidate extraction or accept known inactive
variables in `report_members`; inactive values must not be required candidates
and must not participate in relaxation restrictions. Add a regression test with
one inactive variable and one active variable requiring repair, exercising the
real HiGHS extraction path.

## Warnings

### WR-01: P30 evidence ledger is not pinned to the reviewed HEAD

**Classification:** P2 (WARNING)  
**File:** `docs/release/evidence/P30_SOFT_CONSTRAINTS_RELAXATION.md:3,27-45`

**Issue:** The evidence ledger identifies `e412cb45` as its implementation
candidate, which predates the reviewed fixes through `b22acb8`. Its pass table
and command results therefore do not substantiate exact-HEAD qualification,
especially after the persistent-fixing validation change and this review's
remaining inactive-variable finding.

**Fix:** Rerun the required P30 gate at the final implementation SHA, update the
candidate SHA and command outputs, and leave the qualification result blocked
until CR-01 is fixed and the regression test passes.

## Prior Findings Verification

- Prior P0-01 (persistent softening left the original row hard): closed; the
  current projection widens the softened row's bounds while retaining the
  original relation metadata.
- Prior P0-02 (missing HiGHS capability declaration): closed; the current
  adapter advertises the relevant soft-constraint/portable-relaxation support.
- Prior P0-03 (HiGHS omitted temporary relaxation projection): closed; the
  overlay applies temporary variables, rows, and objective terms, and the real
  HiGHS rollback round trip passes.
- Prior P1 dependency, candidate completeness/objective, cleanup, and
  compilation-identity findings: closed by the current dependency/rebuild,
  validation, cleanup, and report-lineage paths and their focused tests.
- Review 2's P1 persistent-fixing declared-domain finding: closed. Persistent
  fixing now validates against the declared domain rather than relaxing the
  candidate's declared lower/upper bounds; the injected out-of-domain
  persistent-fixing test passes.
- Review 2's P2 findings were rechecked; no additional P3 defect was identified.

## Verification

Passed during this review:

- `cargo test -p roml --test feasibility_relaxation_faults -- --nocapture`
  (7 passed), including the out-of-domain persistent-fixing fault test.
- Focused core soft-constraint and relaxation suites (28 tests passed).
- `cargo test -p roml-highs --test soft_constraints_differential -- --nocapture`
  (3 passed) against real HiGHS 1.15.0.
- Real HiGHS overlay apply/solve/rollback round trip (1 passed).
- `cargo test -p roml-highs --all-targets --quiet && cargo test -p roml --all-targets --quiet`
  (exit 0; all targets passed).

Not independently re-run at the reviewed SHA in this final pass: formatting,
clippy with warnings denied, rustdoc, package-list checks, the quality-policy
script, cross-platform/MSRV/CI gates, and external commercial-solver
qualification. MOSEK and Xpress licensing, SDK, ABI, and runtime gates remain
unverified and are outside the P30 native HiGHS check. The stale evidence ledger
must not be treated as proof for those exact-HEAD gates.

---

_Reviewed: 2026-08-14T17:05:30Z_  
_Reviewer: the agent (gsd-code-reviewer)_  
_Depth: deep_
