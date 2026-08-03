---
phase: 25-semantic-ir-foundation
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/identity.rs
  - src/metadata.rs
  - src/function/mod.rs
  - src/function/scalar.rs
  - src/function/set.rs
  - src/construct/mod.rs
  - src/model/mod.rs
  - src/model/constraint.rs
  - src/model/changelog.rs
  - src/solution/metadata.rs
  - src/snapshot.rs
  - src/delta.rs
  - src/lib.rs
  - tests/m3_baseline_characterization.rs
  - tests/lineage_metadata.rs
  - tests/semantic_ir.rs
  - docs/release/evidence/M3_P25_SEMANTIC_IR.md
autonomous: false
requirements:
  - SM-01.1
  - SM-01.2
  - SM-01.3
  - SM-01.4
  - SM-01.5
  - SM-01.6
  - SM-02.1
  - SM-02.2
  - SM-02.3
  - SM-02.5
  - SM-02.7
  - SM-15.1
---

# Phase 25 — Canonical Semantic IR, Identities, and Metadata

> **For agentic workers:** this phase is a SERIAL chain. Execute Task 1 -> Task 2 -> Task 3 -> Task 4 in order with the TDD protocol from `EXECUTION.md`. Do not parallelize tasks within this phase. Stop after Task 4 and request independent review before marking the phase done.

**Goal:** establish semantic canonical state before adding workflows.

**Requirements:** SM-01.1, SM-01.2, SM-01.3, SM-01.4, SM-01.5, SM-01.6, SM-02.1, SM-02.2, SM-02.3, SM-02.5 (foundations), SM-02.7, SM-15.1 (foundations).

## Files

Create:

- `docs/release/evidence/M3_P25_SEMANTIC_IR.md` — phase evidence file; created empty before implementation (per `EXECUTION.md`), appended as work proceeds.
- `tests/m3_baseline_characterization.rs` — untouched-tree characterization of M2 ordinary behavior.
- `src/identity.rs` — `ModelLineageId`, `ModelInstanceId`, `ConstructId`.
- `src/metadata.rs` — `ModelSource`, `EntityMetadata`, `EntityRef`.
- `tests/lineage_metadata.rs` — lineage/instance/metadata tests.
- `src/function/mod.rs`, `src/function/scalar.rs`, `src/function/set.rs` — function-in-set seam.
- `tests/semantic_ir.rs` — function-in-set conversion and snapshot/delta round-trip tests.
- `src/construct/mod.rs` — generation-safe construct arena and lifecycle.

Modify:

- `src/model/mod.rs` — manual `Default` and `Clone` on `Model` (lineage preserved, instance reallocated); construct and metadata accessors.
- `src/model/constraint.rs` — canonical function-in-set conversion path for `.le`/`.ge`/`.eq`/`.between`.
- `src/model/changelog.rs` — self-contained construct canonical change events.
- `src/solution/metadata.rs` — `SolveMetadata` records `model_lineage` and `model_instance`.
- `src/snapshot.rs` — semantic function/set and construct entries.
- `src/delta.rs` — semantic function/set and construct delta operations.
- `src/lib.rs` — module wiring and reviewed public re-exports.
- `tests/` — extend semantic tests where Task 4 requires.

## Task 1 — Capture baseline and characterization

**Phase:** P25  **Requirements:** SM-01.5, SM-01.6, SM-15.1 (foundations)

**Read first:**
- `docs/release/evidence/M2_P20_BASELINE.md` — prior baseline convention and recorded format.
- `.planning/milestones/M3-semantic-modeling-workflows/EXECUTION.md` — baseline command matrix (§ "Baseline command matrix").
- `.planning/phases/20-public-api-contract/20-PLAN.md` — prior `cargo public-api`/`cargo package --list` capture convention.
- `src/lib.rs`, `Cargo.toml` — the ordinary surface being characterized.

**TDD order** (per `EXECUTION.md`):

