# Phase 35 Corpus Qualification

## 1. Purpose

External corpora serve two independent goals:

1. **parser/model interoperability** — ROML must reconstruct the declared P35 linear LP/MILP semantics and compare that interpretation with an independent mature reader, HiGHS;
2. **IIS workflow qualification** — imported infeasible models must remain infeasible and Phase 29 must produce correctly qualified, source-aware conflicts.

The corpora are not unit-test fixtures and are not shipped in the ROML crate.

The frozen ROML semantics reference is normative. HiGHS is differential evidence, not the source of truth for historically ambiguous MPS behavior.

## 2. Repositories and pins

Design-time pins:

```text
Chinneck infeasible LPs
  repository: https://github.com/sk-surya/infeasiblelps
  upstream:   https://github.com/JChinneck/InfeasibleLPs
  commit:     97a936498e5240d44adaf7dcfe84877fa34ce301

Converted Netlib LPs
  repository: https://github.com/sk-surya/lp-data-netlib
  upstream:   https://github.com/ozy4dm/lp-data-netlib
  commit:     56257eea85b433ce6aa67d26156b36385318fd6f
```

At design time neither upstream exposes a root `LICENSE` file. ROML therefore stores only submodule references/metadata, not copied dataset files. Any future vendoring requires a separate provenance/license review.

## 3. Planned submodule layout

```text
testdata/
└── corpora/
    ├── infeasible-lps/  -> sk-surya/infeasiblelps @ exact SHA
    └── netlib/          -> sk-surya/lp-data-netlib @ exact SHA
```

Properties:

- ordinary clone may leave submodules uninitialized;
- ordinary `cargo test` never requires them;
- `testdata/` remains outside the crate package include allowlist;
- corpus commands verify exact gitlink SHA before running;
- corpus updates are reviewed dependency-like changes with before/after qualification evidence.

## 4. Corpus materialization

The two repositories have different physical layouts and the harness hides that behind one manifest/inventory layer.

### 4.1 Netlib

The Netlib fork stores expanded `.mps` files under `mps_files/`. The qualification harness reads those files directly from the initialized submodule.

### 4.2 Chinneck

The Chinneck fork stores collections in archives such as:

```text
INFfromClassificationData.7z
INFfromNetlibLPs.7z
Arts.7z
expand.zip
```

P35 does not commit extracted copies and does not rewrite the fork for ROML convenience. Selected archive contents are materialized into generated state keyed by source commit + archive identity, conceptually:

```text
target/roml-corpora/
└── chinneck/
    ├── INFfromClassificationData/
    ├── INFfromNetlibLPs/
    ├── Arts/
    └── expand/
```

### 4.3 Archive extraction security contract

Archive entries are untrusted input. The materializer must not rely on an extractor's undocumented path sanitization and must not rely only on a post-extraction scan.

Before writing each entry, the corpus materializer shall:

1. obtain the archive entry's logical path and file type from the archive reader/listing API;
2. reject POSIX absolute paths;
3. reject Windows drive-qualified and UNC paths;
4. normalize lexical path components and reject any `..` that would escape the extraction root;
5. reject symlink, hardlink, device, FIFO, socket, and other special-file entries;
6. construct the destination only by joining a validated relative path beneath a fresh extraction root;
7. refuse to follow pre-existing filesystem symlinks while creating parent directories/files;
8. verify each final destination remains beneath the fresh root before opening it for write.

Operational rules:

- extraction occurs in a newly created temporary directory not shared with prior runs;
- only regular files/directories are materialized;
- source archives remain immutable inside the pinned submodule;
- extraction output is disposable and untracked;
- partial output is never reused as complete;
- a completion marker/cache directory is atomically promoted only after all entries and expected corpus inventory checks succeed;
- cache identity includes corpus SHA + archive identity/hash;
- absent extraction tooling produces an actionable corpus-setup error, never a parser-test failure;
- Tier 0 tests have no archive-tool requirement.

The implementation plan may choose a dev-only archive library/tool, but it must expose enough entry metadata to enforce the rules above **before each write**. A blind `7z x archive -o<dir>` followed by a safety check is not an accepted design.

## 5. Why submodules instead of copied files

Submodules provide:

- exact immutable corpus identity in the ROML graph;
- local/offline reuse after initialization;
- no duplication of external data into ROML history;
- clear provenance;
- owner-controlled fork endpoints;
- explicit reviewed updates rather than floating downloads.

