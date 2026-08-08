# Phase 35 Ultra Design Packet — MPS Import, Differential Qualification, and P36 Write-Back Seam

**Status:** architecture approved in discussion; written-spec review round 1 addressed; re-review pending  
**Baseline:** `main@ff99389b9bf1318c555dc1f72dff6b5c7a4111c0`  
**Design:** `docs/superpowers/specs/2026-08-07-mps-io-design.md`  
**Primary semantic authority:** `35-MPS-SEMANTICS.md`  
**Requirements:** `35-REQUIREMENTS.md`  
**Verification contract:** `35-TEST-MATRIX.md`  
**External qualification:** `35-CORPUS-QUALIFICATION.md`  
**Risk/stop authority:** `35-RISKS.md`  
**Binding decisions:** `35-DECISIONS.md`

## Executive intent

Build a production-quality, solver-independent MPS I/O foundation in ROML so real LP/MILP models can be loaded, solved, diagnosed with Phase 29 IIS, and compared with an independent mature MPS reader. P35 implements import and qualification. P36 implements deterministic write-back and semantic round-trip on the same frozen linear semantics.

P35 is not a disposable parser for one IIS corpus. It is the first durable member of `roml::io`.

## 1. Architectural theorem

For an MPS file `f` and read options `o`, define:

```text
R(f,o) = canonical ROML model produced by the P35 reader
H(f)   = mathematical model produced independently by HiGHS readModel
```

### 1.1 Accepted P35 inputs

For every input **accepted by the declared P35 dialect**, the qualification target is:

```text
normalize(semantics(R(f,o))) == normalize(semantics(H(f)))
```

for the selected objective and selected rim vectors, subject to explicit numeric comparison tolerances.

A mismatch does **not** mean "follow HiGHS." It blocks production qualification until exactly one reviewed disposition is recorded:

```text
roml_bug_fixed
    ROML was wrong relative to the frozen/authoritative contract and is corrected

dialect_narrowed
    ROML deliberately removes that input from its accepted dialect and rejects it

compatibility_exception
    authoritative MPS evidence supports ROML's interpretation;
    the divergence is documented and owner review explicitly accepts it
```

There is no automatic HiGHS-wins policy.

### 1.2 Deliberate strict rejections

Some historically ambiguous inputs are intentionally outside ROML's accepted strict subset even if HiGHS accepts them. Examples frozen in P35:

- duplicate same-row records in the **selected RHS vector**;
- duplicate same-row records in the **selected RANGES vector**;
- a RANGE entry targeting an `N` row in the **selected RANGES vector**.

For these probes:

```text
ROML = typed intentional rejection
HiGHS = observed compatibility telemetry
```

HiGHS acceptance is recorded as `intentional_roml_rejection`; it does not redefine P35 semantics.

### 1.3 IIS theorem

For an accepted imported infeasible model:

```text
Phase29(R(f,o)) reports only guarantees independently verified by P29
AND
all reported row/bound members resolve to explicit-or-synthetic MPS provenance
```

The IIS theorem never requires equality with one solver's particular IIS.

### 1.4 P36 theorem

For every ROML linear LP/MILP model representable by the P36 MPS dialect:

```text
semantics(P35_read(P36_write(model))) == semantics(model)
```

and selected external round trips must remain structurally/solve equivalent when read by native HiGHS.

## 2. Phase split

### P35 — reader + qualification

P35 delivers:

- handwritten streaming MPS reader;
- fixed and free lexical layouts;
- deterministic `Auto` detection;
- linear LP/MILP semantics only;
- MPS-native staging before ROML construction;
- selected-vector semantic resolution;
- typed source-aware errors;
- explicit and synthetic import provenance;
- synthetic/metamorphic/fuzz qualification;
- independent HiGHS differential harness;
- pinned Netlib and Chinneck corpus qualification;
- `MPS -> ROML -> HiGHS -> IIS` end-to-end examples/evidence.

### P36 — writer + round trip

P36 reuses the semantic contract and adds:

- representability analysis;
- deterministic free-format MPS output by default;
- strict optional fixed-format output only if lossless;
- typed representation errors for unrepresentable ROML models;
- `Model -> MPS -> Model` semantic round trips;
- external `MPS -> ROML -> MPS -> HiGHS` qualification;
- deterministic output suitable for diffs.

