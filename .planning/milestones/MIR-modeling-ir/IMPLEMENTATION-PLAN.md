# MIR Tranche 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use Superpowers TDD/systematic-debugging during implementation and verification-before-completion before every phase claim. Execute one MIR phase at a time; do not start MIR-01 until MIR-00 evidence is committed, or MIR-02 until MIR-01 is reviewed and green.

**Goal:** establish the measured baseline, trusted block allocation, packed parametric row/objective storage, and transaction-preserving block-native repricing needed by the later shared modeling IR.

**Architecture:** MIR-00 characterizes the current path first. MIR-01 adds opaque block identities without changing scalar semantics. MIR-02 extends the packed parametric coefficient path with validated L2 dependency layouts and self-contained packed update deltas; automatic L1 eligibility proof remains MIR-03.

**Tech Stack:** Rust (`roml`, `roml-highs`), existing PyO3 benchmark hooks, Cargo tests/bench fixtures, revisioned `Change`/`ModelOp` protocol.

**Spec:** `.planning/milestones/MIR-modeling-ir/DESIGN.md`, D-019 in `docs/release/ARCHITECTURE_DECISIONS.md`.

## Global constraints

- Preserve canonical one-cell-per-`(target,var)` semantics and per-entity stale-ID semantics.
- `VarSpan`/`ParamSpan` are trusted opaque block identities; arbitrary `(start,len)` construction is impossible outside core.
- Parameter creation does not gain a solver-facing add operation solely because block creation exists.
- No per-element block names are materialized in core.
- Eligible packed p-cells contain exactly one `scale × ParamId`; distinct params into one canonical cell are not packable.
- `set_parameters_bulk` preserves queue/commit/rollback semantics.
- Retained deltas are self-contained and replayable without consulting mutable live-model p-base storage.
- MIR-02 may use hand-verified L2 layout fixtures; do not prematurely implement MIR-03's production view/eligibility IR.
- General symbolic fallback remains correct and is the reference oracle.

---

## Task 1 — MIR-00 baseline and observability

**Files:**
- Modify: `python/benchmarks/bench_lpscale.py` and/or a narrowly scoped MIR benchmark fixture.
- Modify: `roml-python/src/expressions.rs`, `roml-python/src/arrays.rs`, `roml-python/src/model.rs` only as needed for read-only lowering counters.
- Modify: `src/model/coefficient.rs` / `src/model/mod.rs` only as needed for propagation counters.
- Create: `.planning/milestones/MIR-modeling-ir/evidence/BASELINE.md`.

**Produces:** measured current construction/lowering/reprice path and a minimal diagnostics surface sufficient to distinguish packed vs general work.

- [ ] Add characterization assertions for the current BESS objective `rm.sum(price * (discharge - charge))`; the test must report which lowering route was selected instead of assuming `PackedSymbolic`.
- [ ] Run the characterization before any storage redesign and record exact base SHA and command output.
- [ ] Add/read counters for lowering (`numeric_bulk`, `parametric_bulk`, `general_affine`) and propagation work (`param_position_lookups`, `overlay_lookups`, `value_expr_evals`, journal/ModelOp counts). Counters must not alter mathematical semantics.
- [ ] Run a BESS300×96 reprice fixture for 100 cycles: 28,800 changed price parameters, 57,600 affected objective cells. Record wall-time distribution and counter totals.
- [ ] Record current scalar-vs-bulk transaction/revision behavior and the current packed-objective propagation shape in `BASELINE.md`.
- [ ] Run existing core/Python focused tests touched by instrumentation; commit MIR-00 evidence and diagnostics separately.

**MIR-00 gate:** IR-01 is evidenced; current behavior is measured, not inferred.

## Task 2 — Trusted span allocation in stores

**Files:**
- Create: `src/bulk.rs` for opaque L2 span/block descriptors shared by core callers.
- Modify: `src/lib.rs` for the intended L2 exposure only.
- Modify: `src/id/arena.rs` for reserve/block allocation support without exposing arbitrary span construction.
- Modify: `src/model/variable.rs`, `src/model/parameter.rs` for store-level block insertion.
- Test: in-module arena/store tests plus API visibility/compile tests as appropriate.

**Produces:** `VarSpan`, `ParamSpan`, store-level sequential block allocation, and per-member liveness reconstruction.

