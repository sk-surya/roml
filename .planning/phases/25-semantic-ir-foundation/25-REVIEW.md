---
phase: 25-semantic-ir-foundation
reviewed: 2026-08-02T22:45:00Z
depth: standard
files_reviewed: 24
files_reviewed_list:
  - src/identity.rs
  - src/metadata.rs
  - src/function/mod.rs
  - src/function/scalar.rs
  - src/function/set.rs
  - src/construct/mod.rs
  - src/model/mod.rs
  - src/model/constraint.rs
  - src/model/changelog.rs
  - src/solution/metadata.rs
  - src/snapshot.rs
  - src/delta.rs
  - src/lib.rs
  - src/advanced.rs
  - src/expr/linear.rs
  - src/solver/facade.rs
  - src/solver/reference.rs
  - src/solver/conformance.rs
  - roml-highs/src/projection.rs
  - tests/m3_baseline_characterization.rs
  - tests/lineage_metadata.rs
  - tests/semantic_ir.rs
  - roml-highs/tests/behavior_tests.rs
  - roml-highs/tests/contract_tests.rs
  - roml-highs/tests/solve_observables_tests.rs
findings:
  critical: 2
  warning: 6
  info: 3
  total: 11
status: issues_found
---

# Phase 25: Code Review Report

**Reviewed:** 2026-08-02T22:45:00Z
**Depth:** standard
**Files Reviewed:** 24
**Status:** issues_found

## Summary

The P25 semantic-IR foundation is structurally sound: the opaque identity model
(checked atomic counters, zero reserved), the `EntityRef`-keyed metadata store
(`VarId`/`ConId`/`ObjId`/`ParamId` all hash generation-stamped IDs, so stale
handles never collide), the generation-safe construct arena (globally-unique
opaque ids make stale-id rejection inherent), and the deterministic
snapshot/delta reconstruction (arena iteration is index-ordered) are all
correctly designed and mostly well-tested.

Two blocker-class defects were found. First, the delta batch's reconstructed
semantic `set` is computed from the `AddConstraint` op's *declared* bounds and
ignores a same-batch `SetConstraintBounds` op — so any constraint added through
the ordinary constant-folding path (`add_constraint((2x + 3).le(10))`) produces
a delta `FunctionEntry.set` that disagrees with the model's canonical view, the
very "no second authority" divergence the code claims to prevent. Second, the
real solve path never binds the solved model's lineage/instance ids into
`SolveMetadata`: `normalize_result` fills them from `..SolveMetadata::default()`,
which allocates *fresh, unrelated* global-counter ids on every solve, and no
binding step exists anywhere in the crate — the code comment promising one is
unimplemented, so `solution.metadata().model_lineage` can never equal
`model.lineage()` on a real solve.

Warning-tier issues cover a genuine nondeterminism gap (`constraint_expression`
iterates a `HashSet`, so term order in the "canonical" `constraint_function`
differs per process and from snapshot entries), tautological invariant checks
on the transitional legacy fields (`debug_assert_eq!(set, ScalarSet::from(bounds))`
compares a value to itself), a counter-wrap re-issuance hole in
`allocate_id`, panic-vs-typed-error inconsistency in overflow handling, and
metadata-liveness gaps (stale/removed entities can carry orphaned metadata, and
removing a construct after attaching metadata makes `validate_invariants`
report a false violation). The HiGHS projection and the test suites themselves
are largely correct; the known-broken mosek/xpress adapters were out of scope.

## Critical Issues

### CR-01: DeltaBatch reconstructed `set` uses pre-adjustment bounds — the semantic IR diverges on constant-folding constraints

**File:** `src/delta.rs:318` (with `src/model/mod.rs:752-758` and `src/expr/linear.rs:644-650`)

**Issue:** `reconstruct_function_entries` builds each `FunctionEntry.set` solely
from the `ModelOp::AddConstraint { bounds }` op and only folds `SetCell` ops
into the linear function. It never consults a same-batch
`ModelOp::SetConstraintBounds` op. But the ordinary canonical path adds a
constraint with a non-zero expression constant by inserting the row at the
declared bounds and then immediately folding the constant into the bounds via
`set_constraint_bounds` (`src/model/mod.rs:752`), which emits a
`SetConstraintBounds` op in the *same* batch. Example:
`model.add_constraint((x + 3.0).le(5.0))` produces `AddConstraint{le(5.0)}`,
`SetCell{x:1.0}`, `SetConstraintBounds{le(2.0)}` in one batch; the reconstructed
delta entry carries `ScalarSet::LessEqual(5.0)` while the model's canonical
`constraint_function(con)` and the snapshot both carry `LessEqual(2.0)`. This is
exactly the second-coefficient-authority divergence that SM-01.1/SM-01.4 forbid,
and it silently breaks the "reconstructed views agree" invariant for the
most common user path (`(2x + 3).le(10)` etc.). No test exercises a constant
term in a constraint expression, so it is untested.

