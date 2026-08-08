---
phase: 35-mps-import
status: planned
baseline: 8a24bbe23b7ae1e2e87a47c7df248698374d84c
design: docs/superpowers/specs/2026-08-07-mps-io-design.md
depends_on: [P29]
parallelism: wave-based with disjoint ownership
---

# Phase 35 — MPS Import and Corpus Qualification

> **For agentic workers:** use `superpowers:subagent-driven-development` or `superpowers:executing-plans`. Every implementation task starts with a failing test, runs focused verification, and ends with a small atomic commit and review.

**Goal:** import fixed/free linear LP/MILP MPS files into a fresh ROML model with deterministic semantics, typed source-aware diagnostics, and qualified interoperability with HiGHS, Netlib, and Chinneck IIS workflows.

**Architecture:** `BufRead/path -> fixed/free records -> MPS staging document -> all-record structural validation -> selected-vector semantic resolution -> transactional fresh Model + MpsMetadata + MpsSourceMap`. The core reader is solver-free. HiGHS is an independent differential oracle, never the semantic authority.

**Global constraints:** Rust 1.85; no parser-generator dependency; no generic interchange IR; no mutation of a caller-owned model; no unsafe parser code; unsupported semantics fail explicitly; ordinary tests and packaging do not require external corpora; P36 writer production code is out of scope.

## DAG and waves

```text
35-00 contract/module seam
  ├──> 35-01 lexical records and state machine
  ├──> 35-02 staging vectors and structural validation
  └──> 35-03 secure corpus materialization
35-01 + 35-02 ──> 35-04 semantic resolution and provenance
35-04 ──> 35-05 transactional public reader API
35-03 + 35-05 ──> 35-06 synthetic/metamorphic/fuzz fixtures
35-05 ──> 35-07 HiGHS differential qualification
35-03 + 35-05 + P29 ──> 35-08 Netlib/Chinneck qualification
35-06 + 35-07 + 35-08 ──> 35-09 evidence, review, and completion gate
```

Wave 0 has one task. Wave 1 runs 35-01, 35-02, and 35-03 in parallel. Wave 2 runs 35-04. Wave 3 runs 35-05. Wave 4 runs 35-06, 35-07, and 35-08 in parallel. Wave 5 runs 35-09.

No parallel worker may edit a file owned by another active worker. Shared public exports, manifests, and evidence files are integration-owned and land only after their dependencies complete.

## Task 35-00 — Freeze module seam and test harness

**Depends on:** none. **Owner files:** `roml/src/io/mod.rs`, `roml/src/io/mps/mod.rs`, reader option/error type declarations, synthetic fixture harness, crate exports.

**Deliverable:** establish `roml::io::mps`, `MpsReader`, `MpsFormat`, vector-selection options, `MpsImport`, typed diagnostic/source-map names, resource limits, and test helpers without implementing parsing semantics.

**Tests first:** compile-level public API tests; default-option determinism; unsupported-section error shape; malformed-input no-panic harness.

**Gate:** all names consumed by later tasks are typed and documented; `cargo check -p roml --all-targets` passes.

## Task 35-01 — Fixed/free lexer and streaming record state machine

**Depends on:** 35-00. **Owner files:** MPS lexer/record/state modules and lexer-focused tests only.

**Deliverable:** line-streaming fixed/free detection, section transitions, source spans, integer marker nesting, numeric parsing, supported record recognition, and explicit unsupported-section errors.

**Tests first:** fixed/free records, long free names, comments/blank lines, malformed columns, invalid numbers, missing ENDATA, section-order errors, marker nesting, unsupported quadratic/conic/vendor sections, and deterministic errors.

**Gate:** parser consumes `BufRead` incrementally, never recurses by input size, checks record/line limits, and has no model dependency.

## Task 35-02 — MPS staging document and all-vector structural validation

**Depends on:** 35-00. **Owner files:** staging document, vector storage/selection, structural validation, and staging tests only.

**Deliverable:** compact column-oriented staging representation preserving all named RHS/RANGES/BOUNDS vectors, duplicate COLUMNS entries, row/variable references, objective metadata, and deterministic selection (`First`, `Named`, `None`).

**Tests first:** duplicate COLUMNS preservation, multiple named vectors, unknown rows/variables in unselected vectors, non-finite numeric rejection, selected/unselected vector determinism, and checked resource limits.

**Gate:** every staged vector receives syntax/numeric/reference validation; semantic checks are deferred until selection.

## Task 35-03 — Safe optional corpus materialization

**Depends on:** 35-00. **Owner files:** corpus manifest/materializer, ignored generated-state helpers, archive-security tests, optional corpus metadata. Do not add production parser dependencies.

**Deliverable:** exact pin validation for `sk-surya/infeasiblelps@97a936498e5240d44adaf7dcfe84877fa34ce301` and `sk-surya/lp-data-netlib@56257eea85b433ce6aa67d26156b36385318fd6f`, plus pre-write-safe Chinneck archive extraction into a fresh temporary root with atomic promotion.

