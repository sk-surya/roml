---
gsd_phase_number: M1R-00
name: truth-reset-and-candidate-admission
milestone: ROML-M1R
goal: Establish the exact candidate truth and admit work requirement-by-requirement.
dependencies: [main@ef37c88]
parallelism: audit lanes may run in parallel; disposition/state has one owner
---

# M1R-00 — Truth Reset and Candidate Admission

## Outcomes
- Frozen candidate head and complete commit/file manifest.
- Requirement disposition: accepted, partial, failed, superseded, owner-blocked, external-blocked.
- All 11 ignored tests reconciled individually.
- Exact command and CI evidence captured.
- Integration strategy chosen: merge candidate, split/replay selected commits, or replace affected work.

## Tasks
### 00.1 Freeze and inventory
- Record `main`, candidate, planning heads and merge base.
- Export commit list, changed-file list, dependency changes, unsafe diff, public API diff, workflow diff, package diff.
- Map each candidate commit to M1 requirement IDs and evidence documents.
- Detect unrelated changes and branch contamination.

### 00.2 Reproduce verification
Run at exact candidate head:
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps
cargo package -p roml --locked
cargo package -p roml-highs --locked
cargo deny check
cargo machete
cargo audit
```
Record OS, architecture, Rust/Cargo version, native HiGHS version/build mode, ignored/skipped tests, and package lists. Never replace unavailable native evidence with solver-free success.

### 00.3 Ignored-test reconciliation
For each ignored test:
1. state the defect/law it characterizes;
2. run after removing `#[ignore]` in a temporary worktree;
3. classify pass/fail/compile-invalid/superseded;
4. map to M1R-C/Q requirement;
5. either promote to mandatory test, replace with a better mandatory test, or document an external impossibility.

### 00.4 Source-vs-claim audit
Inspect and cite exact source for:
- legacy `SolverAdapter` and `SolverModelExt`;
- `drain_changes()` behavior;
- model-owned `SolveOptions`;
- option negotiation semantics;
- public exports/prelude/docs/examples;
- HiGHS constructor, `Send`/`Sync`, return-code handling, trait implementation;
- conformance harness parameterization;
- CI platform/build/package coverage.

### 00.5 External gates
- Record owner license authorization.
- Verify crates.io name ownership/availability without publishing functional code.
- Record commercial SDK/license/runner status separately.

### 00.6 Candidate disposition
Produce `docs/release/evidence/M1R/M1R-00-ADMISSION.md` with a row per requirement and candidate component. Choose one integration strategy:
- **A merge after repairs:** only if candidate is cohesive and reviewable.
- **B split/replay:** preferred when planning, core contract, backend, CI, benchmarks, and commercial docs need independent review.
- **C replace:** when candidate semantics conflict with the frozen architecture.

## Gate
- No stale completion claim remains.
- Every ignored/skipped check has disposition.
- Candidate admission is requirement-level, not branch-level.
- M1R-01 has an exact base and accepted inputs.

## Review packet
Independent reviewer verifies manifest completeness, reruns a sample of commands, checks five randomly selected requirement dispositions, and confirms no implementation begins from unverified state.
