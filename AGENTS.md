# ROML Agent Instructions

## Current owner instruction — MIR shared modeling IR, 2026-09-10

For current modeling/core work, read `.planning/milestones/MIR-modeling-ir/README.md` and the complete MIR packet before implementation. Load `.planning/milestones/MIR-modeling-ir/skills/roml-ir-invariants/SKILL.md` before touching coefficient storage, parameter propagation, delta compilation, shared modeling IR, or Python array/expression lowering.

MIR precedes further Python OO ergonomics and the deferred M4 preview. Tranche 1 (MIR-00, MIR-01, MIR-02) is owner-authorized, one phase at a time. D-019 governs the shared ordinal IR, trusted block spans, packed parametric construction, block-native repricing, self-contained deltas and fallback rules. Preserve branch protections, independent review, canonical one-cell semantics, per-entity staleness, model ownership, transactions, snapshots and revision replay. Do not request permission again for already-authorized MIR-00/01/02 work. Leave runtime implementation PRs reviewable unless separately authorized to merge; publication/tag/release remain separate owner gates.

## Prior owner instruction — Python successor, 2026-09-07 (fulfilled)

The MPY Python interface implementation merged via PR #53 and the post-MPY performance/certification stack merged via PR #55 with closeout #56. The MPY packet remains historical/regression authority, but it is no longer the active routing target. Several architecture/defect descriptions below are historical hardening-baseline text, not evidence that those defects remain on current main; reconcile them against current code and retain resolved defects as regression history.

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
10. the active milestone packet and any required milestone-local skill

The historical audit is anchored at `f9ba192`; the authoritative implementation baseline for each new phase is the exact refreshed `main` recorded by that phase. Do not reuse a historical baseline when the active packet requires a fresh one.

If task prose conflicts with requirements or architecture decisions, requirements and accepted decisions govern. Amend an ADR explicitly rather than silently changing direction.

## Current architecture and known transition

The revisioned canonical model/delta protocol is authoritative. Do not reintroduce destructive one-shot synchronization assumptions that bypass revision replay, snapshot recovery, or per-adapter acknowledgement. MIR fast paths must compile into the same self-contained revision protocol as scalar/general paths.

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
- **Xpress:** use the accepted binding boundary/version policy; any change to commercial binding ownership requires renewed legal/technical evidence.

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
3. Inspect `cargo package --list` for every publishable crate touched by the phase.
4. Record commands, versions, outputs, skipped checks, and residual risks in evidence.
5. Update `.planning/STATE.md` only with verified facts.
6. Request independent review and resolve all P0/P1 findings.

## Branch strategy

Use the active milestone's roadmap for branch names. Keep planning/governance changes separate from production implementation. Do not combine unrelated phases in one PR.

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

Backend checks are separate because native installation and licensing differ. Core commands must not require MOSEK or Xpress; HiGHS checks follow the active phase requirements.

## Historical regression targets

The principal-engineering audit and historical milestone packets contain defects that may already be fixed. Preserve their regression tests and invariants; do not copy stale defect claims into current state without reproducing them on the exact head.

Do not “fix” regressions by deleting tests, weakening errors, or hiding unsupported behavior. Establish the correct invariant and verify it.

## Release safety

- Do not publish any crate, create a tag, or create a release without explicit owner authorization for the exact SHA and crate list.
- Keep commercial backends independently qualified under their accepted support labels.
- Do not use admin merge bypass.
- A phase is complete only when its gate, evidence, and independent review pass. “Works on my Mac” is not cross-platform evidence.
