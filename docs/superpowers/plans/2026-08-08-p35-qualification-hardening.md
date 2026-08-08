# P35 Qualification Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring P35 from Netlib smoke qualification to a green, near-linear, full Q03/Q04, exact-IIS-provenance, and selected-Chinneck qualification gate.

**Architecture:** Keep the solver-free MPS parser and secure materializer boundary. Fix corpus discovery independently of extraction, lower staged sparse cells into row expressions in one pass, and compare normalized native HiGHS and ROML observations through explicit structural/solve dispositions. The archive adapter will enumerate trusted metadata and stream payloads into the existing pre-write-safe materializer.

**Tech Stack:** Rust 1.85, ROML core, `highs-sys` official generated bindings, HiGHS, integration tests, pinned optional corpus gitlinks.

## Global Constraints

- Core MPS parsing remains solver-free and uses no unsafe parser code.
- Ordinary builds and non-recursive clones must not require initialized corpora.
- No blind `7z x` extraction or post-hoc filesystem safety scan is permitted.
- Native HiGHS is an independent interoperability oracle; ROML semantics remain normative.
- Accepted-input discrepancies require `roml_bug_fixed`, `dialect_narrowed`, or evidence-backed `compatibility_exception`.
- Chinneck acceptance verifies ROML irreducibility and exact source provenance, not equality to a Gurobi IIS.
- No P36 writer implementation, publish, tag, or merge to `main` occurs in this phase branch.

## DAG

```text
H1 CI/submodule portability
 ├──> H2 O(nnz) semantic lowering
 ├──> H3 full Q03 structural comparator
 └──> H4 native-vs-ROML Q04 solve comparator
H2 + H3 + H4 ──> H5 exact IIS provenance
H1 + H5 ──> H6 reviewed streaming 7z adapter
H3 + H4 + H5 + H6 ──> H7 selected Chinneck qualification
H7 ──> H8 evidence/review/final verification
```

### Task H1: CI and optional-submodule portability

**Files:** `tests/support/corpus.rs`, `tests/corpus_archive_security.rs`, optional workflow-facing test support.

- [ ] Add a failing test that passes a repository root containing gitlink-shaped directories without initialized Git metadata and asserts `validate_optional_corpora` returns `Ok(None)`.
- [ ] Run `cargo test --test corpus_archive_security` and record the failure caused by parent-repository fallback.
- [ ] Make corpus metadata discovery use an explicit repository-root submodule status/initialization check with no parent fallback; use portable imports for shared support and retain Linux-only code only around descriptor-relative materialization.
- [ ] Replace `Iterator::collect::<String>()` in marker rendering with an MSRV-1.85-compatible deterministic construction.
- [ ] Run `cargo fmt --all -- --check`, MSRV-equivalent clippy/check/test commands, and the archive security test.
- [ ] Commit as `fix(p35): make optional corpus checks portable`.

### Task H2: O(nnz) sparse semantic lowering

**Files:** `src/io/mps/semantic.rs`, semantic/integration tests.

- [ ] Add a failing regression test using multiple rows and columns that instruments or otherwise asserts the resolver consumes each staged matrix entry once while preserving duplicate-cell sums and objective coefficients.
- [ ] Run the focused semantic test before implementation and confirm it fails for the current per-row full scan.
- [ ] Build a row-expression map in one pass over staged columns and entries, then reuse it for objective and constraint construction; preserve deterministic variable ordering and existing model semantics.
- [ ] Run focused MPS tests and compare model snapshots for duplicate cells, objective offsets, ranges, and empty rows.
- [ ] Commit as `perf(p35): lower staged MPS cells in one sparse pass`.

### Task H3: Full Q03 native/ROML structural comparison

**Files:** `roml-highs/src/mps_oracle.rs`, oracle tests, qualification example/report types.

