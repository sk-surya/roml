# P34 M3 Final Qualification Contract

**Status:** binding qualification protocol. P34 is a closure phase, not a feature phase.

## 1. Leaf requirement ledger

P34 must create `34-REQUIREMENT-LEDGER.md` with **one row for every leaf ID** in the authoritative M3 `REQUIREMENTS.md` (`SM-01.1`, `SM-01.2`, ... `SM-15.8`) plus completion-program IDs MPS-W01–W14 and M3-C01–C05.

Required columns:

| Column | Meaning |
|---|---|
| ID | exact leaf requirement ID |
| owner phase | phase that implemented it |
| implementation evidence | exact file/test/API reference |
| qualification command/test | exact P34 or prior exact-head command proving it |
| backend/OS scope | where applicable |
| review disposition | review ID + `pass`, `resolved`, or approved residual |
| residual risk | `none` or concrete bounded risk |
| final state | `PASS` or `BLOCKED` |

No aggregate row such as `SM-10` may substitute for leaf rows. P34 closure requires every mandatory row `PASS`.

## 2. Executable fault-injection matrix

P34 must implement/aggregate deterministic injected failures for the following matrix. “Primary error preserved” always includes cleanup error when cleanup also fails.

| Boundary | Canonical revision | backend health after handled failure | required exact-state behavior | required cleanup/result |
|---|---|---|---|---|
| canonical mutation validation | unchanged | unchanged | prior `(instance,revision)` remains authority | atomic typed error |
| compiler preflight | unchanged | prior health | prior CompilationId remains usable only if no mutation occurred | no backend mutation |
| delta apply before mutation | unchanged | `Ready` or prior | exact prior CompilationId | recoverable error |
| delta apply after partial mutation | unchanged | `RequiresRebuild` | old CompilationId cannot authorize new result | rebuild required |
| rebuild failure | unchanged | `RequiresRebuild` | no fabricated CompilationId | operational error |
| SolvePlan overlay preflight | unchanged | `Ready` | base CompilationId unchanged | no temporary artifacts |
| overlay partial apply | unchanged | `Ready` iff rollback verified; else `RequiresRebuild` | overlay ID cannot survive failed apply | receipt + cleanup evidence |
| solve failure under overlay | unchanged | same rule | no solution accepted | primary preserved |
| solution extraction failure | unchanged | same rule | no partial `Solution` returned | rollback attempted |
| overlay rollback failure | unchanged | `RequiresRebuild` | base restoration not claimed | composite operational error |
| overlay rollback verification failure | unchanged | `RequiresRebuild` | base CompilationId not trusted | rebuild required |
| P29 oracle `Unknown` | unchanged | usable if oracle session isolated/clean | report guarantee remains Unknown | no IIS inflation |
| P29 final verification failure | unchanged | analysis session discarded/recovered | no irreducible claim | Unknown/incomplete report |
| P30 relaxation apply/solve/extract failure | unchanged | shared overlay rules | no repair report accepted before cleanup | primary + cleanup preserved |
| P31 stage objective apply failure | unchanged | shared overlay rules | no later stage executes | exact failed-stage record/error |
| P31 stage lock apply failure | unchanged | shared overlay rules | lock not treated as active unless receipt proves it | stop later stages |
| P31 final rollback failure | unchanged | `RequiresRebuild` | no `MultiObjectiveResult` returned as reusable success | composite error |
| P36 serialize failure before path commit | unchanged | N/A | model identity unchanged | destination unchanged |
| P36 temp write/flush/sync failure | unchanged | N/A | N/A | destination unchanged; temp cleanup attempted |
| P36 atomic replace failure | unchanged | N/A | N/A | old destination remains intact or operation reports platform atomic failure without prior removal |

The matrix is executable: every row maps to a named test/fault point and actual assertions, not prose-only evidence.

## 3. Native/portable qualification corpus

### 3.1 Frozen synthetic fixture names

Create/retain these deterministic fixtures under P34 qualification tests:

```text
Q01_primitive_parameter_delta
Q02_indicator_tight_bounds
Q03_exact_minmax
Q04_absolute_positive_clamp
Q05_binary_product
Q06_pwl_convex_epigraph
Q07_pwl_nonconvex_exact_graph
Q08_overlay_fix_lock
Q09_mip_start_partial
Q10_iis_row_and_bound
Q11_relaxation_weighted_l1
Q12_lexicographic_mixed_sense
Q13_lexicographic_zero_optimum
Q14_mps_parameterized_snapshot
```

Each fixture has a checked expected mathematical result or independent formulation oracle.

### 3.2 Backend/version/OS matrix

