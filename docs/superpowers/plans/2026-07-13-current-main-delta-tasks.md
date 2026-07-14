# Current-Main Delta Task Pack

> This task pack supplements P0–P3 plans for `main@82e2ed95545635b628187ba0081fe8c8b03eaafb`. Execute tasks in their owning phases; do not collapse them into one PR.

**Requirements:** R1.1, R1.5–R1.6, R2.8–R2.9, R3.8–R3.9, R4.6–R4.7, R5.7, R5.11, R6.8–R6.9, R8.5, R9.7.

## P0-D1 — Capture current-main characterization before cleanup

**Files:** create `docs/release/evidence/P0_CURRENT_MAIN_CHARACTERIZATION.md`; add focused tests without changing behavior.

1. Record the exact current base and the four delta commits.
2. Inventory:
   - `SolveOptions`, `LpAlgorithm`, `Model::solver_options`, `set_solver_options`, `apply_options` implementations;
   - semi-continuous state and all backend handlers;
   - Xpress bulk-additive path and its eligibility predicate;
   - all newly copied native constants;
   - all tracked `*.log`, `.claude/settings.json`, `MODELING_API.md`, renamed logging files.
3. Add characterization tests for:
   - one-shot options being cleared after a successful solve;
   - option state when `apply_options` returns an error;
   - model changes when `apply_changes` returns an error;
   - semi-continuous mutation order in the emitted change sequence;
   - Xpress bulk eligibility for each `Change` variant using a pure helper where possible.
4. Do not assert defective loss/partial mutation as the desired future contract. Name tests `current_behavior_*` and cross-link R3.8/R3.9.
5. Capture package file lists showing generated logs and `.claude/settings.json` inclusion/exclusion.

**Acceptance:** every new current-main risk has an executable reproduction or a precise reason it requires the licensed/native P3 environment.

## P0-D2 — Repository artifact and guidance cleanup

**Files:** remove tracked logs; update `.gitignore`, `README.md`, `AGENTS.md`, `MODELING_API.md` status language; review `.claude/settings.json`.

1. Delete `roml.log`, `roml-highs/roml.log`, `roml-mosek/roml.log`, `roml-xpress/roml.log` and any other generated logs.
2. Add a repository-level check that fails when tracked generated logs/native binaries/licenses appear.
3. Decide whether `.claude/settings.json` is:
   - private operator configuration that should be removed/ignored; or
   - reviewed repository policy with least-privilege commands and explicit documentation.
4. Replace unsupported “production-grade” claims with “pre-1.0” plus exact tested support.
5. Mark `MODELING_API.md` as documenting the current API until P5, and add warnings where destructive sync/one-shot solver options will change.
6. Ensure `cargo package --list` does not contain operator-only settings or generated logs.

**Acceptance:** repository/package claims and contents match actual qualification state.

## P1-D1 — Replace fragmented variable domain state

**Current files:** `src/model/mod.rs`, `src/model/variable.rs`, `src/model/changelog.rs`, expression/modeling APIs, tests.

### Red tests

Add failing tests for:

- semi-continuous lower bound `NaN`, negative/invalid policy, greater than upper bound, and infinity;
- repeated `set_semicontinuous` with lower bound increase/decrease;
- conversion semi-continuous -> continuous/integer/binary and back;
- variable removal clearing all domain state;
- clone/snapshot preserving domain exactly;
- no ordinary lower-bound mutation that makes zero infeasible while claiming semi-continuous zero is feasible;
- semi-integer semantics distinct from binary;
- atomic failure of invalid transitions.

### Design

Define one normalized domain representation, e.g.:

```rust
pub enum VariableDomain {
    Continuous,
    Integer,
    Binary,
    SemiContinuous { nonzero_lower: FiniteF64 },
    SemiInteger { nonzero_lower: FiniteF64 },
}

pub struct VariableSpec {
    pub bounds: Bounds,
    pub domain: VariableDomain,
}
```

Names may differ. Required invariants:

- ordinary bounds describe the enclosing interval;
- semi domains explicitly add `{0} ∪ [l,u]` or `{0} ∪ ([l,u] ∩ Z)`;
- zero feasibility is not accidentally removed by setting the ordinary lower bound to `l`;
- binary is exactly `{0,1}` or has a documented normalization/error policy;
- every transition is one validated canonical mutation.

### Implementation

1. Remove `semicontinuous_lower` side storage after migration.
2. Replace independent `VariableTypeChanged` and `SemiContinuousBoundChanged` emission with one typed domain/spec operation in the new P2 representation; retain temporary compatibility translation only behind tests.
3. Update expression/modeling API with clear constructors/mutators.
4. Update invariant checker and property generator.
5. Add migration notes to CHANGELOG and `MODELING_API.md` draft.

**Acceptance:** one source of truth represents every variable domain; no order-dependent event sequence is needed to understand it.

## P1-D2 — Move solve policy out of `Model`

**Current files:** `src/model/mod.rs`, `src/solver/mod.rs`, all adapters and solve examples/tests.

### Red tests

- cloning/model equality is unaffected by transient solve policy;
- two adapter sessions can solve the same model revision with different algorithms concurrently/sequentially;
- retrying a failed solve uses the same immutable request unless caller changes it;
- unsupported algorithm is explicit;
- effective algorithm is visible in result metadata;
- a model snapshot contains no solve-session policy.

