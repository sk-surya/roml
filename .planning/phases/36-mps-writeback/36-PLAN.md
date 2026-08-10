# Phase 36 — MPS Write-Back Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** deterministically serialize representable ROML linear LP/MILP mathematical state to canonical free MPS and prove independent ROML/native-HiGHS round-trip equivalence over the frozen 94-model Netlib corpus.

**Architecture:** `roml::io::mps` gains a solver-free write pipeline with separate semantic projection, section helpers, byte formatting, and path transaction units. P36 is governed by `36-CONTRACT.md`, `36-NETLIB-MANIFEST.md`, `COMPLETION-REQUIREMENTS.md` MPS-W01–W14, and the milestone `SHARED-CONTRACTS.md`.

**Tech Stack:** Rust std I/O, existing ROML canonical snapshot/model APIs, P35 MPS reader, existing HiGHS generated bindings/oracle support, pinned optional Netlib submodule.

## Global constraints

- **Do not start this production plan until PR #45 is accepted and merged.**
- P36 writes free MPS only; fixed output is deferred.
- Default options/report/errors/representability/path semantics are frozen in `36-CONTRACT.md`.
- Parameterized models export one evaluated `(ModelInstanceId, ModelRevision)` mathematical snapshot; parameter graphs are not serialized.
- Writer bytes never embed raw arena slot/generation/debug IDs.
- P36 semantic export never silently compiles active semantic constructs.
- All 94 files in `36-NETLIB-MANIFEST.md` are mandatory: missing input or writer rejection is a qualification failure.
- Core writer remains solver-free.
- Compiled-formulation export is not a P36 public option; it is deferred to a separate future design.
- No later P30/P31/P34 implementation starts until P36 merges.

## File ownership map

Wave 1 parallel tasks have disjoint production ownership:

```text
src/io/mps/write/
  mod.rs          Wave 0 + serial integration only
  error.rs        Wave 0 only
  types.rs        Wave 0 only
  projection.rs   Wave 1A only
  format.rs       Wave 1B only
  path.rs         Wave 1C only
  bounds.rs       Wave 2A only
  objective.rs    Wave 2B only
```

Tests are likewise split so parallel workers do not edit the same files.

---

# Wave 0 — serial contract and representability freeze

## Task 36-00: Baseline, frozen manifest, public seam

**Files:**
- Modify: `src/io/mps/mod.rs` only for module declaration/re-exports.
- Create: `src/io/mps/write/mod.rs`
- Create: `src/io/mps/write/types.rs`
- Create: `src/io/mps/write/error.rs`
- Create: `tests/mps_write_public_contract.rs`
- Evidence: `.planning/phases/36-mps-writeback/36-BASELINE.md`

**Interfaces:** the exact public API in `36-CONTRACT.md`:

```rust
impl MpsWriter {
    pub fn new() -> Self;
    pub fn with_options(options: MpsWriteOptions) -> Self;
    pub fn write<W: std::io::Write>(&self, model: &Model, output: W) -> Result<MpsWriteReport, MpsWriteError>;
    pub fn write_path<P: AsRef<std::path::Path>>(&self, model: &Model, path: P) -> Result<MpsWriteReport, MpsWriteError>;
}
```

`write` is stream-only and may leave partial bytes; it never performs destination replacement. `write_path` alone consults `MpsDestinationPolicy`. Both capture one evaluated canonical snapshot before output.

- [ ] Record exact `main` base SHA, toolchain, current core/HiGHS test counts, public API inventory, and P35 corpus gitlink SHA before production edits.
- [ ] Initialize the Netlib submodule and compare its exact regular-file `.mps` inventory to `36-NETLIB-MANIFEST.md`; require 94/94 exact names and pinned commit `56257eea85b433ce6aa67d26156b36385318fd6f`.
- [ ] If the manifest disagrees, **stop** and amend the written spec through review; do not “fix” implementation around corpus drift.
- [ ] Write compile/failing tests for default options: `PreserveOrGenerate`, `AtomicReplace`, free MPS/LF/canonical numeric output; verify no decorative target/format/vector controls exist.
- [ ] Write failing tests for mandatory top-level error distinctions from `36-CONTRACT.md`.
- [ ] Implement only public contract/types/errors and module seams. `write`/`write_path` may return a typed internal-not-yet-implemented error during this TDD slice, but no public option may advertise unavailable behavior.
- [ ] Verify `cargo fmt`, focused tests, clippy `-D warnings`, and rustdoc `-D warnings`.
- [ ] Commit `feat(mps): freeze write-back contract`.

**Wave gate:** independent task review confirms defaults, report fields, error taxonomy, identity/environment metadata, manifest, and representability matrix are unambiguous before parallel production work.

---

# Wave 1 — parallel, disjoint write ownership

## Task 36-01A: Semantic projection

**Owns:** `src/io/mps/write/projection.rs`, `tests/mps_write_projection.rs` only.

**Consumes:** frozen public types; shared identity/parameter/naming contracts.

**Produces:** a private normalized `MpsWriteDocument`/projection interface consumed later by formatter/integration.

