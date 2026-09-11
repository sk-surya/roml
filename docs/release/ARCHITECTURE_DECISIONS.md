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

**Decision (amended 2026-09-07):** Python uses a direct PyO3 extension packaged
with maturin, as specified in
[MPY](../../.planning/milestones/MPY-python-interface/README.md). The extension
and its Rust dependencies are compiled together; it does not dynamically link
against an independently versioned Rust ABI. An exported ROML C ABI is not a
prerequisite for Python. Other-language bindings remain deferred and require
their own concrete consumer and boundary design.

Ownership, bulk operations, panic/error containment and external identity
validation remain required. A future public C ABI would additionally require
opaque handle and version-negotiation contracts. MPY follows verified M3/P34
qualification and precedes M4; registry publication is not a prerequisite or
authorized side effect. This explicitly supersedes the earlier C-ABI-first
wording for Python under the owner's 2026-09-07 instruction.

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

## D-019 — Shared ordinal modeling IR and block-native core

**Decision:** ROML has one modeling system with two ergonomic frontends (native Rust and Python). Both lower to a shared model-owned **ordinal** array IR, then to L2 CSR/block descriptors and canonical block-native core operations. The core gains trusted variable/parameter spans, packed parametric rows/objectives, and block-native parameter propagation. There is no Pyomo `AbstractModel`; changing data is parameter binding on persistent structure, while changing structure rebuilds.

**Levels:** `roml::modeling` is Level 1; L2 owns language-independent CSR/block/strided dependency descriptors; `roml::advanced` remains the raw/advanced escape hatch. Python wraps L1/L2 rather than maintaining a second modeling IR.

**Initial IR:** symbolic views retain `ModelInstanceId` ownership and wrap `View<S> = span + shape + signed strides + offset`. `LinArray = Σ Term{VarView, CoeffView} + ConstantView`. Initial `CoeffView` kinds are `One`, `Scalar(α)`, `Dense{scale, values}`, and `ScaledParam{scale, ParamView}`.

**Invariants:**

1. `VarSpan`/`ParamSpan` originate only from trusted block allocation; arbitrary `(start,len)` cannot construct them.
2. Block members use the arena's fresh generation; IDs are never reused; per-entity staleness is preserved and there is no span-wide epoch.
3. Symbolic array handles retain model ownership; cross-model composition is rejected before ID reconstruction.
4. Block views are span + shape + signed strides + offset; slicing/transpose edit metadata rather than gathering IDs.
5. Labels and component names are frontend/boundary metadata and do not enter expression nodes.
6. Block allocation never materializes per-element component names; existing scalar borrowed name APIs are not broken merely to synthesize array names.
7. Variable block creation uses one packed journal/delta operation. Parameter block creation preserves existing parameter-creation revision semantics and does not invent a solver-facing add operation solely for parameter existence.
8. Covered coefficient families never materialize `Vec<Affine>` or per-cell `ValueExpr`; scalar scaling of a dense numeric view changes a scalar factor, not the buffer.
9. Parameterized rows/objectives have packed bulk construction paths. A packed p-cell represents one `scale × ParamId` at one canonical `(target,var)`.
10. Distinct parameters contributing to the same canonical cell are not silently collapsed into the p-base representation; they are typed not-packable and use the correct general symbolic path.
11. Persisted dependency blocks use L2/core strided ordinal descriptors, not L1/Python view types.
12. `ParamDepBlock` eligibility requires a metadata proof that `r -> (target(r),var(r))` is collision-free across all terms and that post-canonical coefficient positions admit a strided witness. It is sink-aware and conservative; stride sign/range-disjointness alone are invalid predicates.
13. Core revalidates any supplied `ParamDepLayout` against the post-canonical block before storing it. Uncertain/invalid layouts never become fast-path state.
14. Eligible block dependencies do not populate per-cell `param_positions`; semantic dependency queries and scalar updates remain complete by consulting block and sparse dependency representations.
15. Bulk parameter updates preserve transaction/commit semantics. One committed parameter block yields one packed parameter-value change and one packed coefficient-patch batch containing all affected eligible dependency blocks.
16. Retained delta operations are self-contained. They may share immutable construction topology but never require access to mutable live-model packed storage.
17. Packed base/p-base are append-only for fresh canonical cells; mutation of an existing logical cell uses/shadows into the overlay; block propagation respects dead/shadowed cells.
18. Rule/callback APIs may construct expressions per index but accumulate CSR and mutate the model in bulk.
19. Rust and Python vectorized frontends are compared by normalized ordinal-IR and semantic-journal fingerprints, not raw bytes containing owner IDs or absolute entity IDs.
20. The general symbolic path remains a correct fallback and is not distorted to absorb exotic cases.
21. Diagnostics guard fast paths, but are qualification/debug surfaces rather than mathematical model semantics.

**Initial conservative optimization boundary:** `ParamView * LinArray` stays in L1 fast IR only when variable-term coefficients are `One`/`Scalar` and the constant is `Zero`/`Scalar`. Dense×parameter and parameter×parameter coefficient forms fall back initially and are promoted only if measurements justify another coefficient kind.

**Rejected:** span-wide epoch invalidation; core dependence on Python/L1 view types; Pyomo source/API compatibility; macro DSL as foundation; `IndexSet<T>` inside expression IR; eligibility based only on stride sign/range overlap; solver-facing parameter-add operations that change current creation semantics without evidence.

**Qualification:** flagship BESS uses 28,800 mutable price parameters and 57,600 affected objective cells, 100 bulk reprices on one persistent session, no general-affine lowering for the covered formulation, no per-cell reverse-index/overlay/`ValueExpr` work on the eligible update path, self-contained packed delta replay, snapshot equivalence and independent review.

**Sequencing:** MIR-00 baseline → MIR-01 trusted blocks → MIR-02 parametric packed construction/propagation → MIR-03 shared IR/proof/CSR → MIR-04 Rust L1 → MIR-05 rule builders → MIR-06 Python migration → MIR-07 ergonomics/template binding → MIR-08 qualification.