1. Record `git rev-parse HEAD`, Rust/Cargo versions, target, and OS in the evidence file.
2. Run the untouched baseline matrices for `roml` and `roml-highs`; record exact commands and concise results.
3. Capture `cargo public-api -p roml`, `cargo public-api -p roml-highs`, and `cargo package --list` output for both crates.
4. Write `tests/m3_baseline_characterization.rs` covering: fluent linear modeling (`Model::new` + `add_variable` + `add_constraint` + `maximize`), deterministic snapshot round-trip, parameter update (`set_parameter`), objective constant propagation, solution metadata, and one-rebuild-retry behavior.
5. Run the characterization test and record the expected result (must pass on the untouched tree — this is characterization, not a red/green feature test).
6. Commit the evidence + characterization as one unit.
7. Update evidence and traceability before proceeding to Task 2.

- [ ] Record exact base SHA, Rust/Cargo versions, and supported HiGHS modes.
- [ ] Run untouched fmt/check/clippy/test/doc commands for `roml` and `roml-highs`.
- [ ] Capture `cargo public-api` and `cargo package --list` output.
- [ ] Add characterization tests for fluent linear modeling, deterministic snapshot, parameter update, objective constant, solution metadata, and one-rebuild-retry behavior.
- [ ] Stop when the baseline is fully captured: exact base SHA, tool versions, untouched command outputs, and passing characterization tests, all recorded in `M3_P25_SEMANTIC_IR.md`.
- [ ] Commit as `test(m3): capture semantic modeling baseline`.

**Stopping condition:** the baseline is fully captured — exact base SHA, tool versions, untouched command outputs, and passing characterization tests all recorded in `M3_P25_SEMANTIC_IR.md`.

**Commit:** `test(m3): capture semantic modeling baseline`

**Verification:**

```bash
cargo test -p roml --test m3_baseline_characterization -- --nocapture
```

**Acceptance criteria:**
- `cargo test -p roml --test m3_baseline_characterization -- --nocapture` exits 0 on the untouched tree.
- `docs/release/evidence/M3_P25_SEMANTIC_IR.md` exists and records: base SHA, Rust/Cargo versions, untouched fmt/check/clippy/test/doc results for `roml` and `roml-highs`, `cargo public-api` and `cargo package --list` outputs.
- The baseline matrices (`cargo fmt --all -- --check`, `cargo check -p roml --all-targets`, `cargo clippy -p roml --all-targets -- -D warnings`, `cargo test -p roml --all-targets`, `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps`, `cargo package --list -p roml`, and the HiGHS equivalents) all pass on the untouched tree.

## Task 2 — Add lineage, instance identity, and metadata

**Phase:** P25  **Requirements:** SM-02.1, SM-02.2 (foundations), SM-02.3, SM-02.7

**Read first:**
- `docs/superpowers/specs/2026-08-02-semantic-modeling-and-solve-workflows-design.md` §4 "Identity model" and §5 "Metadata and provenance" — authoritative type shapes.
- `src/model/mod.rs` — current `#[derive(Clone, Debug, Default)]` on `Model` (lines ~124–150); this must become a manual `Default` and `Clone`.
- `src/solution/metadata.rs` — `SolveMetadata`; add `model_lineage` and `model_instance` fields.
- `src/id/mod.rs` — existing generation-safe ID convention for naming consistency.
- `src/lib.rs` — module wiring and re-export review.

**TDD order** (per `EXECUTION.md`):

1. Write failing tests in `tests/lineage_metadata.rs`:
   - `Model::new()` (and `Model::named(...)`) allocate a distinct `ModelLineageId` per independent model, and a distinct `ModelInstanceId`.
   - `clone()` preserves lineage (`clone.lineage() == original.lineage()`) but allocates a new instance ID (`clone.instance() != original.instance()`).
   - Two independent models never share a lineage ID.
   - Metadata setters/getters round-trip for `description`, `group`, `tags`, and `source` per entity.
   - `SolveMetadata` records every available state ID (`model_lineage`, `model_instance`, `model_revision`).
