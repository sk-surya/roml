# M2 — Public API Surface Disposition

**Phase:** 20-public-api-contract (Task 4)
**Requirement IDs:** API-04, API-07, API-08
**Base:** `d1391fb3a58a61d24b5c597aab78e4be7683f894`

Every public entry point of `roml` and `roml-highs` at the P20 base is assigned
exactly one disposition. The inventory is derived from `cargo public-api`
output stored as normalized evidence in
`docs/release/evidence/M2_P20_public_api_roml.txt` (7431 lines) and
`M2_P20_public_api_roml_highs.txt` (80 lines; absolute repository paths
replaced with `$REPO`), cross-checked against `src/lib.rs`, the prelude, and
`src/model/mod.rs`.

**Current vs target signatures:** every row classifies the *current-main*
signature. Where a target reuses an existing method name with a different
signature, the row notes `(target)` and the concrete migration approach is
recorded in "Signature-collision migration" below — Rust forbids inherent
method overloads, so a target cannot simply coexist with the current
signature under the same name.

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
| `roml::prelude::Model`, `ModelError`, `ConstraintSpec`, `ObjectiveSpec`, `LinExpr`, `Sense`, `Bounds`, `VarType`, `Solution` | **golden path** | Core model/solution vocabulary; retained in the curated P23 prelude. Raw IDs (`VarId`/`ConId`/`ObjId`/`ParamId`), `ValueExpr`, `SolverError`, `ConstraintBounds`, and `SolutionBuilder`/`SolutionStore` are classified individually below, not here. |
| `roml::prelude::ConstraintExprExt`, `ObjectiveExprExt` | **golden path** | Fluent `.le/.ge/.eq/.between/.minimize/.maximize` (API-04.3). |
| `roml::prelude::constrain`, `set_objective` (macros) | **compatibility/deprecated** | Effectful macros (D1); replaced by `Model::add_constraint`/`Model::minimize`/`Model::maximize`. Deprecated in P23. |
| `roml::Change` (root and prelude re-exports; `model::changelog::Change`) | **advanced backend extension** | Change journal (API-07.2); move to `roml::backend`. |
| `roml::DeltaBatch`, `ModelOp` | **advanced backend extension** | Protocol types (API-07.2); move to `roml::backend`. |
| `roml::ModelRevision`, `ModelSnapshot` | **advanced backend extension** | Protocol types (API-07.2); move to `roml::backend`. |
| `roml::BackendSession`, `Synchronization`, `SyncReceipt`, `SessionHealth`, `SolutionView`, `BackendMetadata`, `CallbackSession` | **advanced backend extension** | Backend contract (API-07.2/07.3); move to `roml::backend`. |
| `roml::AdapterCursor`, `AdapterHealth`, `ApplyOutcome` | **advanced backend extension** | Sync protocol (API-07.2); move to `roml::backend`. |
| `roml::BackendError`, `ErrorCategory`, `HealthEffect`, `TerminationStatus`, `BackendCapabilities` | **advanced backend extension** | Backend error/capability contract; move to `roml::backend`. |
| `roml::SolveRequest`, `SolveResult`, `SolveSolution`, `EffectiveConfig`, `ConfigAdjustment`, `ConfigRejection` | **advanced backend extension** | Solve-request protocol (D4/D9); move to `roml::backend`. |
| `roml::LpAlgorithm` | **advanced backend extension** | Solve-policy enum used by `SolveRequest`; move to `roml::backend`. |
| `roml::Solution` | **golden path** | Unified golden-path result type (API-03.1). |
| `roml::SolutionBuilder`, `SolutionStore` | **advanced backend extension** | Result construction/storage for frameworks; see Section 6. |
| `roml::ConstraintBounds` | **advanced backend extension** | Raw constraint-bounds form; the golden path uses spec builders (API-04.1). |

## 2. Model constructors and mutators (API-04, API-05, API-06)

