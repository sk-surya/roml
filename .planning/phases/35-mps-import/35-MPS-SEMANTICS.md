# Phase 35 MPS Semantics Reference

This file freezes the mathematical interpretation used by P35. It is an implementation contract, not a general claim that every historical MPS reader agrees on every extension.

Primary references used during design:

- MOSEK, "The MPS File Format": standard/fixed/free layout, row/range/bound semantics, integer markers, and additive duplicate COLUMNS entries.
- IBM CPLEX MPS documentation: first objective/rim-vector selection and negative RHS objective-offset convention.
- HiGHS documentation/C API: independent MPS `readModel` support used for differential qualification.

Where the MPS ecosystem is historically ambiguous, ROML chooses one deterministic interpretation in this file. **This file is normative for P35. HiGHS is evidence, not semantic authority.**

## 1. Validation model: structural for all vectors, semantic for selected vectors

MPS may contain multiple named RHS, RANGES, and BOUNDS vectors. P35 deliberately separates two validation layers.

### 1.1 Whole-document structural validation

Every staged record, selected or not, must be structurally valid:

- lexical layout is valid;
- record/bound kind is recognized in the P35 dialect;
- required numeric fields parse and are finite;
- COLUMNS/RHS/RANGES row references name declared ROWS entries;
- BOUNDS targets name variables declared by ordinary COLUMNS data;
- section order and marker nesting are valid.

A malformed numeric value or unknown row/variable in an **unselected** vector still makes the file invalid.

### 1.2 Selected-vector semantic validation

Only the selected RHS/RANGES/BOUNDS vector contributes mathematical semantics and only that selected vector is checked for combinations whose validity depends on applying the vector to the model.

Consequences:

- an unselected RANGES vector may contain a syntactically/reference-valid entry targeting an `N` row; it is staged and inert;
- selecting that vector fails with `InvalidRange`/the dedicated `N`-row RANGE error;
- an unselected BOUNDS vector may contain a sequence that would resolve to an impossible final domain; it is staged and inert;
- selecting that BOUNDS vector performs the ordered transitions and then fails domain validation;
- duplicate same-row RHS/RANGE entries are detected as selected-vector semantic ambiguity when that vector is selected.

This rule applies uniformly to default `First`, explicit `Named`, and `None` selections.

## 2. Encoding and lines

P35 accepts ASCII MPS input. `LF` and `CRLF` line endings are accepted. A non-ASCII byte produces a typed encoding error rather than silent reinterpretation.

A line whose first character is `*` is a comment and is ignored. Empty/whitespace-only lines are ignored where legal. The reader reports one-based logical line numbers and one-based display columns/spans.

## 3. Section ordering

P35 recognizes the following linear sequence:

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

`OBJSENSE` and `OBJNAME` are accepted in the documented pre-`ROWS` position. `RHS`, `RANGES`, and `BOUNDS` may be omitted and may be present but empty.

For compatibility with historical fixed-format corpora, `NAME` may carry a
description after its first name token, and fixed-format RHS/RANGES/BOUNDS
records may omit the vector name. An omitted fixed-field vector name belongs
to the first synthetic empty-name vector in that section; free-format records
still require the vector name field.

A recognized unsupported semantic section such as `QMATRIX`, `QSECTION`, `QUADOBJ`, `QCMATRIX`, `CSECTION`, `SOS`, or `INDICATORS` terminates parsing with `UnsupportedSection`, even if the preceding linear data is otherwise valid. Unknown section-like headers are errors.

P35 accepts at most one occurrence of each section header. Multiple named vectors live inside one RHS/RANGES/BOUNDS section.

## 4. Fixed lexical fields

P35 fixed-format parsing follows conventional MPS field positions.

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

For `COLUMNS`, field 2 is the variable name. For `RHS` and `RANGES`, it is the vector name.

### BOUNDS

| Field | Start | Width | Meaning |
|---|---:|---:|---|
| bound kind | 2 | 2 | `FR`, `FX`, `LO`, `MI`, `PL`, `UP`, `BV`, `LI`, `UI` |
| vector name | 5 | 8 | bounds-vector name |
| variable name | 15 | 8 | target variable |
| value | 25 | 12 | optional/required by kind |

### Integer markers

Marker records occur inside `COLUMNS`. The marker token is `'MARKER'` and the control value is `'INTORG'` or `'INTEND'`. Marker records contribute no matrix coefficient and create no variable.