### Target API sketch

```rust
pub struct SolveRequest {
    pub lp_algorithm: Option<LpAlgorithm>,
    pub limits: SolveLimits,
    pub output: OutputPolicy,
    pub required_capabilities: CapabilitySet,
}

pub struct EffectiveSolveConfiguration {
    // normalized backend/version-specific decisions
}

pub struct SolveResult {
    pub termination: Termination,
    pub effective_configuration: EffectiveSolveConfiguration,
    pub solution: Option<Solution>,
}
```

1. Remove `Model::solver_options` and `set_solver_options`.
2. Replace `solve_model(&mut Model)` policy consumption with synchronization of a model revision plus `solve(&SolveRequest)`.
3. Decide whether an adapter may adjust a request. If yes, return explicit adjustment details; never silently ignore.
4. Backend mappings must distinguish unsupported from unavailable-under-current-model.
5. Keep backend-specific escape hatches namespaced and typed rather than stringly global options.

**Acceptance:** canonical model identity is independent of requested algorithm; solve behavior is reproducible and inspectable.

## P2-D1 — Close semi-continuous partial-apply counterexample

Build a fault-injection/reference test first, then native HiGHS integration.

### Current counterexample

1. model emits an ordinary lower-bound update;
2. model emits semi-continuous-domain update;
3. HiGHS applies the first;
4. HiGHS returns unsupported on the second;
5. drained changes are lost and backend state is partially mutated.

### Required protocol test

Given revision `r0` and a transaction producing `r1`:

- obtain one atomic `DeltaBatch(r0,r1)` with a coherent variable-domain operation;
- capability preflight may reject the batch before mutation; or
- if application begins and becomes dirty, cursor remains at `r0`, health becomes `RequiresRebuild`, and all deltas remain replayable;
- rebuild from snapshot `r1` either produces a clean explicit unsupported-domain result or applies an approved lowering/transformation policy;
- no false acknowledgement of `r1` occurs.

Test both preflight rejection and injected mid-batch dirty failure. Verify two independent adapter cursors remain correct.

**Acceptance:** R3.2–R3.5 and R3.9 pass with deterministic traces.

## P2-D2 — Make solve attempts retry-safe

1. Define an immutable solve-attempt object containing requested policy and target model revision.
2. Synchronize to the target revision without consuming journal history.
3. Validate capabilities/options before native mutation where possible.
4. Record effective configuration only after adapter acceptance.
5. On configuration failure, retain request and cursor state.
6. On solve failure, distinguish reusable session, rebuild-required session, and terminal session.
7. Test failure injection at:
   - before synchronization;
   - after operation `k`;
   - during request validation;
   - after applying one native option;
   - during solve;
   - during solution extraction.

**Acceptance:** retries are deterministic and do not depend on hidden one-shot state in `Model` or adapter fields.

## P3-D1 — Migrate current option/domain constants to authoritative bindings

### HiGHS

- Verify domain and algorithm support from pinned official headers/generated `highs-sys` bindings.
- Do not emulate unsupported semi-continuity by silently raising the ordinary lower bound.
- Return capability/validation results before model mutation.

### MOSEK

- Delete copied `IPAR_*`, optimizer, basis, hotstart, and semi-domain constants after migrating to official `mosek` enums/API.
- Check every option call and map official errors.
- Verify primal-simplex mapping; do not map a requested primal simplex to dual simplex without an explicit adjustment result.

### Xpress

- Delete copied controls/algorithm/domain constants after generated binding decision.
- Verify `XPRS_DEFAULTALG` values and actual application; current code stores an LP hint but must prove it is applied.
- Replace constructor/control `assert!` with typed errors.
- Verify semi-continuous (`S`) and semi-integer (`R`) semantics from the pinned official header/documentation.

**Acceptance:** adapters contain no manually maintained SDK enum/control values covered by authoritative bindings.

## P3-D2 — Characterize and migrate Xpress bulk updates

### Characterization matrix

For each fixture, run scalar/reference projection and current bulk projection:

- new continuous/integer/binary variables;
- new rows with sparse coefficients;
- objective cells and active-objective switch;
- duplicate symbolic contributors after P1 normalization;
- variable/constraint bound changes in the same batch as creation;
- domain transitions including semi-continuous/semi-integer;
- unsupported/removal operations forcing non-bulk path;
- empty model and zero-column/zero-row cases;
- native failure at add-cols, type-change, add-rows, matrix-change, and objective reload.

Compare normalized backend state and solve observations. Persist fixtures/seeds.

### Migration

1. Consume typed P2 aggregate operations, not raw adjacency-dependent `Change` sequences.
2. Preflight complete batch capability.
3. Define atomicity/dirty-state outcome for each native bulk sub-call.
4. Preserve `(row,column,value)` association deterministically.
5. Report fallback/rebuild decisions explicitly.
6. Benchmark scalar vs bulk application separately from solve time.

**Acceptance:** R6.9 and R8.5 pass; performance is retained only with correctness/recovery evidence.

## Verification and reporting

Each owning phase must include these tasks in its evidence report and requirement closure table. The current-main delta task pack does not authorize cross-phase implementation or publication.