2. Run `cargo test -p roml --test lineage_metadata` and record the expected failure (missing types/fields).
3. Implement:
   - `src/identity.rs`: `pub struct ModelLineageId(u64);`, `pub struct ModelInstanceId(u64);`, `pub struct ConstructId(u64);` — each `#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]` with opaque inner `u64`. Allocate through checked atomic counters with zero reserved (first issued id is 1; overflow returns a typed error rather than wrapping).
   - `src/metadata.rs`: `ModelSource { module: Option<String>, file: Option<String>, line: Option<u32>, external_key: Option<String> }` and `EntityMetadata { description: Option<String>, group: Option<String>, tags: Vec<String>, source: Option<ModelSource> }` — each `#[derive(Clone, Debug, Default, PartialEq, Eq)]` per the approved design §5. `EntityRef { Variable(Variable), Constraint(Constraint), Objective(Objective), Parameter(Parameter), Construct(Construct) }` — `#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]` per the plan's interface contract (required as a `HashMap` key; `Default` is not derivable on a payload-carrying enum without a designated `#[default]` variant).
   - Manual `impl Default for Model` (allocates fresh lineage + instance) and `impl Clone for Model` (preserves lineage, allocates new instance, clones all fields).
   - Metadata store keyed by `EntityRef`; metadata changes are canonical but non-solver-affecting (they never advance backend state — see `EXECUTION.md` § "Incremental semantics").
   - Define `ConstructId(u64)` in `src/identity.rs` (entity identity per design §4.4); the `EntityRef::Construct` variant becomes usable once the construct arena lands in Task 4.
   - Extend `SolveMetadata` in `src/solution/metadata.rs` with `model_lineage: ModelLineageId` and `model_instance: ModelInstanceId`; update `Default`.
4. Run `cargo test -p roml --test lineage_metadata` (must pass), then `cargo test -p roml --all-targets` (must pass).
5. Export lineage/instance/metadata types through reviewed public surfaces in `src/lib.rs`.
6. Update evidence and traceability.
7. Commit one coherent unit.

- [ ] Write failing tests: independent models differ in lineage/instance; clone preserves lineage but receives new instance; solution records all state IDs.
- [ ] Allocate opaque IDs through checked atomic counters with zero reserved.
- [ ] Implement manual `Default` and `Clone` for `Model`.
- [ ] Add metadata store keyed by `EntityRef`; metadata changes are canonical but non-solver-affecting.
- [ ] Define `ConstructId(u64)` in `src/identity.rs` (entity identity per design §4.4); the `EntityRef::Construct` variant becomes usable once the construct arena lands in Task 4.
- [ ] Export lineage/instance/metadata types through reviewed public surfaces.
- [ ] Stop when independent models never share a lineage, clone tests pass, and solution metadata records every state ID.
- [ ] Commit as `feat(model): add lineage instance and metadata`.

**Stopping condition:** independent models never share a lineage, clone tests pass (lineage preserved, new instance), and `SolveMetadata` records every state ID.

**Commit:** `feat(model): add lineage instance and metadata`

**Verification:**

```bash
cargo test -p roml --test lineage_metadata
cargo test -p roml --all-targets
```

**Acceptance criteria:**
- `cargo test -p roml --test lineage_metadata` exits 0; `cargo test -p roml --all-targets` exits 0.
- `src/identity.rs` defines `pub struct ModelLineageId(u64)`, `pub struct ModelInstanceId(u64)`, and `pub struct ConstructId(u64)`, each `Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord`, allocated by checked atomic counters with zero reserved.
- `src/metadata.rs` defines `ModelSource`, `EntityMetadata`, and `EntityRef` with the exact fields from design §5.
- `src/model/mod.rs` has manual `Default` and `Clone` for `Model`; cloning preserves lineage and allocates a new instance ID.
- `src/solution/metadata.rs` `SolveMetadata` records `model_lineage`, `model_instance`, and `model_revision`.

## Task 3 — Add function-in-set canonical constraints

**Phase:** P25  **Requirements:** SM-01.1, SM-01.2, SM-01.4, SM-01.5

**Read first:**
- `docs/superpowers/specs/2026-08-02-semantic-modeling-and-solve-workflows-design.md` §6 "Function-in-set seam" — authoritative enum/struct shapes and the `#[non_exhaustive]` markers.
- `src/expr/linear.rs` — `ConstraintExprExt::eq/le/ge/between` returning `ConstraintSpec` (the conversion sources for `IntoScalarFunction`).
- `src/model/constraint.rs` — existing constraint representation and bounds.
- `src/model/changelog.rs` — `Change` enum; the coefficient index remains authoritative, so this task must not create a parallel coefficient authority.
- `src/snapshot.rs`, `src/delta.rs` — the snapshot entry structs and `ModelOp` enum to extend with semantic function/set entries.
- `src/model/mod.rs` — `take_snapshot` (line ~1307) and changelog integration.

