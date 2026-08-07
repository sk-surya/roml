---
phase: 29-iis-conflict-analysis
status: implementation-slices-complete-with-review-blockers
branch: phase-roml-P29-iis-conflicts
base: d26728e
---

# Phase 29 execution summary

## Delivered

- Frozen LP infeasibility contract and tri-state oracle vocabulary.
- Plan-driven `SolverSession::analyze_infeasibility` orchestration using an
  isolated session and exact `CompilationId`.
- Semantic side-level restriction universe with persistent-fixing layer
  restoration and deterministic atom identity.
- ROML adaptive chunk reduction, single-atom polish, fresh final/member
  verification, call/iteration/time budgets, and guarantee downgrades for
  unknown or limited checks.
- Canonical structured report plus deterministic text and Markdown renderers.
- Explicit `OriginalLp` and `LpRelaxation` scope handling; relaxation columns
  are converted to continuous only in the isolated analysis snapshot.
- Audited bundled HiGHS 1.15.0 native IIS seed provider in
  `roml-highs/src/iis.rs`, using generated `highs-sys` bindings only.
- Correctness, mutation, differential, planted-IIS, persistent-fixing,
  native-seed, isolation, and LP-relaxation regression tests.
- HiGHS API audit and release evidence documents.

## Decisions encoded

The owner-approved packet decisions are encoded in `29-CONTEXT.md`,
`29-DISCUSSION-LOG.md`, and `29-PLAN.md`: ROML is the semantic authority;
native IIS is evidence/seed only; Auto prefers qualified native then reduces
and verifies; no minimum-cardinality claim is representable; Unknown is never
infeasible; feasibility relaxation and MIP-only infeasibility remain outside
this phase; and the persistent solve session is not used for probing.

## Verification evidence

- `cargo fmt --all -- --check`: passed.
- `cargo clippy -p roml --all-targets -- -D warnings`: passed.
- `cargo clippy -p roml-highs --all-targets -- -D warnings`: passed.
- `cargo test -p roml --all-targets`: passed (243 library tests plus all
  integration targets).
- `cargo test -p roml-highs --all-targets`: passed (bundled HiGHS 1.15.0).
- `cargo test -p roml-highs --test iis`: passed (4 tests).
- `cargo bench -p roml --bench iis`: harness passed.
- `cargo doc -p roml --no-deps` and `cargo doc -p roml-highs --no-deps`:
  passed.
- `cargo package --allow-dirty --list` inspected for `roml` and
  `roml-highs`; Phase 29 artifacts are included without native source dumps.
- Exact HiGHS v1.15.0 and `highs-sys` v1.15.0 source/header/generated-binding
  audit recorded in `docs/knowledge/highs_iis_api.md`.

The no-feature `roml-highs` check is not a qualification result: with neither
`bundled` nor `system`, `highs-sys` intentionally has no discovery provider;
the audit host also has no system HiGHS installation.

## Unresolved blockers

1. `InfeasibilityPlan` has no `SolveOverlay` input, so solve-lock and
   temporary-fixing atoms are represented in the contract but not executable
   through the public analysis entry point.
2. `ConflictGrouping::ByConstruct` does not yet collapse multi-row generated
   constructs into one atom because the current identity compiler does not
   emit a multi-row construct bridge inventory.
3. System HiGHS native IIS remains typed `Unsupported` pending a supported
   header/library/version qualification matrix.
4. Independent code review and release-level machine metadata/baseline
   performance comparison remain required gates.

## Next review gate

Review this branch against the packet and `29-PLAN.md`. The reviewer must
either approve the two explicit semantic-scope deferrals for a follow-up
phase or require overlay/grouped-construct completion before Phase 29 can be
marked complete. After that decision, run the repository's independent review
workflow and resolve all P0/P1 findings before integration.
