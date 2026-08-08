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

## 4. Why submodules instead of copied files

Submodules are chosen because they provide:

- exact immutable corpus identity in the ROML commit graph;
- straightforward local/offline reuse after initialization;
- no duplication of tens of MB of external data into ROML history;
- clear external provenance;
- owner-controlled fork endpoints;
- explicit corpus updates rather than floating downloads.

The known disadvantages are accepted and contained:

- recursive clone complexity -> only corpus workflows initialize them;
- GitHub archive behavior -> corpus qualification is not a source-package requirement;
- upstream/fork movement -> exact commits remain the reviewed identity; fork endpoints are owner-controlled.

If submodules later prove operationally harmful, the approved fallback is `corpora.lock` + explicit fetch command using the same URLs/SHAs. Tests must depend on the lock identity, not on HEAD.

## 5. Corpus manifest

P35 implementation shall create a deterministic manifest owned by ROML, conceptually:

```text
corpus_id
relative_path
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

## 6. Netlib role

The converted Netlib repository contains MPS conversions of classic Netlib LP models and explicitly notes that some generator/problem cases were not converted. P35 treats this repository as a broad feasible-LP interoperability corpus, not as an authoritative standard for every possible MPS extension.

Qualification goals:

- parse all files classified inside P35's dialect;
- compare native HiGHS read with ROML read -> HiGHS;
- verify structural equivalence where API access permits;
- verify solve classification and objective value tolerance;
- inventory files that use unsupported syntax/features;
- discover real lexical/semantic edge cases before declaring the reader production-qualified.

## 7. Chinneck role

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

## 8. Differential execution model

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

## 9. Structural comparison contract

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

## 10. Solve comparison contract

### Feasible LPs

Require compatible solve classifications and objective values within a recorded tolerance. Do not require identical primal solutions because alternate optima may exist.

### Infeasible LPs

Require both paths to establish infeasibility under the selected solver policy. If one path returns an ambiguous/limit/numerical status, classify the corpus result as inconclusive/failure according to the qualification tier rather than coercing it to infeasible.

### MILPs

P35 parser supports linear MILP semantics. Corpus MILP qualification may initially be smaller than LP qualification if the chosen repositories are LP-heavy, but synthetic MILP differential fixtures are mandatory.

## 11. IIS qualification contract

For a selected Chinneck model:

1. parse with P35;
2. independently confirm native HiGHS can read the same supported file;
3. solve/establish infeasibility through the ROML HiGHS path;
4. run `analyze_infeasibility` under a recorded plan;
5. require `Conflict` only when the analysis has the evidence to make that claim;
6. require each member to resolve through import metadata to an MPS semantic row or variable bound source;
7. if guarantee is `Irreducible` and completion is complete, rely on/verify the Phase 29 final single-member deletion evidence;
8. record row-member count and bound-member count;
9. compare Gurobi published counts only as informational telemetry.

Different valid IIS membership is expected.

## 12. Tier policy

### Tier 0 — synthetic

Always runs. No submodules. Full semantic coverage.

### Tier 1 — PR corpus smoke

Runs on MPS/IIS-impacting changes. Initializes exact submodules. Small allowlist only. Target is stable minutes-scale CI, not exhaustive IIS performance.

### Tier 2 — scheduled broad corpus

Runs broad supported Netlib parser/solve differential and a bounded Chinneck IIS set. Produces report artifact.

### Tier 3 — manual/release heavy

Runs large Chinneck/Netlib-derived infeasible cases with explicitly configured time/oracle-call/memory budgets. Used for performance/robustness evidence and algorithm research; not a required per-PR gate.

## 13. Result schema

The qualification harness should emit deterministic JSON plus a human-readable summary. Per model:

```text
corpus
corpus_sha
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
workflow/run id where applicable
```

## 14. Corpus update protocol

A submodule pin update is treated like a benchmark dependency update:

1. record old/new SHA;
2. inventory added/removed/changed files;
3. rerun supported-dialect classifier;
4. rerun Tier 1 and relevant Tier 2 qualification;
5. review any newly unsupported constructs;
6. commit updated manifest/evidence with the gitlink change.

No floating branch reference is used as the qualification identity.

## 15. Licensing/provenance posture

This packet is not a legal opinion. Engineering rules are:

- retain upstream and fork URLs in documentation;
- do not copy model contents into ROML without explicit redistribution review;
- submodules are external repositories and remain outside package artifacts;
- synthetic fixtures authored for ROML are the only mandatory in-repository MPS data;
- if upstream later publishes a license, record it in corpus metadata rather than assuming it retroactively.
