# ROML Ultra-Mega Program Design

**Date:** 2026-07-17  
**Planning branch:** `planning/roml-ultra-mega-roadmap-v2`  
**Inherited candidate base:** `planning/roml-M1-native-backends-release`, 20 commits ahead of `main@ef37c88a6d80775ea69d2ccb986655edeb5789ec` at packet creation  
**Execution method:** GSD controls milestones, phases, state, requirements, and evidence. Superpowers controls design, TDD, worktree isolation, debugging, implementation, independent review, and verification.

## 1. Executive decision

ROML will use a two-depth roadmap:

1. **Executable near-term:** ROML-M1R, a release-completion milestone that audits and repairs the current M1 candidate, completes the backend contract migration, qualifies HiGHS natively, and releases `roml` plus `roml-highs` only after exact-SHA evidence.
2. **Strategic horizon:** ROML-M2 through ROML-M5, covering industrial modeling completeness, persistent incremental runtime, language/ecosystem boundaries, and 1.0 stability. These milestones have explicit admission criteria but do not authorize implementation before M1R release evidence.

This avoids two failure modes:

- repeating the already-merged P0-P6 core hardening program;
- accepting milestone state labels that are stronger than the code actually supports.

## 2. Current-state truth

### Established baseline

PR #3 merged the solver-free hardening program into `main`. It added canonical cells, model invariants, revision/snapshot/delta types, reference synchronization, contract tests, CI, package hygiene, documentation, and release controls.

### Candidate branch

The inherited M1 candidate adds:

- license files and metadata;
- `highs-sys` migration work;
- HiGHS CI workflow;
- differential, recovery, status, and negotiation tests;
- performance benchmarks;
- MOSEK and Xpress qualification plans;
- M1 evidence documents.

### Truth gap

The candidate is not accepted as a release candidate merely because its state ledger marks M1.0-M1.5 complete. The following remain visible in source and must be reconciled:

- public `SolverAdapter` still accepts legacy `Change` slices;
- `SolverModelExt::sync_model` still drains changes before backend acknowledgement;
- `SolveOptions` remains documented as best-effort and silently ignored;
- `solve_model` still consumes solve options from canonical `Model` state;
- HiGHS still implements the legacy trait and exposes panic-based constructors;
- the 11 ignored tests cannot simultaneously be called known P1/P2 defects and treated as closed requirements;
- the planning branch contains production implementation, violating the intended separation between canonical planning and implementation lanes.

M1R begins by proving or correcting every claimed closure.

## 3. Product architecture

ROML's stable architecture is:

```text
Modeling API
    |
    v
Canonical mathematical state
    |
    +--> CanonicalSnapshot(revision)
    |
    +--> replayable DeltaBatch(from, to)
              |
              v
       BackendSession / AdapterCursor
              |
              +--> apply delta
              +--> rebuild snapshot
              +--> negotiate SolveRequest
              +--> solve
              +--> expose SolveResult / SolutionView
```

### Core responsibilities

- typed identities and coherent variable domains;
- canonical coefficient cells and parameter dependency graph;
- transactions, revisions, snapshots, journals, and deterministic deltas;
- solver-neutral capability vocabulary, solve requests, status/error model, and solution views;
- modeling ergonomics and diagnostics;
- no raw FFI, native discovery, solver license policy, or process-global initialization.

### Backend responsibilities

- map snapshots and deltas to native state;
- own independent synchronization cursor and health;
- validate requested capabilities and return applied/adjusted/rejected configuration;
- classify every native error and state transition;
- expose backend version/build metadata;
- localize unsafe code and native lifecycle.

### Release responsibilities

- build packed crates in clean consumers;
- test supported OS/toolchain/backend cells;
- preserve exact evidence for the released SHA;
- publish only crates whose independent gates pass.

## 4. Governing laws

### L1 — Commuting projection

For every admitted mutation trace:

```text
project(snapshot(r1)) == apply(project(snapshot(r0)), deltas(r0 -> r1))
```

Equality is observational: entities, bounds, types, matrix, objective, solve configuration, status semantics, and extractable solution values agree within declared numerical tolerances.

### L2 — Replayability

A model revision remains replayable until every required consumer has acknowledged it or an explicit retention/compaction policy permits removal.

### L3 — Failure monotonicity

A native failure may leave an adapter ready, retryable, rebuild-required, or terminal. It may not make canonical model history ambiguous or silently discard work.

### L4 — Explicit capability outcomes

Every requested domain, option, callback, extraction, or warm-start feature is applied, adjusted with explanation, or rejected before ambiguous execution.

### L5 — Authoritative ABI

Maintained generated/vendor bindings own ABI declarations. Adapter source does not copy function prototypes, structs, enum values, callback codes, or parameter constants when an authoritative source exists.

### L6 — Evidence hierarchy

Passing non-native core tests does not prove native backend correctness. Local native tests do not prove cross-platform qualification. A skipped or ignored test is never counted as passing.

## 5. Program milestones

### ROML-M1R — Truth Reset, Native HiGHS Qualification, and v0.1 Release

Exit: `roml` and `roml-highs` are published from an exact verified commit, or the milestone stops with a documented owner/external blocker. Commercial adapters do not block exit.

### ROML-M2 — Industrial Modeling Completeness

