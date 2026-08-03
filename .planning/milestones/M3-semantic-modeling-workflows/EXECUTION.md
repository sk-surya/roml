# M3 Execution Protocol

## Governing order

For M3 implementation, read and obey in this order:

1. `AGENTS.md`
2. `.planning/milestones/M3-semantic-modeling-workflows/REQUIREMENTS.md`
3. `.planning/milestones/M3-semantic-modeling-workflows/DECISIONS.md`
4. `docs/superpowers/specs/2026-08-02-semantic-modeling-and-solve-workflows-design.md`
5. `.planning/milestones/M3-semantic-modeling-workflows/ROADMAP.md`
6. `docs/superpowers/plans/2026-08-02-semantic-modeling-and-solve-workflows.md`
7. current phase evidence/state

Requirements and accepted decisions govern over task prose. Amend a decision explicitly rather than silently changing direction.

## Branch and worktree protocol

Planning branch:

```text
docs/m3-semantic-modeling-workflows
```

Implementation branches:

```text
phase-roml-P25-semantic-ir-foundation
phase-roml-P26-compiler-backend-ir
phase-roml-P27-fixing-locks-overlays
phase-roml-P28-solve-plan-warm-starts
phase-roml-P29-iis-conflict-reports
phase-roml-P30-soft-constraints
phase-roml-P31-lexicographic-objectives
phase-roml-P32-common-constructs
phase-roml-P33-piecewise-linear-bounds
phase-roml-P34-m3-qualification
```

Before each phase:

1. fetch current `main` and record exact SHA;
2. verify prerequisite phase merge/evidence;
3. create an isolated worktree from current `main`;
4. run untouched baseline commands;
5. capture public API and package lists when the phase affects public surface;
6. create the phase evidence file before implementation and append facts as work proceeds.

Do not stack a phase on an unmerged predecessor unless the roadmap explicitly permits parallel work and the base dependency is frozen. Rebase before final review.

## WIP and parallelism

Default WIP:

- one coding branch;
- one review/fix branch.

Permitted bounded parallelism:

- P29, P30, and P31 after P28, each with independent ownership and one integration reviewer; tasks inside P29–P31 run serially (plan Tasks 11→12→13→14) because they share capability, session, facade, and solution files;
- P32 after P26 while P27–P31 proceed, only if P26's compiler API is frozen;
- research/header audits may run in parallel but may not land production code against speculative APIs.

Do not open P34 until every feature branch is merged and its evidence accepted.

## TDD protocol

For every public semantic or backend behavior:

1. write a focused failing semantic/characterization test;
2. run it and record the expected failure;
3. implement the smallest correct behavior;
4. run focused tests;
5. run phase-level tests;
6. commit one coherent unit;
7. update evidence and traceability.

Bridge implementations additionally require:

- a mathematical reference formulation;
- deterministic compiled artifact assertions;
- origin-map completeness assertions;
- solver equivalence tests;
- failure tests for insufficient bounds/capabilities.

No test may be weakened or deleted to make a new representation pass.

## File ownership rules

### Canonical model owners

```text
src/identity.rs
src/metadata.rs
src/function/**
src/construct/**
src/model/**
src/snapshot.rs
src/delta.rs
src/objective_policy.rs
src/assignment.rs
```

Only P25, P27, P30, P31, P32, and P33 should materially modify canonical semantic state. P26 may add compiler-facing read-only projection hooks but must not introduce solver policy into `Model`.

### Compiler owners

```text
src/compiler/**
```

P26 owns foundational types. Later phases add focused bridge modules without changing the core compiler contract unless an executable contradiction is reviewed.

### Solver orchestration owners

```text
src/solver/plan.rs
src/solver/overlay.rs
src/solver/effective_plan.rs
src/solver/multiobjective.rs
src/solver/infeasibility.rs
src/solver/facade.rs
src/solver/session.rs
```

### Backend owners

```text
roml-highs/src/compiler.rs
roml-highs/src/start.rs
roml-highs/src/iis.rs
roml-highs/src/multiobjective.rs
roml-highs/src/session.rs
roml-highs/src/facade.rs
```

Native modules must use authoritative bindings and version gates. Do not duplicate canonical semantics in backend crates.

## Commit policy

Commits are small and single-purpose. Recommended pattern:

```text
test(...): characterize <behavior>
feat(...): add <semantic type/API>
feat(...): compile <construct> to <representation>
fix(...): preserve <invariant>
docs(...): record <public behavior/evidence>
```

A phase should normally contain multiple reviewable commits. Do not combine architecture, all features, docs, and qualification into one commit.

## Review gates

Every phase requires two review passes:

