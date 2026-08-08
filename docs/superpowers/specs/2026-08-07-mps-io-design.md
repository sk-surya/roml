# MPS I/O Architecture Design

**Date:** 2026-08-07  
**Status:** owner-approved architecture, written-spec review pending  
**Repository baseline:** `main@ff99389b9bf1318c555dc1f72dff6b5c7a4111c0`  
**Execution order:** P35 reader/qualification first; P36 writer/round-trip second

## 1. Purpose

ROML needs production-grade interchange with established optimization ecosystems. The immediate driver is to load external infeasible LP/MILP instances and exercise the Phase 29 IIS functionality on real models. The durable goal is broader: MPS becomes the first member of a solver-independent file I/O layer that can later support LP and other interchange formats without pushing file-format concerns into `Model`.

This design deliberately separates **parsing**, **MPS semantic resolution**, **ROML model construction**, and **serialization**. It supports linear LP/MILP MPS only. Nonlinear, quadratic, conic, SOS, indicator, and other vendor extensions fail explicitly rather than being ignored or silently linearized.

## 2. Phase split

### P35 — MPS reader and external qualification

P35 implements:

- fixed-format and free-format MPS reading;
- deterministic format detection;
- standard linear LP/MILP sections and integer markers;
- a staging representation that preserves MPS semantics until validation is complete;
- conversion to a canonical ROML `Model`;
- source/provenance metadata outside the canonical model;
- typed diagnostics;
- differential qualification against HiGHS file reading;
- external corpus qualification using pinned Chinneck and Netlib repositories;
- end-to-end `MPS -> ROML -> HiGHS -> IIS` examples.

### P36 — MPS writer and round-trip qualification

P36 reuses the P35 semantic layer and implements:

- deterministic MPS writing from representable ROML models;
- free-format output as the default canonical writer dialect;
- optional fixed-format output only if it can be emitted without lossy name transformation or undocumented truncation;
- explicit representation errors for ROML constructs that cannot be represented as linear MPS;
- `Model -> MPS -> ROML` semantic round trips;
- `MPS -> ROML -> MPS -> HiGHS` differential round trips;
- stable/canonical output suitable for diffing when deterministic options are used.

P35 must not design itself into a read-only corner. Reader and writer share the same MPS semantic vocabulary and options where appropriate, but the reader remains streaming and the writer does not require preserving the original byte layout.

## 3. Non-goals

The first two phases do not implement:

- quadratic MPS sections such as `QMATRIX`, `QSECTION`, `QUADOBJ`, or `QCMATRIX`;
- conic sections such as `CSECTION`;
- SOS, indicators, lazy constraints, user cuts, PWL objective extensions, or vendor-specific directives;
- exact textual round-trip preservation;
- comments/whitespace preservation;
- minimum-cardinality IIS or solver-specific IIS equality;
- direct delegation to a solver's MPS parser as ROML's public implementation;
- a general universal interchange IR for every future file format.

Unsupported semantic sections are errors. They are never skipped merely because the linear subset could still be parsed.

## 4. Parser-generator decision

P35 uses a **handwritten streaming section/state parser** and does not use LALRPOP.

MPS is fundamentally a line-oriented record format whose interpretation depends on the active section and lexical layout. The difficult behavior is fixed-column extraction, free-field extraction, section transitions, `INTORG`/`INTEND` state, named RHS/RANGES/BOUNDS vectors, duplicate coefficient accumulation, and high-quality source diagnostics. These are clearer in a small explicit state machine than in an LR grammar.

No parser-combinator dependency is added initially. Small lexical helpers remain ordinary Rust functions. This preserves ROML's small dependency surface and keeps the file reader auditable under Rust 1.85.

A parser generator may be reconsidered for a future format if that format is genuinely grammar-dominated.

## 5. Module boundary

The target public namespace is:

```text
roml::io
└── mps
    ├── reader
    ├── writer        # P36
    ├── options
    ├── error
    ├── metadata
    └── semantic      # internal MPS staging/resolution types
```

`Model` does not own file parsing. The preferred API is importer-oriented:

```rust
let imported = roml::io::mps::MpsReader::new().read_path("model.mps")?;
let mut model = imported.model;
```

and stream-oriented:

```rust
let imported = MpsReader::with_options(options).read(reader)?;
```

P36 adds writer-oriented APIs rather than `Model::write_mps` as the architectural primitive:

