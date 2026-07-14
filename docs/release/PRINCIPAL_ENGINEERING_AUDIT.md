# Principal Engineering Audit — ROML Public-Release Baseline

**Audited ref:** `main@f9ba1921e650b5057bbc4de090a78391f7932a53`  
**Audit date:** 2026-07-13  
**Scope:** architecture, semantic correctness, incremental protocol, unsafe/FFI boundaries, native linking, portability, package engineering, CI, documentation, and future language-binding readiness.

## Executive verdict

ROML contains a differentiated and useful core idea: parameter-dependent coefficient expressions with explicit reverse dependency tracking and persistent solver adapters. The repository is functional enough to justify hardening, but it is **not ready for crates.io publication or a production-grade claim**.

The most important risks are not cosmetic file organization. They are semantic and architectural:

1. Multiple symbolic terms can target one solver matrix cell, while adapters apply last-write-wins updates.
2. Model deltas are destructively drained before backend acknowledgement, so failures can lose synchronization information.
3. One changelog cannot support multiple independently synchronized adapters.
4. Handwritten FFI declarations and constants couple ROML to unverified solver versions and ABIs.
5. The MOSEK callback mutates the task inside the callback, contrary to MOSEK's documented callback contract.
6. Native discovery/linking is macOS-centric and incomplete on Linux/Windows.
7. Core package responsibilities are polluted by global logging/configuration policy.
8. Package metadata, licensing, CI, docs, and release evidence are incomplete.

The correct program is therefore not “clean up crates and publish.” It is “close core semantics and synchronization first, then replace solver boundaries, then qualify packages.”

## Severity model

- **P0:** can produce incorrect results, undefined behavior, data loss, or an invalid public release.
- **P1:** material portability, reliability, API, or supportability defect.
- **P2:** maintainability/performance/documentation debt that should close before stable 1.0, but may not block a narrowly labeled 0.1 after review.

## Findings

### A. Canonical model semantics

#### A1 — Duplicate symbolic terms can corrupt matrix semantics — P0

`LinExpr::simplify` combines constant terms by variable but preserves parameter-based terms separately. `CoefficientIndex` permits multiple coefficient objects for the same `(constraint/objective, variable)` pair. Solver matrices, however, have one scalar cell per row/column or objective/column. Adapters call replacement-style APIs such as `Highs_changeCoeff`, `MSK_putaij`, and `XPRSchgcoef`, so later terms overwrite earlier terms. Removal can zero a cell still represented by another coefficient object; parameter updates can overwrite rather than sum.

**Required correction:** establish one canonical coefficient cell per `(target, variable)` whose value expression is the algebraic sum of all contributing terms, or compile expressions into a normalized aggregate before storage. Maintain the reverse parameter dependency graph over that aggregate.

**Required tests:** duplicate constants, duplicate parameter terms, mixed constant/parameter terms, cancellation to zero, removal of one contributor, and multi-parameter updates across all backends plus the reference in-memory projection.

#### A2 — Numeric/domain validation is incomplete — P0

Bounds expose `is_valid`, but public model mutators do not consistently enforce it. NaN values can bypass ordinary ordering semantics. Coefficients/parameters can become non-finite. `ValueExpr::Div` has no zero-denominator policy. Tolerances and fixed-bound construction can be invalid.

**Required correction:** centralized validation with typed errors and explicit finite/infinite policy. Permit infinities only where mathematically meaningful (bounds), reject NaN everywhere, and define expression evaluation failures.

#### A3 — Invalid identities may be silently ignored — P1

Some mutations return errors while others, including parameter changes, can queue or ignore unknown IDs without a typed failure. This undermines agentic use and future language bindings because invalid handles must never appear successful.

#### A4 — `ModelConstants::default` recursion defect — P1

The inherent `default` implementation calls `Self::default`, which resolves recursively rather than intentionally invoking the `Default` trait. Remove the inherent method or call `<Self as Default>::default()` and add a regression test.

#### A5 — Objective constants and model constants need backend-neutral contracts — P1