- [ ] Add failing fixture assertions for matrix values, row bounds, column bounds, integrality, objective coefficients, objective sense, and offset in the normalized comparison.
- [ ] Run the focused oracle tests and confirm the current dimensions/nnz-only summary cannot satisfy them.
- [ ] Extend both native and ROML summaries with canonical row/column identities and tolerance-safe semantic vectors; compare every accessible Q03 field and record explicit unsupported-field dispositions.
- [ ] Run synthetic, fixture, and bounded Netlib structural qualification; reserve “equivalent” for the full comparator.
- [ ] Commit as `feat(p35): compare full MPS structural semantics`.

### Task H4: Native-vs-ROML Q04 solve equivalence

**Files:** `roml-highs/src/mps_oracle.rs`, `roml-highs/examples/mps_corpus_qualification.rs`, oracle/qualification tests.

- [ ] Add a failing test fixture that runs both native `readModel` and ROML projection and asserts termination class plus objective comparison under declared absolute/relative tolerances.
- [ ] Run it against the current runner and confirm only the ROML path is solved.
- [ ] Add bounded native and ROML solve observations, normalized termination classes, objective extraction, and explicit tolerance comparison; keep smoke results distinct from equivalence results.
- [ ] Run the focused solve comparator and three-file smoke qualification.
- [ ] Commit as `feat(p35): add native versus ROML solve qualification`.

### Task H5: Exact IIS variable-bound provenance

**Files:** `roml-highs/tests/mps_iis_qualification.rs`, provenance helpers if required.

- [ ] Change the implicit-bound fixture to extract the exact `(Variable, BoundSide)` reported by P29.
- [ ] Add a failing assertion that resolves that exact restriction to exactly one `MpsBoundOrigin` and rejects a different variable or side.
- [ ] Run the focused IIS qualification and then the full P29 fixture set.
- [ ] Commit as `test(p35): prove exact IIS bound provenance`.

### Task H6: Reviewed streaming 7z adapter

**Files:** qualification-only corpus support, adapter tests, dev dependencies only where necessary.

- [ ] Add adapter-level failing tests for metadata enumeration, regular-file payload streaming, rejected special entries, and adapter errors before any destination write.
- [ ] Select a pinned archive reader/tool whose listing API exposes logical path and entry kind before payload access; do not invoke blind filesystem extraction.
- [ ] Stream each validated regular entry into `materialize_chinneck_archive` and preserve cache identity/inventory/atomic promotion.
- [ ] Run adapter tests plus all A01–A11 security tests, including on the supported non-Linux compile path.
- [ ] Commit as `feat(p35): add secure streaming Chinneck archive adapter`.

### Task H7: Selected Chinneck qualification

**Files:** `roml-highs/examples/mps_corpus_qualification.rs`, corpus manifests/reports, evidence.

- [ ] Add a deterministic selected-archive/model allowlist and expected inventory checks.
- [ ] Materialize the selected archives atomically, then run native HiGHS read, ROML read, full Q03/Q04 comparison, infeasibility, P29 IIS, and exact source-provenance checks.
- [ ] Record supported, intentional rejection, and failed dispositions without treating any solver IIS as normative.
- [ ] Run selected Chinneck qualification and retain generated output only under ignored target state.
- [ ] Commit as `test(p35): qualify selected Chinneck IIS corpus`.

### Task H8: Evidence, review, and final gate

**Files:** `docs/release/evidence/P35_MPS_QUALIFICATION.md`, `.planning/STATE.md`, `.planning/ROADMAP.md`, `CHANGELOG.md`, review artifact.

- [ ] Run the complete core/HiGHS formatting, check, clippy, test, rustdoc, package, security, oracle, Netlib, and Chinneck matrix on the final head.
- [ ] Update evidence only with exact commands, toolchain versions, outputs, skipped checks, and residual risks supported by those runs.
- [ ] Request independent review, resolve all P0/P1 findings, verify the exact PR head, and keep PR #44 draft until the owner approves merge.
- [ ] Commit as `docs(p35): record qualification completion evidence`.
