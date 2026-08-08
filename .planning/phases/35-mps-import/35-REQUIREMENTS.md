# Phase 35 Requirements — MPS Import

## Functional requirements

### MPS-R01 — Input surfaces

The reader shall support both `std::io::BufRead`-compatible streams and filesystem paths without requiring the entire file to be loaded into one string.

### MPS-R02 — Supported dialect

The reader shall support the linear LP/MILP semantics represented by `NAME`, `OBJSENSE`, `OBJNAME`, `ROWS`, `COLUMNS`, `RHS`, `RANGES`, `BOUNDS`, and `ENDATA`, including standard integer markers and the bound types frozen in the design spec.

### MPS-R03 — Unsupported semantics

Quadratic, conic, SOS, indicator, PWL, lazy/user-cut, semi-continuous/semi-integer, and unqualified vendor-extension sections shall fail with a typed unsupported/representation error. Unsupported semantics shall not be ignored.

### MPS-R04 — Fixed/free layouts

The reader shall support fixed and free MPS lexical layouts and shall expose an explicit format option with deterministic automatic detection.

### MPS-R05 — Transactionality

A failed read shall not return or mutate a partially constructed ROML `Model`.

### MPS-R06 — Objective resolution

The reader shall implement deterministic objective-row selection, objective sense, zero-objective handling, and objective-offset semantics defined in the MPS semantics reference.

### MPS-R07 — Row semantics

`E`, `L`, `G`, and `N` rows shall be interpreted according to the selected objective and rim vectors. Ranged rows shall become one semantic ranged ROML constraint.

### MPS-R08 — Matrix semantics

Repeated matrix entries for the same `(row,column)` shall be added algebraically. Resulting exact zeros shall follow ROML canonical coefficient normalization. This additive rule does not extend to RHS/RANGES records.

### MPS-R09 — Rim vectors and validation layers

The staging document shall preserve multiple named RHS, RANGES, and BOUNDS vectors until one vector of each class is selected. Default and named selection shall be deterministic and reported in import metadata.

Every staged rim record shall pass lexical/structural validation: valid record kind, finite numeric syntax where applicable, and valid referenced row/variable identity. Model-semantic validation shall apply only to the selected vector of each class. Thus an unselected syntactically valid alternative vector may contain a combination that would be rejected if selected; selecting that vector must produce the corresponding typed semantic error.

Within a selected RHS vector, specifying the same row more than once is a typed duplicate/ambiguity error. Within a selected RANGES vector, specifying the same row more than once is likewise an error. A selected RANGE entry targeting an `N` row is an error. BOUNDS records remain ordered state transitions under the semantics reference.

### MPS-R10 — Variable domains

Continuous defaults, free/fixed/lower/upper bounds, binary/integer bound records, and integer-marker defaults shall produce mathematically correct ROML domains with typed failures for invalid combinations.

### MPS-R11 — Names

Problem, row, and variable names shall be preserved without undocumented truncation. Free-format long names shall remain intact.

### MPS-R12 — Provenance, including implicit semantics

The import result shall expose source metadata sufficient to relate ROML variables, constraints, bounds, and objective records back to MPS provenance without storing source-file concerns in the canonical `Model`.

Provenance shall distinguish explicit source records from implicit format defaults. At minimum:

- a continuous variable's implicit finite lower bound `0` shall have synthetic `ImplicitContinuousDefault` provenance anchored to the variable's first ordinary COLUMNS record;
- an INTORG variable's implicit lower `0` and upper `1` shall have synthetic `ImplicitIntegerMarkerDefault` provenance anchored to the controlling `INTORG` marker and the variable's first marked COLUMNS record;
- explicit BOUNDS records shall retain their exact source spans.

An IIS member corresponding to a finite imported variable bound must therefore resolve to either an explicit bound record or one of these synthetic MPS origins. Synthetic origins must never be rendered as if a literal BOUNDS line existed.

### MPS-R13 — Diagnostics

Malformed input shall produce typed, source-aware errors that include line and section context where available. Parser code shall not panic on malformed input.

## Architecture requirements

### MPS-A01 — Solver independence

The core reader implementation shall not depend on HiGHS or another solver/parser implementation.

### MPS-A02 — No parser generator

P35 shall begin with a handwritten state-machine parser. A new parsing dependency requires explicit evidence and review.

### MPS-A03 — Staging boundary

Lexical parsing and MPS record parsing shall populate a private MPS-specific staging representation before ROML model construction.

### MPS-A04 — Future writer seam

The staging/semantic vocabulary and public options shall not prevent P36 from serializing representable ROML linear LP/MILP models without depending on reader implementation details.

### MPS-A05 — No premature universal IR

P35 shall not introduce a solver/file-format-neutral interchange IR solely to anticipate future formats.

## Qualification requirements

### MPS-Q01 — Synthetic fixture corpus

