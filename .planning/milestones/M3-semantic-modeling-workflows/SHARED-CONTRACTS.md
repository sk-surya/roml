# M3 Completion Shared Contracts

**Status:** binding written-spec contract for P36, P30, P31, and P34.

This file removes cross-phase ambiguity. A phase plan may refine an implementation detail, but it may not redefine these contracts without an explicit reviewed amendment to this file and the original M3 decisions.

## 1. Identity and equality

### 1.1 `ModelLineageId`

Purpose: compatibility of reusable semantic artifacts such as primal assignments across related clones.

Lifecycle:
- allocated once for a newly independent model;
- preserved by `Model::clone`;
- never changes when the model mutates;
- never reused intentionally within a process;
- not a cross-process or serialized correctness authority.

Equality: exact opaque-ID equality only. Equal lineage means “related model family,” not equal canonical state.

### 1.2 `ModelInstanceId`

Purpose: identity of one concrete live canonical model object.

Lifecycle:
- allocated for every independently created model;
- a clone preserves lineage but receives a fresh instance ID;
- model mutation does not change the instance ID;
- instance IDs are not reconstructed from hashes or entity contents.

Equality: exact opaque-ID equality only. Equal lineage + equal revision is insufficient to identify canonical state if instance IDs differ.

### 1.3 `ModelRevision`

Purpose: monotone canonical-state version within one model instance.

Lifecycle:
- copied as a numeric value when a model is cloned, while the clone receives a new instance ID;
- advances only for successful canonical mutations that affect canonical model semantics;
- metadata-only changes advance revision only where the existing canonical contract already says they do; P30/P31/P36 must not invent a new rule;
- solve overlays, temporary fixings, objective locks, starts, hints, analysis sessions, and MPS export do not advance revision.

Equality: revision numbers are comparable for stale-state authority only together with the same `ModelInstanceId`. The exact canonical-state key is `(ModelInstanceId, ModelRevision)`.

### 1.4 `CompilationId`

Purpose: exact identity of one compiled backend state.

Lifecycle:
- allocated by successful compilation/rebuild/overlay compilation according to the existing compiler/session contract;
- never derived from a recipe hash;
- an overlay solve has an overlay compilation identity distinct from its base compilation identity;
- rollback succeeds only when the session can prove that the expected base compiled state has been restored;
- rebuild creates/establishes a new exact compiled state according to the current compiler contract.

Equality: exact opaque-ID equality only. Backend results, origin maps, overlay receipts, IIS reports, and later analysis artifacts may be composed only when their required `CompilationId` values match exactly.

### 1.5 `RecipeFingerprint`

Deterministic evidence/cache aid only. Fingerprint equality never substitutes for `ModelInstanceId`, revision, or `CompilationId` equality.

## 2. Overlay and dirty-session transaction contract

The following applies to P27 overlays and every later solve-scoped workflow built on them, including P30 relaxation and P31 lexicographic stages.

### 2.1 Apply

1. Validate model instance/revision and exact base `CompilationId` before backend mutation.
2. Compile temporary artifacts against that exact base.
3. Apply transactionally as far as the backend permits, recording an apply receipt sufficient for rollback verification.
4. If apply fails after any backend mutation, rollback is attempted immediately.

### 2.2 Commit

A solve-scoped overlay is never committed into canonical model state. “Commit” means only that an individual backend operation/apply stage succeeded and was recorded in the receipt. Canonical persistence requires an explicit canonical model mutation through the model API.

### 2.3 Rollback

Rollback is successful only if ROML verifies restoration of the expected base compiled state. Successful rollback restores the session to usable `Ready` state and leaves canonical model revision unchanged.

### 2.4 Dirty/uncertain session

If apply or rollback leaves backend state uncertain, session health becomes `RequiresRebuild`. No later ordinary solve may use that backend state until rebuild from the current canonical snapshot succeeds.

### 2.5 Error composition

The primary operation failure and cleanup/rollback failure are both preserved.

Conceptual contract:

```rust
pub struct SolveScopedFailure {
    pub primary: Option<Box<SolveError>>,
    pub cleanup: Box<SolveError>,
    pub session_health: AdapterHealth,
}
```

This shape is illustrative; existing error enums may encode it differently. The binding behavior is:
- primary failure must never be overwritten by rollback failure;
- rollback failure must never be discarded merely because a primary failure already exists;
- successful solve + failed rollback is returned as an operational error, not as a successful reusable solution;
- any uncertain cleanup forces `RequiresRebuild`;
- rebuild failure is preserved as its own operational failure and does not erase the earlier primary/cleanup evidence.

