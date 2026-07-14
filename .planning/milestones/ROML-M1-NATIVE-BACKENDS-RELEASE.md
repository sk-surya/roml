# ROML-M1 — Native Backend Qualification and Public Release

**Base:** `main@ef37c88a6d80775ea69d2ccb986655edeb5789ec`
**Predecessor:** v0.1 core hardening, PR #3
**Mission:** turn the verified solver-free kernel into a genuinely usable, cross-platform optimization library with one production-qualified open-source backend, independently gated commercial backends, reproducible performance evidence, and a controlled crates.io release.

## Strategic thesis

The core is now structurally credible. The next bottleneck is the morphism from canonical ROML state into native solver state. M1 therefore treats each backend as a projection functor from the same revisioned model category into a solver-specific state machine. Correctness means the following square commutes for every supported mutation sequence:

```text
Model(r0) --DeltaBatch--> Model(r1)
   |                         |
 snapshot                  snapshot
   v                         v
SolverState(r0) --apply--> SolverState(r1)
```

Incremental application, full rebuild, and extracted solution observables must agree. Performance work is admitted only after this equivalence is established.

## Release train

1. `roml` — solver-independent core; already verified, receives only compatibility fixes.
2. `roml-highs` — mandatory M1 reference backend and first release backend.
3. `roml-mosek` — separately qualified; may publish only after official-binding migration, licensed CI, and callback redesign.
4. `roml-xpress` — separately qualified; remains `publish = false` until binding/licensing and runner gates pass.
5. crates.io publication — `roml` then `roml-highs`, exact verified SHAs only.

## Phase graph

```text
M1.0 Admission + license/name gates
  ├── M1.1 Backend contract freeze
  │     ├── M1.2 HiGHS binding migration
  │     │     └── M1.3 HiGHS semantic equivalence
  │     │            └── M1.4 HiGHS cross-platform qualification
  │     │                   └── M1.5 Performance and ergonomics
  │     │                          └── M1.8 Release candidate + publication
  │     ├── M1.6 MOSEK qualification [parallel, non-blocking]
  │     └── M1.7 Xpress qualification [parallel, non-blocking]
  └── M1.9 Post-release observability and compatibility
```

## Phase M1.0 — Admission and external gates

- Confirm `MIT OR Apache-2.0`; add exact license files and SPDX metadata.
- Verify crates.io ownership/availability for `roml` and `roml-highs`; reserve names without publishing functional releases if needed.
- Reconcile `.planning/STATE.md`: distinguish verified core requirements from native-backend requirements still open.
- Freeze support labels: core verified; native adapters experimental until their gates pass.
- Record current solver SDK versions, supported targets, installation modes, and redistribution constraints.
- Create milestone requirement matrix and evidence directory.

**Gate:** licenses committed; names controlled; no ambiguous “v0.1 verified” claim conflates solver-free core with native backend readiness.

## Phase M1.1 — Backend protocol and capability freeze

- Freeze `BackendAdapter`, `DeltaBatch`, `AdapterCursor`, `ApplyOutcome`, `AdapterHealth`, `SolveRequest`, `SolveResult`, and capability semantics.
- Define snapshot projection and rebuild protocol.
- Define status lattice preserving incumbent/no-incumbent, proof state, limits, interruption, ambiguous infeasible-or-unbounded, license and numerical failures.
- Define option negotiation: requested/effective/adjusted/rejected.
- Define solution extraction views without mandatory map cloning.
- Define callback taxonomy: observation, interruption, lazy constraints, user cuts, incumbent injection; no false uniformity.
- Add backend conformance harness reusable by all adapters.

**Gate:** reference backend tests prove protocol laws without native libraries; API review freezes the contract for backend agents.

## Phase M1.2 — HiGHS authoritative binding migration

- Replace handwritten FFI with pinned `rust-or/highs-sys`.
- Inventory required C APIs and callback APIs; upstream or narrowly fork only genuine gaps.
- Choose bundled static build as default; optional system discovery as explicit feature.
- Remove local ABI structs/constants and developer-specific paths/rpaths.
- Introduce typed constructor/build/load/version errors.
- Audit ownership, Drop, Send/Sync, callbacks, pointer lengths, return codes, and panic containment.
- Expose backend version/build metadata.

**Gate:** no handwritten HiGHS ABI declarations; clean builds on Linux/macOS/Windows; unsafe code localized and reviewed.

## Phase M1.3 — HiGHS semantic and recovery qualification

