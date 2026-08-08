# MPS I/O Architecture Design

**Date:** 2026-08-07  
**Status:** owner-approved architecture; written-spec review round 1 addressed; independent re-review pending  
**Repository baseline:** `main@ff99389b9bf1318c555dc1f72dff6b5c7a4111c0`  
**Execution order:** P35 reader/qualification first; P36 writer/round-trip second

## 1. Purpose

ROML needs production-grade interchange with established optimization ecosystems. The immediate driver is to load real infeasible LPs and exercise Phase 29 IIS. The durable goal is broader: MPS becomes the first member of a solver-independent `roml::io` layer that can later support LP and other formats without pushing file concerns into `Model`.

This design separates:

```text
lexical parsing
 -> MPS record/staging representation
 -> selected-vector semantic resolution
 -> transactional ROML Model construction
 -> solver-independent metadata/provenance
```

P35 supports **linear LP/MILP MPS only**. Unsupported nonlinear/quadratic/conic/vendor semantics fail explicitly.

## 2. Phase split

### P35 — MPS reader and external qualification

P35 implements:

- fixed and free MPS reading;
- deterministic format detection;
- standard linear LP/MILP sections and integer markers;
- MPS-specific staging before model construction;
- selected RHS/RANGES/BOUNDS semantics;
- source-aware explicit and synthetic provenance;
- typed diagnostics;
- differential qualification against HiGHS `readModel`;
- external qualification using pinned Netlib and Chinneck corpora;
- end-to-end `MPS -> ROML -> HiGHS -> IIS` evidence.

### P36 — MPS writer and round-trip qualification

P36 reuses the P35 semantic contract and implements:

- deterministic MPS writing from representable ROML models;
- free format as default canonical output;
- strict optional fixed output only if lossless;
- typed representation failures;
- `Model -> MPS -> ROML` semantic round trips;
- `MPS -> ROML -> MPS -> HiGHS` external round trips.

P35 is designed so P36 does not need to reverse-engineer parser internals.

## 3. Non-goals

P35/P36 do not initially implement:

- `QMATRIX`, `QSECTION`, `QUADOBJ`, `QCMATRIX`;
- `CSECTION`;
- SOS, indicators, lazy constraints, user cuts, PWL objective extensions;
- semi-continuous/semi-integer MPS records;
- arbitrary vendor directives;
- textual/comment/whitespace-preserving round trip;
- direct delegation to a solver's MPS parser;
- a universal interchange IR for all future file formats.

Unsupported semantics are errors, never silent omissions.

## 4. Parser technology

P35 uses a **handwritten streaming state machine**. It does not use LALRPOP and initially adds no parser-combinator dependency.

MPS is dominated by line/field layout and section state rather than context-free grammar complexity:

- fixed-column versus free-field lexing;
- section transitions;
- `INTORG`/`INTEND` state;
- named RHS/RANGES/BOUNDS alternatives;
- source spans;
- duplicate matrix accumulation.

A parser generator may be appropriate for a later grammar-dominated format, but not for MPS.

## 5. Public/module boundary

Target namespace:

```text
roml::io
└── mps
    ├── reader
    ├── writer        # P36
    ├── options
    ├── error
    ├── metadata
    └── semantic      # private staging/resolution
```

Primary read API shape:

```rust
let imported = MpsReader::new().read_path("model.mps")?;
let model = imported.model;
```

and:

```rust
let imported = MpsReader::with_options(options).read(reader)?;
```

`Model::from_mps` is not the architectural primitive. P36 mirrors the importer with `MpsWriter`.

## 6. Read pipeline and validation layers

```text
path / BufRead
    |
    v
fixed/free lexical records + spans
    |
    v
section/marker state machine
    |
    v
MPS staging document
    |
    +--> validate syntax/references for ALL staged records
    |
    v
select objective + one RHS/RANGES/BOUNDS vector each
    |
    +--> validate MODEL SEMANTICS for SELECTED vectors
    |
    v
fresh ROML Model + MpsMetadata + MpsSourceMap
```

No partially constructed model is returned after an input failure.

