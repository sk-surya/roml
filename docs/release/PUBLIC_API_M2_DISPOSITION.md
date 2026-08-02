# M2 — Public API Surface Disposition

**Phase:** 20-public-api-contract (Task 4)
**Requirement IDs:** API-04, API-07, API-08
**Base:** `d1391fb3a58a61d24b5c597aab78e4be7683f894`

Every public entry point of `roml` and `roml-highs` at the P20 base is assigned
exactly one disposition. The inventory is derived from `cargo public-api`
output stored verbatim in `docs/release/evidence/M2_P20_public_api_roml.txt`
(7431 lines) and `M2_P20_public_api_roml_highs.txt` (110 lines), cross-checked
against `src/lib.rs`, the prelude, and `src/model/mod.rs`.

## Disposition legend

| Disposition | Meaning |
|---|---|
| **golden path** | Recommended for ordinary model authors; stays in the default prelude and documentation. |
| **optional syntax sugar** | Pure builder/shortcut (no effectful semantics); may remain indefinitely. |
| **advanced backend extension** | For framework/backend authors; moves under `roml::advanced` / `roml::backend` (D9) and leaves the default prelude (API-07.3). |
| **compatibility/deprecated** | Superseded by a replacement; kept only for a documented pre-1.0 deprecation window (API-08.1/08.3). |
| **internal exposure to remove** | Implementation detail that leaked into the public API; removed from the public surface. |

Deprecation order and replacement signatures follow in the final sections.

## 1. Root re-exports and prelude (API-07)

`src/lib.rs` currently re-exports a mixed audience. The prelude also carries
protocol types that API-07.2 explicitly excludes from the default prelude.

| Item | Disposition | Replacement / note |
|---|---|---|
| `roml::prelude::Model`, `ModelError`, `ConstraintBounds`, `ConstraintSpec`, `ObjectiveSpec`, `LinExpr`, `Sense`, `Bounds`, `VarType`, `VarId`, `ConId`, `ObjId`, `ParamId`, `ValueExpr`, `Solution`, `SolverError` | **golden path** | Core model/solution vocabulary; retained in the curated P23 prelude. |
| `roml::prelude::ConstraintExprExt`, `ObjectiveExprExt` | **golden path** | Fluent `.le/.ge/.eq/.between/.minimize/.maximize` (API-04.3). |
| `roml::prelude::constrain`, `set_objective` (macros) | **compatibility/deprecated** | Effectful macros (D1); replaced by `Model::constrain`/`Model::minimize`/`Model::maximize`. Deprecated in P23. |
| `roml::prelude::Change`, `CoeffId` | **advanced backend extension** | API-07.2 requires these absent from the default prelude; move to `roml::backend`. |
| `roml::DeltaBatch`, `ModelOp` | **advanced backend extension** | Protocol types (API-07.2); move to `roml::backend`. |
| `roml::ModelRevision`, `ModelSnapshot` | **advanced backend extension** | Protocol types (API-07.2); move to `roml::backend`. |
| `roml::Change` | **advanced backend extension** | Protocol type; move to `roml::backend`. |
| `roml::BackendSession`, `Synchronization`, `SyncReceipt`, `SessionHealth`, `SolutionView`, `BackendMetadata`, `CallbackSession` | **advanced backend extension** | Backend contract (API-07.2/07.3); move to `roml::backend`. |
| `roml::AdapterCursor`, `AdapterHealth`, `ApplyOutcome` | **advanced backend extension** | Sync protocol (API-07.2); move to `roml::backend`. |
| `roml::BackendError`, `ErrorCategory`, `HealthEffect`, `TerminationStatus`, `BackendCapabilities` | **advanced backend extension** | Backend error/capability contract; move to `roml::backend`. |
| `roml::SolveRequest`, `SolveResult`, `SolveSolution`, `EffectiveConfig`, `ConfigAdjustment`, `ConfigRejection` | **advanced backend extension** | Solve-request protocol (D4/D9); move to `roml::backend`. |
| `roml::LpAlgorithm`, `SolverError` | **advanced backend extension** | Solve-policy / error types; move to `roml::backend`. |
| `roml::CoeffId` | **advanced backend extension** | Implementation identity (D11); not user-facing. |
| `roml::Solution`, `SolutionBuilder`, `SolutionStore` | **golden path** | `Solution` is the unified golden-path result (API-03.1). `SolutionBuilder`/`SolutionStore` are advanced result construction. |
| `roml::ValueExpr` | **advanced backend extension** | Parameter-dependent persistent expressions. |