**Tests first:** A01–A11 archive adversarial cases covering absolute, drive, UNC, traversal, symlink, hardlink, device/FIFO/socket/special entries, escaping destinations, partial extraction, and cache promotion.

**Gate:** ordinary builds work without submodules; no blind extraction followed by post-hoc scanning; partial output is never cached.

## Task 35-04 — Semantic resolution and MPS provenance

**Depends on:** 35-01, 35-02. **Owner files:** semantic resolver, model construction adapter, source-map/provenance implementation, semantic tests.

**Deliverable:** deterministic objective/row/range/bounds/integrality semantics and transactional resolution into a fresh ROML `Model`.

**Required semantics:** objective selection `OBJNAME`, then first `N`, else zero; default minimize; negative objective RHS offset; one ranged ROML constraint per ranged MPS row; duplicate selected RHS/RANGES reject; selected RANGE-on-N rejects while unselected structurally valid RANGE-on-N remains inert; ordered BOUNDS transitions; `INTORG + FR` yields unbounded integer; implicit continuous and marker bounds receive synthetic provenance.

**Tests first:** R07–R12, V09–V11, objective offsets, duplicate cells, duplicate selected rim entries, all bound transitions, marker defaults, explicit-side provenance replacement, failed-resolution atomicity, and model invariant checks.

**Gate:** no partially constructed model escapes; every finite imported bound maps to exactly one explicit or synthetic MPS origin.

## Task 35-05 — Public reader API and integration surface

**Depends on:** 35-04. **Owner files:** public reader/path/stream API, metadata and diagnostics exports, documentation and API integration tests.

**Deliverable:** `MpsReader::read(BufRead)` and `MpsReader::read_path`, options, deterministic metadata, typed errors, and `roml::io::mps` exports without `Model::from_mps` coupling.

**Tests first:** stream/path equivalence, transactional failures, options/selection, long names, error source context, deterministic repeated reads, and package/export compile tests.

**Gate:** core remains solver-free and normal tests do not require corpus data or native libraries.

## Task 35-06 — Synthetic, metamorphic, fuzz, and security fixtures

**Depends on:** 35-05 and 35-03. **Owner files:** synthetic MPS fixtures, property/metamorphic tests, fuzz target scaffolding, fixture documentation.

**Deliverable:** complete normal-CI coverage for supported records and edge semantics, including whitespace/layout transformations, duplicate-cell algebra, vector-selection metamorphisms, malformed input, and archive security.

**Gate:** every requirement in `35-TEST-MATRIX.md` has a named test or explicit external qualification classification.

## Task 35-07 — HiGHS differential oracle

**Depends on:** 35-05. **Owner files:** `roml-highs` qualification adapter/tests and differential manifest; no core parser implementation changes.

**Deliverable:** normalized structural and solve comparison between native HiGHS `readModel` and ROML import followed by ROML-to-HiGHS projection.

**Disposition rules:** accepted-input mismatches block until `roml_bug_fixed`, `dialect_narrowed`, or evidence-backed owner-approved `compatibility_exception`; strict ROML rejection accepted by HiGHS is recorded as `intentional_roml_rejection`.

**Gate:** no automatic “HiGHS wins” behavior and no differential result is presented as proof of ROML semantics.

## Task 35-08 — Netlib and Chinneck qualification

**Depends on:** 35-03, 35-05, P29. **Owner files:** optional corpus qualification runner, manifests, ignored generated reports, corpus tests/evidence.

**Deliverable:** Netlib feasible-LP qualification and selected Chinneck infeasible-LP -> ROML IIS qualification with source mapping and irreducibility guarantees.

**Gate:** exact pins are checked; corpora are optional; Chinneck acceptance validates ROML’s own irreducibility and provenance, not equality to a Gurobi IIS.

## Task 35-09 — Evidence, independent review, and completion gate

**Depends on:** 35-06, 35-07, 35-08. **Owner files:** `docs/release/evidence/P35_MPS_QUALIFICATION.md`, phase summary/state, changelog/public docs as applicable.

**Deliverable:** command/version/output evidence, skipped-check rationale, residual risks, package inspection, requirement traceability, independent code review, and final phase verification.

**Required checks:** formatting, core check/clippy/test/doc/package, focused MPS tests, available HiGHS checks, corpus qualification when initialized, and security/policy checks. No native commercial check may be reported as passing when unavailable.

## Commit and integration policy

- Keep one-purpose commits per task or tightly coupled task slice.
- Use isolated worktrees for every active worker.
- Review each task before merging its commit into the phase branch.
- Keep production source, corpus gitlinks, workflows, and planning changes in separately reviewable commits/PRs.
- Do not implement P36 writer behavior in P35.