The model stores objective constants, but adapter extraction and objective reporting must define whether raw solver values include offsets and how multiple objective switches preserve them. “Infinity” constants must not become solver-specific global assumptions.

#### A6 — Wide mutable implementation surface — P1

Public stores, data structs, fields, IDs, and modules expose implementation details that will become semver liabilities. Slot index/generation access is especially unsuitable as a future serialized or foreign identity contract.

### B. Incremental synchronization

#### B1 — Destructive drain before acknowledgement loses changes on failure — P0

The current extension path drains the `ChangeLog` and then calls the adapter. If native application fails after partial mutation, the model no longer retains the operations needed to recover, and the adapter may be inconsistent.

**Required correction:** revisioned immutable delta batches, adapter acknowledgements, health state, and snapshot rebuild. Journal compaction may occur only after all required cursors or an explicit retention policy permit it.

#### B2 — Single-consumer changelog prevents independent adapters — P0

A single `Vec<Change>` that is drained cannot feed HiGHS and MOSEK independently, cannot support shadow verification, and cannot allow one adapter to lag while another advances.

**Required correction:** per-adapter cursors over a revisioned journal or a publish/subscribe-equivalent protocol anchored in canonical model revisions.

#### B3 — Adjacency-dependent batching is brittle — P1

HiGHS/Xpress batching assumes `ConstraintAdded` is immediately followed by all coefficients for that constraint. This is an implicit compiler convention, not a typed contract. Refactors or transactions can break it silently.

**Required correction:** emit typed aggregate operations (`AddRow { coefficients }`) or compile a `DeltaBatch` from canonical changes before adapter application.

#### B4 — `supports_incremental` is not a capability model — P1

All adapters optimistically return true. Real support varies by operation, model class, callback mode, basis state, solver version, and post-solve lifecycle.

**Required correction:** explicit backend capability descriptors and per-operation apply outcomes (`Applied`, `RequiresRebuild`, `Unsupported`, `FailedDirty`).

### C. Solver-neutral contract

#### C1 — Status model loses information — P1

The current status enum cannot faithfully represent feasible-but-not-proven-optimal, objective/iteration/node/time limits with an incumbent, user interruption, license errors, numerical failures, and ambiguous unbounded-or-infeasible states.

#### C2 — Error model lacks backend context and recovery policy — P1

Errors need backend/version, operation, native code/category, message, and whether the adapter remains usable, requires rebuild, or is terminal.

#### C3 — Solution access clones full maps — P2

`solution_values`, duals, and reduced costs clone `HashMap`s. This is expensive for large repeated optimization and awkward for wrappers. Prefer indexed/borrowed views, iterators, caller-provided buffers, and optional bulk extraction.

#### C4 — Callback abstraction promises false uniformity — P0/P1

A generic “add cuts from candidate callback” contract is not valid across all solvers. Capabilities must distinguish observation, interruption, lazy constraints, user cuts, incumbent injection, and whether model mutation is legal during callback.

### D. HiGHS adapter and bindings

#### D1 — Handwritten ABI tied to HiGHS 1.14 — P0

The repository copies function declarations, callback structs, field offsets, and numeric constants. Runtime `HighsInt` width checking does not validate every struct/enum/function signature. HiGHS has continued evolving; current official releases and generated bindings should be used.

**Recommendation:** adopt `rust-or/highs-sys`, which vendors/discovers HiGHS and generates bindings from `highs_c_api.h`. Confirm required callback APIs under the selected version; upstream gaps or pin a narrow fork rather than maintaining a parallel handwritten ABI.

#### D2 — Callback trampoline is not unwind/null/error safe — P0

The callback dereferences user/data pointers and creates slices without null/size validation, invokes arbitrary Rust code without `catch_unwind`, and ignores `Highs_addRow` results. State cleanup is manually managed through raw `Box` conversion.

#### D3 — Status mapping is incorrect — P0

`UNBOUNDED_OR_INFEASIBLE` maps to `Infeasible`, which asserts knowledge the solver did not establish.