No P36 production writer code lands in P35.

## 3. Parser-generator decision

**No LALRPOP.** P35 uses a handwritten streaming section/state parser.

MPS complexity is dominated by:

- line-oriented fixed/free lexical layouts;
- section state;
- `INTORG`/`INTEND` state;
- named alternative rim vectors;
- duplicate coefficient accumulation;
- source spans and diagnostics.

An LR grammar adds machinery without simplifying those semantics. No parser-combinator dependency is added initially either; small lexical helpers use ordinary Rust. A future genuinely grammar-dominated file format may choose differently.

## 4. Module boundary

Target namespace:

```text
roml::io
└── mps
    ├── reader
    ├── writer        # P36
    ├── options
    ├── error
    ├── metadata
    └── semantic      # private MPS staging/resolution
```

`Model` does not own file parsing. Primary API shape:

```rust
let imported = MpsReader::new().read_path("model.mps")?;
let model = imported.model;
```

and:

```rust
let imported = MpsReader::with_options(options).read(reader)?;
```

P36 mirrors this with `MpsWriter`; a convenience `Model` wrapper may exist later only as a thin facade.

## 5. Read pipeline

```text
BufRead / path
     |
     v
lexical line parser
 fixed/free/auto
 source spans
     |
     v
record + section state machine
     |
     v
MPS staging document
 all named vectors retained
     |
     +--> whole-document structural validation
     |
     v
select objective + RHS/RANGES/BOUNDS
     |
     +--> selected-vector semantic validation
     |
     v
fresh transactional ROML Model
     |
     +--> MpsMetadata + MpsSourceMap
```

No live ROML `Model` is mutated while the file remains structurally unresolved.

## 6. Supported P35 dialect

Supported sections/records:

```text
NAME
OBJSENSE
OBJNAME
ROWS: E G L N
COLUMNS
RHS
RANGES
BOUNDS: FR FX LO MI PL UP BV LI UI
INTORG / INTEND markers
ENDATA
fixed MPS
free MPS
linear LP
linear MILP
```

Explicitly unsupported in P35:

```text
QSECTION
QMATRIX
QUADOBJ
QCMATRIX
CSECTION
SOS
INDICATORS
PWLOBJ
LAZYCONS
USERCUTS
SC / SI
unqualified vendor semantic extensions
```

Unsupported semantic sections are hard errors, never warnings or silent omissions.

## 7. Two-layer rim-vector validation

This is a binding distinction introduced by written-spec review.

### 7.1 Structural validation applies to every staged vector

For all RHS/RANGES/BOUNDS records, selected or not:

- syntax/field shape must be valid;
- numeric fields must parse and be finite;
- referenced rows/variables must exist;
- record kinds must be supported;
- section/order rules remain valid.

Thus an unknown row in an unused RHS vector still rejects the file.

### 7.2 Model-semantic validation applies only to the selected vector

Only the selected vector of each class affects the model and is validated for semantics that arise from applying that alternative vector.

Examples:

```text
unselected RANGES vector contains RANGE on N
    -> stage successfully; inert

select that RANGES vector
    -> typed InvalidRangeForNRow

unselected BOUNDS vector would finish lower > upper
    -> stage successfully; inert

select that BOUNDS vector
    -> typed domain error
```

This preserves the purpose of multiple alternative rim vectors without permitting malformed syntax/references to hide in unused data.

## 8. Objective semantics

Selection:

```text
OBJNAME present -> referenced N row
else first N row
else zero objective
```

Absent `OBJSENSE` means minimize.

For selected objective row:

```text
RHS value r -> objective constant = -r
```

Nonselected `N` rows are not constraints and do not affect the ROML objective.

## 9. Matrix and duplicate policy

Staging is column-oriented, not `HashMap<(row,col),...>`.

Duplicate COLUMNS cells are summed algebraically, including duplicate selected-objective coefficient cells. Exact zero after accumulation follows ROML canonical coefficient normalization.

P35 accepts repeated column blocks and merges them by variable name.

**The additive rule stops at COLUMNS.**

For selected vectors:

```text
duplicate RHS same row   -> typed ambiguity error
duplicate RANGE same row -> typed ambiguity error
```

