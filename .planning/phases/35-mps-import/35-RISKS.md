# Phase 35 Risks, Failure Modes, and Stop Conditions

## Risk register

| ID | Risk | Severity | Mitigation / required evidence |
|---|---|---:|---|
| R35-01 | Fixed/free auto-detection silently chooses the wrong layout. | P0 semantic | Dual-parse detection; ambiguity is typed error; fixed/free metamorphic fixtures. |
| R35-02 | Objective row or offset is misinterpreted. | P0 semantic | Explicit OBJNAME/first-N rules; negative RHS offset fixtures; native HiGHS differential probe. |
| R35-03 | RANGE sign/sense handling changes feasible region. | P0 semantic | Frozen four-case table; exhaustive unit + native differential probes. |
| R35-04 | Integer marker defaults are implemented as ordinary integer `[0,+inf]` rather than legacy `[0,1]`. | P0 semantic | Marker-specific domain tests and HiGHS differential probe. |
| R35-05 | Bounds records accidentally remove integrality or binary semantics. | P0 semantic | Domain state machine; explicit conflicting-binary policy; synthetic MILP solves. |
| R35-06 | Unsupported quadratic/conic sections are ignored after the linear portion. | P0 semantic | Recognized unsupported-section hard failures; adversarial fixtures. |
| R35-07 | Parser mutates `Model` before later failure, leaving partial state. | P1 correctness | Staging document + fresh model construction only after validation. |
| R35-08 | Duplicate coefficients are overwritten rather than summed. | P1 correctness | Algebraic COLUMNS accumulation + zero-after-sum tests + native differential. |
| R35-09 | Sparse staging consumes excessive memory on large models. | P1 production | Column-oriented vectors; no `(row,col)` hash map; large-corpus peak-memory characterization. |
| R35-10 | Error messages lack source context and make real corpus debugging impractical. | P1 usability | Typed location/section/entity errors; golden diagnostic tests. |
| R35-11 | Multiple rim vectors accidentally combine or a nonselected vector changes the model. | P0 semantic | Structural validation for all vectors; semantic validation for selected vectors only; R07/R09 and selection tests. |
| R35-12 | Source provenance changes canonical model/compiler identity. | P1 architecture | Source map external to `Model`; compiler behavior independent of source map. |
| R35-13 | HiGHS differential tests become circular because implementation starts using HiGHS parsing. | P0 architecture | Core parser has no solver dependency; HiGHS reader only in qualification path. |
| R35-14 | External corpus absence breaks ordinary development/package tests. | P1 CI | Optional submodules; Tier 0 synthetic tests require no corpus. |
| R35-15 | Corpus licensing/provenance becomes muddled by copied files. | P1 governance | Store gitlinks only; exact fork/upstream URLs; no vendoring without separate review. |
| R35-16 | Submodule HEAD floats and qualification is irreproducible. | P1 reproducibility | Exact gitlink SHAs + report-level SHA check. |
| R35-17 | Corpus test compares exact IIS membership and fails valid alternate IISs. | P1 test validity | Compare guarantee/infeasibility/source mapping; Gurobi membership/count informational only. |
| R35-18 | Writer design later cannot reproduce reader semantics. | P1 architecture | Freeze P36 reverse mapping now; shared semantic vocabulary; no textual-AST dependency. |
| R35-19 | Writer silently loses ROML constructs not representable in MPS. | P0 semantic, P36 | Typed representation errors; no silent lowering outside approved dialect. |
| R35-20 | Future LP format is forced through MPS-specific abstractions. | P2 architecture | Keep staging private/MPS-specific; extract generic conventions only after second-format evidence. |
| R35-21 | Parser accepts hostile inputs that trigger huge allocations/panics. | P1 robustness | Checked counters, configurable limits seam, fuzzing, no input-sized recursion. |
| R35-22 | Historical MPS ambiguity differs among MOSEK/CPLEX/HiGHS. | P1 interop | Frozen ROML semantics are normative; accepted-input HiGHS mismatch blocks merge until `roml_bug_fixed`, `dialect_narrowed`, or owner-approved `compatibility_exception`; strict ROML rejection is recorded separately. |
| R35-23 | Synthetic tests overfit our parser and miss historical real files. | P1 qualification | Netlib + Chinneck corpus tiers and native HiGHS differential. |
| R35-24 | Corpus CI becomes slow/flaky because full IIS analysis is expensive. | P1 CI | Tiered allowlists/budgets; heavy IIS manual/release only. |
| R35-25 | Parser performance optimization obscures correctness. | P1 engineering | Correct staging/state machine first; benchmark only after semantics pass; no unsafe in core. |
| R35-26 | Duplicate selected RHS/RANGE records are accidentally treated like duplicate matrix cells. | P0 semantic | Explicit typed rejection tests; additive policy is COLUMNS-only. |
| R35-27 | IIS reports cannot explain a default MPS bound because no BOUNDS line exists. | P0 diagnostic/semantic | Synthetic `ImplicitContinuousDefault` and `ImplicitIntegerMarkerDefault` origins with source anchors; completeness tests for every finite bound restriction. |
| R35-28 | Chinneck archive extraction writes outside the generated corpus root or creates links/special files. | P0 security | Pre-write path/type validation, fresh no-follow root, reject absolute/drive/UNC/`..`/links/special entries, atomic promotion after full success; adversarial archive fixtures. |

