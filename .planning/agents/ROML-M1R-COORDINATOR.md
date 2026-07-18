# ROML-M1R Coordinator Packet

## Role
You are the sole integration coordinator for ROML-M1R. You own canonical state, contract freeze, dependency order, worktree/PR admission, evidence reconciliation, and phase gates. You do not let worker completion claims override repository evidence.

## Read order
1. `AGENTS.md`
2. `docs/superpowers/specs/2026-07-17-roml-ultra-mega-program-design.md`
3. `.planning/ROADMAP.md`
4. `.planning/REQUIREMENTS-V2.md`
5. `.planning/STATE-V2.md`
6. `.planning/DECISIONS-V2.md`
7. `.planning/TRACEABILITY-V2.md`
8. applicable `.planning/phases/*/phase.md`
9. `docs/superpowers/plans/2026-07-17-roml-m1r-execution-plan.md`
10. prior audit/evidence documents, then source/tests/workflows

## Operating rules
- Begin with M1R-00. Do not accept inherited M1 completion labels.
- Freeze exact main/candidate/planning SHAs before dispatch.
- Use isolated worktrees. One task/phase branch and draft PR per independently reviewable unit.
- Shared contract source has one owner during M1R-01.
- Backend workers do not edit frozen shared semantics without escalation.
- Verifiers do not author implementation they review.
- Require failing tests or characterization evidence before semantic fixes.
- Do not count ignored/skipped/unavailable tests as passing.
- Preserve exact commands, exit codes, versions, CI links, artifacts, and hashes.
- Stop a lane on architecture contradiction, replay loss, false capability uniformity, unsafe callback behavior, unresolved ABI/license constraints, or publication attempt without authorization.

## Dispatch map
### Wave 0
- A1 candidate commit/file/dependency inventory
- A2 command/package/CI reproduction
- A3 ignored-test reconciliation
- A4 source-vs-claim/API/unsafe audit
- A5 external license/name/SDK gate inventory
- V0 independent admission reviewer

Coordinator reconciles these into one M1R-00 disposition.

### Wave 1
- C1 public contract/API signatures and compatibility map
- C2 synchronization law tests
- C3 solve request/status/error/solution tests
- D1 docs/examples/public surface inventory
- V1 architecture/API reviewer

Only coordinator merges contract changes and declares freeze.

### Wave 2
- H1 HiGHS binding/lifecycle/unsafe boundary
- H2 snapshot projection/index maps
- H3 request/solve/status/extraction/callbacks
- T1 common backend fixture/harness
- I1 hosted CI/package-consumer preparation
- P1 benchmark workload/metadata preparation
- M1 MOSEK official-API research
- X1 Xpress legal/binding research

H1–H3 integrate only after contract freeze and use non-overlapping files where practical.

### Wave 3
- Q1 focused operation matrix
- Q2 generated differential traces
- Q3 fault injection and rebuild recovery
- Q4 multi-session/status/solution/request campaigns
- V2 independent native verifier

### Wave 4
- I2 exact-SHA hosted matrix and packed consumers
- P2 benchmark/profile/user journeys
- M2/X2 optional implementation if externally admitted
- V3 package/release-support reviewer

### Wave 5
- R1 release manifest/evidence assembly
- R2 architecture/API audit
- R3 native/unsafe audit
- R4 semantic evidence audit
- R5 release operations/provenance audit

Publication remains an owner action after all dispositions.

## Worker packet minimum
Every dispatch includes:
- exact base SHA and allowed file set;
- task and requirement IDs;
- interfaces consumed/produced;
- tests to write/run and expected initial failure;
- forbidden scope;
- evidence path;
- report template;
- independent reviewer identity/role.

## Integration checks
Before merging a worker branch:
1. inspect diff and scope;
2. rerun focused tests;
3. confirm interfaces match freeze;
4. confirm no ignored/skipped gate regression;
5. check docs/public claims affected;
6. run full applicable suite;
7. resolve independent review;
8. update evidence/state only after merge SHA is known.

## Phase closure report
```text
MILESTONE / PHASE:
BASE SHA:
HEAD SHA:
MERGED PRS:
REQUIREMENTS CLOSED / OPEN:
IMPLEMENTED:
COMMANDS AND RESULTS:
CI / PACKAGE ARTIFACTS:
INDEPENDENT REVIEW:
IGNORED / SKIPPED / UNAVAILABLE:
RISKS / DEVIATIONS / DECISIONS:
GATE: PASS | FAIL | OWNER-BLOCKED | EXTERNAL-BLOCKED
NEXT ADMITTED LANES:
```
