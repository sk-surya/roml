# ROML Mega Roadmap — Production-Grade Public Release

**Program base:** `main@f9ba1921e650b5057bbc4de090a78391f7932a53`  
**Execution model:** GSD milestone/phase control + Superpowers TDD, debugging, verification, and review.  
**Release rule:** no crate publication until Phase 6 has passed and the owner explicitly authorizes release.

## Dependency graph

```text
P0 Baseline and hygiene
 ├──> P1 Core semantic correctness
 │     └──> P2 Revisioned synchronization
 │            └──> P3 Solver boundary and FFI hardening
 │                   └──> P4 Cross-platform CI and backend qualification
 │                          └──> P5 Public API, documentation, packaging
 │                                 └──> P6 Release qualification and staged publication
 └──────────────────────────────────────────────────────────────┘

P7 Foreign-language ABI foundation begins only after P6 and is not part of v0.1.
```

P1 and selected P4 infrastructure tasks may proceed in parallel after P0, but P3 adapter rewrites must target the P2 synchronization contract rather than preserving the current destructive changelog API.

## Release slices

### Slice A — Trustworthy core

Phases P0–P2. Outcome: a solver-independent crate whose canonical model semantics and revision protocol are correct, portable, and testable without native libraries.

### Slice B — Reference backend

Phases P3–P4, initially HiGHS only. Outcome: generated/maintained bindings, safe lifecycle, portable native build, and incremental-vs-rebuild equivalence on Linux/macOS/Windows.

### Slice C — Public package

Phases P5–P6. Outcome: curated API, complete documentation, reproducible package contents, release evidence, and staged crates.io publication.

### Slice D — Commercial adapters

MOSEK and Xpress graduate independently. They must not block the core/HiGHS release, and they must not inherit a “supported” label merely because they compile on one licensed macOS machine.

## Phase 0 — Baseline, repository hygiene, and release controls

**Goal:** establish a reproducible baseline, remove accidental artifacts, define package ownership, and install fast feedback before semantic refactoring.

**Primary requirements:** R0, R1, R7.2–R7.3, R9.5–R9.6.

**Deliverables:**

- Baseline evidence report for formatting, clippy, tests, docs, package contents, dependency tree, unsafe inventory, public API inventory, and current backend build behavior.
- Repository cleanup: placeholder Python scaffold, tracked logs, obsolete configuration, dead documentation link, generated/machine-local artifacts.
- Dual-license files and manifest metadata recommendation implemented after owner confirmation.
- Workspace-level package metadata, dependencies, lints, and release profiles.
- Initial solver-free CI on Linux/macOS/Windows.
- `cargo deny`, `cargo machete` or equivalent unused-dependency check, `cargo audit`, rustdoc warnings, and package-list checks.
- Explicit crate publication map: `roml` and `roml-highs` candidate; commercial adapters gated.

**Gate P0:** core builds/tests/docs/packages on all three operating systems without any solver installation; package lists are reviewed; no generated log or placeholder scaffold remains.

**Detailed plan:** `docs/superpowers/plans/2026-07-13-phase-00-release-baseline.md`

## Phase 1 — Canonical model semantics and invariant closure

**Goal:** eliminate model-level correctness ambiguity before changing synchronization or adapters.

**Primary requirements:** R2, R8.3.

**Critical work:**

- Introduce validated numeric/domain types or centralized validation for bounds, coefficients, parameters, tolerances, and expressions.
- Replace silent invalid-ID behavior with typed failures.
- Define a unique canonical coefficient cell for each `(CoefficientTarget, VarId)` and algebraically combine all terms into one `ValueExpr`.
- Define duplicate-term, zero-term, parameter dependency, deletion cascade, objective constant, and activity semantics.
- Remove recursive/default and API inconsistencies.
- Add an internal `Model::validate()`/invariant checker used by tests and debug paths.
- Add property tests for random legal model mutations and invalid-input rejection.

**Gate P1:** generated model sequences preserve all invariants; duplicate parametric terms produce the mathematically correct coefficient; no public mutation silently fails.

**Detailed plan:** `docs/superpowers/plans/2026-07-13-phase-01-core-correctness.md`

## Phase 2 — Revisioned snapshots, journals, and recoverable synchronization

**Goal:** replace the single-consumer destructive changelog with a synchronization protocol that supports failure recovery and multiple adapters.