```rust
MpsWriter::new().write_path(&model, "model.mps")?;
MpsWriter::with_options(options).write(&model, writer)?;
```

Convenience extension methods may be considered later only if they remain thin wrappers around the I/O layer.

## 6. Three-stage read pipeline

P35 uses three explicit stages:

```text
BufRead
  |
  v
lexical record extraction
  |  fixed/free field locations + source spans
  v
MPS document/staging representation
  |  section/order validation + vector selection + semantic resolution
  v
ROML Model construction
```

### 6.1 Lexical layer

Responsibilities:

- line numbering and byte/column spans;
- blank lines and comments;
- fixed-column field extraction;
- free-field token extraction;
- numeric parsing;
- quoted marker recognition where permitted;
- CRLF/LF normalization without changing reported logical positions;
- deterministic fixed/free detection.

The lexical layer does not know ROML IDs or mutate a model.

### 6.2 MPS staging document

The staging document stores MPS-native concepts:

- problem name;
- objective sense and optional `OBJNAME`;
- ordered row declarations;
- ordered columns and their row/value entries;
- integer-marker state captured per column;
- named RHS vectors;
- named RANGES vectors;
- named BOUNDS vectors;
- source locations for each declaration/record;
- warnings that do not affect mathematical meaning.

It is intentionally MPS-specific. It is not the new canonical model IR.

### 6.3 Semantic resolution / model construction

Only after the document is syntactically and structurally valid does the resolver:

- select the objective row;
- select named/default RHS, RANGES, and BOUNDS vectors;
- apply MPS default bounds;
- resolve integer/binary domains;
- sum duplicate matrix entries;
- derive ranged-row lower/upper bounds;
- apply the objective constant convention;
- validate finite/NaN/ordering rules required by ROML;
- construct a fresh ROML `Model` transactionally.

Failure produces no partially usable `Model` result.

## 7. Supported P35/P36 linear dialect

P35 reads and P36 writes the representable subset of:

```text
NAME
OBJSENSE
OBJNAME
ROWS
COLUMNS
RHS
RANGES
BOUNDS
ENDATA
```

Supported row kinds:

```text
N  nonconstraint/objective candidate
E  equality
L  <=
G  >=
```

Supported integer marker semantics:

```text
'MARKER' 'INTORG'
'MARKER' 'INTEND'
```

Supported linear bound records:

```text
FR  free
FX  fixed
LO  lower bound
MI  minus infinity lower bound
PL  plus infinity upper bound
UP  upper bound
BV  binary
LI  integer lower bound
UI  integer upper bound
```

Semi-continuous/semi-integer and extension records are not included until separately specified and tested.

## 8. Fixed/free input and detection

```rust
pub enum MpsFormat {
    Auto,
    Fixed,
    Free,
}
```

Default input mode is `Auto`.

`Auto` does not use indentation guessing. Before the format is locked, each meaningful record is attempted under both lexical layouts:

- exactly one succeeds: lock that layout;
- both succeed to the same semantic record: accept it and remain undecided;
- both succeed to different records: return `AmbiguousFormat` with the line and both interpretations;
- neither succeeds: return the best section-aware syntax error.

After the format is locked, later records are parsed only in that layout. Mixed-layout files are rejected unless a future compatibility option is explicitly designed.

P36 defaults to deterministic free-format output because it preserves long names and avoids fixed-column truncation. Fixed-format writing is optional and strict: names/fields that do not fit must produce a representation error unless an explicit, reversible naming policy is added in a later design.

## 9. Objective semantics

Objective selection is deterministic:

1. if `OBJNAME` is present, select the referenced `N` row;
2. otherwise select the first `N` row;
3. if no objective row exists, construct a zero objective;
4. if `OBJSENSE` is absent, default to minimization.

Additional `N` rows remain nonconstraint rows and are preserved in import metadata where useful; they are not silently converted into constraints.

RHS entries on the selected objective row use the mainstream MPS convention:

```text
objective_constant = -rhs_value_on_objective_row
```

This rule gets dedicated fixtures because sign errors are easy and solver-dependent folklore is not acceptable.

## 10. Rows, ranges, and semantic preservation

A ranged MPS row becomes one ranged ROML semantic constraint, not two synthetic constraints.

