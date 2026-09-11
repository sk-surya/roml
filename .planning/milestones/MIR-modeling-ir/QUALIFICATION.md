# MIR Qualification

## Flagship benchmark — BESS300×96 repricing

Model: 300 batteries × 96 periods. `charge` and `discharge` each have 28,800 variables; `energy` has 29,100 values over 97 periods. Objective:

```text
maximize dt * sum(price * (discharge - charge))
```

`price` is one 28,800-element `ParamView`. It drives 57,600 objective coefficient cells (one charge and one discharge cell per price parameter). Use equivalent Rust L1 and Python vectorized formulations with a persistent HiGHS session.

Procedure:

```text
build
solve
repeat 100 times:
    queue set_parameters_bulk(price_span, new_prices)   # 28,800 parameter values
    commit / incremental sync
    solve
```

### Hard lowering gates

```text
lowering.general_affine        == 0
lowering.param_dep_blocks       > 0
lowering.param_positions_cells == 0
```

### Hard propagation gates per committed price block

```text
propagation.param_position_lookups == 0
propagation.overlay_lookups        == 0
propagation.value_expr_evals       == 0
propagation.coefficient_patch_batches == 1
```

Revision contents must contain one packed parameter-value change plus one packed coefficient-patch batch. The patch batch may contain multiple dependency-family patches (for example charge and discharge); the gate is **not** "one dependency block" or "one native solver call."

The packed coefficient `ModelOp` must be self-contained and replayable without consulting mutable p-base state.

### Equivalence gates

- Every incremental result matches a fresh-rebuild solve within the declared tolerance.
- Snapshot projection and retained-delta replay produce the same normalized canonical/backend state.
- A scalar update to a single parameter from the same block matches the corresponding one-element bulk update.
- A shadowed eligible cell is skipped by block propagation and remains correct through overlay semantics.

## Construction / IR microbenchmarks

| Fixture | Measure |
|---|---|
| `add_variable_block` 100k / 1M | validation, arena growth, journal length, allocation count |
| `add_parameter_block` 100k / 1M | validation/store cost; no creation journal growth |
| existing `add_linear_rows_bulk` sparse-1M | unchanged regression baseline |
| packed parametric rows 100k | construction, canonicalization, journal shape |
| eligible parameter block reprice 28,800 params / 57,600 cells | arithmetic/value packing, dependency lookup counts, delta size |
| slice/transpose of 1M-element view | allocation count and latency |
| Rust rule 10k rows / Python rule 10k rows | expression construction vs one CSR commit |

## Performance targets

MIR-00 records the baseline and exact measurement method before these become blocking numeric gates.

| Path | Goal versus raw Level-2 bulk |
|---|---|
| Rust L1 vectorized numeric/parametric | ≤ 1.10× |
| Rust closure rules | ≤ 1.50× |
| Python vectorized/high-level | ≤ 1.30× |
| Python rule decorator | Python-bound; core insertion share ≤ 10% wall time |
| Eligible 28,800-param reprice | O(n) strided arithmetic/value packing; no per-cell hash/journal machinery |

A target miss never justifies weaker semantics.
