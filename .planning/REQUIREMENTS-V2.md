# ROML Program Requirements v2

**Authority:** applies to ROML-M1R through ROML-M5. Existing R0–R10 remain historical traceability; these IDs govern new work.

## M1R-G — Governance and truth
- **M1R-G1** State claims distinguish merged, candidate, locally verified, CI-verified, externally blocked, and released.
- **M1R-G2** Ignored, skipped, unavailable, or workspace-only checks never satisfy a gate.
- **M1R-G3** Every phase records base/head SHA, PR, requirements, commands, CI links, residual risks, and independent review.
- **M1R-G4** Planning branches contain planning only after bootstrap; implementation uses isolated worktrees and PRs.
- **M1R-G5** Publishing/tagging requires exact-SHA owner authorization.

## M1R-C — Backend contract closure
- **M1R-C1** Supported synchronization consumes `DeltaBatch` through independent adapter cursors; no supported path destructively drains before acknowledgement.
- **M1R-C2** Canonical `Model` contains no transient solve policy.
- **M1R-C3** Every requested option/capability is applied, adjusted with reason, or rejected.
- **M1R-C4** Adapter health is explicit: ready, retryable, rebuild-required, terminal.
- **M1R-C5** Snapshot rebuild and complete incremental application are observationally equivalent.
- **M1R-C6** Public status/error/solution contracts preserve incumbent, proof, limits, interruption, ambiguity, native code, operation, and recoverability.
- **M1R-C7** Legacy `SolverAdapter`/`SolverModelExt` is removed or retained only as a safe, loudly deprecated shim with no destructive semantics.
- **M1R-C8** All P1/P2 characterization tests execute or are deleted with requirement-backed disposition; none remain ignored without an external impossibility.

## M1R-H — HiGHS qualification
- **M1R-H1** `highs-sys` or another maintained generated binding is the sole ABI owner.
- **M1R-H2** Construction is fallible and typed; no normal missing-library/configuration path panics.
- **M1R-H3** Every native return code, pointer, length, index width, callback, and lifecycle transition is checked.
- **M1R-H4** `Send`/`Sync` claims are minimal, documented, and tested where executable.
- **M1R-H5** Snapshot, every admitted delta operation, rebuild, solve negotiation, solve, and extraction implement the frozen contract.
- **M1R-H6** Status mapping preserves infeasible-or-unbounded ambiguity and feasible-but-not-proven outcomes.
- **M1R-H7** Semi-continuous and unsupported-domain paths cannot partially apply then lose replayability.
- **M1R-H8** Backend version, build mode, index width, and effective solve configuration are queryable.

## M1R-Q — Native evidence
- **M1R-Q1** ReferenceBackend and HiGHS run the same parameterized conformance suite.
- **M1R-Q2** Seeded mutation traces prove incremental-vs-rebuild equivalence over all admitted operations.
- **M1R-Q3** Failure injection covers every multi-call apply boundary and deterministic recovery.
- **M1R-Q4** Multi-adapter lag/catch-up and independent cursors are verified.
- **M1R-Q5** Objective offsets, primal values, duals, reduced costs, basis/hot-start behavior, statuses, and option negotiation have focused tests.

## M1R-P — Platform and packaging
- **M1R-P1** Core and HiGHS mandatory matrices cover Linux x86_64, macOS arm64/x86_64 where available, Windows x86_64, stable, and MSRV.
- **M1R-P2** Bundled/static default and explicit system-discovery mode have clean target-aware behavior.
- **M1R-P3** Packed `.crate` archives build and run in fresh consumers; workspace paths are forbidden in release evidence.
- **M1R-P4** docs.rs topology does not require commercial SDKs or maintainer-machine paths.
- **M1R-P5** fmt, clippy, tests, rustdoc, semver, audit, deny, machete, package contents, licenses, provenance, and SBOM gates pass.
- **M1R-P6** Sanitizer/fuzz/property/unsafe-focused checks run on an explicit cadence.

## M1R-E — Performance and ergonomics
- **M1R-E1** Benchmarks isolate construction, propagation, delta compilation, native apply, rebuild, solve, and extraction.
- **M1R-E2** Every benchmark records dataset, seed, machine, compiler, backend version, and statistical method.
- **M1R-E3** Bulk paths require scalar equivalence and measured benefit.
- **M1R-E4** Performance work cannot weaken correctness, replayability, or error classification.
- **M1R-E5** Public examples cover initial solve, parameter update, structural update, failure/rebuild, and requested/effective configuration.

## M1R-M/X — Commercial adapters
- **M1R-M1** MOSEK uses the official Rust API and never mutates task/environment illegally in callbacks.
- **M1R-M2** MOSEK compile/load/license/solve failures are distinct and protected CI is available before support claims.
- **M1R-X1** Xpress binding redistribution/legal decision is recorded before generated bindings are distributed.
- **M1R-X2** Xpress lifecycle and bulk/scalar equivalence pass the common contract before support claims.
- **M1R-MX3** Commercial crates remain `publish = false` and non-blocking until their independent gates pass.

## M1R-R — Release
- **M1R-R1** `roml` and `roml-highs` versions, features, MSRV, support matrix, changelog, migration guide, and release notes are frozen.
- **M1R-R2** Independent architecture/API, native-safety, correctness, and release-operations reviews have no unresolved blocker.
- **M1R-R3** Publish order is `roml` then `roml-highs` using exact released dependency versions.
- **M1R-R4** Tag, checksums, SBOM, package archives, CI evidence, and release notes refer to the same exact commit.
- **M1R-R5** Post-release patch, compatibility, security, and deprecation processes are active.

## Strategic milestone admission requirements
- **M2-A1** M1R published and at least one patch/rehearsal validates release operations.
- **M3-A1** M2 model/interchange identities are stable enough for persistence.
- **M4-A1** Versioned external identity exists independently of arena slots and Rust layout.
- **M5-A1** Multiple public releases and external usage provide evidence for 1.0 stability decisions.