### 6.1 Whole-document structural validation

Every vector, selected or not, must have:

- supported record kinds;
- valid numeric syntax/finite values;
- declared row references for COLUMNS/RHS/RANGES;
- declared variable references for BOUNDS;
- legal section and marker structure.

Malformed or dangling records cannot hide in an unused vector.

### 6.2 Selected-vector semantic validation

Only the selected vector affects the model. Therefore semantics that arise from applying an alternative vector are validated only when that vector is selected.

Examples:

```text
RANGE-on-N in unselected vector
    -> structurally valid, staged, inert

select that vector
    -> typed semantic rejection

unselected BOUNDS vector resolves lower > upper
    -> structurally valid, staged, inert

select that vector
    -> typed domain rejection
```

This distinction is binding for P35.

## 7. Supported dialect

```text
NAME
OBJSENSE
OBJNAME
ROWS       E G L N
COLUMNS
RHS
RANGES
BOUNDS     FR FX LO MI PL UP BV LI UI
INTORG / INTEND
ENDATA
fixed MPS
free MPS
linear LP
linear MILP
```

## 8. Fixed/free detection

```rust
pub enum MpsFormat {
    Auto,
    Fixed,
    Free,
}
```

`Auto` dual-interprets meaningful records until layout is determined:

- only fixed succeeds -> lock Fixed;
- only free succeeds -> lock Free;
- both succeed with same record -> accept and remain undecided;
- both succeed differently -> `AmbiguousFormat`;
- neither succeeds -> section-aware syntax error.

Once locked, mixed layout is rejected in P35.

## 9. Objective semantics

Selection:

1. `OBJNAME` if present and referencing an `N` row;
2. otherwise first `N` row;
3. otherwise zero objective.

Absent `OBJSENSE` means minimize.

Selected objective-row RHS follows the negative-offset convention:

```text
objective_constant = -rhs_value
```

Additional `N` rows remain nonconstraint metadata.

## 10. Matrix semantics

Staging is column-oriented rather than a hash map keyed by `(row,column)`.

Duplicate COLUMNS cells are added algebraically, including duplicate selected-objective coefficients. Exact zero after accumulation follows ROML canonical normalization.

P35 accepts repeated variable blocks and merges them deterministically even though strict historical MPS expects grouping.

**The additive duplicate rule is COLUMNS-only.** It is not generalized to RHS/RANGES.

## 11. Rim-vector semantics

Multiple named RHS/RANGES/BOUNDS vectors are staged. Selection:

```rust
pub enum MpsVectorSelection {
    First,
    Named(String),
    None,
}
```

Default is `First` for each class. Selected vector names are recorded in metadata.

### 11.1 RHS duplicates

A selected RHS vector specifying the same row more than once is rejected as ambiguous. P35 does not sum, overwrite, first-win, or last-win duplicate RHS assignments.

An unselected duplicate-containing RHS vector can remain staged if all syntax/references are structurally valid; selecting it produces the typed duplicate error.

### 11.2 RANGES

For selected RHS `b` and selected range `r`:

```text
E, r >= 0 : b       <= a'x <= b + r
E, r <  0 : b + r   <= a'x <= b
G           : b       <= a'x <= b + |r|
L           : b-|r|   <= a'x <= b
```

A ranged MPS row maps to one ranged ROML semantic constraint.

MOSEK's current MPS reference defines the RANGE transformation for E/G/L and leaves N without a transformation. P35 policy is therefore:

```text
selected RANGE-on-N   -> typed error
unselected RANGE-on-N -> stage if syntax/reference-valid; inert
```

Duplicate same-row entries in the selected RANGES vector are also rejected rather than accumulated.

### 11.3 BOUNDS records

BOUNDS is an ordered state-transition stream, not an RHS-style assignment table. Repeated records execute in file order under the bound transition rules below.

## 12. Variable-domain semantics

Default continuous variable:

```text
Continuous [0,+inf]
```

Variable appearing while `INTORG` is active:

```text
Integer [0,1]
```

Selected BOUNDS transitions:

