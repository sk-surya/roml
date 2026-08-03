---
phase: 32-common-constructs
plan: 01
subsystem: compiler
tags: [bounds, bridge, interval-analysis, big-m, bound-analyzer, bridge-finalizer]
dependency_graph:
  requires: [P26 compiler backend IR (CompiledEntityRegistry, OriginMap, CompilationReport, CompileError), P25 construct arena]
  provides: [src/compiler/bounds.rs, src/compiler/bridge/mod.rs, tests/compiler_bridges.rs]
  affects: [Task 16 (logical construct bridges written against BoundAnalyzer/BridgeFinalizer), P33 (PWL bound analysis), P30 soft-constraint bound inference]
tech-stack:
  added: []
  patterns:
    - deterministic linear interval propagation over ScalarFunction::Linear (sorted term order, NaN rejection)
    - one-sided Big-M derivation returning a finite value or a construct-aware UnboundedBigM marker (never a default constant)
    - bridge finalizer enforcing EntityOrigin::Construct completeness (D5/SM-02.5) with dense deterministic compiled ids
key-files:
  created:
    - src/compiler/bounds.rs
    - src/compiler/bridge/mod.rs
    - tests/compiler_bridges.rs
    - docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md
  modified:
    - src/compiler/mod.rs
    - src/compiler/origin.rs
    - src/compiler/report.rs
    - src/advanced.rs
decisions:
  - BoundAnalyzer derives intervals from declared bounds/coefficients only — no auxiliary LP for tightening (SM-13.6)
  - Big-M helpers return the construct-aware CompileError::UnboundedBigM (never a silent default constant); explicit M validated against known bounds (SM-13.3)
  - BridgeFinalizer allocates dense compiled ids in the bridge's per-role call order and records EntityOrigin::Construct for every generated entity
  - GeneratedRole::Bridge added as the generic role so the framework can record construct origins before the per-construct roles land (Task 16)
  - BoundSource kept #[doc(hidden)] pub because BoundTrace.sources is a public field (design §9 signature)
  - BigMRequest context struct bundles the 8 derivation inputs to keep the helper surface clippy-clean
metrics:
  duration: ~50 min
  completed: 2026-08-03
  tasks: 1 (Task 15 of 2-task phase)
  commits: 1 (plus this summary commit)
status: complete
actuals:
  tokens: 21452   # chars/4 over the realized Task 15 diff (85,808 bytes)
  tasks: 1
  commits: 2
---

# Phase [32] Task [15]: Add interval bounds and bridge framework

Deterministic interval bound analysis (`BoundAnalyzer`), one-sided Big-M helpers returning finite derived/validated values or a construct-aware `UnboundedBigM` marker, the bridge contract + `BridgeFinalizer` (deterministic generated order, dependency capture, complete `EntityOrigin::Construct` origins, and bound-evidence report entries), the public `CompileError::UnboundedBigM { construct, expression }`, and bound-evidence report-entry support. No arbitrary Big-M constant exists (D12); M3 runs no auxiliary LP for bound tightening (SM-13.6); NaN input is rejected with a typed `BoundError` (SM-13.1).

## What was built

- **`src/compiler/bounds.rs`** — the P32 bound-analysis foundation (SM-13.1/13.2/13.3/13.6, D12):
  - `Interval { lower, upper }` with `exact`/`is_bounded`/`contains` (design §9).
  - `BoundTrace { sources, result }` + `#[doc(hidden)] pub BoundSource` provenance marker (`DeclaredVariableBounds`/`FixedValue`/`ParameterValue`/`Constant`).
  - `BoundError` typed analysis failures (`NonFiniteCoefficient`/`NonFiniteBound`/`InvalidBounds`/`NonFiniteConstant`/`NonFiniteParameterValue`/`ArithmeticNan`/`UnsupportedFunctionKind`).
  - `BoundAnalyzer::interval_of` — deterministic linear interval propagation over coefficient signs, constants, fixed/equal-bound variables, infinite bounds, and evaluated parameters (terms processed in sorted var order); NaN/inverted/non-finite input is a typed `BoundError`. `interval_of_snapshot` reads declared bounds + evaluated parameters from a `ModelSnapshot`.
  - `BigMImplication { Upper, Lower }` + `BigMRequest` context; `bound_big_m_implied` derives `max(0, max f - rhs)`/`max(0, rhs - min f)` or the construct-aware `CompileError::UnboundedBigM`; `validated_explicit_big_m` rejects inconsistent/non-positive/non-finite explicit M and accepts a finite explicit value when the derived bound is infinite (D12). The `pub(crate) UnboundedBigM { construct, expression }` marker converts into the public error (SM-13.4).
