# Contributing to ROML

Thank you for your interest in contributing to ROML (Rust Optimization Modeling Library). This document provides guidelines and instructions for contributing to the project.

## Table of Contents

- [Development Prerequisites](#development-prerequisites)
- [Getting Started](#getting-started)
- [Solver-Free Development](#solver-free-development)
- [Backend-Specific Prerequisites](#backend-specific-prerequisites)
- [Conventional Commits](#conventional-commits)
- [Pull Request Expectations](#pull-request-expectations)
- [Testing](#testing)
- [Code Style](#code-style)
- [Project Structure](#project-structure)

## Development Prerequisites

- **Rust toolchain**: 1.75 or later (edition 2021, no nightly features used)
- **Cargo**: included with Rust
- **No solver required for core development**: the `roml` core crate compiles and tests without any third-party solver installed

The project uses:
- Cargo workspace with resolver v2
- Stable Rust only; no nightly features
- Formatting and linting via `rustfmt` and `clippy` (both shipped with the Rust toolchain)

## Getting Started

```bash
# Clone the repository
git clone <repo-url>
cd roml

# Build the entire workspace
cargo build

# Build just the core crate
cargo build -p roml

# Run all workspace tests
cargo test --workspace

# Check compilation without running tests
cargo check --workspace
```

## Solver-Free Development

Most development work on the core `roml` crate does not require any solver backend. The core crate provides the model layer, expression DSL, change tracking, and solution storage.

```bash
# Build only the core crate (no solvers required)
cargo build -p roml

# Run only core crate tests (no solvers required)
cargo test -p roml

# Run clippy on the core crate
cargo clippy -p roml

# Format the core crate
cargo fmt -p roml
```

Integration tests for solvers and workspace-level tests that exercise `roml-highs` do require the HiGHS backend to be installed.

## Backend-Specific Prerequisites

Each solver adapter is optional and lives in its own crate within the workspace. You only need to install the solver(s) relevant to your work.

### HiGHS (`roml-highs`)

- **Required for**: developing HiGHS adapter features, running HiGHS integration tests
- **Installation**: HiGHS must be pre-installed on the system. The `roml-highs/build.rs` script locates the HiGHS headers and libraries automatically.
- **Verification**: `cargo build -p roml-highs` should succeed after installation.
- **Platforms**: Linux, macOS, and Windows are all supported.

### MOSEK (`roml-mosek`)

- **Required for**: developing the MOSEK adapter
- **Installation**: MOSEK must be installed and licensed separately. The adapter links against the MOSEK C API.
- **Compile-only without license**: The crate compiles without a MOSEK license, but solving requires a valid MOSEK license.
- **Platforms**: compile-only on Linux, macOS, and Windows.

### FICO Xpress (`roml-xpress`)

- **Required for**: developing the Xpress adapter
- **Installation**: Xpress Optimizer libraries must be installed separately.
- **Compile-only without license**: The crate compiles without an Xpress license, but solving requires a valid Xpress license.
- **Platforms**: compile-only on Linux, macOS, and Windows.

## Conventional Commits

If the project adopts conventional commits, the expected format is:

```
<type>(<scope>): <description>

[optional body]
```

Types:
- `feat`: a new feature
- `fix`: a bug fix
- `docs`: documentation changes
- `refactor`: code refactoring without feature change or bug fix
- `test`: adding or updating tests
- `perf`: performance improvements
- `ci`: CI configuration changes
- `chore`: maintenance tasks

Scopes (examples): `model`, `highs`, `mosek`, `xpress`, `solver`, `solution`, `expr`, `changelog`, `docs`.

This convention is not yet enforced; it is a recommended practice for contributors.

## Pull Request Expectations

- **One change per PR**: Keep pull requests focused on a single concern.
- **Tests included**: New features should include tests; bug fixes should include a regression test.
- **All tests pass**: Run `cargo test --workspace` before submitting.
- **Lint clean**: `cargo clippy --workspace` should produce no warnings for new code.
- **Formatted**: Code must be formatted with `cargo fmt` before committing.
- **Changelog entry**: Notable changes should include a corresponding entry in `CHANGELOG.md` under the `Unreleased` section.
- **Review process**: PRs require at least one review from a maintainer before merging.
- **CI**: All CI checks must pass before merging.

## Testing

The project uses Rust's built-in `#[test]` mechanism. No external test frameworks are required.

```bash
# Run all tests across the workspace
cargo test --workspace

# Run tests for a specific package
cargo test -p roml
cargo test -p roml-highs
cargo test -p roml-mosek
cargo test -p roml-xpress

# Run a specific test by name
cargo test -p roml-highs simple_lp_solve

# Run tests with logging output visible
ROML_LOG_FILE=roml.log cargo test -- --nocapture

# Run tests without capturing stdout/stderr
cargo test -- --nocapture
```

**Test organization:**
- Unit tests live inline in each source file under `#[cfg(test)] mod tests { ... }`.
- Integration tests live in `tests/` at the workspace root and in each backend crate's `tests/` directory.
- Integration tests typically follow the pattern: build model -> drain changelog -> apply changes to adapter -> solve -> assert.

**Assertions:**
- Standard `assert_eq!` and `assert!` macros are used throughout.
- Floating-point comparisons use an `approx_eq()` helper with 1e-6 tolerance in HiGHS tests.

## Code Style

- **Formatting**: All code must be formatted with `rustfmt` using default settings (no custom `rustfmt.toml`).
- **Linting**: All code must pass `cargo clippy` with no warnings. There is no custom `clippy.toml`; default clippy rules apply.
- **Error handling**: The project uses hand-written error enums with `Display + Error` impls. `thiserror` and `anyhow` are not used.
- **Naming conventions**:
  - Typed IDs use the `define_id!` macro (e.g., `VarId`, `ConId`, `ObjId`, `ParamId`, `CoeffId`).
  - Store types are named `{Entity}Store` (e.g., `VariableStore`, `ConstraintStore`).
  - Error types are named `{Domain}Error` (e.g., `ModelError`, `SolverError`).
- **Public API**: Public types are re-exported from `src/lib.rs` for a flat import experience: `use roml::{VarId, LinExpr, Model}`.
- **No unsafe code** in the core crate. Unsafe code is confined to FFI bindings in adapter crates.

## Project Structure

```
roml/                       # Workspace root / core crate
  src/
    model/                  # Core model entities and stores
    id/                     # Typed identifiers and arena allocation
    expr/                   # LinExpr builder DSL
    value_expr/             # ValueExpr persistent AST
    solver/                 # SolverAdapter trait
    solution/               # Solution storage and introspection
  roml-highs/               # HiGHS solver adapter (separate crate)
  roml-mosek/               # MOSEK solver adapter (separate crate)
  roml-xpress/              # FICO Xpress solver adapter (separate crate)
  tests/                    # Workspace-level integration tests
  docs/                     # Project documentation
```