## 2. Model constructors and mutators (API-04, API-05, API-06)

| Item | Disposition | Replacement / note |
|---|---|---|
| `Model::new()` | **golden path** | Retained. |
| `Model::with_name(name)` / `Model::named(name)` | **golden path** (target) | `named` is the P22 target name-bearing constructor (target fixture). |
| `Model::add_var()` | **compatibility/deprecated** | Replaced by `add_variable(continuous())` (D7) in P22. |
| `Model::add_binary()` | **compatibility/deprecated** | Replaced by `add_variable(binary())` (D7). |
| `Model::add_integer(Bounds)` | **compatibility/deprecated** | Replaced by `add_variable(integer())` with optional `.bounds(...)` (D7). |
| `Model::add_variable(Bounds, VarType)` | **compatibility/deprecated** | Signature collides with target `add_variable(VariableDef)` (D7); replaced in P22. |
| `Model::add_parameter(f64)` | **compatibility/deprecated** | Replaced by `add_parameter(parameter(value))` (D7) in P22. |
| `Model::add_constraint(ConstraintBounds)` | **golden path** (target form) | `add_constraint(spec)` (API-04.1) is the canonical form; the raw bounds form remains advanced. |
| `Model::constrain(spec)` | **golden path** | Canonical constraint mutation (API-04.1, D1). |
| `Model::constraint(spec)` | **compatibility/deprecated** | Redundant alias of `constrain`; deprecate in P23. |
| `Model::add_constraint_expr(expr, bounds)` | **advanced backend extension** | Low-level constraint mutation; sparse/advanced. |
| `Model::minimize(expr)` / `Model::maximize(expr)` | **golden path** | Canonical single-objective mutations (API-04.2, D1). |
| `Model::set_objective(spec)` | **compatibility/deprecated** | Superseded by `minimize`/`maximize` (D1); deprecated in P23. |
| `Model::add_objective_expr(expr, sense)` / `add_objective_spec(spec)` | **advanced backend extension** | Low-level objective mutation; returns `(ObjId, constant)` pair. |
| `Model::set_active_objective`, `clear_active_objective`, `active_objective`, `num_objectives` | **advanced backend extension** | Multi-objective control; the ordinary path uses `minimize`/`maximize`. |
| `Model::objective_constant(obj)`, `active_objective_constant()` | **golden path** | Objective constant inspection (API-03.5). |
| `Model::set_parameter(param, f64)` (infallible) | **compatibility/deprecated** | Target is fallible `set_parameter(param, value) -> Result` (D10, target fixture); P22 makes it fallible. |
| `Model::parameter_value(param)` | **golden path** | Parameter inspection (API-06.3). |
| `Model::set_variable_bounds`, `set_variable_type`, `set_binary`, `set_variable_active`, `set_constraint_bounds`, `set_constraint_active` | **advanced backend extension** | Low-level mutators; fluent builders and definitions cover the golden path. |
| `Model::set_semicontinuous(var, lower)` | **advanced backend extension** | Semi-continuous domain (advanced). |
| `Model::commit()` | **advanced backend extension** | Optional explicit revision boundary (D5); ordinary path auto-commits on `solve`. |
| `Model::rollback()`, `has_uncommitted()`, `has_pending_changes()`, `changelog_sequence()`, `current_revision()` | **advanced backend extension** | Transaction/revision mechanics. |
| `Model::drain_changes()` | **compatibility/deprecated** | Destructive changelog drain superseded by revisioned sync (D2); deprecated in P23. |
| `Model::take_snapshot()` | **advanced backend extension** | Rebuild input; orchestration handles it inside core (D2). |
| `Model::deltas_since(rev)` | **internal exposure to remove** | Test-only helper (`/// for testing`); replace with core-internal batch selection. |
| `Model::pprint()` | **golden path** | Diagnostics/formatting (D6). |
| `Model::constraint_slack`, `violated_constraints`, `bound_violations`, `constraint_expression`, `objective_expression` | **golden path** | Algebraic introspection of a `Solution`. |
| `Model::validate_invariants()` | **advanced backend extension** | Debug/validation utility. |
| `ModelConstants`, `ModelConstants::set_feas_tol` | **internal exposure to remove** | Duplicated/default-recursing constants builder (known defect); fold into model configuration. |
| `Model::num_variables/num_constraints/num_objectives/num_parameters/num_coefficients` | **golden path** | Counts used by users and diagnostics. |

## 3. Expression and operator traits (API-04)

