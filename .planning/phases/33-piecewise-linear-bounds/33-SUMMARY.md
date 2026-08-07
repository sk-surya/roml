---
phase: 33-piecewise-linear-bounds
plan: 01
subsystem: compiler
tags: [piecewise-linear, pwl, big-m, exact-graph, zero-binary, curvature, bound-analysis]
dependency_graph:
  requires: [P26 compiler backend IR (frozen contract), P32 Plan 02 bridge framework (BridgeFinalizer, BoundAnalyzer, exact segment binaries)]
  provides: [PiecewiseLinearConstraint, PwlRelation, ExtrapolationPolicy, PwlCurvature, PwlPoint, Model::add_piecewise_linear, zero-binary one-sided PWL bridges, exact segment-binary PWL graph, PwlEvalError typed errors]
  affects: [P34 (qualification), P30 (soft-constraint relaxations over PWL), future NLP (nonconvex-exactness precedent)]
tech-stack:
  added: []
  patterns:
    - convex epigraph / concave hypograph compiled with zero binaries (one-sided Big-M rows from BoundAnalyzer intervals)
    - nonconvex PWL compiled exactly via deterministic segment binaries — never a convex relaxation (SM-14.5 nonconvex-exactness proof)
    - curvature classification at add time (PwlCurvature) with relation/curvature mismatch rejection (no-silent-relaxation)
    - extrapolation policy enforced at compile time (b529989)
    - typed PwlEvalError for parameter-dependent point values (no panics on valid parameterized payloads, P1-01)
    - honest capability reporting: SupportLevel::Bridge, never a native claim (BackendFeature::PiecewiseLinear/Sos2 stay Unsupported on HiGHS)
key-files:
  created:
    - src/construct/piecewise_linear.rs
    - src/compiler/bridge/piecewise_linear.rs
    - tests/piecewise_linear.rs
    - docs/release/evidence/M3_P33_PIECEWISE_LINEAR_BOUNDS.md
  modified:
    - src/construct/mod.rs
    - src/model/mod.rs
    - src/lib.rs
    - src/advanced.rs
    - src/compiler/bridge/mod.rs
    - src/compiler/origin.rs
    - src/compiler/capability.rs
    - src/compiler/session.rs
    - src/compiler/report.rs
    - roml-highs/src/session.rs
    - roml-highs/tests/formulation_equivalence.rs
decisions:
  - No new native payloads in BackendConstraint and no HiGHS native PWL/SOS2 projection: BackendConstraint stays empty; PWL is selected/reported as SupportLevel::Bridge with the exact graph compiled through deterministic segment binaries (P32 F4 rule)
  - BackendFeature::NativePiecewiseLinear and BackendFeature::Sos2 remain SupportLevel::Unsupported on HiGHS; NativeRequired on a PWL construct rejects with CompileError::UnsupportedFeature
  - Convex epigraph/concave hypograph use zero binaries; exact/nonconvex graphs use exact segment binaries (never a relaxation)
  - point_value/evaluate/classify_curvature/segment_slopes return typed PwlEvalError (ParameterizedPointValue/MissingParameter) for parameter-dependent or unresolved-parameter inputs; the _with resolver variants evaluate parameterized payloads
  - ExtrapolationPolicy (Constant/Linear) enforced at compile time
  - Interval analysis extended to the PWL argument expression; bound-source traces recorded (SM-13 extension)
metrics:
  duration: ~3 days (multi-review-round phase)
  completed: 2026-08-04
  tasks: 3 (PWL semantics tracer; one-sided zero-binary bridges; exact segment-binary graph + randomized equivalence + HiGHS)
  commits: feat commits (4d01605→b529989→a2adbcc→006f8be post-rebase) + evidence commits; rebase note: all SHAs changed on 2026-08-04 after PR #33 merge
status: complete
actuals:
  tokens: ~45000   # evidence doc ~40KB; verify against diff if precision needed
  tasks: 3
  commits: 6 (3 task + evidence + 2 review-fix; post-rebase)
---

# Phase [33] Task [1-3]: Piecewise-linear functions and bound analysis

Safe convexity-aware PWL modeling over the frozen P26 compiler contract + P32 bridge framework: exact piecewise-linear semantics and formulations, with convex epigraph/concave hypograph compiled with zero binaries and exact/nonconvex graphs compiled exactly (never a convex relaxation), all reported with curvature, representation, generated counts, and binary-avoidance/introduction reasons. Requirements closed: SM-13 (full closure), SM-14.1–14.7, SM-12.1/12.2/12.5 (closed by P32, recorded here), SM-12.8 (advanced).