P30 and P31 tests must inject failures at apply, solve, extraction, rollback, rollback verification, and rebuild boundaries.

## 3. Parameterized export contract

P36 MPS is a numeric interchange format, not a symbolic parameter serialization format.

### 3.1 Evaluated snapshot

Semantic MPS export evaluates every parameter-dependent coefficient, bound, objective coefficient, objective offset, and other supported numeric expression against one exact canonical snapshot identified by `(ModelInstanceId, ModelRevision)`.

### 3.2 Environment metadata

A successful `MpsWriteReport` records:
- `ModelLineageId`;
- `ModelInstanceId`;
- `ModelRevision`;
- deterministic evaluated-parameter entries sufficient to identify every parameter value consumed by export;
- active naming policy;
- emitted dimensions/counts and deterministic name map.

Evaluated parameter entries are ordered deterministically by export-local canonical traversal, not by debug rendering of arena/slot IDs.

### 3.3 Round-trip claim

`read(write(model))` is compared to the **evaluated mathematical snapshot**. It is not expected to reconstruct parameter identities, dependency graphs, or future mutability.

Unbound, stale, or non-finite parameter evaluation is a typed P36 error before path commit.

## 4. Deterministic export naming

P36 output bytes must not embed unstable arena slot/generation/debug IDs.

Default name policy: `PreserveOrGenerate`.

Rules:
1. Preserve an existing user name only when it is valid free-MPS text for the selected dialect and unique in the required MPS namespace.
2. Otherwise generate an export-local name from deterministic canonical traversal ordinal:
   - variables: `X000001`, `X000002`, ...;
   - rows: `R000001`, `R000002`, ...;
   - objective row: `OBJ` unless collision requires `OBJ000001`;
   - vectors: `RHS1`, `RNG1`, `BND1`.
3. Generated ordinals come from the writer's normalized export projection order, never from raw model slot IDs.
4. Collision resolution is deterministic and recorded in `MpsWriteReport.name_map`.
5. Repeated writes of the same canonical state/options on the same ROML version emit identical names/bytes.

Cross-construction canonical isomorphism is not a P36 byte-identity claim; the guarantee is deterministic export of one canonical state.

## 5. Objective degradation lock formula

P31 owns the executable objective-policy semantics. P27/P30 may consume this formula but may not redefine it.

For a completed stage with scalar stage objective value `z*`, finite nonnegative absolute tolerance `a`, and finite nonnegative relative tolerance `r`:

```text
scale = abs(z*)
delta = a + r * scale
```

For minimization:

```text
f(x) <= z* + delta
```

For maximization:

```text
f(x) >= z* - delta
```

Consequences:
- `z* > 0`: conventional relative degradation around magnitude `|z*|`;
- `z* = 0`: relative tolerance contributes zero; absolute tolerance is the only allowed degradation;
- `z* < 0`: the relative component still uses positive magnitude `|z*|`, avoiding sign reversal.

No `max(1, |z*|)` hidden scale is permitted. If a backend-native multiobjective API uses a different formula, ROML uses the portable executor unless an explicitly reviewed semantic adapter proves equivalence to this contract.

For a weighted stage, `f` is the fully normalized scalar stage objective after objective-sense normalization and parameter evaluation.

## 6. Objective-policy ownership and shared priority type

### 6.1 Sole owner

**P31 is the sole phase owner of canonical `ObjectivePolicy`.** P30 must not introduce a competing policy type.

### 6.2 Shared priority type

P31 introduces and owns one public/shared validated priority type:

```rust
pub struct ObjectivePriority(u32);
```

Semantics:
- `0` is highest/earliest priority;
- larger values execute later;
- equality means one stage/level;
- ordering is ascending numeric order;
- construction is checked only as needed for API invariants; all `u32` values are representable.

P31 uses this same type for lexicographic levels and adds `PenaltyTarget::Priority(ObjectivePriority)` to the P30 penalty surface. No `LexicographicPriority`, integer alias, or second priority newtype is permitted.

### 6.3 SM-10.6 ownership split

P30 implements executable `PenaltyTarget::{None,Objective}` and parameterized finite weights. It does **not** ship a decorative priority field before P31 exists.

P31 closes the priority-target portion of SM-10.6 by adding `PenaltyTarget::Priority(ObjectivePriority)` and exercising it through actual lexicographic execution. The leaf-level requirement ledger must record this split explicitly.

## 7. Authority rule

If any P36/P30/P31/P34 plan text contradicts this file, this file wins until a reviewed amendment changes it. Plans must cite the relevant section rather than restating a divergent formula or identity rule.
