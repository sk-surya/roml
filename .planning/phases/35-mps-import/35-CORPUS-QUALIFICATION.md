# Phase 35 Corpus Qualification

## 1. Purpose

External corpora serve two independent goals:

1. **parser/model equivalence** — ROML must reconstruct the same linear LP/MILP semantics that an independent mature reader (HiGHS) sees;
2. **IIS workflow qualification** — imported infeasible models must remain infeasible and Phase 29 must produce correctly qualified, source-aware conflicts.

The corpora are not unit-test fixtures and are not shipped in the ROML crate.

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

At design time neither upstream exposes a root `LICENSE` file. The ROML repository therefore stores only submodule references/metadata, not copied dataset files. Any future vendoring requires a separate provenance/license review.

## 3. Planned submodule layout

```text
testdata/
└── corpora/
    ├── infeasible-lps/  -> sk-surya/infeasiblelps @ exact SHA
    └── netlib/          -> sk-surya/lp-data-netlib @ exact SHA
```

Properties:

- ordinary clone: submodule directories may be empty/uninitialized;
- ordinary `cargo test`: does not require them;
- crate package: `testdata/` is outside the existing package include allowlist;
- corpus command/workflow: verifies gitlink SHA before running;
- corpus update: reviewed dependency-like change with before/after qualification report.

## 4. Corpus materialization

The two repositories have different physical layouts and the harness must hide that difference behind one manifest/inventory layer.

### Netlib

The Netlib fork stores expanded `.mps` files under its `mps_files/` directory. The qualification harness reads those files directly from the initialized submodule.

### Chinneck

The Chinneck fork stores collections as archives such as:

```text
INFfromClassificationData.7z
INFfromNetlibLPs.7z
Arts.7z
expand.zip
```

P35 does **not** commit extracted copies and does not rewrite the fork merely for ROML's convenience. The corpus setup command extracts only the archive(s) needed by the selected tier into an ignored/generated directory, conceptually:

```text
target/roml-corpora/
└── chinneck/
    ├── INFfromClassificationData/
    ├── INFfromNetlibLPs/
    ├── Arts/
    └── expand/
```

Rules:

- source archives remain immutable inside the pinned submodule;
- extraction output is disposable and untracked;
- extraction is idempotent and keyed by corpus SHA + archive identity;
- a partial extraction is never reused as complete; setup writes an atomic completion marker only after success;
- the harness validates the expected archive exists before extraction;
- Linux corpus CI may install/use `7z`/`7zz` solely as qualification tooling; no archive library becomes a `roml` runtime dependency;
- ZIP extraction may use an available system tool or the same 7-Zip executable;
- absence of the extraction tool produces an actionable corpus-setup error, never a parser test failure;
- normal Tier 0 tests have no archive-tool requirement.

A future change may materialize the user's fork with expanded files, but that is not required by P35 and would be reviewed as a corpus-provenance change.

## 5. Why submodules instead of copied files

Submodules are chosen because they provide:

- exact immutable corpus identity in the ROML commit graph;
- straightforward local/offline reuse after initialization;
- no duplication of tens of MB of external data into ROML history;
- clear external provenance;
- owner-controlled fork endpoints;
- explicit corpus updates rather than floating downloads.

The known disadvantages are accepted and contained:

- recursive clone complexity -> only corpus workflows initialize them;
- archived Chinneck layout -> materialize into `target/roml-corpora`, never commit extracted data;
- GitHub archive behavior -> corpus qualification is not a source-package requirement;
- upstream/fork movement -> exact commits remain the reviewed identity; fork endpoints are owner-controlled.

If submodules later prove operationally harmful, the approved fallback is `corpora.lock` + explicit fetch command using the same URLs/SHAs. Tests must depend on the lock identity, not on HEAD.

## 6. Corpus manifest

P35 implementation shall create a deterministic manifest owned by ROML, conceptually:

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

The manifest is ROML-authored metadata and may be stored in the repository even though the model files are external.

The manifest must not encode a particular Gurobi IIS as the only correct IIS.

## 7. Netlib role

The converted Netlib repository contains MPS conversions of classic Netlib LP models and explicitly notes that some generator/problem cases were not converted. P35 treats this repository as a broad feasible-LP interoperability corpus, not as an authoritative standard for every possible MPS extension.

Qualification goals:

- parse all files classified inside P35's dialect;
- compare native HiGHS read with ROML read -> HiGHS;
- verify structural equivalence where API access permits;
- verify solve classification and objective value tolerance;
- inventory files that use unsupported syntax/features;
- discover real lexical/semantic edge cases before declaring the reader production-qualified.

The repository exposes many ordinary MPS files directly, including small cases such as `afiro.mps`. Exact Tier 1 selection is frozen after the implementation-plan inventory step rather than guessed from filenames alone.

## 8. Chinneck role

The Chinneck repository contains five collections of infeasible linear models. Set 1 alone includes dense classification-derived LPs with all-free or lower-bounded variable variants and empty objective `OBJFCN` rows. The repository notes that models often contain many IISs and that different solvers may isolate different IISs.

P35 therefore uses the corpus to validate:

