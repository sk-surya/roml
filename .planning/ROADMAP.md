# ROML Program Roadmap v2

**Planning date:** 2026-07-17  
**Canonical planning branch:** `planning/roml-ultra-mega-roadmap-v2`  
**Main baseline:** `main@ef37c88a6d80775ea69d2ccb986655edeb5789ec`  
**Inherited M1 candidate:** `planning/roml-M1-native-backends-release`, 20 commits ahead of main when this packet was created  
**Design authority:** `docs/superpowers/specs/2026-07-17-roml-ultra-mega-program-design.md`

## Program objective

Turn ROML from a verified solver-free incremental modeling kernel into a released, cross-platform optimization modeling system with one production-qualified open-source backend, independently gated commercial adapters, industrial modeling completeness, persistent incremental sessions, language boundaries, and an evidence-backed path to 1.0.

## Truth rule

Phase state is accepted only when code, tests, external environment, CI, package artifacts, and independent review all support the claim. A document saying “complete” is not evidence by itself. Ignored, skipped, unavailable, or workspace-only checks are not passing gates.

## Program milestone graph

```text
Merged predecessor: v0.1 solver-free core hardening (PR #3)
                  |
                  v
ROML-M1R Truth Reset + HiGHS Qualification + v0.1 Release
                  |
                  v
ROML-M2 Industrial Modeling Completeness
                  |
                  v
ROML-M3 Persistent Incremental Runtime
                  |
                  v
ROML-M4 Language and Ecosystem Boundary
                  |
                  v
ROML-M5 1.0 Stability and Governance
```

Commercial adapters are independent side trains:

```text
M1R contract freeze -> MOSEK qualification -> optional commercial release
                    -> Xpress qualification -> optional commercial release
```

Neither commercial train blocks `roml` + `roml-highs`.

# Milestone ROML-M1R — Truth Reset, Native HiGHS Qualification, and v0.1 Release

## Mission

Audit the current unmerged M1 candidate, repair the gap between the new revisioned protocol and the still-public legacy solver path, qualify HiGHS against the actual protocol on supported platforms, and publish only after exact-SHA evidence and owner authorization.

## M1R phase graph

```text
M1R-00 Truth reset and candidate admission
  -> M1R-01 Backend contract migration closure
       -> M1R-02 HiGHS projection/session rewrite
            -> M1R-03 Native differential and fault qualification
                 -> M1R-04 Cross-platform and package qualification
                      -> M1R-05 Performance and ergonomics acceptance
                           -> M1R-08 Release candidate and publication
                                -> M1R-09 Post-release operations

M1R-01 -> M1R-06 MOSEK independent track [non-blocking]
M1R-01 -> M1R-07 Xpress independent track [non-blocking]
```

## Phase M1R-00 — Truth reset and candidate admission

**Goal:** establish the exact candidate state and prevent stale completion claims from driving implementation.

**Critical work:**

- inventory every commit/file in the inherited M1 candidate;
- reconcile its M1.0-M1.5 completion claims against source and executable evidence;
- classify all 11 ignored tests by current behavior and requirement;
- verify license file authorization and crates.io name status separately;
- identify planning/implementation branch contamination and define replay/split strategy;
- produce requirement-level disposition: accepted, partially satisfied, failed, external-blocked, or superseded;
- freeze the M1R base SHA and evidence manifest.

**Gate:** no contradiction remains between `.planning` state, source, tests, CI, and known skipped checks. Candidate work is admitted task-by-task, not wholesale.

**Packet:** `.planning/phases/00-truth-reset-and-candidate-admission/phase.md`

## Phase M1R-01 — Backend contract migration closure

**Goal:** make the revisioned snapshot/delta/session contract the supported public execution path and retire destructive legacy behavior.

**Critical work:**

- define the final public `BackendAdapter`/`BackendSession` surface;
- migrate synchronization from `Change` drain to `DeltaBatch` + cursor acknowledgement;
- remove model-owned transient solve options;
- replace best-effort silent option handling with explicit negotiation;
- unify status/error/solution types; preserve a compatibility shim only if its semantics are safe and loudly deprecated;
- parameterize the conformance harness for real adapters;
- remove or close every ignored P1/P2 characterization test;
- review public docs/examples against the new path.

**Gate:** supported solve calls cannot lose deltas or policy; no supported API silently ignores options; all required contract tests execute, not ignore.

**Packet:** `.planning/phases/01-backend-contract-migration-closure/phase.md`

## Phase M1R-02 — HiGHS projection/session rewrite

**Goal:** make `roml-highs` a safe implementation of the frozen backend contract using authoritative bindings.

**Critical work:**

