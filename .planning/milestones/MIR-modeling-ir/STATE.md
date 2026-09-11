# MIR State

**Objective:** shared model-owned ordinal array IR plus block-native construction and parameter propagation, so elegant Rust and Python formulations lower through one bulk path.

**Status:** MIR-00 complete (IR-01); MIR-01 complete (IR-02…IR-07); MIR-02 complete (IR-08…IR-17); MIR-03 next.

**Planning base:** `main@c590692ace5446cc20c7eb91cb8fa0d594a054b0`.

**Execution base (MIR-00):** `main@c590692ace5446cc20c7eb91cb8fa0d594a054b0`; implementation branch based on `docs/mir-modeling-ir-plan@3363ec34227fe6676cc8df467bf58fd1e03fb319`.

**Authorization:** owner authorized MIR ahead of further modeling ergonomics and M4. Tranche 1 (MIR-00/01/02) is authorized after this planning bootstrap. Publication/tags/releases are not authorized.

**Decisions:** D-019; no span epoch; owner-checked ordinal views; L2 persisted dependency layout; sink-aware metadata eligibility with core revalidation; append-only packed bases; block-native transactional repricing; labels/names outside the expression IR; `Template::bind` instead of `AbstractModel`; no macro DSL foundation.

**Blockers:** none known for MIR-00. Runtime facts must be refreshed by the executor.

**Next gate:** MIR-03 — shared `roml::modeling` strided array IR, the
sink-aware `try_param_block_layout` proof, and the mixed CSR builder
(IR-18…IR-23). Tranche 1 (MIR-00/01/02) is now implemented.

| Phase | State | Evidence |
|---|---|---|
| MIR-00 | complete (IR-01) | `evidence/BASELINE.md`; `evidence/baseline-mir-bess-sample.json`; `tests/mir00_baseline_characterization.rs` |
| MIR-01 | complete (IR-02…IR-07) | `evidence/MIR-01-REPORT.md`; `src/bulk.rs`; `tests/mir01_block_allocation.rs` |
| MIR-02 | complete (IR-08…IR-17) | `evidence/MIR-02-REPORT.md`; `tests/mir02_parametric_blocks.rs`; `tests/mir02_parametric_rows.rs` |
| MIR-03 | not started | — |
| MIR-04 | not started | — |
| MIR-05 | not started | — |
| MIR-06 | not started | — |
| MIR-07 | not started | — |
| MIR-08 | not started | — |
