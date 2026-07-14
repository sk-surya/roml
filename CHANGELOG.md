# Changelog

All notable changes to ROML are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
once a 1.0.0 release is published. Prior to 1.0.0, breaking changes may occur
between minor versions.

## [Unreleased] — Pre-1.0 Hardening Program

### Added

#### Core model correctness (P1)
- **Canonical coefficient cells** — duplicate terms for the same `(target, variable)` pair
  are algebraically combined (`p*x + q*x → (p+q)*x`) instead of overwriting.
- **`Model::validate_invariants()`** — debug/test invariant checker validating referential
  integrity, index consistency, active objective count, and cached value freshness.
- **Typed validation module** (`model::validation`) — `FiniteScalar`, `BoundValue`,
  `Tolerance` types with `debug_assert!` guards on parameter and bound inputs.
- **Characterization tests** — 53 tests capturing pre-P1 behavior; 4 ignored tests
  documenting known defects (last-write-wins, semi-continuous partial-apply, solve options).

#### Revisioned synchronization (P2)
- **`ModelRevision`** — monotonic revision counter with overflow detection.
- **`ModelSnapshot`** — deterministic projection of canonical model state at a revision.
- **`DeltaBatch` / `ModelOp`** — immutable, self-contained typed operation batches
  with explicit `from → to` revision pairs.
- **`Journal`** — `BTreeMap`-backed delta batch storage with sequential gap detection
  and `deltas_since(revision)` replay query.
- **`AdapterCursor` / `AdapterHealth`** — per-adapter progress tracking with
  `Ready` / `RequiresRebuild` / `Terminal` health states.
- **`SyncCoordinator`** — model-owned bridge between journal and multiple independent
  adapter cursors.
- **`ReferenceBackend`** — solver-neutral projection backend proving the
  commuting square: `project(r1) == apply(project(r0), deltas r0→r1)`.
- **`StagingTransaction` / `ModelTransaction`** — atomic transaction system that
  collects `ModelOp` values and commits them as `DeltaBatch` values.
- **Sync characterization tests** — 7 failing tests proving current destructive
  changelog weaknesses (all ignored, fixed by revisioned sync).

#### Solver boundaries (P3)
- **`BackendInfo` / `BackendCapabilities`** — granular capability flags for
  backend feature detection.
- **`BackendError` / `ErrorCategory` / `HealthEffect`** — categorized native
  errors with adapter health implications.
- **`TerminationStatus`** — precise solve termination status enumeration.
- **`SolveRequest` / `SolveResult`** — immutable solver policy with explicit
  apply/adjust/reject semantics (replaces `Model.solver_options`).
- **`validate_request()`** — capability-aware option validation.
- **Xpress binding decision document** — `docs/release/XPRESS_BINDING_DECISION.md`.

#### Repository infrastructure (P0, P4)
- **CI workflows** — 3-OS core matrix (Linux, macOS, Windows) with fmt, clippy,
  test, docs; policy workflow (audit, deny, unused-deps); MSRV job at Rust 1.85.
- **`deny.toml`** — `cargo-deny` configuration for advisories, licenses, bans.
- **Workspace lints** — `unsafe_code = "deny"` in core crate.
- **Governance documents** — `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md`,
  `RELEASE_CHECKLIST.md`, `SUPPORT_MATRIX.md`, `PACKAGING.md`.

#### Examples (P5)
- `examples/simple_lp.rs` — solver-free model construction demonstration.
- `examples/parameter_update.rs` — parameter propagation and canonical cell combining.

### Changed
- **Public API narrowing** — internal store types (`VariableStore`, `ConstraintStore`,
  `ObjectiveStore`, `ParameterStore`, `CoefficientIndex`, `ChangeLog`, `Transaction`)
  narrowed from `pub` to `pub(crate)`; data types (`VariableData`, `ConstraintData`,
  `ObjectiveData`, `ParameterData`, `CoefficientData`) likewise narrowed; their re-exports
  in `model::mod` are now `pub(crate)`. Internal modules (`journal`, `transaction`)
  narrowed to `pub(crate)`; `delta`, `snapshot`, `sync` kept `pub` for integration tests
  (to be narrowed in P5).
- **Documentation added** — field-level doc comments on `VariableEntry`,
  `ConstraintEntry`, `ObjectiveEntry`, `ParameterEntry`, `CellEntry`, and `ApplyOutcome`
  variants. Improved `ModelOp` variant documentation.
- **Unused imports removed** — `ConstraintData`, `ObjectiveData`, `VariableData`
  re-exports removed from `model/mod.rs` (not directly referenced).
- `ModelConstants::default()` no longer recursively calls itself.
- `add_constraint_coefficient` and `add_objective_coefficient` now emit
  `CoefficientValueChanged` when combining into an existing cell.
- `CoefficientIndex` now enforces one canonical cell per `(target, variable)` pair.
- ID types (`VarId`, `ConId`, `ObjId`, `ParamId`, `CoefficientTarget`) now implement
  `Ord` and `PartialOrd` for deterministic snapshot ordering.
- `ObjectiveStore` now exposes `active_count()`.

### Removed
- **`init_logging()`** — global logger initialization removed from core public API.
  Applications configure their own logger via the `log` facade.
- **`log4rs`, `serde_yaml`, `rand`** runtime dependencies removed from core;
  `rand` retained as dev-dependency.
- **Repository contamination** — Python scaffold (`main.py`, `pyproject.toml`,
  `uv.lock`), solver configuration (`config.yaml`, `log4rs.bak`), generated solver
  logs (`roml*.log`), IDE config (`.vscode/`), and Python tooling (`.python-version`).
- **Inherent `ModelConstants::default()`** — removed; use the `Default` trait impl.

### Fixed
- Canonical coefficient cells: duplicate parametric terms now produce mathematically
  correct combined values instead of last-write-wins.
- `ModelConstants::default()` recursion defect.
- Rustdoc broken intra-doc links and unclosed HTML tags.
- All clippy errors in core crate (lib + test targets).
- Workspace-wide rustfmt formatting.

### Security
- Core crate denies `unsafe_code` at the lint level.
- `roml-mosek` and `roml-xpress` gated with `publish = false`.
- No panic may cross FFI boundaries (enforced by design, P3 hardening in progress).
- Package `exclude` list prevents `.claude/`, `AGENTS.md`, `.github/` and planning
  artifacts from entering published crates.
