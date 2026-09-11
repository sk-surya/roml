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

#### Rust Level-1 array surface (MIR-04, unreleased)
- `Model::var(name, shape)` / `Model::param(name, shape, values)` return
  model-owned `VarArray` / `ParamArray` handles over the MIR-03 ordinal IR with
  metadata-only `slice` / `reverse` / `transpose` (no per-cell allocation).
- Cell-wise rows via `LinArray::{le, ge, eq, le_each, ge_each, eq_each}` and
  `Model::add_row`; leading-axis reduction rows via
  `LinArray::{rows_eq, rows_le, rows_ge}` and `Model::add_rows`; array
  objectives via `Model::maximize_array` / `Model::minimize_array` (packed
  parametric path with a general symbolic fallback).
- `Labeled<A>` attaches checked boundary labels (alignment mismatches are
  typed; labels never enter the IR); `Model::normalized_ordinal_fingerprint()`
  exposes a deterministic fingerprint over the normalized ordinal IR.
- `examples/l1_bess.rs`, `examples/l1_transportation.rs`, and
  `examples/l1_min_cost_flow.rs`, guarded by a source gate that rejects raw
  ids or manual linear-expression construction in ordinary model code.

#### Structural variable-array naming (P2A, unreleased)
- `m.vars()` no longer formats, hashes, or stores one string per
  element. Arrays own a structural reservation (base name + length);
  implicit `base[i]` names materialize on demand at handle creation,
  and a compact reverse index answers prospective-array collisions
  without string scans. Collision semantics are unchanged (including
  the constraint-names-vs-variable-elements asymmetry, out-of-range
  and zero-length behavior, and bracketed bases), as is atomic
  rejection. `repr(model)` counts come from canonical entity state.
  As a focused correction, sliced-view scalars now display the root
  ordinal (`x[2:7][0]` is `x[2]`, previously mislabeled `x[0]`; values
  always flowed by identity). 1M vars: ~330 ms → ~70 ms with
  substantially lower peak RSS. No public API or spelling change.

#### Lazy sinks converge onto bulk primitives (P1E, unreleased)
- Model sinks now classify a persistent lazy tree once and lower
  directly into the cheapest matching core primitive: all-numeric
  objectives into `set_linear_objective_bulk`, `scale × Param`
  objectives into `set_linear_objective_param_bulk`, and all-numeric
  single-row comparisons into one-row `add_linear_rows_bulk`.
  Classification is a single iterative walk that preserves structural
  information (numeric buffers never become per-term `ValueExpr`s on
  the fast paths); genuinely general expressions keep the unchanged
  general `Affine` path, including the core's documented duplicate
  handling. A 1M scalar chain inserts in ~81 ms instead of ~412 ms,
  and the previously stalled 200k general-path solve now syncs like
  the packed equivalent. No public API or spelling change.

#### Persistent lazy scalar expressions (P1D, unreleased)
- Scalar `Var`/`Param`/`Expr` algebra now builds a persistent immutable
  expression tree (`O(1)` per operator, structural sharing, no term
  copying, no per-operation canonicalization) instead of cloning and
  re-canonicalizing a flat term vector on every `+`. The old
  `total = total + v` loop was `O(N²)` (100k terms: ~31 s); it is now
  linear (100k: ~0.33 s, 1M: ~3.3 s, fitted exponent `p ≈ 0.94`).
- Packed forms stay packed through surrounding algebra (`rm.sum(x) + 5`,
  `2 * rm.sum(x)`) instead of materializing a million `ExprTerm`s at
  each step. Exactly one iterative flattening (explicit stack, no
  recursion) plus duplicate combination runs at each model sink, and
  deep-tree teardown is iterative as well.
- No public API or spelling change: construction-time nonlinear,
  foreign-model, finiteness, and division errors raise exactly as
  before; sinks lower to the identical canonical `Affine` handling.