#### D4 — Objective switching has edge/association risks — P0/P1

Zeroing range `0..num_cols-1` is invalid for zero columns. Building column and cost vectors from separate hash-map iterators is unnecessarily fragile; collect pairs in one pass and define deterministic ordering.

#### D5 — Native build script is not a complete distribution strategy — P1

The script supports several environment variables and source build, but lacks a declared `links` owner, pinned source/version policy, complete Windows runtime DLL handling, robust static transitive linking, docs.rs behavior, and full architecture matrix. Rpath policy should not be embedded casually into a library crate.

### E. MOSEK adapter and bindings

#### E1 — Official Rust API is bypassed — P0/P1

ROML copies C declarations, enum values, parameter IDs, and callback info indices even though MOSEK publishes and documents an official Rust API crate. This creates unnecessary ABI/version risk.

**Recommendation:** rewrite the adapter over the official `mosek` crate, pin the supported major/minor range, and expose backend version/capabilities.

#### E2 — Callback mutates task in an undefined context — P0

The callback appends constraints and writes matrix entries from inside the MOSEK callback. MOSEK's official callback documentation states callbacks must not invoke solver/environment/task functions except permitted integer-solution retrieval; otherwise state and outcome are undefined.

**Required correction:** remove this behavior immediately. A safe emulation can collect violated cuts in callback-owned memory, request termination, unregister/exit, apply cuts outside the callback, and re-optimize—only if official return-code and lifecycle semantics support that protocol. Otherwise mark lazy-cut mutation unsupported.

#### E3 — Build/discovery script is incomplete — P1

Hardcoded candidates cover limited platforms and filenames; Windows naming/import libraries and Linux arm64 are incomplete. Runtime shared dependencies such as TBB and license setup are not represented. Official MOSEK guidance identifies platform directories and runtime library path requirements.

#### E4 — Environment/task strategy is suboptimal — P2

The adapter owns one environment per instance, while official guidance recommends one or no explicit environment and sharing it among tasks where appropriate. License-token lifecycle and concurrency semantics require deliberate design.

### F. Xpress adapter and bindings

#### F1 — Handwritten constants/signatures without pinned header verification — P0

Attribute/control IDs and function declarations are copied manually. A solver upgrade can compile while invoking wrong semantics.

#### F2 — Build script defaults to a macOS application path — P1

The default `/Applications/FICO Xpress/...` path is not suitable public behavior. Linux/Windows/architecture/version discovery, import libraries, runtime dependencies, and actionable diagnostics are missing.

#### F3 — Initialization/lifecycle and return codes are incomplete — P0/P1

Process-global `XPRSinit` panics on error, several API results are ignored, `XPRSfree` lifecycle is not clearly owned, and the message callback writes directly to stdout.

#### F4 — Capability/implementation mismatch — P1

The repository's latest commit claims callback support across HiGHS, MOSEK, and Xpress, but the Xpress adapter does not implement the generic callback handler found in the other two adapters. Claims, tests, and capability declarations must agree.

#### F5 — Binding/legal decision required — P1

Before publishing an Xpress sys crate, verify whether generated bindings/header-derived constants may be redistributed and under what license. Runtime loading may improve compile/docs portability but does not remove licensing obligations.

### G. Logging and configuration

#### G1 — Library owns global logging policy — P1

The core depends on `log4rs` and `serde_yaml`, searches parent directories for configuration, mutates environment variables, prints from initialization, and assumes workspace layout. A library should emit through a facade and let the application select/configure the subscriber/logger.

#### G2 — Logging implementation contains correctness/test-isolation defects — P1

The parent `config.yaml` path is shadowed rather than retained; a computed resolved path is discarded; `unwrap` can panic; tests mutate process-wide current directory/environment and can race.

**Recommendation:** retain only `log` or `tracing` events in core. Move optional convenience initialization to examples, a feature-gated helper crate, or remove it.

### H. Package/repository engineering

#### H1 — No license files — P0