Admission: M1R release is operational and at least one patch cycle has validated the release process.

Outcome: named entities and metadata, sparse bulk construction, LP/MPS interchange, basis and warm-start abstractions, IIS/diagnostics, SOS/indicator capability modeling, coherent model inspection, and stable serialization design. Features graduate only when the reference backend and at least one native backend have semantics and evidence.

### ROML-M3 — Persistent Incremental Runtime

Admission: M2's model/interchange contracts are stable enough to persist and replay.

Outcome: explicit long-lived solve sessions, journal retention/checkpoint policy, crash-safe replay, shadow backend verification, structured cancellation/progress, repeated reoptimization workflows, and performance envelopes for large sparse models.

### ROML-M4 — Language and Ecosystem Boundary

Admission: Rust model/session contracts have real usage feedback and a versioned serialization identity independent of arena slots.

Outcome: `roml-c-api`, generated C headers, Python bindings first, Java/.NET feasibility, stable error/ownership ABI, and package/install automation. No wrapper binds directly to unstable Rust internals.

### ROML-M5 — 1.0 Stability and Governance

Admission: core plus HiGHS have multiple public releases, migration evidence, and external usage.

Outcome: semver-frozen stable surface, deprecation policy, backend compatibility matrix, performance regression governance, security response, maintainer/reviewer ownership, release automation, and a defensible 1.0 support statement.

## 6. M1R phase design

```text
M1R-00 Truth reset and candidate admission
  -> M1R-01 Backend contract migration closure
       -> M1R-02 HiGHS projection/session rewrite
            -> M1R-03 Native differential and fault qualification
                 -> M1R-04 Cross-platform/package qualification
                      -> M1R-05 Performance and ergonomics acceptance
                           -> M1R-08 Release candidate and publication
                                -> M1R-09 Post-release operations

M1R-01 -> M1R-06 MOSEK independent track [non-blocking]
M1R-01 -> M1R-07 Xpress independent track [non-blocking]
```

### Phase ownership

- Coordinator owns shared contracts, state, integration order, and evidence acceptance.
- Backend implementers own one adapter only.
- CI/package worker owns workflows and fresh-consumer harnesses, not adapter semantics.
- Benchmark worker cannot weaken correctness/recovery contracts.
- Independent verifier does not author implementation under review.
- Owner alone authorizes publishing, crate names, licenses, and protected commercial runners.

## 7. Branch and PR topology

- `planning/roml-ultra-mega-roadmap-v2` is planning-only after this packet is committed.
- Current inherited M1 implementation is treated as candidate evidence and must be split or replayed into reviewed implementation branches.
- Each phase uses an isolated worktree and a draft PR.
- Shared contract changes have one integration owner and merge before backend implementation branches rebase.
- Commercial adapters use separate PR trains and never merge unfinished support claims into the public release path.
- Release PR contains no unrelated refactor or new feature.

## 8. Testing architecture

### Layer A — Core laws

Solver-free deterministic and property tests for snapshots, deltas, journal/cursor behavior, transactions, capability negotiation, statuses, errors, and solution views.

### Layer B — Backend conformance

A parameterized harness runs identical semantic cases against ReferenceBackend and each admitted native backend.

### Layer C — Differential traces

Seeded legal mutation traces compare incremental application with full rebuild. Every failure records seed, operation index, revision pair, backend version, and normalized state difference.

### Layer D — Fault injection

Inject failures before, during, and after native sub-operations. Verify cursor/health classification, retained replay, and deterministic recovery.

### Layer E — Consumer/package tests

Build and run examples from `.crate` archives in fresh projects on supported platforms. Workspace-path success is insufficient.

### Layer F — Performance evidence

Measure construction, parameter propagation, delta compilation, native apply, rebuild, solve, and extraction separately. Correctness gates run before benchmark comparison.

## 9. Release scope

M1R stable candidates:

1. `roml`
2. `roml-highs`

Independent experimental tracks:

- `roml-mosek`, unpublished until official API migration, legal callback behavior, protected CI, and full conformance.
- `roml-xpress`, unpublished until binding redistribution decision, lifecycle safety, protected CI, and full conformance.

No crate publication is implied by plan completion. Publication requires exact-SHA owner authorization after release evidence.

## 10. Stop conditions

Stop the affected lane when:

- code contradicts a claimed completed requirement;
- an ignored/skipped test is required for a gate;
- the backend contract cannot represent a solver's documented semantics without false uniformity;
- a native failure can lose replayability;
- a callback requires unsupported reentrancy or model mutation;
- a binding or generated artifact has unresolved redistribution rights;
- a build depends on maintainer-machine paths or undeclared environment state;
- a performance optimization changes semantics or recovery behavior;
- release metadata, crate ownership, or dependency order is unresolved;
- an agent proposes publication without owner authorization.

## 11. Success criteria

The program design succeeds when:

- canonical planning state matches code and evidence;
- legacy destructive synchronization is removed from the supported public path;
- HiGHS implements the frozen session/projection contract;
- native differential/fault tests pass across supported platforms;
- packed consumer projects work without repository knowledge;
- release claims distinguish core, HiGHS, and commercial adapter support;
- exact release evidence is archived;
- later milestones cannot begin by silently expanding the v0.1 release scope.
