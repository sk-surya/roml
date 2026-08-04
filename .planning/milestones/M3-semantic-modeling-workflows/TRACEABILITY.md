# M3 Requirement Traceability

Status values: `Planned`, `In progress`, `Closed`, `Blocked`, `Deferred`.

| Requirement | Primary phase | Secondary phase/evidence | Status |
|---|---|---|---|
| SM-01 Canonical semantic IR | P25 | P34 public API/NLP audit | Partially closed: SM-01.1–01.3, 01.5, 01.6 closed in P25; SM-01.4 partial (semantic constructs in P25, objective policies P31); P34 audit pending |
| SM-02 Identity/metadata/provenance | P25 | P26 compilation/origin IDs; P29 reports | Partially closed: SM-02.1, 02.3, 02.7 closed in P25; SM-02.4/02.5 closed in P26; SM-02.2 foundations only (validation mechanism P27); SM-02.6 foundations in P26, full in P29 |
| SM-03 Compiler/backend IR | P26 | P34 equivalence and stale-state evidence | Partially closed: SM-03.1–03.8 closed in P26; SM-03.9 partial (results and metadata carry exact `CompilationId` with façade mismatch rejection in P26; overlay artifacts P27, analysis artifacts P29); P34 evidence pending |
| SM-04 Typed capabilities | P26 | P28/P29/P31/P33 backend features | Partially closed: SM-04.1, 04.3, 04.4 closed in P26; SM-04.2 partial (typed set reports Native/Bridge separately; bridge features land P32); SM-04.5 deferred to P28; P28+/P33 features pending |
| SM-05 Persistent fixing | P27 | P34 regression/performance | Closed in P27 (`M3_P27_FIXING_LOCKS_OVERLAYS.md`); P34 regression/performance pending |
| SM-06 Assignments/solution reuse | P27 | P28 starts/hints | Closed in P27 (`M3_P27_FIXING_LOCKS_OVERLAYS.md`); P28 starts/hints pending |
| SM-07 SolvePlan/overlays | P27–P28 | P31 objective locks; P34 failure matrix | Partially closed: SM-07.3–07.6 closed in P27; SM-07.1–07.2/07.7 → P28; P31 objective locks (stage optimum `z`), P34 failure matrix pending |
| SM-08 Starts/hints | P28 | P34 support matrix | Planned |
| SM-09 IIS/conflicts | P29 | P34 docs/fresh consumers | Planned |
| SM-10 Soft constraints | P30 | P34 examples/equivalence | Planned |
| SM-11 Objective policies | P31 | P34 native/portable corpus | Planned |
| SM-12 Common constructs | P32 | P34 public qualification | Partially closed: SM-12.3/12.4/12.6/12.7 closed in P32; remaining clauses P33/P34 |
| SM-13 Big-M/bound analysis | P33 | P32 indicator/product bridges | Partially closed: SM-13.1-13.6 (compiler foundations) closed in P32; full closure P33 |
| SM-14 PWL | P33 | P34 solver/version matrix | Planned |
| SM-15 Qualification/NLP readiness | P34 | all phase evidence | Planned |

## Detailed evidence map

### P25 evidence

Expected file: `docs/release/evidence/M3_P25_SEMANTIC_IR.md`

Must include:

- M2 source-compatibility compile matrix;
- independent-lineage tests;
- clone-preserves-lineage/new-instance tests;
- metadata access and formatting tests;
- function-in-set snapshot/delta round-trip;
- construct-store lifecycle/property tests;
- public API diff;
- invariant-checker results.

Closes: SM-01.1–SM-01.3, SM-01.5, SM-01.6, foundations of SM-01.4 (semantic constructs in P25; objective policies P31); SM-02.1, SM-02.3, SM-02.7, foundations of SM-02.2 (validation mechanism P27) and SM-02.5; SM-15.1 foundations.

### P26 evidence

Expected file: `docs/release/evidence/M3_P26_COMPILER_BACKEND_IR.md`

Must include:

- canonical-to-backend compile determinism;
- unique exact `CompilationId` allocation and propagation;
- divergent-clone/equal-revision stale-state rejection;
- proof that recipe fingerprints are not accepted as exact authority;
- origin-map completeness;
- backend feature/limitation matrix;
- primitive identity-compiler equivalence including objective policy;
- compiled delta versus rebuild randomized tests;
- ReferenceBackend and HiGHS migration;
- backend contract/public API review.

Closes: SM-02.4–SM-02.6, SM-03, SM-04, compiler foundations of SM-13.

### P27 evidence

Expected file: `docs/release/evidence/M3_P27_FIXING_LOCKS_OVERLAYS.md`

Must include:

- declared/effective domain tests;
- continuous/integer/binary fixing matrix;
- fix/unfix bound-update traces;
- assignment lineage/stale-handle tests;
- overlay exact-compilation-ID validation;
- overlay apply/rollback failure matrix;
- subsequent-solve leak checks;
- HiGHS incremental bound evidence.

Closes: SM-05, SM-06, SM-07.3–SM-07.6, SM-02.2 (assignment validation: lineage + generations + value/domain).

### P28 evidence

Expected file: `docs/release/evidence/M3_P28_SOLVE_PLAN_STARTS_HINTS.md`

Must include:

- SolvePlan validation and convenience compatibility;
- start/hint capability matrix by HiGHS version;
- partial/full start fixtures;
- unsupported/conversion policy tests;
- effective-plan metadata snapshots with exact compilation identity;
- proof that hints/starts do not alter model feasibility;
- fresh public examples.

Closes: SM-07.1–SM-07.2, SM-07.7, SM-08.

**P28 status (executor):** evidence file recorded (baseline, per-task RED/GREEN,
HiGHS audit summary, full verification matrix, public API diff) and the HiGHS
audit record (`docs/knowledge/highs_mip_start_api.md`) landed on branch
`phase-roml-P28-solve-plan-warm-starts` (commits `40ef027`, `286c6c7`,
`cd46ebb`). Requirement closure is pending the independent review gates (the
orchestrator marks SM-07.1–07.2, SM-07.7, SM-08 complete after no-P0/P1
resolution).

### P29 evidence

Expected file: `docs/release/evidence/M3_P29_IIS_CONFLICTS.md`

Must include:

- pinned official-header/API audit;
- version-gated support table;
- row/bound/fixing/lock/construct conflict fixtures;
- exact conflict/origin `CompilationId` match and stale-ID rejection;
- text/Markdown structured report golden tests;
- exact scope/minimality/completion claims;
- unsupported behavior tests.

Closes: SM-09 and IIS-related SM-02.6/SM-04.3.

### P30 evidence

Expected file: `docs/release/evidence/M3_P30_SOFT_CONSTRAINTS.md`

Must include:

- algebra derivations and direct numerical checks;
- lower/upper/equality/ranged slack fixtures;
- penalty sign tests for min/max objectives;
- parameter weight update tests;
- signed-correction L1 tests;
- native feasibility-relaxation support matrix;
- solution violation accessors/examples.

Closes: SM-10.

### P31 evidence

Expected file: `docs/release/evidence/M3_P31_LEXICOGRAPHIC.md`

Must include:

- objective-policy validation;
- weighted mixed-sense normalization tests;
- portable stage/lock traces;
- minimize/maximize degradation formulas;
- stage-limit/continuation behavior;
- native semantics audit;
- native-versus-portable differential corpus;
- all-objective result metadata.

Closes: SM-11 and objective-overlay parts of SM-07.

### P32 evidence

Expected file: `docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md`

Must include per construct:

- semantic definition;
- accepted/rejected domains;
- native and bridge representation;
- origin map;
- explicit reference formulation;
- randomized solver equivalence;
- formulation report output;
- unsupported/unbounded failure behavior.

Closes: SM-12 and selected SM-13 requirements.

### P33 evidence

Expected file: `docs/release/evidence/M3_P33_PWL_BOUNDS.md`

Must include:

- interval-analysis test corpus;
- bound-source traces;
- convexity/concavity classification;
- zero-binary convex epigraph/concave hypograph proof;
- exact graph native/SOS2/binary equivalence;
- nonconvex exactness tests;
- scaling diagnostics;
- representation report examples.

Closes: SM-13, SM-14.

### P34 evidence

Expected files:

- `docs/release/evidence/M3_QUALIFICATION.md`
- `docs/release/evidence/M3_PUBLIC_API.md`
- `docs/release/evidence/M3_PERFORMANCE.md`
- `docs/release/evidence/M3_NLP_READINESS.md`

Must include:

- complete requirement closure table;
- full CI/test/doc/package matrix;
- public API/semver disposition;
- fresh packed-consumer results;
- native/portable formulation corpus summary;
- failure-injection matrix;
- exact-identity/stale-state matrix;
- benchmark comparison to untouched baseline;
- independent engineering and OR reviews;
- NLP-readiness pass/fail findings;
- residual risks and publication prohibition.

Closes: SM-15 and all residual requirements.

## PR traceability rule

Every M3 PR description must contain:

```text
Requirements: SM-xx.y, SM-aa.b
Phase: Pnn
Evidence: path/to/evidence.md
Baseline: <sha>
Head: <sha>
Focused checks: <commands/results>
Full checks: <commands/results>
Skipped: <none or explicit reason>
Residual risks: <none or explicit list>
```

A requirement is not closed by code presence alone. It is closed only after tests, evidence, and independent review pass.