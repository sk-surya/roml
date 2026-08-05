---
phase: 33-piecewise-linear-bounds
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/construct/piecewise_linear.rs
  - src/construct/mod.rs
  - src/model/mod.rs
  - src/lib.rs
  - src/advanced.rs
  - src/compiler/bridge/piecewise_linear.rs
  - src/compiler/bridge/mod.rs
  - src/compiler/origin.rs
  - src/compiler/capability.rs
  - src/compiler/session.rs
  - src/compiler/report.rs
  - roml-highs/src/session.rs
  - tests/piecewise_linear.rs
  - roml-highs/tests/formulation_equivalence.rs
  - docs/release/evidence/M3_P33_PIECEWISE_LINEAR_BOUNDS.md
autonomous: false
requirements:
  - SM-12.1
  - SM-12.2
  - SM-12.5
  - SM-12.8
  - SM-13.1
  - SM-13.2
  - SM-13.3
  - SM-13.4
  - SM-13.5
  - SM-13.6
  - SM-14.1
  - SM-14.2
  - SM-14.3
  - SM-14.4
  - SM-14.5
  - SM-14.6
  - SM-14.7
must_haves:
  truths:
    - "Every PWL point is finite with strictly increasing breakpoints; non-finite, duplicate, out-of-order, and underspecified points are typed rejections (SM-14.1)"
    - "Curvature (affine/convex/concave/nonconvex) is classified deterministically from segment slopes (SM-14.2)"
    - "Convex epigraph and concave hypograph compile to supporting-inequality rows with zero generated binaries (SM-14.3)"
    - "Exact and nonconvex graphs compile to a deterministic exact representation (qualified native PWL, SOS2, or exact segment binaries) and never fall back to a convex relaxation (SM-14.4/SM-14.5)"
    - "No unproven Big-M: PWL bridges introduce no default Big-M constant; any M is derived from finite bounds or explicit validated user values (SM-13.2/SM-13.3, D12)"
    - "The compilation report states curvature, relation, selected representation, generated counts, and why binaries were introduced or avoided (SM-14.6, SM-13.5)"
    - "Randomized fixed-input PWL evaluations agree with compiled formulations over the tested domain for all curvature classes (SM-14.7)"
    - "The PWL API returns a stable Construct handle plus a result-variable handle and exposes formulation diagnostics (SM-12.8)"
  artifacts:
    - src/construct/piecewise_linear.rs (PiecewiseLinearConstraint, PwlRelation, ExtrapolationPolicy, PwlCurvature, PwlPoint)
    - src/compiler/bridge/piecewise_linear.rs (one-sided zero-binary + exact segment-binary bridges)
    - src/construct/mod.rs (ConstructKind::PiecewiseLinear variant + derive_parameter_dependencies)
    - src/model/mod.rs (add_piecewise_linear builder)
    - src/compiler/origin.rs (PWL GeneratedRole variants)
    - src/compiler/capability.rs (BackendFeature::PiecewiseLinear additive variant)
    - roml-highs/src/session.rs (PiecewiseLinear declared SupportLevel::Bridge; no native claim)
    - tests/piecewise_linear.rs
    - roml-highs/tests/formulation_equivalence.rs (pwl sections)
    - docs/release/evidence/M3_P33_PIECEWISE_LINEAR_BOUNDS.md
  key_links:
    - "Curvature classification drives one-sided (zero-binary) versus exact (segment-binary) compilation selection"
    - "BoundAnalyzer interval over the PWL argument expression provides finite bound evidence; no default Big-M is ever introduced (D12)"
    - "native_payloads_available() stays false while BackendConstraint is empty, so PWL is Bridge-only and NativeRequired rejects (P32 F4 rule)"
    - "BridgeFinalizer records EntityOrigin::Construct { construct, role } for every generated PWL variable/row (SM-02.5)"
---

# Phase 33 — Piecewise-linear Functions and Bound Analysis

> **For agentic workers:** this phase is packet Task 18 translated faithfully — PWL semantics, validation, curvature classification, a direct evaluator, one-sided zero-binary bridges, and the exact-graph representation decision. Execute Task 1 first and alone (the semantic tracer: payload, validation, classification, evaluator), then Task 2 (one-sided zero-binary compilation), then Task 3 (exact graph + randomized equivalence + HiGHS + evidence + OR review), strictly serially. Follow the TDD protocol from `EXECUTION.md` for every task: write a focused failing test, record the expected failure, implement the smallest correct behavior, run focused then phase tests, commit one coherent unit, update evidence and traceability. Do NOT run `roml-mosek`/`roml-xpress` — they are known-broken against the current facade and out of scope (M2 convention); never use workspace-wide commands. Do not touch git branches (`phase-roml-P33-piecewise-linear-bounds` exists; work on it).

**Goal:** provide safe convexity-aware PWL modeling and Big-M evidence over the frozen compiler contract — exact piecewise-linear semantics and formulations, with convex epigraph / concave hypograph compiled with zero binaries and exact/nonconvex graphs compiled exactly (never a convex relaxation), all reported with curvature, representation, generated counts, and binary-avoidance reasons.

