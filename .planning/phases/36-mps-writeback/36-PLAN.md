# Phase 36 — MPS Write-Back Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** deterministically serialize ROML linear LP/MILP models to free MPS and prove semantic/solve equivalence through ROML and native HiGHS round trips.

**Architecture:** extend `roml::io::mps` with a solver-free projection/writer pipeline. The writer validates representability before emitting a semantic model, canonicalizes ordering/numeric formatting, and exposes path/stream APIs. Qualification compares normalized ROML snapshots and native HiGHS `readModel` summaries using the P35 differential machinery.

**Tech Stack:** Rust std I/O, existing ROML model/snapshot APIs, existing P35 MPS reader/source types, `roml-highs` HiGHS differential utilities, existing Netlib submodule/corpus harness.

## Global Constraints

- P36 writes deterministic **free MPS only**.
- Supported semantic scope is standard linear LP/MILP only.
- `SemanticModel` export never silently compiles unsupported high-level constructs.
- Floating output is locale independent; non-finite values reject.
- Writer core remains solver-free.
- Stream writes may leave partial bytes on error; `write_path` must be transaction-like and not replace a prior file until full serialization succeeds.
- Existing P35 read behavior and public APIs remain source-compatible.
- No LP-format writer/parser, quadratic extension, publication, or release work in P36.

---

## File map

**Core files**

- Create `src/io/mps/write.rs` — public writer/options/errors and stream/path orchestration.
- Create `src/io/mps/write_projection.rs` — semantic model -> normalized MPS write document, representability validation.
- Create `src/io/mps/write_format.rs` — deterministic free-MPS record and numeric formatting.
- Modify `src/io/mps/mod.rs` — modules/re-exports only; keep reader definitions source-compatible.
- Modify `src/io/mod.rs` / `src/lib.rs` only if existing export pattern requires it.

**Tests**

- Create `tests/mps_write_contract.rs` — public API, deterministic bytes, errors, transaction behavior.
- Create `tests/mps_roundtrip.rs` — normalized ROML semantic round trips/metamorphic variants.
- Create `roml-highs/tests/mps_write_differential.rs` — native HiGHS structure/solve differential checks.
- Extend `roml-highs/examples/mps_corpus_qualification.rs` or create `mps_roundtrip_qualification.rs` — Netlib transcode qualification without changing P35 evidence semantics.

**Evidence/docs**

- Create `docs/release/evidence/P36_MPS_WRITEBACK_QUALIFICATION.md`.
- Create `.planning/phases/36-mps-writeback/36-REVIEW.md`, `36-VERIFICATION.md`, `36-SUMMARY.md` during execution.

---

### Task 36-00: Freeze writer public contract and characterize representability

**Files:**
- Create: `tests/mps_write_contract.rs`
- Create: `src/io/mps/write.rs`
- Modify: `src/io/mps/mod.rs`

