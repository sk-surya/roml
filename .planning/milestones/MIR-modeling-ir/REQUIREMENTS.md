# MIR Requirements and Acceptance Ledger

Every row is required. Execution evidence records exact commands, test node IDs, head SHAs and artifact paths under `evidence/`. Planned tests are not evidence.

| ID | Requirement | Phase | Acceptance evidence target |
|---|---|---|---|
| IR-01 | Capture current construction/reprice timings and the actual lowering path for LP-scale BESS `rm.sum(price * (discharge - charge))`; distinguish 28,800 updated price parameters from 57,600 affected objective cells | MIR-00 | `evidence/BASELINE.md`; measured path/counters, no guessed path |
| IR-02 | `VarSpan`/`ParamSpan` are opaque and constructible only by trusted block allocation | MIR-01 | compile-fail/API visibility test; source review |
| IR-03 | Variable block creation validates once, reserves once, allocates sequentially, emits one packed variable-add `Change` and one self-contained variable-block `ModelOp`; zero per-variable journal records | MIR-01 | journal/delta assertions for n=1 and n=100k |
| IR-04 | Parameter block creation validates/reserves/allocates in bulk with zero per-parameter journal records and preserves existing scalar parameter-creation revision semantics (no new solver-facing add op merely for existence) | MIR-01 | scalar-vs-block revision test; store/journal assertions |
| IR-05 | Block allocation materializes no per-element names; existing borrowed scalar name APIs remain compatible; L1/frontend component names are compact metadata | MIR-01/MIR-04 | memory/ownership audit; existing name tests unchanged |
| IR-06 | Per-entity stale-ID semantics are unchanged: deleting one block member leaves siblings valid and the deleted member is a typed stale error | MIR-01 | deletion-in-block tests |
| IR-07 | Existing `add_linear_rows_bulk` public behavior and journal shape remain unchanged | MIR-01 | existing contract tests pass unmodified |
| IR-08 | Parametric packed row insertion canonicalizes atomically; duplicate same-var/same-param scales merge, while same canonical cell with distinct params is typed not-packable and leaves state unchanged | MIR-02 | differential tests including duplicates, zero scales and distinct-param collision |
| IR-09 | Existing packed parametric objective construction is retrofitted to store eligible block dependencies without regressing current objective bulk semantics | MIR-02 | current P1C-2 tests unchanged + new dependency-block test |
| IR-10 | `ParamDepLayout` is an L2 witness; core validates it against post-canonical storage before accepting it; invalid witness is typed and atomic | MIR-02 | forged-layout rejection tests |
| IR-11 | Eligible families store `ParamDepBlock` and do not populate per-cell `param_positions`; dependency iteration/introspection remains semantically complete across block+sparse representations | MIR-02 | storage/counter tests; dependency-query equivalence |
| IR-12 | `set_parameters_bulk` preserves queue/commit transaction semantics and bulk-updates parameter storage atomically | MIR-02 | pending/commit/rollback tests; invalid block leaves state unchanged |
| IR-13 | Eligible bulk propagation performs no per-cell `param_positions`/overlay lookup or `ValueExpr` evaluation and emits one packed coefficient-patch batch per committed parameter block (not one per cell/dependency block) | MIR-02 | propagation counters; revision delta inspection |
| IR-14 | Bulk parameter commit emits a self-contained revision delta: one packed parameter-value change plus one packed coefficient-patch batch; no `ModelOp` may require mutable live-model p-base access | MIR-02 | replay test from retained delta only; snapshot equivalence |
| IR-15 | Scalar `set_parameter` remains correct for a parameter created inside a block | MIR-02 | scalar update vs bulk update differential |
| IR-16 | Overlay shadowing/deletion is respected during block propagation without converting eligible families to per-cell reverse-index lists | MIR-02 | shadow one cell, bulk reprice, snapshot/delta equality |
| IR-17 | Fresh packed cells append to base/p-base after prior solves; mutation of an existing logical cell uses overlay | MIR-02 | build→solve→append/mutate storage-location tests |
| IR-18 | L1 `View` uses span+shape+signed-strides+offset; slicing/transpose/negative-step edit metadata without per-cell allocation; symbolic views retain model owner | MIR-03 | allocation tests + cross-model typed-error test |
| IR-19 | Initial `CoeffView` set is `One`, `Scalar`, scaled `Dense`, `ScaledParam`; scalar scaling of Dense is zero-copy; covered families do not materialize `Vec<Affine>`/per-cell `ValueExpr` | MIR-03 | lowering corpus + allocation/counter assertions |
| IR-20 | Initial `ParamView * LinArray` fast subset is conservative (`One`/`Scalar` terms; `Zero`/`Scalar` constant); uncovered dense/param or param/param products fall to general symbolic path correctly | MIR-03 | fast/fallback differential corpus |
| IR-21 | `try_param_block_layout` is metadata-only, sink-aware and conservative; counterexamples are covered | MIR-03 | broadcast-over-rows eligible; broadcast-into-objective ineligible; non-monotone stride handled/fallback; interleaved disjoint views handled; two params one cell ineligible |
| IR-22 | Mixed constant+parametric rows with disjoint canonical cells can commit in one row batch; collisions into one canonical cell fall back/generalize without duplicate physical cells | MIR-03 | mixed-row differential tests |
| IR-23 | General symbolic path remains correct for every IR rejection | MIR-03 | normalized canonical snapshot differential corpus |
| IR-24 | Rust L1 BESS, transportation and min-cost-flow examples use no raw `VarId`/manual `LinExpr` in ordinary model code | MIR-04 | examples + grep gate |
| IR-25 | Labels/components are boundary metadata; different label types produce equal **normalized ordinal IR fingerprints**, not raw byte equality including owners/absolute IDs | MIR-04 | normalized fingerprint tests |
| IR-26 | Rust closure rules and Python decorator rules accumulate into CSR and bulk-commit once | MIR-05 | `rule_bulk_commits == 1`; journal-length tests |
| IR-27 | Python arrays/expressions wrap shared `roml::modeling` IR; no duplicated permanent array IR remains | MIR-06 | ownership/LOC audit; current LP-scale/MPC suites pass |
| IR-28 | Equivalent Rust and Python BESS formulations produce equal normalized ordinal-IR and semantic-journal fingerprints | MIR-06 | cross-language fingerprint fixture |
| IR-29 | Python ergonomics add `ConcreteModel`, `Set`/`RangeSet`, NumPy ingestion and pandas adapters with strict alignment | MIR-07 | ergonomics fixture + misalignment errors |
| IR-30 | `Template::bind` uses bulk parameter updates on one persistent session; structural shape change rebuilds | MIR-07 | 100 binds without rebuild; shape-change rebuild test |
| IR-31 | Flagship BESS300×96 benchmark passes hard counter/equivalence gates and frozen post-baseline performance targets | MIR-08 | raw data, commands, counters |
| IR-32 | No weakened validation/stale checks, no unjustified unsafe growth, required CI green and independent review clear | All | diff audit; `evidence/FINAL-REPORT.md` |

## Acceptance priority

Correct canonical-cell semantics, ownership and stale-ID behavior dominate performance. False-positive fast-path eligibility is a correctness defect. Conservative fallback is a performance finding, not a correctness failure, unless it violates a frozen MIR-08 performance gate.
