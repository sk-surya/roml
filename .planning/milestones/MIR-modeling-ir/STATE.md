# MIR State

**Objective:** shared model-owned ordinal array IR plus block-native construction and parameter propagation, so elegant Rust and Python formulations lower through one bulk path.

**Status:** MIR-00 (IR-01), MIR-01 (IR-02…IR-07) and MIR-02 (IR-08…IR-17) are complete and **merged to `main`** via PR #58 (`da7b383`), after the owner-review remediation and an independent green CI run (24 checks). MIR-02 is additionally guarded by the invariant mutation gauntlet (PR #61, `e215387`; **7/7 mutations killed**). **MIR-03 (IR-18…IR-23) is complete** at the Rust/shared-IR level (PR #63, head `77eab3a`; all gates and exact-head CI green). Deferred by design: MIR-04/05 user-facing mixed-row differential and MIR-06 Python BESS lowering.

**Planning base:** `main@c590692ace5446cc20c7eb91cb8fa0d594a054b0`.

**Tranche-1 delivery:** planning PR #57 (`6ccdb6a`); MIR-02 remediation head `6a3cdf2`; CI test-isolation fix `fe6ea03`; merged head `da7b383`. Work split out of the tranche: MPS INTORG fix + dead-code removal PR #59 (`b7debc5`), post-MIR test hardening PR #60 (`8f471b0`), mutation gauntlet PR #61 (`e215387`). The full pre-split coverage excursion is preserved on `coverage-hardening-after-mir02`.

**Execution base (MIR-03):** `main@e2153879c6a42adb95ff93a474b1e0741d9765d5`.

**Authorization:** owner authorized MIR ahead of further modeling ergonomics and M4. Tranche 1 (MIR-00/01/02) is accepted and merged. Tranche 2 (MIR-03/04/05) proceeds one phase at a time with a review gate before advancing. Publication/tags/releases are not authorized.

**Decisions:** D-019; no span epoch; owner-checked ordinal views; L2 persisted dependency layout; sink-aware metadata eligibility with core revalidation; append-only packed bases; block-native transactional repricing; labels/names outside the expression IR; `Template::bind` instead of `AbstractModel`; no macro DSL foundation.

**Blockers:** none.

**Next gate:** MIR-04 (Rust L1 ergonomics: BESS/transportation/min-cost-flow examples with no raw IDs; labels as boundary metadata; IR-24/IR-25). MIR-03's deferred items are tracked as MIR-04/05 and MIR-06 debt in `evidence/MIR-03-REPORT.md`.

| Phase | State | Evidence |
|---|---|---|
| MIR-00 | complete (IR-01) | `evidence/BASELINE.md`; `evidence/baseline-mir-bess-sample.json`; `tests/mir00_baseline_characterization.rs` |
| MIR-01 | complete (IR-02…IR-07) | `evidence/MIR-01-REPORT.md`; `src/bulk.rs`; `tests/mir01_block_allocation.rs` |
| MIR-02 | complete + merged (IR-08…IR-17) | `evidence/MIR-02-REPORT.md`; `evidence/MIR-02-MUTATION-REPORT.md`; `tests/mir02_parametric_blocks.rs`; `tests/mir02_parametric_rows.rs`; `tests/mir02_remediation.rs`; `tests/mir02_backend_batching.rs`; `roml-highs/tests/mir02_bess_batching.rs` |
| MIR-03 | complete (IR-18…IR-23) | `evidence/MIR-03-REPORT.md`; `MIR-03-PLAN.md`; `src/modeling/{view,coeff,eligibility,builder}.rs`; `Model::set_linear_objective_from_linarray`; `Model::add_rows_from_plan`; `MixedRowBlock` / `Change::BulkMixedRows` / `ModelOp::AddMixedRows`; `model::mir03_*` tests |
| MIR-04 | in progress (IR-24, IR-25): M4-1 array handles + M4-2 objective expression composition done | `MIR-04-PLAN.md`; `src/modeling/array.rs`; `tests/mir04_arrays.rs`; `tests/mir04_expr.rs` |
| MIR-05 | not started | — |
| MIR-06 | not started | — |
| MIR-07 | not started | — |
| MIR-08 | not started | — |