| Item | Disposition | Replacement / note |
|---|---|---|
| `LinExpr` | **golden path** | Canonical expression type. |
| `ConstraintExprExt` (`.le/.ge/.eq/.between`) | **golden path** | Canonical constraint builders (API-04.3). |
| `ObjectiveExprExt` (`.minimize/.maximize`) | **golden path** | Canonical objective builders. |
| `ConstraintSpec`, `ObjectiveSpec` | **golden path** | Builder result types consumed by `Model::constrain`/`minimize`. |
| `ValueExpr` (persistent parameter expressions) | **advanced backend extension** | Parameter-dependent coefficient expressions. |
| `Term`, `TermCoeff` | **advanced backend extension** | Expression internals; keep public for advanced algebra but not in prelude. |
| `LinExpr::compile_for_constraint/compile_for_objective`, `simplify`, `evaluate` | **advanced backend extension** | Advanced expression surgery. |
| Operator impls on `VarId`/`f64`/`ValueExpr`/`ParamId` (`Mul`, `Add`, `Into<LinExpr>`) | **golden path** | The algebra a user writes (`3.0 * x + y`). |

## 4. Macros (API-04.4)

| Item | Disposition | Replacement / note |
|---|---|---|
| `constraint!` | **optional syntax sugar** | Pure `ConstraintSpec` builder; may remain (D1). |
| `objective!` | **optional syntax sugar** | Pure `ObjectiveSpec` builder; may remain (D1). |
| `constrain!` | **compatibility/deprecated** | Effectful (D1); replaced by `model.constrain(...)`. Deprecated in P23. |
| `set_objective!` | **compatibility/deprecated** | Effectful (D1); replaced by `model.maximize(...)`/`model.minimize(...)`. Deprecated in P23. |

## 5. IDs and coefficient APIs (API-05.6, API-06, D8, D11)

| Item | Disposition | Replacement / note |
|---|---|---|
| `VarId`, `ConId`, `ObjId`, `ParamId` | **advanced backend extension** | Raw opaque IDs remain for advanced/backend use (D8); golden path gains semantic aliases `Variable`, `Constraint`, `Objective`, `Parameter`. |
| `CoeffId` | **advanced backend extension** | Implementation identity (D11); callers reason about matrix cells. |
| `Generation`, `IdArena` | **internal exposure to remove** | Raw arena/identity internals (API-07.4); make crate-private. |
| `CoefficientData`, `CoefficientTarget`, `CellKey`, `ModelOp` | **advanced backend extension** | Sparse coefficient internals (D11). |
| `Model::add_coeff`, `add_objective_coeff`, `add_constraint_coefficient`, `add_objective_coefficient`, `remove_coefficient`, `coefficient` | **advanced backend extension** | Low-level sparse APIs; D11 defines `set_coefficient` / `add_to_coefficient` / `remove_coefficient` semantics. |

## 6. Solution / status / result types (API-03, D4)

| Item | Disposition | Replacement / note |
|---|---|---|
| `Solution` | **golden path** | One golden-path result type (API-03.1). |
| `SolverStatus` (`roml::solver::SolverStatus`) | **golden path** | User-facing status; D4/API-03.2 may rename to `SolveStatus`. |
| `SolutionBuilder`, `SolutionStore` | **advanced backend extension** | Result construction/storage for frameworks. |
| `SolveRequest`, `SolveResult`, `SolveSolution`, `EffectiveConfig` | **advanced backend extension** | Backend-protocol result types (D4); `Solution` is the public normalization. |
| `TerminationStatus` | **advanced backend extension** | Native termination mapping retained for protocol (API-03.2 distinctions). |
| `SolverError` | **golden path** | Public error type for the solve path. |
| `roml_highs::HighsError` (= `BackendError`) | **advanced backend extension** | Backend error; golden path uses `SolveError` (API-01.1/01.2) once P21 lands. |

## 7. Backend session traits and sync types (API-02, D2, D9)

| Item | Disposition | Replacement / note |
|---|---|---|
| `BackendSession` | **advanced backend extension** | Frozen backend contract (D2, API-02.4); `SolverSession<B>` orchestrates from core. |
| `SessionHealth`, `SolutionView`, `BackendMetadata` | **advanced backend extension** | Supplementary contract traits. |
| `Synchronization`, `SyncReceipt`, `BackendFixture` | **advanced backend extension** | Sync protocol and conformance harness (D9). |
| `AdapterCursor`, `AdapterHealth`, `ApplyOutcome`, `ApplyError` | **advanced backend extension** | Cursor/health protocol (API-07.2). |
| `roml_highs::HighsSession` | **advanced backend extension** | Remains public for framework authors/tests (D3); golden path uses `roml_highs::Highs` (P21). |
| `roml_highs::HighsFixture` | **advanced backend extension** | Conformance-test fixture; test-only. |
| `roml_highs::HighsInt` | **advanced backend extension** | Binding re-export for advanced use. |
| `roml::solver::reference::ReferenceBackend`, `NormalizedView`, `solver::conformance` | **advanced backend extension** | Reference implementation and conformance suite for backend authors. |

