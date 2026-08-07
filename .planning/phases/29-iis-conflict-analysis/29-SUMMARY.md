---
phase: 29-iis-conflict-analysis
status: complete
branch: phase-roml-P29-iis-conflicts
base: d26728e
---

# Phase 29 execution summary

## Delivered

- Frozen LP infeasibility contract and tri-state oracle vocabulary.
- Plan-driven `SolverSession::analyze_infeasibility` orchestration using an
  isolated session and exact `CompilationId`.
- Semantic side-level restriction universe with persistent-fixing layer
  restoration, default semantic equality/construct atoms, overlay layers,
  and deterministic atom identity.
- ROML adaptive chunk reduction, single-atom polish, fresh final/member
  verification, call/iteration/time budgets, and guarantee downgrades for
  unknown or limited checks.
- Canonical structured report plus deterministic text and Markdown renderers.
- Explicit `OriginalLp` and `LpRelaxation` scope handling; relaxation columns
  are converted to continuous only in the isolated analysis snapshot.
- Audited bundled HiGHS 1.15.0 native IIS seed provider in
  `roml-highs/src/native_iis.rs`, using generated `highs-sys` bindings only;
  the portable oracle is shared by bundled and system HiGHS.
- Correctness, mutation, differential, planted-IIS, persistent-fixing,
  native-seed, isolation, overlay, budget, verification-failure, and
  LP-relaxation regression tests.
- A real bundled-HiGHS planted-IIS comparison harness covering portable
  adaptive, portable single-atom, native-seeded ROML, and native-only cases
  at sizes 32/128/512.
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
- `cargo test -p roml-highs --test iis`: passed (12 tests).
- `cargo bench -p roml-highs --bench iis -- --nocapture`: passed for the
  planted 32/128/512 comparison corpus.
- `cargo doc -p roml --no-deps` and `cargo doc -p roml-highs --no-deps`:
  passed.
- `cargo package --allow-dirty --list` inspected for `roml` and
  `roml-highs`; Phase 29 artifacts are included without native source dumps.
- Exact HiGHS v1.15.0 and `highs-sys` v1.15.0 source/header/generated-binding
  audit recorded in `docs/knowledge/highs_iis_api.md`.

The temporary system HiGHS 1.15.1 source build and install completed, but the
system-feature Rust check could not run because this audit environment lacked
the cached `pkg-config` crate and could not resolve crates.io. This is an
environment limitation, not a code qualification result. The actual system
CI lane remains the required 1.9.x portable-oracle qualification.

## Deferred follow-up

1. System HiGHS native IIS remains typed `Unsupported` pending a supported
   header/library/version qualification matrix; this does not block Phase 29.
2. Release-level machine metadata and baseline performance comparison remain
   subsequent qualification/release work; the deterministic planted-IIS
   comparison harness is complete.

## Completion gate

All required CI lanes are green, expert review `4884735350` is CLEAR TO MERGE,
and PR #39 was merged by squash as
`19c8c70e3f463fc96b2b723537deb71759b825f5`. Phase 29 is closed; the next
active phase is P30 soft constraints.