**TDD order** (per `EXECUTION.md`):

1. Write failing conversion tests in `tests/semantic_ir.rs`:
   - `(x + y).le(4.0)` converts to `FunctionConstraint { function: ScalarFunction::Linear(expr), set: ScalarSet::LessEqual(ValueExpr::from(4.0)) }`.
   - `.ge(lower)` -> `ScalarSet::GreaterEqual`; `.eq(rhs)` -> `ScalarSet::EqualTo`; `.between(lower, upper)` -> `ScalarSet::Interval`.
   - Round-trip: a constraint added through the ordinary builder reproduces the same `LinExpr` from the coefficient index (the index stays authoritative).
2. Run `cargo test -p roml --test semantic_ir` and record the expected failure (missing types).
3. Implement:
   - `src/function/mod.rs`, `src/function/scalar.rs`, `src/function/set.rs`: `#[non_exhaustive] pub enum ScalarFunction { Linear(LinExpr) }`, `#[non_exhaustive] pub enum ScalarSet { LessEqual(ValueExpr), GreaterEqual(ValueExpr), EqualTo(ValueExpr), Interval { lower: ValueExpr, upper: ValueExpr } }`, `pub struct FunctionConstraint { pub function: ScalarFunction, pub set: ScalarSet }`, and `pub trait IntoScalarFunction { fn into_scalar_function(self) -> ScalarFunction; }` — exactly as design §6. M3 implements only `ScalarFunction::Linear`.
   - Keep the existing coefficient index authoritative in P25 and reconstruct linear functions deterministically from it; add no second coefficient authority.
   - Extend canonical snapshot/delta with semantic function/set data while invariant-checking transitional legacy fields (the legacy constraint/cell fields remain during the transition; every such field carries an invariant assertion that it is consistent with the reconstructed function/set).
4. Run `cargo test -p roml --test semantic_ir` (must pass), then `cargo test -p roml --test m3_baseline_characterization` and `cargo test -p roml --all-targets` (must pass — SM-01.5: the ordinary M2 `LinExpr` path stays green).
5. Wire the new modules in `src/lib.rs`.
6. Update evidence and traceability.
7. Commit one coherent unit.

- [ ] Write failing conversion tests for `.le`, `.ge`, `.eq`, and `.between`.
- [ ] Implement `ScalarFunction`, `ScalarSet`, `FunctionConstraint`, and `IntoScalarFunction`.
- [ ] Keep the existing coefficient index authoritative in P25 and reconstruct linear functions deterministically.
- [ ] Extend canonical snapshot/delta with semantic function/set data while invariant-checking transitional legacy fields.
- [ ] Stop when the coefficient index remains authoritative, every transitional legacy field is invariant-checked, and all conversion tests pass.
- [ ] Commit as `feat(model): add linear function-in-set semantics`.

**Stopping condition:** the coefficient index remains authoritative, every transitional legacy field is invariant-checked, and all conversion tests pass.

**Commit:** `feat(model): add linear function-in-set semantics`

**Verification:**

```bash
cargo test -p roml --test semantic_ir
cargo test -p roml --test m3_baseline_characterization
cargo test -p roml --all-targets
```

**Acceptance criteria:**
- `cargo test -p roml --test semantic_ir` exits 0; `cargo test -p roml --test m3_baseline_characterization` and `cargo test -p roml --all-targets` still exit 0 (SM-01.5 preserved).
- `src/function/` defines `#[non_exhaustive] ScalarFunction::Linear(LinExpr)`, `#[non_exhaustive] ScalarSet` with the four variants from design §6, `FunctionConstraint`, and `IntoScalarFunction`.
- The coefficient index remains the single coefficient authority; linear functions reconstruct deterministically from it.
- `src/snapshot.rs` and `src/delta.rs` carry semantic function/set entries, and every transitional legacy field is guarded by an invariant check.

## Task 4 — Add canonical construct lifecycle