They are not summed, overwritten, or resolved by first/last wins.

BOUNDS is different: selected BOUNDS records are an ordered state-transition stream and execute in file order under the frozen transition table.

## 10. RANGES

Let selected RHS be `b`, selected range be `r`:

```text
E, r >= 0 : b       <= a'x <= b + r
E, r <  0 : b + r   <= a'x <= b
G           : b       <= a'x <= b + |r|
L           : b-|r|   <= a'x <= b
```

One MPS ranged row remains one ROML ranged semantic constraint.

The cited MOSEK table defines E/G/L transformations and leaves `N` without a transformation. P35 therefore rejects selected RANGE-on-`N`. The same record in an unselected vector is structurally valid metadata until that vector is selected.

## 11. Variable domains

Defaults:

```text
ordinary continuous:
    Continuous [0,+inf]

INTORG variable before selected BOUNDS transitions:
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

Transitions apply in file order. Ordinary bound records do not erase integrality. Current ROML supports unbounded general integer variables, so `INTORG + FR -> Integer [-inf,+inf]` is required behavior.

## 12. Provenance model, including implicit bounds

`MpsSourceMap` remains outside canonical `Model` state but must fully explain finite imported restrictions used by IIS.

### Explicit origins

Selected BOUNDS records preserve exact source spans and bound kind.

### Continuous implicit default

If a continuous variable retains its default finite lower bound `0`:

```text
ImplicitContinuousDefault {
    side: Lower,
    value: 0,
    variable_first_columns_span,
}
```

The COLUMNS span anchors variable introduction; rendering states clearly that the value is an MPS default, not an explicit BOUNDS line.

### INTORG implicit defaults

If a marked integer variable retains marker-default lower/upper:

```text
ImplicitIntegerMarkerDefault {
    side: Lower | Upper,
    value: 0 | 1,
    intorg_marker_span,
    variable_first_marked_columns_span,
}
```

If an explicit bound overrides only one side, the other side retains its synthetic marker provenance.

### Completeness invariant

Every **finite imported variable-bound restriction** that Phase 29 can report must resolve to exactly one explicit-or-synthetic MPS origin. Synthetic origins never fabricate source lines.

## 13. Errors

Errors are typed and source-aware. Representative categories:

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

Source context includes path, line, span, section, raw field/record, and entity where available.

## 14. Fixed/free detection

`MpsFormat::{Auto,Fixed,Free}`.

Auto behavior before lock:

```text
only fixed succeeds -> lock Fixed
only free succeeds  -> lock Free
both same semantics -> accept, remain undecided
both differ         -> AmbiguousFormat
neither             -> best section-aware error
```

After lock, mixed layout is rejected in P35.

## 15. External corpora

Planned optional pinned submodules:

```text
testdata/corpora/infeasible-lps
    -> sk-surya/infeasiblelps
    -> 97a936498e5240d44adaf7dcfe84877fa34ce301

testdata/corpora/netlib
    -> sk-surya/lp-data-netlib
    -> 56257eea85b433ce6aa67d26156b36385318fd6f
```

Normal tests/package do not require submodules. No corpus files are copied into the crate.

## 16. Chinneck archive safety

Chinneck collections are archived. Materialization is qualification infrastructure and treats archive content as untrusted.

Before writing an entry, the helper must reject:

- POSIX absolute paths;
- Windows drive-qualified paths;
- UNC paths;
- normalized `..` traversal;
- symlink/hardlink entries;
- device/FIFO/socket/special-file entries;
- any destination not provably beneath a fresh extraction root.

Extraction occurs into a new temporary root with no-follow behavior. A cache/completion marker is atomically promoted only after all entries and inventory checks succeed. Blind `7z x` followed only by post-hoc validation is not an accepted implementation.

## 17. Differential qualification contract

Native path:

```text
file -> HiGHS readModel -> inspect/solve
```

ROML path:

```text
file -> MpsReader -> Model -> ROML compiler -> HiGHS -> inspect/solve
```

Compare normalized dimensions, matrix, row bounds, column bounds/types, objective coefficients/sense/offset, status, and objective where meaningful.

Accepted-input divergence uses only the three dispositions from §1.1. Strict-policy probes record `intentional_roml_rejection` when applicable.

The differential harness must never call HiGHS parsing to implement ROML parsing.

## 18. IIS qualification contract

For selected Chinneck cases:

```text
safe materialization
 -> P35 parse
 -> structural/native differential
 -> ROML -> HiGHS proves infeasible
 -> Phase 29 analyze_infeasibility
 -> explicit/synthetic source-map every member
 -> validate P29 guarantee/completion
 -> record statistics
