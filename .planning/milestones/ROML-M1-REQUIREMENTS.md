# ROML-M1 Requirements

Every task/PR/evidence record cites these IDs.

## M1-R0 Governance
- R0.1 Dual license confirmed and packaged.
- R0.2 Crate names controlled; publish requires exact-SHA owner authorization.
- R0.3 Core/backend support labels are honest and independently gated.

## M1-R1 Backend contract
- R1.1 Canonical snapshot and typed delta projections are versioned.
- R1.2 Apply outcomes preserve adapter health and recovery policy.
- R1.3 Capabilities cover operations, model class, callbacks, warm starts, extraction and options.
- R1.4 Solve policy is immutable per attempt and reports effective configuration.
- R1.5 Status/error models preserve native information and incumbent/proof state.

## M1-R2 HiGHS binding safety
- R2.1 Maintained/generated authoritative bindings replace handwritten ABI.
- R2.2 Bundled and discovery build modes have one `links` owner.
- R2.3 Target-aware build logic supports Linux/macOS/Windows.
- R2.4 Constructors, calls, callbacks and destruction are typed, checked and panic-safe.
- R2.5 Backend version/build metadata is queryable.

## M1-R3 HiGHS correctness
- R3.1 Snapshot projection covers every admitted model construct.
- R3.2 Incremental application is observationally equivalent to rebuild.
- R3.3 Failure injection loses no delta and deterministic rebuild restores state.
- R3.4 Status, options, objective offsets, solutions, duals and reduced costs are correct.
- R3.5 Unsupported domains/callbacks/options are rejected before ambiguous partial application.
- R3.6 Multiple adapter cursors independently lag and catch up.

## M1-R4 Qualification
- R4.1 Clean CI runners pass Linux, macOS, Windows and MSRV.
- R4.2 Packed-crate consumer tests pass.
- R4.3 docs.rs topology needs no unavailable commercial SDK.
- R4.4 Unsafe/fuzz/sanitizer/property checks execute on schedule.
- R4.5 Dependency, license, audit, semver and package checks pass.

## M1-R5 Performance
- R5.1 Reproducible benchmarks isolate construction, propagation, compile, apply, rebuild, solve and extraction.
- R5.2 Bulk paths are admitted only with scalar equivalence and measured benefit.
- R5.3 Reoptimization/warm-start claims include solver/version/machine evidence.
- R5.4 No performance optimization weakens recovery or canonical correctness.

## M1-R6 Commercial backends
- R6.1 MOSEK uses official bindings and legal callback/lifecycle semantics.
- R6.2 Xpress has an approved binding and redistribution decision.
- R6.3 Licensed CI separates compile/load/license/solve failures and protects secrets/artifacts.
- R6.4 Commercial crates remain unpublished until their own gates pass.

## M1-R7 Release
- R7.1 Release candidate is tested from archives in fresh consumers.
- R7.2 Independent API, FFI, correctness and release reviews have no unresolved blockers.
- R7.3 `roml` publishes before `roml-highs`; exact versions are used.
- R7.4 Tag, checksums, SBOM, evidence and release notes correspond to the exact published commit.
- R7.5 Post-release compatibility, security and patch processes are active.