| Item | Disposition | Replacement / note |
|---|---|---|
| `Model::new()` | **golden path** | Retained. |
| `Model::with_name(name)` / `Model::named(name)` | **golden path** (target) | `named` is the P22 target name-bearing constructor (target fixture). |
| `Model::add_var()` | **compatibility/deprecated** | Replaced by `add_variable(continuous())` (D7) in P22. |
| `Model::add_binary()` | **compatibility/deprecated** | Replaced by `add_variable(binary())` (D7). |
| `Model::add_integer(Bounds)` | **compatibility/deprecated** | Replaced by `add_variable(integer())` with optional `.bounds(...)` (D7). Kept as a wrapper, but its return type becomes fallible (`Result<VarId, ModelError>`, D10) — see "Signature-collision migration". |
| `Model::add_variable(Bounds, VarType)` | **compatibility/deprecated** (removed, not overloadable) | Current signature (infallible, returns `VarId`). The target `add_variable(VariableDef)` (D7) cannot coexist — Rust forbids overloads. Approach: intentional pre-1.0 break; the two-arg form is removed in the same release P22 adds the builder form; `add_var()`/`add_integer(Bounds)`/`add_binary()` remain as the deprecated compatibility wrappers. See "Signature-collision migration". |
| `Model::add_parameter(f64)` | **compatibility/deprecated** | Replaced by `add_parameter(parameter(value))` (D7) in P22. Single-argument shape allows an `Into<ParameterDef>` bridge (`From<f64>`) preserving the call shape; return type changes — see "Signature-collision migration". |
| `Model::add_constraint(ConstraintBounds)` | **golden path** `(target)` | Current signature takes raw bounds (returns `ConId`). The target `add_constraint(spec)` (API-04.1, D1) is canonical and subsumes it via the generic pattern `S: Into<ConstraintSpec>` that `Model::constrain` already uses, with `impl From<ConstraintBounds> for ConstraintSpec` — both call shapes compile through one method. See "Signature-collision migration". |
| `Model::constrain(spec)` | **compatibility/deprecated** | Current canonical-by-habit form (generic `S: Into<ConstraintSpec>`). Superseded by `add_constraint(spec)` (API-04.1, D1); deprecated in P23 after the replacement compiles (D12). |
| `Model::constraint(spec)` | **compatibility/deprecated** | Redundant alias of `constrain`; superseded by `add_constraint(spec)`; deprecate in P23. |
| `Model::add_constraint_expr(expr, bounds)` | **advanced backend extension** | Low-level constraint mutation; sparse/advanced. |
| `Model::minimize(expr)` / `Model::maximize(expr)` | **golden path** | Canonical single-objective mutations (API-04.2, D1). |
| `Model::set_objective(spec)` | **compatibility/deprecated** | Superseded by `minimize`/`maximize` (D1); deprecated in P23. |
| `Model::add_objective_expr(expr, sense)` / `add_objective_spec(spec)` | **advanced backend extension** | Low-level objective mutation; returns `(ObjId, constant)` pair. |
| `Model::set_active_objective`, `clear_active_objective`, `active_objective`, `num_objectives` | **advanced backend extension** | Multi-objective control; the ordinary path uses `minimize`/`maximize`. |
| `Model::objective_constant(obj)`, `active_objective_constant()` | **golden path** | Objective constant inspection (API-03.5). |
| `Model::set_parameter(param, f64)` (infallible) | **compatibility/deprecated** (return type breaks) | Current signature returns `()`. The target is fallible `set_parameter(param, value) -> Result<(), ModelError>` (D10/API-06.3, target fixture); old and new signatures cannot coexist (return type is not part of the method key). Approach: intentional pre-1.0 break in P22; statement-style call sites keep compiling for the window, gaining a `#[must_use]` warning. See "Signature-collision migration". |
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
| `ConstraintSpec`, `ObjectiveSpec` | **golden path** | Builder result types consumed by `Model::constrain` today; the canonical target consumer is `Model::add_constraint(spec)` (API-04.1). |
| `ValueExpr` (persistent parameter expressions) | **advanced backend extension** | Parameter-dependent coefficient expressions. |
| `Term`, `TermCoeff` | **advanced backend extension** | Expression internals; keep public for advanced algebra but not in prelude. |
| `LinExpr::compile_for_constraint/compile_for_objective`, `simplify`, `evaluate` | **advanced backend extension** | Advanced expression surgery. |
| Operator impls on `VarId`/`f64`/`ValueExpr`/`ParamId` (`Mul`, `Add`, `Into<LinExpr>`) | **golden path** | The algebra a user writes (`3.0 * x + y`). |