#### Packed parameterized objectives (P1C-2, unreleased)
- New `Model::set_linear_objective_param_bulk(sense, vars, params, scales,
  constant)`: one fused validation scan, one packed parametric append
  (`scale * parameter` cells with evaluated caches plus a compact reverse
  parameter index, no `ValueExpr` per cell), one packed
  `Change::BulkObjectiveParamCoefficients` journal entry compiling to a
  single `ModelOp::SetObjectiveParamCells` delta op. Canonical state is
  identical to the scalar path (same-variable/same-parameter scales sum;
  same variable with distinct parameters installs through the general
  overlay with the combined expression). Packed parametric cells propagate
  parameter updates through the reverse index and shadow into the general
  overlay under the same identity on arbitrary symbolic mutation, exactly
  like packed constants.
- New `ValueExpr::scaled_param(scale, param)` canonical constructor (bare
  `Param` at unit scale, otherwise `Constant * Param` — the exact scalar
  fold shape, so snapshot/delta forms agree bit-for-bit).
- Python `rm.dot` with structurally cheap parameter-only coefficients
  (`ParamArray`, broadcast scalar `Param`, trivially representable
  parameter scalar expressions) over packed numeric decision arrays now
  lowers without per-element `Affine` expansion and inserts through the
  parametric bulk primitive. Mixed, general-expression, and
  parameter-dependent-constant cases keep the existing lowering with
  identical semantics; the public Python API is unchanged.

#### Packed array expressions (P1C-1, unreleased)

- Array arithmetic over `VarArray`s (`+`, `-`, negation, numeric
  scaling/division, slicing) and numeric comparisons now stay in a packed
  structural form until `m.add()`, which inserts through the bulk row
  primitive in one call. No per-element expression objects on this path.
  Anything parameterized, mixed, or densely-bounded falls back to the
  existing per-element machinery with identical semantics; the public
  Python API is unchanged.

#### Bulk linear rows (P1A/P1B, unreleased)
- New `Model::add_linear_rows_bulk(row_ptr, vars, values, bounds)`:
  whole-batch validation, per-row canonicalization (sorted variables,
  duplicate accumulation, near-zero drop — exactly matching the scalar
  row path), one packed `Change::BulkLinearRows` journal entry compiling
  to a single `ModelOp::AddLinearRows` delta op (at most one backend row
  op per row with coefficients inline).
- Python `Model.add_linear_rows` keeps its exact public contract and now
  routes directly to the bulk primitive (no per-row expression objects).
- Behavior note: CSR rows with sub-`EPSILON` nonzero coefficients are now
  dropped exactly like scalar-built rows (previously only exact zeros
  were dropped on the CSR path). Cancellation-to-zero and duplicate
  accumulation are unchanged.

#### Packed coefficient store (P1.5B, unreleased)
- Internal coefficient storage is now a packed constant base plus a sparse
  mutation overlay under stable generational `CoeffId` identities. Canonical
  cells, algebraic combine, removal cleanup, parameter propagation, stale-ID
  errors, and snapshot/delta equivalence are unchanged; per-cell hash
  topology is gone (bulk construction appends contiguously, the global
  variable index builds lazily on first use).
- `Model::coefficient()` now returns an owned `CoefficientData` snapshot
  instead of a reference (packed cells have no per-cell record to borrow);
  field reads work exactly as before.
- `Model::set_linear_objective_bulk` routes into packed construction.

#### Bulk constant-objective insertion (P0, unreleased)
- New `Model::set_linear_objective_bulk(sense, vars, coeffs, constant)`:
  one fused validation scan, one storage reservation, one packed
  `Change::BulkObjectiveCoefficients` journal entry compiling to a single
  `ModelOp::SetObjectiveCells` delta op — instead of one `simplify` plus
  one general coefficient mutation per term. Canonical state is identical
  to `minimize`/`maximize`; duplicates fall back to algebraic combine
  (R2.2); rejection is atomic (API-06.5).