- **`src/compiler/bridge/mod.rs`** — the bridge contract + finalizer (design §8.5, SM-13.5, SM-02.5):
  - `BridgeRepresentation { LinearRows, LinearRowsWithAuxiliaryVariables }` (`#[non_exhaustive]`).
  - `BridgeDependency { Construct, Variable, Parameter }` — dependency capture.
  - `BridgeFinalizer` — dense compiled ids in per-role call order, `EntityOrigin::Construct { construct, role }` for every generated variable/row, `record_bound_evidence` (M value, derivation, bound sources), and `finish` → `BridgeOutput`. Exact representations that cannot be produced surface as a typed `CompileError`.
- **`src/compiler/mod.rs`** — `pub mod bounds; pub mod bridge;` + `CompileError::UnboundedBigM { construct, expression }` (SM-13.4, design §19) and `CompileError::InvalidBigM { construct, expression, reason }` (invalid-Big-M family).
- **`src/compiler/origin.rs`** — generic `GeneratedRole::Bridge` (the finalizer's role before Task 16's per-construct roles).
- **`src/compiler/report.rs`** — `FormulationDecision::bound_evidence(key, m_value, derivation, bound_sources)` (SM-13.5).
- **`src/advanced.rs`** — re-exports the bounds/bridge framework surface.
- **Tests** — `tests/compiler_bridges.rs` (15 integration tests: interval classes, NaN rejection, determinism, snapshot convenience) + in-crate `bounds.rs` (9) and `bridge/mod.rs` (7) unit tests.

## Verification

| Command | Result |
|---|---|
| `cargo test -p roml --test compiler_bridges` | 0 — 15 passed |
| `cargo test -p roml --all-targets` | 0 — 706 passed; 0 failed; 0 ignored (baseline 675 + 31 new) |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 — clean |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 — clean |
| `cargo test -p roml-highs --all-targets` | 0 — 114 passed; 0 failed (unchanged) |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 — clean |
| `cargo public-api -p roml` | 0 — 15123 → 15983 items (+860 additive) |

## Deviations from plan

1. **Test placement (F3 precedent).** The construct-dependent Big-M-helper and BridgeFinalizer tests live in-crate (`#[cfg(test)]` in `bounds.rs`/`bridge/mod.rs`) because there is no public way to obtain a canonical `Construct` handle until Task 16's public builders land (`add_construct_fixture` is crate-private per A30; `ConstructId::allocate` is `pub(crate)`). This mirrors the F3 precedent documented in `tests/semantic_ir.rs`. `tests/compiler_bridges.rs` covers the public surface that needs no construct handle.
2. **`BoundSource` is `#[doc(hidden)] pub`** rather than strictly crate-private: the design §9 public signature `BoundTrace { pub sources: Vec<BoundSource> }` requires the marker type to be nameable (a crate-private type in a public field is a `private_interfaces` error).
3. **`GeneratedRole::Bridge` added in Task 15** (Rule 2 — missing critical functionality): the finalizer cannot record `EntityOrigin::Construct` with an empty role enum. The generic `Bridge` role is additive; Task 16's per-construct roles extend the `#[non_exhaustive]` enum.
4. **`CompileError::InvalidBigM` added** for the design §19 "invalid Big-M" family (NaN/non-finite analysis), separate from the unbounded marker.
5. **`BigMRequest` context struct** for the two one-sided Big-M helpers — keeps the public surface clippy-clean (`clippy::too_many_arguments`); helper names and semantics unchanged.

Full detail (RED failures, verification matrix, acceptance criteria, per-item dispositions) is in `docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md`.

## Commit trail

| # | SHA | Message |
|---|---|---|
| 1 | `8afa0e8` | `feat(compiler): add safe bridge infrastructure` |
| 2 | (this commit) | `docs(32): summarize Task 15 evidence and deviations` |