**Primary requirements:** R3, R4.1, R8.1.

**Target protocol:**

```text
ModelRevision r
CanonicalSnapshot(r)
DeltaBatch { from: r0, to: r1, operations: [...] }
AdapterCursor { applied_revision, health }
apply(batch) -> Acknowledgement | RecoverableFailure | DirtyFailure
rebuild(snapshot) -> Acknowledgement
```

**Critical work:**

- Define revisions and typed delta batches with explicit ordering guarantees.
- Keep journal entries replayable until retention policy permits compaction.
- Give each attached adapter an independent cursor.
- On apply failure, preserve the model delta and mark adapter state; rebuild from snapshot when needed.
- Make transactions atomic at the model revision boundary.
- Build a reference in-memory backend to test synchronization independent of native solvers.
- Establish the core theorem by executable testing: applying all deltas to revision `r` is observationally equivalent to projecting snapshot `r`.

**Gate P2:** two adapters can independently lag/catch up; injected failures lose no model changes; randomized incremental projection equals snapshot rebuild.

**Detailed plan:** `docs/superpowers/plans/2026-07-13-phase-02-revisioned-sync.md`

## Phase 3 — Binding topology, native discovery, and unsafe boundary hardening

**Goal:** isolate all ABI risk, replace handwritten declarations where authoritative bindings exist, and redesign callbacks/lifecycles around official contracts.

**Primary requirements:** R4, R5, R6.

**Binding decisions:**

- **HiGHS:** adopt `rust-or/highs-sys` if its generated official-header surface covers required APIs. Upstream missing callback symbols or pin a narrow fork before creating a new ROML sys crate.
- **MOSEK:** use the official `mosek` crate/API. Remove handwritten enum/function declarations. The current callback implementation is invalid because it mutates the task from inside a callback; redesign as collect/terminate/apply-outside/re-optimize or expose only supported callback capabilities.
- **Xpress:** create a dedicated binding boundary only after verifying header redistribution and package licensing. Prefer generated bindings plus runtime loading for commercial-library availability, or a link-time sys crate with strict target discovery if runtime loading is unsuitable.
- Add a small internal adapter-support crate/module only for genuinely solver-neutral mechanics such as dense index bookkeeping and revision application scaffolding; do not force solver semantics into false uniformity.

**Unsafe rules:**

- no panic crossing C;
- no unchecked null/length assumptions;
- no ignored return codes;
- no undocumented `Send`/`Sync`;
- callback cleanup is RAII and unwind-safe;
- backend versions and capabilities are queryable;
- constructors return errors, not asserts/panics.

**Gate P3:** core contains no raw FFI; HiGHS and MOSEK contain no handwritten ABI declarations; callback and lifecycle invariants have dedicated tests and safety comments; Xpress has an approved binding/licensing decision.

**Detailed plan:** `docs/superpowers/plans/2026-07-13-phase-03-solver-boundaries.md`

## Phase 4 — Cross-platform CI and backend qualification

**Goal:** turn portability from an assumption into an executable support matrix.

**Primary requirements:** R6, R7, R8.

**Matrix:**

| Layer | Linux | macOS | Windows | License |
|---|---:|---:|---:|---|
| Core | required | required | required | none |
| HiGHS build/load/solve | required | required | required | MIT |
| MOSEK compile/load | required where supported | required where supported | required where supported | install + protected license for solve |
| Xpress compile/load | required where supported | required where supported | required where supported | install + protected license for solve |
| MSRV | Linux required | optional smoke | optional smoke | none |
| Miri/fuzz/sanitizer | scheduled Linux | sanitizer where useful | optional | none |

**Critical work:**

- Build reusable CI workflows with a fast core lane and backend lanes.
- Validate both clean-host failure diagnostics and successful native discovery.
- Test HiGHS bundled/static and optional system discovery modes.
- Test runtime library resolution without embedding developer-machine rpaths.
- Separate compile, native load, license acquisition, and solve failures.
- Add randomized differential tests and benchmark smoke thresholds.
- Add protected self-hosted runner design for commercial solvers without leaking binaries/licenses.

**Gate P4:** all required matrix cells are green; unsupported cells are explicitly documented; a clean user environment receives actionable diagnostics rather than linker/runtime mysteries.