**Requirements:** SM-13 (full closure), SM-14 (all clauses), and SM-12.1/SM-12.2/SM-12.5/SM-12.8 (P32 closures recorded in the P33 TRACEABILITY update; SM-12.8 re-exercised by the PWL builder's stable-handle + formulation-diagnostics surface).

## Wave-0 — Baseline verification (precondition)

**Base re-pointed.** The p33 branch base was originally the broken `f905b0d` (which had reverted the accepted P27/P32 implementation source despite the P32 merge `538336d` being an ancestor). The main-tree recovery commit `40af9f4` restored all P27/P32 files onto main, and the p33 worktree was re-pointed (`git reset --hard origin/main`). The branch base is now `main@40af9f4`, which contains the full accepted P32 implementation: `src/compiler/bounds.rs` (`BoundAnalyzer`, `Interval`, `BoundTrace`, `BoundSource`, one-sided Big-M helpers), `src/compiler/bridge/mod.rs` (`BridgeFinalizer`, `BridgeContext`, `select_path`, `native_payloads_available` gate) and the bridge/construct payload modules, `src/assignment.rs`, `src/solver/overlay.rs`, `src/solver/facade.rs`, and the P27/P32 test files. No restoration is required; Wave-0 is now verification only. The original gap analysis is preserved in the git history of this plan file (first committed version) for reference.

**Wave-0 steps (executor, before Task 1):**

1. Record the exact base SHA and branch in the evidence file: `git rev-parse HEAD` on `phase-roml-P33-piecewise-linear-bounds` (expect `40af9f4`).
2. Verify P32 presence: `test -f src/compiler/bounds.rs && test -f src/compiler/bridge/mod.rs && test -f src/construct/minmax.rs` — all must succeed on the re-pointed base. If any file is missing, **stop and escalate** — do not start Task 1 against a P32-less tree; the PWL bridge cannot be written without `BridgeFinalizer`/`BoundAnalyzer`/`select_path`.
3. Re-verify the P32 baseline so the P33 gate is measured on a P32-complete tree:
   - `cargo test -p roml --all-targets`
   - `cargo test -p roml-highs --all-targets`
   - `cargo clippy -p roml --all-targets -- -D warnings`
   - `cargo clippy -p roml-highs --all-targets -- -D warnings`
   - `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps`
4. Create `docs/release/evidence/M3_P33_PIECEWISE_LINEAR_BOUNDS.md` with scope/requirements, baseline and environment, the re-pointing note (base restored from `f905b0d` to `main@40af9f4`), and the commit trail header before implementation.

**Native-vs-bridge honesty decision (mandatory statement):** This phase implements **no new native payloads** in `BackendConstraint` and **no HiGHS native PWL/SOS2 projection**. `BackendConstraint` stays empty; `native_payloads_available()` stays false; PWL is selected and reported as `SupportLevel::Bridge` with the exact graph compiled through the **deterministic exact segment-binary formulation** (ROML portable bridge). `BackendFeature::NativePiecewiseLinear` and `BackendFeature::Sos2` remain `SupportLevel::Unsupported` on HiGHS (no false native claim — P32 F4 rule). `NativeRequired` on a PWL construct rejects with `CompileError::UnsupportedFeature`. SOS2 is a declared possible representation only when a backend qualifies native `Sos2` (none does in M3); the actual emitted representation is exact segment binaries.

## Requirements

- **SM-12.1 / SM-12.2 / SM-12.5** — closed by P32 Task 16; the P33 TRACEABILITY update records the SM-12 row as closed (primary phase P32).
- **SM-12.8** — construct APIs return stable handles/results and expose formulation diagnostics. Advanced by Task 1 (the PWL builder returns `(Construct, VarId)`) and Task 2/3 (the compilation report exposes PWL formulation decisions: curvature, relation, representation, generated counts, binary-avoidance reason).
- **SM-13.1** — deterministic interval analysis computes bounds for linear scalar functions. Extended to the PWL argument expression via `BoundAnalyzer::interval_of_snapshot` in Task 2/3 (the report records the argument interval and bound sources).
- **SM-13.2** — a Big-M bridge requires a finite derived value or explicit user value. PWL bridges introduce no Big-M (exact segment binaries and supporting inequalities need none); any M in a selector-like row is derived from finite bounds only (Task 2/3).
- **SM-13.3** — explicit M values are validated against known bounds where possible. Not applicable to the zero-Big-M PWL bridges; recorded as N/A in evidence.
- **SM-13.4** — compilation errors identify the construct and missing/unbounded expression. Task 1/2/3 surface `CompileError::UnboundedBigM { construct, expression }` / `UnsupportedFeature` naming the PWL construct where an exact representation is unavailable.
- **SM-13.5** — compilation reports record M values, derivations, and bound sources. Task 2/3 record `pwl.*` bound-evidence report entries: argument interval, breakpoint/value ranges, bound sources, representation, counts.
- **SM-13.6** — M3 does not silently run auxiliary optimization problems for bound tightening. PWL bound analysis derives from declared bounds/breakpoints only; no auxiliary LP (Task 2/3).
- **SM-14.1** — PWL points are finite, strictly ordered, and have an explicit extrapolation policy. Task 1.
- **SM-14.2** — convexity/concavity classification is deterministic from segment slopes. Task 1.
- **SM-14.3** — convex epigraph and concave hypograph formulations introduce no binaries. Task 2.
- **SM-14.4** — exact graph formulations use qualified native PWL, SOS2, or exact binary representation. Task 3 (exact segment binaries).
- **SM-14.5** — nonconvex exact graphs never fall back to a convex relaxation. Task 3 (proof test).
- **SM-14.6** — representation choice and introduced binaries/auxiliaries appear in the compilation report. Task 2/3.
- **SM-14.7** — randomized PWL evaluations agree with compiled formulations over the tested domain. Task 3.

## Files

Create:

- `docs/release/evidence/M3_P33_PIECEWISE_LINEAR_BOUNDS.md` — phase evidence file (created in Wave-0 with baseline + environment; appended as work proceeds).
- `src/construct/piecewise_linear.rs` — `PiecewiseLinearConstraint`, `PwlRelation`, `ExtrapolationPolicy`, `PwlCurvature`, `PwlPoint` (Task 1).
- `src/compiler/bridge/piecewise_linear.rs` — the PWL bridges: one-sided zero-binary (Task 2) and exact segment-binary (Task 3).
- `tests/piecewise_linear.rs` — PWL validation, classification, evaluator, zero-binary, exactness, randomized-equivalence, and report tests (Tasks 1–3).

Modify:

- `src/construct/mod.rs` — add `ConstructKind::PiecewiseLinear(PiecewiseLinearConstraint)`; extend `derive_parameter_dependencies` over point values.
- `src/model/mod.rs` — public builder `add_piecewise_linear` returning `(Construct, VarId)` (SM-12.8) with typed validation rejections.
- `src/lib.rs` / `src/advanced.rs` — re-export the PWL payload types and helper enums (A30 pattern).
- `src/compiler/bridge/mod.rs` — declare `pub(crate) mod piecewise_linear;`.
- `src/compiler/origin.rs` — `GeneratedRole` PWL role variants.
- `src/compiler/capability.rs` — additive `BackendFeature::PiecewiseLinear` variant (mirroring the P32 additive-feature pattern).
- `src/compiler/session.rs` — dispatch `ConstructKind::PiecewiseLinear` through the bridge framework in deterministic construct-id order.
- `src/compiler/report.rs` — PWL formulation-decision entries (curvature, relation, representation, counts, binary-avoidance reason, scaling diagnostic) if the existing `FormulationDecision` surface needs extension.
- `roml-highs/src/session.rs` — declare `BackendFeature::PiecewiseLinear` as `SupportLevel::Bridge` (no native claim); `Sos2`/`NativePiecewiseLinear` stay unsupported.
- `roml-highs/tests/formulation_equivalence.rs` — append `pwl` reference-vs-portable equivalence sections.

## Task 1 — PWL semantics: payload, validation, curvature classification, direct evaluator (tracer)

**Phase:** P33  **Requirements:** SM-14.1, SM-14.2, SM-12.8 (builder handles); foundation for SM-13.4

**Read first:**
- `docs/superpowers/specs/2026-08-02-semantic-modeling-and-solve-workflows-design.md` §17 "Piecewise-linear functions", §7 "Canonical semantic constructs", §19 "Failure semantics".
- `docs/superpowers/plans/2026-08-02-semantic-modeling-and-solve-workflows.md` — Task 18 (verbatim translation target).
- `src/construct/minmax.rs` (P32) — the payload pattern to mirror (`output` variable created by the builder, top-level construct origin, `parameter_dependencies()`).
- `src/construct/mod.rs` — `ConstructKind`/`ConstructEntry` (A30 public surface) and `derive_parameter_dependencies`.
- `src/model/mod.rs` — the `add_minmax`/`add_absolute_value` builder pattern (validation + `Change::ConstructAdded` + stable handle).
- `src/lib.rs` / `src/advanced.rs` — the A30-pattern re-export points.
- `src/value_expr/` — `ValueExpr` for parameter-dependent point values.
- `tests/piecewise_linear.rs` — new test file (create empty in Wave-0 if needed).

**TDD order** (per `EXECUTION.md`):

1. Wave-0 baseline + evidence (see above).
2. Write failing tests in `tests/piecewise_linear.rs`:
   - **Validation (SM-14.1):** non-finite breakpoint or value; duplicate breakpoint; out-of-order (decreasing) breakpoint; fewer than two points — each a typed `ModelError` rejection.
   - **Curvature (SM-14.2):** affine (equal slopes), convex (non-decreasing slopes), concave (non-increasing slopes), nonconvex (slope sign change) — deterministic from segment slopes.
   - **Direct evaluator:** linear interpolation between breakpoints and extrapolation per the explicit `ExtrapolationPolicy` (constant vs linear) for arguments inside and outside the breakpoint range.
   - **Builder (SM-12.8):** `add_piecewise_linear` returns a stable `Construct` handle plus the output-variable handle; the output variable is created by the builder and stored in the payload.
3. Run the tests and record the expected failures (missing `roml::construct::PiecewiseLinearConstraint`, missing `Model::add_piecewise_linear`).
4. Implement:
   - `src/construct/piecewise_linear.rs`: `PwlPoint { x: f64, value: ValueExpr }`; `PiecewiseLinearConstraint { points: Vec<PwlPoint>, relation: PwlRelation, extrapolation: ExtrapolationPolicy, argument: LinExpr, output: VarId }`; `PwlRelation { Epigraph, Hypograph, ExactGraph }` (design §17 relation identifiers); `ExtrapolationPolicy` (explicit, SM-14.1); `PwlCurvature { Affine, Convex, Concave, NonConvex }`; `parameter_dependencies()` over point values (F1); `classify_curvature()` deterministic from segment slopes; `evaluate(x: f64) -> f64` direct interpolation/extrapolation.
   - `src/construct/mod.rs`: `ConstructKind::PiecewiseLinear(PiecewiseLinearConstraint)`; extend `derive_parameter_dependencies`.
   - `src/model/mod.rs`: `add_piecewise_linear(argument, points, relation, extrapolation, preference) -> Result<(Construct, VarId), ModelError>` — validate finite points, strictly increasing breakpoints, at least two points, finite/validated values; create the output variable; record `Change::ConstructAdded`; accept an optional per-construct `FormulationPreference` (A29 single authority).
   - `src/lib.rs` / `src/advanced.rs`: re-export `PiecewiseLinearConstraint`, `PwlRelation`, `ExtrapolationPolicy`, `PwlCurvature`, `PwlPoint`.
5. Run `cargo test -p roml --test piecewise_linear` (must pass), then `cargo test -p roml --all-targets`, `cargo clippy -p roml --all-targets -- -D warnings`, `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps`.
6. Update evidence.
7. Commit one coherent unit.

- [ ] Reject non-finite, duplicate, out-of-order, and underspecified points (SM-14.1).
- [ ] Classify affine/convex/concave/nonconvex from segment slopes (SM-14.2).
- [ ] Implement direct interpolation/extrapolation evaluator.
- [ ] Return a stable `Construct` handle plus the output-variable handle from the builder (SM-12.8).
- [ ] Stop when all four bullets hold and `piecewise_linear` is green.
- [ ] Commit as `feat(model): add piecewise linear semantics`.

**Stopping condition:** PWL point validation rejects the four invalid-input classes with typed `ModelError`s; curvature classification is deterministic from segment slopes for affine/convex/concave/nonconvex; the direct evaluator agrees with hand-computed interpolation and extrapolation cases; `cargo test -p roml --test piecewise_linear` exits 0.

**Commit:** `feat(model): add piecewise linear semantics`

**Verification:**

```bash
cargo test -p roml --test piecewise_linear
cargo test -p roml --all-targets
cargo clippy -p roml --all-targets -- -D warnings
```

**Acceptance criteria:**
- All three commands exit 0.
- `src/construct/piecewise_linear.rs` defines `PiecewiseLinearConstraint` with explicit `PwlRelation` and `ExtrapolationPolicy` fields, deterministic `classify_curvature` from segment slopes, and a direct `evaluate` implementing interpolation/extrapolation (SM-14.1, SM-14.2).
- `Model::add_piecewise_linear` returns a stable `Construct` handle plus the output-variable handle and rejects non-finite, duplicate, out-of-order, and underspecified points with typed errors (SM-14.1, SM-12.8).
- `ConstructKind::PiecewiseLinear` carries the payload; `derive_parameter_dependencies` covers point-value parameters (SM-01.3, F1).

## Task 2 — One-sided zero-binary PWL bridges (convex epigraph / concave hypograph) and reports

**Phase:** P33  **Requirements:** SM-14.3, SM-14.6, SM-13.5, SM-13.4 (exercised)

**Read first:**
- Design §17 "Compilation: convex epigraph: supporting linear inequalities, zero binaries; concave hypograph: supporting linear inequalities, zero binaries".
- `src/compiler/bridge/minmax.rs` (P32) — the one-sided zero-binary row pattern (`MinMaxEpigraphRow`/`MinMaxHypographRow`).
- `src/compiler/bridge/mod.rs` — `BridgeFinalizer` (`new`, `add_variable`, `add_row`, `add_dependency`, `record_bound_evidence`, `add_decision`, `finish`), `BridgeContext`, `select_path`, `native_payloads_available()` gate.
- `src/compiler/bounds.rs` — `BoundAnalyzer::interval_of_snapshot` for the argument interval (SM-13.1/SM-13.5 evidence).
- `src/compiler/origin.rs` — `GeneratedRole` (add PWL role variants).
- `src/compiler/capability.rs` — `BackendFeature` (additive `PiecewiseLinear`) and `FeatureSupport::bridge`.
- `src/compiler/session.rs` — `compile_snapshot` construct dispatch and `preflight_constructs`.
- `src/compiler/report.rs` — `FormulationDecision` (bound-evidence and representation entries).
- `roml-highs/src/session.rs` — `highs_capability_set` bridge declarations.

**TDD order:**

1. Write failing tests in `tests/piecewise_linear.rs`:
   - **Zero binaries (SM-14.3):** for a convex PWL with `relation = Epigraph`, the compiled snapshot's `Construct`-origin generated variables contain **no binary** variables; for `relation = Hypograph` on a concave PWL, the same zero-binary assertion. Assert the exact row shapes: `output >= v_i + s_i * (argument - x_i)` (epigraph) and `output <= v_i + s_i * (argument - x_i)` (hypograph) for every breakpoint i, where s_i is the segment slope.
   - **Relation-curvature validity:** `Epigraph` on a non-convex PWL and `Hypograph` on a non-concave PWL are typed errors — never a silent relaxation (D13).
   - **Report (SM-14.6/SM-13.5):** the compilation report records curvature, relation, representation (`"supporting inequalities"`), generated counts (zero binaries), and the binary-avoidance reason; the argument interval and bound sources are recorded as bound evidence (SM-13.5).
   - **Origins (SM-02.5):** every generated row carries `EntityOrigin::Construct { construct, role }`.
2. Run and record the expected failures (`ConstructKind::PiecewiseLinear` dispatch missing / returns `UnsupportedFeature`).
3. Implement:
   - `src/compiler/bridge/piecewise_linear.rs`: `compile(payload, ctx, next_variable_index, next_row_index)` dispatching on `relation`. For `Epigraph` on affine/convex curvature, emit `output >= v_i + s_i * (argument - x_i)` rows for all i via `BridgeFinalizer::add_row` with zero generated binaries. For `Hypograph` on affine/concave curvature, emit the mirror rows. Mismatched relation/curvature → typed `CompileError` (no relaxation). Record bound-evidence report entries (argument interval, slope/breakpoint derivation, bound sources) and a representation decision (`"supporting inequalities"`, `generated_binaries = 0`, `binary_avoidance_reason`).
   - `src/compiler/origin.rs`: `GeneratedRole::{PwlEpigraphRow, PwlHypographRow, PwlExactGraphRow, PwlSegmentBinary, PwlWeightVariable}` (additive, `#[non_exhaustive]` stays).
   - `src/compiler/capability.rs`: additive `BackendFeature::PiecewiseLinear` variant.
   - `src/compiler/session.rs`: dispatch `ConstructKind::PiecewiseLinear` through the P32 bridge framework.
   - `src/compiler/bridge/mod.rs`: `pub(crate) mod piecewise_linear;`.
   - `roml-highs/src/session.rs`: declare `BackendFeature::PiecewiseLinear` as `SupportLevel::Bridge` (no native claim); `Sos2`/`NativePiecewiseLinear` stay unsupported (SM-04.3).
4. Run focused then full verification.
5. Update evidence.
6. Commit one coherent unit.

- [ ] Compile convex epigraph and concave hypograph with zero binary variables (SM-14.3).
- [ ] Prove zero generated binaries with a direct assertion on the compiled snapshot.
- [ ] Record curvature, relation, representation, generated counts, and binary-avoidance reason in the report (SM-14.6, SM-13.5).
- [ ] Reject relation/curvature mismatches without a silent relaxation (D13).
- [ ] Stop when the zero-binary rows, report entries, and origin completeness hold.
- [ ] Commit as `feat(compiler): add zero-binary PWL one-sided rows`.

**Stopping condition:** convex epigraph and concave hypograph compile to supporting-inequality rows with zero generated binaries; relation/curvature mismatches are typed errors; the report records curvature/relation/representation/counts/binary-avoidance-reason and bound evidence; every generated row carries a construct origin; `cargo test -p roml --test piecewise_linear` exits 0.

**Commit:** `feat(compiler): add zero-binary PWL one-sided rows`

**Verification:**

```bash
cargo test -p roml --test piecewise_linear
cargo test -p roml --all-targets
cargo clippy -p roml --all-targets -- -D warnings
```

**Acceptance criteria:**
- All commands exit 0.
- Convex `Epigraph` and concave `Hypograph` PWL constructs compile to supporting-inequality linear rows with zero generated binaries (SM-14.3).
- The compilation report states curvature, relation, representation, generated counts, and why binaries were avoided (SM-14.6) and records the argument interval + bound sources as bound evidence (SM-13.5).
- `BackendFeature::PiecewiseLinear` is declared `SupportLevel::Bridge` on HiGHS with no native claim (SM-04.2/SM-04.3).

## Task 3 — Exact graph via deterministic exact segment binaries, randomized equivalence, HiGHS, and OR review

**Phase:** P33  **Requirements:** SM-14.4, SM-14.5, SM-14.6, SM-14.7, SM-13.1 (argument interval), SM-13.2 (no unproven Big-M)

**Read first:**
- Design §17 "Compilation: exact graph: qualified native PWL, SOS2, or exact segment-binary formulation; nonconvex exact graph: never a convex relaxation."
- `src/compiler/bridge/minmax.rs` (P32) — the bounded selector / generated-binary pattern and the `BridgeFinalizer` binary/auxiliary allocation.
- `src/compiler/backend_ir.rs` — `BackendConstraint` (stays empty), `native_payloads_available()` (stays false), `BackendSnapshotBuilder`.
- `src/compiler/bridge/mod.rs` — `select_path` (Auto/Portable/NativeRequired) and the F4 native gate.
- `roml-highs/tests/formulation_equivalence.rs` — the P32 reference-vs-portable equivalence pattern to append `pwl` sections to.
- `tests/piecewise_linear.rs` — the randomized direct-evaluation pattern from P32 Task 17 (algebraic existential checks in core; solve-based equivalence on HiGHS).

**TDD order:**

1. Write failing tests:
   - **Exact graph (SM-14.4):** for `relation = ExactGraph` with affine/convex/concave/nonconvex PWL, the compiled formulation's feasible set equals the graph: for sampled `argument = x`, the output `y = f(x)` is feasible, and `y +/- delta` is infeasible for a small delta (algebraic existential check over the generated binaries — the P32 Task 17 pattern; the actual solve-based equivalence runs on HiGHS).
   - **Nonconvex exactness (SM-14.5):** for a nonconvex PWL, the exact-graph feasible set is **not** the convex relaxation — a point admitted by the convex hull of the graph but not on the graph is infeasible in the compiled formulation (no convex relaxation, no silent relaxation). This is the phase-gate proof test.
   - **Representation selection (SM-14.4/SM-14.6):** under `Auto`/`Portable` the exact graph selects the deterministic exact segment-binary representation; the report records representation, generated binary/auxiliary counts, and the reason binaries were introduced. Under `NativeRequired`, the PWL construct rejects with `CompileError::UnsupportedFeature` (no native payload exists — P32 F4 rule).
   - **Randomized equivalence (SM-14.7):** fixed-seed random arguments over the tested domain; direct `evaluate` agrees with the compiled formulation for all four curvature classes.
   - **HiGHS equivalence:** `roml-highs/tests/formulation_equivalence.rs` `pwl` sections — reference-vs-portable feasible-set equality on HiGHS for convex, concave, and nonconvex PWL.
2. Run and record the expected failures.
3. Implement:
   - Extend `src/compiler/bridge/piecewise_linear.rs` with the exact-graph bridge: the deterministic exact segment-binary convex-combination formulation — weights `lambda_i >= 0` over the points with `sum lambda = 1`, `argument = sum lambda_i * x_i`, `output = sum lambda_i * v_i`, and adjacency binaries `z_k` (one per segment, `sum z = 1`) with `lambda_0 <= z_0`, `lambda_i <= z_{i-1} + z_i` for interior points, `lambda_n <= z_{n-1}`. Emit through `BridgeFinalizer` with `PwlExactGraphRow`/`PwlSegmentBinary`/`PwlWeightVariable` roles in deterministic order. No Big-M is introduced anywhere (SM-13.2); the report records the argument interval from `BoundAnalyzer` and the breakpoint/value ranges as bound evidence.
   - Representation decision: `"exact segment binaries"` under `Auto`/`Portable`; SOS2/native PWL never selected because `native_payloads_available()` is false and HiGHS declares no native SOS2/PWL (report explains the binary-introduction reason — exactness of the nonconvex graph).
   - `NativeRequired` → `CompileError::UnsupportedFeature` (no false native claim; P32 F4 rule).
   - `src/compiler/report.rs`: PWL formulation-decision entries (curvature, relation, representation, generated counts, binary-avoidance/introduction reason, scaling diagnostic per ROADMAP P33 "numerical scaling diagnostics").
4. Run the full verification matrix (focused + `--all-targets` for both crates + clippy + rustdoc + `cargo public-api -p roml`).
5. Append the evidence file: per-curvature tables, the nonconvex-exactness proof, randomized-equivalence corpus, representation report examples, scaling diagnostics, public API diff, and residual risks. Update TRACEABILITY.md P33 evidence path to `M3_P33_PIECEWISE_LINEAR_BOUNDS.md` and mark SM-12/SM-13/SM-14 closures.
6. Request OR review at the P33 boundary (Pass 1 + Pass 2 per `EXECUTION.md`); resolve all P0/P1 findings.
7. Commit one coherent unit (the packet Task 18 final commit).

- [ ] Compile the exact graph through deterministic exact segment binaries for all curvature classes (SM-14.4).
- [ ] Prove nonconvex exact graphs never fall back to a convex relaxation (SM-14.5).
- [ ] Verify random fixed-input output for all curvature classes (SM-14.7).
- [ ] Report curvature, relation, representation, generated counts, and binary-avoidance/introduction reason (SM-14.6).
- [ ] Confirm HiGHS reference-vs-portable equivalence for PWL.
- [ ] Stop when the phase gate holds and both `--all-targets` suites are green.
- [ ] Commit as `feat(model): add piecewise linear functions` and request OR review.

**Stopping condition:** the exact-graph formulation is exact for affine/convex/concave/nonconvex PWL (randomized fixed-input agreement), the nonconvex graph provably excludes the convex relaxation, the report explains every representation, HiGHS reference-vs-portable equivalence passes, and the phase gate holds. `feat(model): add piecewise linear functions` committed and OR review requested.

**Commit:** `feat(model): add piecewise linear functions`

**Verification:**

```bash
cargo test -p roml --test piecewise_linear
cargo test -p roml-highs --test formulation_equivalence pwl
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
cargo clippy -p roml --all-targets -- -D warnings
cargo clippy -p roml-highs --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
cargo public-api -p roml
```

**Acceptance criteria:**
- All commands exit 0.
- Exact graphs compile through deterministic exact segment binaries for all curvature classes; nonconvex exact graphs never fall back to a convex relaxation (SM-14.4, SM-14.5).
- Randomized fixed-input PWL evaluations agree with compiled formulations over the tested domain for all curvature classes (SM-14.7).
- The compilation report states curvature, relation, representation, generated counts, and why binaries were introduced or avoided (SM-14.6) and records the argument interval + bound sources (SM-13.5).
- No Big-M is introduced without a finite proof; no default Big-M constant exists (SM-13.2, D12).
- `NativeRequired` on PWL rejects with `CompileError::UnsupportedFeature`; no native PWL/SOS2 claim is made (P32 F4 rule).
- Every generated PWL entity carries `EntityOrigin::Construct { construct, role }` (SM-02.5).

## Verification

Phase-level checks (all must exit 0; per-crate only — never workspace-wide):

```bash
cargo fmt --all -- --check
cargo test -p roml --test piecewise_linear
cargo test -p roml-highs --test formulation_equivalence pwl
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
cargo clippy -p roml --all-targets -- -D warnings
cargo clippy -p roml-highs --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps
cargo public-api -p roml
```

Baseline matrix (untouched tree on the re-pointed base `main@40af9f4`, recorded in the evidence file; `roml-mosek`/`roml-xpress` out of scope):

```bash
cargo fmt --all -- --check
cargo check -p roml --all-targets
cargo check -p roml-highs --all-targets
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
cargo package --list -p roml
cargo package --list -p roml-highs
```

Per P30–P33 mandatory checks in `EXECUTION.md`: algebra/reference formulation tests; native/portable equivalence; generated-origin completeness; no-silent-relaxation tests; parameter dependency/update tests. Record any skip in the evidence file, never treat it as passing.

## Waves and parallelization

This plan has three tasks and **one execution wave — strictly serial within the plan**:

- **Task 1** (semantics tracer) first and alone: it proves the PWL vertical slice end to end (payload → builder → validation → curvature classification → direct evaluator), establishing the semantic definitions Tasks 2 and 3 compile.
- **Task 2** (one-sided zero-binary bridges) strictly after Task 1.
- **Task 3** (exact graph + randomized equivalence + HiGHS + OR review) strictly after Task 2.

**Can the three tasks run in parallel? No — three independent reasons:**

1. **File sharing.** All three modify `src/construct/mod.rs`, `src/model/mod.rs`, `src/lib.rs`, `src/advanced.rs`, `tests/piecewise_linear.rs`, `src/compiler/report.rs`, and the evidence file; Tasks 2 and 3 additionally share `src/compiler/session.rs`, `src/compiler/origin.rs`, `src/compiler/capability.rs`, `src/compiler/bridge/mod.rs`, `roml-highs/src/session.rs`, and `roml-highs/tests/formulation_equivalence.rs` — parallel execution would produce overlapping edits on the same files.
2. **Pattern dependency.** Task 2's bridges are written against Task 1's payload/curvature/evaluator (they compile the payload Task 1 defines and classify what Task 1 classified); Task 3's exact-graph bridge extends Task 2's bridge module and reuses its report machinery.
3. **Evidence and gate sequencing.** The evidence file appends per-task facts in order; the curvature/relation foundations (Task 1) and zero-binary proof (Task 2) are prerequisites the exact-graph and randomized-equivalence tests (Task 3) reference.

The packet's serial Task 18 order and D26 (one active implementation phase) bind the phase to a single branch. **No cross-task parallelism is exposed.**

## Source coverage audit

| Source | Item | Coverage |
|---|---|---|
| GOAL (ROADMAP P33) | safe convexity-aware PWL modeling and Big-M evidence | Tasks 1–3 |
| GOAL (ROADMAP P33) | interval analyzer and bound-source traces | Tasks 2/3 (`BoundAnalyzer::interval_of_snapshot` + `pwl.*` bound evidence) |
| GOAL (ROADMAP P33) | PWL validation, interpolation/extrapolation, curvature classification | Task 1 |
| GOAL (ROADMAP P33) | convex epigraph/concave hypograph rows with zero binaries | Task 2 |
| GOAL (ROADMAP P33) | exact native/SOS2/segment-binary graph bridges | Task 3 (deterministic exact segment binaries; native/SOS2 unqualified — honesty decision) |
| GOAL (ROADMAP P33) | per-construct formulation override | Tasks 1–3 (A29 `FormulationPreference` on the PWL `ConstructEntry`; `select_path` honors it) |
| GOAL (ROADMAP P33) | randomized direct-evaluation/solver equivalence | Task 3 |
| GOAL (ROADMAP P33) | numerical scaling diagnostics | Task 3 (report scaling diagnostic per curvature/breakpoint spread) |
| GOAL (ROADMAP P33) | public examples | **P34 Task 19** (`examples/m3_pwl.rs`) — excluded, no silent omission |
| REQ | SM-14.1, SM-14.2 | Task 1 |
| REQ | SM-14.3 | Task 2 |
| REQ | SM-14.4, SM-14.5, SM-14.6, SM-14.7 | Task 3 |
| REQ | SM-13.1, SM-13.2, SM-13.3, SM-13.4, SM-13.5, SM-13.6 (full closure) | Tasks 1–3 (argument interval, no-unproven-Big-M, construct-identifying errors, bound-source report entries, no auxiliary LP) |
| REQ (closed by P32, recorded here) | SM-12.1, SM-12.2, SM-12.5 | P33 TRACEABILITY update records the SM-12 row closed |
| REQ | SM-12.8 | Task 1 (stable handles), Tasks 2/3 (formulation diagnostics) |
| RESEARCH (design) | §17 PWL semantics and compilation; §8.2 capability model; §8.5 bridge contract; §19 failure semantics | Tasks 1–3 |
| CONTEXT (decisions) | D12 Big-M requires proof; D13 exactness not from objective; D24 PWL relation determines formulation; A29 preference single authority; P32 F4 no-false-native-claims rule | Tasks 1–3 (zero Big-M; exact/one-sided distinct; relation-driven compilation; preference threading; Bridge-only selection) |
| NOT in scope | Native PWL/SOS2 payloads in `BackendConstraint`; HiGHS native PWL projection; public examples (`m3_pwl.rs`); NLP | excluded — honesty decision + P34 scope |

## Review gates

Per `EXECUTION.md` § "Review gates", P33 receives two independent review passes at the phase boundary (after Task 3).

- **Pass 1 — Specification and correctness:** requirement coverage (SM-13 clause-level full closure, SM-14.1–14.7, SM-12.8 advanced); semantic correctness of the supporting-inequality zero-binary rows, the relation/curvature matching, the exact segment-binary formulation, and the nonconvex-exactness proof; invariant preservation (D12 no default Big-M, D13 no relaxation from objective context, D24 relation-determines-formulation); origin completeness for every generated PWL entity; unsupported/error behavior (`UnsupportedFeature` under `NativeRequired`, relation/curvature mismatch errors); API coherence (A29 preference, SM-12.8 stable handles); test quality (validation matrix, randomized fixed-input equivalence, nonconvex proof).
- **Pass 2 — Integration and operations:** incremental/rebuild behavior (PWL construct compilation through the frozen P26 compiler contract + P32 bridge framework; no compiler-contract change without an amendment); failure recovery; cross-platform/version behavior (HiGHS equivalence sections); public API diff (`cargo public-api -p roml`); package/docs impact (evidence file); migration accuracy.

**Blocking rules:**

- P0/P1 findings **block merge**.
- P2 findings may merge only when explicitly accepted and scheduled.
- `autonomous: false` — the executor pauses after Task 3, requests OR review, and does not declare the phase complete until both review passes resolve to no P0/P1 findings.

Evidence requirement: `docs/release/evidence/M3_P33_PIECEWISE_LINEAR_BOUNDS.md` must record the interval-analysis test corpus, bound-source traces, convexity/concavity classification, the zero-binary convex epigraph/concave hypograph proof, exact graph segment-binary equivalence, nonconvex exactness tests, scaling diagnostics, representation report examples, the RED failures per task, the full verification matrix, the public API diff, reviewer findings and dispositions, and residual risks before the gate result is marked pass.

## Artifacts this plan produces

New modules and symbols (all names/signatures follow the approved design §7/§17 and the P32 patterns):

- `src/construct/piecewise_linear.rs` — `PiecewiseLinearConstraint`, `PwlRelation`, `ExtrapolationPolicy`, `PwlCurvature`, `PwlPoint`.
- `src/compiler/bridge/piecewise_linear.rs` — the one-sided zero-binary bridge (Task 2) and the exact segment-binary bridge (Task 3), both through the P32 `BridgeFinalizer`.
- `src/compiler/origin.rs` — `GeneratedRole` PWL role variants.
- `src/compiler/session.rs` — the `ConstructKind::PiecewiseLinear` dispatch arm.
- `src/construct/mod.rs` — the `PiecewiseLinear` variant + `derive_parameter_dependencies` extension.
- `src/model/mod.rs` — the public `add_piecewise_linear` builder.
- `src/lib.rs` / `src/advanced.rs` — A30-pattern re-exports of the PWL payload types and helper enums.
- `src/compiler/capability.rs` — additive `BackendFeature::PiecewiseLinear`.
- `src/compiler/report.rs` — PWL formulation-decision entries.
- `roml-highs/src/session.rs` — `SupportLevel::Bridge` declaration for `BackendFeature::PiecewiseLinear`.
- Test files: `tests/piecewise_linear.rs` (created), `roml-highs/tests/formulation_equivalence.rs` (appended `pwl` sections).
- Evidence: `docs/release/evidence/M3_P33_PIECEWISE_LINEAR_BOUNDS.md`.

## must_haves

Goal-backward verification (the ROADMAP P33 gate, verbatim):

> **Gate:** no unproven Big-M; one-sided convex/concave formulations introduce zero binaries; exact/nonconvex graphs remain exact; reports explain every representation.

**Truths (observable behaviors):**

1. **Every PWL point is finite and strictly increasing; invalid points are typed rejections.** Non-finite, duplicate, out-of-order, and underspecified points return typed `ModelError`s — never silently accepted (SM-14.1).
2. **Curvature classification is deterministic from segment slopes.** Affine/convex/concave/nonconvex derive from the sign pattern of consecutive slopes (SM-14.2).
3. **Convex epigraph and concave hypograph introduce zero binaries.** The compiled snapshot for these relations contains no generated binary variables — proven by a direct assertion (SM-14.3).
4. **Exact and nonconvex graphs remain exact.** The exact graph compiles to the deterministic exact segment-binary formulation; a nonconvex graph provably excludes the convex relaxation — never a silent relaxation (SM-14.4/SM-14.5).
5. **No unproven Big-M.** PWL bridges introduce no default Big-M constant; the only bound-derived quantities come from the `BoundAnalyzer` over the argument interval and breakpoints (SM-13.2, D12).
6. **Reports explain every representation.** The compilation report states curvature, relation, representation, generated counts, and why binaries were introduced or avoided (SM-14.6).
7. **Randomized fixed-input evaluations agree with compiled formulations** over the tested domain for all four curvature classes (SM-14.7).
8. **Stable handles and formulation diagnostics.** `add_piecewise_linear` returns a stable `Construct` handle plus the output-variable handle, and the report exposes the PWL formulation decisions (SM-12.8).

**Required artifacts and wiring** are listed in the frontmatter `must_haves` block (truths / artifacts / key_links). The `key_links` entries name the critical connections: curvature → representation selection, `BoundAnalyzer` → finite bound evidence, `native_payloads_available()` → Bridge-only selection, `BridgeFinalizer` → origin completeness.

## Threat model

This is a modeling library; P33 introduces no network, filesystem, auth, or untrusted-input surface. The relevant trust boundaries are integrity/invariant boundaries (extending P32's table):

| Boundary | Description | Mitigation in this plan |
|----------|-------------|--------------------------|
| canonical PWL construct state → compiler | the compiler reads immutable snapshot construct entries only | `compile_snapshot` iterates `ModelSnapshot.constructs` deterministically; no mutable `Model` access (SM-03.2) |
| compiler → generated PWL entities | generated state must be complete, deterministic, and traceable | P32 `BridgeFinalizer` enforces deterministic generated order and `EntityOrigin::Construct` completeness; `BackendSnapshot::validate` re-checks (D5) |
| curvature classification → representation | a wrong curvature silently changes the feasible set | deterministic slope-sign classification (SM-14.2); relation/curvature mismatch is a typed error (D13) |
| exactness semantics | a convex relaxation mislabeled as the exact nonconvex graph | the nonconvex-exactness proof test excludes the convex hull relaxation (SM-14.5) |
| numeric arithmetic | NaN/infinity in breakpoints, values, or slopes | builder validates finite points (SM-14.1); `BoundAnalyzer` rejects non-finite input (SM-13.1) |
| capability gating | a native PWL/SOS2 claim without qualification misleads backend selection | `native_payloads_available()` stays false; PWL is Bridge-only; `NativeRequired` rejects (P32 F4, SM-04.3) |

**STRIDE register:**

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-33-01 | Tampering | PWL point validation | high | mitigate | typed rejections for non-finite/duplicate/out-of-order/underspecified points (SM-14.1) |
| T-33-02 | Tampering | curvature classification | high | mitigate | deterministic slope-sign classification; relation/curvature mismatch is a typed error (SM-14.2, D13) |
| T-33-03 | Spoofing | nonconvex exact graph mislabeled exact | high | mitigate | exact segment-binary formulation + nonconvex proof test; never a convex relaxation (SM-14.5) |
| T-33-04 | Tampering | zero-binary one-sided rows | high | mitigate | supporting-inequality rows with zero generated binaries asserted in tests (SM-14.3) |
| T-33-05 | Tampering | exact segment-binary formulation | high | mitigate | deterministic adjacency-binary convex-combination; randomized fixed-input equivalence (SM-14.4/14.7) |
| T-33-06 | Tampering | Big-M / bound evidence | high | mitigate | no default Big-M; only `BoundAnalyzer`-derived quantities; bound sources in report (SM-13.2/13.5, D12) |
| T-33-07 | Information disclosure | `CompileError` messages | low | mitigate | errors name the construct and expression (SM-13.4) — internal model identifiers only, no secrets |
| T-33-SC | Tampering | npm/pip/cargo installs | low | accept | no new dependencies this plan (stdlib + existing workspace only); no package install tasks |

No new `unsafe`, environment mutation, filesystem scan, or stdout output is introduced by this plan.

## Gate

Plan 01 (packet Task 18) passes when:

- Task 1 lands the PWL semantic payload, the four validation rejections, deterministic curvature classification, and a direct interpolation/extrapolation evaluator (SM-14.1, SM-14.2, SM-12.8);
- Task 2 lands the zero-binary convex epigraph / concave hypograph supporting-inequality rows with the binary-avoidance reason and bound evidence reported (SM-14.3, SM-14.6, SM-13.5);
- Task 3 lands the deterministic exact segment-binary exact graph for all curvature classes, proves nonconvex graphs never fall back to a convex relaxation, verifies randomized fixed-input equivalence, and adds HiGHS reference-vs-portable PWL equivalence (SM-14.4, SM-14.5, SM-14.7);
- the report explains every representation, curvature, relation, generated counts, and binary-avoidance/introduction reason (SM-14.6);
- no unproven Big-M exists anywhere in the PWL bridges (SM-13.2, D12); no auxiliary LP runs for bound tightening (SM-13.6);
- `NativeRequired` on a PWL construct rejects with `CompileError::UnsupportedFeature` and no native PWL/SOS2 claim is made (P32 F4 rule; `BackendConstraint` stays empty);
- every generated PWL entity carries `EntityOrigin::Construct { construct, role }` (SM-02.5);
- the frozen P26 compiler contract and the P32 bridge framework are unchanged (no compiler-contract or framework amendment without a reviewed `DECISIONS.md` change);
- all phase-level verification commands exit 0 (focused `piecewise_linear`/`formulation_equivalence pwl` targets, both `--all-targets` suites, both clippy lanes, rustdoc with warnings denied, and `cargo public-api -p roml`);
- the ROADMAP P33 gate holds verbatim: no unproven Big-M; one-sided convex/concave formulations introduce zero binaries; exact/nonconvex graphs remain exact; reports explain every representation;
- the evidence bundle (interval-analysis corpus, bound-source traces, classification, zero-binary proof, exact-graph equivalence, nonconvex exactness, scaling diagnostics, representation report examples), public API diff, clause-level scope statements, and residual risks are recorded; and
- the requested OR review and the phase-boundary independent review passes resolve with no P0/P1 findings.

No crate publication, tag, or release is part of this plan (SM-15.8 / M3 stopping condition).

## Output

Create `.planning/phases/33-piecewise-linear-bounds/33-SUMMARY.md` when done, per the phase completion protocol.
