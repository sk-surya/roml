---
phase: 30-soft-constraints
reviewed: 2026-08-14T17:16:05Z
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

**Reviewed:** 2026-08-14T17:16:05Z  
**Depth:** deep  
**Files Reviewed:** 23  
**Status:** issues_found  
**Verdict:** NOT ACCEPTED — one P1 blocker remains.

## Summary

Reviewed the P30 plans/context, review 3, prior P0/P1 dispositions, HEAD
`2379ed6269a716f864040288990c0815d76f2123`, and the updated evidence ledger.
The ordinary CR-01 failure is fixed: the new
`portable_repair_ignores_inactive_compiled_candidate` regression passes, as do
the focused fault suite (7 tests), real HiGHS differential/rollback suite (3
tests), full `roml` targets (293 library tests), full `roml-highs` targets,
formatting, denied-warning clippy, rustdoc, package-list, quality-policy, and
diff checks.

The fix is not complete under the existing malformed-provider validation
contract. `report_members` records an inactive candidate in
`candidate_values` before continuing. Later active-constraint validation reads
that map for every coefficient, so a nonzero candidate for an inactive
variable can change the recomputed active-constraint expression and produce
incorrect relaxation evidence. The new regression does not cover this because
the inactive variable is absent from the active constraint and the deterministic
reference backend returns zero for it.

The evidence ledger is now content-pinned to the exact current HEAD
`2379ed6269a716f864040288990c0815d76f2123`, and the recorded commands were
independently rerun successfully. The ledger edit is still a working-tree
change; if it is committed at a new SHA, its exact-head line and command record
must be re-pinned as part of that commit.

## Critical Issues

### CR-01: Inactive candidate values still affect active-constraint evidence

**Classification:** P1 (BLOCKER)  
**File:** `src/solver/relaxation.rs:755-761, 842-865`

**Issue:** The new code correctly accepts a known inactive variable and skips
its direct domain/restriction checks, but it inserts the candidate into
`candidate_values` first. The active-constraint loop then fetches that value
for every coefficient, including coefficients belonging to inactive variables.
For example, an active relaxable row `z + x >= 2` with inactive `z` and active
`x` must evaluate `z` as the compiled fixed value `0`. A malformed provider
candidate `z = 2, x = 0` is currently treated as satisfying the row, even
though the compiled overlay has `z` fixed to `[0, 0]`; the report can therefore
claim zero/too-small repair violation and an accepted outcome. This violates
the requirement that inactive values not participate in relaxation evidence.

**Fix:** Keep separate duplicate tracking for all provider entries, but do not
put inactive candidates into the map used for model evaluation. When evaluating
constraint cells, skip inactive variables or substitute their canonical fixed
value `0.0`. Add a regression with an inactive variable appearing in an active
relaxed constraint and an injected nonzero inactive candidate; require a typed
numerical error or evidence identical to treating that inactive variable as
zero.

## Warnings

### WR-01: The new regression does not exercise the real HiGHS extraction path

**Classification:** P2 (WARNING)  
**File:** `tests/feasibility_relaxation.rs:244-271`

**Issue:** The regression uses the synthetic `ReferenceSolveSession`, and its
inactive variable has no coefficient in the active constraint. The existing
real HiGHS suite still passes, but it does not include this new inactive-variable
fixture. Thus the exact HiGHS extraction behavior that caused CR-01 is covered
by the old real-backend tests only indirectly.

**Fix:** Add the inactive variable to a real HiGHS repair fixture (ideally as a
zero-fixed coefficient in an active relaxed row), or add a focused adapter-level
assertion that inactive compiled columns are either filtered or mapped to the
canonical inactive value before report validation.

## Prior Findings Verification

- Prior P0-01 (persistent softening left the original row hard): **closed**;
  softened row bounds are widened before generated side rows are applied.
- Prior P0-02 (missing HiGHS capability declaration): **closed**; the adapter
  advertises the bridge capabilities and the capability qualification passes.
- Prior P0-03 (HiGHS omitted temporary relaxation projection): **closed**; the
  real HiGHS overlay applies temporary variables/rows/objective and rolls back
  successfully twice.
- Prior P1 dependency, candidate objective/completeness, cleanup/rebuild, and
  exact compilation-identity findings: **closed by inspection and passing
  focused/full tests**.
- Review 2's P1 persistent-fixing declared-domain finding: **closed**; declared
  domain validation is active and the injected out-of-domain candidate test
  passes.
- Review 3's CR-01 ordinary inactive-column failure: **partially closed**;
  normal inactive candidates no longer invalidate an otherwise valid repair,
  but the remaining map/evaluation defect above prevents acceptance.

## Evidence and Residual Gates

- Evidence SHA: **verified** — the working-tree ledger names exactly
  `2379ed6269a716f864040288990c0815d76f2123`, matching `git rev-parse HEAD`.
- Exact-head local gates: **pass** — focused regression/fault/HiGHS suites,
  full core and HiGHS targets, fmt, clippy with warnings denied, rustdoc,
  package lists, quality policy, and `git diff --check`.
- Residual P2/external gates: the new regression still needs real-HiGHS
  coverage; MOSEK/Xpress SDK, ABI, license/runtime, native-relaxation, and
  cross-platform/MSRV qualification remain external or deferred gates. No
  crate publication, tag, or release action was performed.

---

_Reviewed: 2026-08-14T17:16:05Z_  
_Reviewer: the agent (gsd-code-reviewer)_  
_Depth: deep_