Known disadvantages are contained:

- recursive clone complexity -> only corpus workflows initialize them;
- Chinneck archive layout -> safe materialization into generated state;
- GitHub archive behavior -> corpus qualification is not a source-package requirement;
- fork movement -> exact commits remain the reviewed identity.

If submodules later become operationally harmful, the approved fallback is `corpora.lock` + explicit fetch using the same URLs/SHAs. Qualification identity must never be floating HEAD.

## 6. Corpus manifest

P35 shall create deterministic ROML-owned corpus metadata, conceptually:

```text
corpus_id
source_repo
source_commit
source_archive_or_directory
relative_model_path
expected_dialect = supported | unsupported(reason)
qualification_tier = pr-smoke | scheduled | manual
known_rows
known_cols
known_nnz
known_feasibility = feasible | infeasible | unknown
notes
```

The manifest must not encode one Gurobi IIS as the only correct IIS.

## 7. Netlib role

The converted Netlib repository contains MPS conversions of classic Netlib LP models and notes that some cases were not converted. P35 treats it as a broad feasible-LP interoperability corpus, not as an authority for every MPS extension.

Goals:

- parse every file classified inside P35's accepted dialect;
- compare native HiGHS read with ROML read -> HiGHS;
- verify normalized structural equivalence where APIs permit;
- verify solve classification and objective tolerance;
- inventory unsupported syntax/features explicitly;
- discover real lexical/semantic edge cases before production qualification.

A Tier-1 allowlist is frozen during implementation planning after actual feature inventory rather than guessed from names.

## 8. Chinneck role

The Chinneck repository contains collections of deliberately infeasible LPs. Set 1 includes dense classification-derived LPs, free/lower-bounded variants, and empty `OBJFCN` objectives; the repository notes many models have numerous IISs and different solvers may isolate different IISs.

P35 uses it to validate:

- free-variable parsing;
- implicit lower-bound parsing/provenance;
- dense matrix scaling;
- empty objectives;
- infeasibility preservation;
- IIS source mapping to explicit and synthetic row/bound origins;
- Phase 29 irreducibility guarantees;
- runtime/memory characterization by tier.

Published Gurobi IIS counts are descriptive telemetry, not exact pass criteria.

## 9. Differential execution model

Each supported file has two independent construction paths.

### Native path

```text
file
 -> HiGHS readModel / Highs_readModel
 -> native HiGHS model
 -> inspect structure
 -> solve/status as needed
```

### ROML path

```text
file
 -> MpsReader
 -> MpsImport + Model
 -> ROML CompilationSession
 -> roml-highs
 -> inspect compiled/backend structure
 -> solve/status/IIS as needed
```

The ROML parser must never be implemented by invoking HiGHS `readModel` and extracting the result.

## 10. Differential authority and result classes

### 10.1 Accepted P35 inputs

For a file ROML accepts inside the P35 dialect, compare normalized semantics. A mismatch is not waved through because HiGHS is mature; it is a merge blocker until one reviewed disposition exists:

- `roml_bug_fixed` — ROML changes to match the frozen/authoritative semantics;
- `dialect_narrowed` — ROML deliberately rejects that input going forward;
- `compatibility_exception` — authoritative format evidence supports ROML's semantics and owner review explicitly accepts the documented divergence.

### 10.2 Intentional strict ROML rejection

For files outside ROML's strict accepted subset but still useful as probes — e.g. duplicate same-row selected RHS/RANGE or selected RANGE-on-`N` — the harness records:

```text
roml_parse_status = intentional_roml_rejection(<typed reason>)
native_highs_read_status = <observed result>
```

HiGHS accepting such a probe does not redefine the ROML semantics and is not reported as semantic equivalence.

### 10.3 No unresolved discrepancy class

A corpus report may record an investigation status during development, but P35 production qualification may not close with an accepted-input semantic discrepancy lacking one of the dispositions above.

## 11. Structural comparison contract

Normalize representation-only differences and compare where exposed:

- active E/G/L row count;
- variable count;
- nonzero matrix count after documented duplicate/zero normalization;
- coefficient values by names where reliable;
- row lower/upper bounds;
- column lower/upper bounds;
- integrality/binary status;
- objective coefficients, sense, and offset;
- model name as informational metadata when normalized by backend.

Floating comparisons use explicit absolute/relative tolerances recorded in the harness.

## 12. Solve comparison contract

### Feasible LPs

