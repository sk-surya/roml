# MIR State

**Objective:** shared model-owned ordinal array IR plus block-native construction and parameter propagation, so elegant Rust and Python formulations lower through one bulk path.

**Status:** MIR-00 (IR-01), MIR-01 (IR-02…IR-07) and MIR-02 (IR-08…IR-17) are implemented. MIR-02 was rejected on owner review 2026-09-11 and remediated; tranche 1 is **not accepted** and awaits independent re-review. MIR-03 was not started.

**Planning base:** `main@c590692ace5446cc20c7eb91cb8fa0d594a054b0`.

**Execution base (MIR-00):** `main@c590692ace5446cc20c7eb91cb8fa0d594a054b0`; implementation branch based on `docs/mir-modeling-ir-plan@3363ec34227fe6676cc8df467bf58fd1e03fb319`.

**Authorization:** owner authorized MIR ahead of further modeling ergonomics and M4. Tranche 1 (MIR-00/01/02) is authorized after this planning bootstrap. Publication/tags/releases are not authorized.

**Decisions:** D-019; no span epoch; owner-checked ordinal views; L2 persisted dependency layout; sink-aware metadata eligibility with core revalidation; append-only packed bases; block-native transactional repricing; labels/names outside the expression IR; `Template::bind` instead of `AbstractModel`; no macro DSL foundation.

**Blockers:** none known for MIR-00. Runtime facts must be refreshed by the executor.

**Next gate:** independent re-review of PR #58 (`phase-mir-tranche1`),
including the MIR-02 remediation and the direct persistent-HiGHS solve gate.
MIR-03 (shared `roml::modeling` strided array IR, sink-aware
`try_param_block_layout`, mixed CSR builder) is **not started** and must not
begin until tranche 1 is accepted.

| Phase | State | Evidence |
|---|---|---|
| MIR-00 | complete (IR-01) | `evidence/BASELINE.md`; `evidence/baseline-mir-bess-sample.json`; `tests/mir00_baseline_characterization.rs` |
| MIR-01 | complete (IR-02…IR-07) | `evidence/MIR-01-REPORT.md`; `src/bulk.rs`; `tests/mir01_block_allocation.rs` |
| MIR-02 | remediated (IR-08…IR-17) — pending independent re-review | `evidence/MIR-02-REPORT.md` (incl. remediation section); `tests/mir02_parametric_blocks.rs`; `tests/mir02_parametric_rows.rs`; `tests/mir02_remediation.rs`; `tests/mir02_backend_batching.rs`; `roml-highs/tests/mir02_bess_batching.rs` |
| MIR-03 | not started | — |
| MIR-04 | not started | — |
| MIR-05 | not started | — |
| MIR-06 | not started | — |
| MIR-07 | not started | — |
| MIR-08 | not started | — |
