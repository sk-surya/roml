# MPY Qualification Contract

## Correctness gates

1. Every public example is an executable installed-package test. Include scalar LP, integer model, repeated parameter solve and the synthetic BESS MPC example.
2. Compare Python ROML, direct Rust ROML and direct highspy on mathematically identical fixtures. Validate model dimensions and coefficient/bound/objective content before solving. Independent mathematical objective/feasibility evaluation is required; different optimal primal vectors are acceptable.

   Separate two experiments. In matched-input solver equivalence/performance,
   all arms receive a frozen stream of identical initial energy, forecasts,
   bounds and commitments at each gate, independent of their own chosen actions.
   In closed-loop MPC validation, each arm advances its own energy from its
   applied action; alternate optima can diverge into different next states.
   Validate each such solve against a fresh oracle built from that arm's exact
   current state. Do not require equal cross-arm objectives after trajectories
   diverge, or attribute solve-time differences on unequal models to the wrapper.
3. LP objective tolerance: absolute `1e-7 + 1e-7*max(abs(reference), abs(candidate))`; primal row/bound residual tolerance `1e-7` for well-scaled deterministic fixtures. Align native tolerances explicitly. MIP integrality residual ≤ `1e-6`; compare certified bounds and feasible objectives within the configured gap, not a false identical optimum requirement for limited solves.
4. Test empty/constant models, free variables, integer/binary domains, objective offsets, duplicate terms, negative prices, invalid numeric inputs, coefficient overflow after parameter updates, array shapes/strides/dtypes, cross-model identity, old snapshots, missing values, and poisoned/dirty sessions.
5. Invalid bulk requests are all-or-none, including rejection after a previous valid pending update. Compare canonical mathematical state and pending values, not just row count.
6. Test limit with valid incumbent, limit without incumbent, infeasible, unbounded, ambiguous infeasible-or-unbounded, unknown and native operational error separately. Every asserted diagnostic must have actual evidence.
7. The standard CPython concurrency gate proves Python heartbeat progress during native work, deterministic same-model/session contention, independent-model concurrency and safe destruction on worker threads. No new free-threaded claim.

## Workloads and fixed fixture definitions

All fixtures use RNG seed `20260907`, one solver thread, matched presolve/settings and exact native solver versions. Commit generated-input recipes and checksums with raw measurements. No customer data or network data fetch is required.

| Workload | Definition | Purpose |
|---|---|---|
| LP-small | 24-period battery, LP relaxation explicitly labeled, positive/negative prices | wrapper overhead floor; basic incrementality |
| MILP-MPC | 24-period battery from DESIGN, binary direction, 1,000 rolling gates, 24-period causal synthetic forecasts, energy advanced from first action | application correctness and latency |
| LP-scale | 96 periods × 100 independently indexed batteries, affine balances and bounds; 9,600 price cells updated each gate | batch crossing and construction scaling |
| Bulk-only | 1k, 10k, 100k coefficients, positive bounded continuous variables, one sparse row per 10 variables, duplicate-entry variant | Python-loop and CSR cost detection |
| Construct-update | small Rust ROML soft-constraint/PWL fixture, changing a construct-dependent parameter | honest rebuild classification; no Python PWL API prerequisite |
| Memory-soak | 10,000 update/solve cycles on LP-small; discard all but one result after each cycle | session/handle/result leak detection |

Use `price[t,k] = 50 + 60*sin((t+k)/4) + epsilon[t,k]`, with seeded Gaussian epsilon of standard deviation 5, regenerated causally for each decision gate rather than exposing realized future prices. This is synthetic computational validation, not economic evidence of policy quality. Define realized prices independently from the forecast sequence in the fixture and freeze both recipes. Optional derates and a hard end-of-horizon energy floor can be added as separate named cases; every comparison arm must receive the same case.

For LP-scale construction, use shaped arrays `(100,96)` and explicit flat CSR rows where array axis reduction is unavailable. Keep the LP feasible by bounded charge/discharge and valid initial energy. Do not hide Python loops in the measured ROML bulk arm.