## 4. Macros (API-04.4)

| Item | Disposition | Replacement / note |
|---|---|---|
| `constraint!` | **optional syntax sugar** | Pure `ConstraintSpec` builder; may remain (D1). |
| `objective!` | **optional syntax sugar** | Pure `ObjectiveSpec` builder; may remain (D1). |
| `constrain!` | **compatibility/deprecated** | Effectful (D1); replaced by the canonical `model.add_constraint(...)` (API-04.1). Deprecated in P23. |
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
| `SolverError` (`roml::solver::SolverError`) | **compatibility/deprecated** | Current public error type for the solve path (simple `pub struct SolverError(pub String)`). Superseded by `SolveError` (D4, API-01.2) once the P21 façade lands; deprecated in P23 after `SolveError` compiles (D12). |
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

Full target bodies are frozen as compile-and-run tests in
`roml-highs/tests/target_quickstart.rs` and
`roml-highs/tests/target_incremental.rs` (promoted from the P20
`tests/ui/` fixtures by P21).

## Deprecation order (D12: replacement before deprecation)

| Order | Deprecated item | Replacement | When |
|---|---|---|---|
| 1 | `drain_changes()` (destructive drain) | implicit commit + revisioned sync in `SolverSession<B>` | after P21 |
| 2 | `set_objective`, `set_objective!` | `minimize`/`maximize` | after P21 |
| 3 | `constrain!` | `model.add_constraint(...)` | after P21 |
| 4 | `add_var`, `add_binary`, `add_integer`, `add_parameter(f64)` | definition-builder forms (`add_integer` and `add_parameter` carry return-type breaks — see Signature-collision migration) | after P22 |
| 5 | `add_variable(Bounds, VarType)` | removed in the same release the builder form lands (pre-1.0 break); wrappers `add_var`/`add_integer`/`add_binary` from row 4 cover the old shapes | after P22 (see Signature-collision migration) |
| 6 | infallible `set_parameter(param, f64)` | fallible `set_parameter` (return-type break, pre-1.0) | after P22 (see Signature-collision migration) |
| 7 | `Model::constrain`, `Model::constraint` (aliases) | `Model::add_constraint(spec)` (API-04.1) | after P22 |
| 8 | Protocol/sync/ID items in the default prelude | `roml::advanced` / `roml::backend` | after P23 curation |

All deprecations are documented in `MIGRATION.md` and `CHANGELOG.md` and remain
tested for the chosen window (API-08.2/08.3).

## Signature-collision migration (Rust has no method overloading)

D12 promises "replacement before deprecation". Five current signatures cannot
coexist with their targets under the same method name, so each needs an
explicit approach. The packet's accepted assumption — "M2 may make documented
pre-1.0 breaking changes with migration notes" — applies where noted.

**"Compatibility" here means *input-shape* compatibility only.** Every target
method is fallible (D10), so return types change on every bridged or
replaced method (`ConId` → `Result<ConId, ModelError>`, `VarId` →
`Result<Variable, ModelError>`, `()` → `Result<(), ModelError>`). A bridge
preserves how the call is *written*; it cannot preserve the old infallible
semantics. Statement-style call sites that ignore the result keep compiling
for the window, gaining a `#[must_use]` warning.