The resolver applies standard MPS range semantics based on the row kind, RHS value, and sign/magnitude of the RANGE value. The exact table is frozen in the P35 semantics reference and tested exhaustively.

This is important for Phase 29 IIS reports: a conflict member should map to the original MPS row and bound semantics rather than importer-created artifacts.

## 11. Matrix representation and duplicates

Parsing directly into `HashMap<(row, col), f64>` is rejected because of memory overhead on large sparse models.

The staging representation is column-oriented:

```rust
struct MpsColumn {
    name: String,
    integer_marker: bool,
    entries: Vec<(RowIndex, f64, SourceSpan)>,
}
```

Rows are interned in declaration order. Duplicate `(row, column)` entries are accumulated algebraically during finalization. Exact-zero results are normalized according to ROML's canonical coefficient rules.

The target complexity is:

- streaming parse: O(lines);
- memory: O(rows + columns + nnz + selected metadata);
- finalization: near O(nnz), with local sorting/grouping only when duplicate detection requires it.

Large-corpus benchmarks must report peak memory and throughput but cannot become brittle CI timing gates without stable runners.

## 12. RHS/RANGES/BOUNDS vector selection

MPS can contain multiple named rim vectors. P35 preserves them in staging and resolves one selected vector of each kind.

```rust
pub enum MpsVectorSelection {
    First,
    Named(String),
    None,
}
```

Defaults:

```text
RHS     -> First
RANGES  -> First
BOUNDS  -> First
```

The import metadata records the selected names. A requested missing vector is a typed error. Nonselected vectors do not alter the model.

P36 emits one deterministic RHS, one RANGES vector when necessary, and one BOUNDS vector when necessary. Canonical names are writer options; output must not depend on hash iteration order.

## 13. Variable-domain semantics

Default continuous MPS variable domain:

```text
0 <= x <= +inf
```

Variables declared inside `INTORG`/`INTEND` follow standard MPS integer-marker defaults, including the legacy 0..1 default when no overriding bound information is supplied. `BV`, `LI`, and `UI` explicitly establish integer/binary semantics.

Conflicting or impossible combinations produce typed semantic errors rather than last-write-wins surprises unless standard MPS ordering unambiguously specifies the result. The semantics reference freezes precedence and repeated-bound behavior before implementation.

P36 derives the minimal deterministic bound record set required to reproduce the ROML domain under P35 semantics.

## 14. Source provenance

File provenance stays outside canonical `Model` state.

```rust
pub struct MpsImport {
    pub model: Model,
    pub metadata: MpsMetadata,
    pub source_map: MpsSourceMap,
    pub diagnostics: Vec<MpsDiagnostic>,
}
```

`MpsSourceMap` maps imported ROML entities back to relevant source records, including:

- row declaration;
- coefficient records when useful for diagnostics;
- RHS/RANGE record;
- variable declaration-through-first-column occurrence;
- bound records;
- integer marker context.

This allows a future IIS renderer or example to annotate members with `file:line` without making file locations part of solver/compiler semantics.

P36 does not promise to reproduce the original source map. It writes canonical output and may optionally return a new output source map later.

## 15. Error model

Errors are typed and source-aware. The public category is non-exhaustive. Representative kinds:

```text
Io
InvalidEncoding
InvalidSectionOrder
UnsupportedSection
InvalidRecord
InvalidNumber
DuplicateRow
UnknownRow
UnknownVariable
InvalidMarkerNesting
InvalidBound
InvalidRange
MissingRequiredSection
MissingEndata
UnknownVector
AmbiguousFormat
RepresentationError
ModelConstruction
```

Each syntax/semantic error carries the most precise available location and context:

```text
path
line
column/span
active section
raw field/record
entity name when known
```

Warnings are reserved for mathematically equivalent normalization or compatibility conditions that do not make interpretation ambiguous. The reader must not downgrade unsupported semantics into warnings.

## 16. Corpus ownership and repository layout

External corpus files are **not copied into ROML**.

The planned layout is:

```text
testdata/
└── corpora/
    ├── infeasible-lps/   # git submodule
    └── netlib/           # git submodule
```

Pinned sources at design time:

```text
sk-surya/infeasiblelps@97a936498e5240d44adaf7dcfe84877fa34ce301
sk-surya/lp-data-netlib@56257eea85b433ce6aa67d26156b36385318fd6f
```