- [ ] Write failing tests proving users cannot construct a span from `(start,len)`, a fresh block is contiguous, and deleting one member makes only that member stale.
- [ ] Add the minimum arena/store block primitive: validate inputs outside mutation, reserve once, append sequentially, and return an opaque span.
- [ ] Add parameter block insertion with finite-value prevalidation and no journal semantics at the store level.
- [ ] Add variable block insertion supporting uniform and per-element bounds/domain input needed by the public L2 API; preserve scalar store behavior.
- [ ] Prove reconstructed member IDs are checked through existing arena liveness/generation semantics; do not introduce a span epoch.
- [ ] Run all ID/store tests and commit the store layer independently.

## Task 3 — Model variable-block API and packed revision op

**Files:**
- Modify: `src/model/mod.rs`, `src/model/changelog.rs`, `src/delta.rs`.
- Modify: identity delta compiler in `src/model/mod.rs` (`compile_change`).
- Modify: backend/session application code that matches exhaustive `ModelOp` variants, including `roml-highs` and reference/in-memory projections.
- Test: core changelog/delta/snapshot tests and backend differential tests.

**Produces:** `Model::add_variable_block(...) -> VarSpan`, one packed `Change::VariableBlockAdded`, one self-contained `ModelOp::AddVariableBlock`.

- [ ] Write a failing n=100k test: journal growth for the block is one variable-block change, not n `VariableAdded` entries; delta likewise contains one packed op.
- [ ] Add whole-block bounds/type validation before mutation; invalid element leaves model, changelog and revision unchanged.
- [ ] Implement the packed change/op with owned/shared solver-facing bounds/type payload; adapters may expand internally but may not query live model state.
- [ ] Add scalar-vs-block snapshot equivalence for n=1 and representative mixed bounds; include removal/stale-member coverage.
- [ ] Run all exhaustive `Change`/`ModelOp` matcher tests plus core/HiGHS focused suites; commit the model/delta layer.

## Task 4 — Model parameter-block API without creation revision drift

**Files:**
- Modify: `src/model/mod.rs`, `src/model/parameter.rs`, `src/bulk.rs`.
- Test: parameter creation/revision/name regression tests.

**Produces:** `Model::add_parameter_block(&[f64]) -> ParamSpan` with bulk store mutation but the same creation/journal semantics as scalar `add_parameter`.

- [ ] Write a failing differential test comparing scalar parameter creation vs block creation: neither introduces solver-facing parameter-add operations or an otherwise new revision solely for existence.
- [ ] Implement whole-buffer finite-value validation, reserve once and sequential allocation.
- [ ] Verify current borrowed scalar `parameter_name()` behavior remains unchanged and no `name[i]` strings are created for blocks.
- [ ] Run parameter/transaction/snapshot regression tests and commit MIR-01 closure evidence.

**MIR-01 gate:** IR-02…IR-07 evidenced; existing constant-row bulk tests pass unmodified.

## Task 5 — Parametric row canonicalization and packed p-base block insertion

**Files:**
- Modify: `src/model/coefficient.rs`, `src/model/mod.rs`.
- Modify: `src/delta.rs`, `src/model/changelog.rs` for packed parametric row construction payloads.
- Test: coefficient canonicalization, atomicity, row journal/delta tests.

**Produces:** packed parametric row construction and an internal mixed-row commit seam without duplicate physical canonical cells.

- [ ] Write failing tests for same-var/same-param duplicate scale merge, zero-after-merge removal, merged nonfinite rejection, and same-var/distinct-param typed not-packable rejection with zero mutation.
- [ ] Add pure parametric canonicalization analogous to constant-row canonicalization, but preserving the one-parameter-per-packed-cell invariant.
- [ ] Add whole-block preflight for bounds, variable/parameter liveness and finiteness before row identities or coefficient storage mutate.
- [ ] Add packed parametric row journal/delta payloads that are self-contained for construction replay.
- [ ] Add an internal mixed-row commit primitive that allocates row targets once, stores disjoint constant and parametric canonical cells in their respective bases, and rejects/falls back on same-cell collision.
- [ ] Differential-test packed construction against equivalent scalar/general `ValueExpr` rows and commit.

## Task 6 — L2 dependency layouts and canonical dependency authority

**Files:**
- Modify: `src/bulk.rs` for `StridedMap`/layout witness descriptors.
- Modify: `src/model/coefficient.rs` for stored `ParamDepBlock`/block-dependency index.
- Modify: existing packed objective insertion path in `src/model/mod.rs`.
- Test: forged-layout rejection, dependency query, scalar update and shadow/removal tests.

