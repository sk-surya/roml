# Phase 35 MPS Semantics Reference

This file freezes the mathematical interpretation used by P35. It is an implementation contract, not a general claim that every historical MPS reader agrees on every extension.

Primary references used during design:

- MOSEK Optimizer API, "The MPS File Format" (11.2): standard/fixed/free layout, row/range/bound semantics, integer markers, duplicate coefficient interpretation.
- IBM CPLEX MPS documentation: first objective/rim-vector selection and negative RHS objective-offset convention.
- HiGHS documentation/C API: independent MPS `readModel` support used for qualification.

Where the original MPS ecosystem is historically ambiguous, ROML chooses one deterministic interpretation and differential-tests it.

## 1. Encoding and lines

P35 accepts ASCII MPS input. `LF` and `CRLF` line endings are accepted. A non-ASCII byte produces a typed encoding error in P35 rather than being silently reinterpreted.

A line whose first character is `*` is a comment and is ignored. Empty/whitespace-only lines are ignored where they do not split a fixed-field token.

The reader is streaming and reports one-based line numbers and one-based display columns/spans.

## 2. Section ordering

P35 recognizes the following linear sequence, allowing optional sections where noted:

```text
NAME       optional
OBJSENSE   optional
OBJNAME    optional
ROWS       required
COLUMNS    required
RHS        optional
RANGES     optional
BOUNDS     optional
ENDATA     required
```

`OBJSENSE` and `OBJNAME` are accepted in the documented pre-`ROWS` position. P35 does not accept them after matrix/rim sections in the first dialect.

`RHS`, `RANGES`, and `BOUNDS` may be omitted. Empty present sections are legal.

A recognized unsupported semantic section such as `QMATRIX`, `QSECTION`, `QUADOBJ`, `QCMATRIX`, `CSECTION`, `SOS`, or `INDICATORS` terminates parsing with `UnsupportedSection` even if the preceding linear data is otherwise valid.

Unknown section-like headers are errors, not comments.

## 3. Fixed lexical fields

P35 fixed-format parsing follows the conventional MPS field positions used by MOSEK documentation.

### ROWS

| Field | Start | Width | Meaning |
|---|---:|---:|---|
| row kind | 2 | 1 | `E`, `G`, `L`, `N` |
| row name | 5 | 8 | unique row identifier |

### COLUMNS / RHS / RANGES

| Field | Start | Width | Meaning |
|---|---:|---:|---|
| field 2 | 5 | 8 | column name or vector name |
| row 1 | 15 | 8 | row name |
| value 1 | 25 | 12 | numeric value |
| row 2 | 40 | 8 | optional row name |
| value 2 | 50 | 12 | optional numeric value |

For `COLUMNS`, field 2 is the column/variable name. For `RHS` and `RANGES`, field 2 is the vector name.

### BOUNDS

| Field | Start | Width | Meaning |
|---|---:|---:|---|
| bound kind | 2 | 2 | `FR`, `FX`, `LO`, `MI`, `PL`, `UP`, `BV`, `LI`, `UI` |
| vector name | 5 | 8 | bounds-vector name |
| variable name | 15 | 8 | target variable |
| value | 25 | 12 | optional/required depending on bound kind |

### Integer markers

Marker records occur inside `COLUMNS` using the same fixed fields. The row-name-like field contains `'MARKER'` and the later field contains `'INTORG'` or `'INTEND'`. The marker record is control metadata and contributes no matrix coefficient.

## 4. Free lexical format

Free MPS uses whitespace-separated fields with no blanks inside identifiers. It permits names longer than eight characters. Quoted marker tokens are recognized according to marker syntax rather than treated as ordinary identifiers.

P35 does not accept arbitrary quoted identifiers with embedded spaces in the first dialect.

## 5. Numeric syntax

P35 accepts finite decimal/scientific values compatible with ordinary MPS examples:

```text
12
-12
+12.5
.5
-.5
1e6
1E-6
-2.3e+08
```

Numeric fields that parse to NaN are rejected. Infinity must be expressed through bound/row semantics rather than numeric `inf`/`infinity` tokens in P35.

The reader stores values as `f64` after syntax validation. ROML numeric/domain validation remains authoritative during model construction.

## 6. Row declarations

Each `ROWS` record declares a unique row name and one kind:

| Kind | Base activity lower | Base activity upper |
|---|---:|---:|
| `E` | RHS | RHS |
| `G` | RHS | `+inf` |
| `L` | `-inf` | RHS |
| `N` | `-inf` | `+inf` |

