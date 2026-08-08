# Phase 35 Risks, Failure Modes, and Stop Conditions

## Risk register

| ID | Risk | Severity | Mitigation / required evidence |
|---|---|---:|---|
| R35-01 | Fixed/free auto-detection silently chooses the wrong layout. | P0 semantic | Dual-parse detection; ambiguity is typed error; fixed/free metamorphic fixtures. |
| R35-02 | Objective row or offset is misinterpreted. | P0 semantic | Explicit OBJNAME/first-N rules; negative RHS offset fixtures; native HiGHS/CPLEX-compatible differential probe. |
| R35-03 | RANGE sign/sense handling changes feasible region. | P0 semantic | Frozen four-case table; exhaustive unit + native differential probes. |
| R35-04 | Integer marker defaults are implemented as ordinary integer `[0,+inf]` rather than legacy `[0,1]`. | P0 semantic | Marker-specific domain tests and HiGHS differential probe. |
| R35-05 | Bounds records accidentally remove integrality or binary semantics. | P0 semantic | Domain state machine; explicit conflicting-binary policy; synthetic MILP solves. |
| R35-06 | Unsupported quadratic/conic sections are ignored after the linear portion. | P0 semantic | Recognized unsupported-section hard failures; adversarial fixtures. |
| R35-07 | Parser mutates `Model` before later failure, leaving partial state. | P1 correctness | Staging document + fresh model construction only after validation. |
| R35-08 | Duplicate coefficients are overwritten rather than summed. | P1 correctness | Algebraic accumulation + zero-after-sum tests + native differential. |
| R35-09 | Sparse staging consumes excessive memory on large models. | P1 production | Column-oriented vectors; no `(row,col)` hash map; large-corpus peak-memory characterization. |
| R35-10 | Error messages lack source context and make real corpus debugging impractical. | P1 usability | Typed location/section/entity errors; golden diagnostic tests. |
| R35-11 | Multiple rim vectors accidentally combine or a non-first vector affects defaults. | P1 semantic | Staging all vectors, one explicit selection, interleaving tests, metadata evidence. |
| R35-12 | Source provenance changes canonical model/compiler identity. | P1 architecture | Source map external to `Model`; compiler behavior equivalence tests with/without source map consumer. |
| R35-13 | HiGHS differential tests become circular because implementation starts using HiGHS parsing. | P0 architecture | Core parser has no solver dependency; review dependency graph; HiGHS reader only in qualification path. |
| R35-14 | External corpus absence breaks normal development/package tests. | P1 CI | Optional submodules; Tier 0 synthetic tests require no corpus. |
| R35-15 | Corpus licensing/provenance becomes muddled by copied files. | P1 governance | Store gitlinks only; exact fork/upstream URLs; no vendoring without separate review. |
| R35-16 | Submodule HEAD floats and qualification is irreproducible. | P1 reproducibility | Exact gitlink SHAs + report-level SHA check. |
| R35-17 | Corpus test compares exact IIS membership and fails valid alternative IISs. | P1 test validity | Compare guarantee/infeasibility/source mapping; Gurobi membership/count informational only. |
| R35-18 | Writer design later cannot reproduce reader semantics. | P1 architecture | Freeze P36 reverse mapping now; shared semantic vocabulary; no textual-AST dependency. |
| R35-19 | Writer silently loses ROML constructs not representable in MPS. | P0 semantic, P36 | Typed representation errors; no silent lowering outside approved dialect. |
| R35-20 | Future LP format is forced through MPS-specific abstractions. | P2 architecture | Keep staging private/MPS-specific; only generic I/O conventions may be extracted later. |
| R35-21 | Parser accepts hostile inputs that trigger huge allocations/panics. | P1 robustness | Checked counters, configurable limits seam, fuzzing, no input-sized recursion. |
| R35-22 | Historical MPS ambiguity differs among MOSEK/CPLEX/HiGHS. | P1 interop | Freeze ROML policy; differential-probe high-risk semantics; record intentional differences explicitly. |
| R35-23 | Synthetic tests overfit our parser and miss historical real files. | P1 qualification | Netlib + Chinneck corpus tiers and native HiGHS differential. |
| R35-24 | Corpus CI becomes slow/flaky because full IIS analysis is expensive. | P1 CI | Tiered allowlists/budgets; heavy IIS manual/release only. |
| R35-25 | Parser performance optimization obscures correctness. | P1 engineering | Correct staging/state machine first; benchmark only after semantics pass; no unsafe in core. |

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
9. treat native HiGHS MPS parsing as ROML's implementation rather than an independent oracle;
10. assert that ROML must return the same IIS members as Gurobi/HiGHS;
11. report `Irreducible` without the existing Phase 29 verification contract;
12. introduce a generic interchange IR without a concrete second-format requirement that justifies it;
13. broaden P35 into quadratic/conic/SOS/indicator parsing;
14. broaden P35 into MPS writer production code instead of preserving the P35/P36 execution boundary;
15. add unsafe code to the solver-free parser;
16. introduce a parser dependency without a measured/technical reason and owner review;
17. change the frozen MPS semantic interpretation solely to make one corpus file pass without independent reference evidence;
18. introduce timing thresholds on GitHub-hosted runners as correctness gates.

## Review escalations, not automatic blockers

The following findings require explicit review but may result in a documented compatibility policy rather than redesign:

- native HiGHS differs on repeated RHS or unusual repeated BOUNDS semantics;
- one external file relies on a vendor extension outside P35;
- a Netlib conversion contains nonstandard but mathematically obvious formatting;
- free-format names exercise characters beyond the chosen ASCII contract;
- HiGHS normalizes zero coefficients/dimensions differently after read;
- a model has alternate optimum solutions or solver-status nuances that prevent vector-level comparison.

## P36-specific deferred risks

P35 must record but does not close these:

- canonical free-MPS writer naming for objective/RHS/RANGES/BOUNDS vectors;
- deterministic numeric formatting and precision/round-trip choice;
- fixed-format writer field-width constraints;
- representation of integer variables: marker versus LI/UI/BV records;
- whether writer should omit explicit defaults aggressively or favor readability;
- handling objective names when the ROML objective lacks a public name;
- whether source-map-aware write diagnostics are useful.

These are P36 implementation-design details. The P36 contract remains: deterministic output, semantic round-trip, no silent loss.
