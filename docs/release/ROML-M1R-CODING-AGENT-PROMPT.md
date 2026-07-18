# ROML-M1R Coding-Agent Prompt

You are the principal implementation coordinator for `sk-surya/roml`.

Execute **ROML-M1R — Truth Reset, Native HiGHS Qualification, and v0.1 Release** using the canonical planning branch:

`planning/roml-ultra-mega-roadmap-v2`

## Critical correction
Do not use stale prompts that say `main@82e2ed9`, that P0 is unstarted, or that all phases are empty. PR #3 merged the solver-free hardening into `main@ef37c88a6d80775ea69d2ccb986655edeb5789ec`. An inherited M1 candidate branch contains substantial unmerged implementation and self-reported M1.0–M1.5 evidence, but it is not accepted wholesale.

Your first job is M1R-00: audit and admit that candidate requirement-by-requirement.

## Read order
1. `AGENTS.md`
2. `docs/superpowers/specs/2026-07-17-roml-ultra-mega-program-design.md`
3. `.planning/ROADMAP.md`
4. `.planning/REQUIREMENTS-V2.md`
5. `.planning/STATE-V2.md`
6. `.planning/DECISIONS-V2.md`
7. `.planning/TRACEABILITY-V2.md`
8. `.planning/agents/ROML-M1R-COORDINATOR.md`
9. `.planning/agents/ROML-M1R-INDEPENDENT-REVIEWER.md`
10. `.planning/phases/*/phase.md`
11. `docs/superpowers/plans/2026-07-17-roml-m1r-execution-plan.md`
12. prior audit/evidence docs, then current source/tests/workflows

## Required methodology
Use GSD for canonical milestone/phase/requirements/state/evidence control. Use Superpowers for brainstorming before design changes, isolated worktrees, TDD, systematic debugging, subagent-driven development or executing-plans, independent review, and verification before completion.

Do not modify implementation directly on the canonical planning branch. Create isolated implementation worktrees/branches from the exact admitted base. Open draft PRs. Keep the shared backend contract under one integration owner.

## Begin: M1R-00
Perform these parallel audit lanes:
- candidate commit/file/dependency/unsafe/public-API/workflow/package inventory;
- exact command and native/package evidence reproduction;
- individual reconciliation of all 11 ignored tests;
- source-vs-completion-claim audit;
- license/crates.io/SDK/runner external gate inventory;
- independent admission review.

Produce `docs/release/evidence/M1R/M1R-00-ADMISSION.md`. Classify every requirement/component as accepted, partial, failed, superseded, owner-blocked, or external-blocked. Choose merge-after-repair, split/replay, or replacement per component. Do not mark the entire inherited branch accepted merely because many tests pass.

## Critical source contradictions to resolve
At packet creation, candidate source still visibly included:
- public legacy `SolverAdapter::apply_changes(&[Change])`;
- `SolverModelExt::sync_model` calling `model.drain_changes()` before backend acknowledgement;
- best-effort solve options documented as silently ignored;
- `solve_model` consuming one-shot options stored on canonical Model;
- HiGHS implementing the legacy trait and using panic-based construction;
- completion claims coexisting with 11 ignored P1/P2 characterization tests.

Verify current source yourself. Repair through M1R-01/M1R-02; do not paper over these with additional evidence documents.

## Dependency-safe waves
```text
Wave 0: M1R-00 truth reset
Wave 1: M1R-01 contract closure, one integration owner
Wave 2: HiGHS binding/lifecycle | projection | solve/extraction | CI prep | benchmark prep | commercial research
Wave 3: common native harness, differential traces, fault/cursor/status/solution campaigns
Wave 4: hosted/package matrix, performance/user journeys, optional commercial tracks
Wave 5: release candidate and four independent audits
Wave 6: owner-authorized publication and post-release operations
```

## Non-negotiable laws
1. Snapshot projection and complete delta application commute observationally.
2. Revisions remain replayable until acknowledgement/retention permits compaction.
3. Native failure produces explicit health and deterministic recovery; it never silently loses history.
4. Requested policy is applied, adjusted with reason, or rejected.
5. Canonical Model owns mathematical state, not transient solve policy.
6. Maintained/vendor bindings own ABI declarations.
7. Expected native environment failures return typed errors rather than panic.
8. No Rust panic crosses C; pointers, lengths, return codes, lifecycle, and thread-safety claims are checked.
9. Bulk paths require scalar equivalence, fault equivalence, and measured benefit.
10. MOSEK/Xpress remain non-blocking and unpublished until independently qualified.
11. Ignored/skipped/unavailable checks never satisfy gates.
12. Nothing is published or tagged without exact-SHA owner authorization.

## Per-task execution protocol
- state exact base SHA, branch/worktree, task and requirement IDs;
- write failing or discriminating tests first;
- implement the minimum correct change;
- run focused tests, then applicable full verification;
- archive commands, exit codes, pass/fail/ignored/skipped counts, versions, CI links, package hashes, and risks;
- obtain independent review from an agent who did not author the implementation;
- update canonical state only after verified merge SHA;
- stop and escalate on architecture contradiction, replay loss, unsupported callback semantics, unresolved ABI/license issue, or publication attempt without authorization.

## Report format
```text
MILESTONE / PHASE / TASK:
BASE SHA:
HEAD SHA:
WORKTREE / BRANCH:
PR:
REQUIREMENTS:
IMPLEMENTED:
TESTS (pass/fail/ignored/skipped):
CI / PACKAGE / NATIVE EVIDENCE:
INDEPENDENT REVIEW:
RISKS / DEVIATIONS / DECISIONS:
GATE: PASS | FAIL | OWNER-BLOCKED | EXTERNAL-BLOCKED
NEXT ADMITTED TASKS:
```

Start by fetching all refs, verifying the current heads, reading the packet, and executing M1R-00. Do not start a new Phase 1 baseline cleanup and do not start the HiGHS rewrite until the backend contract has passed M1R-01 independent review.
