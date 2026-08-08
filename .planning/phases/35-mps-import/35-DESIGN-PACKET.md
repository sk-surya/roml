# Phase 35 Ultra Design Packet — MPS Import, Differential Qualification, and P36 Write-Back Seam

**Status:** architecture approved in discussion; written-spec review pending  
**Baseline:** `main@ff99389b9bf1318c555dc1f72dff6b5c7a4111c0`  
**Design:** `docs/superpowers/specs/2026-08-07-mps-io-design.md`  
**Primary requirement files:** `35-REQUIREMENTS.md`, `35-MPS-SEMANTICS.md`, `35-TEST-MATRIX.md`  
**External qualification:** `35-CORPUS-QUALIFICATION.md`  
**Risk authority:** `35-RISKS.md`  
**Decisions:** `35-DECISIONS.md`

## Executive intent

Build a production-quality, solver-independent MPS import layer in ROML so real LP/MILP models can be loaded, solved, diagnosed with Phase 29 IIS, and compared against an independent mature MPS reader. Architect the same semantic boundary so P36 can write deterministic MPS and prove semantic round-trip correctness without redesigning the importer.

P35 is not "just enough parser code to run Chinneck." It is the first durable file-format feature in ROML.

## Architectural theorem

For every MPS file inside P35's declared linear dialect and read options `o`, let:

```text
R(file,o) = canonical mathematical ROML model produced by MpsReader
H(file)   = mathematical model read independently by HiGHS
```

P35's qualification target is that, after normalization of representation-only differences:

```text
semantics(R(file,o)) == semantics(H(file))
```

for the selected objective and rim vectors under the compatible policy being tested.

For infeasible models, the additional theorem is:

```text
infeasible(R(file,o))
  && Phase29(R(file,o)) reports only guarantees it independently verifies
  && every reported semantic member can be related to the imported MPS source
```

P36 adds:

```text
semantics(read(write(model))) == semantics(model)
```

for every ROML model representable by the selected linear MPS writer dialect.

## Topology

```text
                         P35 CORE

          path / BufRead / bytes
                   |
                   v
         +--------------------+
         | lexical line layer |
         | fixed/free/auto    |
         | spans/numbers      |
         +---------+----------+
                   |
                   v
         +--------------------+
         | record/state layer |
         | sections/markers   |
         | named vectors      |
         +---------+----------+
                   |
                   v
         +--------------------+
         | MPS staging doc    |
         | rows/cols/rim data |
         | source provenance  |
         +---------+----------+
                   |
            validate/resolve
                   |
                   v
         +--------------------+
         | fresh ROML Model   |
         +---------+----------+
                   |
        +----------+-----------+
        |                      |
        v                      v
   roml-highs solve       Phase 29 IIS
        |                      |
        +----------+-----------+
                   |
                   v
             qualification

                       INDEPENDENT ORACLE

      original file ----------------------> HiGHS readModel
                                                  |
                                                  v
                                          structure / solve
                                                  |
                   +------------------------------+
                   |
                   v
             normalized comparison

                         P36 FOLLOW-ON

          ROML Model
              |
              v
      representability check
              |
              v
     deterministic MPS writer
              |
              v
          free MPS
              |
         +----+----+
         |         |
         v         v
      P35 read   HiGHS readModel
         |         |
         +----+----+
              |
              v
       semantic round trip
```

## Package/module boundary

Planned core API namespace:

```text
src/io/mod.rs
src/io/mps/mod.rs
src/io/mps/reader.rs
src/io/mps/lex.rs
src/io/mps/record.rs
src/io/mps/semantic.rs
src/io/mps/options.rs
src/io/mps/error.rs
src/io/mps/metadata.rs
```

P36 may add:

```text
src/io/mps/writer.rs
```

Exact file split is implementation-plan territory, but responsibilities must remain isolated. A single monolithic `mps.rs` owning lexing, semantic resolution, model mutation, writing, and diagnostics is not acceptable.

## Public seam sketch

Names are provisional until implementation planning verifies consistency with current ROML API conventions, but semantics are binding.

