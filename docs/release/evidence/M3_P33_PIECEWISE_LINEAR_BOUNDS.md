# P33 Evidence — Piecewise-linear Functions and Bound Analysis

**Phase:** 33-piecewise-linear-bounds
**Plan:** `33-PLAN.md` — Tasks 1 (PWL semantics tracer), 2 (one-sided zero-binary bridges), 3 (exact segment-binary graph + randomized equivalence + HiGHS)
**Requirements:** SM-13 (full closure), SM-14.1–14.7, SM-12.1/12.2/12.5 (closed by P32, recorded here), SM-12.8 (advanced)
**Branch:** `phase-roml-P33-piecewise-linear-bounds` (implementation worktree `/Users/skrishnan/repos/roml/.git/p33-impl`)
**Base:** `main@40af9f4` (restored main — full accepted P27/P32 implementation present)
**HEAD at execution start:** `c7cacf5` (`docs(33): add piecewise linear bounds phase plan`)
**Status:** Wave-0 baseline captured — implementation in progress.

This document records the P33 deliverables per `EXECUTION.md` § "Evidence file structure": the untouched baseline matrix, the re-pointing note, per-task TDD verification (RED failures first), the focused/full verification matrix, the public API diff, the zero-binary and exact-graph proofs, the randomized-equivalence corpus, the nonconvex-exactness proof, scaling diagnostics, representation report examples, deviations, and residual risks.

## Scope and requirements

P33 delivers safe convexity-aware PWL modeling and Big-M evidence over the frozen P26 compiler contract + P32 bridge framework: exact piecewise-linear semantics and formulations, with convex epigraph / concave hypograph compiled with zero binaries and exact/nonconvex graphs compiled exactly (never a convex relaxation), all reported with curvature, representation, generated counts, and binary-avoidance/introduction reasons.

**Native-vs-bridge honesty decision (mandatory statement):** This phase implements **no new native payloads** in `BackendConstraint` and **no HiGHS native PWL/SOS2 projection**. `BackendConstraint` stays empty; `native_payloads_available()` stays false; PWL is selected and reported as `SupportLevel::Bridge` with the exact graph compiled through the **deterministic exact segment-binary formulation** (ROML portable bridge). `BackendFeature::NativePiecewiseLinear` and `BackendFeature::Sos2` remain `SupportLevel::Unsupported` on HiGHS (no false native claim — P32 F4 rule). `NativeRequired` on a PWL construct rejects with `CompileError::UnsupportedFeature`. SOS2 is a declared possible representation only when a backend qualifies native `Sos2` (none does in M3); the actual emitted representation is exact segment binaries.