**Produces:** validated block dependency descriptors for eligible packed p-base families, without `param_positions` population on that path.

- [ ] Write direct/core fixtures that hand-construct known-valid L2 layouts; also forge wrong parameter span, wrong coefficient run/stride and wrong canonical target/var topology layouts and require typed atomic rejection.
- [ ] Resolve a valid witness only after canonicalization; stored `ParamDepBlock` must use core/L2 descriptors, never MIR-03 `ParamView`/Python types.
- [ ] Retrofit existing packed parametric objective insertion so an eligible objective can store block dependencies while preserving current objective construction semantics.
- [ ] Ensure eligible cells are absent from per-cell `param_positions`; add counters/assertions.
- [ ] Make `for_param`/dependency introspection/removal/shadow logic semantically complete across block dependencies and sparse/overlay dependencies.
- [ ] Prove scalar `set_parameter` on one member of a block updates all of that parameter's live packed cells correctly without rebuilding per-cell reverse lists.
- [ ] Commit dependency storage separately from propagation.

## Task 7 — Transactional block parameter updates and self-contained patch deltas

**Files:**
- Modify: `src/model/transaction.rs` (or the canonical transaction owner used by `Model`).
- Modify: `src/model/mod.rs`, `src/model/changelog.rs`, `src/delta.rs`, `src/model/coefficient.rs`.
- Modify: reference compiler/projection and `roml-highs` adapter for the new packed parameter-value/coeff-patch operation shape.
- Test: transaction atomicity, retained replay, snapshot equivalence and scalar-vs-bulk differentials.

**Produces:** `set_parameters_bulk(span, values)` queueing plus one packed parameter-value change and one self-contained packed coefficient-patch batch at commit.

- [ ] Write failing queue/rollback/commit tests: bulk update is pending until commit, rollback restores no changes, invalid span/value/length is atomic.
- [ ] Add transaction storage that can retain block updates without expanding them into n scalar map insertions merely to represent the request; define deterministic interaction if scalar and block writes target the same parameter before commit.
- [ ] Implement block propagation as O(n) strided arithmetic over stored dependency descriptors; dead/shadowed cells are skipped or emitted through correct overlay/general semantics.
- [ ] Journal one packed parameter-value change for the committed block and one packed coefficient-patch batch for all affected eligible dependency blocks. Do not journal per-cell coefficient changes on this path.
- [ ] Make the coefficient-patch `ModelOp` self-contained: include/share immutable `(target,var,new_value)` topology/value data required by adapters; do not store only mutable p-base positions.
- [ ] Update reference and HiGHS application paths; native APIs may receive expanded calls, but canonical delta count remains packed.
- [ ] Prove retained-delta replay without the live model equals snapshot projection, including a shadowed-cell case and a mixed eligible/general dependency case.
- [ ] Run the direct MIR-02 BESS-shaped fixture and require propagation counters `param_position_lookups=0`, `overlay_lookups=0`, `value_expr_evals=0`, `coefficient_patch_batches=1` for the eligible family.

## Task 8 — MIR-02 qualification and handoff

**Files:**
- Create/update: `.planning/milestones/MIR-modeling-ir/evidence/MIR-01-REPORT.md`, `MIR-02-REPORT.md`.
- Modify: `.planning/milestones/MIR-modeling-ir/STATE.md` and requirement evidence references.

**Produces:** reviewed tranche-1 core with exact evidence; no MIR-03 feature leakage.

- [ ] Run `cargo fmt --all -- --check`.
- [ ] Run `cargo check -p roml --all-targets` and `cargo clippy -p roml --all-targets -- -D warnings`.
- [ ] Run `cargo test -p roml --all-targets`; run all touched `roml-highs` tests and Python regression tests required by MIR-00 diagnostics.
- [ ] Run rustdoc with warnings denied and `git diff --check`.
- [ ] Re-run scalar/general-path differential oracles, retained-delta replay, snapshot equivalence, stale-member tests and direct eligible-block counters on the exact review head.
- [ ] Request independent review; resolve all P0/P1 findings before marking tranche 1 complete.
- [ ] Record residual performance/correctness limitations and the exact contract MIR-03 may consume. Do not claim automatic high-level eligibility before MIR-03 implements `try_param_block_layout`.

**Tranche-1 exit:** IR-01…IR-17 have evidence and the exact head is green/reviewed under the packet's normal gates.
