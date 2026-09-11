# MIR-02 invariant mutation gauntlet

Owner-directed defense of the MIR-02 architecture: instead of more line
coverage, break each named invariant deliberately and prove the tests catch it.
Harness: [`scripts/mir_mutation_gauntlet.py`](../../../../scripts/mir_mutation_gauntlet.py)
— a hand-crafted semantic gauntlet (not `cargo-mutants` operator soup). For each
mutation it first requires the defending test(s) to pass unmutated, applies the
source edit(s), requires the test run to **fail**, then reverts and re-checks the
tree is clean.

Run (no HiGHS build needed — all defenders are `roml`):

```bash
CARGO_TARGET_DIR=<dir> python3 scripts/mir_mutation_gauntlet.py \
    --out .planning/milestones/MIR-modeling-ir/evidence/mir-mutation-report.json
```

## Result — 7/7 killed

| Mutation | Invariant | Defending test | Result |
|---|---|---|---|
| disable both ownership checks | a cell is owned by at most one `ParamDepBlock` | `model::coefficient::…::duplicate_cell_positions_in_one_witness_are_rejected` | KILLED |
| disable both ownership checks | (same, objective layout path) | `mir02_remediation::overlapping_witnesses_are_rejected_atomically` | KILLED |
| drop `propagate_packed_positions_span` | bulk update drives non-eligible packed fallback | `mir02_remediation::bulk_update_propagates_non_eligible_packed_positions` | KILLED |
| column-major traversal in `StridedMap::get` | frozen row-major ordinal convention | `bulk::tests::strided_map_row_major_ordinals_and_zero_stride` | KILLED |
| expand `SetObjectiveCosts` into scalar ops | packed objective batching through Backend IR | `mir02_backend_batching::eligible_reprice_compiles_to_one_packed_cost_op` | KILLED |
| stop skipping `p_shadowed` in `propagate_packed_span` | shadowed packed cells are skipped | `mir02_edge_cases::fully_shadowed_family_emits_value_change_without_patch_batch` | KILLED |
| replace the symbolic `ValueExpr` with a constant on replay | `ReferenceBackend` preserves the parameterized cell | `mir02_remediation::reference_replay_preserves_symbolic_patch_cells` | KILLED |

Raw outcomes: [`mir-mutation-report.json`](mir-mutation-report.json) (`verdict:
all-killed`).

## Findings

1. **The overlap invariant is enforced twice (defense in depth).** Disabling
   only `CoefficientIndex::validate_param_dep_blocks` is caught by
   `validate_objective_dep_layout`, and vice-versa; the mutant that represents
   "allow overlapping `ParamDepBlocks`" must disable **both**, and then both the
   in-crate duplicate-cell test and the cross-witness integration test fail.
   (The two near-identical validators are a future consolidation candidate, not
   a correctness gap.)
2. **The shadow-skip defender is the edge-case test, not the same-named one.**
   `mir02_parametric_blocks::shadowed_cell_is_skipped_and_stays_correct` is a
   final-snapshot equivalence test and does **not** by itself detect the
   removed skip; `mir02_edge_cases::fully_shadowed_family_emits_value_change_without_patch_batch`
   does. The invariant is defended; the first test's name over-claims and could
   be tightened or renamed in a later cleanup.
3. Every other named mutation is killed by exactly the expected test, with no
   additional tests required.

## Scope

Test/evidence only — no production behavior is changed. The script edits and
reverts sources in a throwaway working tree and refuses to run when the tree is
dirty.

## CI wiring (proposed, not included)

The existing `Mutation score` CI job is currently `skipping`. This gauntlet
could run there (scoped to `-p roml`) once a per-mutation timeout budget is set;
it is intentionally left out of this change to keep the PR reviewable.