```text
FR       -> [-inf,+inf]
FX v     -> [v,v]
LO v     -> lower=v
MI       -> lower=-inf
PL       -> upper=+inf
UP v     -> upper=v
BV       -> Binary [0,1]
LI v     -> Integer lower=ceil(v)
UI v     -> Integer upper=floor(v)
```

Transitions apply in file order. Ordinary bound records do not erase integrality. Current ROML supports unbounded general integer domains, so `INTORG + FR -> Integer [-inf,+inf]` is supported.

## 13. Provenance and implicit defaults

Source provenance lives in `MpsImport`, not canonical `Model` state:

```rust
pub struct MpsImport {
    pub model: Model,
    pub metadata: MpsMetadata,
    pub source_map: MpsSourceMap,
    pub diagnostics: Vec<MpsDiagnostic>,
}
```

Explicit selected BOUNDS records retain exact source spans.

P35 also assigns **synthetic MPS provenance** to finite implicit bounds so IIS reports can explain restrictions that have no literal BOUNDS record.

### Continuous default lower bound

```text
ImplicitContinuousDefault {
    side: Lower,
    value: 0,
    variable_first_columns_span,
}
```

### INTORG defaults

```text
ImplicitIntegerMarkerDefault {
    side: Lower | Upper,
    value: 0 | 1,
    intorg_marker_span,
    variable_first_marked_columns_span,
}
```

If an explicit BOUNDS record overrides one side, only that side changes to explicit provenance. Retained implicit sides keep their synthetic origin.

Synthetic origins render explicitly as **format-derived defaults** and never fabricate a BOUNDS line.

Invariant:

> Every finite imported variable-bound restriction that Phase 29 can report resolves to exactly one explicit-or-synthetic MPS origin.

## 14. Error model

Errors are typed/source-aware. Representative categories include:

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
DuplicateRhsEntry
DuplicateRangeEntry
InvalidRangeForNRow
InvalidBound
InvalidRange
MissingRequiredSection
MissingEndata
UnknownVector
AmbiguousFormat
RepresentationError
ModelConstruction
```

Errors carry path/line/span/section/raw-field/entity context where available.

## 15. Differential qualification: HiGHS is oracle, not authority

HiGHS `readModel` is an independent implementation used to expose parser/semantic mistakes and interoperability differences.

Native path:

```text
file -> HiGHS readModel -> inspect/solve
```

ROML path:

```text
file -> MpsReader -> Model -> ROML compiler -> HiGHS -> inspect/solve
```

### Accepted P35 input mismatch policy

If ROML accepts a file but normalized semantics differ from HiGHS, production qualification stops until one disposition is approved:

1. `roml_bug_fixed` — ROML is corrected to the frozen/authoritative semantics;
2. `dialect_narrowed` — P35 is explicitly narrowed and now rejects that input;
3. `compatibility_exception` — authoritative MPS evidence supports ROML, divergence is documented, and owner review accepts it.

There is **no automatic "HiGHS wins" rule**.

### Strict-policy probe policy

For duplicate selected RHS/RANGE and selected RANGE-on-N, ROML's expected result is the frozen typed rejection. If HiGHS accepts, qualification records `intentional_roml_rejection` and native behavior. It does not relabel the ROML rejection as failure or silently change semantics.

## 16. Corpora and repository layout

Planned optional pinned submodules:

```text
testdata/corpora/infeasible-lps
    -> sk-surya/infeasiblelps
    -> 97a936498e5240d44adaf7dcfe84877fa34ce301

testdata/corpora/netlib
    -> sk-surya/lp-data-netlib
    -> 56257eea85b433ce6aa67d26156b36385318fd6f
