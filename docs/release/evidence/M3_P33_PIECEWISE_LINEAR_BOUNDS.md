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
| 2 | `ba88f54` | `feat(compiler): add zero-binary PWL one-sided rows` |
| 3 | `4d01605` | `feat(model): add piecewise linear functions` |

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
**Status:** implementation complete — committed as `feat(model): add piecewise linear functions`. **Independent review pending** (P33 gate, `autonomous: false` — the orchestrator runs Pass 1/Pass 2 next).

### TDD — RED failures (recorded before implementation)

The exact-graph compile-success tests failed against the Task 2 bridge, which still returned the placeholder typed error. Expected failure, recorded verbatim:

```text
thread 'pwl_exact_graph_feasible_set_equals_graph_for_all_curvatures' panicked:
  snapshot must compile: UnsupportedFeature("exact PWL graph bridge lands in P33 Task 3")
```

The same `UnsupportedFeature` failure hit `pwl_nonconvex_exact_graph_excludes_convex_relaxation`,
`pwl_exact_graph_selects_segment_binaries_and_reports`, `pwl_exact_graph_randomized_fixed_input_agreement`,
and `pwl_exact_graph_entities_are_origin_complete`. The obsolete `pwl_exact_graph_is_typed_error_before_task3`
test was removed once the Task 3 bridge landed.

### Implementation

- **`src/compiler/bridge/piecewise_linear.rs`** — the exact-graph bridge (`PwlRelation::ExactGraph`):
  the deterministic exact segment-binary convex-combination formulation with points `(x_i, v_i)`:
  - weights `lambda_i >= 0` (`i in 0..m`, role `PwlWeightVariable`) with `sum lambda = 1`;
  - `argument = sum_i x_i * lambda_i` and `output = sum_i v_i * lambda_i` (equality rows);
  - adjacency binaries `z_k` (`k in 0..m-1`, role `PwlSegmentBinary`) with `sum z = 1`;
  - `lambda_0 <= z_0`, `lambda_i <= z_{i-1} + z_i` (interior), `lambda_m <= z_{m-1}`.
  - **No Big-M is introduced anywhere** (SM-13.2, D12). Entities are emitted in deterministic order
    (weights, then binaries, then rows) through `BridgeFinalizer` with `EntityOrigin::Construct`
    roles (SM-02.5).
  - Report decisions (SM-14.6): `pwl.representation` (`"exact segment binaries"`), `pwl.generated_binaries`
    (one per segment), `pwl.generated_auxiliary_variables` (one weight per point),
    `pwl.binary_introduction_reason` (exactness of the possibly-nonconvex graph; no convex relaxation),
    and `pwl.scaling` (numerical scaling diagnostic — value span over breakpoint span, ROADMAP P33).
    SOS2/native PWL are never selected because `native_payloads_available()` is false and HiGHS declares
    no native SOS2/PWL (P32 F4).
  - `NativeRequired` on a PWL construct rejects with `CompileError::UnsupportedFeature` via `select_path`.
- **`tests/piecewise_linear.rs`** — 6 new Task 3 tests:
  - `pwl_exact_graph_feasible_set_equals_graph_for_all_curvatures` — the compiled formulation's
    feasible set equals the graph (on-graph feasible, `y ± delta` infeasible) for all four curvature
    classes (SM-14.4).
  - `pwl_nonconvex_exact_graph_excludes_convex_relaxation` — the **phase-gate proof**: the convex-hull
    point `(x=1, y=0.5)` of the zigzag graph `[(0,0),(1,1),(2,0),(3,1)]` is infeasible in the compiled
    formulation, so no convex relaxation is emitted (SM-14.5).
  - `pwl_exact_graph_selects_segment_binaries_and_reports` — representation selection + report entries
    + scaling diagnostic (SM-14.4/SM-14.6).
  - `pwl_native_required_rejects_exact_graph` — `NativeRequired` → `UnsupportedFeature` (P32 F4).
  - `pwl_exact_graph_randomized_fixed_input_agreement` — fixed-seed LCG random arguments over the tested
    domain; direct `evaluate` agrees with the compiled formulation for all curvature classes (SM-14.7).
  - `pwl_exact_graph_entities_are_origin_complete` — role inventory + origin completeness (SM-02.5).