These are forks of the original public repositories and preserve their provenance. At design time neither upstream repository exposes a root `LICENSE` file. Therefore ROML must not vendor/copy the dataset contents into its own package or source tree without a separate licensing decision.

The submodule gitlinks and `.gitmodules` are repository metadata; ordinary `cargo package` remains controlled by the existing include allowlist and does not ship `testdata/`.

Normal CI and `cargo test` must work with submodules absent. A dedicated corpus qualification job initializes them recursively/pinned. Local developers opt in with the documented submodule command.

If submodules become operationally painful, the fallback is a manifest of repository URLs + exact SHAs and an explicit fetch script. The mathematical qualification contract does not depend on the acquisition mechanism.

## 17. HiGHS differential oracle

HiGHS file reading is an **independent test oracle**, not ROML's implementation.

For each qualifying MPS model:

### Path A — native HiGHS read

```text
file.mps
  -> HiGHS readModel / Highs_readModel
  -> extracted/solved HiGHS model
```

### Path B — ROML read

```text
file.mps
  -> ROML MpsReader
  -> ROML Model
  -> ROML compiler
  -> HiGHS backend
```

Compare, where exposed and semantically meaningful:

- row count;
- column count;
- nonzero structure/count after duplicate normalization;
- row lower/upper bounds;
- column lower/upper bounds;
- variable integrality;
- objective sense;
- objective coefficients;
- objective offset;
- model feasibility/termination classification;
- objective value for solved feasible models within recorded tolerances.

The qualification harness must distinguish a parser mismatch from a solver numerical/termination difference.

For MPS extensions that HiGHS accepts but P35 deliberately does not support, ROML's typed `UnsupportedSection` remains correct; differential acceptance is only for the declared P35 dialect.

## 18. Chinneck IIS qualification

The Chinneck corpus is valuable because it contains deliberately infeasible LPs, many with multiple IISs, free-vs-lower-bounded pairs, empty objectives, dense matrices, and large bound participation.

The acceptance test is **not** equality with Gurobi's published IIS membership. Different solvers can validly return different IISs.

For selected corpus tiers:

1. ROML parses the MPS file successfully.
2. Structural statistics match known corpus metadata when available.
3. ROML -> HiGHS proves the model infeasible.
4. Phase 29 analysis returns a conflict with a guarantee appropriate to completion.
5. Every conflict member maps to an original MPS row/bound/fixing semantic origin.
6. For a complete irreducible result, the existing final-verification contract independently proves the full set infeasible and each one-member deletion feasible under the recorded numerical policy.
7. Results record oracle calls, elapsed time, and member counts without claiming minimum-cardinality IIS.

Large instances are tiered so pull-request CI remains bounded while scheduled/manual qualification can run the heavier sets.

## 19. Netlib qualification

The converted Netlib corpus is primarily a parser/model-equivalence and feasible-solve corpus.

It tests real-world diversity in:

- sparse matrix patterns;
- equality/inequality mixtures;
- ranges and bounds;
- scaling and numerical magnitudes;
- objective data;
- larger row/column counts;
- naming/layout patterns from historical MPS tooling.

For each selected model, compare ROML-imported HiGHS behavior with native HiGHS `readModel` behavior. Feasible models should agree on solve status and objective value within explicit tolerances. Structural comparisons should be preferred over solution-vector equality when alternate optima exist.

A corpus manifest records expected support/skip reasons for files outside the declared P35 dialect.

## 20. Test strategy

### Unit/lexical

- fixed fields at boundary columns;
- free fields and long names;
- comments/blank lines;
- LF and CRLF;
- scientific notation and signed values;
- malformed numeric fields;
- non-UTF/non-ASCII policy fixtures;
- auto-detection ambiguity.

### Grammar/section

Every supported section and legal transition, plus illegal order, duplicate sections where forbidden, missing sections, missing `ENDATA`, marker nesting errors, and unsupported section detection.

### Semantic

- `E/L/G/N` rows;
- first/explicit objective selection;
- minimize/maximize;
- zero/empty objective;
- objective RHS sign convention;
- all RANGE row/sign cases;
- default continuous bounds;
- `FR/FX/LO/MI/PL/UP/BV/LI/UI`;
- integer markers and repeated regions;
- multiple named rim vectors;
- duplicate coefficients;
- zero-after-sum normalization;
- name preservation;
- transactional failure/no partial model.

### Metamorphic

Equivalent mathematical models encoded with:

