# Changelog

All notable changes to ROML (Rust Optimization Modeling Library) are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
once a 1.0.0 release is published. Prior to 1.0.0, breaking changes may occur
between minor versions.

This changelog tracks changes during the pre-1.0 hardening program.

## [Unreleased]

### Added

- Initial core model layer: variables, constraints, objectives, parameters, coefficients
- Solver-agnostic model API with change tracking via `ChangeLog`
- `SolverAdapter` trait for solver backend integration
- `Solution` and `SolutionStore` for immutable solution introspection
- `LinExpr` builder DSL for constructing linear expressions
- `ValueExpr` persistent AST for parameter-dependent coefficient values
- `IdArena<T>` typed identifier and arena allocation system
- HiGHS solver adapter (`roml-highs`) with FFI bindings and incremental model updates
- MOSEK solver adapter (`roml-mosek`) — compile-only (requires license for solve)
- FICO Xpress solver adapter (`roml-xpress`) — compile-only (requires license for solve)
- `Transaction` system for batched parameter updates
- MIP callback support across HiGHS, MOSEK, and Xpress adapters
- Semi-continuous variable support
- `SolveOptions` plumbing for per-solve LP algorithm override
- `set_variable_type` and `set_binary` methods on `Model`
- Batching for incremental model updates in the Xpress adapter
- `init_logging()` with log4rs configuration and automatic workspace-root discovery
- Workspace integration tests (`tests/changelog_integration.rs`)
- HiGHS integration tests (`roml-highs/tests/integration.rs`)

### Changed

- (None yet)

### Deprecated

- (None yet)

### Removed

- (None yet)

### Fixed

- (None yet)

### Security

- (None yet)
