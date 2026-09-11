# ROML Public-Release Hardening Project

## Scope amendment — MIR shared modeling IR, 2026-09-10

The owner has authorized [MIR — Shared Modeling IR and Block-Native Core](milestones/MIR-modeling-ir/README.md) as the active successor after the merged MPY interface and post-MPY performance/certification work. MIR establishes one shared ordinal modeling system for native Rust and Python, strengthens block-native construction and parameter propagation, and precedes additional Python OO ergonomics and the deferred M4 preview.

This amendment does not authorize Pyomo source compatibility, a separate `AbstractModel` subsystem, a macro-first DSL, publication, or M4 production. Existing canonical coefficient, stale-ID, ownership, transaction, revision/snapshot, solver-independence, native-safety and review gates remain binding. MIR tranche 1 (MIR-00/01/02) is authorized one phase at a time under its packet.

## Prior scope amendment — Python successor milestone, 2026-09-07 (fulfilled)

The MPY Python interface was authorized under the prior amendment and has since merged via PR #53. Its PyO3 + maturin boundary remains accepted; its packet is retained as regression/history rather than current routing. The wrapper non-goal below describes the original release train, not a continuing prohibition.

**Historical authoritative baseline:** `main@82e2ed95545635b628187ba0081fe8c8b03eaafb`
**Historical audit baseline:** `main@f9ba1921e650b5057bbc4de090a78391f7932a53`
**Original planning date:** 2026-07-13
**Target:** a trustworthy pre-1.0 Rust workspace and crates.io release process, not an immediate publication.

The historical principal-engineering audit is supplemented by `docs/release/CURRENT_MAIN_DELTA_AUDIT.md`. Active milestone packets must refresh exact `main` and reconcile historical audit claims before implementation.

## Product thesis

ROML is a solver-independent MILP modeling kernel optimized for repeated model mutation and solver re-optimization. Its differentiator is not merely ergonomic model construction; it is explicit dependency tracking from mutable parameters to model coefficients, revision-aware delta projection, and efficient synchronization into long-lived solver instances.

The core abstraction is:

`parameter state -> symbolic coefficient graph -> canonical model state -> revisioned delta stream -> backend projection -> solution state`

The public release must make each arrow explicit, testable, recoverable, and independent of machine-specific native-library assumptions.

MIR adds a user-facing corollary without changing the canonical thesis:

`Rust/Python modeling syntax -> shared ordinal array IR -> block/CSR lowering -> canonical model state`

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
- Python uses the accepted direct PyO3/maturin boundary; other-language bindings require their own reviewed boundary design.

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
- Automatic installation or redistribution of commercial solvers.
- A uniform callback feature that pretends all solvers support identical mutation semantics.
- ABI compatibility across arbitrary solver major versions.
- Performance claims without reproducible benchmarks and matched baselines.

The historical “no Python wrappers” non-goal is superseded by D-016/MPY. MIR's shared Rust/Python modeling layer is governed by D-019.

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
- MIR high-level Rust/Python vectorized paths are measured against raw Level-2 bulk construction and preserve canonical/snapshot/revision equivalence.

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
