# M2 Execution Protocol

## Branch strategy

Planning branch:

```text
docs/public-api-ergonomics-gsd-ultra
```

Implementation branches, created from reviewed current `main` descendants:

```text
phase-roml-P20-api-contract
phase-roml-P21-solver-facade
phase-roml-P22-modeling-ergonomics
phase-roml-P23-surface-curation
phase-roml-P24-consumer-qualification
```

Do not implement production code on the planning branch. Do not combine multiple phases into one PR.

## Work-in-progress limit

- one active implementation branch;
- one active review/fix branch;
- no next-phase coding before the current phase gate passes;
- documentation inventory is allowed in parallel, but no final API claims before P23.

## Required workflow per phase

1. Fetch current refs and record exact base SHA.
2. Read this milestone packet and the phase plan.
3. Create an isolated worktree.
4. Run and record baseline commands before modifications.
5. Add characterization or failing tests first.
6. Implement the smallest behavior that closes the test.
7. Run focused tests after each coherent change.
8. Commit small, single-purpose units.
9. Run phase verification matrix.
10. Request independent API/protocol review.
11. Resolve all blocking findings.
12. Update M2 `STATE.md` and final evidence with verified facts only.

## Commit policy

Recommended commit sequence:

```text
test: characterize <behavior>
feat: add <small public capability>
refactor: route <existing path> through <new capability>
docs: document <accepted API>
test: qualify <failure or consumer path>
```

Avoid mixed commits that simultaneously rename public APIs, change synchronization, and rewrite docs.

## Baseline matrix

Core:

```bash
cargo fmt --all -- --check
cargo check -p roml --all-targets
cargo clippy -p roml --all-targets -- -D warnings
cargo test -p roml --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
cargo test --doc -p roml
cargo package --list -p roml
```

HiGHS:

```bash
cargo check -p roml-highs --all-targets
cargo clippy -p roml-highs --all-targets -- -D warnings
cargo test -p roml-highs --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps
cargo package --list -p roml-highs
```

Workspace final:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --all-features
cargo test --doc --workspace
cargo package -p roml --locked
cargo package -p roml-highs --locked
cargo deny check
```

## Review gates

### API review

Verify discoverability, naming consistency, type inference, errors, migration, and rustdoc. Reviewer should build at least one model without reading implementation source.

### Protocol review

Verify commit atomicity, delta selection, rebuild fallback, cursor advancement, health classification, and stale-solution invalidation.

### Consumer review

Verify packed crates from a fresh project, no workspace path leakage, documented imports, and native discovery behavior.

## Evidence standards

Every claimed passing check records:

- command;
- toolchain and platform;
- commit SHA;
- exit status;
- test count or relevant output;
- skipped checks and reason.

No screenshot-only evidence. No “works locally” claim without command output.

## Completion handoff

P24 produces `docs/release/evidence/M2_PUBLIC_API.md`. The owner reviews the evidence and decides whether to merge the final phase. This milestone does not authorize publication.