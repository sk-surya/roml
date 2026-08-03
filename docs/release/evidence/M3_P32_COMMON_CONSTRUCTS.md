# P32 Evidence — Common Semantic Modeling Constructs

**Phase:** 32-common-constructs
**Plan:** `32-PLAN.md` — Task 15 (add interval bounds and bridge framework)
**Requirements (Task 15):** SM-13.1, SM-13.2, SM-13.3, SM-13.4, SM-13.5, SM-13.6, SM-02.5 (foundation)
**Branch:** `phase-roml-P32-common-constructs` (executor worktree: `worktree-agent-a76643e4a6805f906`)
**Base:** `main@192cd00502b6795c5265131483bfee8974039cf6`
**Status:** Task 15 in progress — baseline captured, RED failures recorded, implementation following.

This document records the P32 Task 15 deliverables per `EXECUTION.md` § "Evidence file structure": the untouched baseline matrix, per-task TDD verification (RED failures first), the focused/full verification matrix, the public API diff, deviations, and residual risks. Task 16 (logical constructs + A30) appends its own sections. The SM-13 clause-level scope statement (full SM-13 closure remains P33) and the SM-12 follow-up-plan disclosure (min/max/abs/products land in the Task 17 follow-up plan) are recorded below per the plan's review-gate requirement.

## Scope and requirements

Task 15 delivers the P32 foundation: deterministic interval bound analysis (`src/compiler/bounds.rs`), one-sided Big-M helpers returning finite derived/validated values or a construct-aware `UnboundedBigM` marker, the bridge contract + `BridgeFinalizer` (`src/compiler/bridge/mod.rs`), the `CompileError::UnboundedBigM { construct, expression }` public error (design §19, SM-13.4), and bound-evidence report entries (SM-13.5). No arbitrary default Big-M constant exists (D12); M3 runs no auxiliary LP for bound tightening (SM-13.6).

**Clause-level scope:** SM-13 is closed by P32 only at the level the common-construct bridges need (interval analysis over linear scalar functions; finite-bound/validated Big-M; construct-identifying errors; M-value/bound-source reporting; no silent auxiliary tightening). Full SM-13 closure (PWL interval analysis extended to curvature, bound-source traces, `BigM` evidence on the P33 fixtures) remains **P33**. SM-12 is NOT a mandate of this plan's requirement list; Task 16 advances SM-12.1/12.2/12.5/12.8, and the remaining SM-12 clauses (SM-12.3/12.4/12.6/12.7) land in the Task 17 follow-up plan (Plan 02 of P32).

## Baseline and environment