```

Normal tests/package do not initialize or require them. External model blobs are not copied into the ROML crate.

## 17. Chinneck archive materialization security

The Chinneck repository stores collections in archives. Corpus extraction is test infrastructure but consumes untrusted entries.

Before writing any entry, the materializer must reject:

- absolute POSIX paths;
- Windows drive-qualified paths;
- UNC paths;
- normalized `..` traversal;
- symlink/hardlink entries;
- device/FIFO/socket/special entries;
- any destination not provably beneath a fresh extraction root.

Extraction must use a fresh temporary root and no-follow filesystem behavior. Partial trees are never promoted/reused. The completed cache is atomically promoted only after every entry and expected inventory check succeeds.

A blind archive extraction followed only by a post-hoc safety scan is not accepted.

## 18. Corpus roles

### Netlib

Primary purpose: broad feasible-LP parser/model/solve interoperability over diverse historical MPS encodings.

### Chinneck

Primary purpose: infeasible LP import, free/default-bound semantics, dense scaling, empty objectives, IIS provenance, and Phase 29 guarantee validation.

The Chinneck README explicitly notes many models have multiple IISs; exact Gurobi IIS membership is not a pass criterion.

## 19. Qualification tiers

```text
Tier 0: synthetic semantics + metamorphic + fuzz + archive-security fixtures
Tier 1: small corpus smoke on MPS/IIS-impacting PRs
Tier 2: scheduled broad Netlib + bounded Chinneck qualification
Tier 3: manual/release heavy IIS/performance characterization
```

No hosted-runner wall-clock threshold is a correctness gate.

## 20. P36 writer target

P36 writes models representable by the frozen linear P35 dialect.

Requirements:

- deterministic free MPS default;
- deterministic row/column/vector naming/order;
- one MPS row for one ROML linear constraint;
- RANGES for genuine ranged rows;
- negative objective-row RHS for objective constant;
- deterministic minimal BOUNDS/integrality encoding;
- typed failure for unrepresentable constructs;
- semantic, not textual, round trip;
- native HiGHS external round-trip qualification.

## 21. Security/robustness

Core reader requirements:

- checked counters/capacity arithmetic;
- no input-size recursion;
- configurable limits seam for line/rows/cols/nonzeros/records;
- no panic on malformed input;
- fuzzable lexical and end-to-end surfaces;
- no unsafe code in solver-free parser;
- large-corpus memory characterization.

Archive materialization uses the stricter rules in §17.

## 22. Decision summary

- D1: handwritten streaming parser; no LALRPOP.
- D2: `roml::io::mps`; parser not Model-owned.
- D3: lexical -> staging -> selected semantic resolution -> fresh Model.
- D4: fixed + free input with deterministic Auto.
- D5: linear LP/MILP only.
- D6: unsupported semantic extensions fail loudly.
- D7: one semantic ranged row remains one ROML constraint.
- D8: duplicate COLUMNS cells add algebraically.
- D9: duplicate selected RHS/RANGE records reject.
- D10: structural validation applies to all vectors; model semantics only to selected vectors.
- D11: selected RANGE-on-N rejects; unselected structurally valid RANGE-on-N is inert.
- D12: objective row is OBJNAME then first N; default minimize.
- D13: objective RHS uses negative-offset convention.
- D14: default continuous and marker domains are explicit.
- D15: implicit finite bounds receive synthetic provenance.
- D16: HiGHS is differential oracle, not normative authority.
- D17: accepted-input mismatch blocks until bug fix, dialect narrowing, or evidence-backed owner-approved compatibility exception.
- D18: corpora use optional pinned submodules to owner forks.
- D19: Chinneck archives are safely materialized; unsafe paths/links/special files reject before write.
- D20: IIS tests validate P29 guarantee/provenance, not exact solver IIS.
- D21: P36 write-back is designed now, implemented next.
- D22: default writer is deterministic free MPS.
- D23: round trip is semantic, not byte-preserving.
- D24: no generic interchange IR before second-format evidence.

## 23. Written-spec gate

The design branch contains no production parser/writer code, submodule gitlinks, workflow changes, or active roadmap routing changes.

Written-spec review round 1 raised four issues and this revision closes them in the binding semantic/requirements/test/corpus documents:

1. RANGE-on-N selected versus unselected validation;
2. explicit differential authority/disposition policy, including duplicate RHS;
3. synthetic provenance for implicit continuous and INTORG bounds;
4. archive path/link/special-file extraction safety.

Independent re-review is required before generating executable `35-PLAN.md`.