## 5. Free lexical format

Free MPS uses whitespace-separated fields with no blanks inside identifiers and permits names longer than eight characters. Quoted marker tokens are recognized as marker syntax. P35 does not accept arbitrary quoted identifiers containing spaces in the first dialect.

## 6. Numeric syntax

P35 accepts finite decimal/scientific values such as:

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

NaN is rejected. Infinity is expressed through row/bound semantics rather than numeric `inf`/`infinity` tokens. Values are stored as `f64` after syntax validation; ROML domain validation remains authoritative during model construction.

## 7. Row declarations

Each `ROWS` record declares a unique row and one kind:

| Kind | Base lower | Base upper |
|---|---:|---:|
| `E` | RHS | RHS |
| `G` | RHS | `+inf` |
| `L` | `-inf` | RHS |
| `N` | `-inf` | `+inf` |

When selected RHS omits an `E/G/L` row, its RHS defaults to zero. `N` rows are nonconstraint rows. Duplicate row names are errors.

## 8. Objective selection and sense

Objective selection:

1. if `OBJNAME` is present, it must name an existing `N` row and that row is selected;
2. otherwise the first `N` row is selected;
3. if there is no `N` row, ROML receives a zero objective.

Nonselected `N` rows do not become constraints. Their COLUMNS/RHS data may be retained in import metadata but have no model effect.

Objective sense:

- `MIN` / `MINIMIZE` -> minimize;
- `MAX` / `MAXIMIZE` -> maximize;
- absent `OBJSENSE` -> minimize.

Multiple sense records or invalid sense text are errors.

## 9. COLUMNS and duplicate coefficients

A COLUMNS data record contains one variable name and one or two `(row,value)` pairs. Every referenced row must have been declared.

- selected objective row -> objective coefficient;
- `E/G/L` row -> constraint matrix coefficient;
- nonselected `N` row -> inert metadata.

A variable exists if it has at least one ordinary COLUMNS data record. If the same matrix/objective cell occurs multiple times, values are **added algebraically**. Exact zero after accumulation is omitted according to ROML canonical coefficient rules.

P35 accepts repeated column blocks even though strict historical MPS expects one variable's elements to be grouped. Staging merges them deterministically.

**The additive duplicate rule is specific to COLUMNS/objective coefficient cells. It does not imply additive RHS or RANGE semantics.** MOSEK explicitly documents additive duplicate matrix entries; P35 does not extrapolate that rule to other sections.

## 10. RHS vectors

Multiple RHS vectors are staged. Selection uses `First`, `Named(name)`, or `None`; default is `First`. If no vector is selected/present, row RHS values default to zero.

For the selected RHS vector:

- `E/G/L` entries define the base RHS value;
- selected objective-row entry `r` produces objective constant `-r`;
- entries on nonselected `N` rows have no model effect.

### 10.1 Duplicate selected RHS entries

If the **selected RHS vector** specifies the same row more than once, P35 rejects the vector with a typed duplicate/ambiguity error. Values are **not summed, first-wins, or last-wins**.

If a nonselected RHS vector contains repeated same-row entries, the records may remain staged because structural syntax/references are valid. Selecting that vector causes the duplicate error.

This strict rule is intentional because the cited MPS reference defines RHS assignment but does not define duplicate RHS accumulation.

## 11. RANGES vectors

Multiple RANGES vectors are staged; one is selected with `First`, `Named`, or `None`. Let selected RHS value be `b` and selected range value be `r`.

| Row kind | Condition | Lower | Upper |
|---|---|---:|---:|
| `E` | `r >= 0` | `b` | `b + r` |
| `E` | `r < 0` | `b + r` | `b` |
| `G` | any sign | `b` | `b + abs(r)` |
| `L` | any sign | `b - abs(r)` | `b` |

A ranged row becomes **one** ranged ROML semantic constraint.

### 11.1 RANGE on `N`

The standard RANGE transformation is defined for constraint rows; the MOSEK reference leaves `N` without a RANGE transformation. Therefore:

- a RANGE entry targeting `N` in the **selected** RANGES vector is a typed semantic error;
- the same syntactically/reference-valid record in an **unselected** vector is staged but inert;
- explicitly selecting that vector produces the same typed semantic error.

### 11.2 Duplicate selected RANGE entries