**Fix:** After collecting the cells, apply any later same-batch
`SetConstraintBounds { con }` op's bounds as the effective bounds (mirror the
final-activity fold already done in `reconstruct_construct_entries`):

```rust
// in reconstruct_function_entries, for each AddConstraint { con, .. }:
let mut effective = *bounds;
for later in operations {
    if let ModelOp::SetConstraintBounds { con: c, bounds } = later {
        if c == con {
            effective = *bounds;
        }
    }
}
let set = ScalarSet::from(effective);
```

### CR-02: Real solve path never binds the model's lineage/instance into `SolveMetadata`

**File:** `src/solver/facade.rs:44-53` (and `src/solution/metadata.rs:45-57`)

**Issue:** `normalize_result` constructs `SolveMetadata` with
`..SolveMetadata::default()`, and `SolveMetadata::default()` allocates **fresh**
`ModelLineageId`/`ModelInstanceId` from the global per-family counters
(`src/solution/metadata.rs:53-54`). The comment at `facade.rs:51-52` claims the
fresh default ids "are filled in when a model binds the solution (SM-02.7)", but
a crate-wide search shows no binding step exists: the only writers of
`model_lineage`/`model_instance` are `SolveMetadata::default()` and test code.
`SolverSession::solve_with` holds `model: &mut Model` (which exposes
`model.lineage()`/`model.instance()`) yet never copies them into the result.
Consequently every `Solution` produced through the primary solve path carries
lineage/instance ids that are guaranteed *unequal* to the solved model's — a
consumer comparing `solution.metadata().model_lineage == model.lineage()`
(assignment-reuse compatibility across clones, SM-02.1/SM-02.7 foundations)
always gets `false`. Each solve also silently burns two global counter ids,
advancing counters meant for model allocation. The P25 contract (§4, SM-02.7)
is not delivered on the actual solve path.

**Fix:** Pass the model identity into `normalize_result` and set the fields
explicitly (instead of relying on a nonexistent binding step):

```rust
// in SolverSession::solve_with, step 9:
let solution = normalize_result(
    &result,
    committed,
    active_objective,
    self.backend.name(),
    sync_mode,
    model.lineage(),
    model.instance(),
)?;
// and in normalize_result:
.metadata(SolveMetadata {
    backend_name: backend_name.to_string(),
    model_revision,
    effective_configuration: result.effective_configuration.clone(),
    synchronization,
    model_lineage,
    model_instance,
})
```

## Warnings

### WR-01: `constraint_expression` term order is nondeterministic and disagrees with snapshot function entries

**File:** `src/expr/linear.rs:757-770`, `src/model/coefficient.rs:305-307`

**Issue:** `constraint_expression` (used by `Model::constraint_function`, the
canonical SM-01.1 reconstruction) iterates `self.coefficients.for_constraint(con)`,
which flattens a `HashSet<CoeffId>`. HashSet iteration order is seeded randomly
per process, so the term order of the "canonical" `ScalarFunction::Linear`
differs across runs and, within one run, almost always differs from the
snapshot/delta function entries (built from deterministic arena/cell order,
`src/snapshot.rs:174-196`). The invariant checks in `Model::take_snapshot`
(`src/model/mod.rs:1595-1602`) and `validate_invariants` only compare `set`,
never the function expression, so the divergence is silently tolerated — but any
consumer comparing `snapshot.functions[i].function` against
`model.constraint_function(con).function` gets spurious inequality for the same
row. This undermines the "deterministic reconstruction" contract for the P25
semantic IR.

**Fix:** Sort coefficients by a stable key (`var` or index) when reconstructing
the expression, and/or assert the function equality (not just `set`) in the
invariant checks.

### WR-02: The "transitional legacy field" invariant checks are tautological

**File:** `src/snapshot.rs:188-190`, `src/delta.rs:320-321`

**Issue:** Both `reconstruct_function_entry` and `reconstruct_function_entries`
end with `debug_assert_eq!(set, ScalarSet::from(bounds))` where `set` was just
computed as `ScalarSet::from(bounds)` on the previous line. This compares a value
to itself and can never fail. The comments present these as enforcing the
"derived from legacy bounds, never a parallel authority" invariant, which gives
false confidence — as CR-01 demonstrates, a real divergence (`AddConstraint`
bounds vs. final folded bounds) is not caught by any assert. The checks are
decorative.

**Fix:** Compare against an independently derived authority (e.g., the model's
`constraint_function` in `Model::take_snapshot`), or remove the self-comparison
assertions and document that the reconstruction is untested-by-assertion.

### WR-03: `allocate_id` wraps the counter to 0 after `IdentityOverflow`, re-issuing id 1

**File:** `src/identity.rs:32-38`

