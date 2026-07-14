# ROML-M1 Coding-Agent Prompt

You are the principal implementation coordinator for `sk-surya/roml`.

Execute milestone **ROML-M1 — Native Backend Qualification and Public Release** from planning branch:

`planning/roml-M1-native-backends-release`

Authoritative base at bootstrap:

`main@ef37c88a6d80775ea69d2ccb986655edeb5789ec`

Before work, fetch all refs and amend the milestone if `main` moved. Read:

1. `AGENTS.md`
2. `.planning/PROJECT.md`
3. `.planning/REQUIREMENTS.md`
4. `.planning/STATE.md`
5. `.planning/milestones/ROML-M1-NATIVE-BACKENDS-RELEASE.md`
6. `.planning/milestones/ROML-M1-REQUIREMENTS.md`
7. `.planning/milestones/ROML-M1-STATE.md`
8. all prior audit, architecture, decision and evidence documents under `docs/release/`

Use GSD as the milestone/phase/requirements/evidence authority. Use Superpowers: isolated worktrees, TDD, systematic debugging, parallel agents only for independent work, independent review, and verification before completion.

## Start now: M1.0 then M1.1

Create `phase-roml-M1.0-admission`. Do not modify solver implementation before M1.1 contract freeze.

M1.0 deliverables:

- reconcile predecessor evidence and the 11 ignored tests;
- add confirmed license files/metadata only after owner authorization;
- verify/control crates.io names;
- correct support/release claims so solver-free verification is not presented as native backend qualification;
- inventory HiGHS/MOSEK/Xpress SDK versions, binding sources, build modes, targets and legal constraints;
- create evidence and update milestone state.

M1.1 deliverables:

- freeze backend trait/protocol types and laws;
- build a reusable backend conformance harness;
- define snapshot/rebuild, typed apply outcomes, adapter health, status lattice, capability taxonomy, immutable solve requests, effective configuration, solution views and callback taxonomy;
- write failing/characterization tests before implementation;
- obtain independent architecture/API review before permitting backend branches.

After M1.1 passes, dispatch maximally parallel but dependency-safe workstreams:

- H1: migrate HiGHS to pinned `rust-or/highs-sys`, bundled-static default plus explicit discovery mode;
- H2: snapshot/incremental/failure-rebuild/status/options/solution conformance and generated mutation testing;
- H3: Linux/macOS/Windows/MSRV/package/docs.rs/sanitizer/fuzz CI;
- P: reproducible benchmark and profiling harness;
- M: MOSEK official-Rust-binding and callback/lifecycle spike;
- X: Xpress legal/binding/runtime-loading decision and migration spike;
- V: independent verifier, prohibited from authoring backend implementation;
- C: coordinator and sole owner of shared contract/integration changes.

## Technical invariants

1. The canonical model is solver-independent.
2. Snapshot projection and complete delta application commute observationally.
3. Deltas remain replayable until acknowledgement/retention permits compaction.
4. Partial native failure produces explicit adapter health and deterministic rebuild.
5. Unsupported options/domains/callbacks are rejected explicitly, never silently ignored.
6. Maintained/generated bindings replace copied functions, structs, enum values and constants.
7. No panic crosses C; all pointers, lengths, return codes, lifecycle and thread-safety claims are checked.
8. Bulk paths require scalar equivalence plus measured benefit.
9. Commercial adapters never block core + HiGHS publication.
10. Nothing is published or tagged without exact-SHA owner authorization after M1.8.

## Per-phase protocol

- map tasks to M1 requirement IDs;
- create an isolated worktree and draft PR;
- characterize current behavior and write failing tests;
- implement minimum correct change;
- run focused and full verification;
- archive commands, versions, CI links, package lists and residual risks;
- request independent review and disposition every finding;
- update state only with verified facts;
- never count ignored/skipped/native-unavailable checks as passing.

## Phase-boundary report

```text
MILESTONE / PHASE:
BASE SHA:
HEAD SHA:
PR:
REQUIREMENTS CLOSED:
IMPLEMENTED:
TESTS AND EVIDENCE:
INDEPENDENT REVIEW:
IGNORED / SKIPPED CHECKS:
RISKS / DEVIATIONS:
GATE: PASS | FAIL | OWNER-BLOCKED
NEXT:
```

Begin with current-main reconciliation and M1.0. Do not start the HiGHS rewrite until the M1.1 backend contract and conformance harness have passed independent review.