- Full snapshot projection tests for LP, MILP, ranged rows, objective offsets, variable domains, activation/deactivation, deletes, and objective switching.
- Incremental-vs-rebuild differential tests over generated mutation traces.
- Multi-adapter cursor and lag/catch-up tests.
- Fault injection around every native apply sub-step; classify clean failure vs dirty/rebuild-required.
- Close the semi-continuous unsupported partial-apply counterexample without lost deltas.
- Status/error mapping conformance tests.
- Solve-request negotiation tests.
- Callback support only where officially valid; unsupported capabilities explicitly rejected.
- Basis/hot-start lifecycle tests where supported.

**Gate:** commuting-square property holds for all admitted operations; no ignored native errors; rebuild recovers every dirty state.

## Phase M1.4 — HiGHS platform qualification

- GitHub Actions: Ubuntu x86_64, macOS arm64/x86_64 where available, Windows x86_64, MSRV.
- Bundled/static and optional discovered-system build modes.
- Fresh consumer projects built from packed `.crate` archives.
- docs.rs-compatible feature topology.
- Cross-compilation diagnostics and target-vs-host build-script tests.
- Sanitizer/Miri/fuzz lanes for safe boundary and generated mutation harness.
- Supply-chain, license, audit, deny, semver and package-content checks.

**Gate:** mandatory matrix green from clean runners; package consumers require no maintainer-machine paths.

## Phase M1.5 — Performance, bulk projection, and user ergonomics

- Criterion or equivalent benchmarks separating model build, parameter propagation, delta compilation, native apply, rebuild, solve, and extraction.
- Fixed datasets/seeds and machine metadata.
- Bulk row/column/matrix projection APIs where evidence shows FFI-call dominance.
- Warm-start and repeated-reoptimization studies.
- Memory/allocation profiling for large sparse models.
- End-to-end examples: initial solve, parameter update, structural update, failure/rebuild, multi-backend shadow verification.
- API friction review and migration guide from pre-M1 adapter APIs.

**Gate:** no correctness regression; performance claims reproducible; documented workload-specific tradeoffs.

## Phase M1.6 — MOSEK qualification [non-blocking]

- Replace handwritten C FFI/constants with official `mosek` Rust API.
- Redesign callback flow; never mutate task/environment illegally inside callbacks.
- Establish environment/task/license lifecycle and thread-safety policy.
- Implement contract and differential suite.
- Protected licensed CI with compile/load/license/solve separated.
- Decide publish/support status independently.

## Phase M1.7 — Xpress qualification [non-blocking]

- Complete legal/redistribution memo for generated bindings.
- Select generated link-time sys crate or runtime loading.
- Formalize process initialization/free and license discovery.
- Migrate characterized bulk-additive path onto typed delta operations.
- Prove bulk-vs-scalar and incremental-vs-rebuild equivalence.
- Protected licensed CI and independent publish decision.

## Phase M1.8 — Release candidate and publication

- Freeze exact versions, dependency order, MSRV, features, support matrix, changelog, migration guide and release notes.
- Build/test from `.crate` archives in fresh consumers.
- Independent principal-engineer, FFI-safety, API-semver and release-operations reviews.
- Publish `roml`, verify crates.io/docs.rs, then publish `roml-highs` against the exact released core version.
- Tag exact verified merge commit; archive evidence/checksums/SBOM.
- No publication without explicit owner authorization.

## Phase M1.9 — Post-release operations

- Compatibility matrix against new HiGHS releases.
- Issue templates with environment/backend metadata.
- Release telemetry limited to public CI/download/issue signals; no hidden runtime collection.
- Patch-release policy, backport process, security response, and deprecation window.

## Parallelism model

After M1.1 contract freeze:

- Agent H1: HiGHS binding/build migration.
- Agent H2: HiGHS semantic projection and differential harness.
- Agent H3: platform/package CI.
- Agent P: benchmark and profiling infrastructure.
- Agent M: MOSEK official-binding spike.
- Agent X: Xpress legal/binding spike.
- Verifier V: independent conformance/fault-injection review; authors no backend implementation.
- Coordinator C: contract ownership, integration, phase gates, evidence reconciliation.

Do not parallelize edits to the backend contract. Backend agents consume a frozen interface.

## Milestone exit criteria

- `roml` and `roml-highs` published from exact verified commits.
- HiGHS is correct under snapshot, incremental, failure/rebuild, status, option, and solution contracts on Linux/macOS/Windows.
- Native binding ownership is authoritative and maintainable.
- Commercial adapters have honest independent status.
- All claims are backed by archived commands, CI, package-consumer tests, and independent review.
