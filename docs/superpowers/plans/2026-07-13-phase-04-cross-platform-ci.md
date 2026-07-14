# Phase 04 Plan — Cross-Platform CI and Backend Qualification

**Goal:** make platform and backend support executable, reproducible, and accurately labeled.

**Requirements:** R6.*, R7.*, R8.*.

## Task 4.1 — Define the support matrix as data

**Files:** `docs/release/SUPPORT_MATRIX.md`, optional `ci/support-matrix.toml` or YAML consumed by scripts.

For each crate/backend/target record:

- support label;
- Rust toolchain/MSRV;
- architecture;
- native version;
- build mode (bundled/system/runtime-loaded);
- license requirement;
- mandatory commands;
- runner type and cadence;
- known exclusions.

Initial mandatory matrix:

- core: Ubuntu, macOS, Windows;
- core MSRV: Ubuntu;
- HiGHS: Ubuntu, macOS, Windows end-to-end;
- MOSEK/Xpress: no “supported” label until protected load/solve lanes exist.

## Task 4.2 — Split CI by responsibility

**Files:** `.github/workflows/ci-core.yml`, `ci-highs.yml`, `ci-policy.yml`, `ci-nightly.yml`, optional commercial workflows.

Core lane:

```bash
cargo fmt --all -- --check
cargo check -p roml --all-targets
cargo clippy -p roml --all-targets -- -D warnings
cargo test -p roml --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
cargo package --list -p roml
```

Policy lane:

- cargo-deny/audit;
- unused dependencies;
- semver checks after first baseline;
- package archive consumer smoke;
- spelling/link checks where signal is high.

Nightly/advisory:

- Miri for eligible core modules;
- fuzz/property elevated cases;
- sanitizers;
- minimum/maximum dependency version checks if adopted;
- benchmarks with non-flaky thresholds.

## Task 4.3 — Qualify HiGHS on all OS families

1. Test default bundled/static mode on Linux/macOS/Windows.
2. Add optional system-discovery jobs where package managers make this reliable.
3. Test clean cache and warm cache.
4. Run backend contract suite, LP/MIP fixtures, incremental-vs-rebuild randomized tests, callback tests where supported.
5. Verify architecture-specific linking and MSVC runtime compatibility.
6. Verify a downstream consumer project using the packed `roml-highs` crate.
7. Record HiGHS version and feature flags in test output.

**Acceptance:** no environment-specific absolute paths; Windows DLL/import/static behavior and macOS/Linux C++ runtime linkage are validated by CI, not inferred.

## Task 4.4 — Test failure diagnostics as first-class behavior

Create clean-host tests for:

- native library absent;
- invalid override path;
- incompatible version/ABI;
- runtime shared dependency absent;
- license absent/expired/unavailable;
- unsupported target;
- docs.rs mode.

Assertions must check typed category and actionable diagnostics, not exact machine paths.

## Task 4.5 — Commercial solver runner architecture

**Files:** `docs/release/COMMERCIAL_CI.md`, protected workflow files if approved.

Design:

- self-hosted runners isolated by solver/vendor;
- repository/environment secrets for licenses only;
- no upload of proprietary binaries as public artifacts;
- ephemeral workspace cleanup;
- runner labels and concurrency limits matching license tokens;
- compile, load, license, and solve reported as separate steps;
- fork PRs never receive secrets or execute privileged solver jobs;
- scheduled qualification plus trusted-branch required checks only when stable.

Add a mock/no-license lane on hosted runners if the official crate can compile without installed native software; otherwise label unsupported rather than hacking around it.

## Task 4.6 — Differential and generated backend testing

Use P2 mutation generators to create bounded deterministic LP/MIP instances. For each backend capability:

1. build snapshot and solve;
2. apply a random sequence incrementally and solve;
3. rebuild the same final snapshot in a fresh session and solve;
4. compare status class, objective within tolerance, feasibility, and selected primal values;
5. for LP, compare dual/reduced-cost observations with appropriate numerical tolerance/sign conventions;
6. persist failing seed/model/delta trace.

Do not compare exact solutions when multiple optima exist; compare objective and feasibility or canonical tie-broken fixtures.

## Task 4.7 — Native lifecycle stress

Add repeated tests:

- create/load/drop thousands of small sessions where practical;
- reset/rebuild cycles;
- concurrent independent sessions within documented thread/license limits;
- callback registration/unregistration;
- failure during construction/apply/solve/drop;
- process-level init/free ordering for Xpress;
- MOSEK environment/license token lifecycle.

Use sanitizers or platform diagnostics for leaks/use-after-free where supported.

## Task 4.8 — Benchmark infrastructure

**Files:** `benches/` or a dedicated benchmark crate; `docs/performance/BENCHMARKING.md`.

Separate measurements:

- model construction;
- expression normalization;
- parameter dependency propagation;
- delta compilation;
- adapter apply;
- snapshot rebuild;
- solve;
- solution extraction;
- memory/allocations where measurable.

Record seeds, native/Rust versions, CPU/OS, build flags, and confidence intervals. CI uses smoke/regression envelopes only for stable low-variance microbenchmarks; comprehensive comparisons run on controlled hardware.

## Task 4.9 — Cross-platform verification

Required checks:

- all hosted OS jobs green from clean checkout;
- core tests require no solver env vars;
- HiGHS packed-crate consumer tests green on each OS;
- docs.rs simulation or actual docs build green;
- commercial lanes show separate availability/license results;
- unsupported target errors are explicit.

Update support matrix only after evidence. Never label an unexecuted cell supported.

**Phase gate:** mandatory support matrix is continuously green, HiGHS is portable end-to-end, commercial support labels match real protected evidence, and clean-host failures are diagnosable.