**Phase:** P25  **Requirements:** SM-01.3, SM-01.4, SM-01.6, SM-02.5 (foundations)

**Read first:**
- `docs/superpowers/specs/2026-08-02-semantic-modeling-and-solve-workflows-design.md` §7 "Canonical semantic constructs" — `ConstructId` arena, `ConstructKind`, activity state, metadata, parameter dependencies, snapshot/delta representation.
- `src/identity.rs` (Task 2 artifact) — `ConstructId`.
- `src/metadata.rs` (Task 2 artifact) — `EntityRef::Construct` variant.
- `src/model/mod.rs` — Model invariants (live references, metadata, auxiliary ownership) and `take_snapshot`.
- `src/model/changelog.rs` — `Change` enum; add self-contained construct canonical changes.
- `src/snapshot.rs`, `src/delta.rs` — extend with construct entries.
- `docs/release/evidence/M3_P25_SEMANTIC_IR.md` — finish this phase evidence and request independent review.

**TDD order** (per `EXECUTION.md`):

1. Write add/clone/snapshot/activity/remove/stale-generation tests in `tests/semantic_ir.rs` (or a focused construct section) using a private fixture payload:
   - Add returns a stable `ConstructId`; the same payload reads back equal.
   - Clone preserves constructs with the same ids and activity.
   - Snapshot/delta round-trip preserves every construct entry.
   - Activity toggling is reflected in snapshot.
   - Remove invalidates the id; stale ids are rejected with a typed error.
   - The construct store survives rebuild (snapshot -> empty model -> restore -> same constructs).
2. Run the failing tests and record the expected failures.
3. Implement:
   - One generation-safe construct arena in `src/construct/mod.rs`; `pub type Construct = ConstructId;`, `#[non_exhaustive] pub enum ConstructKind { ... }` per design §7 (indicator/reification/minmax/absolute/boolean/cardinality/binary product/PWL/soft variants declared as the extension surface — P25 stores only private fixture payloads until the per-construct modules land in P30/P32/P33).
   - `ConstructEntry { pub id: Construct, pub kind: ConstructKind, pub active: bool }` and `FormulationPreference { Auto, Portable, NativeRequired }`.
   - Self-contained construct canonical changes in `src/model/changelog.rs`.
   - Derive parameter dependencies from the payload; validate any cache with an invariant proving equality.
   - Extend model invariants for live references, metadata, and auxiliary ownership.
   - Extend `src/snapshot.rs` and `src/delta.rs` with construct entries; `EntityRef::Construct` becomes usable now.
4. Run `cargo test -p roml --all-targets`, `cargo clippy -p roml --all-targets -- -D warnings`, `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps`, and `cargo public-api -p roml` — all must exit 0.
5. Finish `docs/release/evidence/M3_P25_SEMANTIC_IR.md` with the P25 evidence bundle (M2 compile matrix, lineage tests, metadata tests, function-in-set round-trip, construct lifecycle tests, public API diff, invariant-checker results).
6. Request independent review at the P25 phase boundary.
7. Commit one coherent unit.

- [ ] Write add/clone/snapshot/activity/remove/stale-generation tests using a private fixture payload.
- [ ] Implement one generation-safe construct arena.
- [ ] Add self-contained construct canonical changes.
- [ ] Derive parameter dependencies from payload; validate any cache.
- [ ] Extend model invariants for live references, metadata, and auxiliary ownership.
- [ ] Finish P25 evidence and request independent review.
- [ ] Commit as `feat(model): add canonical construct lifecycle`.

> **Scope note:** P25 ships `src/construct/mod.rs` plus the construct arena and lifecycle. The per-construct modules (`indicator`, `minmax`, `absolute`, `boolean`, `product`, `piecewise_linear`, `soft`) land in Tasks 13/16/17/18 of the implementation plan; P25 must not pre-implement their formulations.

**Stopping condition:** every construct fixture survives clone/snapshot/delta/activity/remove/rebuild (the ROADMAP P25 gate, verbatim), all phase gates pass, P25 evidence is finished, and independent review has been requested.

**Commit:** `feat(model): add canonical construct lifecycle`

**Verification:**

```bash
cargo test -p roml --all-targets
cargo clippy -p roml --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
cargo public-api -p roml
```