| Item | Value |
|---|---|
| Base commit (`main`) | `192cd00502b6795c5265131483bfee8974039cf6` (P26 merged, PR #28) |
| HEAD at baseline capture | `192cd00502b6795c5265131483bfee8974039cf6` |
| Branch | `phase-roml-P32-common-constructs` (executor worktree `worktree-agent-a76643e4a6805f906`) |
| `rustc --version` | `rustc 1.97.1 (8bab26f4f 2026-07-14)` |
| `cargo --version` | `cargo 1.97.1 (c980f4866 2026-06-30)` |
| `rustc -vV` host | `aarch64-apple-darwin` |
| OS | `Darwin 25.4.0 arm64` |
| `cargo public-api --version` | `cargo-public-api 0.52.0` |
| HiGHS build | bundled via `highs-sys 1.15.0` (cmake); no system HiGHS |

All commands ran on the platform above with the toolchain above at the HEAD above, on the **untouched** tree (no P32 source modification; working tree clean except pre-existing untracked `.planning/config.json`, `.planning/graphs/`, `graphify-out/`).

### Untouched baseline matrix — `roml`

| Command | Exit | Result |
|---|---|---|
| `cargo fmt --all -- --check` | 0 | formatting clean |
| `cargo check -p roml --all-targets` | 0 | clean |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `cargo test -p roml --all-targets` | 0 | **675 passed; 0 failed; 0 ignored** |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 | docs generated, no warnings |
| `cargo package --list -p roml` | 0 | 88 files |
| `cargo public-api -p roml` | 0 | 15123 public items |

### Untouched baseline matrix — `roml-highs`

| Command | Exit | Result |
|---|---|---|
| `cargo check -p roml-highs --all-targets` | 0 | clean |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `cargo test -p roml-highs --all-targets` | 0 | **114 passed; 0 failed; 0 ignored** |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` | 0 | docs generated, no warnings |
| `cargo package --list -p roml-highs` | 0 | 32 files |

`roml-mosek`/`roml-xpress` are known-broken against the current facade and out of scope (M2 convention) — never exercised with workspace-wide commands.

### Public API / package capture

Raw captures at the P32 Task 15 base:

- `cargo public-api -p roml` — **15123** items (raw).
- `cargo package --list -p roml` — **88** files.
- `cargo package --list -p roml-highs` — **32** files.

## Commit trail

| # | SHA | Message |
|---|---|---|
| — | — | (Task 15 commit recorded after GREEN) |

---

## Task 15 — Add interval bounds and bridge framework

**Phase:** P32  **Requirements:** SM-13.1, SM-13.2, SM-13.3, SM-13.4, SM-13.5, SM-13.6, SM-02.5 (foundation)
**Status:** complete — committed as `feat(compiler): add safe bridge infrastructure`.

### TDD — RED failures (recorded before implementation)

`cargo test -p roml --test compiler_bridges` failed to compile against the untouched tree — the `roml::compiler::{bounds, bridge}` modules and the entire bound-analysis/bridge surface did not exist. Expected failure, recorded verbatim:

```text
error[E0432]: unresolved import `roml::compiler::bounds`
  --> tests/compiler_bridges.rs:21:5
   |
21 | use roml::compiler::bounds::{BoundAnalyzer, BoundError, BoundSource, BoundTrace, Interval};
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no `bounds` in `compiler`
For more information about this error, try `rustc --explain E0432`.
error: could not compile `roml` (test "compiler_bridges") due to 1 previous error
```

The in-crate `#[cfg(test)]` suites (`src/compiler/bounds.rs`, `src/compiler/bridge/mod.rs`) likewise failed to compile (the framework types did not exist). No production source existed before the RED tests.

### Implementation

- **`src/compiler/bounds.rs` (create)** — deterministic interval bound analysis and Big-M safety:
  - `pub struct Interval { lower, upper }` (design §9) with `exact`/`is_bounded`/`contains`.
  - `pub struct BoundTrace { sources: Vec<BoundSource>, result: Interval }`; `BoundSource` is a `#[doc(hidden)] pub` provenance marker (`DeclaredVariableBounds`, `FixedValue`, `ParameterValue`, `Constant`) — see deviations.
  - `pub enum BoundError` — typed analysis failures (`NonFiniteCoefficient`, `NonFiniteBound`, `InvalidBounds`, `NonFiniteConstant`, `NonFiniteParameterValue`, `ArithmeticNan`, `UnsupportedFunctionKind`) with `Display` + `Error`.
  - `pub struct BoundAnalyzer` — `interval_of(function, variable_bounds, parameter_values)` performs deterministic linear interval propagation over `ScalarFunction::Linear`: coefficient signs flip the contribution, constants offset the interval, fixed variables (equal lower/upper bounds — the fixing representation) contribute exact values, infinite bounds propagate to unbounded endpoints, and bare-parameter coefficients evaluate against the supplied parameter values. Terms are processed in sorted variable order (determinism regardless of input term order). NaN/inverted/non-finite input is a typed `BoundError` (SM-13.1); no auxiliary LP is ever run (SM-13.6). `interval_of_snapshot` reads declared bounds and evaluated parameters from a canonical `ModelSnapshot`.
  - `pub enum BigMImplication { Upper, Lower }` and `pub struct BigMRequest<'a, F, G>` (construct, expression, function, side, rhs, variable_bounds, parameter_values).
  - `pub fn bound_big_m_implied` — derives the tightest finite one-sided M (`max(0, max f - rhs)` / `max(0, rhs - min f)`), returning the finite M or the construct-aware `CompileError::UnboundedBigM` (SM-13.2, D12). `pub fn validated_explicit_big_m` — validates an explicit user M against known bounds (rejects below the derived minimum — SM-13.3; accepts a finite value when the derived bound is infinite — the D12 explicit-value contract; rejects non-finite/non-positive). Non-finite analysis surfaces as `CompileError::InvalidBigM` (SM-13.1).
  - `pub(crate) struct UnboundedBigM { construct, expression }` — the implementation-detail marker the helpers convert (via `From`) into the public `CompileError::UnboundedBigM`; never a silent default constant.
- **`src/compiler/bridge/mod.rs` (create)** — the bridge contract + `BridgeFinalizer` (design §8.5):
  - `pub enum BridgeRepresentation { LinearRows, LinearRowsWithAuxiliaryVariables }` (`#[non_exhaustive]`).
  - `pub enum BridgeDependency { Construct, Variable, Parameter }` — dependency capture (design §8.5).
  - `pub struct BridgeFinalizer` — allocates dense compiled ids in the bridge's fixed per-role call order (deterministic generated order), records `EntityOrigin::Construct { construct, role }` for every generated variable/row (D5, SM-02.5) via `add_variable`/`add_row`, captures dependencies (`add_dependency`), records bound/Big-M evidence report entries (`record_bound_evidence`, SM-13.5), and finalizes into `BridgeOutput`. Exact representations that cannot be produced surface as a typed `CompileError` (design §19) — the finalizer never silently relaxes.
- **`src/compiler/mod.rs`** — declares `pub mod bounds; pub mod bridge;` and adds `CompileError::UnboundedBigM { construct, expression }` (SM-13.4, design §19) and `CompileError::InvalidBigM { construct, expression, reason }` (the design §19 "invalid Big-M" family — see deviations).
- **`src/compiler/origin.rs`** — adds the generic `GeneratedRole::Bridge` role (see deviations); `#[non_exhaustive]` boundary stays.
- **`src/compiler/report.rs`** — adds `FormulationDecision::bound_evidence(key, m_value, derivation, bound_sources)` — the bound/Big-M evidence report-entry constructor (SM-13.5).
- **`src/advanced.rs`** — re-exports the bounds and bridge framework surface for framework/backend authors.
- **Tests** — `tests/compiler_bridges.rs` (15 integration tests) plus in-crate `#[cfg(test)]` suites in `bounds.rs` (9) and `bridge/mod.rs` (7) — see deviations for the placement split.

### Focused verification

| Command | Result |
|---|---|
| `cargo test -p roml --test compiler_bridges` | 0 — **15 passed; 0 failed** |
| `cargo test -p roml --lib bounds` | 0 — **9 new bounds unit tests pass** (16 matched incl. pre-existing) |
| `cargo test -p roml --lib bridge` | 0 — **7 new bridge unit tests pass** |

### Full verification

| Command | Exit | Result |
|---|---|---|
| `cargo fmt --all -- --check` | 0 | formatting clean |
| `cargo test -p roml --test compiler_bridges` | 0 | **15 passed** |
| `cargo test -p roml --all-targets` | 0 | **706 passed; 0 failed; 0 ignored** (baseline 675 + 31 new) |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 | docs generated, no warnings |
| `cargo test -p roml-highs --all-targets` | 0 | **114 passed; 0 failed** (unchanged) |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` | 0 | docs generated, no warnings |
| `cargo public-api -p roml` | 0 | **15123 → 15983 items** (+860 additive: the bounds/bridge framework types and `CompileError::UnboundedBigM`/`InvalidBigM`; M2 guarded surface unchanged) |

Baseline comparison: `roml` grew from 675 to 706 passing tests (+31 = 15 integration + 9 bounds unit + 7 bridge unit). `roml-highs` is unchanged at 114. No existing test weakened or deleted.

### Acceptance criteria

- **`src/compiler/bounds.rs` defines `Interval { lower, upper }`, `BoundTrace { sources, result }`, and `BoundAnalyzer` whose deterministic linear propagation handles coefficient signs, constants, fixed/equal-bound variables, infinite bounds, and evaluated parameters, and rejects NaN/non-finite input with a typed error (SM-13.1, SM-13.6)** — met. No auxiliary LP is ever run (SM-13.6).
- **One-sided Big-M helpers return a finite derived value, a validated explicit user value, or the construct-aware `UnboundedBigM` marker — never a silent default constant (SM-13.2, D12); explicit M is validated against known bounds where possible (SM-13.3)** — met. `bound_big_m_implied` derives finite M or `CompileError::UnboundedBigM { construct, expression }`; `validated_explicit_big_m` rejects inconsistent/non-positive/non-finite explicit M and accepts a finite explicit value when the derived bound is infinite.
- **`src/compiler/bridge/mod.rs` implements bridge finalization with deterministic generated order, dependency capture, complete origins (`EntityOrigin::Construct { construct, role }`, SM-02.5), and report entries recording M values/derivations/bound sources (SM-13.5)** — met. `BridgeFinalizer` allocates dense ids in call order, records construct origins for every generated entity (completeness validator returns empty), captures dependencies, and appends `FormulationDecision::bound_evidence` entries.
- **`src/compiler/mod.rs` declares `CompileError::UnboundedBigM { construct, expression }` identifying the construct and the missing/unbounded expression (SM-13.4, design §19)** — met.
- All three Task 15 verification commands exit 0 — met.

### Deviations

1. **Test placement (F3 precedent).** The plan's Task 15 test bullet lists one-sided Big-M and bridge-finalization tests under `tests/compiler_bridges.rs`. There is NO public way to obtain a canonical `Construct` handle until Task 16's public builders land (`Model::add_construct_fixture` is crate-private per A30; `ConstructId::allocate` is `pub(crate)`), so integration tests cannot exercise the construct-dependent surfaces. The construct-dependent tests therefore live in-crate (`src/compiler/bounds.rs` and `src/compiler/bridge/mod.rs` `#[cfg(test)]`), mirroring the F3 precedent documented in `tests/semantic_ir.rs` ("construct-lifecycle tests moved IN-CRATE"). `tests/compiler_bridges.rs` covers the public surface that does not need a construct handle: interval analysis over all five coefficient/bound classes, NaN rejection, determinism, and the snapshot convenience.
2. **`BoundSource` is `#[doc(hidden)] pub`, not crate-private.** The design §9 public signature `pub struct BoundTrace { pub sources: Vec<BoundSource>, ... }` requires `BoundSource` to be nameable (a crate-private type in a public field is a `private_interfaces` error). It is kept `#[doc(hidden)]` and documented as an implementation-detail provenance marker; it is reachable only because `compiler` is a public module. Not re-exported as a documented surface.
3. **`GeneratedRole::Bridge` added in Task 15.** The plan's Task 15 test bullet requires `EntityOrigin::Construct { construct, role }` for every generated bridge entity, but `GeneratedRole` was an empty `#[non_exhaustive]` enum (P26). Added the generic `Bridge` role so the finalizer can record origins — Rule 2 auto-add of missing critical functionality (the framework cannot function without any role value). `src/compiler/origin.rs` is in the phase's `files_modified` list; Task 16's specific per-construct roles are additive and the `#[non_exhaustive]` boundary stays.
4. **`CompileError::InvalidBigM { construct, expression, reason }` added.** The plan's `src/compiler/mod.rs` change bullet explicitly permits "any bridge error variants the design §19 families need"; design §19's "unbounded or invalid Big-M" family needs a distinct typed error for NaN/non-finite analysis (SM-13.1), separate from the unbounded marker.
5. **`BigMRequest` context struct.** The two one-sided Big-M helpers take a `BigMRequest { construct, expression, function, side, rhs, variable_bounds, parameter_values }` context struct rather than 8–9 positional arguments, to keep the public surface clippy-clean (`clippy::too_many_arguments`) and readable. The helper names and semantics are unchanged from the plan.

### Commit trail

- `feat(compiler): add safe bridge infrastructure` — Task 15 implementation + tests + evidence (single coherent unit).

---