## Stop conditions during implementation planning/execution

Stop and request architectural review if any proposed task would:

1. add HiGHS as a dependency of `roml` core;
2. parse directly into a live `Model` while the file is still structurally unvalidated;
3. silently skip an unsupported semantic section;
4. split an MPS ranged row into unrelated public semantic constraints solely for parser convenience;
5. truncate or rewrite identifiers without an explicit reversible naming contract;
6. infer fixed/free format using a heuristic that can silently produce two different semantic interpretations;
7. make external corpora mandatory for `cargo test`, docs.rs, or crate packaging;
8. copy external corpus files into ROML without a separate redistribution/provenance decision;
9. treat native HiGHS MPS parsing as ROML's implementation or semantic authority;
10. assert that ROML must return the same IIS members as Gurobi/HiGHS;
11. report `Irreducible` without the existing Phase 29 verification contract;
12. introduce a generic interchange IR without concrete second-format evidence;
13. broaden P35 into quadratic/conic/SOS/indicator parsing;
14. broaden P35 into MPS writer production code instead of preserving P35/P36 execution boundary;
15. add unsafe code to the solver-free parser;
16. introduce a parser dependency without measured/technical reason and owner review;
17. change frozen MPS semantics solely to make one HiGHS/corpus case pass without authoritative evidence;
18. introduce timing thresholds on GitHub-hosted runners as correctness gates;
19. sum/overwrite duplicate selected RHS or RANGE records instead of returning the frozen typed error;
20. reject a whole file solely because an unselected vector is semantically unusable, when its syntax/references are valid;
21. return an IIS bound member without explicit-or-synthetic MPS provenance;
22. extract an archive entry before proving its path/type is safe beneath the fresh materialization root;
23. run blind archive extraction and rely only on post-extraction validation.

## Differential review dispositions

For an input P35 accepts, a native HiGHS semantic mismatch has exactly three allowed closure paths:

1. **`roml_bug_fixed`** — ROML was wrong relative to the frozen/authoritative semantics and is corrected;
2. **`dialect_narrowed`** — the input is removed from the accepted P35 dialect and ROML now rejects it explicitly;
3. **`compatibility_exception`** — authoritative evidence supports ROML's behavior, the divergence is documented, and owner review approves it.

For inputs deliberately rejected by strict P35 policy, native HiGHS acceptance is recorded as `intentional_roml_rejection` telemetry rather than forcing semantic convergence.

## P36-specific deferred risks

P35 records but does not close:

- canonical free-MPS writer naming for objective/RHS/RANGES/BOUNDS vectors;
- deterministic numeric formatting and precision/round-trip choice;
- fixed-format writer field-width constraints;
- writer integrality representation: marker versus LI/UI/BV;
- whether writer omits explicit defaults aggressively or favors readability;
- handling objective names when the ROML objective lacks a public name;
- whether source-map-aware write diagnostics are useful.

The P36 contract remains deterministic output, semantic round-trip, and no silent loss.