**Acceptance criteria:**
- `cargo test -p roml --all-targets`, `cargo clippy -p roml --all-targets -- -D warnings`, `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps`, and `cargo public-api -p roml` all exit 0.
- `src/construct/mod.rs` defines `Construct` (alias for `ConstructId`), `#[non_exhaustive] ConstructKind`, `ConstructEntry { id, kind, active }`, and `FormulationPreference { Auto, Portable, NativeRequired }`.
- `src/snapshot.rs` and `src/delta.rs` carry construct entries; `src/model/changelog.rs` carries self-contained construct canonical changes.
- Every construct fixture survives clone/snapshot/delta/activity/remove/rebuild.
- `docs/release/evidence/M3_P25_SEMANTIC_IR.md` is complete per the P25 evidence structure in `TRACEABILITY.md` and `EXECUTION.md`.
- No backend index, native handle, selected Big-M, or solve overlay exists in any canonical state added by this phase (SM-01.6).

## Verification

Phase-level checks (all must exit 0):

```bash
cargo test -p roml --test m3_baseline_characterization -- --nocapture
cargo test -p roml --test lineage_metadata
cargo test -p roml --test semantic_ir
cargo test -p roml --all-targets
cargo clippy -p roml --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
cargo public-api -p roml
```

Baseline matrix (untouched tree, recorded in Task 1 evidence):

```bash
cargo fmt --all -- --check
cargo check -p roml --all-targets
cargo clippy -p roml --all-targets -- -D warnings
cargo test -p roml --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
cargo package --list -p roml
```

Per P25 mandatory checks in `EXECUTION.md`: canonical snapshot/delta tests; model invariant property tests; M2 compile-pass examples; public API diff.

## Waves and parallelization

P25 is a **serial chain**: `Task 1 -> Task 2 -> Task 3 -> Task 4`. There is exactly one plan (`wave: 1`, `depends_on: []`); there is no intra-phase wave parallelization.

Rationale:

- **Task 1 must run first and alone.** It captures the untouched tree (`cargo test -p roml --test m3_baseline_characterization` plus the baseline matrix and public API/package outputs) before any source modification. Any parallel task touching `src/` would invalidate the baseline evidence.
- **Tasks 2, 3, and 4 share ownership of the same canonical-state files** — `src/model/mod.rs`, `src/lib.rs`, `src/snapshot.rs`, and `src/delta.rs` are all modified by Task 2 (model/instance/metadata), Task 3 (function/set snapshot/delta), and Task 4 (construct snapshot/delta). Parallel agents editing these files would produce merge conflicts on every shared file.
- **Tasks 2 -> 3 -> 4 also have a type dependency chain:** Task 3's snapshot/delta extension and Task 4's `EntityRef::Construct` usage both build on types Task 2 creates (`ConstructId` in `src/identity.rs`, `EntityRef` in `src/metadata.rs`), and Task 4 extends the snapshot/delta entries Task 3 introduces.

The explicit choice is **merge-conflict minimization**: serial execution of one coherent plan per phase (matching `D26 — One active implementation phase by default` and the P25 evidence structure). Each task commits one reviewable unit before the next begins.

## Review gates

Per `EXECUTION.md` two-stage review, P25 receives **independent review at the phase boundary after Task 4**.

- **Pass 1 — Specification and correctness:** requirement coverage (SM-01.1–SM-01.6, SM-02.1–SM-02.3, SM-02.5 foundations, SM-02.7, SM-15.1 foundations); semantic correctness; invariant preservation; unsupported/error behavior; origin completeness (SM-02.5 foundation); API coherence; test quality.
- **Pass 2 — Integration and operations:** incremental/rebuild behavior; failure recovery; cross-platform/version behavior; public API diff; package/docs impact; performance evidence; migration accuracy.

**Blocking rules:**

- P0/P1 findings **block merge**.
- P2 findings may merge only when explicitly accepted and scheduled.
- `autonomous: false` — the executor pauses after Task 4 and does not declare the phase complete until both review passes resolve to no P0/P1 findings.

Evidence requirement: `docs/release/evidence/M3_P25_SEMANTIC_IR.md` must record reviewer findings and dispositions before the gate result is marked pass (per `EXECUTION.md` § "Evidence file structure").