```rust
pub struct MpsReader {
    options: MpsReadOptions,
}

pub struct MpsReadOptions {
    pub format: MpsFormat,
    pub rhs: MpsVectorSelection,
    pub ranges: MpsVectorSelection,
    pub bounds: MpsVectorSelection,
    pub limits: MpsReadLimits,
}

#[non_exhaustive]
pub enum MpsFormat {
    Auto,
    Fixed,
    Free,
}

#[non_exhaustive]
pub enum MpsVectorSelection {
    First,
    Named(String),
    None,
}

pub struct MpsImport {
    pub model: Model,
    pub metadata: MpsMetadata,
    pub source_map: MpsSourceMap,
    pub diagnostics: Vec<MpsDiagnostic>,
}
```

The final public API must preserve the ability to add new MPS dialect options without breaking semver, so public option/result/error enums should be non-exhaustive where appropriate.

## Internal state machine

High-level parser states:

```text
Start
  -> NameOrPrelude
  -> Rows
  -> Columns { integer_region: bool }
  -> Rhs
  -> Ranges
  -> Bounds
  -> End
```

Optional sections are transitions, not separate parsers that independently mutate the model.

Each record handler receives:

```text
active section
locked/undecided lexical format
line number + raw bytes
staging document builder
```

and returns either:

```text
accepted record + possible format lock
```

or a typed source-aware error.

## Staging document invariants

Before semantic resolution:

1. every row name is unique;
2. every COLUMNS/RHS/RANGES reference resolves to a declared row;
3. every BOUNDS target resolves to a declared-by-COLUMNS variable;
4. marker nesting is balanced;
5. every numeric field is finite and syntax-valid;
6. section order is valid;
7. no unsupported semantic section was encountered;
8. named rim vectors retain first-seen vector ordering and record ordering;
9. source spans remain associated with each record;
10. no ROML `Model` has been constructed yet.

After resolution:

1. one objective row or zero objective is selected;
2. at most one RHS/RANGES/BOUNDS vector affects the model;
3. duplicate matrix/objective coefficients are accumulated;
4. ranges are converted into exact row lower/upper bounds;
5. variable domains are fully resolved;
6. final domains and numeric values satisfy ROML validation;
7. one fresh `Model` is built transactionally;
8. import metadata records all selected policies/vector names.

## Writer seam invariants frozen for P36

P35 must preserve enough semantic clarity that P36 can construct MPS without reaching into parser internals.

P36 receives a canonical ROML model and must:

1. check representability before writing;
2. choose deterministic names for objective/rim vectors;
3. emit deterministic row/column order;
4. encode ranged constraints as one row + RANGES;
5. encode objective constant as negative objective-row RHS;
6. encode variable domains using a documented minimal deterministic BOUNDS policy;
7. emit integrality deterministically;
8. default to free-format MPS;
9. reject unrepresentable constructs instead of silently lowering them;
10. produce output that P35 and native HiGHS both interpret equivalently.

No requirement exists to reproduce the input bytes/comments/layout.

## Supported P35 dialect

Supported:

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

## High-risk semantic table

### RANGES

Let RHS be `b`, range record be `r`.

```text
E, r >= 0 : b       <= a'x <= b + r
E, r <  0 : b + r   <= a'x <= b
G           : b       <= a'x <= b + |r|
L           : b-|r|   <= a'x <= b
```

A ranged row stays one ROML semantic constraint.

### Default variable bounds

```text
ordinary continuous variable:
    [0,+inf]

integer-marker variable before selected BOUNDS modifications:
    integer [0,1]
```

### BOUNDS transitions

```text
FR -> [-inf,+inf]
FX v -> [v,v]
LO v -> lower=v
MI -> lower=-inf
PL -> upper=+inf
UP v -> upper=v
BV -> binary [0,1]
LI v -> integer lower=ceil(v)
UI v -> integer upper=floor(v)
```

Selected records apply deterministically in file order; ordinary bound records do not erase integrality.

### Objective offset

```text
RHS value r on selected objective row
    -> ROML objective constant = -r
```

## Dependency policy

P35 adds no parser generator and initially no parser-combinator crate. `roml` remains solver-free and `unsafe_code = deny` remains applicable.

HiGHS-specific differential access remains outside core. The reader must be fully testable with synthetic fixtures without a native solver.

## External corpora

Planned optional submodules:

```text
testdata/corpora/infeasible-lps
    -> https://github.com/sk-surya/infeasiblelps
    -> 97a936498e5240d44adaf7dcfe84877fa34ce301

testdata/corpora/netlib
    -> https://github.com/sk-surya/lp-data-netlib
    -> 56257eea85b433ce6aa67d26156b36385318fd6f
```

