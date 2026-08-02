# Phase 22 — Modeling Ergonomics and Entity Names

> **For agentic workers:** preserve existing algebra and canonical-cell behavior. Builders are convenience inputs to one validated model representation.

**Goal:** create one discoverable, named, method-first modeling API for LP/MILP and parameterized coefficients.

**Requirements:** API-04, API-05, API-06.

## Planned file structure

Create:

- `src/model/definition.rs` — `VariableDef`, `ParameterDef`, constructor functions.
- `tests/modeling_ergonomics.rs` — golden-path behavioral tests.
- `tests/named_entities.rs` — lifecycle and formatting tests.

Modify:

- `src/model/mod.rs`
- `src/model/variable.rs`
- `src/model/parameter.rs`
- `src/model/constraint.rs`
- `src/model/objective.rs`
- `src/expr/linear.rs`
- `src/lib.rs`

## Interfaces

```rust
pub type Variable = VarId;
pub type Constraint = ConId;
pub type Objective = ObjId;
pub type Parameter = ParamId;

pub fn continuous() -> VariableDef;
pub fn integer() -> VariableDef;
pub fn binary() -> VariableDef;
pub fn parameter(value: f64) -> ParameterDef;
```

```rust
impl VariableDef {
    pub fn bounds(self, lower: f64, upper: f64) -> Self;
    pub fn lower_bound(self, lower: f64) -> Self;
    pub fn upper_bound(self, upper: f64) -> Self;
    pub fn named(self, name: impl Into<String>) -> Self;
}

impl ParameterDef {
    pub fn named(self, name: impl Into<String>) -> Self;
}
```

```rust
impl Model {
    pub fn named(name: impl Into<String>) -> Self;
    pub fn add_variable(&mut self, def: VariableDef) -> Result<Variable, ModelError>;
    pub fn add_parameter(&mut self, def: ParameterDef) -> Result<Parameter, ModelError>;
    pub fn add_constraint(&mut self, spec: ConstraintSpec) -> Result<Constraint, ModelError>;
    pub fn minimize<E: Into<LinExpr>>(&mut self, expr: E) -> Result<Objective, ModelError>;
    pub fn maximize<E: Into<LinExpr>>(&mut self, expr: E) -> Result<Objective, ModelError>;
}
```

## Task 1 — Add validated definitions

- [ ] Write tests for default continuous `[0,+inf)`, integer `[0,+inf)`, and binary `[0,1]` definitions.
- [ ] Write rejection tests for inverted bounds, NaN, invalid infinities, and binary bounds outside `[0,1]`.
- [ ] Ensure failed creation does not change counts, changelog, or revision.
- [ ] Implement definitions by converting into the existing canonical variable/parameter representation.
- [ ] Commit as `feat: add validated model entity definitions`.

## Task 2 — Expose semantic aliases and names

- [ ] Add alias compile tests proving existing expression operators work unchanged.
- [ ] Add named creation tests for all entity types.
- [ ] Add getters `variable_name`, `parameter_name`, `constraint_name`, and `objective_name` returning `Option<&str>` or typed stale-ID errors according to the accepted consistency rule.
- [ ] Test duplicate names and document that names are diagnostics, not unique keys.
- [ ] Test names survive clone, snapshot/rebuild metadata where represented, and ordinary mutation.
- [ ] Commit as `feat: expose named model entities`.

## Task 3 — Canonical constraint path

- [ ] Add tests for `add_constraint((x + y).le(4.0))`, equality, lower bound, ranged constraint, expression constant adjustment, parameter coefficients, and named constraints.
- [ ] Route `add_constraint` through the existing expression compilation/canonical-cell path.
- [ ] Keep raw bounds-only row creation as an explicitly advanced method such as `add_empty_constraint`.
- [ ] Ensure no ambiguity with existing `add_constraint(ConstraintBounds)` remains.
- [ ] Commit as `feat: establish canonical constraint API`.

## Task 4 — Canonical objective path

- [ ] Add tests for `minimize`/`maximize`, objective constants, replacement/activation, parameter coefficients, and optional named variants.
- [ ] Preserve advanced multiple-objective creation and switching under explicit names.
- [ ] Ensure ordinary single-objective calls activate the returned objective exactly once.
- [ ] Commit as `feat: establish canonical objective API`.

## Task 5 — Clarify sparse coefficient operations

- [ ] Add cell-coordinate tests for replace, algebraic add, and remove.
- [ ] Implement public advanced methods with exact semantics:

```rust
set_coefficient(target, variable, value)
add_to_coefficient(target, variable, value)
remove_coefficient_at(target, variable)
```

- [ ] Keep raw `CoeffId` operations only in the advanced API.
- [ ] Verify canonical cell count remains one for repeated additions.
- [ ] Commit as `feat: clarify sparse coefficient mutation semantics`.

## Task 6 — Model diagnostics

- [ ] Update model formatting to prefer names and fall back to stable debug handles.
- [ ] Add tests for named constraints/objectives in diagnostic text.
- [ ] Ensure formatting never panics on removed/stale IDs.
- [ ] Commit as `feat: improve named model diagnostics`.

## Verification

```bash
cargo fmt --all -- --check
cargo clippy -p roml --all-targets -- -D warnings
cargo test -p roml --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
```

## Gate

P22 passes when LP, MILP, parameterized, named, and sparse examples use one coherent style; validation is atomic and release-safe; and canonical-cell/expression characterization remains green.