- use pinned `highs-sys` as the ABI owner;
- replace panic constructors with typed fallible construction;
- implement snapshot rebuild, typed delta apply, cursor/health transitions, request negotiation, solve, and solution views;
- check every native return code and pointer/length assumption;
- define `Send`/`Sync` precisely and remove unjustified unsafe implementations;
- implement correct statuses including infeasible-or-unbounded ambiguity;
- expose HiGHS version/build/configuration metadata;
- support callbacks only through official interfaces for the pinned version.

**Gate:** no handwritten ABI, no panic-based normal construction, no legacy-only adapter implementation, and unsafe review has no unresolved blocker.

**Packet:** `.planning/phases/02-highs-projection-session-rewrite/phase.md`

## Phase M1R-03 — Native differential and fault qualification

**Goal:** prove that HiGHS incremental behavior equals rebuild behavior and that native failures preserve recovery.

**Critical work:**

- run the shared contract suite against ReferenceBackend and HiGHS;
- generate seeded legal mutation traces over all admitted operations;
- compare normalized native model state where queryable and solve observables otherwise;
- inject failures at each multi-call apply boundary;
- verify multi-cursor lag/catch-up;
- close semi-continuous partial-apply recovery;
- validate statuses, requested/effective options, objective offsets, primal/dual/reduced-cost extraction, and basis lifecycle;
- archive every failing seed before fixing it.

**Gate:** commuting-square and deterministic rebuild laws pass for HiGHS; no fault loses replayability; unsupported capabilities reject before ambiguous partial application.

**Packet:** `.planning/phases/03-native-differential-fault-qualification/phase.md`

## Phase M1R-04 — Cross-platform and package qualification

**Goal:** prove that supported users can build, load, package, and consume ROML + HiGHS without maintainer-machine assumptions.

**Critical work:**

- Linux x86_64, macOS arm64 and x86_64 where runners exist, Windows x86_64, MSRV;
- bundled/static default and explicit discovered-system mode;
- target-aware build scripts and clean diagnostics;
- packed `.crate` consumer projects, locked dependencies, examples, docs, semver, audit, deny, machete;
- docs.rs-compatible feature topology;
- scheduled property/fuzz/sanitizer checks;
- artifact and provenance capture.

**Gate:** required matrix is green on clean runners; package consumers use archives rather than workspace paths; support matrix matches evidence.

**Packet:** `.planning/phases/04-cross-platform-package-qualification/phase.md`

## Phase M1R-05 — Performance and ergonomics acceptance

**Goal:** establish a correct, reproducible baseline and remove release-blocking API friction without expanding scope.

**Critical work:**

- separate model construction, parameter propagation, delta compile, native apply, rebuild, solve, and extraction benchmarks;
- benchmark repeated reoptimization and basis reuse;
- measure bulk vs scalar projection with equivalence tests;
- record machine, solver, compiler, dataset, seed, and statistical method;
- profile allocation/memory for sparse models;
- run user journeys from public docs and packed crates;
- document pre-M1 migration and explicit unsupported features.

**Gate:** no performance change weakens correctness or recovery; claims are workload-scoped and reproducible; release examples use supported APIs only.

**Packet:** `.planning/phases/05-performance-ergonomics-acceptance/phase.md`

## Phase M1R-06 — MOSEK independent qualification

**Goal:** migrate MOSEK to the official Rust API and honest backend semantics without blocking the open-source release.

**Critical work:**

- official `mosek` crate/API; remove handwritten FFI/build duplication;
- fallible environment/task/license lifecycle;
- no task mutation inside callbacks;
- declare lazy constraints/user cuts unsupported unless official semantics prove otherwise;
- full contract/differential/fault suite;
- protected licensed CI separating compile/load/license/solve;
- independent publish decision.

**Gate:** all M1R requirements for a supported commercial backend pass. Otherwise remain `publish = false` and experimental.

**Packet:** `.planning/phases/06-mosek-independent-qualification/phase.md`

## Phase M1R-07 — Xpress independent qualification

**Goal:** resolve binding redistribution and qualify Xpress without contaminating the main release path.

**Critical work:**

- obtain and record the binding/header redistribution decision;
- select generated sys crate, runtime loader, official binding, or local-only policy;
- process-safe init/free and license discovery;
- migrate legacy `Change` path and bulk adjacency assumptions to typed deltas;
- prove bulk/scalar and incremental/rebuild equivalence;
- protected licensed CI;
- independent publish decision.

**Gate:** legal, binding, semantic, lifecycle, and CI gates all pass. Otherwise remain unpublished.

**Packet:** `.planning/phases/07-xpress-independent-qualification/phase.md`

## Phase M1R-08 — Release candidate and publication

**Goal:** produce and, only with owner authorization, publish a defensible v0.1 release.

**Critical work:**