Mandatory:

| Backend | Version/config | Linux | macOS | Windows | role |
|---|---|---:|---:|---:|---|
| ReferenceBackend | repository implementation | required | core CI as applicable | core CI as applicable | solver-free semantic/recovery oracle |
| HiGHS bundled | highs-sys/HiGHS 1.15.0 pinned by workspace | required | required | required | primary native solve/incremental oracle |
| HiGHS system | 1.9.0 qualification floor | required | optional/nonblocking unless CI already supports | optional/nonblocking | compatibility floor; unsupported native IIS remains explicit |

If the workspace pin changes before P34, P34 must amend this table through review rather than silently substitute versions.

### 3.3 Observables

Per applicable fixture/path compare:
- normalized termination class;
- objective value(s);
- final primal values only where unique/reference fixture requires them;
- row/column bounds and matrix values for formulation differential;
- integrality;
- objective coefficients/sense/offset;
- selected native/bridge/provider labels;
- generated entity/origin completeness;
- exact CompilationId/stage identity consistency;
- stage sequence/locks for P31;
- violation members/weights/outcome for P30.

### 3.4 Frozen numeric rules

Structural finite scalar comparison:

```text
abs_diff <= 1e-10 + 1e-10 * max(|a|, |b|)
```

Optimal objective comparison:

```text
abs_diff <= 1e-7 + 1e-8 * max(|za|, |zb|)
```

Integrality/domain/IDs/status/provider labels compare exactly unless a fixture explicitly states a semantic equivalence class.

### 3.5 Discrepancy dispositions

Every mismatch is classified as exactly one of:

```text
roml_core_bug
roml_compiler_bug
roml_backend_bug
portable_formulation_bug
native_provider_semantic_mismatch
backend_version_limitation
test_oracle_bug
approved_numerical_exception
```

Only `backend_version_limitation` and `approved_numerical_exception` may remain as residuals, and each requires evidence + owner/reviewer approval. No unresolved discrepancy counts as pass.

## 4. Reproducible primitive performance fixture

### 4.1 Fixture

Name: `P34_PRIMITIVE_PARAMETER_UPDATE_V1`.

Commit a standalone deterministic harness under `tools/p34-perf/` during P34 with these generated-model constants:

```text
variables = 512 continuous variables, bounds [0, 100]
constraints = 512 linear <= rows
matrix = 8 deterministic nonzeros per row (4096 nnz), generated by fixed seed 0x524F4D4C
parameter = one scalar parameter used in 64 RHS/coefficient cells
objective = deterministic linear minimization with finite coefficients
warmups = 20 update+solve attempts
measured = 200 update+solve attempts
HiGHS = bundled, output off, threads=1, fixed random seed when supported
profile = --release
```

The generator and expected baseline feasibility/optimal status are committed so both historical/current checkouts use the same mathematical fixture.

### 4.2 Baseline

Historical implementation baseline: `main@4d111cceafce17aea44a6e396a838d1cc9ef255d` (P25 implementation base / pre-M3 semantic implementation).

Candidate: exact P34 head.

Run both on the same machine/toolchain family and bundled HiGHS version where possible. If the historical checkout needs only a mechanical harness compatibility adapter, that adapter lives in the external benchmark harness and may not change the mathematical workload or measured operation.

### 4.3 Metric/gate

Primary metric: median wall time for `set_parameter + solve` after warmup. Also record p25/p75 and synchronization/rebuild classification if exposed.

Binding gate from original M3:

```text
candidate median overhead <= max(5% of baseline median, 50 microseconds)
```

Interpretation: candidate median may exceed baseline by at most the larger of those two allowances.

A breach requires profiler evidence and explicit owner-approved exception before M3 closure.

## 5. Packed consumer protocol

P34 creates `scripts/p34-packed-consumers.sh` implementing the exact protocol below and records its SHA/output.

### 5.1 Package commands

Run from a clean exact-head worktree:

```bash
set -euo pipefail
cargo package --list -p roml > /tmp/p34-roml-package-list.txt
cargo package --list -p roml-highs > /tmp/p34-roml-highs-package-list.txt
cargo package -p roml --locked
```

For `roml-highs`, retain the established pre-publication limitation honestly. Attempt:

```bash
cargo package -p roml-highs --locked
```

If it succeeds, use the produced `.crate`. If it fails **only** because `roml` is not yet published/resolvable from crates.io, record that exact diagnostic and build a workspace-independent packed-source tree using the same `cargo package --list -p roml-highs` manifest plus the extracted `roml` `.crate`; no source may be referenced from the live workspace. Any other packaging failure blocks P34.

