# MIR State

**Objective:** shared model-owned ordinal array IR plus block-native construction and parameter propagation, so elegant Rust and Python formulations lower through one bulk path.

**Status:** MIR-00 complete (IR-01 evidenced); MIR-01 in progress (IR-02, IR-04 evidenced; variable-block packed Change/ModelOp pending).

**Planning base:** `main@c590692ace5446cc20c7eb91cb8fa0d594a054b0`.

**Execution base (MIR-00):** `main@c590692ace5446cc20c7eb91cb8fa0d594a054b0`; implementation branch based on `docs/mir-modeling-ir-plan@3363ec34227fe6676cc8df467bf58fd1e03fb319`.

**Authorization:** owner authorized MIR ahead of further modeling ergonomics and M4. Tranche 1 (MIR-00/01/02) is authorized after this planning bootstrap. Publication/tags/releases are not authorized.

**Decisions:** D-019; no span epoch; owner-checked ordinal views; L2 persisted dependency layout; sink-aware metadata eligibility with core revalidation; append-only packed bases; block-native transactional repricing; labels/names outside the expression IR; `Template::bind` instead of `AbstractModel`; no macro DSL foundation.

**Blockers:** none known for MIR-00. Runtime facts must be refreshed by the executor.

**Next gate:** finish MIR-01 — the Model variable-block API with one packed
`Change::VariableBlockAdded` and one self-contained `ModelOp::AddVariableBlock`,
plus IR-03/IR-05/IR-06/IR-07 evidence — then MIR-02.

| Phase | State | Evidence |
|---|---|---|
| MIR-00 | complete (IR-01) | `evidence/BASELINE.md`; `evidence/baseline-mir-bess-sample.json`; `tests/mir00_baseline_characterization.rs`; `python/benchmarks/bench_mir_bess.py` |
| MIR-01 | in progress (IR-02, IR-04) | `src/bulk.rs` (compile_fail opacity); `src/id/arena.rs`; `tests/mir01_block_allocation.rs`; variable-block packed op pending |
| MIR-02 | not started | — |
| MIR-03 | not started | — |
| MIR-04 | not started | — |
| MIR-05 | not started | — |
| MIR-06 | not started | — |
| MIR-07 | not started | — |
| MIR-08 | not started | — |