- freeze exact versions/features/MSRV/support matrix;
- verify packed consumers in dependency order;
- run independent correctness, API, unsafe/FFI, security, documentation, and release-operations reviews;
- generate checksums, SBOM, changelog, migration guide, release notes, and evidence index;
- publish `roml`, verify crates.io/docs.rs, then publish `roml-highs` against the exact released core version;
- tag only the exact published commit.

**Gate:** all mandatory checks green, no unresolved P0/P1 finding, exact-SHA owner authorization recorded.

**Packet:** `.planning/phases/08-release-candidate-publication/phase.md`

## Phase M1R-09 — Post-release operations

**Goal:** make the release maintainable rather than a one-time event.

**Critical work:**

- compatibility matrix against supported HiGHS versions;
- patch/backport/deprecation/security policies;
- issue templates that capture OS, target, ROML/solver versions, features, build mode, and diagnostics;
- release rollback/yank procedure;
- scheduled dependency and native-version probes;
- first patch-cycle retrospective feeding M2 admission.

**Gate:** operational owners and recurring checks exist; M2 admission decision is evidence-based.

**Packet:** `.planning/phases/09-post-release-operations/phase.md`

# Milestone ROML-M2 — Industrial Modeling Completeness

**Admission:** M1R release and at least one patch-cycle retrospective.

## M2 phases

- **M2-00 Usage-driven requirements reset:** collect real user/maintainer friction; freeze scope.
- **M2-01 Names, metadata, and stable external identity:** names/tags/metadata without coupling serialized identity to arena slots.
- **M2-02 Sparse bulk construction and matrix views:** CSR/CSC/triplet ingestion, bulk bounds/types, deterministic inspection.
- **M2-03 LP/MPS interchange:** import/export round trips, names, objective sense/offset, integer/domain semantics, diagnostics.
- **M2-04 Advanced linear/MIP constructs:** SOS1/SOS2, indicator constraints, semi-integer, capability fallback policy; no silent reformulation.
- **M2-05 Basis, warm starts, and solution starts:** solver-neutral request/result contracts with backend-specific capability evidence.
- **M2-06 Diagnostics and infeasibility:** activity/slack, bound violations, IIS/conflict capability, model explain reports.
- **M2-07 Model transformation layer:** explicit, inspectable transformations with source mapping and reversible diagnostics.
- **M2-08 v0.2 qualification:** semver/migration/package/backend evidence.

**Exit:** industrial LP/MILP models can be built/imported, inspected, diagnosed, incrementally solved, and round-tripped without hidden transformations.

# Milestone ROML-M3 — Persistent Incremental Runtime

**Admission:** M2 stable identity and interchange contracts.

## M3 phases

- **M3-00 Session architecture freeze**
- **M3-01 Journal retention, checkpoints, and compaction**
- **M3-02 Crash-safe session persistence and replay**
- **M3-03 Structured cancellation, progress, and event streams**
- **M3-04 Shadow backend verification and divergence reports**
- **M3-05 Parallel solve portfolios without canonical-state races**
- **M3-06 Large-model memory and throughput engineering**
- **M3-07 Service/process boundary feasibility**
- **M3-08 v0.3 qualification**

**Exit:** long-lived solve workflows can persist, recover, replay, compare backends, and operate under explicit resource/cancellation contracts.

# Milestone ROML-M4 — Language and Ecosystem Boundary

**Admission:** M3 stable session and serialization identity.

## M4 phases

- **M4-00 Versioned external model/session format**
- **M4-01 C ABI design and threat model**
- **M4-02 `roml-c-api` opaque handles and ownership**
- **M4-03 Generated headers and ABI compatibility tests**
- **M4-04 Python package and ergonomic modeling layer**
- **M4-05 Java/.NET feasibility and FFI benchmarks**
- **M4-06 Binary distribution and native dependency packaging**
- **M4-07 Cross-language conformance corpus**
- **M4-08 v0.4 qualification**

**Exit:** Python and future wrappers consume a versioned boundary, not Rust internals.

# Milestone ROML-M5 — 1.0 Stability and Governance

**Admission:** multiple public releases and external usage evidence.

## M5 phases

- **M5-00 Stable-surface inventory and deprecation closure**
- **M5-01 Semver and compatibility policy freeze**
- **M5-02 Backend support-tier governance**
- **M5-03 Performance regression budgets and benchmark lab**
- **M5-04 Security, unsafe, and supply-chain audit**
- **M5-05 Maintainer/reviewer ownership and release automation**
- **M5-06 Documentation and migration consolidation**
- **M5-07 1.0 release candidate and ecosystem validation**
- **M5-08 1.0 publication and long-term support operations**

**Exit:** ROML can make a precise, evidence-backed stable API and support promise.

# Parallel execution policy

## Before M1R-01 merge