## 8. Callback and capability types

| Item | Disposition | Replacement / note |
|---|---|---|
| `CallbackSession`, `CallbackHandler`, `CallbackData`, `CallbackAction`, `CallbackCut` | **advanced backend extension** | MIP callback contract for framework/backend authors (PROJECT.md non-goal: no uniform callback feature). |
| `BackendCapabilities`, `BackendInfo` | **advanced backend extension** | Explicit capability declarations. |
| `LpAlgorithm` | **advanced backend extension** | Solve-policy enum used by `SolveRequest`. |

## 9. Other public types

| Item | Disposition | Replacement / note |
|---|---|---|
| `ModelSnapshot` (module `snapshot`, incl. `CellEntry`, `ConstraintEntry`, `ObjectiveEntry`, `VariableEntry`, `take_snapshot`) | **advanced backend extension** | Rebuild input for orchestration (D2). |
| `ModelRevision`, `RevisionError` | **advanced backend extension** | Revision identity; core-orchestrated (D2). |
| `DeltaBatch` | **advanced backend extension** | Revisioned delta stream (D2). |
| `Change` / `model::changelog::Change` | **advanced backend extension** | Change journal; supersedes destructive drain (D2). |
| `ValidationError`, `BoundValue`, `FiniteScalar`, `Tolerance` | **advanced backend extension** | Validation building blocks (D10). |
| `Transaction` internals | **internal exposure to remove** | Model-private transaction; verify no public leakage. |

## Replacement signatures (from DECISIONS.md D7 and target fixtures)

```rust
// Definition builders (P22, D7) — golden path.
pub fn continuous() -> VariableDef;
pub fn integer() -> VariableDef;
pub fn binary() -> VariableDef;
pub fn parameter(value: f64) -> ParameterDef;

impl Model {
    pub fn add_variable(&mut self, def: VariableDef) -> Result<Variable, ModelError>;
    pub fn add_parameter(&mut self, def: ParameterDef) -> Result<Parameter, ModelError>;
    pub fn set_parameter(&mut self, param: Parameter, value: f64) -> Result<(), ModelError>; // fallible (D10)
}

// Façade (P21, D3) — golden path.
pub struct roml_highs::Highs;
impl Highs {
    pub fn new() -> Result<Highs, HighsError>;
    pub fn solve(&mut self, model: &mut Model) -> Result<Solution, SolveError>;
    pub fn solve_with(&mut self, model: &mut Model, options: SolveOptions) -> Result<Solution, SolveError>;
}
```

Full target bodies are frozen in `tests/ui/target_quickstart.rs` and
`tests/ui/target_incremental.rs`.

## Deprecation order (D12: replacement before deprecation)

| Order | Deprecated item | Replacement | When |
|---|---|---|---|
| 1 | `drain_changes()` (destructive drain) | implicit commit + revisioned sync in `SolverSession<B>` | after P21 |
| 2 | `set_objective`, `set_objective!` | `minimize`/`maximize` | after P21 |
| 3 | `constrain!` | `model.constrain(...)` | after P21 |
| 4 | `add_var`, `add_binary`, `add_integer`, `add_parameter(f64)`, `add_variable(Bounds, VarType)` | definition-builder forms | after P22 |
| 5 | infallible `set_parameter(param, f64)` | fallible `set_parameter` | after P22 |
| 6 | `Model::constraint` (alias) | `Model::constrain` | after P22 |
| 7 | Protocol/sync/ID items in the default prelude | `roml::advanced` / `roml::backend` | after P23 curation |

All deprecations are documented in `MIGRATION.md` and `CHANGELOG.md` and remain
tested for the chosen window (API-08.2/08.3).

## Open items inherited from planning (recorded in M2 STATE.md)

- Exact compatibility window for the two effectful macros.
- Whether `SolveStatus` replaces `SolverStatus` immediately or ships as an alias.
- Exact name of the generic core façade (`SolverSession<B>` unless compile ergonomics prove otherwise).
- Whether semantic aliases are type aliases or transparent wrappers (default: aliases).

These are decided during P21/P22 implementation, not guessed here; the
dispositions above do not depend on their resolution.
