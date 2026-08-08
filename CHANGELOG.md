# Changelog

All notable changes to ROML are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
once a 1.0.0 release is published. Prior to 1.0.0, breaking changes may occur
between minor versions.

## [Unreleased] — Pre-1.0 Hardening Program

### Deprecated

#### Surface curation (P23)
- **`Model::add_var()`** — use `Model::add_variable(continuous())` (D7).
- **`Model::add_binary()`** — use `Model::add_variable(binary())` (D7).
- **`Model::add_integer(Bounds)`** — use `Model::add_variable(integer().bounds(...))` (D7).
- **`Model::constrain(spec)` / `Model::constraint(spec)`** — use `Model::add_constraint(spec)` (API-04.1).
- **`constrain!` (effectful macro)** — use `model.add_constraint(constraint!(...))` or fluent specs.
- **`Model::set_objective(spec)` / `set_objective!` (effectful macro)** — use `model.maximize(expr)` / `model.minimize(expr)`.
- **`Model::drain_changes()`** — use `model.commit()`; the `roml_highs::Highs` façade synchronizes automatically.
- **`Model::add_parameter(f64)`** — the call shape is preserved via the `Into<ParameterDef>` bridge, but `model.add_parameter(parameter(value))` is the recommended definition form (see `MIGRATION.md`).

All deprecated APIs remain tested for the pre-1.0 window (API-08.3); the full
before/after migration is in `MIGRATION.md`.

### Added

#### MPS import qualification (P35, in progress)
- Added solver-independent fixed/free MPS stream and path readers with
  transactional staging, typed source-aware diagnostics, deterministic LP/MILP
  semantics, and explicit row/variable provenance.
- Added synthetic, metamorphic, fuzz-surface, HiGHS differential, pinned
  Netlib, and imported P29 IIS qualification coverage. Chinneck archive
  materialization remains a qualification gate before P35 completion.

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

#### M3 semantic modeling and solve workflows (P25–P33)
- **Semantic IR and four-identity provenance** (P25) — `ModelLineageId`,
  `ModelInstanceId`, `ModelRevision`, exact `CompilationId` identity on every
  solution; canonical semantic constructs preserved in model state.
- **Compiler/backend IR boundary** (P26) — `BackendSnapshot`,
  `BackendDeltaBatch` with exact from/to identity envelopes, typed
  `BackendFeature` capabilities, `EntityOrigin` on every generated entity,
  and the `Highs` session synchronized through compiled IR only.
- **Persistent fixing, assignments, locks, reversible overlays** (P27) —
  `Model::fix_variable` / `release_variable`, `PrimalAssignment` with
  lineage/instance/revision provenance, `SolutionLock`, and
  `SolveOverlay` with temporary fixings/locks/objective-locks/cutoffs
  applied and rolled back per solve attempt (`Highs::solve_with_overlay`).
- **Solve plans, warm starts, hints, effective-plan reporting** (P28) —
  `SolvePlan` (options + overlay + starts + hints + objective override +
  unsupported-feature policy), `MipStart`/`RepairPolicy`,
  `VariableHints`/`HintPriority`, default-reject `UnsupportedFeaturePolicy`
  with explicit recorded conversions, `Highs::solve_plan` as the single
  plan executor, and `Solution::metadata().effective_plan` carrying applied
  features, adjustments, rejections, and the exact compilation identity.
  HiGHS start support is qualified from the pinned official header audit.
- **Common construct library** (P32) — indicator, boolean, cardinality,
  min/max, absolute value, and binary-product constructs
  (`Model::add_indicator` / `add_boolean` / `add_cardinality` /
  `add_minmax` / `add_absolute_value` / `add_binary_times_linear`), each
  returning a stable `Construct` handle and compiling through the portable
  bridge with origin-complete generated entities.
- **Piecewise-linear functions and bound analysis** (P33) —
  `Model::add_piecewise_linear` with explicit relation
  (epigraph/hypograph/exact graph) and extrapolation policy; deterministic
  curvature classification; zero-binary convex epigraph / concave hypograph;
  exact segment-binary representation for exact/nonconvex graphs (never a
  convex relaxation); no unproven Big-M; typed `PwlEvalError` with
  parameter-resolver variants for parameterized point values.
- **Showcase examples** — `pwl_production_planning`, `warm_start_mip`,
  `overlay_solve`, `constructs` under `roml-highs/examples/`, exercising the
  M3 capabilities end-to-end with HiGHS.

### Changed

#### Documentation and consumer qualification (P24)
- **Rewritten README** — the golden-path HiGHS solve and incremental
  parameter-update examples are the primary content, both extracted as
  compiled-and-run fixtures (`roml-highs/tests/readme_quickstart.rs`,
  `readme_incremental.rs`). Root protocol imports are presented as
  legacy/migration-era; the curated prelude + `roml::advanced` are the
  recommended surfaces.
- **Rewritten modeling guide** (`MODELING_API.md`) — 11 chapters teaching the
  canonical path first with labeled advanced escape hatches. Every snippet is
  compiled (`roml-highs/tests/modeling_guide.rs`) or linked to a compiled
  example.
- **Examples moved to `roml-highs/examples/`** — `simple_lp`, `simple_mip`,
  `parameter_update`, `solve_options`, `sparse_build`. They solve with HiGHS,
  so they live in the backend crate and compile under the HiGHS CI targets.
  The solver-free `roml` examples were removed.
- **Rustdoc closure** — `missing_docs` is enabled (warn) on both crates and
  the public surface is fully documented, including `# Errors` sections on the
  `Highs`/`SolverSession` façade and `SolveStatus::from_termination`.

#### Surface curation and validation (P23)
- **Curated default prelude** — `roml::prelude` now exports only common model,
  expression, definition, solver, solution, and error types (API-07.1).
  Protocol/backend types (`Change`, `CoeffId`, `DeltaBatch`, `ModelOp`,
  `ModelRevision`, `ModelSnapshot`, `AdapterCursor`, `AdapterHealth`,
  `Synchronization`, `BackendSession`, `SyncReceipt`) are absent from the
  prelude (API-07.2) and grouped under `roml::advanced` (API-07.3).
- **Packaging hygiene (P24)** — `roml` gained an `include` filter so the
  packed crate contains exactly its intended files (no repo-level `.planning/`,
  `tools/`, `.foundry.toml`, `badges/`, or `docs/knowledge/` leakage);
  `roml-highs` gained a matching `include` filter.
- **HiGHS feature wiring fixed (P24)** — `roml-highs` `bundled` and `system`
  features now map to `highs-sys` `build`/`discover`. Previously both were
  no-ops and `system` silently built HiGHS from source instead of discovering
  an installed library.
- **`roml::advanced` namespace** — backend contract, revisions, snapshots,
  deltas, cursors, capabilities, callbacks, raw IDs, and expression internals
  with explicit stability and semver documentation; `IdArena` made
  crate-private (API-07.4).
- **`VarId - VarId` expression operator** — `x - y` now compiles, mirroring
  the existing `x + y` form.
- **Validation is release-safe** — `set_variable_bounds`,
  `set_constraint_bounds`, `set_semicontinuous`, and the raw
  `add_constraint_coefficient`/`add_objective_coefficient` mutators reject
  NaN/inverted/non-finite inputs with typed errors in all build profiles;
  `add_constraint` and `add_constraint_expr` reject NaN constraint bounds
  atomically (API-06, D10).

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