Only truth audit, test classification, external binding research, CI log inspection, and documentation inventory may run in parallel. No adapter implementation may define its own contract.

## After M1R-01 contract merge

- H: HiGHS implementation
- D: differential/fault harness adaptation
- C: CI/package qualification
- P: benchmark infrastructure
- M: MOSEK spike
- X: Xpress legal/binding spike
- V: independent verifier
- I: coordinator/integrator

Shared contract files have one owner. Backend workers rebase after contract changes.

# Program stop conditions

Stop and escalate when an official solver contract contradicts the generic abstraction; an ignored test is required for a gate; replayability can be lost; a callback requires unsupported mutation/reentrancy; binding redistribution is unresolved; a build embeds machine paths; a performance change weakens semantics; crate ownership or publication order is unresolved; or publication lacks exact-SHA owner authorization.

# Program definition of done

The program reaches ROML 1.0 only when each admitted milestone has requirement traceability, executable evidence, independent review, honest support labels, reproducible packages, and operational ownership. Future milestone text is not permission to bypass the current milestone gate.

### Phase 9: Truth reset and candidate admission

**Goal:** establish the exact candidate state and prevent stale completion claims from driving implementation
**Requirements**: M1R-G1–G5
**Depends on:** None
**Plans:** 5 plans

Plans:

- [ ] 09-01-PLAN.md — Candidate State Evidence Foundation (commit inventory, license, crates.io)
- [ ] 09-02-PLAN.md — M1 Claim Reconciliation and Source Audit (M1.0-M1.5 claims, legacy patterns)
- [ ] 09-03-PLAN.md — Test Classification and Contamination Analysis (11 ignored tests, branch split)
- [ ] 09-04-PLAN.md — Fix and Pin All 11 Ignored Tests (fix P1-1/P1-2, pin 9 for M1R-01)
- [ ] 09-05-PLAN.md — Compile M1R-00-ADMISSION.md and Freeze Base SHA (admission report, state freeze)

### Phase 10: Backend contract migration closure

**Goal:** make the revisioned snapshot/delta/session contract the supported public execution path and retire destructive legacy behavior
**Requirements**: M1R-C1–C8
**Depends on:** Phase 9
**Plans:** 0 plans

Plans:

- [ ] TBD (run /gsd-plan-phase 10 to break down)

### Phase 11: HiGHS projection/session rewrite

**Goal:** make roml-highs a safe implementation of the frozen backend contract using authoritative bindings
**Requirements**: M1R-H1–H8
**Depends on:** Phase 10
**Plans:** 0 plans

Plans:

- [ ] TBD (run /gsd-plan-phase 11 to break down)

### Phase 12: Native differential and fault qualification

**Goal:** prove that HiGHS incremental behavior equals rebuild behavior and that native failures preserve recovery
**Requirements**: M1R-Q1–Q5
**Depends on:** Phase 11
**Plans:** 0 plans

Plans:

- [ ] TBD (run /gsd-plan-phase 12 to break down)

### Phase 13: Cross-platform and package qualification

**Goal:** prove that supported users can build, load, package, and consume ROML + HiGHS without maintainer-machine assumptions
**Requirements**: M1R-P1–P6
**Depends on:** Phase 12
> Infrastructure may prepare in parallel, but gate blocked by M1R-03
**Plans:** 0 plans

Plans:

- [ ] TBD (run /gsd-plan-phase 13 to break down)

### Phase 14: Performance and ergonomics acceptance

**Goal:** make ROML understandable, measurable, and honest in performance, ergonomics, and support claims
**Requirements**: M1R-E1–E5
**Depends on:** Phase 12
**Plans:** 0 plans

Plans:

- [ ] TBD (run /gsd-plan-phase 14 to break down)

### Phase 15: Commercial backend tracks (MOSEK + Xpress)

**Goal:** qualify MOSEK and Xpress without contaminating or blocking the main release — non-blocking side track
**Requirements**: M1R-M1–M2, M1R-X1–X2, M1R-MX3
**Depends on:** Phase 10
**Plans:** 0 plans

Plans:

- [ ] TBD (run /gsd-plan-phase 15 to break down)

### Phase 16: Release candidate and publication

**Goal:** produce and, only with owner authorization, publish a defensible v0.1 release
**Requirements**: M1R-R1–R4
**Depends on:** Phase 14, Phase 15
**Plans:** 0 plans

Plans:

- [ ] TBD (run /gsd-plan-phase 16 to break down)

### Phase 17: Post-release operations

**Goal:** make the release maintainable rather than a one-time event
**Requirements**: M1R-R5
**Depends on:** Phase 16
**Plans:** 0 plans

Plans:

- [ ] TBD (run /gsd-plan-phase 17 to break down)
