# ROML Public-Release Requirements

Requirement IDs are stable. Every implementation PR must list the IDs it satisfies and the tests/evidence that close them.

## R0 — Release governance

- **R0.1** Every publishable crate has complete crates.io metadata, an SPDX license expression, a packaged README, repository URL, rust-version, keywords/categories, and an explicit include/exclude policy.
- **R0.2** The repository contains LICENSE-MIT and LICENSE-APACHE and uses `MIT OR Apache-2.0`, unless the owner records a replacement licensing decision before implementation.
- **R0.3** Publishing, tagging, and release creation require explicit owner authorization and a signed release checklist.
- **R0.4** Every phase produces machine-verifiable evidence and an independent review.

## R1 — Repository and workspace hygiene

- **R1.1** Remove placeholder Python files, generated logs, obsolete local configuration, and machine-specific artifacts from the Rust release workspace.
- **R1.2** Workspace package/dependency/lint metadata is centralized where appropriate.
- **R1.3** Inter-workspace dependencies include both `path` and `version` so packaged crates resolve from crates.io.
- **R1.4** `cargo package --list` contains only intended files for every publishable crate.
- **R1.5** README links, examples, badges, and claims are accurate.

## R2 — Core semantic correctness

- **R2.1** Invalid or non-finite bounds, coefficients, parameters, tolerances, and division operations are rejected with typed errors.
- **R2.2** A canonical model cell exists for each `(target, variable)` pair; multiple expression terms are algebraically combined rather than projected by last-write-wins.
- **R2.3** Entity removal preserves referential integrity and emits deterministic, complete changes.
- **R2.4** Objective constants and all model constants have defined solver-neutral semantics.
- **R2.5** Public mutators never silently ignore stale/unknown IDs.
- **R2.6** Model invariants are executable and property-tested.
- **R2.7** The recursive inherent `ModelConstants::default` defect is removed and protected by regression tests.

## R3 — Revisioned incremental synchronization

- **R3.1** Model state has a monotonically increasing revision and can produce a canonical snapshot.
- **R3.2** Deltas are not destructively drained before successful adapter acknowledgement.
- **R3.3** Multiple adapters can synchronize independently from one model using per-adapter cursors or equivalent revision tokens.
- **R3.4** Partial backend failure marks adapter state dirty and supports deterministic rebuild from a snapshot.
- **R3.5** Incremental projection is observationally equivalent to a full rebuild over generated mutation sequences.
- **R3.6** Transaction commit/rollback semantics are explicit and tested.
- **R3.7** Change ordering and batching rules are documented and do not rely on accidental adjacency unless guaranteed by a typed batch representation.

## R4 — Solver-neutral contract

- **R4.1** Replace `supports_incremental(&Change) -> bool` with an explicit capability model covering operation, solve type, callbacks, hot starts, duals, reduced costs, and backend limitations.
- **R4.2** Solver statuses preserve feasible-but-not-optimal, interrupted, limit, license, numerical, and backend-error states without false equivalence.
- **R4.3** Errors include backend identity, native code/category, operation, and recoverability.
- **R4.4** Solution access avoids mandatory full-map cloning and supports borrowed/indexed views or iterators.
- **R4.5** Cancellation and callback contracts specify thread, reentrancy, panic, and mutation semantics per backend.

## R5 — FFI and native library safety

- **R5.1** Raw bindings are isolated from safe adapter code.
- **R5.2** HiGHS uses the maintained `rust-or/highs-sys` binding when compatible; missing APIs are upstreamed or introduced in a narrowly scoped, generated fallback—not copied by hand.
- **R5.3** MOSEK uses the official `mosek` Rust API unless a documented gap proves a lower-level boundary necessary.
- **R5.4** Xpress receives a dedicated generated sys/runtime-loading boundary only after header redistribution and crate licensing are verified.
- **R5.5** Every native-linking package declares the Cargo `links` key and has one owner for discovery/link instructions.
- **R5.6** Build logic uses target variables (`TARGET`, `CARGO_CFG_TARGET_*`), not host `cfg!`, for cross-compilation decisions.
- **R5.7** Constructors return typed errors instead of panicking on missing libraries, incompatible ABI, initialization, or license failures.
- **R5.8** No Rust panic can unwind through C. Callback trampolines use `catch_unwind`, null/length validation, and deterministic cleanup.
- **R5.9** All native return codes are checked.
- **R5.10** ABI compatibility is pinned and verified against official headers/version APIs.