If the selected RANGES vector specifies the same row more than once, P35 rejects it as ambiguous. RANGE entries are transformations, not sparse matrix coefficients, and are never algebraically accumulated.

## 12. Variable defaults before selected BOUNDS

Ordinary continuous variable:

```text
lower = 0
upper = +inf
kind  = continuous
```

Variable appearing while `INTORG` is active:

```text
lower = 0
upper = 1
kind  = integer
```

MOSEK documents the legacy marker default `[0,1]`. If a variable appears in both marked and ordinary COLUMNS records, integer status dominates; it does not toggle back to continuous.

## 13. Integer markers

Marker state begins outside an integer region.

- `INTORG` while inside -> `InvalidMarkerNesting`;
- `INTEND` while outside -> `InvalidMarkerNesting`;
- leaving COLUMNS/EOF while inside -> `InvalidMarkerNesting`;
- multiple disjoint balanced regions are accepted.

Every variable with any ordinary data record while the marker is active is marked integer and receives the marker default before selected BOUNDS transitions.

## 14. BOUNDS vectors

Multiple BOUNDS vectors are staged; one is selected. Default is `First`. If no selected BOUNDS exists, section 12 defaults remain.

Only the selected BOUNDS vector is executed as a domain state machine. Records apply in file order:

| Kind | Lower | Upper | Makes integer? | Value required? |
|---|---:|---:|---|---|
| `FR` | `-inf` | `+inf` | no | no |
| `FX` | `v` | `v` | no | yes |
| `LO` | `v` | unchanged | no | yes |
| `MI` | `-inf` | unchanged | no | no |
| `PL` | unchanged | `+inf` | no | no |
| `UP` | unchanged | `v` | no | yes |
| `BV` | `0` | `1` | binary | no |
| `LI` | `ceil(v)` | unchanged | yes | yes |
| `UI` | unchanged | `floor(v)` | yes | yes |

`LI/UI` rounding follows the MOSEK interpretation. After all selected records, `lower <= upper` and ROML domain validation must succeed.

A `BV` variable remains binary only if final bounds remain inside `[0,1]`; a later selected transition that broadens it outside binary bounds is a typed conflict under P35. Ordinary bound records do not remove integrality established by markers/LI/UI.

Repeated BOUNDS records are **not** governed by the RHS/RANGE duplicate-rejection rule: BOUNDS is an ordered transition stream, so repeated records execute in file order under this table.

An unselected BOUNDS vector is structurally validated but its final domain is not resolved. If it would be invalid, that matters only when selected.

## 15. Named vector identity and selection

Vector names are compared by exact ASCII byte equality. P35 does not case-fold row, variable, vector, or objective names.

Records for vectors may be interleaved within a rim section; staging groups by vector name while preserving first-seen vector order and per-vector record order.

Selection:

```text
First       -> first vector name encountered
Named(name) -> exact named vector or typed missing-vector error
None        -> no entries of that class affect the model
```

## 16. ENDATA and auto layout

`ENDATA` terminates the model. After it, only blank/comment lines are accepted. Missing `ENDATA` is an error.

Before fixed/free layout is locked:

- one valid interpretation -> lock it;
- both valid and semantically identical -> accept, remain undecided;
- both valid but different -> `AmbiguousFormat`;
- neither valid -> section-aware parse error.

After lock, later records use only that layout.

## 17. Explicit and synthetic source provenance

The imported model is mathematical/canonical, not a textual AST. `MpsSourceMap` must nevertheless provide a provenance origin for every finite imported bound that can appear as a Phase 29 restriction.

### 17.1 Explicit bound records

A bound produced or last modified by a selected explicit BOUNDS record retains the exact record `SourceSpan` and bound kind.

### 17.2 Implicit continuous default

If an ordinary continuous variable retains the MPS default finite lower bound `0`, its lower-bound provenance is synthetic:

```text
ImplicitContinuousDefault {
    side: Lower,
    value: 0,
    variable_first_columns_span,
}
```

The first ordinary COLUMNS record is an anchor showing where the variable entered the MPS model. The provenance explicitly states that no BOUNDS line supplied the value.

The default `+inf` upper side is metadata but is not a finite IIS restriction.

### 17.3 Implicit INTORG defaults

If an integer-marked variable retains either marker default, provenance is synthetic:

