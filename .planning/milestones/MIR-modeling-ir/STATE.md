# MIR State

**Objective:** shared model-owned ordinal array IR plus block-native construction and parameter propagation, so elegant Rust and Python formulations lower through one bulk path.

**Status:** planning packet reviewed and bootstrapped; MIR-00 not started.

**Planning base:** `main@c590692ace5446cc20c7eb91cb8fa0d594a054b0`.

**Authorization:** owner authorized MIR ahead of further modeling ergonomics and M4. Tranche 1 (MIR-00/01/02) is authorized after this planning bootstrap. Publication/tags/releases are not authorized.

**Decisions:** D-019; no span epoch; owner-checked ordinal views; L2 persisted dependency layout; sink-aware metadata eligibility with core revalidation; append-only packed bases; block-native transactional repricing; labels/names outside the expression IR; `Template::bind` instead of `AbstractModel`; no macro DSL foundation.

**Blockers:** none known for MIR-00. Runtime facts must be refreshed by the executor.

**Next gate:** MIR-00 baseline evidence on the then-current `main`, including the measured current path for `rm.sum(price * (discharge - charge))` and a profile of 28,800-price-parameter repricing / 57,600 affected objective cells.

| Phase | State | Evidence |
|---|---|---|
| MIR-00 | not started | — |
| MIR-01 | not started | — |
| MIR-02 | not started | — |
| MIR-03 | not started | — |
| MIR-04 | not started | — |
| MIR-05 | not started | — |
| MIR-06 | not started | — |
| MIR-07 | not started | — |
| MIR-08 | not started | — |