- free-variable parsing;
- lower-bound parsing;
- dense matrix scaling;
- empty objective semantics;
- preservation of infeasibility;
- IIS source mapping to rows/bounds;
- correctness of ROML's irreducibility guarantee;
- IIS runtime/memory characterization across sizes.

Published Gurobi IIS row/bound counts are useful descriptive/reference data but are not exact pass criteria.

The initial Tier 1 IIS set shall come from the extracted classification archive and include at least one all-free model and one lower-bounded model. A small bound-participating case such as `IC-wine-LB.mps` is a preferred smoke candidate once extraction inventory confirms the exact archive path/name.

## 9. Differential execution model

Each file has two independent construction paths.

### Native path

```text
file
 -> HiGHS readModel / C API Highs_readModel
 -> native HiGHS model
 -> inspect structure
 -> solve/analyze status as needed
```

### ROML path

```text
file
 -> MpsReader
 -> MpsImport + Model
 -> ROML CompilationSession
 -> roml-highs
 -> inspect compiled/backend structure
 -> solve/analyze status as needed
```

The harness must never implement the ROML parser by calling `readModel` and extracting the model. Native HiGHS is test evidence only.

## 10. Structural comparison contract

Normalize before comparison where representation differs but mathematics does not.

Compare:

- number of active constraint rows (`E/G/L` only; objective/nonselected N excluded as appropriate);
- number of variables;
- nonzero matrix count after duplicate summation/zero normalization;
- coefficient values by row/column name where both APIs expose names reliably;
- row lower/upper bounds;
- column lower/upper bounds;
- integrality/binary status;
- objective coefficient vector;
- objective sense;
- objective constant/offset;
- model name when available (informational if a backend normalizes it).

Floating comparisons use explicit absolute/relative tolerances recorded in the harness; exact file decimals should normally round to identical `f64` values but the test contract must not rely on string formatting.

## 11. Solve comparison contract

### Feasible LPs

Require compatible solve classifications and objective values within a recorded tolerance. Do not require identical primal solutions because alternate optima may exist.

### Infeasible LPs

Require both paths to establish infeasibility under the selected solver policy. If one path returns an ambiguous/limit/numerical status, classify the corpus result as inconclusive/failure according to the qualification tier rather than coercing it to infeasible.

### MILPs

P35 parser supports linear MILP semantics. Corpus MILP qualification may initially be smaller than LP qualification if the chosen repositories are LP-heavy, but synthetic MILP differential fixtures are mandatory.

## 12. IIS qualification contract

For a selected Chinneck model:

1. materialize the source archive from the exact pinned submodule;
2. parse the extracted file with P35;
3. independently confirm native HiGHS can read the same supported file;
4. solve/establish infeasibility through the ROML HiGHS path;
5. run `analyze_infeasibility` under a recorded plan;
6. require `Conflict` only when the analysis has the evidence to make that claim;
7. require each member to resolve through import metadata to an MPS semantic row or variable bound source;
8. if guarantee is `Irreducible` and completion is complete, rely on/verify the Phase 29 final single-member deletion evidence;
9. record row-member count and bound-member count;
10. compare Gurobi published counts only as informational telemetry.

Different valid IIS membership is expected.

## 13. Tier policy

### Tier 0 — synthetic

Always runs. No submodules. Full semantic coverage.

### Tier 1 — PR corpus smoke

Runs on MPS/IIS-impacting changes. Initializes exact submodules, materializes only required Chinneck archive(s), and executes a small allowlist. Target is stable minutes-scale CI, not exhaustive IIS performance.

### Tier 2 — scheduled broad corpus

Runs broad supported Netlib parser/solve differential and a bounded Chinneck IIS set. Materializes required archives and produces a report artifact.

### Tier 3 — manual/release heavy

Runs large Chinneck/Netlib-derived infeasible cases with explicitly configured time/oracle-call/memory budgets. Used for performance/robustness evidence and algorithm research; not a required per-PR gate.

## 14. Result schema

The qualification harness should emit deterministic JSON plus a human-readable summary. Per model:

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
iis_source_map_complete
iis_oracle_calls
iis_elapsed_ms
skip_or_failure_reason
```

Machine/environment metadata belongs at report level:

```text
OS/arch
Rust version
HiGHS version/build mode
CPU
memory
7z/7zz version when Chinneck archives are materialized
workflow/run id where applicable
```

## 15. Corpus update protocol

A submodule pin update is treated like a benchmark dependency update:

1. record old/new SHA;
2. inventory added/removed/changed files and archive names;
3. invalidate any materialization cache keyed to the old SHA;
4. rerun supported-dialect classifier;
5. rerun Tier 1 and relevant Tier 2 qualification;
6. review any newly unsupported constructs;
7. commit updated manifest/evidence with the gitlink change.

No floating branch reference is used as the qualification identity.

## 16. Licensing/provenance posture

This packet is not a legal opinion. Engineering rules are:

- retain upstream and fork URLs in documentation;
- do not copy model contents into ROML without explicit redistribution review;
- submodules are external repositories and remain outside package artifacts;
- extracted Chinneck files remain generated/untracked test material;
- synthetic fixtures authored for ROML are the only mandatory in-repository MPS data;
- if upstream later publishes a license, record it in corpus metadata rather than assuming it retroactively.