When an RHS entry is omitted for `E/G/L`, its RHS value is zero.

`N` rows are nonconstraint rows. One may supply the objective coefficient vector.

Duplicate row names are errors.

## 7. Objective selection and sense

Objective row selection:

1. if `OBJNAME` is present, it must name an existing `N` row and that row is selected;
2. otherwise the first `N` row in declaration order is selected;
3. if there is no `N` row, the ROML model receives a zero objective.

Nonselected `N` rows do not become constraints. They remain import metadata only.

Objective sense:

- `MIN` or `MINIMIZE` -> minimize;
- `MAX` or `MAXIMIZE` -> maximize;
- absent `OBJSENSE` -> minimize.

Multiple sense records or invalid sense text are errors.

## 8. COLUMNS and matrix coefficients

A COLUMNS data record contains one column name and one or two `(row,value)` pairs.

All referenced row names must have been declared in `ROWS`.

Entries targeting the selected objective row contribute to the objective coefficient for that variable. Entries targeting `E/G/L` rows contribute to the constraint matrix. Entries targeting nonselected `N` rows are preserved only as nonselected free-row metadata and do not affect the ROML model.

A variable exists if it has at least one ordinary COLUMNS data record. Marker records do not create variables.

If the same mathematical matrix/objective entry occurs more than once, values are added algebraically. This applies equally to repeated objective-row coefficients. Exact zero after accumulation is omitted from the canonical ROML coefficient cell.

P35 accepts repeated column blocks even though conventional MPS recommends grouping all entries of one variable together; the staging finalizer merges them deterministically. A strict-grouping warning option may be added later but is not required for semantic correctness.

## 9. RHS vectors

Multiple RHS vector names may appear. P35 staging preserves all of them.

Selection is controlled by `MpsVectorSelection`:

- `First`: first RHS vector name encountered in file order;
- `Named(name)`: exact named vector or typed missing-vector error;
- `None`: behave as if no RHS entries were supplied.

Default selection is `First`. If no RHS vector exists, every row RHS defaults to zero.

For `E/G/L` rows, selected RHS values establish the base row value described above.

For the selected objective row, an RHS value `r` sets the ROML objective constant to `-r`.

RHS values on nonselected `N` rows do not affect the ROML objective.

If an RHS vector specifies the same row multiple times, P35 adds those entries algebraically, matching the matrix-duplicate policy. This is a ROML deterministic interpretation and receives differential fixtures.

## 10. RANGES vectors

Multiple RANGES vectors are staged and one is selected with the same `First` / `Named` / `None` policy. Default is `First`.

Let selected RHS value be `b` and selected range value be `r`.

The resolved constraint bounds are:

| Row kind | Condition | Lower | Upper |
|---|---|---:|---:|
| `E` | `r >= 0` | `b` | `b + r` |
| `E` | `r < 0` | `b + r` | `b` |
| `G` | any sign | `b` | `b + abs(r)` |
| `L` | any sign | `b - abs(r)` | `b` |

A RANGE entry for an `N` row is rejected in P35 as semantically invalid for the supported dialect rather than silently ignored.

A ranged row is emitted into ROML as one ranged semantic constraint with both finite bounds where appropriate.

Repeated selected RANGE entries for the same row are rejected as ambiguous rather than accumulated. RANGE values are transformations, not sparse linear coefficients.

## 11. Variable defaults before BOUNDS

For an ordinary continuous variable:

```text
lower = 0
upper = +inf
kind  = continuous
```

For a variable whose COLUMNS entries occur inside an active `INTORG`/`INTEND` region:

```text
lower = 0
upper = 1
kind  = integer
```

The integer-marker default of `[0,1]` is a compatibility behavior documented by MOSEK and is deliberately reproduced.

If a variable appears in both integer-marked and ordinary COLUMNS blocks, it remains integer. Conflicting marker structure itself is an error; repeated appearances do not toggle the variable back to continuous.

## 12. Integer markers

Marker state begins outside an integer region.

- `'INTORG'` while already inside an integer region -> `InvalidMarkerNesting`.
- `'INTEND'` while outside -> `InvalidMarkerNesting`.
- reaching the end of `COLUMNS`/file while still inside -> `InvalidMarkerNesting`.
- multiple disjoint properly paired integer regions are accepted.

Each variable with any data record encountered while marker state is active is marked integer and receives the marker default bounds unless selected BOUNDS records modify them.

## 13. BOUNDS vectors

Multiple BOUNDS vectors are staged and one is selected. Default selection is `First`. If no BOUNDS vector exists, the domains from section 11 remain.