- **`roml-highs/tests/formulation_equivalence.rs`** — `pwl_highs_exact_graph_matches_reference_for_all_curvatures`:
  reference-vs-portable feasible-set equality on HiGHS for convex, concave, and nonconvex PWL exact graphs
  under both `Auto` (→ bridge) and `Portable` policies (SM-14.4/SM-14.7).
- **`.planning/.../TRACEABILITY.md`** — P33 evidence path updated to `M3_P33_PIECEWISE_LINEAR_BOUNDS.md`;
  SM-12 row marked closed (P32 primary, SM-12.8 advanced by P33); SM-13/SM-14 marked closed in P33
  (implementation; independent review pending per the P33 gate).

### Nonconvex-exactness proof (SM-14.5)

Zigzag graph `[(0,0), (1,1), (2,0), (3,1)]` (slopes `1, -1, 1`, curvature `NonConvex`). The convex hull of
the graph contains `(1, 0.5)` (on the chord `(0,0)-(2,0)`), but `f(1) = 1`. The compiled exact segment-binary
formulation makes `(x=1, y=0.5)` **infeasible** while `(x=1, y=1)` is feasible — the exact graph never
falls back to a convex relaxation.

### Randomized-equivalence corpus (SM-14.7)

For each curvature class (affine/convex/concave/nonconvex), 64 fixed-seed LCG arguments sampled uniformly
over the breakpoint domain. For every sample, the direct `evaluate` value is feasible in the compiled
formulation and `y ± 0.05` is infeasible. Total 256 fixed-input checks across the four classes, all passing.

### Representation report examples

Convex exact graph `[(0,0),(1,1),(2,4)]` (2 segments):

```text
pwl.path:                       exact bridge (no qualified native PWL/SOS2; F4)
pwl.curvature:                  Convex
pwl.relation:                   ExactGraph
pwl.argument_interval:          [0, 2]  (bound sources: [Constant, DeclaredVariableBounds(...)])
pwl.representation:             exact segment binaries
pwl.generated_binaries:         2 (one adjacency binary per segment)
pwl.generated_auxiliary_variables: 3 (one convex-combination weight per point)
pwl.binary_introduction_reason: exactness of the (possibly nonconvex) graph; no convex relaxation
pwl.scaling:                    value_span 4.000000 over x_span 2.000000 (avg |slope| 2.000000)
```

### Focused verification

| Command | Result |
|---|---|
| `cargo test -p roml --test piecewise_linear` | 0 — **27 passed; 0 failed** |
| `cargo test -p roml-highs --test formulation_equivalence pwl` | 0 — **1 passed; 0 failed** |
| `cargo test -p roml --all-targets` | 0 — **914 passed; 0 failed; 0 ignored** |
| `cargo test -p roml-highs --all-targets` | 0 — **132 passed; 0 failed; 0 ignored** |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 — clean, warnings denied |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 — clean, warnings denied |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 — docs generated, no warnings |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` | 0 — docs generated, no warnings |
| `cargo public-api -p roml` | 0 — 22178 items (raw; baseline 18792) |
| `cargo fmt --all -- --check` | 0 — formatting clean |

### Acceptance criteria

- All commands exit 0.
- Exact graphs compile through deterministic exact segment binaries for all curvature classes; nonconvex
  exact graphs never fall back to a convex relaxation (SM-14.4, SM-14.5).
- Randomized fixed-input PWL evaluations agree with compiled formulations over the tested domain for all
  curvature classes (SM-14.7).
- The compilation report states curvature, relation, representation, generated counts, and why binaries
  were introduced or avoided (SM-14.6) and records the argument interval + bound sources (SM-13.5).
- No Big-M is introduced without a finite proof; no default Big-M constant exists (SM-13.2, D12).
- `NativeRequired` on PWL rejects with `CompileError::UnsupportedFeature`; no native PWL/SOS2 claim is made
  (P32 F4 rule).
- Every generated PWL entity carries `EntityOrigin::Construct { construct, role }` (SM-02.5).

---

## Phase-level verification matrix (P33)

All commands run in the implementation worktree on `phase-roml-P33-piecewise-linear-bounds` at HEAD
`c7cacf5 + Task 1-3 commits`. All exit 0.

| Command | Exit | Result |
|---|---|---|
| `cargo fmt --all -- --check` | 0 | formatting clean |
| `cargo test -p roml --test piecewise_linear` | 0 | **27 passed; 0 failed** |
| `cargo test -p roml-highs --test formulation_equivalence pwl` | 0 | **1 passed; 0 failed** |
| `cargo test -p roml --all-targets` | 0 | **914 passed; 0 failed; 0 ignored** |
| `cargo test -p roml-highs --all-targets` | 0 | **132 passed; 0 failed; 0 ignored** |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 | docs generated, no warnings |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` | 0 | docs generated, no warnings |
| `cargo public-api -p roml` | 0 | 22178 items (raw) |