| Collision | Current | Target | Approach |
|---|---|---|---|
| `Model::add_variable` | `add_variable(Bounds, VarType) -> VarId` (infallible) | `add_variable(VariableDef) -> Result<Variable, ModelError>` (D7) | **Intentional pre-1.0 break.** The two-arg call shape cannot be preserved through a generic (two arguments vs one). The two-arg form is removed in the same pre-1.0 release that adds the builder form; `add_var()`, `add_integer(Bounds)`, and `add_binary()` (row 4) remain as the deprecated compatibility wrappers for those exact shapes until P23. MIGRATION.md documents the mechanical rewrite. |
| `Model::set_parameter` | `set_parameter(ParamId, f64)` (infallible, `-> ()`) | `set_parameter(Parameter, f64) -> Result<(), ModelError>` (D10/API-06.3; `Parameter` is a D8 alias of `ParamId`, so the parameter type is unchanged in effect) | **Intentional pre-1.0 break.** Return type is not part of the method key, so the two signatures cannot coexist; fallibility is mandatory for release-mode validation (D10). The method's return type changes in one release. Statement-style call sites (`model.set_parameter(p, 3.0);`) keep compiling for the window, gaining a `#[must_use]` warning; MIGRATION.md documents `_ = …` and `?` handling. |
| `Model::add_parameter` | `add_parameter(f64) -> ParamId` (infallible) | `add_parameter(ParameterDef) -> Result<Parameter, ModelError>` (D7) | **Generic compatibility input (partial).** Single-argument shape allows one method `add_parameter<P: Into<ParameterDef>>` with `impl From<f64> for ParameterDef` (initial value, no name), so `add_parameter(1.0)` and `add_parameter(parameter(1.0))` both compile. Input-shape compatible only: return type changes `ParamId` → `Result<Parameter, ModelError>`. |
| `Model::add_constraint` | `add_constraint(ConstraintBounds) -> ConId` (infallible) | `add_constraint(spec) -> Result<ConId, ModelError>` (API-04.1) | **Generic compatibility input (partial).** One method `add_constraint<S: Into<ConstraintSpec>>` (the pattern `Model::constrain` already uses) plus `impl From<ConstraintBounds> for ConstraintSpec` preserves the raw-bounds input shape; return type changes `ConId` → `Result<ConId, ModelError>` (D10). **Required internal refactor:** `add_constraint_expr` currently calls the public `add_constraint(bounds)` (`src/expr/linear.rs:578`); once the public method becomes the generic spec API, that call must route through a private primitive `add_empty_constraint(bounds)` (arena insert + changelog) so the expr-only path neither round-trips through `ConstraintSpec` nor captures the generic's spec semantics. Fallback if the bridge proves infeasible in P22: intentional pre-1.0 break for the raw-bounds form. |
| `Model::add_integer(Bounds)` | `add_integer(Bounds) -> VarId` (infallible) | kept as compatibility wrapper (deprecation row 4); semantics via `add_variable(integer().bounds(...))` (D7) | **Return-type break.** The wrapper stays but must become fallible: `add_integer(Bounds) -> Result<VarId, ModelError>` — D10 forbids infallible mutation where invalid bounds are possible (API-06.1/06.4). Invalid bounds surface as typed errors instead of asserting. `add_var()` and `add_binary()` take no inputs, have no failure mode, and may remain `-> VarId`. |

These approaches are recorded here so P22 implements the migration instead of
guessing it. `constrain`/`constraint` and the builder wrappers carry the
normal deprecation cycle (replacement compiles first, then deprecation).

## Open items inherited from planning (recorded in M2 STATE.md)

- Exact compatibility window for the two effectful macros.
- Whether `SolveStatus` replaces `SolverStatus` immediately or ships as an alias.
- Exact name of the generic core façade (`SolverSession<B>` unless compile ergonomics prove otherwise).
- Whether semantic aliases are type aliases or transparent wrappers (default: aliases).

These are decided during P21/P22 implementation, not guessed here; the
dispositions above do not depend on their resolution.