## Artifacts this phase produces

New modules and symbols (all names/signatures from the approved design):

- `src/identity.rs` — `pub struct ModelLineageId(u64)`, `pub struct ModelInstanceId(u64)`, `pub struct ConstructId(u64)` (opaque, checked atomic counters, zero reserved).
- `src/metadata.rs` — `ModelSource { module, file, line, external_key }`, `EntityMetadata { description, group, tags, source }`, `EntityRef { Variable, Constraint, Objective, Parameter, Construct }`.
- `src/function/mod.rs`, `src/function/scalar.rs`, `src/function/set.rs` — `#[non_exhaustive] ScalarFunction::Linear(LinExpr)`, `#[non_exhaustive] ScalarSet`, `FunctionConstraint { function, set }`, `IntoScalarFunction`.
- `src/construct/mod.rs` — `Construct` (= `ConstructId`), `#[non_exhaustive] ConstructKind`, `ConstructEntry { id, kind, active }`, `FormulationPreference { Auto, Portable, NativeRequired }`, generation-safe construct arena.
- `src/model/mod.rs` — manual `Default` and `Clone` for `Model` (lineage preserved on clone, new instance ID).
- `src/solution/metadata.rs` — `SolveMetadata` extended with `model_lineage` and `model_instance`.
- `src/snapshot.rs`, `src/delta.rs`, `src/model/changelog.rs` — semantic function/set and construct entries/operations.
- Test files: `tests/m3_baseline_characterization.rs`, `tests/lineage_metadata.rs`, `tests/semantic_ir.rs`.
- Evidence file: `docs/release/evidence/M3_P25_SEMANTIC_IR.md`.

## must_haves

Goal-backward verification (from the ROADMAP P25 gate, verbatim):

**Truths (observable behaviors):**

1. Existing linear models remain observationally equivalent.
2. Independent models reject cross-lineage assignments.
3. Clones share lineage but never instance identity.
4. Every construct fixture survives clone/snapshot/delta/activity/remove/rebuild.

**Artifacts (files that must exist):**

- `src/identity.rs`, `src/metadata.rs`, `src/function/**`, `src/construct/mod.rs`
- `tests/m3_baseline_characterization.rs`, `tests/lineage_metadata.rs`, `tests/semantic_ir.rs`
- `docs/release/evidence/M3_P25_SEMANTIC_IR.md`

**Key links (critical connections where breakage cascades):**

- Manual `Model::clone`/`Model::default` — lineage must be preserved across clone while the instance ID is reallocated; a derived `Clone` would silently copy the instance ID (D28 violation).
- Metadata store keyed by `EntityRef` — `EntityRef::Construct` stays unusable until Task 4's arena lands; metadata changes must be canonical but non-solver-affecting.
- Snapshot/delta semantic entries — must round-trip deterministically with every transitional legacy field invariant-checked; the coefficient index stays the single authority.
- Generation-safe construct arena — remove must invalidate the id and stale ids must be rejected with a typed error; rebuild must restore identical constructs.

## Gate

P25 passes when:

- Task 1 baseline is captured on the untouched tree and recorded in `M3_P25_SEMANTIC_IR.md`;
- Tasks 2, 3, and 4 complete in order with every acceptance criterion met;
- all phase-level verification commands exit 0;
- M2 ordinary linear models remain observationally equivalent (SM-01.5, SM-15.1 foundations);
- independent models reject cross-lineage assignments and clones share lineage but never instance identity (SM-02.1, SM-02.2, SM-02.7);
- metadata accessors and formatting tests pass (SM-02.3);
- function-in-set canonical constraints round-trip through snapshot/delta (SM-01.1, SM-01.2, SM-01.4);
- the construct store lifecycle and invariant tests pass (SM-01.3, SM-01.6, SM-02.5 foundations);
- the P25 evidence bundle and public API diff are recorded; and
- both independent review passes resolve with no P0/P1 findings.

No crate publication, tag, or release is part of this phase (SM-15.8 / M3 stopping condition).

## Output

Create `.planning/phases/25-semantic-ir-foundation/25-SUMMARY.md` when done, per the phase completion protocol.
