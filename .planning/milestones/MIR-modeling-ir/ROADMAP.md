# MIR Execution Roadmap

| Phase | End state | Depends on | Exit gate |
|---|---|---|---|
| MIR-00 | Current baseline timings, lowering path and reprice mechanics measured; temporary/diagnostic counters available | owner instruction | IR-01; `evidence/BASELINE.md` |
| MIR-01 | Trusted variable/parameter block allocation; packed variable-add delta; parameter-creation semantics preserved | MIR-00 | IR-02…IR-07 |
| MIR-02 | Packed parametric rows/objectives, validated L2 dependency layouts, block-native transactional propagation and self-contained packed deltas | MIR-01 | IR-08…IR-17 |
| MIR-03 | Shared model-owned strided array IR, conservative layout proof, mixed CSR builder | MIR-02 | IR-18…IR-23 |
| MIR-04 | Elegant native Rust L1 + label/component boundary | MIR-03 | IR-24, IR-25 |
| MIR-05 | Rust/Python rule builders bulk-commit CSR | MIR-03 | IR-26 |
| MIR-06 | Python arrays/expressions migrated to shared IR | MIR-03, MIR-05 | IR-27, IR-28 |
| MIR-07 | Python OO ergonomics + Template/bind | MIR-06 | IR-29, IR-30 |
| MIR-08 | Flagship qualification, frozen performance gates, independent review | MIR-07 | IR-31, IR-32 |

## Tranches

- **Tranche 1 — authorized:** MIR-00, MIR-01, MIR-02. Core-first, with only the delta/compiler/backend/diagnostic/Python benchmark-hook changes required to prove the core path. No Sets/ConcreteModel/pandas work.
- **Tranche 2:** MIR-03, MIR-04, MIR-05.
- **Tranche 3:** MIR-06, MIR-07, MIR-08.

One implementation phase at a time, with a review gate before advancing. Keep planning/governance commits separate from runtime implementation commits.

## Acceptance checkpoints

- **After MIR-00:** measured current BESS lowering/reprice path; explicitly report 28,800 changed parameters and 57,600 affected objective cells.
- **After MIR-01:** block allocation differential and journal/revision evidence; no core per-element name materialization.
- **After MIR-02:** use a direct/core block-shaped fixture (not a not-yet-implemented MIR-03 proof) to show transactional bulk repricing, block dependency storage, self-contained packed delta replay and shadowing correctness. The high-level BESS automatic-eligibility gate waits for MIR-03.
- **After MIR-03:** BESS parametric objective automatically produces eligible dependency blocks through metadata proof; `general_affine == 0` on the flagship formulation.
- **After MIR-04:** native Rust BESS/transport/network examples contain no raw-ID/manual-expression boilerplate in ordinary user code.
- **After MIR-06:** Rust/Python normalized IR and semantic-journal fingerprints match.
- **After MIR-08:** return exact heads, benchmark raw data, counter summary, CI/review status and residual limitations.

## Stop conditions

Stop and report if:
- any fast path differs from the general path on normalized canonical snapshot semantics;
- a claimed eligible dependency layout cannot be revalidated post-canonicalization;
- a retained `ModelOp` depends on mutable live-model storage;
- block propagation cannot preserve scalar update, overlay/shadowing or transaction semantics without per-cell dependency lists;
- cross-model ownership checks would need to be weakened;
- current MPY regression suites fail.

## Completion predicate

MIR is complete only when IR-01…IR-32 have execution evidence, the flagship qualification passes its hard counter/equivalence gates and frozen performance targets, required CI is green, independent review has no open P0/P1, and STATE matches repository reality.
