---
name: roml-ir-invariants
description: Invariants and checks for ROML's MIR shared modeling IR and block-native core (D-019). Load before editing coefficient storage, parameter propagation, delta compilation, modeling IR, or Python array/expression lowering.
---

# ROML IR Invariants (D-019)

Read `../../DESIGN.md` and D-019 first. Local convenience never overrides these rules; when a fast representation cannot prove them, fall back to the general path.

## Required invariants

1. Trusted spans only: no arbitrary `VarSpan`/`ParamSpan` construction.
2. No span epoch. One deleted block member does not invalidate siblings.
3. Symbolic arrays are model-owned; cross-model composition is rejected.
4. Views are metadata (`span + shape + signed strides + offset`), not gathered ID vectors.
5. Labels/component names stay outside expression nodes.
6. No per-element block names in core.
7. Variable block creation is one packed journal/delta op; parameter block creation preserves current non-journaled existence semantics.
8. Packed p-base cell = exactly one `scale × ParamId` at one canonical `(target,var)`.
9. Same-var/same-param duplicates may merge scales; distinct params into one canonical cell are not packable.
10. Eligible dependency families use L2 `ParamDepBlock`, not per-cell `param_positions`.
11. Eligibility is sink-aware canonical-cell injectivity + post-canonical strided-storage witness; stride sign/range overlap are not proofs.
12. Core validates the layout witness after canonicalization.
13. Bulk parameter updates preserve queue/commit/rollback semantics.
14. One committed parameter block emits one packed parameter-value change plus one packed coefficient-patch batch; the patch batch may contain multiple dependency blocks.
15. Retained `ModelOp`s are self-contained and cannot dereference mutable live-model packed positions.
16. Dependency queries and scalar updates remain correct for block-created parameters.
17. Fresh cells append to base/p-base even after solves; mutation of an existing cell uses overlay/shadowing.
18. Covered coefficient families do not materialize `Vec<Affine>`/per-cell `ValueExpr`.
19. `Dense` scalar scaling is represented by a scale factor, not a copied numeric buffer.
20. Rule APIs accumulate CSR and bulk-commit once.
21. Rust/Python equivalence uses normalized semantic fingerprints, not raw owner/ID bytes.
22. General symbolic fallback remains correct.

## Review checklist

- [ ] Did a change add a `Vec<VarId>` / `Vec<ParamId>` gather only to represent a slice/view? Remove it.
- [ ] Did a block path journal/hash/name once per entity? Batch it or justify why it is not eligible.
- [ ] Did parameter creation start emitting solver-facing add operations unlike scalar creation? Revert or amend D-019 explicitly.
- [ ] Does `ParamDepBlock` import/use L1 or Python view types? Move the persisted descriptor to L2/core.
- [ ] Can two distinct parameters reach the same canonical `(target,var)` in this packed path? Reject/fallback.
- [ ] Did you decide eligibility from stride sign, bounding ranges, or the var view without the sink? Wrong.
- [ ] Does core accept a caller layout without post-canonical validation? Wrong.
- [ ] Did you recompute eligibility at bind/update? Read stored dependency metadata.
- [ ] Does a retained `ModelOp` need the live model/p-base to interpret its cells? Wrong.
- [ ] Did bulk update bypass transaction/rollback behavior? Wrong.
- [ ] Does scalar `set_parameter` still work for one parameter inside a block?
- [ ] Does shadowing one p-base cell preserve the rest of the block fast path?
- [ ] Did `α * Dense` copy the dense buffer? Keep scale separate.
- [ ] Did raw byte comparison accidentally include `ModelInstanceId`/absolute IDs? Use normalized fingerprints.
- [ ] Do counterexample tests cover broadcast-across-rows, objective broadcast collision, non-monotone positive strides, interleaved disjoint views, and two-params-one-cell?
- [ ] Flagship counts: 28,800 price params; 57,600 affected objective cells.

## Diagnostics to preserve

Lowering: `numeric_bulk`, `parametric_bulk`, `general_affine`, `param_dep_blocks`, `param_positions_cells`, `rule_rows_accumulated`, `rule_bulk_commits`.

Propagation: `param_position_lookups`, `overlay_lookups`, `value_expr_evals`, `coefficient_patch_batches`.