## R6 — Backend-specific correctness

- **R6.1** HiGHS status mapping does not collapse `unbounded-or-infeasible` into `infeasible`.
- **R6.2** HiGHS objective replacement handles empty models and preserves `(column,cost)` association.
- **R6.3** HiGHS callback behavior is implemented only through officially supported callback inputs/outputs for the pinned version.
- **R6.4** MOSEK callbacks do not mutate task/environment state; the existing append-row callback strategy is removed or redesigned as a terminate/collect/apply/re-optimize protocol outside the callback.
- **R6.5** MOSEK environment sharing, task lifecycle, license behavior, and native dependencies follow official guidance.
- **R6.6** Xpress initialization/free lifecycle is process-safe and checked; no unconditional stdout writes occur inside callbacks.
- **R6.7** Xpress feature claims match implemented callback/capability behavior.
- **R6.8** Backend index maps and cache state remain coherent after add/remove/deactivate/reactivate sequences.

## R7 — Cross-platform and CI

- **R7.1** Core CI covers Linux x86_64, macOS arm64/x86_64 where practical, and Windows x86_64.
- **R7.2** Stable and MSRV toolchains are tested; nightly-only checks are advisory unless explicitly promoted.
- **R7.3** Formatting, clippy with warnings denied, unit/integration tests, rustdoc with warnings denied, package verification, dependency policy, and semver checks run in CI.
- **R7.4** HiGHS is tested end-to-end on Linux, macOS, and Windows using reproducible bundled or discovered builds.
- **R7.5** MOSEK/Xpress jobs distinguish compile, load, license, and solve gates; licensed jobs may use protected self-hosted runners and must not expose proprietary artifacts.
- **R7.6** docs.rs builds succeed without requiring commercial native libraries.
- **R7.7** Miri, sanitizers, fuzz/property tests, and unsafe-focused checks are scheduled at an appropriate cadence.

## R8 — Performance and regression evidence

- **R8.1** Benchmarks separately measure model construction, parameter propagation, delta generation, delta application, full rebuild, solve, and solution extraction.
- **R8.2** Benchmarks use reproducible datasets, fixed seeds, machine metadata, and statistically meaningful comparisons.
- **R8.3** Performance changes cannot silently regress canonical correctness.
- **R8.4** Bulk APIs exist where per-element FFI calls dominate, with backend-specific evidence.

## R9 — Public API, documentation, and supportability

- **R9.1** Public modules and fields are curated; implementation stores are private or explicitly unstable.
- **R9.2** The crate root explains the architecture, safety model, incremental protocol, and backend separation.
- **R9.3** Every public item has useful rustdoc and examples compile as doctests where appropriate.
- **R9.4** README contains a minimal solver-free model example and a HiGHS end-to-end example.
- **R9.5** CONTRIBUTING, SECURITY, CHANGELOG, release, support, and native troubleshooting documentation exist.
- **R9.6** The project clearly labels experimental adapters/features and avoids unsupported “production-grade” claims before qualification.

## R10 — Future language boundary

- **R10.1** Rust internals are not exposed as a stable foreign ABI.
- **R10.2** A future `roml-c-api` design uses opaque handles, explicit ownership, status codes, version negotiation, and panic containment.
- **R10.3** Stable external identity/serialization is designed independently from arena slot indices and Rust enum layouts.
- **R10.4** Language wrappers are deferred until R2–R9 are complete for the core and reference backend.