A public source repository without an explicit license does not grant normal reuse rights, and crates.io requires license metadata or a license file. Recommended Rust ecosystem default: `MIT OR Apache-2.0`, subject to owner approval.

#### H2 — Incomplete package metadata — P1

Root and adapter manifests lack repository/readme/license/rust-version/keywords/categories and publication policy. Path dependencies need versions for registry packaging.

#### H3 — Release contamination — P1

Tracked placeholder `main.py`, placeholder `pyproject.toml`, `uv.lock`, generic config, and `roml-mosek/roml.log` should not be in the Rust release workspace unless they serve a documented tool.

#### H4 — README defects/overclaim — P1

The README links to missing `MODELING_API.md` and calls the library production-grade before cross-platform/FFI/recovery qualification.

#### H5 — No CI or release controls — P0/P1

No GitHub Actions workflows were found. There is no MSRV, package verification, dependency policy, semver check, docs.rs strategy, release checklist, changelog, security policy, or provenance flow.

### I. Testing and performance

#### I1 — Test tiers are not separated — P1

Core, open-source backend, commercial backend, licensed solve, and platform tests need independent lanes. A user must be able to test core without all native solvers installed.

#### I2 — Missing incremental-vs-rebuild oracle — P0

The defining claim must be tested by applying random legal mutation sequences both incrementally and from a fresh canonical snapshot, comparing model/solution observations.

#### I3 — Unsafe/ABI tests are insufficient — P0/P1

Need generated bindings, version probes, null/panic callback tests, lifecycle tests, load diagnostics, sanitizers/Miri where applicable, and platform runtime resolution.

#### I4 — Performance claims lack reproducible decomposition — P2

Benchmark model construction, propagation, journal compilation, native delta application, rebuild, solve, and extraction separately. Add batch APIs based on profiles rather than intuition.

### J. Future language wrappers

#### J1 — Rust public API is not a stable foreign ABI — P1

Do not bind Python/Java/.NET directly to current Rust layouts or slot indices. After Rust release qualification, introduce `roml-c-api` with opaque handles, explicit ownership, version negotiation, panic containment, bulk operations, and stable external IDs.

## Recommended crate topology

Near-term:

```text
roml                 # solver-free canonical model, revisions, deltas, capabilities
roml-highs           # safe HiGHS adapter; depends on maintained highs-sys
roml-mosek           # safe adapter over official mosek crate; independently gated
roml-xpress          # safe adapter; binding strategy pending legal/technical review
```

Optional only if duplication remains after adapter rewrites:

```text
roml-adapter-support # private/non-user-facing index/revision projection mechanics
```

Post-v0.1:

```text
roml-c-api           # stable C ABI for foreign wrappers
roml-python          # PyO3 or C-ABI wrapper based on deployment goals
roml-java / roml-dotnet
```

Do **not** create `roml-highs-sys` or `roml-mosek-sys` merely for naming symmetry. A sys crate is an ownership boundary, not a required layer in every backend.

## External authoritative references used

- Cargo manifest/package/link metadata: https://doc.rust-lang.org/cargo/reference/manifest.html
- Cargo build scripts and target variables: https://doc.rust-lang.org/cargo/reference/build-scripts.html
- HiGHS official repository/C API: https://github.com/ERGO-Code/HiGHS
- Maintained generated HiGHS bindings: https://github.com/rust-or/highs-sys
- MOSEK official Rust API: https://docs.mosek.com/latest/rustapi/index.html
- MOSEK callback restrictions: https://docs.mosek.com/latest/rustapi/callback.html
- MOSEK deployment/runtime guidance: https://docs.mosek.com/latest/rustapi/guidelines-optimizer.html

## Final recommendation

Proceed with the hardening program. Do not publish the current `0.1.0`. The highest-value next implementation is Phase 0 baseline/hygiene followed immediately by Phase 1 canonical coefficient correctness and Phase 2 revisioned synchronization. Solver crate reorganization before those phases would optimize the wrong boundary and increase rework.