No corpus files are copied into the crate. Normal tests require no recursive checkout.

## Qualification tiers

```text
Tier 0: synthetic, every PR, no corpus
Tier 1: small corpus smoke, MPS/IIS-impacting PRs
Tier 2: broad scheduled corpus differential
Tier 3: heavy/manual performance + large IIS
```

The corpus harness emits machine-readable JSON evidence and a compact human summary.

## HiGHS comparison strategy

Native path:

```text
file -> Highs_readModel/readModel -> inspect/solve
```

ROML path:

```text
file -> MpsReader -> Model -> ROML compiler -> HighsSession -> inspect/solve
```

Required normalized comparison includes dimensions, matrix values, row bounds, variable bounds/types, objective coefficients/sense/offset, solve status, and objective where meaningful.

Native parser acceptance of an intentionally unsupported P35 extension does not make ROML wrong; the comparison applies only inside the declared ROML dialect.

## IIS test strategy

For selected Chinneck models:

```text
parse
 -> structural differential
 -> prove infeasible
 -> ROML analyze_infeasibility
 -> source-map every conflict member
 -> verify Phase 29 guarantee
 -> record statistics
```

Do not require Gurobi's exact IIS. The Chinneck repository itself states that many models have numerous IISs and different solvers will likely isolate different subsets.

## P35 verification pyramid

```text
                    external corpora
                  /                  \
          HiGHS differential     IIS end-to-end
                 /                    \
             metamorphic semantic tests
                     |
              section/state tests
                     |
              lexical/error tests
```

Every P0 semantic rule must have a small direct fixture even if the corpus covers it incidentally.

## Quality gates expected for implementation

The future executable plan must include, at minimum:

```text
cargo fmt --all -- --check
cargo check -p roml --all-targets --locked
cargo clippy -p roml --all-targets --locked -- -D warnings
cargo nextest run -p roml --all-targets --locked
cargo test -p roml --doc --locked
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps --locked

# HiGHS differential/qualification
cargo check -p roml-highs --all-targets --features bundled --locked
cargo clippy -p roml-highs --all-targets --features bundled --locked -- -D warnings
cargo nextest run -p roml-highs --features bundled --locked
# plus system HiGHS lane where applicable

# parser quality
fuzz smoke / bounded fuzz target
mutation coverage for semantic branches where practical
corpus Tier 1
scheduled/manual Tier 2/3 evidence separately
```

Exact commands belong in the implementation plan after written-spec approval and must match repository verification policy at execution time.

## Implementation slice preview — not yet an executable plan

This is architectural decomposition only. The detailed task plan is intentionally withheld until the written-spec review gate passes.

Likely slices:

```text
S0 characterize current Model/domain/public I/O seams
S1 lexical + source diagnostics
S2 section/record staging parser
S3 semantic resolution + transactional Model construction
S4 public MpsReader API + docs
S5 synthetic/metamorphic/fuzz qualification
S6 HiGHS independent differential harness
S7 corpus submodules + manifest + tiered workflow
S8 Chinneck IIS end-to-end qualification + example
S9 evidence/public docs/independent review

P36:
W1 writer representability + options
W2 deterministic free MPS emission
W3 domains/ranges/objective offset
W4 synthetic semantic round trip
W5 external HiGHS/corpus round trip
W6 optional strict fixed writer / docs / evidence
```

The future detailed plan must make each slice TDD-first, with failing characterization/regression tests before implementation and independent review before merge.

## Questions deliberately closed by this packet

- LALRPOP? **No.**
- Parser dependency? **No by default.**
- Direct parse into Model? **No.**
- External universal IR now? **No.**
- Linear only? **Yes.**
- Fixed and free? **Yes.**
- Unsupported extensions ignored? **No.**
- Duplicate matrix entries? **Sum.**
- Multiple rim vectors? **Stage all, select one; first by default.**
- HiGHS reader? **Independent oracle.**
- Copy datasets into repo? **No.**
- Submodules? **Yes, optional pinned submodules to owner forks.**
- Exact Gurobi IIS match? **No.**
- Write-back? **P36, designed now.**
- Default writer dialect? **Deterministic free MPS.**
- Round trip textual? **No, semantic.**

## Written-spec review gate

This branch intentionally contains no production parser code, no submodule gitlinks, and no active roadmap-routing mutation. The next step after owner review is to invoke the detailed implementation-planning workflow and create the executable P35 task plan from this packet.