Per P30–P33 mandatory checks in `EXECUTION.md`: algebra/reference formulation tests (row-shape assertions in
Task 2, exact-graph feasibility-set equality in Task 3); native/portable equivalence (HiGHS
reference-vs-portable PWL sections); generated-origin completeness (SM-02.5 asserted for every generated
entity); no-silent-relaxation tests (relation/curvature mismatch rejections + nonconvex-exactness proof);
parameter dependency/update tests (Task 1 `parameter_dependencies` derivation). No skips.

## Public API diff

`cargo public-api -p roml` grew from **18792** items (P32 baseline capture) to **22178** items. The P33
additions (all additive, `#[non_exhaustive]` boundaries preserved):

- `roml::construct::{PiecewiseLinearConstraint, PwlRelation, ExtrapolationPolicy, PwlCurvature, PwlPoint}` (and `roml::` / `roml::advanced::` re-exports; `roml::construct::piecewise_linear::*`).
- `Model::add_piecewise_linear` (SM-12.8 stable-handle + output-variable builder).
- `ModelError::{PwlTooFewPoints, PwlNonFiniteBreakpoint(f64), PwlNonFinitePointValue, PwlDuplicateBreakpoint { value }, PwlOutOfOrderBreakpoint { value, previous }}`.
- `BackendFeature::PiecewiseLinear` (additive).
- `GeneratedRole::{PwlEpigraphRow, PwlHypographRow, PwlExactGraphRow, PwlSegmentBinary, PwlWeightVariable}` (additive).
- `PwlPoint::from<(f64, f64)>`, `PiecewiseLinearConstraint::{classify_curvature, segment_slopes, evaluate, parameter_dependencies}`.

Raw capture: `docs/release/evidence/M3_P33_public_api_roml.txt`.

## Deviations from Plan

**None.** The plan was executed exactly as written. The obsolete
`pwl_exact_graph_is_typed_error_before_task3` test (which asserted the Task 2 placeholder behavior) was
removed when the Task 3 exact-graph bridge landed — the placeholder was replaced by the real
implementation, and the remaining Task 3 tests cover exact-graph compilation. This is the natural
TDD RED→GREEN transition, not a plan deviation.

## Residual risks

- **Numerical scaling diagnostics are recorded but not yet hardened:** `pwl.scaling` reports the value
  span over the breakpoint span; the ROADMAP P33 diagnostics surface is minimal and will be exercised by
  the P34 numerical-quality audit.
- **SOS2 / native PWL remain unqualified** (Bridge-only honesty decision): a future backend that qualifies
  native SOS2/PWL would select it under `Auto`; M3 declares none, so the emitted representation is always
  the exact segment-binary formulation.
- **Parameter-dependent point values:** the direct `evaluate`/`classify_curvature` on the payload resolve
  constant point values only; the compiler bridge resolves parameter-dependent values against the snapshot.
  A user calling `evaluate` on a parameter-dependent payload panics with a clear message (documented
  limitation; the compile path is the correct evaluator for parameter-dependent PWL).
- **OR review pending:** Pass 1 (spec/correctness) and Pass 2 (integration/operations) have not yet run;
  this evidence bundle records the implementation state for those gates.

## Review gates (pending)

Per `EXECUTION.md` § "Review gates", P33 receives two independent review passes at the phase boundary
(after Task 3). `autonomous: false` — the executor pauses here and the orchestrator runs Pass 1
(specification and correctness) and Pass 2 (integration and operations). This evidence bundle is the input
to those gates. P0/P1 findings block merge; P2 findings may merge only when explicitly accepted and
scheduled.
