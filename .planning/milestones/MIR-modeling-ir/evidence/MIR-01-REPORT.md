# MIR-01 Report — Trusted Block Allocation

**Phase:** MIR-01
**Requirements:** IR-02 … IR-07
**Head:** `phase-mir-tranche1` (implementation commits `7cd6113`, `c226b7e`)
**Date:** 2026-09-11

## What landed

- `src/bulk.rs` — opaque `VarSpan`/`ParamSpan` (private fields, no public
  `(start,len)` constructor), `BlockBounds`/`BlockBoundsOwned`, and the
  self-contained `VariableBlock` payload.
- `IdArena::allocate_block` (reserve once, append sequentially, one shared
  fresh generation); `VariableStore::add_block` / `ParameterStore::add_block`.
- `Model::add_variable_block` and `Model::add_parameter_block`.
- `Change::VariableBlockAdded` and `ModelOp::AddVariableBlock`.
- Adapter projection: identity compiler, backend compiler expansion to
  per-column `BackendOp::AddVariable`, reference backend expansion, and
  MOSEK/Xpress member-wise application.

## Requirement evidence

| ID | Evidence |
|---|---|
| IR-02 | `src/bulk.rs` `compile_fail` doctest (`VarSpan` private fields); no public constructor. |
| IR-03 | `tests/mir01_block_allocation.rs::variable_block_emits_one_packed_change_and_op` — n=1 and n=100 000 each produce **one** delta batch with **one** `AddVariableBlock` op. |
| IR-04 | `parameter_block_matches_scalar_creation_revision_semantics` and `parameter_block_rejects_non_finite_atomically` — zero journal records, revision unchanged, atomic rejection. |
| IR-05 | `variable_block_materializes_no_names_and_stales_per_member` — `variable_name` returns `None` for every member; scalar named API still returns `Some("x")`. |
| IR-06 | Same test — removing one member leaves the other two live and the removed member absent. |
| IR-07 | `variable_block_participates_in_constant_row_bulk` plus the unmodified existing `add_linear_rows_bulk` contract tests (full suite green). |

Additional IR-03 evidence:
`variable_block_matches_scalar_canonical_state` (block vs scalar snapshot
equality) and `variable_block_delta_replays_without_the_live_model` (retained
delta applied to a fresh `ReferenceBackend` equals a snapshot rebuild).

## Exact commands and results

```text
cargo fmt --all -- --check                                        clean
cargo clippy -p roml -p roml-highs -p roml-python --all-targets \
  --features roml-highs/bundled -- -D warnings                    clean
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps            clean
cargo nextest run -p roml -p roml-highs --features roml-highs/bundled
  → 1512 passed, 4 skipped
cargo nextest run -p roml --test mir01_block_allocation
  → 8 passed
```

## Residual risk / not verified

- `roml-mosek` and `roml-xpress` cannot be type-checked in this environment
  (proprietary SDK build scripts). Their new `Change::VariableBlockAdded` arms
  are `rustfmt`-parsed; type verification is deferred to the backend CI that
  has the SDKs.
- `VariableSpan::ids()` is public so adapters can expand a packed block; this
  exposes member identities but not the ability to forge a span.
- No automatic block use in the Python/vars path yet: `vars()` still adds
  scalars one at a time. Rewiring it onto `add_variable_block` is a later
  (MIR-04/MIR-06) concern.

## Contract MIR-02 may consume

`Model::add_variable_block(n, ty, BlockBounds)` and
`Model::add_parameter_block(&[f64])` are the trusted block allocators.
`VariableBlock` is self-contained and adapter-expandable. `VarSpan`/`ParamSpan`
reconstruct members via `ids()` (variables) with per-member arena validation.