## What was built

- **`src/construct/piecewise_linear.rs`** — `PiecewiseLinearConstraint` (breakpoints + point values + extrapolation policy), `PwlRelation`, `ExtrapolationPolicy`, `PwlCurvature`, `PwlPoint` (`From<(f64, f64)>`); `classify_curvature`, `segment_slopes`, `evaluate`, `parameter_dependencies` + `_with` resolver variants; typed `PwlEvalError` (`ParameterizedPointValue { index, parameter }` / `MissingParameter { parameter }`) — no panic on valid parameterized payloads (P1-01 fix). `ModelError::{PwlTooFewPoints, PwlNonFiniteBreakpoint, PwlNonFinitePointValue, PwlDuplicateBreakpoint, PwlOutOfOrderBreakpoint}`.
- **`src/model/mod.rs`** — `Model::add_piecewise_linear` (SM-12.8 stable-handle + output-variable builder).
- **`src/compiler/bridge/piecewise_linear.rs`** — Task 2: one-sided zero-binary bridges for convex epigraph (`y >= f(x)`) and concave hypograph (`y <= f(x)`) from BoundAnalyzer intervals (Big-M rows, `UnboundedBigM` construct-aware marker); Task 3: exact segment-binary graph for arbitrary relations (deterministic generated ids, `GeneratedRole::{PwlEpigraphRow, PwlHypographRow, PwlExactGraphRow, PwlSegmentBinary, PwlWeightVariable}`), `BackendFeature::PiecewiseLinear` additive capability.
- **`src/compiler/{origin,capability,session,report}.rs`** — origin mapping for every generated PWL entity (SM-02.5 asserted), capability declarations, bound-source traces in reports (SM-13.5), extrapolation policy enforced at compile time (b529989).
- **`roml-highs/src/session.rs`** — `SupportLevel::Bridge` selection/reporting; `NativePiecewiseLinear`/`Sos2` remain `Unsupported` (no false native claim).
- **`tests/piecewise_linear.rs`** (35 tests) — TDD RED→GREEN per task; **`roml-highs/tests/formulation_equivalence.rs`** — PWL reference-vs-portable equivalence (1 test).

## Verification

| Command | Result |
|---|---|
| `cargo test -p roml --test piecewise_linear` | 0 — 35 passed (post-second-round) |
| `cargo test -p roml-highs --test formulation_equivalence pwl` | 0 — 1 passed |
| `cargo test -p roml --all-targets` | 0 — 914 passed; 0 failed; 0 ignored (phase-level matrix) |
| `cargo test -p roml-highs --all-targets` | 0 — 132 passed; 0 failed; 0 ignored |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 — clean |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 — clean |
| `RUSTDOCFLAGS='-D warnings' cargo doc` (both crates) | 0 — clean |
| `cargo fmt --all -- --check` | 0 — clean |
| `cargo public-api -p roml` | 0 — 22,178 items (additive-only; baseline capture caveat documented in evidence) |

## Deviations from plan

**None.** The plan was executed exactly as written. The obsolete `pwl_exact_graph_is_typed_error_before_task3` test (which asserted the Task 2 placeholder behavior) was superseded by the Task 3 implementation — recorded as the TDD RED→GREEN transition, not a plan deviation.

## Known Stubs

- **No native PWL/SOS2:** BackendConstraint stays empty; `native_payloads_available()` stays false; `BackendFeature::NativePiecewiseLinear`/`Sos2` remain `Unsupported` on HiGHS (F4 rule). SOS2 is a declared possible representation only when a backend qualifies native `Sos2` (none does in M3); the actual emitted representation is exact segment binaries.
- **`PwlEvalError` variants** cover parameter-dependent points and unresolved parameters; constant-only evaluation is fully typed (no panic paths remain).

## Self-Check: PASSED

Merged via PR #31 (`4caf03c`) onto main. Created files verified: `src/construct/piecewise_linear.rs`, `src/compiler/bridge/piecewise_linear.rs`, `tests/piecewise_linear.rs`, `docs/release/evidence/M3_P33_PIECEWISE_LINEAR_BOUNDS.md`. Post-fix verification matrix all exit 0 (piecewise_linear 35/35; roml 35 suites; roml-highs 18 suites; clippy/rustdoc `-D warnings`; fmt). Full review dispositions (P1-01 constant-only panic fix, P2-03 baseline-capture caveat, second-round owner disposition) in `REVIEW.md`; integration verdict in `REVIEW-INTEGRATION.md`.