**Detailed plan:** `docs/superpowers/plans/2026-07-13-phase-04-cross-platform-ci.md`

## Phase 5 — Public API curation, documentation, and package engineering

**Goal:** make ROML understandable, semver-manageable, and installable by users without repository knowledge.

**Primary requirements:** R0, R1, R9.

**Critical work:**

- Audit every public module/type/field and narrow visibility.
- Define prelude intentionally; remove implementation stores from stable surface where possible.
- Add `#![deny(missing_docs)]` when documentation debt is closed.
- Write architecture, incremental protocol, native backend, migration, troubleshooting, and performance guides.
- Provide solver-free, HiGHS, incremental-parameter, transactions, and failure-recovery examples.
- Establish semver policy and run `cargo-semver-checks` against the release baseline.
- Add changelog, contributing, security, support, and release documents.
- Verify docs.rs behavior and package contents for each crate.

**Gate P5:** a new Rust user can discover, install, model, synchronize, solve with HiGHS, update parameters, and diagnose failures from public documentation alone; public API review has no unintentional exposures.

**Detailed plan:** `docs/superpowers/plans/2026-07-13-phase-05-public-api-packaging.md`

## Phase 6 — Release qualification and staged publication

**Goal:** produce a defensible release candidate and publish in dependency order only after explicit authorization.

**Primary requirements:** all R0–R9 requirements applicable to the selected release crates.

**Critical work:**

- Freeze versions and generate a release-candidate evidence bundle.
- Test crates from packed `.crate` archives in fresh projects/containers, not from workspace paths.
- Verify dependency publication order and exact versions.
- Run semver, license, provenance, checksum, docs, examples, benchmark, and compatibility checks.
- Conduct independent principal-engineer review and unsafe/FFI review.
- Publish canary/pre-release where appropriate; validate crates.io/docs.rs; then publish stable crates under owner authorization.
- Tag only the exact verified commit and archive evidence.

**Gate P6:** signed checklist, zero unresolved P0/P1 issues, all mandatory CI green, package-consumer smoke tests green, owner authorization recorded.

**Detailed plan:** `docs/superpowers/plans/2026-07-13-phase-06-release-qualification.md`

## Phase 7 — Foreign-language ABI foundation (post-v0.1)

**Goal:** establish a stable C-facing contract suitable for Python, Java, and .NET wrappers without exporting Rust ABI or arena internals.

**Primary requirements:** R10.

**Proposed form:**

- `roml-c-api` crate producing `cdylib`/`staticlib`.
- opaque handles with generation-safe registries;
- explicit ownership and destroy functions;
- versioned function table or ABI version negotiation;
- error objects/status codes with thread-local or caller-owned messages;
- no unwinding across ABI;
- bulk array APIs for model construction and updates;
- stable external IDs independent of Rust slot indices;
- generated C header and ABI compatibility tests;
- wrappers layered over the C API rather than directly over internal Rust types.

**Start condition:** P6 complete and at least one released Rust/HiGHS version has real-world usage feedback.

## Parallel execution map

After P0:

- **Track A:** P1 core semantics.
- **Track B:** CI scaffolding portions of P4 that do not encode old APIs.
- **Track C:** external binding/legal research and upstream probes for P3.
- **Track D:** documentation inventory and examples research for P5, without freezing unstable APIs.

After P1:

- P2 is the critical path.
- HiGHS binding integration spikes may run in parallel, but adapter implementation waits for the P2 delta contract.

After P2:

- HiGHS, MOSEK, and Xpress adapter work can run as independent worktrees/agents with a frozen backend contract.
- A separate verifier owns cross-backend equivalence and does not author adapter code.

## Program-level stop conditions

Stop and escalate when:

- an official solver contract contradicts the intended generic abstraction;
- a proprietary license forbids distributing generated bindings or package metadata;
- an unsafe invariant cannot be expressed and tested;
- a semantic change would silently alter existing model results;
- a performance optimization requires weakening recoverability or correctness;
- crates.io name ownership or dependency publication order is unresolved;
- the implementation agent proposes publishing before P6.

## Definition of done

The roadmap is complete when the selected release crates satisfy all mapped requirements, all phase gates have evidence, public claims match tested support, and a fresh consumer can use ROML without relying on the maintainer's macOS paths, solver installations, or implicit knowledge.