**Clause-level scope:** SM-13 is closed by P33 (the SM-13.1–13.6 compiler foundations were closed in P32; P33 extends interval analysis to the PWL argument expression and records bound-source traces). SM-12.1/12.2/12.5 were closed by P32 Task 16 and are recorded as closed in the P33 TRACEABILITY update; SM-12.8 is advanced by Task 1 (the PWL builder's stable-handle + formulation-diagnostics surface). SM-14.1–14.7 are closed by Tasks 1–3.

## Baseline and environment

| Item | Value |
|---|---|
| Base commit (`main`) | `40af9f4` (`restore P27 P32 files onto main`) |
| HEAD at baseline capture | `c7cacf5` (`docs(33): add piecewise linear bounds phase plan`) |
| Branch | `phase-roml-P33-piecewise-linear-bounds` (implementation worktree) |
| `rustc --version` | `rustc 1.97.1 (8bab26f4f 2026-07-14)` |
| `cargo --version` | `cargo 1.97.1 (c980f4866 2026-06-30)` |
| `rustc -vV` host | `aarch64-apple-darwin` |
| OS | `Darwin 25.4.0 arm64` |
| `cargo public-api --version` | `cargo-public-api 0.52.0` |
| HiGHS build | bundled via `highs-sys 1.15.0` (cmake); no system HiGHS |

### Re-pointing note (base restored)

The p33 branch base was originally the broken `f905b0d` (which had reverted the accepted P27/P32 implementation source despite the P32 merge `538336d` being an ancestor). The main-tree recovery commit `40af9f4` restored all P27/P32 files onto main, and the p33 worktree was re-pointed (`git reset --hard origin/main`). The branch base is now `main@40af9f4`, which contains the full accepted P32 implementation: `src/compiler/bounds.rs` (`BoundAnalyzer`, `Interval`, `BoundTrace`, `BoundSource`, one-sided Big-M helpers), `src/compiler/bridge/mod.rs` (`BridgeFinalizer`, `BridgeContext`, `select_path`, `native_payloads_available` gate) and the bridge/construct payload modules, `src/assignment.rs`, `src/solver/overlay.rs`, `src/solver/facade.rs`, and the P27/P32 test files. No restoration was required; Wave-0 is verification only. The plan commit `c7cacf5` sits on top of `main@40af9f4`; the evidence baseline records HEAD at execution start (`c7cacf5`) with base `main@40af9f4`.

### P32 file-presence verification

| File | Present |
|---|---|
| `src/compiler/bounds.rs` | yes |
| `src/compiler/bridge/mod.rs` | yes |
| `src/construct/minmax.rs` | yes |

Additional P32-anchored files verified present: `src/compiler/bridge/{indicator,reification,boolean,cardinality,minmax,absolute,product}.rs`, `src/construct/{absolute,boolean,cardinality,indicator,minmax,product,reification}.rs`, `src/assignment.rs`, `src/solver/overlay.rs`, `src/solver/facade.rs`.

### Untouched baseline matrix — `roml`

| Command | Exit | Result |
|---|---|---|
| `cargo fmt --all -- --check` | 0 | formatting clean |
| `cargo check -p roml --all-targets` | 0 | clean |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `cargo test -p roml --all-targets` | 0 | **887 passed; 0 failed; 0 ignored** |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 | docs generated, no warnings |
| `cargo package --list -p roml` | 0 | 110 files |

### Untouched baseline matrix — `roml-highs`

| Command | Exit | Result |
|---|---|---|
| `cargo check -p roml-highs --all-targets` | 0 | clean |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `cargo test -p roml-highs --all-targets` | 0 | **131 passed; 0 failed; 0 ignored** |
| `cargo package --list -p roml-highs` | 0 | 33 files |

`roml-mosek`/`roml-xpress` are known-broken against the current facade and out of scope (M2/M3 convention) — never exercised with workspace-wide commands. Every command in this phase is `-p roml` or `-p roml-highs` scoped.

## Commit trail

| # | SHA | Message |
|---|---|---|
| 1 | `1804c90` | `feat(model): add piecewise linear semantics` |
| 2 | (recorded after commit) | `feat(compiler): add zero-binary PWL one-sided rows` |

---

## Task 1 — PWL semantics: payload, validation, curvature classification, direct evaluator (tracer)

**Phase:** P33  **Requirements:** SM-14.1, SM-14.2, SM-12.8 (builder handles); foundation for SM-13.4
**Status:** complete — committed as `feat(model): add piecewise linear semantics`.

### TDD — RED failures (recorded before implementation)

`cargo test -p roml --test piecewise_linear` failed to compile against the untouched tree — the PWL payload types, the `ConstructKind::PiecewiseLinear` variant, and `Model::add_piecewise_linear` did not exist. Expected failure, recorded verbatim:

```text
error[E0432]: unresolved imports `roml::construct::ExtrapolationPolicy`, `roml::construct::PiecewiseLinearConstraint`, `roml::construct::PwlCurvature`, `roml::construct::PwlPoint`, `roml::construct::PwlRelation`
error[E0599]: no variant, associated function, or constant named `PiecewiseLinear` found for enum `ConstructKind`
error[E0599]: no method named `add_piecewise_linear` found for struct `roml::Model`
```

The `ConstructKind::PiecewiseLinear` session dispatch was added as a typed `UnsupportedFeature` placeholder in Task 1 (the bridge lands in Tasks 2/3); `validate_construct_finiteness` gained the PWL point-value finiteness arm in the same pass.

### Implementation

- **`src/construct/piecewise_linear.rs` (create)** — the PWL payload:
  - `PwlPoint { x: f64, value: ValueExpr }` with `From<(f64, f64)>` (constant-valued convenience).
  - `PwlRelation { Epigraph, Hypograph, ExactGraph }` (design §17 relation identifiers; `#[non_exhaustive]`).
  - `ExtrapolationPolicy { Constant, Linear }` (explicit, SM-14.1).
  - `PwlCurvature { Affine, Convex, Concave, NonConvex }` (`#[non_exhaustive]`).
  - `PiecewiseLinearConstraint { points, relation, extrapolation, argument: LinExpr, output: VarId }`.
  - `parameter_dependencies()` over point values and the argument (F1).
  - `classify_curvature()` deterministic from segment slopes (SM-14.2): affine when all slopes equal; convex when non-decreasing; concave when non-increasing; non-convex on a slope sign change. `segment_slopes()` is public for reporting/testing.
  - `evaluate(x: f64) -> f64` direct interpolation/extrapolation (SM-14.2/14.7): constant policy clamps outside the breakpoint range; linear policy continues the end segment slope.
- **`src/construct/mod.rs`** — `ConstructKind::PiecewiseLinear(PiecewiseLinearConstraint)` variant; `derive_variable_dependencies` (output + argument variables) and `derive_parameter_dependencies` (payload) extended.
- **`src/model/mod.rs`** — `add_piecewise_linear(argument, points, relation, extrapolation, preference) -> Result<(Construct, VarId), ModelError>` (SM-12.8): validates at least two finite strictly increasing breakpoints, finite evaluated point values (F5: missing parameter is `ModelError::ParameterNotFound`), and the argument expression; creates the output variable (IN-03 atomicity: construct id reserved first); records `Change::ConstructAdded`; accepts an optional `FormulationPreference` (A29). New `ModelError` variants: `PwlTooFewPoints`, `PwlNonFiniteBreakpoint(f64)`, `PwlNonFinitePointValue`, `PwlDuplicateBreakpoint { value }`, `PwlOutOfOrderBreakpoint { value, previous }`.
- **`src/compiler/session.rs`** — `ConstructKind::PiecewiseLinear` dispatch arm (typed `UnsupportedFeature` placeholder until Task 2) and the `validate_construct_finiteness` PWL arm (evaluated point values must be finite).
- **`src/lib.rs` / `src/advanced.rs`** — A30 re-exports of `PiecewiseLinearConstraint`, `PwlRelation`, `ExtrapolationPolicy`, `PwlCurvature`, `PwlPoint`.
- **`tests/piecewise_linear.rs` (create)** — 15 integration tests: the four validation rejections, deterministic curvature classification for all four classes, direct interpolation/extrapolation, the builder's stable handle + output variable, and parameter-dependency derivation.

### Focused verification

| Command | Result |
|---|---|
| `cargo test -p roml --test piecewise_linear` | 0 — **15 passed; 0 failed** |
| `cargo test -p roml --all-targets` | 0 — **902 passed; 0 failed; 0 ignored** (baseline 887 + 15) |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 — clean, warnings denied |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 — docs generated, no warnings |
| `cargo fmt --all -- --check` | 0 — formatting clean |

### Acceptance criteria

- All commands exit 0.
- `src/construct/piecewise_linear.rs` defines `PiecewiseLinearConstraint` with explicit `PwlRelation` and `ExtrapolationPolicy` fields, deterministic `classify_curvature` from segment slopes, and a direct `evaluate` implementing interpolation/extrapolation (SM-14.1, SM-14.2).
- `Model::add_piecewise_linear` returns a stable `Construct` handle plus the output-variable handle and rejects non-finite, duplicate, out-of-order, and underspecified points with typed errors (SM-14.1, SM-12.8).
- `ConstructKind::PiecewiseLinear` carries the payload; `derive_parameter_dependencies` covers point-value parameters (SM-01.3, F1).

---

## Task 2 — One-sided zero-binary PWL bridges (convex epigraph / concave hypograph) and reports

**Phase:** P33  **Requirements:** SM-14.3, SM-14.6, SM-13.5, SM-13.4 (exercised)
**Status:** complete — committed as `feat(compiler): add zero-binary PWL one-sided rows`.

### TDD — RED failures (recorded before implementation)

`cargo test -p roml --test piecewise_linear` failed to compile — `BackendFeature::PiecewiseLinear` and the PWL `GeneratedRole` variants did not exist, and the PWL session dispatch was the Task 1 placeholder (returns `UnsupportedFeature`). Expected failures, recorded verbatim:

```text
error[E0599]: no variant, associated function, or constant named `PiecewiseLinear` found for enum `BackendFeature`
error[E0599]: no variant, associated function, or constant named `PwlEpigraphRow` found for enum `GeneratedRole`
error[E0599]: no variant, associated function, or constant named `PwlHypographRow` found for enum `GeneratedRole`
```

The compile-success tests (`pwl_convex_epigraph_compiles_with_zero_binaries_and_supporting_rows`, etc.) additionally fail against the Task 1 placeholder dispatch, which returns `UnsupportedFeature` for any PWL construct.

### Implementation

- **`src/compiler/bridge/piecewise_linear.rs` (create)** — the PWL bridge:
  - `compile(payload, ctx, next_variable_index, next_row_index)` dispatches on `relation` (D24) after `select_path` gating on `BackendFeature::PiecewiseLinear`.
  - **Convex epigraph** (curvature Convex/Affine): zero-binary supporting-inequality rows `output >= v_i + s_i*(argument - x_i)` for every breakpoint `i` via `BridgeFinalizer::add_row` with `GeneratedRole::PwlEpigraphRow` (SM-14.3). The final breakpoint uses the last segment slope.
  - **Concave hypograph** (curvature Concave/Affine): the mirror zero-binary rows `output <= v_i + s_i*(argument - x_i)` with `GeneratedRole::PwlHypographRow` (SM-14.3).
  - **Relation/curvature mismatch** (`Epigraph` on non-convex, `Hypograph` on non-concave): typed `CompileError::UnsupportedFeature` — never a silent relaxation (D13). `ExactGraph` is a typed `UnsupportedFeature` until Task 3.
  - **Report (SM-14.6/SM-13.5):** `pwl.path` (exact bridge, no native claim), `pwl.curvature`, `pwl.relation`, `pwl.representation` (`"supporting inequalities (...)"`), `pwl.generated_binaries` (`"0"`), `pwl.binary_avoidance_reason`, `pwl.argument_interval` (from `BoundAnalyzer::interval_of_snapshot`, with bound sources), `pwl.breakpoint_range`.
  - Curvature is classified from the EVALUATED point values (parameter-dependent values resolve against the snapshot's parameter map) via the shared `classify_curvature_from_slopes` helper — never panics on parameter-dependent PWL.
- **`src/compiler/origin.rs`** — additive `GeneratedRole::{PwlEpigraphRow, PwlHypographRow, PwlExactGraphRow, PwlSegmentBinary, PwlWeightVariable}` (`#[non_exhaustive]` stays).
- **`src/compiler/capability.rs`** — additive `BackendFeature::PiecewiseLinear` variant (P32 additive-feature pattern).
- **`src/compiler/session.rs`** — `ConstructKind::PiecewiseLinear` dispatched through the P32 bridge framework.
- **`src/compiler/bridge/mod.rs`** — `pub(crate) mod piecewise_linear;`.
- **`roml-highs/src/session.rs`** — `BackendFeature::PiecewiseLinear` declared `SupportLevel::Bridge` (no native claim); `Sos2`/`NativePiecewiseLinear` stay `Unsupported` (P32 F4 rule, SM-04.3).
- **`src/construct/piecewise_linear.rs`** — shared `classify_curvature_from_slopes(slopes)` helper (constant and evaluated classification never diverge).
- **`tests/piecewise_linear.rs`** — 7 new tests: zero-binary + exact row-shape proofs for convex epigraph and concave hypograph, both mismatch rejections, the report/bound-evidence assertions, exact-graph-still-error, and origin completeness.

### Focused verification

| Command | Result |
|---|---|
| `cargo test -p roml --test piecewise_linear` | 0 — **22 passed; 0 failed** |
| `cargo test -p roml --all-targets` | 0 — **909 passed; 0 failed; 0 ignored** |
| `cargo test -p roml-highs --all-targets` | 0 — **131 passed; 0 failed; 0 ignored** |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 — clean, warnings denied |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 — clean, warnings denied |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 — docs generated, no warnings |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` | 0 — docs generated, no warnings |
| `cargo fmt --all -- --check` | 0 — formatting clean |

### Acceptance criteria

- All commands exit 0.
- Convex `Epigraph` and concave `Hypograph` PWL constructs compile to supporting-inequality linear rows with zero generated binaries (SM-14.3) — proven by a direct assertion on the compiled snapshot.
- The compilation report states curvature, relation, representation, generated counts, and why binaries were avoided (SM-14.6) and records the argument interval + bound sources as bound evidence (SM-13.5).
- `BackendFeature::PiecewiseLinear` is declared `SupportLevel::Bridge` on HiGHS with no native claim (SM-04.2/SM-04.3).

---

## Task 3 — Exact graph via deterministic exact segment binaries, randomized equivalence, HiGHS, and OR review

**Phase:** P33  **Requirements:** SM-14.4, SM-14.5, SM-14.6, SM-14.7, SM-13.1 (argument interval), SM-13.2 (no unproven Big-M)

<!-- gsd:write-continue -->