The repository shall contain small redistributable synthetic fixtures covering every supported record and edge semantic. These fixtures shall run in normal CI without external repositories.

### MPS-Q02 — HiGHS differential reader and authority policy

For selected supported files, qualification shall compare native HiGHS file reading with ROML MPS reading followed by ROML-to-HiGHS compilation.

The frozen ROML MPS semantics reference is normative; HiGHS is an independent compatibility oracle, not the authority that defines ROML semantics. For inputs accepted by P35, a semantic mismatch with HiGHS is a production-merge blocker until one of three reviewed dispositions occurs: ROML is corrected; the P35 accepted dialect is narrowed so the input is rejected; or an explicit compatibility exception is approved using authoritative format evidence. HiGHS accepting an input that ROML deliberately rejects under a frozen strict policy is recorded as `intentional_roml_rejection`, not silently treated as parser equivalence.

### MPS-Q03 — Structural equivalence

Where accessible, differential tests shall compare dimensions, matrix semantics, row bounds, column bounds, integrality, objective sense, objective coefficients, and objective offset after normalization.

### MPS-Q04 — Solve equivalence

Feasible corpus cases shall compare termination classification and objective value within explicit recorded tolerances. Tests shall not require identical primal vectors where alternate optima exist.

### MPS-Q05 — Infeasible corpus

Selected Chinneck cases shall be parsed by both paths, proven infeasible, and passed through ROML Phase 29 analysis.

### MPS-Q06 — IIS guarantee validation

Complete ROML IIS reports shall be validated according to the Phase 29 irreducibility contract. Tests shall not require equality to Gurobi's published IIS membership.

### MPS-Q07 — Source-aware IIS evidence

For imported models, reported IIS members shall be resolvable to original MPS semantic rows and/or variable bounds with explicit-or-synthetic provenance as defined by MPS-R12.

### MPS-Q08 — External corpus pins

Corpus qualification shall record exact repository URLs and commit SHAs. The design-time pins are `sk-surya/infeasiblelps@97a936498e5240d44adaf7dcfe84877fa34ce301` and `sk-surya/lp-data-netlib@56257eea85b433ce6aa67d26156b36385318fd6f`.

### MPS-Q09 — Optional corpora

Ordinary builds/tests shall not fail when submodules are absent. A dedicated qualification command/workflow shall initialize or validate them.

### MPS-Q10 — Support manifest

External qualification shall maintain a machine-readable or deterministic manifest classifying each corpus file as supported/pass, intentionally unsupported, skipped with reason, or failed.

## Robustness requirements

### MPS-S01 — Resource safety

Counters and capacity arithmetic shall be checked. Parsing shall not recurse proportional to input size.

### MPS-S02 — Fuzzability

Lexical/record parsing and end-to-end parse-to-result/error surfaces shall be designed so fuzz targets can exercise them without native solvers.

### MPS-S03 — Large model behavior

The staging representation shall avoid per-nonzero hash-map/object overhead that makes large sparse MPS models impractical. Qualification shall characterize parsing of larger corpus files.

### MPS-S04 — Determinism

Given identical bytes and read options, successful imports shall produce deterministic semantic ordering/metadata and deterministic errors for invalid input.

### MPS-S05 — Safe corpus archive extraction

Any Chinneck archive materialization helper shall treat archive entries as untrusted input. Before writing an entry it shall reject absolute paths, Windows drive/UNC paths, any normalized `..` traversal, symlink/hardlink/special-file entries, and any destination that does not remain beneath the fresh extraction root. Extraction shall occur in a fresh temporary directory, shall not follow filesystem symlinks, and shall be atomically promoted/cached only after full validation and successful completion.

## P36 forward requirements

These are binding constraints on P35 design, not P35 production deliverables.

### MPS-W01 — Writer API

P36 shall provide stream/path writer APIs under `roml::io::mps`.

### MPS-W02 — Canonical output

Default P36 output shall be deterministic free-format MPS.

### MPS-W03 — Representation failures

A valid ROML model outside the selected linear MPS dialect shall produce a typed representation error rather than silent lowering that changes semantics.

### MPS-W04 — Semantic round trip

Representable models shall satisfy `Model -> MPS -> Model` semantic equivalence under a defined snapshot comparison.

### MPS-W05 — External round trip

Selected external files shall satisfy `MPS -> ROML -> canonical MPS -> native HiGHS` structural/solve equivalence.

### MPS-W06 — No textual preservation promise

P36 need not reproduce original comments, spacing, record ordering, or byte layout.

## Exit criteria for written-spec approval

Before implementation planning begins:

- every requirement above has one unambiguous interpretation;
- the semantics reference contains no unresolved placeholders;
- the test matrix names a verification strategy for each high-risk semantic;
- corpus acquisition and licensing posture are explicit;
- P35/P36 boundary is explicit;
- no production code or submodule gitlinks have landed on the design branch.