- fixed versus free format;
- reordered rows/columns when legal;
- split versus combined coefficient records;
- duplicate additive matrix entries;
- explicit versus default bounds;
- writer canonicalization.

### Differential

Native HiGHS read versus ROML read -> HiGHS on synthetic fixtures, selected Netlib models, and selected Chinneck models.

### Round-trip (P36)

- ROML model -> write -> read -> semantic snapshot equality;
- read external MPS -> write canonical MPS -> read -> equality;
- read external MPS -> write -> native HiGHS read -> structural/solve equivalence;
- deterministic byte output for identical model + writer options.

## 21. CI tiers

### Tier 0 — normal PR CI

No external submodules required. Synthetic fixtures only. Runs on Linux/macOS/Windows + MSRV as appropriate.

### Tier 1 — corpus smoke

Dedicated Linux job initializes pinned submodules and runs a small deterministic allowlist from both corpora. Suitable for PRs touching MPS or IIS code.

### Tier 2 — corpus qualification

Scheduled/manual Linux job runs the broad supported corpus and stores a machine-readable report artifact with model counts, support/skip reasons, structural mismatches, statuses, objectives, IIS statistics, timing, and versions.

### Tier 3 — performance characterization

Manual/release qualification on declared hardware. Measures parse throughput, peak RSS where tooling permits, compile/solve overhead, and IIS cost. Informational until a stable benchmark policy is approved.

## 22. Public API compatibility principles

- parser/writer option structs and result/error enums are `#[non_exhaustive]` where future dialect additions are expected;
- MPS-specific staging internals remain private;
- public metadata does not expose implementation-specific row/column storage;
- adding another file format must not require changing `Model` internals;
- writer failures distinguish "invalid ROML model" from "valid ROML model not representable in selected MPS dialect";
- future LP reader/writer should reuse generic I/O conventions, diagnostics, and qualification patterns, not MPS record types.

## 23. Security/robustness

The reader consumes untrusted text files and must defend against accidental or malicious resource abuse:

- configurable maximum line length;
- configurable maximum rows/columns/nonzeros/records where practical;
- checked integer arithmetic for counters/capacity estimates;
- no recursion proportional to file size;
- no panics on malformed input;
- no path traversal behavior because parsing consumes caller-provided streams/paths only;
- fuzz targets for lexical/record parser and end-to-end parse-to-error behavior;
- memory behavior characterized on large corpora.

Default limits should be generous enough for benchmark models and may initially be unlimited where imposing a correct default would be arbitrary; the API seam must allow future limits without redesign.

## 24. Decision summary

- D1: handwritten streaming parser; no LALRPOP.
- D2: `roml::io::mps`, not `Model`-owned parsing.
- D3: lexical -> MPS staging -> semantic resolution -> fresh Model.
- D4: fixed + free input; deterministic auto-detection.
- D5: linear LP/MILP only.
- D6: unsupported semantic extensions fail loudly.
- D7: preserve one semantic ROML row for one ranged MPS row.
- D8: duplicate matrix coefficients add algebraically.
- D9: first RHS/RANGES/BOUNDS vector by default; named selection supported.
- D10: objective row selection is `OBJNAME` then first `N`; default minimize.
- D11: objective RHS follows the negative-offset convention.
- D12: MPS variable defaults and integer-marker defaults are explicit and tested.
- D13: source provenance lives in `MpsImport`, outside canonical `Model`.
- D14: P35 does not copy external corpus data into ROML.
- D15: corpora use optional pinned submodules to the owner's forks.
- D16: ordinary tests/packaging do not require submodules.
- D17: native HiGHS `readModel` is a differential oracle, not the implementation.
- D18: IIS corpus tests validate ROML's guarantee, not equality to a solver's particular IIS.
- D19: P36 write-back is designed now and implemented after the reader is qualified.
- D20: default writer output is deterministic free MPS; fixed output is strict/optional.
- D21: round-trip equivalence is semantic, not byte-preserving.
- D22: no general interchange IR is introduced before a second format demonstrates the need.

## 25. Acceptance boundary

The design is ready for implementation planning when the owner accepts this written specification and the companion P35 planning packet contains no unresolved semantic placeholders. The implementation plan must then be generated from this spec using the repository's Superpowers/TDD workflow. Production code, submodule gitlinks, and roadmap routing changes wait for that approval gate.
