# ROML Public-Release Architecture Decisions

These decisions govern implementation until amended by a reviewed ADR.

## D-001 — Release topology

**Decision:** publish the solver-independent core and reference HiGHS adapter first. Treat MOSEK and Xpress as independently gated adapters.

**Rationale:** commercial native installation/licensing must not prevent core builds, docs, tests, or a useful open-source release.

**Consequence:** `roml-mosek` and `roml-xpress` may remain workspace members with `publish = false` or experimental status until qualified.

## D-002 — Core is solver-free

**Decision:** `roml` has no native solver, raw FFI, library discovery, global logger configuration, or commercial license dependency.

**Consequence:** users can compile, test, document, serialize/inspect models, and use an in-memory test projection without native software.

## D-003 — Canonical coefficient cell

**Decision:** the canonical model has at most one coefficient cell for each `(CoefficientTarget, VarId)`. All source terms are algebraically composed into that cell's expression.

**Rejected:** storing multiple coefficient objects and relying on backend accumulation. Common solver mutation APIs replace cells; removal/update semantics become incorrect.

## D-004 — Revisioned model protocol

**Decision:** replace destructive changelog draining with immutable revisioned delta batches and per-adapter acknowledgement cursors.

**Properties:**

- model mutations commit at a revision boundary;
- snapshots reconstruct any retained revision or at least current canonical state;
- adapters acknowledge only fully applied revisions;
- failure classifies adapter health;
- dirty adapters rebuild from snapshot;
- multiple adapters synchronize independently;
- journal compaction is explicit.

## D-005 — Capability algebra, not boolean support

**Decision:** backend support is represented by explicit capabilities and per-operation outcomes.

A capability set should distinguish:

- model classes: LP/MIP/QP/conic as applicable;
- incremental operation classes;
- deletion vs deactivate/fix strategies;
- objective switching;
- basis/warm starts;
- dual/reduced-cost availability;
- progress observation;
- interruption;
- lazy constraints/user cuts/incumbent injection;
- thread/reentrancy restrictions;
- backend/version-specific constraints.

## D-006 — HiGHS binding ownership

**Decision:** use `rust-or/highs-sys` as the default raw binding/build boundary if it exposes the selected official HiGHS C API.

**Procedure for gaps:**

1. verify the function exists in the pinned official header;
2. confirm bindgen exposure and feature/version behavior;
3. upstream a fix;
4. if release timing requires, pin a minimal reviewed fork;
5. create a ROML-specific sys crate only if the maintained crate cannot satisfy the product contract.

**Reason:** generated bindings from the official header dominate copied layouts/constants.

## D-007 — MOSEK binding ownership

**Decision:** implement `roml-mosek` over the official `mosek` Rust crate/API.

**Immediate safety decision:** remove/disable task mutation inside callbacks. MOSEK documentation says callbacks must not invoke task/environment/solver functions except the permitted integer-solution retrieval path.

**Allowed redesign:** callback records data/cuts in Rust-owned state and requests termination; adapter applies changes after optimize returns and optionally re-optimizes, subject to official semantics and tests. Otherwise report unsupported capability.

## D-008 — Xpress binding investigation

**Decision:** do not publish handwritten Xpress ABI declarations.

Before implementation, produce a binding decision memo covering:

- official header version and supported solver versions;
- permission to redistribute generated declarations/constants;
- target/architecture library names and dependencies;
- initialization/free and license lifecycle;
- official callback mutation rules;
- link-time sys crate vs runtime dynamic loading;
- docs.rs/clean-host compilation behavior;
- CI availability.

**Preferred direction:** generated bindings isolated in a dedicated boundary. Runtime loading is favored if it materially improves commercial-solver optionality and diagnostics without violating licensing or safety constraints.

## D-009 — Native build/link policy

**Decision:** exactly one crate owns each native `links` value and emits native link metadata.

Rules:

- decisions use `TARGET`/`CARGO_CFG_TARGET_*`, not host `cfg!`;
- no developer-machine default paths;
- environment overrides are documented and validated;
- discovery reports searched locations and expected filenames;
- link-time and runtime search are treated separately;
- library crates do not indiscriminately inject executable rpaths;
- commercial binaries are never vendored;
- HiGHS may be reproducibly bundled/static under its license;
- docs.rs has an explicit no-native strategy where required.

## D-010 — Errors, panics, and unsafe code

**Decision:** missing libraries, incompatible versions, initialization failures, license failures, and solver errors return typed errors.

Rules:

- no `assert!`, `expect`, or `panic!` on user/environment/native failure paths;
- no unwind through C; trampolines use `catch_unwind` and convert panic to backend interruption/error;
- every pointer and length from C is validated before dereference/slice creation;
- every native return code is checked or explicitly documented as infallible;
- `unsafe impl Send/Sync` requires official thread-safety evidence plus an invariant comment and tests;
- resource cleanup is RAII and valid on every early-return path.

## D-011 — Logging

**Decision:** core emits events through a logging facade but does not configure a global logger, scan for YAML files, mutate logging environment variables, or print.

**Default:** keep `log` initially to minimize churn; evaluate `tracing` as a separate API decision. Remove `log4rs` and `serde_yaml` from core unless an optional integration crate proves necessary.

## D-012 — Public API and pre-1.0 semver

**Decision:** intentionally curate the API before first publication. Existing public visibility on the unpublished repository does not create a compatibility obligation.

Rules:

- stores/data fields are private unless users need them as stable concepts;
- use constructors/accessors and typed views;
- implementation modules may be private with selected re-exports;
- rustdoc defines invariants and failure semantics;
- semver checks begin from the first release tag;
- experimental items are feature-gated and clearly labeled.

## D-013 — Licensing

**Recommendation:** `MIT OR Apache-2.0`, with both license texts, subject to owner confirmation before implementation merge.

**Reason:** conventional Rust ecosystem compatibility and explicit reuse rights. Commercial solver adapters remain subject to vendor licenses; ROML's license does not redistribute solver binaries or licenses.

## D-014 — CI support labels

**Decision:** distinguish:

- **supported:** mandatory platform/backend jobs pass continuously;
- **tested:** periodic/protected job passes but is not guaranteed for every change;
- **compile-only:** type/build surface checked without native load/solve;
- **experimental:** API may change and support matrix is incomplete;
- **unsupported:** no claim.

No backend inherits “supported” from a single local test.

## D-015 — Reference correctness oracle

**Decision:** build a solver-neutral in-memory projection of canonical variables, rows, objectives, and values. It need not optimize; it exists to validate delta application, indices, revision recovery, and snapshot equivalence.

Native backends are additionally checked by rebuild-vs-incremental solve equivalence on deterministic fixtures and generated bounded instances.

## D-016 — Foreign language boundary

**Decision:** future wrappers target a versioned C ABI or equivalent stable boundary, never Rust ABI.

The C ABI uses opaque handles, explicit ownership, bulk operations, version negotiation, panic containment, and stable external IDs. Work begins after v0.1 qualification, not during core hardening.

## D-017 — Performance method

**Decision:** optimize measured stages, preserving correctness/recoverability. Benchmarks separate symbolic construction, dependency propagation, delta compilation, native application, rebuild, solve, and extraction. Bulk operations are introduced where profiles show FFI/setup overhead.

## D-018 — Implementation sequencing

**Decision:** do not reorganize adapters deeply before the core canonical and revision contracts are frozen.

Sequence:

1. baseline/hygiene/CI;
2. canonical model correctness;
3. revision/snapshot/journal protocol;
4. binding and adapter rewrites;
5. platform/backend qualification;
6. API/docs/package/release.

This minimizes rework and forces adapter design to implement the intended production contract rather than preserve prototype accidents.