## Comparison arms

- Python ROML persistent session using batched updates.
- Python ROML fresh model/session rebuilt at each gate.
- Direct highspy persistent model with equivalent coefficient/bound edits.
- Direct Rust ROML persistent model with the same formulation and update stream.

Start with a 30-iteration warmup, then at least 30 independent timed repetitions for construction/bulk measurements. For MPC use the 1,000 gate trace. Randomize arm order at the repetition level; separate compilation/import/native-library cold start from steady-state. Measure wall time with a monotonic clock and report p50/p95/p99, native solve time, non-solve time, peak RSS and final retained RSS. Record OS, CPU, RAM, Python/Rust/dependency/native versions, build mode and thread counts. Record explicit warm-start requests/dispositions and synchronization classification, rather than claiming them from object reuse.

## Performance acceptance thresholds

These are engineering targets, not claims about the existing project. Freeze hardware/configuration before evaluating them. Investigate failures using profiles; do not change fixture size or semantics to pass.

- On LP-small, Python persistent end-to-end p50 is no worse than `1.25 * Rust persistent p50 + 1 millisecond`. Additive allowance avoids meaningless ratios for tiny solves.
- On LP-scale, Python persistent non-solve p50 is no worse than `1.5 * Rust persistent non-solve p50 + 5 milliseconds`.
- Bulk construction of the 100k coefficient fixture is at least 3× faster than ROML's documented Python scalar-loop construction on the same host. All arms produce the same model. This specifically measures whether bulk operations avoid interpreter overhead.
- Primitive-only updates must report delta/no-change synchronization after warmup and must not silently rebuild. The construct-dependent fixture may rebuild and must report it honestly. If native/compiler capability changes this classification, amend the observed capability table with evidence; never forge a delta label.
- Wrapper performance relative to highspy is reported for every workload, but universal solver-speed superiority is not required. The decisive gates are bounded wrapper overhead, correct state reuse and batching benefit.
- MPC ordinary native calls use a 2-second time limit and report total elapsed time separately. Limit overruns caused by setup/native behavior are recorded and cannot be mislabeled hard real-time compliance. The synthetic small fixture must produce a feasible result at every feasible gate; any fallback is explicitly counted and investigated.
- After 1,000 memory-soak warmup cycles, retained RSS increase over the next 9,000 cycles is ≤ max(32 MiB, 10% of post-warmup RSS), after explicit collection at measurement boundaries. Distinguish allocator high-water marks from live growth by repeated blocks/native allocation evidence. Deliberately retaining all results is a separate expected-growth experiment, not the leak test.

## Artifact matrix

| OS / architecture | CPython 3.13 | CPython 3.14 | Artifact gate |
|---|---|---|---|
| Linux x86_64 | required | required | portable wheel audit + installed tests |
| macOS arm64 | required | required | linked-library audit + installed tests |
| Windows x86_64 | required | required | DLL dependency audit + installed tests |

Build sdist once and prove a clean extracted-sdist wheel build on Linux. If platform build paths differ materially, add corresponding sdist verification only to resolve that concrete risk. No wheel is claimed portable merely because it built on the developer machine. Require installed NumPy input/output tests and type stubs on both interpreter versions. Per-version wheels are the default; abi3 is optional only with evidence.

## Evidence and final report

Create `evidence/DEPENDENCIES.md`, `PR-INVENTORY.md`, `CORRECTNESS.md`, `PERFORMANCE.md`, `WHEELS.md`, `FINAL-REVIEW.md` and `FINAL-REPORT.md` as the work happens. Keep raw measurements as compact JSON/CSV and CI artifact links; don't commit native binaries. Every report states exact head and commands, expected versus observed behavior, skips, limitations and requirement IDs. FINAL-REPORT must distinguish core, behavior, integration and distribution checks and explicitly say no publication occurred.
