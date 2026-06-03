# Repository Guidelines

## Project Overview

**ROML** (Rust Optimization Modeling Library) is a production-grade, incremental MILP (Mixed-Integer Linear Programming) modeling library. It provides a solver-agnostic model layer where users define variables, constraints, objectives, and coefficients, then solve through a concrete solver adapter (first target: HiGHS). The library supports efficient model mutation (add/remove/modify) with change tracking for incremental solver updates, parameter-driven coefficient expressions, and solution introspection (variable values, duals, reduced costs).

## Architecture & Data Flow

```
User Code
    │
    ▼
┌──────────────────────┐
│       Model          │  ← Solver-agnostic. Owns variables, constraints,
│  (src/model/*.rs)    │     objectives, parameters, coefficients.
│                      │     Mutations → ChangeLog.
└──────────┬───────────┘
           │ drain_changelog()
           ▼
┌──────────────────────┐
│    ChangeLog         │  ← Vec<Change> enum with old/new values.
│  (model/changelog.rs)│     Solver adapters consume this for incremental sync.
└──────────┬───────────┘
           │ apply_changes()
           ▼
┌──────────────────────┐
│   SolverAdapter      │  ← Trait in src/solver/mod.rs
│   (trait + impl)     │     Concrete impl: roml-highs (separate crate)
└──────────────────────┘
           │ solve()
           ▼
┌──────────────────────┐
│     Solution         │  ← Immutable. Stored separately from model.
│  (src/solution/*.rs) │     Multiple solutions kept (latest, named snapshots).
│                      │     Expression evaluation vs solutions without solver.
└──────────────────────┘
```

**Key design decisions:**

- **Model is solver-agnostic.** Solver concepts never leak into `src/model/`. The `SolverAdapter` trait is the sole bridge.
- **All mutations go through a ChangeLog.** The model never mutates solver state directly. Solver adapters consume the log to apply incremental updates.
- **IDs are stable and never reused.** `IdArena<T>` uses monotonic allocation with generation counters for staleness detection. Deleted slots are not reclaimed.
- **LinExpr is a temporary builder.** Expressions (the algebraic builder DSL) are compiled into coefficients and then discarded — not stored in the model. This differs from ValueExpr which IS stored and evaluated at coefficient-update time.
- **ValueExpr** is an AST for coefficient values that can depend on parameters. When parameters change, dependent coefficients re-evaluate without rebuilding the model.
- **Only one active objective at a time.** The ObjectiveStore enforces this by deactivating the current active when a new one is activated.
- **Transaction system** batches parameter changes. `set_param()` queues changes; `commit()` applies all and propagates to coefficients atomically.

## Key Directories

| Path | Purpose |
|---|---|
| `src/model/` | Core model entities: variable, constraint, objective, parameter, coefficient, changelog, transaction |
| `src/id/` | Typed identifiers (`VarId`, `ConId`, `ObjId`, `ParamId`, `CoeffId`) with arena allocation |
| `src/expr/` | `LinExpr` — temporary linear expression builder for constructing constraints/objectives |
| `src/value_expr/` | `ValueExpr` — persistent AST for parameter-dependent coefficient values |
| `src/solver/` | `SolverAdapter` trait + `SolverStatus`/`SolverError` types |
| `src/solution/` | `Solution`, `SolutionBuilder`, `SolutionStore` — immutable solution storage |
| `src/logging.rs` | `init_logging()` — log4rs initialization with automatic workspace-root discovery |
| `roml-highs/` | Separate crate: HiGHS solver adapter implementation |
| `tests/` | Integration tests at workspace level |

## Development Commands

```bash
# Build everything (workspace: roml + roml-highs)
cargo build

# Build just the HiGHS adapter
cargo build -p roml-highs

# Run all tests
cargo test --workspace

# Run specific test
cargo test -p roml-highs -- simple_lp_solve

# Run with logging visible
# (log4rs.yaml at root controls output; roml.log written to workspace root)
ROML_LOG_FILE=roml.log cargo test

# Check compilation without running
cargo check --workspace
```

- Workspace resolver v2, edition 2021.
- Dev profile: `opt-level = 0` (fast compile).
- Only `[profile.dev]` is set — no release profile configured yet.

## Code Conventions & Common Patterns

### Naming

- `define_id!` macro for typed IDs: `VarId`, `ConId`, `ObjId`, `ParamId`, `CoeffId` — each wraps `(u32, Generation)`.
- Store types are `{Entity}Store` (e.g. `VariableStore`, `ConstraintStore`), wrapping an `IdArena<{Entity}Data>`.
- Error types: `ModelError` (cloneable, `#[derive(Clone, Debug, PartialEq)]`), `SolverError` (cloneable, opaque).
- Public re-exports in `lib.rs` flatten core types: `pub use id::{VarId, ConId, ...}`.

### Error handling

- Homegrown error enums with `Display + Error` impls. No `thiserror` or `anyhow` dependencies.
- `ModelError` returned from model operations (validation failures).
- `SolverError` returned from solver adapter operations.
- `Result<(), Box<dyn std::error::Error>>` used in `init_logging()` and test/example harness code.
- `check_status()` helper in roml-highs maps HiGHS integer return codes to `SolverError::InternalError`.

### State management

- **Model struct** owns all stores inline (not behind `Rc`/`Arc`). Clone is derived.
- **Variables, constraints, objectives, parameters, coefficients** each have dedicated `Store` structs with `IdArena<Data>`.
- **ChangeLog** is separate from the model — drained explicitly via `model.drain_changelog()`, not auto-flushed.
- **Transaction** is owned by the model — uncommitted changes trigger a warning and auto-commit at solver sync.
- **SolutionStore** holds multiple solutions independently of the model.

