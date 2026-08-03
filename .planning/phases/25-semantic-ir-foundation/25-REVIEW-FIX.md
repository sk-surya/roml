---
phase: 25-semantic-ir-foundation
fixed_at: 2026-08-02T23:20:00Z
review_path: .planning/phases/25-semantic-ir-foundation/25-REVIEW.md
iteration: 1
findings_in_scope: 11
fixed: 11
skipped: 0
status: all_fixed
---

# Phase 25: Code Review Fix Report

**Fixed at:** 2026-08-02T23:20:00Z
**Source review:** `.planning/phases/25-semantic-ir-foundation/25-REVIEW.md`
**Iteration:** 1

**Summary:**
- Findings in scope: 11 (2 critical, 6 warning, 3 info)
- Fixed: 11
- Skipped: 0

**Verification ran in:** the `phase-roml-P25-semantic-ir-foundation` worktree
(the task checkout). `workflow.use_worktrees` is `false`, so no isolated fixer
worktree was created — all edits/commits landed directly on this branch and all
gate numbers below are reproducible from it.

## Fixed Issues

### CR-01: DeltaBatch reconstructed `set` uses pre-adjustment bounds

**Files modified:** `src/delta.rs`, `tests/semantic_ir.rs`
**Commit:** `5c748e1`
**Applied fix:** `reconstruct_function_entries` now folds the LAST same-batch
`ModelOp::SetConstraintBounds { con }` op's bounds into the reconstructed
`ScalarSet` for each `AddConstraint` (mirroring the final-activity fold in
`reconstruct_construct_entries`), so the ordinary constant-folding path
(`add_constraint((x + 3.0).le(5.0))`) produces a delta entry whose set equals
the model's canonical folded set (`LessEqual(2.0)`) instead of the
pre-adjustment `LessEqual(5.0)`. Logic change validated by two new TDD tests
(`delta_set_reflects_bounds_folded_from_expression_constant`,
`delta_set_reflects_folded_bounds_for_ge_and_between`) that assert the exact
folded set (`le`/`ge`/`between`) and equality with `constraint_function(con).set`;
both failed before the fix (reported `LessEqual(5.0)`/`GreaterEqual(5.0)`) and
pass after.

### CR-02: Real solve path never binds the model's lineage/instance into `SolveMetadata`

**Files modified:** `src/solver/facade.rs`, `tests/lineage_metadata.rs`
**Commit:** `37e67f0`
**Applied fix:** `normalize_result` gained `model_lineage`/`model_instance`
parameters; `SolverSession::solve_with` threads `model.lineage()` and
`model.instance()` into it, and the `SolveMetadata` literal sets the fields
explicitly instead of `..SolveMetadata::default()` (which allocates fresh
unrelated counter ids on every solve). The misleading "filled in when a model
binds the solution" comment was removed — there is no binding step. Logic
change validated by a new TDD test
(`real_solve_binds_model_lineage_and_instance_into_metadata`) that drives the
real `SolverSession::solve` path through a minimal session-trait backend and
asserts `solution.metadata().model_lineage == model.lineage()` (and instance);
it failed before (solution `ModelLineageId(43)` vs model `ModelLineageId(5)`).

### WR-01: `constraint_expression` term order is nondeterministic and disagrees with snapshot function entries

**Files modified:** `src/expr/linear.rs`, `src/snapshot.rs`, `src/delta.rs`, `src/model/mod.rs`, `tests/semantic_ir.rs`
**Commit:** `5c748e1`
**Applied fix:** `constraint_expression` sorts reconstructed terms by `var`
(`VarId` implements `Ord`) instead of iterating the `HashSet`; the snapshot and
delta reconstructions sort their `ScalarFunction::Linear` terms by `var` the
same way so every reconstruction agrees in order. New TDD test
(`constraint_expression_term_order_matches_snapshot`) asserted canonical vs
snapshot expression equality and failed before (canonical `[var1, var0]` vs
snapshot `[var0, var1]`); it passes after.

### WR-02: The "transitional legacy field" invariant checks are tautological

**Files modified:** `src/delta.rs`, `src/snapshot.rs`, `src/model/mod.rs`
**Commit:** `5c748e1`
**Applied fix:** Removed the self-comparison `debug_assert_eq!(set, ScalarSet::from(bounds))`
in `src/snapshot.rs` and `src/delta.rs` (the set was just computed from those
same bounds). The real cross-check in `Model::take_snapshot` now additionally
asserts `entry.function == fc.function` (function expression equality), which
is meaningful now that WR-01 makes both sides deterministic.