- [ ] Write failing tests covering primitive LP/MILP projection, inactive primitives, persistent fixing exact lowering, active-construct rejection, semi-domain rejection, parameter evaluation, no-active-objective, duplicate/missing/invalid names.
- [ ] Build deterministic export-local entity order and name allocation without rendering raw IDs.
- [ ] Evaluate all parameter-dependent supported numeric state once from the captured snapshot; populate report environment metadata.
- [ ] Preserve canonical matrix cells without syntactic re-expansion/dedup guesswork.
- [ ] Return typed representability/nonfinite/stale errors with entity context.
- [ ] Do **not** format text, create files, or use HiGHS.
- [ ] Verify focused tests + core all-targets.
- [ ] Commit `feat(mps): project evaluated semantic write document`.

## Task 36-01B: Canonical free-MPS formatter

**Owns:** `src/io/mps/write/format.rs`, `tests/mps_write_format.rs` only.

**Consumes:** normalized write-document/record interfaces frozen at Wave 0/coordination boundary.

- [ ] Write golden failing tests for section order, LF-only output, stable whitespace, canonical vector names, `-0.0 -> 0`, exponent magnitudes, and repeated byte identity.
- [ ] Implement finite locale-independent f64 formatting accepted by P35 parser + native HiGHS.
- [ ] Emit each normalized matrix cell exactly once.
- [ ] No `Debug`/`Display` of IDs may enter output bytes.
- [ ] Do not decide representability/domain/objective semantics here; formatter consumes normalized records.
- [ ] Verify 100 repeated writes of one normalized document are byte-identical.
- [ ] Commit `feat(mps): format canonical free MPS`.

## Task 36-01C: Cross-platform path transaction

**Owns:** `src/io/mps/write/path.rs`, `tests/mps_write_path.rs` only.

- [ ] Implement the internal `MpsPathOps`-equivalent injection seam from `36-CONTRACT.md`.
- [ ] Write red tests for failures at `CreateTemp`, `Write`, `Flush`, `Sync`, `Replace`, and `Cleanup`.
- [ ] Prove `CreateNew` never modifies an existing destination and handles destination races.
- [ ] Prove `AtomicReplace` stages in the same directory and never uses remove-then-rename.
- [ ] Provide reviewed platform atomic-replace implementation for Linux/macOS/Windows or return `AtomicReplaceUnavailable` before touching the destination on an unsupported platform.
- [ ] Preserve both primary and cleanup errors according to `SHARED-CONTRACTS.md` error-composition rules.
- [ ] Do not project/format models in this file.
- [ ] Commit `feat(mps): add transactional path commit`.

**Wave 1 integration gate:** serial integrator wires the three units through `write/mod.rs`, resolves interface-only conflicts, runs core all-target tests, and obtains one integration review before Wave 2.

---

# Wave 2 — parallel semantic sections and independent oracles

## Task 36-02A: Bounds and integer markers

**Owns:** `src/io/mps/write/bounds.rs`, `tests/mps_write_bounds.rs`.

- [ ] Table-drive continuous default/free/lower/upper/fixed/finite interval, binary, integer default/custom/free, and persistent-fixing cases.
- [ ] Emit no BOUNDS record only when P35 MPS defaults reproduce the exact mathematical domain.
- [ ] Use deterministic contiguous `INTORG`/`INTEND` marker regions.
- [ ] Explicitly override integer-marker `[0,1]` defaults whenever ROML's evaluated domain differs.
- [ ] Round-trip each table case through P35 reader using the **independent oracle**, not a writer helper.
- [ ] Commit `feat(mps): preserve variable domains in write-back`.

## Task 36-02B: Objective, RHS, RANGES

**Owns:** `src/io/mps/write/objective.rs`, `tests/mps_write_objective.rs`.

- [ ] Test min/max, positive/zero/negative objective offset, empty objective, equality/lower/upper/ranged rows, and every RANGES sign/sense case from P35 semantics.
- [ ] Implement exact inverse of P35 objective-offset convention.
- [ ] Preserve one semantic ranged row through RHS/RANGES rather than splitting semantic identity.
- [ ] Reject nonfinite evaluated values before formatter/path commit.
- [ ] Commit `feat(mps): preserve objective and row semantics`.

## Task 36-02C: Independent ROML semantic oracle

**Owns:** `tests/support/mps_write_oracle.rs`, `tests/mps_write_roundtrip.rs`.

- [ ] Build a **test-local independent** normalized mathematical extractor using public/snapshot APIs only.
- [ ] Normalize objective sense/coefficients/offset, variable domains/integrality, row bounds, and matrix coordinates.
- [ ] Do not import writer projection, naming, report, or HiGHS-oracle helpers.
- [ ] Compare before vs `MpsReader(read(write(model)))` under the frozen structural tolerance in `36-CONTRACT.md`.
- [ ] Add deterministic hand fixtures and at least 256 fixed-seed randomized legal primitive LP/MILP cases within CI budget.
- [ ] For parameterized fixtures, compare against the evaluated pre-write snapshot, not parameter graph identity.
- [ ] Commit `test(mps): add independent semantic round-trip oracle`.

