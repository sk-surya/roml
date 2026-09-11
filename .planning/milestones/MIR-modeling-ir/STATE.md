# MIR State

**Objective:** shared model-owned ordinal array IR plus block-native construction and parameter propagation, so elegant Rust and Python formulations lower through one bulk path.

**Status:** MIR-00 complete (IR-01 evidenced); MIR-01 complete (IR-02…IR-07 evidenced); MIR-02 next.

**Planning base:** `main@c590692ace5446cc20c7eb91cb8fa0d594a054b0`.

**Execution base (MIR-00):** `main@c590692ace5446cc20c7eb91cb8fa0d594a054b0`; implementation branch based on `docs/mir-modeling-ir-plan@3363ec34227fe6676cc8df467bf58fd1e03fb319`.

**Authorization:** owner authorized MIR ahead of further modeling ergonomics and M4. Tranche 1 (MIR-00/01/02) is authorized after this planning bootstrap. Publication/tags/releases are not authorized.

**Decisions:** D-019; no span epoch; owner-checked ordinal views; L2 persisted dependency layout; sink-aware metadata eligibility with core revalidation; append-only packed bases; block-native transactional repricing; labels/names outside the expression IR; `Template::bind` instead of `AbstractModel`; no macro DSL foundation.

**Blockers:** none known for MIR-00. Runtime facts must be refreshed by the executor.

**Next gate:** MIR-02 — packed parametric rows/objectives, validated L2
dependency layouts, and block-native transactional propagation with
self-contained packed deltas (IR-08…IR-17).

| Phase | State | Evidence |
|---|---|---|
| MIR-00 | complete (IR-01) | `evidence/BASELINE.md`; `evidence/baseline-mir-bess-sample.json`; `tests/mir00_baseline_characterization.rs`; `python/benchmarks/bench_mir_bess.py` |
| MIR-01 | complete (IR-02…IR-07) | `evidence/MIR-01-REPORT.md`; `src/bulk.rs`; `src/model/mod.rs`; `tests/mir01_block_allocation.rs` |
| MIR-02 | in progress (IR-08…IR-17) | — |
| MIR-03 | not started | — |
| MIR-04 | not started | — |
| MIR-05 | not started | — |
| MIR-06 | not started | — |
| MIR-07 | not started | — |
| MIR-08 | not started | — |
