# Phase 35 Decisions — MPS Import

These decisions are binding for P35 implementation planning unless explicitly amended by owner review.

| ID | Decision | Rationale / consequence |
|---|---|---|
| D35-01 | Implement a handwritten streaming parser; do not use LALRPOP. | MPS is section/record/state dominated rather than grammar dominated. Fixed-column parsing, marker state, rim vectors, and diagnostics are clearer in explicit Rust. |
| D35-02 | Add MPS under `roml::io::mps`; do not make `Model::from_mps` the architectural primitive. | File formats are adapters around the canonical model, not model responsibilities. |
| D35-03 | Parse into an MPS-specific staging document before constructing `Model`. | Prevents partial model mutation and permits correct handling of objective/range/rim-vector semantics. |
| D35-04 | Support linear LP/MILP MPS only in P35. | Keeps scope aligned with current ROML production focus and the target IIS/Netlib corpora. |
| D35-05 | Reject unsupported semantic sections explicitly. | Never silently linearize or discard quadratic/conic/vendor semantics. |
| D35-06 | Support fixed and free MPS input. | Required for practical interoperability. |
| D35-07 | `MpsFormat::Auto` uses dual interpretation until one format is uniquely determined; ambiguity is an error. | Avoids brittle whitespace heuristics and silent misparses. |
| D35-08 | Preserve one MPS ranged row as one ROML ranged constraint. | Preserves user/file semantics and gives Phase 29 IIS reports meaningful origins. |
| D35-09 | Sum duplicate matrix coefficients algebraically. | Matches established MPS reader behavior and canonical coefficient semantics. |
| D35-10 | Use a compact column-oriented staging representation, not a hash map keyed by `(row,col)`. | Avoids severe sparse-matrix memory overhead. |
| D35-11 | Objective selection is `OBJNAME` if present, else first `N` row, else zero objective. | Deterministic and compatible with mainstream readers. |
| D35-12 | Default objective sense is minimize. | Standard behavior; must be explicit in metadata/tests. |
| D35-13 | RHS on the selected objective row produces the negative objective offset. | Required interoperability convention; dedicated differential tests required. |
| D35-14 | Preserve all named RHS/RANGES/BOUNDS vectors in staging; resolve exactly one of each. | Correctly represents MPS before user/default selection. |
| D35-15 | Default rim-vector selection is `First`; explicit named selection and `None` are supported. | Compatible with established readers while keeping behavior reproducible. |
| D35-16 | Default continuous variable domain is `[0,+inf]`; integer-marker defaults and explicit integer/binary bound records are implemented exactly and tested. | Domain mistakes change mathematical meaning and IIS membership. |
| D35-17 | Source provenance is returned in `MpsImport` and is not stored in canonical `Model`. | Supports file-aware diagnostics/IIS display without polluting model/compiler state. |
| D35-18 | Parser errors are typed, source-aware, and non-panicking on malformed input. | File input is untrusted and library diagnostics must be actionable. |
| D35-19 | Do not add a parser dependency in P35 unless implementation evidence proves standard-library parsing inadequate. | Preserve dependency hygiene and auditability. |
| D35-20 | Use native HiGHS file reading as an independent differential oracle. | Validates ROML interpretation without delegating implementation to the solver. |
| D35-21 | Differential comparison prioritizes mathematical structure and solve equivalence, not internal ordering. | Row/column ordering and alternative optima must not create false mismatches. |
| D35-22 | Chinneck qualification validates ROML IIS guarantees, not equality with the published Gurobi IIS. | The corpus explicitly contains multiple IISs; membership equality is not a correctness condition. |
| D35-23 | External corpora are optional pinned git submodules under `testdata/corpora/`. | Reproducible, local/offline-capable qualification without copying dataset blobs into ROML. |
| D35-24 | Submodules point to `sk-surya` forks at reviewed SHAs. | Stable owner-controlled endpoints while preserving upstream fork provenance. |
| D35-25 | Normal tests, packaging, and ordinary CI do not initialize or require corpora. | Core development remains fast and source package remains self-contained. |
| D35-26 | Corpus qualification gets a dedicated workflow/tier and machine-readable results. | External data and expensive IIS runs must not make every PR slow or flaky. |
| D35-27 | P36 MPS write-back is designed now but implemented after P35 reader qualification. | Reader must preserve enough semantics for a clean writer; implementation risk remains staged. |
| D35-28 | P36 default output is deterministic free MPS. | Avoids lossy fixed-width naming and provides stable diffs. |
| D35-29 | P36 fixed-format output, if included, is strict and never silently truncates names. | Lossy identifier rewriting is unacceptable without an explicit reversible naming scheme. |
| D35-30 | Round-trip guarantee is semantic, not textual. | Comments, whitespace, ordering, and original vector names need not be byte-preserved. |
| D35-31 | Do not introduce a generic interchange IR in P35. | One format is insufficient evidence for the right generic abstraction; extract it only after another format demonstrates commonality. |
| D35-32 | P35 is pulled forward by owner priority without renumbering P30-P34. | Existing roadmap references remain stable. |
| D35-33 | Production code/submodule gitlinks/roadmap routing wait for written-spec approval. | Maintains design-review gate and avoids accidental implementation during planning. |
| D35-34 | Staging validates syntax, finite numerics, and row/variable references for every rim-vector record, but model-semantic validation is applied only to the selected RHS/RANGES/BOUNDS vector. | Nonselected vectors are alternative model data and must not invalidate an import solely because they would be semantically unusable if selected; selecting such a vector produces the typed semantic error. |
| D35-35 | A selected RANGES vector that contains an entry for any `N` row is rejected; an unselected syntactically/reference-valid RANGES vector may contain such a record and remains inert metadata. | MOSEK's RANGE interpretation is defined for constraint rows and leaves `N` without a transformation; this closes the R07/R09 ambiguity. |
| D35-36 | Duplicate RHS entries for the same row within the selected RHS vector are rejected as ambiguous; they are not summed. Duplicate selected RANGE entries for the same row are likewise rejected. | The MPS ecosystem explicitly documents additive duplicate matrix cells, not RHS/RANGE transformations. ROML chooses strict rejection rather than inventing algebraic semantics. BOUNDS remains an ordered state-transition stream, so repeated bound records are handled by the frozen BOUNDS transition policy. |
| D35-37 | The frozen ROML MPS semantics are normative; HiGHS is differential evidence, not semantic authority. | If an accepted P35 fixture differs from HiGHS, production merge is blocked until ROML is fixed, the accepted dialect is narrowed, or an explicit compatibility exception is approved with authoritative evidence. An intentional strict ROML rejection may coexist with HiGHS acceptance and is recorded as such. |
| D35-38 | Implicit MPS bounds receive synthetic provenance in `MpsSourceMap`. | A default continuous lower bound `0` is attributed to `ImplicitContinuousDefault` anchored to the variable's first COLUMNS record; INTORG's implicit lower `0` and upper `1` are attributed to `ImplicitIntegerMarkerDefault` anchored to the controlling INTORG marker plus the variable's first marked COLUMNS record. IIS bound members therefore always have explicit or synthetic MPS provenance. |
| D35-39 | Chinneck archive materialization is path-safe by construction and rejects unsafe archive entries before writing. | The corpus extractor must reject absolute/drive/UNC paths, `..` traversal, symlink/hardlink/special-file entries, and any destination escaping the extraction root; extraction occurs in a fresh temporary directory and is atomically promoted only after validation. |