```text
ImplicitIntegerMarkerDefault {
    side: Lower | Upper,
    value: 0 | 1,
    intorg_marker_span,
    variable_first_marked_columns_span,
}
```

If an explicit selected BOUNDS record overrides one side, that side uses the explicit record while any retained marker-default side keeps the synthetic marker origin.

### 17.4 Rendering rule

Synthetic provenance must render as a format-derived default with source anchors; it must never fabricate a BOUNDS line number. IIS acceptance requires every finite variable-bound member to resolve to exactly one explicit-or-synthetic MPS origin.

Row provenance analogously retains row declaration plus selected RHS/RANGE provenance/default metadata as useful, but the critical P35 blocker is finite variable-bound origin completeness.

## 18. Normalization versus source preservation

P35 may normalize:

- duplicate COLUMNS coefficient entries into one coefficient;
- explicit zero selected RHS to semantic zero;
- equivalent selected bound transition sequences to one final domain;
- fixed/free lexical differences;
- row/column record grouping.

It does **not** normalize duplicate selected RHS or RANGE entries into a value; those are errors.

`MpsMetadata` and `MpsSourceMap` preserve enough provenance for diagnostics/IIS mapping but do not preserve comments/spacing for textual replay.

## 19. P36 writer semantic target

P36 writes models representable by this linear dialect. Reading P36 canonical free-format output with P35 must reconstruct the same mathematical ROML snapshot modulo model identity/revision and nonsemantic source metadata.

Writer reverse rules include:

- one selected objective row;
- deterministic generated rim-vector names;
- one row per ROML linear constraint;
- RANGES only for genuine ranged rows;
- minimal deterministic BOUNDS records;
- deterministic integrality representation;
- objective constant encoded as negative RHS on objective row;
- stable ROML canonical ordering, never hash iteration.

## 20. Differential authority and disposition policy

The compatibility target is not "whatever HiGHS does." The authority order is:

1. this frozen P35 semantics reference;
2. cited authoritative MPS documentation supporting each frozen standard behavior;
3. explicit strict ROML policy for behavior the historical format does not define sufficiently.

HiGHS `readModel` is an independent implementation used to discover mistakes and interoperability differences.

### 20.1 Accepted P35 inputs

For an input P35 accepts, normalized ROML and HiGHS semantics are expected to match. A mismatch is a **production-merge blocker** until one reviewed disposition is recorded:

- `roml_bug_fixed`: ROML changes to the frozen/authoritative semantics;
- `dialect_narrowed`: the input is removed from the accepted P35 dialect and ROML now rejects it explicitly;
- `compatibility_exception`: authoritative evidence supports ROML's semantics, the difference is documented in support metadata, and owner review approves the exception.

There is no rule that automatically changes ROML to match HiGHS.

### 20.2 Intentional strict rejections

For frozen strict-policy inputs such as duplicate selected RHS, duplicate selected RANGE, or selected RANGE-on-`N`, ROML is expected to reject. If HiGHS accepts such a file, the differential harness records `intentional_roml_rejection` plus native behavior. That is not semantic equivalence and must not be reported as a pass of the accepted-input theorem; it is compatibility telemetry for an input outside ROML's accepted strict subset.

### 20.3 Required probes

Accepted-input probes:

1. objective-row RHS sign;
2. duplicate COLUMNS objective/matrix entries;
3. all four RANGE cases;
4. marker-default `[0,1]`;
5. marker + explicit LO/UP;
6. `LI/UI` rounding;
7. `FR`, `MI`, `PL`, `FX` sequences;
8. multiple named rim vectors and first-vector selection;
9. long-name free format;
10. empty objective.

Strict-policy probes:

11. duplicate same-row selected RHS;
12. duplicate same-row selected RANGE;
13. selected RANGE on `N`.

Every observed divergence receives one of the explicit classifications above. No discrepancy may be silently accepted.

## 21. Normative notes tied to the cited MOSEK reference

The current MOSEK MPS reference explicitly documents:

- `E/G/L/N` row meaning and first-`N` objective behavior;
- standard RANGE transformations for `E/G/L`, while `N` has no transformation entry;
- integer-marker default lower `0` and upper `1`;
- multiple disjoint integer marker sections;
- additive repeated **COLUMNS matrix** elements;
- fixed and free MPS layouts.

Those observations motivate, but do not replace, the precise ROML policy frozen above.