### WR-03: `allocate_id` wraps the counter to 0 after `IdentityOverflow`, re-issuing id 1

**Files modified:** `src/identity.rs`
**Commit:** `a845968`
**Applied fix:** `allocate_id` uses a saturating `fetch_update` whose closure
returns `None` at `pre == u64::MAX`, so the counter stays saturated and every
subsequent call returns `Err(IdentityOverflow)` (no wrap, no id reuse). New
TDD test (`allocate_id_saturates_on_overflow_without_wrapping`) failed before
(second call returned `Ok(1)` after the wrap) and passes after.

### WR-04: Inconsistent overflow handling — `Model::default`/`clone` and `SolveMetadata::default` panic, construct allocation is typed

**Files modified:** `src/identity.rs`
**Commit:** `a845968`
**Applied fix:** Documented the panic boundary explicitly in the identity.rs
module doc: counters saturate (WR-03), the infallible constructors
(`Model::default`/`Clone`, `SolveMetadata::default`) panic only when a family
counter is truly exhausted, and the fallible construct APIs return the typed
`IdentityOverflow`. A panicking constructor can never re-issue an id.

### WR-05: Metadata accepts stale entities and is never cascaded on removal

**Files modified:** `src/model/mod.rs`, `tests/lineage_metadata.rs`
**Commit:** `839361f`
**Applied fix:** `set_metadata` now returns `Result<(), ModelError>` and rejects
a stale/removed entity with the entity's typed `*NotFound` error (the entity
stores are the liveness authority). `remove_variable`/`remove_constraint`/
`remove_objective` now cascade `self.metadata.remove(...)` before pushing the
changelog event, so add/remove churn leaves no orphaned metadata. Existing
`set_metadata` callers updated for the new `Result` signature. New tests
(`set_metadata_rejects_stale_entities`, `removing_entity_cascades_metadata`)
could not compile against the old unit-returning signature and pass after.

### WR-06: Removing a construct after attaching metadata produces a false `validate_invariants` violation

**Files modified:** `src/model/mod.rs`, `tests/semantic_ir.rs`
**Commit:** `839361f`
**Applied fix:** `remove_construct` now `self.metadata.remove(&EntityRef::Construct(id))`
before pushing the changelog event. New test
(`construct_remove_cascades_metadata_and_invariants_pass`) attaches metadata to
a construct, removes it, and asserts `validate_invariants` passes.

### IN-01: `Change::affects_solver` is dead code

**Files modified:** `src/model/changelog.rs`
**Commit:** `4335e2d`
**Applied fix:** Removed the dead `Change::affects_solver` method (zero callers
workspace-wide; the sync-path filtering decision belongs to P26).

### IN-02: The identity overflow path is untested and untestable

**Files modified:** `src/identity.rs`
**Commit:** `a845968`
**Applied fix:** Added a `#[cfg(test)]` `seed_counter_for_test` seam to the
`define_opaque_id!` macro (the macro now declares a module-level counter
static) plus a dedicated test-only id family (`TestOverflowId`) so the overflow
branch is exercised without 2^64 allocations and without racing the shared
families under parallel tests. Test `family_allocate_saturates_on_overflow_without_reissuing`
seeds `u64::MAX` and asserts the first and second calls are both
`Err(IdentityOverflow)`.

### IN-03: Construct-rebuild test relies on fragile ordering assumptions

**Files modified:** `tests/semantic_ir.rs`
**Commit:** `6bb6476`
**Applied fix:** `construct_store_survives_rebuild` now tracks each rebuilt
entry's fresh id and looks each one up by id in the rebuilt snapshot, asserting
the full `ConstructEntry` (kind + activity) matches the original instead of the
index-based double-negated `[0]` comparison.

## Skipped Issues

None — all findings were fixed.

## Gate results (main checkout)

- `cargo test -p roml --all-targets` — **594 passed, 0 failed**
- `cargo clippy -p roml --all-targets -- -D warnings` — **clean**
- `cargo test -p roml-highs --all-targets` — **all pass** (ripple check)
- `cargo fmt --all -- --check` — **clean**

Public API surface preserved: `Model::new`/`Default`/`Clone` and
`solve`/`solve_with` signatures unchanged. `set_metadata` gained a
`Result<(), ModelError>` return (prescribed by WR-05) and the P25-internal
`normalize_result` gained two parameters (prescribed by CR-02); neither is in
the guarded M2 surface. `.planning/STATE.md` and `.planning/ROADMAP.md` (root)
were not modified.

---

_Fixed: 2026-08-02T23:20:00Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