Require compatible solve classification and objective values within recorded tolerance. Do not require identical primal solutions because alternate optima may exist.

### Infeasible LPs

Require both accepted construction paths to establish infeasibility under the selected policy. Ambiguous/limit/numerical status is inconclusive/failure according to tier, never coerced to infeasible.

### MILPs

P35 supports linear MILP semantics. Synthetic MILP differential fixtures are mandatory even if the external corpora are LP-heavy.

## 13. IIS qualification and provenance contract

For a selected Chinneck model:

1. safely materialize the exact pinned archive;
2. parse with P35;
3. independently confirm native HiGHS reads the supported file;
4. establish infeasibility through ROML -> HiGHS;
5. run `analyze_infeasibility` under a recorded plan;
6. require `Conflict` only with appropriate evidence;
7. resolve every row member to an MPS row origin;
8. resolve every finite variable-bound member to **exactly one** explicit BOUNDS origin or synthetic MPS-default origin;
9. for complete `Irreducible`, rely on/verify Phase 29 final single-member deletion evidence;
10. record row/bound counts and Gurobi counts only as telemetry.

Synthetic bound provenance follows the semantics reference:

- continuous default lower `0` -> `ImplicitContinuousDefault` anchored at the variable's first COLUMNS record;
- INTORG lower `0` / upper `1` -> `ImplicitIntegerMarkerDefault` anchored at the controlling `INTORG` marker plus the variable's first marked COLUMNS record.

Synthetic provenance must be rendered as a format-derived default, never as a fabricated BOUNDS source line.

## 14. Tier policy

### Tier 0 — synthetic

Always runs. No submodules. Full semantic and security-fixture coverage.

### Tier 1 — PR corpus smoke

For MPS/IIS-impacting changes. Initializes exact submodules, safely materializes only required Chinneck archives, and runs a small deterministic allowlist.

### Tier 2 — scheduled broad corpus

Broad supported Netlib differential plus bounded Chinneck IIS set. Produces report artifact.

### Tier 3 — manual/release heavy

Large cases with explicit time/oracle-call/memory budgets. Used for robustness/performance evidence, not per-PR correctness timing gates.

The qualification executable is `roml-highs/examples/mps_corpus_qualification.rs`.
Run a bounded PR smoke without solve logs with:

```text
cargo run -p roml-highs --example mps_corpus_qualification -- . --max-files 3 --no-solve
```

Omit `--max-files` and `--no-solve` for the scheduled broad run. The command
validates both exact submodule pins, securely materializes the complete
reviewed Chinneck archives through the pre-write-safe materializer, emits
deterministic JSONL, and fails on an accepted-input structural discrepancy.
The resulting archive caches are fresh, atomically completed state under
`target/roml-corpora/chinneck`; model qualification still uses the explicit
reviewed allowlist.

## 15. Result schema

Per model:

```text
corpus
corpus_sha
source_archive_or_directory
path
file_bytes
roml_commit
roml_parse_status
roml_parse_ms
native_highs_read_status
differential_disposition
structural_comparison_status
rows
cols
nnz
variable_types
solve_status_roml
solve_status_native
objective_roml
objective_native
objective_delta
iis_requested
iis_outcome
iis_completion
iis_guarantee
iis_members_rows
iis_members_bounds
iis_explicit_bound_origins
iis_synthetic_bound_origins
iis_source_map_complete
iis_oracle_calls
iis_elapsed_ms
skip_or_failure_reason
```

Report-level environment metadata:

```text
OS/arch
Rust version
HiGHS version/build mode
CPU
memory
archive materializer/library version when used
workflow/run id
```

## 16. Corpus update protocol

A corpus pin update is dependency-like:

1. record old/new SHA;
2. inventory added/removed/changed files and archives;
3. invalidate old materialization cache;
4. rerun dialect classifier;
5. rerun Tier 1 and relevant Tier 2 qualification;
6. review new unsupported constructs/divergences;
7. commit manifest/evidence with gitlink change.

No floating branch reference is qualification identity.

## 17. Licensing/provenance posture

This is an engineering policy, not a legal opinion:

- retain upstream/fork URLs;
- do not copy model contents into ROML without redistribution review;
- submodules remain external and outside package artifacts;
- extracted Chinneck files remain generated/untracked;
- synthetic ROML-authored fixtures are the only mandatory in-repository MPS data;
- record any future upstream license in corpus metadata rather than assuming it.
