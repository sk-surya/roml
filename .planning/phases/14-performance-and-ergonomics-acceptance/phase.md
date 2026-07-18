---
gsd_phase_number: M1R-05
name: performance-ergonomics-acceptance
milestone: ROML-M1R
goal: Establish reproducible performance baselines and validate release user journeys without weakening correctness.
dependencies: [M1R-03]
parallelism: benchmark, profiling, documentation journey, and migration lanes may run independently
---

# M1R-05 — Performance and Ergonomics Acceptance

## Principle
M1R does not require ROML to be the fastest modeling library. It requires performance behavior to be decomposed, reproducible, non-pathological, and honest. A benchmark improvement is rejected if it weakens canonical semantics, acknowledgement, or recovery.

## Workload families
- Tiny interactive LP: API overhead and first solve.
- Medium sparse LP: construction, matrix projection, solve, extraction.
- Repeated parameter LP: parameter propagation, delta compile/apply, basis reuse.
- Structural incremental model: add/remove rows/columns and rebuild crossover.
- Small/medium MILP: request negotiation, incumbent/status extraction, repeated solves.
- Failure/rebuild case: cost of deterministic recovery.

Use fixed public/generated datasets and seeds. Record dimensions, density, domain mix, mutation distribution, solver settings, and expected invariants.

## Tasks
### 05.1 Benchmark architecture
Separate timings for:
1. model construction;
2. expression/canonical-cell processing;
3. parameter propagation;
4. transaction commit and delta compilation;
5. native incremental application;
6. full snapshot rebuild;
7. solver execution;
8. solution extraction;
9. end-to-end user operation.

Do not report end-to-end solve time as modeling overhead. Keep solver logs disabled unless the workload tests logging.

### 05.2 Baselines and metadata
Each run records:
- ROML SHA/version;
- HiGHS version/build/index width;
- Rust/Cargo/LLVM versions;
- OS/architecture/CPU/memory;
- release profile/features;
- thread count and solve request/effective configuration;
- warmup/sample/statistical method;
- dataset and seed.

Store machine-readable result summaries and a human interpretation. Avoid hard universal thresholds from one machine; use regression bands on controlled runners and descriptive evidence elsewhere.

### 05.3 Incremental/rebuild crossover
Measure operation families and batch sizes to determine when incremental application is advantageous versus rebuild. The session may choose rebuild only through an explicit, tested policy that preserves observable semantics and reports the decision where diagnostics require it.

### 05.4 Bulk-path admission
For any bulk projection:
- run scalar and bulk against the same typed batch;
- prove normalized/solve equivalence;
- inject failure in bulk boundaries;
- measure benefit across sparse/dense/batch-size regimes;
- retain scalar fallback and diagnostic classification.

### 05.5 Memory/allocation profile
Profile large sparse construction, snapshot creation, delta retention, native index maps, and solution extraction. Identify unbounded retention or repeated full-map cloning. Any retention/compaction change belongs to a separate contract-reviewed task.

### 05.6 Public user journeys
Run from packed crates and public docs:
- build and solve first LP;
- update parameter and re-solve;
- apply structural update;
- request algorithm/limits and inspect effective configuration;
- recover after a forced rebuild-required state;
- inspect primal, objective, and supported LP attributes;
- diagnose unavailable/unsupported native setup.

Record friction as release blockers, deferred improvements, or documentation changes. Avoid adding broad modeling features during M1R.

### 05.7 Migration guide
Document changes from the pre-M1 public adapter APIs, including construction, synchronization, solve requests, statuses/errors, solution access, and callbacks. Provide compile-checked before/after examples where practical.

## Gate
- M1R-E1–E5 pass.
- Benchmark decomposition and metadata are reproducible.
- No admitted optimization lacks equivalence/recovery evidence.
- Packed-crate user journeys succeed using only public documentation.
- Performance claims are scoped to measured workloads and exact configurations.