```

Do not require exact Gurobi/HiGHS IIS membership. Chinneck explicitly notes multiple IISs are common.

## 19. Qualification tiers

```text
Tier 0: synthetic + metamorphic + fuzz + archive-security fixtures
Tier 1: small corpus smoke on MPS/IIS-impacting PRs
Tier 2: broad scheduled Netlib + bounded Chinneck qualification
Tier 3: heavy/manual large IIS and performance characterization
```

Timing thresholds on hosted runners are not correctness gates.

## 20. P36 writer seam

P36 receives a canonical ROML model and must:

1. check representability before writing;
2. default to deterministic free MPS;
3. choose deterministic objective/rim names;
4. emit deterministic row/column order;
5. encode ranged constraints with one row + RANGES;
6. encode objective constant using negative objective-row RHS;
7. emit minimal deterministic BOUNDS/integrality records under P35 semantics;
8. reject unrepresentable ROML constructs rather than changing semantics;
9. round-trip semantically through P35;
10. interoperate with native HiGHS on selected external round trips.

No textual/comment/whitespace preservation is promised.

## 21. Expected implementation slices — preview only

The executable `35-PLAN.md` is still withheld until written-spec approval. Expected decomposition:

```text
S0 characterize current Model/domain/IIS seams
S1 lexical + source diagnostics
S2 section/record staging parser
S3 vector selection + semantic resolution
S4 provenance + transactional Model construction
S5 public MpsReader API/docs
S6 synthetic/metamorphic/fuzz/security qualification
S7 HiGHS differential harness + divergence disposition machinery
S8 corpus submodules + manifest + safe archive materializer
S9 Netlib + Chinneck IIS qualification
S10 evidence/public docs/independent review
```

Every slice must be TDD-first and run the repository quality gates appropriate to the affected crates.

## 22. Questions deliberately closed

- LALRPOP? **No.**
- Parser dependency? **No by default.**
- Direct parse into Model? **No.**
- Linear LP/MILP only? **Yes.**
- Fixed + free? **Yes.**
- Unsupported extensions ignored? **No.**
- Duplicate COLUMNS entries? **Sum.**
- Duplicate selected RHS/RANGE? **Reject.**
- RANGE on selected `N` row? **Reject.**
- RANGE on `N` in unselected vector? **Stage if structurally valid; inert until selected.**
- Semantic validation of unselected rim alternatives? **No; structural validation yes.**
- BOUNDS repeats? **Ordered transitions, not RHS-style duplicate rejection.**
- Multiple rim vectors? **Stage all; select one; first by default.**
- HiGHS authoritative? **No; independent differential oracle.**
- What if accepted ROML semantics differ from HiGHS? **Block until bug-fix, dialect narrowing, or owner-approved evidence-backed compatibility exception.**
- Implicit MPS bound provenance? **Synthetic explicit origin types with source anchors.**
- Copy corpora into ROML? **No.**
- Submodules? **Optional pinned submodules to owner forks.**
- Chinneck archive traversal/links? **Rejected before write.**
- Exact solver IIS match? **No.**
- Write-back? **P36, designed now.**
- Default writer dialect? **Deterministic free MPS.**
- Round trip textual? **No, semantic.**
- Generic interchange IR now? **No.**

## 23. Written-spec review gate

This branch remains design-only: no production parser/writer code, no `.gitmodules`/gitlinks, no workflow changes, and no active roadmap routing changes.

Review round 1 identified and this packet now closes:

1. selected-vs-unselected RANGE/vector semantic validation;
2. normative differential policy when HiGHS differs, including duplicate RHS strict semantics;
3. synthetic provenance for implicit continuous and INTORG bounds;
4. safe archive extraction rules for Chinneck materialization.

The next step is independent re-review. Only after written-spec approval is the executable `35-PLAN.md` generated.