Selected bound records are applied in file order to the current domain. "unchanged" below means the current value from defaults/markers/prior selected records remains.

| Kind | Lower | Upper | Makes integer? | Value required? |
|---|---:|---:|---|---|
| `FR` | `-inf` | `+inf` | no | no |
| `FX` | `v` | `v` | no | yes |
| `LO` | `v` | unchanged | no | yes |
| `MI` | `-inf` | unchanged | no | no |
| `PL` | unchanged | `+inf` | no | no |
| `UP` | unchanged | `v` | no | yes |
| `BV` | `0` | `1` | yes, binary | no |
| `LI` | `ceil(v)` | unchanged | yes | yes |
| `UI` | unchanged | `floor(v)` | yes | yes |

For `LI/UI`, the rounding behavior follows the MOSEK documented interpretation and avoids creating a fractional integer bound.

After all selected records for a variable are applied, `lower <= upper` must hold and ROML domain validation must succeed.

A `BV` variable is binary when final bounds remain `[0,1]`. Subsequent selected records that broaden it beyond `[0,1]` are rejected as a conflicting binary-domain representation in P35 rather than silently converting it to a general integer. General integer variables should use marker/`LI`/`UI` semantics.

A continuous `FX/LO/UP/FR/MI/PL` record does not remove integrality already established by a marker or integer bound record.

## 14. Named vector identity

Vector names are compared exactly under ASCII byte equality. P35 does not case-fold row, variable, vector, or objective names.

Records for one vector may be interleaved with records for another vector inside the same rim section; staging groups them by vector name while preserving first-seen vector order and per-vector record order.

## 15. Section multiplicity

P35 accepts at most one occurrence of each section header in the first dialect. Multiple named vectors live inside a single RHS/RANGES/BOUNDS section. Repeated section headers are `InvalidSectionOrder`/duplicate-section errors.

## 16. ENDATA and trailing input

`ENDATA` terminates the model. After `ENDATA`, only blank/comment lines are accepted. Additional data/section records after `ENDATA` are errors.

Missing `ENDATA` is an error in P35 even if EOF otherwise follows a complete BOUNDS/RANGES/RHS/COLUMNS section. This makes truncation visible.

## 17. Auto fixed/free detection

Before layout is locked, a meaningful record is interpreted under both fixed and free lexical rules.

- one valid interpretation -> lock to that layout;
- both valid and semantically identical -> accept and remain undecided;
- both valid but semantically different -> `AmbiguousFormat`;
- neither valid -> section-aware parse error.

Once locked, the layout does not change within the file.

Section headers are recognized independently enough to establish section state before data-line dual parsing.

## 18. Normalization versus source preservation

The imported ROML model is mathematical/canonical, not a textual AST. P35 may normalize:

- duplicate coefficient entries into one coefficient;
- explicit zero RHS to default zero;
- equivalent bound sequences to one final domain;
- fixed/free lexical differences;
- row/column record grouping.

`MpsMetadata` and `MpsSourceMap` preserve enough record provenance for diagnostics and IIS mapping, but P35 does not preserve comments/spacing for P36 byte-for-byte replay.

## 19. P36 writer semantic target

P36 writes models that are representable by the P35 linear dialect. Its canonical free-format output shall, when read by P35, reconstruct the same mathematical ROML snapshot modulo model revision/instance identity and nonsemantic source metadata.

Writer rules shall be derived from this file in reverse:

- one selected objective row;
- deterministic generated vector names;
- one row per ROML linear constraint;
- RANGES only for genuine ranged rows;
- minimal deterministic BOUNDS records;
- integer/binary representation chosen consistently;
- objective constant encoded as negative RHS on objective row;
- stable row/column ordering from ROML canonical ordering, never hash iteration.

## 20. Required differential probes before implementation closes

Because MPS has historical reader variation, the following selected semantics must be checked against native HiGHS on synthetic files before P35 claims compatibility:

1. objective-row RHS sign;
2. duplicate objective/matrix entries;
3. all four RANGE cases;
4. marker-default `[0,1]`;
5. marker + explicit LO/UP bounds;
6. `LI/UI` rounding;
7. `FR`, `MI`, and `PL` sequences;
8. multiple named rim vectors and first-vector selection;
9. repeated RHS same-row policy;
10. long-name free format.

If native HiGHS differs from this contract, the implementation review must determine whether ROML keeps the documented contract and records an intentional difference or amends this semantics file before production merge. No discrepancy may be silently accepted.