### 5.2 Extraction/assertions

```bash
rm -rf /tmp/p34-packed
mkdir -p /tmp/p34-packed
# exact archive version is read from cargo metadata during the script
# extract generated .crate archives under /tmp/p34-packed
```

Assertions for both candidate packages/packed trees:
- no `.planning/`, `.worktrees/`, `testdata/corpora/`, `.git/`, target artifacts, solver logs, or machine-local paths;
- required README/license/manifest/src files present according to package include policy;
- no `path =` dependency in packed Cargo.toml that points at the source workspace;
- `roml` consumer compiles without native solver/compiler requirements beyond Rust toolchain;
- HiGHS consumer uses only packed/extracted dependencies + declared registry/native build dependencies.

### 5.3 Exact fresh consumers

Create these directories from scratch on every run:

```text
/tmp/p34-consumer-core
/tmp/p34-consumer-highs
/tmp/p34-consumer-mps
/tmp/p34-consumer-iis-relax
/tmp/p34-consumer-lexicographic
```

For each:

```bash
rm -rf /tmp/p34-consumer-<name>
cargo new --quiet --bin /tmp/p34-consumer-<name>
# script writes Cargo.toml with only packed/extracted dependency paths
# script writes frozen example source copied from committed P34 consumer fixtures
cargo run --manifest-path /tmp/p34-consumer-<name>/Cargo.toml --locked
```

Required assertions:
- core: named LP builds/model-validates without HiGHS;
- highs: solve + parameter update returns frozen objectives;
- mps: write/read round trip and native HiGHS solve passes;
- iis-relax: infeasible -> IIS -> relaxation returns expected supported-origin report;
- lexicographic: frozen two-stage result/lock metadata matches expected values.

Temporary consumer directories are never committed.

## 6. Concrete quadratic/NLP readiness shapes

P34 does not implement them. Reviewers must trace each shape through the future seams.

### Shape N1 — convex QP objective

```text
min 0.5 x^T Q x + c^T x + k
s.t. A x <= b
Q symmetric PSD
```

### Shape N2 — convex quadratic constraint

```text
min c^T x
s.t. x^T Q x + a^T x <= b
Q symmetric PSD
```

### Shape N3 — nonconvex bilinear scalar function

```text
min x * y
s.t. linear bounds on x,y
```

Must be representable semantically without being falsely labeled convex/exactly MILP-bridged.

### Shape N4 — smooth nonlinear scalar constraint/objective

```text
min exp(x) + (y - p)^2
s.t. sin(x) + y <= 1
```

Parameter dependency, value evaluation, derivative/backend requirements, origin/reporting, and solver capability must have an additive design path.

### Required per-component verdict

For each N1–N4, reviewers issue one verdict for each component:

```text
ScalarFunction
ScalarSet / function-in-set constraint
parameter dependency graph
canonical snapshots/deltas
backend IR
compiler recipes/report
capability registry
origin map
ModelLineageId / ModelInstanceId / revision
CompilationId
SolvePlan / overlays
assignments/starts/hints
ObjectivePolicy
IIS/relaxation reporting
file-I/O boundary
```

Allowed verdicts:

```text
READY_ADDITIVE
READY_WITH_BOUNDED_M4_AMENDMENT
BLOCKED_REPLACEMENT_REQUIRED
```

Every `READY_WITH_BOUNDED_M4_AMENDMENT` must state exact amendment owner/interface/blast radius. Any `BLOCKED_REPLACEMENT_REQUIRED` blocks M3 closure unless the owner explicitly changes the M3 architectural goal through a reviewed decision.

## 7. Positive M3 closure predicate

M3 is complete only when **all** are true:

```text
P25..P33 accepted where applicable
AND P35/P36 accepted
AND P30 accepted
AND P31 accepted
AND every leaf SM-xx.y + MPS-Wxx + M3-Cxx ledger row = PASS
AND executable fault matrix = PASS for every mandatory row
AND native/portable/backend-version matrix = PASS with only approved bounded residuals
AND P34_PRIMITIVE_PARAMETER_UPDATE_V1 performance gate = PASS or owner-approved profiled exception
AND all packed-consumer/package assertions = PASS (with only the documented unpublished-roml packaging limitation)
AND all public examples/docs/rustdoc/API/package checks = PASS
AND NLP readiness has no BLOCKED_REPLACEMENT_REQUIRED verdict
AND all independent review perspectives have zero unresolved P0/P1
AND exact-head hosted mandatory CI = PASS
AND owner-authorized P34 merge is complete
```

“No P0/P1” alone is not a positive closure predicate. Every listed gate must have affirmative evidence.