### Pass 1 — Specification and correctness

Reviewer checks:

- requirement coverage;
- semantic correctness;
- invariant preservation;
- unsupported/error behavior;
- origin completeness;
- API coherence;
- official backend evidence;
- test quality.

### Pass 2 — Integration and operations

Reviewer checks:

- incremental/rebuild behavior;
- failure recovery;
- cross-platform/version behavior;
- public API diff;
- package/docs impact;
- performance evidence;
- migration accuracy.

P0/P1 findings block merge. P2 findings may merge only when explicitly accepted and scheduled.

## Native API research protocol

For HiGHS starts, hints, IIS, PWL, SOS, and multiobjective:

1. identify bundled and minimum system versions actually supported by current manifests/CI;
2. inspect the exact pinned official C headers and generated bindings;
3. record symbol signatures, availability, return codes, lifecycle, and documented semantics in `docs/knowledge/`;
4. add compile-time/version characterization tests;
5. implement through existing official/generated binding boundaries;
6. qualify absence as typed unsupported rather than guessing;
7. update support matrix and rustdoc.

Do not infer struct layouts, enum values, callback fields, or feature behavior from another version.

## Baseline command matrix

Core fast lane:

```bash
cargo fmt --all -- --check
cargo check -p roml --all-targets
cargo clippy -p roml --all-targets -- -D warnings
cargo test -p roml --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
cargo package --list -p roml
```

HiGHS lane:

```bash
cargo check -p roml-highs --all-targets
cargo clippy -p roml-highs --all-targets -- -D warnings
cargo test -p roml-highs --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps
cargo package --list -p roml-highs
```

Workspace/policy lane where currently supported:

```bash
cargo fmt --all -- --check
cargo test --workspace --all-targets
cargo deny check
cargo machete
cargo audit
```

Public API/package qualification:

```bash
cargo public-api -p roml
cargo public-api -p roml-highs
cargo package -p roml --locked
cargo package -p roml-highs --locked
```

Exact commands may be extended by phase plans (per-phase plans live at `.planning/phases/Pnn-*/Pnn-PLAN.md`, following the M2 convention; evidence files live at the paths listed in TRACEABILITY.md). Skips must be recorded, not treated as passing.

## Phase-specific mandatory checks

### P25

- canonical snapshot/delta tests;
- model invariant property tests;
- M2 compile-pass examples;
- public API diff.

### P26

- compiler determinism;
- origin completeness;
- ReferenceBackend/HiGHS conformance;
- randomized compiled delta versus rebuild;
- recovery/failure tests.

### P27–P28

- overlay failure injection;
- model revision invariance under temporary operations;
- subsequent-solve leak tests;
- capability/effective-plan assertions.

### P29

- official-header evidence;
- exact report guarantees;
- origin mapping for every conflict member kind;
- version-gated unsupported tests.

### P30–P33

- algebra/reference formulation tests;
- native/portable equivalence;
- generated origin completeness;
- no-silent-relaxation tests;
- parameter dependency/update tests.

### P34

- all mandatory CI lanes;
- fresh packed consumers;
- public API/semver/package review;
- benchmark comparison;
- independent engineering, OR, and NLP-readiness reviews.

## Evidence file structure

Every phase evidence document uses:

```markdown
# Pnn Evidence — Title

## Scope and requirements
## Baseline and environment
## Commit trail
## Public interfaces
## Focused verification
## Full verification
## Native/backend evidence
## Failure/recovery evidence
## Public API and packaging
## Deviations and decisions
## Reviewer findings
## Residual risks
## Gate result
```

Use exact outputs/summaries and link raw artifacts when large.

## Integration protocol

1. rebase phase branch on current `main`;
2. rerun focused and full gates after rebase;
3. update evidence with final head SHA;
4. request independent review;
5. resolve all P0/P1 findings;
6. merge without admin bypass;
7. verify merge-commit CI;
8. update M3 `STATE.md` and `TRACEABILITY.md` in a separate planning-state commit or next planning PR;
9. delete/sunset superseded branches.

## Design amendment protocol

An amendment is required when implementation changes:

- canonical semantics;
- public type/method names used by later phases;
- backend IR or synchronization contract;
- origin/provenance guarantees;
- native/portable selection policy;
- exactness or Big-M rules;
- phase dependencies or acceptance gates.

Amend `DECISIONS.md`, design spec, requirements/traceability, and implementation plan before merging contradictory code.

## Completion and publication rule

M3 completion means code, evidence, reviews, docs, packages, and fresh consumers are qualified. It does not authorize publication. No crate, tag, or release is created without a separate explicit owner approval for the exact SHA and crate list.
