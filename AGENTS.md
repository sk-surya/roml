# ROML Agent Instructions

## Repository mission

ROML is a pre-1.0 Rust MILP modeling workspace centered on parameter-dependent coefficients and incremental projection into persistent solver sessions. Current workspace crates are:

- `roml` — solver-independent model, expressions, changes, solutions, and solver contract.
- `roml-highs` — HiGHS adapter.
- `roml-mosek` — MOSEK adapter.
- `roml-xpress` — FICO Xpress adapter.

The current implementation is functional but not yet release-qualified. Do not repeat the existing “production-grade” label as a verified fact. The public-release program is intended to establish that quality bar with evidence.

## Governing documents

Read these before implementation:

1. `.planning/PROJECT.md`
2. `.planning/REQUIREMENTS.md`
3. `.planning/ROADMAP.md`
4. `.planning/STATE.md`
5. `docs/release/PRINCIPAL_ENGINEERING_AUDIT.md`
6. `docs/release/CURRENT_MAIN_DELTA_AUDIT.md`
7. `docs/release/ARCHITECTURE_DECISIONS.md`
8. `docs/superpowers/specs/2026-07-13-public-release-hardening-design.md`
9. the applicable file under `docs/superpowers/plans/`

The historical audit is anchored at `f9ba192`; the authoritative implementation baseline for this plan is `main@82e2ed95545635b628187ba0081fe8c8b03eaafb`. The delta audit reconciles the four later commits.

If task prose conflicts with requirements or architecture decisions, requirements and accepted decisions govern. Amend an ADR explicitly rather than silently changing direction.

## Current architecture and known transition

Current flow:

```text
Model mutation -> ChangeLog -> destructive drain -> SolverAdapter::apply_changes -> solve
```

Target flow:

```text
Canonical model revision
    -> immutable typed DeltaBatch
    -> per-adapter cursor and acknowledgement
    -> safe backend session
    -> deterministic snapshot rebuild on unsupported/dirty failure
```

Do not deepen dependencies on the current destructive drain protocol. Adapter optimizations must migrate to the revisioned P2 contract.

## Non-negotiable invariants

- One canonical coefficient cell exists for each `(target, variable)` pair. Multiple symbolic terms are algebraically combined.
- Invalid/stale IDs and invalid numeric/domain values return typed errors; they are not silently ignored.
- A failed backend synchronization cannot lose model operations.
- Multiple adapters can synchronize independently from one model.
- Incremental projection and full snapshot projection are observationally equivalent.
- The `roml` core is solver-free. Transient solver options belong to a solve request/session, not canonical model state.
- Unsupported solve options and backend capabilities are explicit, not silently ignored.
- Raw FFI is isolated behind authoritative generated/official binding boundaries.
- No Rust panic may unwind through C.
- Every native return code is checked or justified.
- `unsafe impl Send/Sync` requires vendor thread-safety evidence, a precise invariant, and tests.
- Commercial solver binaries, headers, licenses, credentials, and machine-specific paths are never committed or packaged.

## Binding policy

- **HiGHS:** prefer pinned `rust-or/highs-sys`, generated from the official C header. Upstream or narrowly fork for genuine API gaps before considering a ROML-specific sys crate.
- **MOSEK:** use the official `mosek` Rust API. Remove handwritten declarations/constants. Never mutate a task from inside a callback unless official documentation explicitly permits the exact operation.
- **Xpress:** first complete the legal/technical binding decision. Use a generated sys boundary or runtime loader only after verifying header-derived redistribution, SDK versions, lifecycle, and supported targets.

A sys crate is an ownership boundary for ABI/build/link policy, not a layer to add uniformly for naming symmetry.

## Workflow

Use GSD for milestone state, phase progression, requirement traceability, and evidence. Use Superpowers for worktree isolation, TDD, systematic debugging, parallel execution, review, and verification.

Before coding:

1. Fetch current refs and record the exact base SHA.
2. Confirm phase prerequisites and requirement IDs.
3. Create an isolated worktree/branch.
4. Run the phase baseline before modifications.
5. Write characterization or failing tests first.

During coding:

- keep commits small and single-purpose;
- preserve current useful behavior with tests before refactoring;
- never infer FFI signatures, constants, layouts, filenames, or callback rules;
- derive native details from a pinned official header/API/version;
- separate compile-time availability, runtime loading, license acquisition, and solve success;
- avoid global logger initialization, filesystem scans, environment mutation, and unsolicited stdout output in library code;
- update docs and CHANGELOG with public behavior;
- record deviations in the applicable design/audit document.

Before completion:

1. Run focused tests and the entire phase matrix.
2. Run formatting, clippy with warnings denied, tests, rustdoc, policy checks, and package checks.
3. Inspect `cargo package --list` for every publishable crate.
4. Record commands, versions, outputs, skipped checks, and residual risks in evidence.
5. Update `.planning/STATE.md` only with verified facts.
6. Request independent review and resolve all P0/P1 findings.

## Branch strategy

Planning branch:

`docs/public-release-production-roadmap`

Suggested implementation branches:

- `phase-roml-P0-release-baseline`
- `phase-roml-P1-core-correctness`
- `phase-roml-P2-revisioned-sync`
- `phase-roml-P3-solver-boundaries`
- `phase-roml-P4-cross-platform-ci`
- `phase-roml-P5-public-api-packaging`
- `phase-roml-P6-release-qualification`

Keep planning/governance changes separate from production implementation. Do not combine unrelated phases in one PR.

## Baseline commands

The exact phase plan governs, but the normal core baseline is:

```bash
cargo fmt --all -- --check
cargo check -p roml --all-targets
cargo clippy -p roml --all-targets -- -D warnings
cargo test -p roml --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
cargo package --list -p roml
```

Backend checks are separate because native installation and licensing differ. Core commands must not require HiGHS, MOSEK, or Xpress.

## Current high-severity defects to preserve as regression targets

- Duplicate parameterized expression terms can target one solver cell with replacement/last-write behavior.
- `sync_model` drains changes before backend acknowledgement.
- Semi-continuous HiGHS synchronization can apply an ordinary bound change and then fail as unsupported, leaving partial backend mutation with no replayable batch.
- `ModelConstants::default()` recursively calls itself.
- `Model` currently owns one-shot `SolveOptions`, leaking solver policy into canonical state.
- Unsupported solve options are documented as silently ignored.
- Handwritten HiGHS/MOSEK/Xpress ABI declarations and constants are version-fragile.
- The MOSEK callback mutates the task inside the callback despite official restrictions.
- Native constructors/callbacks contain panic, unchecked-pointer, ignored-return-code, and lifecycle risks.
- Generated solver logs and placeholder non-Rust scaffolding contaminate repository/package boundaries.

Do not “fix” these by deleting tests, weakening errors, or hiding unsupported behavior. Establish the correct invariant and verify it.

## Release safety

- Do not publish any crate, create a tag, or create a release without explicit owner authorization for the exact SHA and crate list after Phase 6.
- Keep `roml-mosek` and `roml-xpress` unpublished/experimental until independently qualified.
- Do not use admin merge bypass.
- A phase is complete only when its gate, evidence, and independent review pass. “Works on my Mac” is not evidence of cross-platform support.