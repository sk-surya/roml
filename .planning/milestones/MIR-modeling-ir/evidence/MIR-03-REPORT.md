# MIR-03 Report — shared modeling IR (foundation, post-review remediation)

**Phase:** MIR-03. **Requirements:** IR-18…IR-23.
**Branch:** `phase-mir-03`. **Execution base:**
`main@43887eece93e77bcd2581bd94a79e084502c6e20`.

**Status: in progress / architecture review.** The isolated IR foundation was
remediated against owner review of `0cae40a`. **No model/compiler wiring has
been attempted** (deliberately deferred until this review passes). The exit gate
(BESS automatic eligibility, `general_affine == 0`) is not met.

## Review remediation

1. **`try_param_block_layout` redesigned around `r -> (target(r), var_j(r))`.**
   The sink now supplies target/layout metadata ([`SinkMap`] with compact
   [`TargetRun`]s + per-term packed bases); the variable comes from each
   `Term.vars`. The preconstructed `SinkCells::cell(r) -> CanonicalCell` was
   removed. A BESS-like two-term objective regression (one objective target,
   disjoint charge/discharge `VarView`s, one shared `ParamView`, two
   `ScaledParam` terms) is eligible and yields two dependency families.
2. **Metadata-only proof.** No `Vec<CanonicalCell>` enumeration. Eligibility is
   conservative algebra over the target runs, each term's `VarView` map, the
   parameter maps and the packed bases: pairwise-disjoint variable spans; per-run
   non-overlapping packed ranges; for runs longer than one ordinal, contiguous
   dense (row-major canonical) variable/parameter views with positive parameter
   strides. A one-ordinal run is trivially injective and admits a broadcast
   (zero-stride) variable/parameter. False negative → fallback; false positive is
   a defect.
3. **Honest row targets.** The witness `row` is `Some(target)` for a row run and
   `None` only for an objective run; a family spanning several row targets is
   **split into per-target blocks**. The previous test that used targets `0..N`
   while emitting `row = None` is gone. `broadcast_over_rows_uses_honest_row_targets`
   asserts the witness rows agree with the sink targets.
4. **`LinArray::new` validates all coefficient-side metadata**: `ScaledParam`
   parameter owner (coefficient *and* constant) must equal the array owner;
   `Dense` values and `ScaledParam` parameter shapes must match the array shape
   (coefficient *and* constant). Cross-model parameter composition fails before
   any ID reconstruction. Direct regressions construct malicious public
   `CoeffView`/`ConstantView` values.
5. **Checked `View` transforms.** `slice`/`reverse` use checked
   `usize -> isize`, multiplication, addition and stride negation; overflow is a
   typed `ViewError::IndexOverflow`, never a wrap or debug panic. Reversing a
   zero-length axis is a no-op (offset unchanged). Tests cover huge start/dim,
   `isize::MIN` negation, offset multiply/add overflow, and zero-length reverse.
6. **`NumView` scope: option A.** `NumView` is now a validated strided view
   (owned/shared buffer + `View<()>`) with `new` (buffer-range-validated from
   metadata in O(rank)), metadata-only `slice`/`reverse`/`transpose`, and
   zero-copy sharing. The full IR-19 representation is now documented, not just
   contiguous.

## IR-21 regressions

| Case | Expectation | Test |
|---|---|---|
| BESS two-term objective | eligible, two families | `bess_objective_two_terms_produce_two_families` |
| broadcast over rows | eligible, honest `row=Some` | `broadcast_over_rows_uses_honest_row_targets` |
| broadcast into one objective cell | ineligible | `broadcast_into_one_objective_cell_is_ineligible` |
| overlapping two-term spans | ineligible | `overlapping_two_term_spans_are_ineligible` |
| interleaved / overlapping packed bases | fallback | `interleaved_or_overlapping_packed_bases_fall_back` |
| non-monotone parameter stride | fallback | `non_monotone_parameter_stride_falls_back` |
| zero stride over a multi-ordinal run | fallback | `zero_stride_over_a_multi_ordinal_run_falls_back` |
| malformed sink cover / duplicate rows | typed rejection | `sink_map_rejects_bad_run_covers_and_duplicate_rows` |
| cross-model parameter coefficient | typed rejection | `builder::tests::cross_model_parameter_coefficient_is_rejected`, `coeff::tests::linarray_rejects_foreign_and_misshaped_coefficient_metadata` |

## Verification (remediation head)

```text
cargo fmt --all -- --check                              clean
cargo clippy -p roml --all-targets -- -D warnings       clean
cargo nextest run -p roml                               1492 passed, 4 skipped
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps  clean
```

26 in-crate `modeling::*` tests.

## Residual / deliberately not done

1. **Core post-canonical revalidation of the honest witness is not yet
   exercised.** The witnesses are unit-tested for internal consistency with the
   sink targets; feeding them through `validate_objective_dep_layout` /
   `validate_param_dep_blocks` requires the model/compiler integration, which is
   out of scope until this review passes.
2. **IR-23 model-level fallback differential** (fast vs general normalized
   snapshots) is not written.
3. Broadcasting beyond exact-shape match and reductions/matmul topology
   metadata remain outside the initial conservative subset.
4. `roml-mosek`/`roml-xpress` remain untestable locally (proprietary SDKs).
