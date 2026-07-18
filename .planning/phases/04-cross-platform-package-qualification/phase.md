---
gsd_phase_number: M1R-04
name: cross-platform-package-qualification
milestone: ROML-M1R
goal: Establish clean-runner portability and packed-crate consumer correctness.
dependencies: [M1R-03]
parallelism: hosted CI, package consumers, docs/semver, and scheduled safety lanes may run in parallel
---

# M1R-04 — Cross-Platform and Package Qualification

## Required support matrix
| Artifact/lane | Linux x86_64 | macOS arm64 | macOS x86_64 | Windows x86_64 | MSRV |
|---|---:|---:|---:|---:|---:|
| `roml` fmt/check/test/clippy/docs/package | required | required | where hosted | required | Linux required |
| `roml-highs` bundled build/load/solve | required | required | where hosted | required | defined smoke |
| system-discovered HiGHS | scheduled/required before claim | scheduled | scheduled | scheduled | no |
| packed consumer | required | required | optional duplicate | required | Linux smoke |
| sanitizer/fuzz/property | scheduled | selected | optional | optional | no |

Unsupported cells must be explicitly documented; they are not silently dropped.

## Tasks
### 04.1 Workflow topology
- Fast core workflow for every PR.
- HiGHS workflow triggered by backend/shared-contract changes.
- Package-consumer workflow using generated `.crate` archives.
- Scheduled property/fuzz/sanitizer workflow.
- Release-candidate workflow pinned to exact lockfile/toolchains.
- Commercial workflows separate and protected.

Use concurrency cancellation for superseded commits without cancelling release evidence. Pin actions to reviewed major or commit policy and minimize token permissions.

### 04.2 Native build modes
For bundled/default and system discovery:
- use `TARGET`/`CARGO_CFG_TARGET_*`, never host `cfg!` for target decisions;
- one Cargo `links` owner;
- no developer rpaths or absolute SDK paths in packages;
- distinguish compile, link, load, ABI/version, and runtime failures;
- provide actionable diagnostics with searched paths/configuration;
- verify static/shared runtime dependency behavior.

### 04.3 Packed consumers
For each publishable crate:
1. `cargo package --locked`;
2. extract or use package archive in a clean temporary project;
3. depend on archive/path to extracted package without workspace inheritance;
4. compile examples and run smoke solves;
5. verify licenses, README, metadata, include/exclude list, feature defaults, and dependency versions.

Test `roml` independently, then `roml-highs` against the packed core artifact or exact candidate version strategy.

### 04.4 docs.rs and feature topology
- Ensure docs build without commercial SDKs.
- Ensure default features are release-intentional.
- Document native feature combinations and unsupported combinations.
- Run rustdoc warnings denied and doctests where appropriate.

### 04.5 Policy and supply chain
Run:
- `cargo audit`;
- `cargo deny check`;
- `cargo machete` with documented exceptions;
- `cargo semver-checks` against the chosen baseline;
- license/package-list checks;
- SBOM and checksum generation rehearsal;
- dependency duplication/version review.

### 04.6 Scheduled safety lanes
- property traces with persisted failing seeds;
- fuzz targets for delta decoding/application boundaries or public parsers where applicable;
- sanitizer/native boundary smoke on supported host;
- Miri for solver-free unsafe-free/core logic where useful;
- unsafe inventory diff.

### 04.7 CI evidence integrity
Archive workflow/run/job links, exact SHA, matrix cell, tool versions, package hashes, and artifacts. A rerun after code movement supersedes prior evidence; do not combine results across SHAs into one “green” claim.

## Gate
- M1R-P1–P6 pass.
- Required hosted matrix is green for the same candidate SHA.
- Fresh packed consumers work on Linux, macOS, and Windows.
- docs.rs topology is credible without unavailable commercial SDKs.
- Support matrix contains no untested production claim.