**Interfaces:**

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MpsWriteTarget {
    #[default]
    SemanticModel,
    CompiledLinearFormulation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MpsWriteOptions {
    pub target: MpsWriteTarget,
    pub model_name: Option<String>,
    pub rhs_name: String,
    pub ranges_name: String,
    pub bounds_name: String,
}

#[derive(Clone, Debug, Default)]
pub struct MpsWriter {
    options: MpsWriteOptions,
}

impl MpsWriter {
    pub fn new() -> Self;
    pub fn with_options(options: MpsWriteOptions) -> Self;
    pub fn write<W: std::io::Write>(&self, model: &Model, output: W) -> Result<MpsWriteReport, MpsWriteError>;
    pub fn write_path<P: AsRef<std::path::Path>>(&self, model: &Model, path: P) -> Result<MpsWriteReport, MpsWriteError>;
}
```

`CompiledLinearFormulation` may initially return typed `UnsupportedTarget` until a later explicitly approved task; the enum is frozen only if review confirms it does not promise unavailable state. If review finds this premature, keep compiled export out of the public enum rather than ship a decorative option.

- [ ] Write compile/use tests proving `MpsWriter::new`, `with_options`, stream/path methods, report/error types, and root/module exports.
- [ ] Write failing tests that semantic export rejects unsupported canonical constructs rather than silently lowering them.
- [ ] Write failing tests that a normal primitive LP and MILP pass representability preflight.
- [ ] Run `cargo test -p roml --test mps_write_contract` and confirm failures are missing API/behavior, not fixture mistakes.
- [ ] Add minimal contract types and typed error variants with `Display`/`Error`.
- [ ] Run focused tests, clippy, rustdoc.
- [ ] Commit `feat(mps): define deterministic writer contract`.

**Review gate:** public options must all govern execution or be removed before next task.

---

### Task 36-01: Build normalized semantic write projection

**Files:**
- Create: `src/io/mps/write_projection.rs`
- Test: `tests/mps_write_contract.rs`
- Test: `tests/mps_roundtrip.rs`

**Produces:** private normalized structures with no textual layout concerns:

```rust
struct MpsWriteDocument {
    name: String,
    objective: WriteObjective,
    rows: Vec<WriteRow>,
    columns: Vec<WriteColumn>,
    rhs: Vec<WriteRhs>,
    ranges: Vec<WriteRange>,
    bounds: Vec<WriteBound>,
}
```

- [ ] Add a failing fixture for a named minimization LP with row senses, objective constant, ranged row, free/fixed/bounded variables.
- [ ] Add a failing MILP fixture covering integer and binary domains and an integer with nondefault bounds.
- [ ] Add failing tests for inactive/deleted entities according to current snapshot semantics; the writer must emit only the effective live model.
- [ ] Add failing tests for unsupported variable domains/construct state with exact entity-identifying errors.
- [ ] Implement one-pass projection from canonical snapshot/state; use canonical coefficient cells, never re-expand expression syntax.
- [ ] Normalize objective sense/coefficient/offset semantics to match P35 reader conventions.
- [ ] Generate one semantic row for ranged constraints; do not split them into two public rows.
- [ ] Verify duplicate canonical coefficient cells are impossible at this layer; assert invariant through existing model validation rather than ad hoc deduplication.
- [ ] Run focused tests and `cargo test -p roml --all-targets`.
- [ ] Commit `feat(mps): project semantic models for write-back`.

---

### Task 36-02: Deterministic free-MPS formatter

**Files:**
- Create: `src/io/mps/write_format.rs`
- Test: `tests/mps_write_contract.rs`
- Test: `tests/mps_roundtrip.rs`

**Formatting contract:**

- deterministic section order;
- deterministic entity order by stable ID;
- no line-layout dependency on host locale;
- shortest/round-trippable finite f64 representation accepted by P35 reader and HiGHS;
- normalize `-0.0` to `0`;
- whitespace deterministic;
- each logical matrix coefficient emitted once;
- deterministic marker placement for integer columns.

- [ ] Add golden-byte test for a small LP.
- [ ] Add golden-byte test for a mixed continuous/integer/binary model.
- [ ] Add repeated-write test asserting exact byte identity for 100 repeated writes.
- [ ] Add negative-zero and exponent-magnitude tests.
- [ ] Add randomized finite-f64 formatting round-trip test: formatter -> P35 numeric parser path -> same f64 bits/value according to documented tolerance policy.
- [ ] Implement formatter using `write!`/`BufWriter`; no third-party formatting dependency unless measured need is demonstrated.
- [ ] Run focused tests, fmt/clippy.
- [ ] Commit `feat(mps): format deterministic free MPS`.

---

### Task 36-03: Correct BOUNDS and integer marker emission

**Files:**
- Modify: `src/io/mps/write_projection.rs`
- Modify: `src/io/mps/write_format.rs`
- Test: `tests/mps_roundtrip.rs`

- [ ] Create table-driven failing tests for default continuous `[0,+inf]`, free, minus-infinity-only, upper-only, lower-only, fixed, finite interval, binary, integer defaults, integer free, integer custom bounds.
- [ ] Assert writer omits records only when MPS defaults reproduce ROML semantics exactly.
- [ ] Ensure `BV` is used only when domain and bounds semantics match a binary variable exactly.
- [ ] Ensure integer variables that cannot use `BV` are wrapped by deterministic `INTORG/INTEND` and receive explicit bound records whenever marker defaults would change semantics.
- [ ] Round-trip every table case through `MpsReader` and compare canonical domains.
- [ ] Commit `feat(mps): preserve domains in MPS write-back`.

---

### Task 36-04: Objective/RHS/RANGES exact semantics

**Files:** same core formatter/projection files.

- [ ] Add objective-offset tests for positive, zero, and negative constants and both objective senses.
- [ ] Verify writer uses the inverse of P35's documented `objective = c^T x - RHS(objective-row)` convention.
- [ ] Add every `RANGES` sense/sign case from `35-MPS-SEMANTICS.md` as write->read semantic tests.
- [ ] Add empty-objective model test.
- [ ] Add named objective/N-row tests.
- [ ] Implement minimal canonical record emission.
- [ ] Run all core MPS tests.
- [ ] Commit `feat(mps): preserve objective and ranged-row semantics`.

---

### Task 36-05: Transactional path writing and diagnostics

**Files:**
- Modify: `src/io/mps/write.rs`
- Test: `tests/mps_write_contract.rs`

- [ ] Add test with an existing destination file and injected writer/projection failure; destination contents must remain unchanged.
- [ ] Add test for successful replacement.
- [ ] Add test for path/open/rename errors preserving path and underlying cause in `MpsWriteError`.
- [ ] Implement preflight projection before opening/replacing destination where possible.
- [ ] Serialize to a uniquely named sibling temp file, flush/sync according to repository policy, then rename atomically where supported.
- [ ] Clean temp files on ordinary failure; preserve primary error if cleanup also fails using existing project error-composition conventions.
- [ ] Document stream partial-write semantics separately.
- [ ] Commit `feat(mps): make path write-back transactional`.

---

### Task 36-06: ROML normalized semantic round-trip oracle

**Files:**
- Create: `tests/mps_roundtrip.rs`
- Reuse: P35 normalized comparison utilities where solver-free; otherwise create test-local normalized snapshot helper.

- [ ] Define a normalized semantic comparison including variable names/types/bounds, constraint names/bounds/coefficient cells, objective sense/coefficients/offset, and relevant active-state semantics.
- [ ] Add hand-built LP/MILP fixtures.
- [ ] Add randomized legal linear model generator with a fixed seed and bounded size.
- [ ] For each fixture assert `normalize(model) == normalize(read(write(model)).model)`.
- [ ] Add metamorphic tests: creation order changes that preserve normalized model must produce semantically equivalent output; repeated output from identical state must be byte-identical.
- [ ] Run at least 256 randomized cases in normal test budget or split heavier cases to qualification if runtime exceeds CI target.
- [ ] Commit `test(mps): prove semantic write round trips`.

---

### Task 36-07: Native HiGHS structural and solve oracle

**Files:**
- Create: `roml-highs/tests/mps_write_differential.rs`
- Modify: P35 differential helpers only if extension is reusable and source-compatible.

- [ ] Build one model directly through ROML->HiGHS and one through ROML->MPS->native `Highs_readModel`.
- [ ] Compare dimensions, named matrix coefficients, row bounds, column bounds, integrality, objective sense, coefficients, and offset using P35 tolerance constants.
- [ ] Solve both paths and compare termination class; compare objective when status makes it meaningful.
- [ ] Include optimal LP, optimal MILP, infeasible LP, unbounded LP, ranged rows, empty objective, free integer, and objective offset fixtures.
- [ ] Any accepted-input semantic mismatch is P1 until classified/fixed; do not add compatibility exceptions without evidence and owner review.
- [ ] Commit `test(highs): qualify MPS write-back against native reader`.

---

### Task 36-08: Netlib transcode corpus qualification

**Files:**
- Create: `roml-highs/examples/mps_roundtrip_qualification.rs` or extend the existing runner with a distinct schema/mode.
- Create/update: `docs/release/evidence/P36_MPS_WRITEBACK_QUALIFICATION.md`

Qualification per supported Netlib input:

```text
original MPS
 -> ROML read
 -> deterministic ROML write
 -> ROML re-read full structure compare
 -> HiGHS native read full structure compare
```

Bounded selected smoke set also runs native/ROML solve comparison.

- [ ] Pin the same Netlib commit as P35 unless an explicit corpus-update PR is approved.
- [ ] Attempt all 94 P35 structurally supported models.
- [ ] Record supported-pass, intentionally-unrepresentable, reader-rejection, writer-rejection, and unresolved-discrepancy separately.
- [ ] P36 gate requires zero unresolved discrepancies among semantic-model-representable files.
- [ ] Record timings and output sizes as evidence, not performance guarantees.
- [ ] Commit `test(mps): qualify Netlib write-back round trips`.

---

### Task 36-09: Documentation, package, review, closure

**Files:**
- Modify: `README.md`, `MODELING_API.md`, `CHANGELOG.md` only where necessary.
- Create: `36-SUMMARY.md`, `36-VERIFICATION.md`, `36-REVIEW.md`.
- Update: root/milestone state only after independent CLEAR.

- [ ] Add one compiled runnable write-back example.
- [ ] Document semantic-vs-source round-trip distinction and unsupported surface.
- [ ] Run `cargo fmt --all -- --check`.
- [ ] Run core/HiGHS all-target tests and clippy `-D warnings`.
- [ ] Run rustdoc `-D warnings`.
- [ ] Run package lists/fresh consumer as required by current policy.
- [ ] Run hosted Core/MSRV, HiGHS, Coverage, Quality, Policy on exact head.
- [ ] Obtain independent review with no P0/P1 findings.
- [ ] Record exact-head evidence and move P36 to `pending_merge`.
- [ ] Owner-authorized merge; only after merge route P30 active.

## P36 stop conditions

Stop and return to design review if:

- standard free MPS cannot faithfully represent a canonical primitive LP/MILP case we claim to support;
- write-back needs backend-specific state for ordinary semantic export;
- numeric formatting produces cross-reader semantic drift;
- corpus equivalence requires silently following HiGHS over frozen ROML semantics;
- compiled-formulation export starts expanding P36 materially; defer it instead.