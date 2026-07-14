# ROML Public-Release Hardening Project

**Authoritative baseline:** `main@82e2ed95545635b628187ba0081fe8c8b03eaafb`  
**Historical audit baseline:** `main@f9ba1921e650b5057bbc4de090a78391f7932a53`  
**Planning date:** 2026-07-13  
**Target:** a trustworthy pre-1.0 Rust workspace and crates.io release process, not an immediate publication.

The historical principal-engineering audit is supplemented by `docs/release/CURRENT_MAIN_DELTA_AUDIT.md`, which reconciles variable-type mutation, Xpress bulk synchronization, semi-continuous domains, and per-solve option plumbing added after `f9ba192`.

## Product thesis

ROML is a solver-independent MILP modeling kernel optimized for repeated model mutation and solver re-optimization. Its differentiator is not merely ergonomic model construction; it is explicit dependency tracking from mutable parameters to model coefficients, revision-aware delta projection, and efficient synchronization into long-lived solver instances.

The core abstraction is:

`parameter state -> symbolic coefficient graph -> canonical model state -> revisioned delta stream -> backend projection -> solution state`

The public release must make each arrow explicit, testable, recoverable, and independent of machine-specific native-library assumptions.

## Release objective

Produce a workspace in which:

- `roml` is a portable, solver-free modeling and incremental-state crate.
- Solver/solve-session policy such as algorithm choice, limits, logging, and callbacks is supplied through an explicit solve request rather than stored in canonical `Model` state.
- Solver adapters are optional crates with safe Rust APIs and explicit backend capabilities.
- Raw FFI and native discovery are isolated behind maintained sys/official binding packages.
- Core correctness is established by model-state invariants, property tests, differential tests, and rebuild-vs-incremental equivalence.
- Variable domains, including semi-continuous and semi-integer semantics, are modeled coherently rather than spread across bounds, types, and side maps.
- Linux, macOS, and Windows are first-class targets.
- crates.io packages contain only intended source, metadata, licenses, documentation, and examples.
- future Python/Java/.NET bindings can target a stable C ABI or versioned wire/model representation rather than Rust's unstable ABI.

## Release train

The first publishable train is intentionally narrow:

1. `roml` core.
2. `roml-highs` as the reference open-source backend.
3. `roml-mosek` and `roml-xpress` only after their licensing, installation, CI, and callback semantics are independently qualified. They may remain unpublished or marked experimental for the first train.

## Architectural boundaries

### Core

Owns typed identities, model entities and domains, canonical coefficients, parameter expressions, revisions, transactions, snapshots, delta journals, solver-neutral capabilities, solution views, and user-facing modeling ergonomics.

The core must not own:

- transient solver/algorithm options,
- global logger initialization,
- YAML configuration,
- native library discovery,
- solver-specific status constants,
- raw pointers,
- runtime loader policy,
- solver licenses,
- process-wide backend initialization.

### Backend adapters

Own safe translation from canonical model operations into a solver API, backend capability declarations, solve-request validation, effective-configuration reporting, native error normalization, index mappings, lifecycle, incremental application, solve control, and solution extraction.

### Raw bindings

Own generated or vendor-maintained declarations, native discovery/build/linking, target-specific filenames, ABI/version checks, and the Cargo `links` contract. They expose no modeling policy.

## Non-goals for the first release

- A universal nonlinear or conic modeling language.
- Stable serialization of internal Rust IDs without a separate format contract.
- Python, Java, or .NET wrappers.
- Automatic installation or redistribution of commercial solvers.
- A uniform callback feature that pretends all solvers support identical mutation semantics.
- ABI compatibility across arbitrary solver major versions.
- Performance claims without reproducible benchmarks and matched baselines.

## Quality bar

Release readiness means:

- no known correctness defects in canonical model/domain semantics or incremental synchronization;
- no handwritten ABI layouts/constants where maintained generated/official bindings exist;
- no panics across FFI boundaries;
- no silent native return-code loss;
- no silent ignore of requested solver options or capabilities;
- no destructive delta or solve-request loss on synchronization/solve failure;
- no platform path encoded as the default production behavior;
- no mandatory native solver dependency for building/testing/docs of `roml`;
- documented MSRV and supported target matrix;
- reproducible package and release evidence.

## Success metrics

- Incremental application is observationally equivalent to rebuilding from a canonical snapshot for every supported change sequence.
- Every model revision is either acknowledged by an adapter or remains replayable.
- Requested solve policy is either explicitly applied, explicitly adjusted, or explicitly rejected; the effective configuration is inspectable.
- Core CI passes on stable, MSRV, Linux, macOS, and Windows without native solvers.
- HiGHS integration passes end-to-end on all three operating systems.
- `cargo package --list` and `cargo package --no-verify`/`--locked` checks are clean for each publishable crate.
- Public API and semver checks detect unreviewed breakage.
- Unsafe code is localized, documented, and covered by focused tests or executable assertions.
- Release documentation allows a new contributor to build, test, package, and diagnose native discovery without tribal knowledge.

## Operating principles

- Correctness before micro-optimization.
- Canonical state before deltas.
- Acknowledged revisions before destructive cleanup.
- Explicit effective configuration rather than best-effort silence.
- Capabilities rather than optimistic booleans.
- Generated/official ABI declarations rather than copied constants.
- Platform matrices rather than single-host assumptions.
- Separate compile-time availability from runtime license/library availability.
- Evidence before release claims.