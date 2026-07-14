# Phase 06 Plan — Release Qualification and Staged Publication

> Publication is a separate explicitly authorized action. Completing this phase does not itself authorize `cargo publish`, tags, or GitHub releases.

**Goal:** create a reproducible release candidate, independently verify it, and publish selected crates in dependency order only after owner approval.

**Requirements:** all applicable R0–R9.

## Task 6.1 — Freeze release scope and versions

**Files:** `.planning/STATE.md`, `CHANGELOG.md`, manifests, `docs/release/RELEASE_CANDIDATE.md`.

Record:

- exact release commit and clean tree;
- selected crates and excluded/experimental crates;
- versions and dependency order;
- supported Rust/platform/backend matrix;
- native solver versions/features;
- unresolved P2 items accepted for pre-1.0;
- zero unresolved P0/P1 findings;
- crates.io name/ownership verification.

No code changes enter the candidate except reviewed release blockers; any change invalidates prior evidence and restarts affected checks.

## Task 6.2 — Build immutable package artifacts

For each selected crate:

```bash
cargo package --list -p <crate>
cargo package -p <crate> --locked
sha256sum target/package/<crate>-<version>.crate
```

Store checksums, normalized manifests, archive file lists, and toolchain versions in a release evidence artifact. Confirm license/readme/source/provenance files are present and no proprietary/local/generated files are included.

## Task 6.3 — Test from packed artifacts

In fresh temporary directories/containers/VMs:

1. install/use the `.crate` artifacts through a local registry or unpacked verified source without workspace paths;
2. build core on Linux/macOS/Windows;
3. build and solve with HiGHS on all supported targets;
4. run examples/doctests;
5. test default/minimal/all supported features;
6. test MSRV;
7. test clean-host failure messages;
8. test any selected commercial adapter according to its support tier.

Evidence must prove crates do not rely on untracked workspace files, git submodules unavailable to crates.io, or path-only dependencies.

## Task 6.4 — Run full qualification suite

Mandatory:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --all-features
cargo test --doc --workspace --all-features
cargo deny check
cargo audit
cargo semver-checks check-release -p roml
cargo semver-checks check-release -p roml-highs
```

Plus:

- support matrix workflows;
- generated incremental-vs-rebuild suites with retained seeds;
- unsafe/FFI audit;
- callback/lifecycle stress;
- benchmark report;
- docs link/example checks;
- package consumer matrix.

Document every skipped check with reason and whether it changes the support label. Mandatory checks cannot be waived silently.

## Task 6.5 — Independent reviews

Request at least:

1. principal architecture/core correctness review;
2. Rust public API/semver review;
3. unsafe/FFI/native lifecycle review;
4. packaging/license/release review;
5. backend-specific review for each published adapter.

Reviewers must inspect evidence and source, not only CI status. Resolve all P0/P1 comments and record dispositions for P2 comments.

## Task 6.6 — Security and supply-chain review

Verify:

- dependency advisories/licenses/sources;
- build script behavior and network assumptions;
- vendored HiGHS source/license/notices;
- no commercial binary/header leakage;
- GitHub Actions pinned to reviewed major/full SHAs per policy;
- least-privilege workflow permissions;
- release provenance/attestation strategy;
- secrets cannot reach fork jobs;
- callback panic containment and unsafe inventory complete.

## Task 6.7 — Dry-run publication order

Expected order subject to final topology:

1. any new publishable low-level/shared crate;
2. `roml`;
3. `roml-highs`;
4. independently qualified commercial adapters.

Use `cargo publish --dry-run -p <crate> --locked` where supported and re-run fresh consumer resolution using exact registry-style versions. Ensure dependency versions exist before dependent publication.

## Task 6.8 — Owner authorization checkpoint

Present a compact decision packet:

- candidate SHA;
- crate/version list;
- support matrix;
- evidence links/checksums;
- unresolved accepted risks;
- reviewer approvals;
- dry-run output;
- rollback/yank procedure;
- exact publish commands.

Proceed only on explicit owner authorization naming the candidate SHA and crates.

## Task 6.9 — Staged publish and post-publish validation

After authorization:

1. publish one crate at a time in dependency order;
2. wait only as required for registry indexing within the same operator session/process—do not assume success;
3. create a fresh crates.io consumer and compile/test;
4. verify crates.io metadata/readme/license;
5. verify docs.rs build and public links;
6. verify checksums/version/source match candidate;
7. publish dependents only after prerequisite validation;
8. tag the exact candidate commit and create release notes only after all selected crates validate.

If a critical defect appears, stop; assess yank under documented policy. Never overwrite a published version.

## Task 6.10 — Archive release evidence and open next milestone

Commit/update:

- final CHANGELOG;
- release notes;
- support matrix;
- `.planning/STATE.md` with release SHA/tag/versions;
- archived evidence checksums and links;
- post-release monitoring/issues;
- next milestone candidates.

Schedule no automatic publication. Future releases repeat this qualification flow.

**Phase gate:** exact package artifacts pass all mandatory matrices and independent reviews; owner authorizes the exact candidate; registry/docs fresh-consumer validation succeeds; tag and release correspond to verified source.