### Logging

- Uses the `log` crate facade (`info!`, `warn!`, etc.).
- `init_logging()` configures `log4rs` from `log4rs.yaml` (or `LOG4RS_CONFIG` env var).
- `ROML_LOG_FILE` env var controls log file path; auto-set to workspace root if unset.
- `info!` for solver sync operations; `warn!` for stale IDs, missing entities.

### Common patterns

```rust
// All stores wrap an IdArena
pub struct VariableStore {
    arena: IdArena<VariableData>,
}

// Units of work on stores use Result or Option returns
pub fn add_variable(&mut self, data: VariableData) -> VarId { ... }
pub fn get(&self, id: VarId) -> Option<&VariableData> { ... }

// Method chaining for model operations
let expr = LinExpr::new().term(x, 2.0).term(y, 3.0);
let con = model.add_constraint(expr, ConstraintBounds::le(10.0));

// Operator overloading on IDs for expression DSL
// (VarId * f64, VarId + VarId, ParamId * VarId, etc.)
let expr = 2.0 * x + 3.0 * y;

// Infallible tests (Result<_, Box<dyn Error>>)
// Logging errors in tests are often ignored.
let _ = roml::init_logging();
```

### Cargo workspace

- Workspace members: `["."]` (roml crate itself) and `"roml-highs"`.
- `roml-highs` depends on `roml` via `path = ".."`.
- No feature flags used yet.

## Important Files

| File | Role |
|---|---|
| `src/lib.rs` | Crate root; module declarations and public re-exports |
| `src/main.rs` | Binary entry point (currently placeholder) |
| `src/model/mod.rs` | Core `Model` struct (~550 lines of impl) + `ModelError` |
| `src/model/variable.rs` | `VarType`, `Bounds`, `VariableData`, `VariableStore` |
| `src/model/constraint.rs` | `ConstraintBounds`, `ConstraintData`, `ConstraintStore` |
| `src/model/objective.rs` | `Sense`, `ObjectiveData`, `ObjectiveStore` |
| `src/model/parameter.rs` | `ParameterData`, `ParameterStore` |
| `src/model/coefficient.rs` | `CoefficientTarget`, `CoefficientData`, `CoefficientIndex` |
| `src/model/changelog.rs` | `Change` enum (22 variants), `ChangeLog` |
| `src/model/transaction.rs` | `Transaction` — batched parameter updates |
| `src/id/mod.rs` | `define_id!` macro, `Generation`, typed ID structs |
| `src/id/arena.rs` | `IdArena<T>` — monotonic arena with generation tracking |
| `src/expr/linear.rs` | `LinExpr`, `Term`, `TermCoeff` + operator overloads |
| `src/value_expr/mod.rs` | `ValueExpr` AST enum + operator overloads |
| `src/solver/mod.rs` | `SolverAdapter` trait, `SolverStatus`, `SolverError` |
| `src/solution/mod.rs` | `Solution`, `SolutionBuilder`, `SolutionStore` |
| `src/logging.rs` | `init_logging()` + workspace-root discovery |
| `log4rs.yaml` | Logging configuration (stdout + file, refresh 30s) |
| `config.yaml` | Environment overrides (e.g. `ROML_LOG_FILE`) |
| `Cargo.toml` | Workspace root manifest |
| `roml-highs/src/adapter.rs` | `HighsAdapter` — HiGHS solver adapter (~627 lines) |
| `roml-highs/src/ffi.rs` | Hand-written FFI bindings to HiGHS C API |
| `roml-highs/src/index_map.rs` | `IndexMap<Id>` — typed ID → HiGHS dense index |
| `tests/changelog_integration.rs` | Workspace-level integration tests for changelog |
| `roml-highs/tests/integration.rs` | HiGHS integration tests (LP, MIP, parameters, bounds) |

## Runtime/Tooling Preferences

- **Language**: Rust (edition 2021), no nightly features used.
- **Build system**: Cargo workspace.
- **Package manager**: Cargo (no JS toolchain needed).
- **Python**: `pyproject.toml` exists (v0.1.0, requires Python ≥3.13) but the `main.py` is a placeholder — the project is Rust-first. Python binding may be planned but not yet implemented.
- **Formatter**: No explicit `rustfmt.toml` — relies on defaults.
- **Linter**: No `clippy.toml` — runs with default clippy.
- **FFI**: Hand-written `extern "C"` bindings for HiGHS. Validates `HighsInt == i32` at runtime.
- **HiGHS integration**: Built via `roml-highs/build.rs` which locates HiGHS headers/libraries. HiGHS is expected to be pre-installed or linked by the build script.

## Testing & QA

- **Test framework**: Rust's built-in `#[test]` + `#[cfg(test)] mod tests { ... }` in every source file.
- **No external test framework** (no `rstest`, `proptest`, etc.).
- **Two test locations**:
  - Inline unit tests within each module (under `#[cfg(test)] mod tests`)
  - Integration tests: `tests/changelog_integration.rs` (workspace) and `roml-highs/tests/integration.rs` (HiGHS adapter)
- **HiGHS tests** follow a pattern: build model → `drain_changelog()` → `adapter.apply_changes()` → `solve()` → assert status/value.
- **Assertions**: `assert_eq!`, `assert!` with `approx_eq()` helper (1e-6 tolerance for f64 in HiGHS tests).
- **Logging in tests**: `init_test_logging()` helper (ignores errors silently) called at the start of integration tests.
- **tempfile** dependency for tests that need temporary directories.
- **Coverage**: Not yet configured. Run `cargo test --workspace` for complete validation.
- **Test verbosity**: `cargo test -- --nocapture` to see logging output during test runs.