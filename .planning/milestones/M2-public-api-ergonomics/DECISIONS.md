# M2 Architecture and API Decisions

## D1 — Method-first modeling is canonical

**Decision:** documentation and examples use `model.add_constraint(...)`, `model.minimize(...)`, and `model.maximize(...)`.

**Reason:** methods provide normal Rust discoverability, composition, typed errors, and visible mutation. A macro-first DSL produces attractive snippets but creates parallel semantics and weaker IDE/error behavior.

**Consequence:** `constraint!` may remain as a pure `ConstraintSpec` builder. Effectful `constrain!` and `set_objective!` are deprecated after replacements are proven.

## D2 — The user-facing solver owns synchronization

**Decision:** a generic core `SolverSession<B>` coordinates model commit, delta selection, snapshot rebuild, solve execution, and result normalization.

**Reason:** the model owns the journal and the backend owns native state. Core is the only layer that can coordinate both without leaking model internals or duplicating recovery logic in every backend crate.

**Invariant:** a solve attempt performs at most one automatic rebuild retry.

## D3 — HiGHS exposes a small façade

**Decision:** `roml_highs::Highs` wraps `SolverSession<HighsSession>` and exposes `new`, `solve`, and `solve_with`.

**Reason:** ordinary users should not construct snapshots or call `BackendSession::synchronize`.

**Consequence:** `HighsSession` remains available under the advanced backend surface for framework authors and tests.

## D4 — One golden-path solution and status model

**Decision:** façade calls return core `Solution` with `SolveStatus` and `SolveMetadata`.

**Reason:** `Solution`/`SolverStatus` and `SolveResult`/`SolveSolution`/`TerminationStatus` currently split ordinary access from backend-protocol access.

**Semantics:** mathematical termination returns `Ok(Solution)`; operational failure returns `Err(SolveError)`.

## D5 — Solve performs implicit transaction closure

**Decision:** `solve` commits pending model mutations before synchronization. Users may still call `commit` explicitly to establish a revision boundary.

**Reason:** requiring `commit` for the ordinary path exposes protocol mechanics. Multiple mutations before solve naturally form one atomic revision.

**Constraint:** implicit commit failure returns before backend mutation.

## D6 — Names are first-class model metadata

**Decision:** variable, parameter, constraint, and objective creation supports names; names are queryable and appear in diagnostics/formatting.

**Reason:** optimization users need names for debugging, large models, solver logs, exports, and infeasibility analysis.

**Non-goal:** names are not stable serialized identities.

## D7 — Definitions separate construction policy from handles

**Decision:** `continuous()`, `integer()`, `binary()`, and `parameter(value)` return validated definition builders consumed by model creation methods.

**Planned signatures:** 

```rust
pub fn continuous() -> VariableDef;
pub fn integer() -> VariableDef;
pub fn binary() -> VariableDef;
pub fn parameter(value: f64) -> ParameterDef;

impl Model {
    pub fn add_variable(&mut self, def: VariableDef) -> Result<Variable, ModelError>;
    pub fn add_parameter(&mut self, def: ParameterDef) -> Result<Parameter, ModelError>;
}
```

**Reason:** builders permit names and future domain metadata without multiplying constructor names.

## D8 — Semantic aliases, not new handle wrappers, in M2

**Decision:** prelude exports semantic aliases `Variable`, `Constraint`, `Objective`, and `Parameter` for current opaque IDs.

**Reason:** new wrappers would require broad operator-overload and backend-contract changes with little immediate user benefit.

**Reversal trigger:** P20 proves aliases create confusing rustdoc or diagnostics that cannot be corrected.

## D9 — Advanced protocol concepts remain public but recede

**Decision:** protocol and backend extension types move under `roml::advanced` or `roml::backend`; they are absent from the prelude.

**Reason:** external backend authors still need the contract, but model authors should not encounter it by default.

## D10 — Validation is uniformly fallible

**Decision:** model mutation APIs return typed errors for invalid numeric values, bounds, domains, stale IDs, and options in all build profiles.

**Reason:** `debug_assert!` is not input validation and creates different release behavior.

## D11 — Sparse cell semantics are explicit

**Decision:** advanced sparse APIs distinguish:

```rust
set_coefficient(target, variable, value)      // replace canonical cell
add_to_coefficient(target, variable, value)   // algebraically add
remove_coefficient(target, variable)          // remove by coordinate
```

**Reason:** `CoeffId` is an implementation identity; callers reason about matrix cells.

## D12 — Replacement before deprecation

**Decision:** no API is deprecated until the replacement compiles in tests and the migration is mechanical.

**Reason:** the current documentation already references missing APIs. M2 must close the loop before narrowing it.

## D13 — Defer indexed modeling containers

**Decision:** collections such as `add_variables(range, def)` are outside M2.

**Reason:** they are valuable but orthogonal. The immediate bottleneck is completing and simplifying the scalar model-to-solve path.