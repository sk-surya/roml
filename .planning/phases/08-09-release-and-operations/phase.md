---
gsd_phase_number: M1R-08-09
name: release-candidate-publication-and-operations
milestone: ROML-M1R
dependencies: [M1R-04, M1R-05]
parallelism: evidence assembly and independent reviews may run in parallel; publication is strictly serialized
---

# M1R-08 — Release Candidate and Publication

## Admission
- M1R-01 through M1R-05 accepted for one exact SHA.
- All mandatory hosted CI and packed-consumer cells pass that SHA.
- No unresolved correctness, FFI safety, package, legal, or production-support blocker.
- Crate names and dual-license authorization recorded.

## Tasks
### 08.1 Freeze release scope
Freeze exact:
- `roml` and `roml-highs` versions;
- dependency versions and publication order;
- MSRV and supported target matrix;
- default/optional features;
- backend version compatibility;
- public API and semver baseline;
- changelog, migration guide, release notes, support claims, and known limitations.

No MOSEK/Xpress crate enters the release without its independent gate and explicit owner decision.

### 08.2 Rebuild release evidence
From a clean exact-SHA checkout:
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps
cargo package -p roml --locked
cargo package -p roml-highs --locked
cargo deny check
cargo machete
cargo audit
```
Run exact hosted matrix and packed consumers. Archive package file lists, archives, checksums, SBOM, dependency tree, action/run links, compiler/backend versions, benchmark summary, and skipped count.

### 08.3 Independent reviews
Four independent dispositions:
1. architecture/public API and semver;
2. native/unsafe/ABI/lifecycle;
3. semantic correctness/differential/fault evidence;
4. release operations/package/provenance.

Every finding is accepted, fixed, rejected with evidence, or explicitly owner-waived. P0/P1 correctness or safety findings cannot be waived for stable publication.

### 08.4 Publication rehearsal
- Use dry-run/package archive consumers.
- Verify crates.io metadata, ownership, token scope, dependency ordering, and version availability.
- Prepare commands but do not execute publication without owner authorization.
- Ensure release automation cannot publish commercial crates accidentally.

### 08.5 Owner authorization record
Record exact SHA, crate/version list, package hashes, evidence index, unresolved low-risk items, and authorization timestamp. Any code or metadata change invalidates authorization and requires rerunning affected gates.

### 08.6 Serialized publication
1. Publish `roml` exact version.
2. Verify crates.io metadata and docs.rs/build status.
3. Build `roml-highs` against the released core version in a fresh consumer.
4. Publish `roml-highs`.
5. Verify crates.io/docs.rs and end-to-end consumer.
6. Tag the exact source commit and create release notes/evidence links.

On failure, stop; do not retag or publish a different SHA without a new evidence/authorization record.

## M1R-08 gate
M1R-R1–R4 pass and published artifacts correspond exactly to evidence. A technically complete but unauthorized candidate is `Owner-blocked`, not released.

# M1R-09 — Post-Release Operations

## Tasks
### 09.1 Compatibility watch
Maintain tested compatibility against supported HiGHS patch/minor versions, Rust stable/MSRV, and target matrix. New native versions enter support only after CI and semantic smoke evidence.

### 09.2 Patch-release process
Define severity, branch/backport policy, required regression tests, release evidence delta, owner authorization, and versioning. Do not rebuild old tags with changed dependencies.

### 09.3 Security response
Activate SECURITY guidance, private reporting, native dependency advisory handling, credential/token rotation, embargo process, and release remediation procedure.

### 09.4 Issue and diagnostic templates
Collect ROML version/SHA, backend version/build mode, OS/architecture, Rust version, model size/class, request/effective configuration, status/native code, and minimal reproduction. Avoid requesting proprietary model data by default.

### 09.5 Deprecation and semver
Define pre-1.0 compatibility promises, deprecation window, migration notes, and semver-check baseline. Track intentional unstable modules explicitly.

### 09.6 M2 admission review
After a real patch cycle or equivalent rehearsal, assess M2-A1. Do not start industrial modeling expansion merely because v0.1 was published.

## M1R-09 gate
M1R-R5 passes; patch/security/compatibility procedures are executable; M1R milestone is archived with retrospective and exact evidence.