**Issue:** `fetch_add(1)` when `pre == u64::MAX` wraps the counter to 0 before the
error is returned. A subsequent call then observes `pre == 0` and returns id 1 —
re-issuing an id already allocated long ago, and violating the doc's own
guarantee ("further allocation returns this error instead of reusing ids") and
the "zero is reserved" invariant. Practically unreachable (requires 2^64
allocations), but the checked-overflow contract is not actually upheld across
the error boundary.

**Fix:** Saturate on overflow so the counter stays exhausted, e.g. a CAS loop
that stores `u64::MAX` when the pre-increment value was `u64::MAX - 1` (or use
`fetch_update`), so every subsequent call returns `Err` without advancing.

### WR-04: Inconsistent overflow handling — `Model::default`/`clone` and `SolveMetadata::default` panic, construct allocation is typed

**File:** `src/model/mod.rs:201-202, 226`, `src/solution/metadata.rs:53-54`

**Issue:** `Model::new()`/`clone()` and `SolveMetadata::default()` call
`.expect("... counter exhausted")`, panicking on counter exhaustion, while
`add_construct_fixture` returns the typed `ModelError::IdentityOverflow`
(`src/model/mod.rs:363-366`). `src/identity.rs` claims exhaustion "returns a
typed IdentityOverflow error rather than wrapping" — false for the three
`expect` paths. A long-lived process (a service solving many models/solutions)
is the population most likely to hit exhaustion, and a panic in `Default`/`Clone`
is un-recoverable. At minimum the module docs should state which paths panic.

**Fix:** Prefer saturating the counters (WR-03) so allocation never panics, or
document the panic boundary explicitly and keep the typed path for the
fallible `add_construct_*` API only.

### WR-05: Metadata accepts stale entities and is never cascaded on removal

**File:** `src/model/mod.rs:309-321`

**Issue:** `set_metadata` performs no liveness check — metadata can be attached
to a stale/removed `EntityRef::Variable`/`Constraint`/`Objective`/`Parameter`
and silently stored. Conversely, the `remove_variable`/`remove_constraint`/
`remove_objective`/`remove_parameter` paths never remove the metadata entries
for the removed entity, so churn (add/remove cycles on a long-lived model)
grows the metadata map without bound and leaves orphans that are only reachable
by stale handles. `validate_invariants` only checks `EntityRef::Construct`
metadata against dead constructs (`src/model/mod.rs:1753-1761`), so the other
four entity kinds are unchecked either way.

**Fix:** Validate the referenced entity is live in `set_metadata` (return a
typed error, consistent with D10), and/or cascade metadata removal inside the
`remove_*` methods.

### WR-06: Removing a construct after attaching metadata produces a false `validate_invariants` violation

**File:** `src/model/mod.rs:408-416` and `src/model/mod.rs:1753-1761`

**Issue:** `remove_construct` removes the arena entry but not any
`EntityRef::Construct(k)` metadata. `validate_invariants` then flags the
orphaned construct metadata as a violation ("construct metadata references dead
construct"). So a perfectly valid user sequence — attach metadata to a
construct, then remove the construct — makes the invariant checker report a
corrupted model. This is asymmetric: orphaned *variable* metadata (WR-05) is
not flagged, only construct metadata is. Either cascade construct metadata
removal in `remove_construct`, or relax the invariant to tolerate orphans.

**Fix:** In `remove_construct`, also `self.metadata.remove(&EntityRef::Construct(id))`
before pushing the changelog event.

## Info

### IN-01: `Change::affects_solver` is dead code

**File:** `src/model/changelog.rs:248-251`

`affects_solver` is defined but never called anywhere in `src/` or
`roml-highs/src/`. If the delta/journal path is meant to skip solver sync for
pure-parameter changes, wire it in; otherwise remove it to avoid the misleading
impression that construct/parameter changes are filtered somewhere.

### IN-02: The identity overflow path is untested and untestable

**File:** `src/identity.rs:84-123`

The unit tests cover distinctness, ordering, hashing, and per-family counters but
not `IdentityOverflow`, and the design makes it untestable — the counters are
function-local `static` values with no injection seam. Given WR-03, the
overflow contract is unverified. Consider a `#[cfg(test)]` constructor that can
seed a counter at `u64::MAX - 1` so the overflow branch is actually exercised.

### IN-03: Construct-rebuild test relies on fragile ordering assumptions

**File:** `tests/semantic_ir.rs:319-323`

`construct_store_survives_rebuild` asserts
`assert!(!rebuilt_snap.constructs[0].active == !snap.constructs[0].active)` —
the `[0]` index is only correct because both stores are inserted in the same
order so the sorted-by-id order coincides with insertion order, and the
assertion is double-negated. It also never checks kind/preference equality
across the rebuild. Prefer locating the entry by id and asserting the full
`ConstructEntry` (kind + activity) matches.

---

_Reviewed: 2026-08-02T22:45:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