- New `ModelError::MismatchedBulkLengths` for mismatched bulk inputs.
- Python: `rm.sum(VarArray)` / `rm.dot(numeric, VarArray)` stay packed
  into the core bulk primitive (no per-term `Affine` normalization);
  all other expressions keep the general path with identical semantics.

#### Python interface over persistent sessions (MPY, unreleased)

- New `roml-python` crate (PyO3 + maturin, `pip install roml-python`)
  exposing the solver-independent model with NumPy-shaped bulk
  modeling, atomic named parameter updates, and persistent `Highs`
  sessions returning immutable solution snapshots.
- Scalar and shaped expressions with parameter-dependent coefficients;
  `rm.sum` / `rm.dot` bulk reductions execute in Rust with one
  extension call per vector operation; CSR bulk rows accumulate
  duplicates algebraically.
- Detached native solves (GIL released), deterministic busy errors,
  explicit warm-start requests with measured disposition, LP-only
  duals/reduced costs, and honest solution metadata including the
  synchronization mode.
- Full typing (`py.typed`, strict-checked stubs) and runnable examples
  (scalar production LP, rolling-battery MPC).
- Known limitation: parameter-dependent objective *constants* are
  rejected explicitly (the core has no replaceable constant cell).
- Known limitation: long-lived models retain committed delta batches
  for lagging adapters (unbounded journal), so the 10k-cycle memory
  gate currently fails pending a journal-bounding design decision.

#### Persistent soft constraints and portable feasibility repair (P30)
- Added revisioned persistent soft-constraint handles with exact lower/upper
  violation rows, finite caps, parameterized nonnegative weights, and explicit
  `None`/`Objective` penalty targets.
- Added solve-scoped portable weighted-L1 feasibility relaxation with exact
  base/overlay identities, typed outcomes, acceptance policy, provider
  fallback metadata, P29 origin mapping, and cleanup/rebuild error retention.
- Objective-priority, lexicographic execution, and unqualified native
  feasibility-relaxation calls remain outside this phase.

#### MPS write-back qualification (P36)
- `MpsWriteReport::nonzeros` now counts only mathematically nonzero emitted coefficients; explicit and synthetic zero entries remain in the MPS output but are excluded from the report.
- Added deterministic solver-free free-MPS writing for representable linear
  LP/MILP models, including evaluated parameter snapshots, objective offsets,
  RHS/RANGES rows, variable domains, integer markers, typed errors, and
  transactional path publication.
- Added independent ROML semantic round-trip, native HiGHS structure/solve,
  and exact 94-model Netlib transcode qualification coverage.

#### MPS import qualification (P35, in progress)
- Added solver-independent fixed/free MPS stream and path readers with
  transactional staging, typed source-aware diagnostics, deterministic LP/MILP
  semantics, and explicit row/variable provenance.
- Added synthetic, metamorphic, fuzz-surface, HiGHS differential, pinned
  Netlib, and imported P29 IIS qualification coverage. Chinneck archive
  materialization remains a qualification gate before P35 completion.

#### M3 final qualification (P34, in progress)
- Frozen Q01–Q14 native/portable qualification corpus with hand-verified
  optima and formulation fingerprints; import-to-repair and MILP
  orchestration workflows; deterministic `P34_PRIMITIVE_PARAMETER_UPDATE_V1`
  perf gate; packed-consumer package protocol; executable fault matrix;
  capability truth table; NLP-readiness review.

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
- **Objective policies and lexicographic solves** (P31) — one canonical
  `ObjectivePolicy` / `ObjectivePriority`, portable sequential
  weighted/lexicographic execution with exact normalized `|z*|` degradation
  locks, `PenaltyTarget::Priority` integration, and provider-policy
  separation (`SolverSession::solve_objective_policy`). Explicit per-attempt
  options and a shared staged deadline via
  `solve_objective_policy_with_options` (`PolicyClock`/`SystemPolicyClock`
  seam); budget exhaustion preserves the last valid incumbent and reports
  `MultiObjectiveResult::budget_exhausted` or typed
  `ObjectiveExecutionError::BudgetExhausted` when no stage ran.

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