## Task 36-02D: Native HiGHS oracle

**Owns:** `roml-highs/tests/mps_write_highs_oracle.rs` and, if needed, a new test-only helper file under `roml-highs/tests/support/` with no overlap with P35 files.

- [ ] Independently build direct ROML->HiGHS and ROML->MPS->native `Highs_readModel` paths.
- [ ] Compare full structure under the frozen `1e-10 + 1e-10*scale` structural rule.
- [ ] Compare normalized termination classes and paired optimal objective values under `1e-7 + 1e-8*scale`.
- [ ] Include optimal LP/MILP, infeasible, unbounded/ambiguous classification, ranged rows, objective offset, free integer, and no-objective fixtures.
- [ ] Enforce frozen mismatch dispositions from `36-CONTRACT.md`; no silent “HiGHS wins.”
- [ ] Commit `test(highs): qualify native MPS write oracle`.

**Wave 2 integration gate:** serial integrator runs all core/HiGHS MPS tests, checks writer report lowerings/name map/environment metadata, and obtains a semantic integration review.

---

# Wave 3 — serial 94-model Netlib transcode qualification

## Task 36-03: Frozen corpus runner and evidence

**Owns:**
- Create: `roml-highs/examples/mps_write_corpus_qualification.rs`
- Create/update: `docs/release/evidence/P36_MPS_WRITEBACK_QUALIFICATION.md`
- Test: `roml-highs/tests/mps_write_corpus_contract.rs`

For every exact path in `36-NETLIB-MANIFEST.md`:

```text
external MPS
 -> P35 ROML read
 -> P36 semantic write
 -> deterministic second write (byte equality)
 -> P35 ROML re-read
 -> independent ROML full mathematical compare
 -> native HiGHS full structural compare
```

- [ ] Fail before running if submodule missing, wrong SHA, any manifest file missing, unexpected `.mps` drift exists, or inventory count != 94.
- [ ] Writer rejection of any manifest model is a **P36 failure**, not an “intentionally unrepresentable” pass category.
- [ ] Require 94 explicit final PASS rows for deterministic write + ROML structure + native structure.
- [ ] Run bounded deterministic solve subset including at least the existing P35 Netlib smoke cases; record normalized statuses/objectives under frozen rules.
- [ ] Emit machine-readable deterministic result artifact plus human evidence summary.
- [ ] Any mismatch must receive one frozen disposition; unresolved or owner-unapproved exception blocks P36.
- [ ] Record per-file input/output size and timings only as telemetry, not performance guarantees.
- [ ] Commit `test(mps): qualify all frozen Netlib write transcodes`.

---

# Wave 4 — serial docs, package, independent review, closure

## Task 36-04: Consumer documentation and package qualification

**Files:** README/MODELING_API/CHANGELOG only where user-visible behavior changed; phase summary/review/verification/evidence.

- [ ] Add one compiled runnable primitive LP/MILP MPS-write example.
- [ ] Document default options, evaluated-parameter semantics, mathematical-vs-source round trip, persistent-fixing lowering, active-construct rejection, stream partial-write semantics, and path atomicity contract.
- [ ] Run `cargo fmt --all -- --check`.
- [ ] Run `cargo test -p roml --all-targets` and `cargo test -p roml-highs --all-targets`.
- [ ] Run clippy both crates with `-D warnings` and rustdoc both crates with `-D warnings`.
- [ ] Run current policy/coverage/package-list/fresh-consumer checks required by repository governance.
- [ ] Re-run frozen 94-model qualification on the exact candidate head.
- [ ] Obtain independent full PR review; zero unresolved P0/P1.
- [ ] Verify hosted Core/MSRV, HiGHS, Coverage, Quality, Policy on **exact head**.
- [ ] Write `36-VERIFICATION.md`, `36-REVIEW.md`, `36-SUMMARY.md`, and final evidence with exact SHA.
- [ ] Move state to `pending_merge`; owner-authorized merge only.
- [ ] Only after merge may routing activate P30.

## P36 positive closure predicate

P36 is complete **iff**:

```text
MPS-W01..MPS-W14 all have evidence
AND exact Netlib manifest = 94 present files at pinned SHA
AND 94/94 writer + deterministic + ROML-structure + HiGHS-structure PASS
AND required solve subset obeys frozen status/objective rules
AND all mandatory core/HiGHS/docs/package/policy/coverage/MSRV checks PASS
AND exact-head independent review has zero unresolved P0/P1
AND no unresolved mismatch/representability/path-transaction issue remains
AND owner-authorized merge is complete
```

## Stop conditions

Stop and return to written-spec review if ordinary primitive LP/MILP semantics require backend state, the exact manifest disagrees with the pinned corpus, cross-platform atomic path semantics cannot be achieved as frozen, a supposedly representable Netlib model is rejected, an independent oracle contradicts writer semantics, or a mismatch would require weakening frozen ROML semantics to follow HiGHS.
