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

## Task 16 — Add indicators, reification, Boolean, and cardinality

**Phase:** P32  **Requirements:** SM-01.3, SM-02.3, SM-02.5, SM-13.4 (exercised); advances SM-12.1/SM-12.2/SM-12.5/SM-12.8 (full SM-12 closure remains the Task 17 follow-up plan)
**Status:** complete — committed as `feat(model): add logical semantic constructs`.

### TDD — RED failures (recorded before implementation)

`cargo test -p roml --test common_constructs indicator` failed to compile against the Task 15 tree — `roml::construct` was still `pub(crate)`, the payload types did not exist, and the `Model::add_*` builders were absent. Expected failure, recorded verbatim:

```text
error[E0603]: module `construct` is private
  --> tests/common_constructs.rs:20:11
   |
20 | use roml::construct::{
   |           ^^^^^^^^^ private module
note: the module `construct` is defined here
  --> src/lib.rs:22:1
   |
22 | pub(crate) mod construct;
   | ^^^^^^^^^^^^^^^^^^^^^^^^
```

`roml-highs/tests/formulation_equivalence.rs` likewise failed to compile (missing payload types/builders).

### Implementation

- **A30 public-surface step (mandatory plan bullet):**
  - `src/lib.rs` — `pub(crate) mod construct;` → `pub mod construct;`; crate-root re-exports `ConstructKind`, `ConstructEntry`, and the four payload types plus the payload-kind enums (`IndicatorDirection`, `BooleanKind`, `CardinalityKind`). `Construct`/`FormulationPreference` re-exports kept; the `#[non_exhaustive]` extension boundary on `ConstructKind` stays (A30).
  - **Stay crate-private:** the `Fixture` variant; `FixturePayload` (fields `pub(crate)`, `pub(crate)` constructor `FixturePayload::new` keeps the in-crate `#[cfg(test)]` `fixture()` helper working); `Model::add_construct_fixture`/`Model::construct` (both `pub(crate)`). `ConstructData` was additionally made `pub(crate)` (it is P25 internal scaffolding, A30).
  - `cargo public-api -p roml` — **15983 → 17536 items** (+1553 additive). `Fixture`, `FixturePayload`, `add_construct_fixture`, `ConstructData`, and `ConstructKind::Fixture` are ABSENT from the output (verified by `grep`).
- **`src/construct/{indicator,reification,boolean,cardinality}.rs` (create)** — design §7 payloads:
  - `IndicatorConstraint { activator: VarId, direction, function, set }` with `IndicatorDirection { WhenOne, WhenZero }` (one-way implication).
  - `ReificationConstraint { activator: VarId, function, set, separation_tolerance: Option<f64>, proven_integrality: bool }` — the reification result binary variable is created by the builder and stored in `activator` (design §16.2).
  - `BooleanConstraint { kind }` with `BooleanKind { Implication, Equivalence, Any, All }`.
  - `CardinalityConstraint { variables: Vec<VarId>, kind, k: usize }` with `CardinalityKind { Exactly, AtMost, AtLeast }`.
- **`src/construct/mod.rs`** — four `ConstructKind` variants; `derive_parameter_dependencies` extended (indicator/reification defer to the constrained function's derived dependencies); the `Fixture` variant and `FixturePayload` stay crate-private.
- **`src/model/mod.rs`** — public builders `add_indicator`, `add_reify`, `add_boolean`, `add_cardinality`, each returning `Result<Construct, ModelError>`, accepting an optional per-construct `FormulationPreference` (A29 single authority — stored on the `ConstructEntry`), validating the four invalid-input classes, and recording `Change::ConstructAdded` with the exact payload + preference. `add_reify` creates the reification result binary variable (a failed reify leaks no variable).
- **Validation rejections (SM-12.2/SM-12.5):** non-binary activators and Boolean/cardinality inputs → `ModelError::NonBinaryVariable(VarId)`; duplicate cardinality inputs → `ModelError::DuplicateCardinalityVariable(VarId)`; invalid `k` (negative, non-integral, or above the input length) → `ModelError::InvalidCardinalityK { k, reason }`; continuous exact reification without separation → `ModelError::ContinuousReificationWithoutSeparation`; non-finite/non-positive separation → `ModelError::InvalidReificationSeparation(f64)`.
- **`src/compiler/origin.rs`** — `GeneratedRole` per-construct role variants: `IndicatorImplicationRow`, `IndicatorNative`, `ReificationImplicationRow`, `ReificationComplement`, `BooleanImplicationRow`, `BooleanEquivalenceRow`, `BooleanAnyRow`, `BooleanAllRow`, `CardinalityRow` (`#[non_exhaustive]` stays).
- **`src/compiler/capability.rs`** — `FeatureSupport::bridge(limitations)` (SM-04.2); `BackendCapabilitySet::is_bridge`; additive `BackendFeature::{Reification, Boolean, Cardinality}` variants.
- **`src/compiler/bounds.rs`** — `bound_big_m_implied_snapshot` crate convenience returning the finite M plus the `BoundTrace` provenance sources (SM-13.5 evidence).
- **`src/compiler/bridge/mod.rs`** — `BridgeContext` (construct, snapshot, user→compiled variable map, evaluated parameters, effective policy, capabilities), `ConstructPath`, `select_path` (design §8.1), `function_coefficients`, `resolve_variable`, `combine_coefficients`, `eval_bound`.
- **`src/compiler/bridge/{indicator,reification,boolean,cardinality}.rs` (create)** — the four exact bridges:
  - indicator: qualified native `BackendFeature::Indicator` selection (Auto) or the exact finite-bound one-way Big-M rows; M derived via `BoundAnalyzer` (never a default constant — D12); missing finite M → `CompileError::UnboundedBigM`.
  - reification: exactly two implication rows (forward + complement); unit gap only from proven integrality; explicit separation tolerance honored (D14).
  - boolean: exact linear rows for implication/equivalence/any/all (no auxiliaries).
  - cardinality: exact linear row for exactly/at-most/at-least-k (no auxiliaries).
- **`src/compiler/session.rs`** — `compile_snapshot` iterates active constructs in the snapshot's deterministic construct-id order, dispatches each to the matching bridge under the effective policy (global narrowed by the per-construct `FormulationPreference`), merges generated variables/rows/origins/decisions into the compiled snapshot, and surfaces typed errors on unbounded (`UnboundedBigM`) / unsupported (`UnsupportedFeature`) / dangling-reference (`MissingConstructReference`).
- **`src/compiler/backend_ir.rs`** — `BackendSnapshotBuilder::add_formulation_decisions`; **`src/compiler/report.rs`** — `CompilationReport::new` appends the extra decisions after the objective-policy decision.
- **`src/compiler/mod.rs`** — `CompileError::MissingConstructReference { construct, variable }` (design §19 family — a construct referencing a removed variable is a typed error, never a dropped coefficient).
- **`src/advanced.rs`** — re-exports the four payload types + kind enums + `ConstructEntry`/`ConstructKind` for framework/backend authors.
- **`roml-highs/src/session.rs`** — `highs_capability_set` declares `BackendFeature::{Indicator, Reification, Boolean, Cardinality}` as `SupportLevel::Bridge` (exact ROML formulations, P32's first bridge declarations — SM-04.2) with NO qualified native claim (SM-04.3).
- **Tests** — `tests/common_constructs.rs` (28 integration tests) and `roml-highs/tests/formulation_equivalence.rs` (3 HiGHS tests) plus one in-crate bridge-declaration test in `roml-highs/src/session.rs`.

### Per-construct evidence (TRACEABILITY.md)

| Construct | Semantic definition | Accepted domain | Rejected domain | Native | Bridge | Origin roles | Reference formulation |
|---|---|---|---|---|---|---|---|
| Indicator | `WhenOne`: `z=1 ⇒ f∈S`; `WhenZero`: `z=0 ⇒ f∈S` (one-way) | binary activator; le/ge/eq/interval sets | continuous/integer activator | qualified `BackendFeature::Indicator` (Auto) | exact finite-bound one-way Big-M row(s) | `IndicatorNative` / `IndicatorImplicationRow` | `f + M z ≤ rhs + M` / `f − M z ≥ rhs − M` (M from bounds) |
| Reification | `b=1 ⟺ f∈S` (two implications) | binary activator (created by builder); le/ge thresholds | eq/interval sets (typed build-time rejection); continuous without separation | none (always bridge) | forward + complement rows | `ReificationImplicationRow`, `ReificationComplement` | `f + M1 b ≤ rhs + M1`; `f + M2 b ≥ rhs + sep` (unit gap iff proven integral) |
| Boolean | implication / equivalence / any / all | binary vars | non-binary vars | none (always bridge) | exact linear rows | `Boolean{Implication,Equivalence,Any,All}Row` | `a − b ≤ 0`; `a − b ≤ 0 ∧ b − a ≤ 0`; `Σ v ≥ 1`; `Σ v ≥ n` |
| Cardinality | exactly/at-most/at-least-k | binary vars, duplicate-free, `0 ≤ k ≤ len` | duplicates, non-binary, invalid `k` | none (always bridge) | exact linear row | `CardinalityRow` | `Σ v = k` / `≤ k` / `≥ k` |

Unsupported/unbounded failure behavior: missing finite M → `CompileError::UnboundedBigM { construct, expression }` (SM-13.4); `NativeRequired` without native → `CompileError::UnsupportedFeature`; no native and no bridge under `Auto` → `CompileError::UnsupportedFeature`; dangling construct reference → `CompileError::MissingConstructReference`. No silent relaxation exists (D13/D12). Randomized solver equivalence is covered by the small-binary-domain feasible-set enumeration (semantic/reference/native/portable equality) in `tests/common_constructs.rs` and by the HiGHS `roml-highs/tests/formulation_equivalence.rs` suites.

### Focused verification

| Command | Result |
|---|---|
| `cargo test -p roml --test common_constructs indicator` | 0 — **9 passed; 0 failed** |
| `cargo test -p roml-highs --test formulation_equivalence indicator` | 0 — **1 passed; 0 failed** |

### Full verification

| Command | Exit | Result |
|---|---|---|
| `cargo fmt --all -- --check` | 0 | formatting clean |
| `cargo test -p roml --test common_constructs` | 0 | **28 passed; 0 failed** |
| `cargo test -p roml --all-targets` | 0 | **734 passed; 0 failed; 0 ignored** (baseline 706 + 28 new) |
| `cargo test -p roml-highs --all-targets` | 0 | **118 passed; 0 failed** (baseline 114 + 4 new) |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 | docs generated, no warnings |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` | 0 | docs generated, no warnings |
| `cargo public-api -p roml` | 0 | **15983 → 17536 items** (+1553 additive) |

### Public API diff (A30)

`cargo public-api -p roml` grew by **1553 items** (15123 → 15983 at Task 15 → 17536 at Task 16). Additions include:

- `roml::construct::{ConstructKind, ConstructEntry, IndicatorConstraint, ReificationConstraint, BooleanConstraint, CardinalityConstraint, IndicatorDirection, BooleanKind, CardinalityKind}` and their inherent/derived impls.
- `roml::Model::add_indicator`, `add_reify`, `add_boolean`, `add_cardinality`.
- `roml::model::ModelError::{NonBinaryVariable, DuplicateCardinalityVariable, InvalidCardinalityK, ContinuousReificationWithoutSeparation, InvalidReificationSeparation, UnsupportedReificationSet, EmptyConstructInput}`.
- `roml::compiler::capability::{FeatureSupport::bridge, BackendCapabilitySet::is_bridge, BackendFeature::{Reification, Boolean, Cardinality}}`.
- `roml::compiler::origin::GeneratedRole` per-construct variants.
- `roml::compiler::CompileError::MissingConstructReference`.
- `roml::advanced` re-exports.

**MUST-NOT items verified absent:** `Fixture`, `FixturePayload`, `add_construct_fixture`, `ConstructData`, and the `ConstructKind::Fixture` variant do NOT appear in the `cargo public-api -p roml` output (grep count 0).

### Deviations

1. **`ReificationConstraint.activator` field added.** The plan's payload description lists "scalar-function-in-set relation, separation tolerance, proven-integrality flag" but a reification construct cannot compile without referencing its binary result variable. Per design §16.2 (`reify` creates and returns the indicator variable), `add_reify` creates a fresh binary variable and stores it in the payload as `activator`. The builder returns the `Construct` handle (per the plan's interface contract); the binary variable is reachable through the payload in canonical snapshots.
2. **`add_cardinality` takes `k: f64` (validated), stored as `usize`.** The plan requires typed rejections for negative, non-integral, AND above-length `k`. A `usize` parameter would make negative/non-integral `k` unrepresentable, so the builder takes `f64`, validates (finite, non-negative, integral, ≤ length), and stores `k: usize` in the payload.
3. **Native indicator selection emits the exact finite-bound row (role `IndicatorNative` + decision).** The P26 backend IR has no native-constraint representation (`BackendConstraint` is an empty enum and `BackendSnapshot.native_constraints` is always empty), and the plan forbids a compiler-contract amendment without review. So a qualified native `BackendFeature::Indicator` selection is recorded as a `FormulationDecision` (`indicator.representation = "native indicator"`) and the emitted exact row carries the `IndicatorNative` role; the emitted IR is the exact one-way row. A true native-constraint emission would require a `BackendConstraint` variant (documented; none of the P32 backends declare native Indicator, so the gap is unreachable in practice).
4. **Reification of equality/interval relations is a typed build-time rejection.** The P32 two-implication reification contract covers `le`/`ge` thresholds; equality/interval reification needs a disjunctive complement (an auxiliary binary), which lands beyond P32's scope. `add_reify` returns `ModelError::UnsupportedReificationSet`.
5. **`BackendFeature::{Reification, Boolean, Cardinality}` added.** The design §8.2 capability enumeration lists only `Indicator` among the logical constructs, but the plan requires HiGHS to declare "the logical-construct features" as `SupportLevel::Bridge`. The three additive variants (the enum is `#[non_exhaustive]`) give each construct a distinct feature for capability gating / `NativeRequired` rejection.
6. **`ConstructData` made `pub(crate)` (A30).** The plan's crate-private list names `FixturePayload`, `ConstructData`, `add_construct_fixture`; making the whole module public surfaced `ConstructData`, so it was tightened to `pub(crate)` to satisfy A30's "absent from public-api" requirement.

### Commit trail

- `feat(model): add logical semantic constructs` — Task 16 implementation + tests + evidence (single coherent unit).

---

## Plan 02 — Task 17a: Add exact min/max and one-sided epigraph/hypograph relations

**Phase:** P32 (Plan 02)  **Requirements:** SM-12.3, SM-12.8; exercises SM-13.2/13.4/13.5
**Status:** complete — committed as `feat(model): add exact min/max and one-sided relations`.

### Scope

Task 17a (the algebraic-construct tracer) lands the min/max construct family: `MinMaxConstraint` with `MinMaxSense { Min, Max }` and `MinMaxRelation { Exact, Epigraph, Hypograph }`, the `Model::add_minmax` builder (stable `Construct` + output-variable handle, SM-12.8), the zero-binary max-epigraph / min-hypograph rows, the bounded exact selector bridge with finite derived M values, and the construct-aware `UnboundedBigM` for unbounded exact operands. Clause-level scope statement: SM-12.1/12.2/12.5 (logical constructs) stay closed by Plan 01 Task 16; SM-12.4/12.6/12.7 land in Tasks 17b/17c; SM-13 full closure remains P33 (this task exercises SM-13.2/13.4/13.5 only).

### TDD — RED failures (recorded before implementation)

The new min/max tests in `tests/common_constructs.rs` referenced `roml::construct::{MinMaxSense, MinMaxRelation}` and `Model::add_minmax`, none of which existed on the Task 16 tree. Expected failure, recorded verbatim:

```text
error[E0433]: failed to resolve: could not find `MinMaxSense` in `construct`
  --> tests/common_constructs.rs:22:5
   |
22 |     MinMaxRelation, MinMaxSense,
   |                       ^^^^^^^^^^^ could not find `MinMaxSense` in `construct`
```

`roml-highs/tests/formulation_equivalence.rs` likewise failed to compile (missing `add_minmax`).

### Implementation

- **`src/construct/minmax.rs` (create)** — `MinMaxConstraint { operands: Vec<LinExpr>, output: VarId, sense, relation }` with `MinMaxSense { Min, Max }` and `MinMaxRelation { Exact, Epigraph, Hypograph }` (design §16.3 identifiers). The output variable is created by the builder and stored in the payload (top-level construct origin — the output is the construct's canonical result). `parameter_dependencies` collects every operand's parameter deps (F1).
- **`src/model/mod.rs`** — `add_minmax(operands, sense, relation, preference)` returns `(Construct, VarId)` (SM-12.8); rejects `< 2` operands (`MinMaxTooFewOperands`), trivially-satisfiable `Min`+`Epigraph` / `Max`+`Hypograph` (`TriviallySatisfiableMinMax`), and non-finite/stale operands (via `validate_expression_entities`). Output-variable bounds reflect the relation: exact → `[l_min, u_max]`, max-epigraph → `[l_min, +inf)`, min-hypograph → `(-inf, u_max]`, using `BoundAnalyzer` intervals over the model's declared bounds.
- **`src/compiler/bridge/minmax.rs` (create)** — the exact bridge (design §16.3, §8.5):
  - max epigraph: rows `x_i <= y` per operand, zero binaries, `MinMaxEpigraphRow` roles;
  - min hypograph: rows `x_i >= y` per operand, zero binaries, `MinMaxHypographRow` roles;
  - exact max: `y >= x_i`, binary `z_i` per operand with `sum z = 1`, `y <= x_i + M_i(1-z_i)` with finite derived `M_i = u_max - l_i` (`u_max = max_j u_j`);
  - exact min: the mirror selector with `M_i = u_i - l_min` (`l_min = min_j l_j`);
  - unbounded exact operand → `CompileError::UnboundedBigM { construct, expression }` naming the construct and the operand expression (SM-13.4, D12) — never a silent default constant;
  - M values, derivations, and bound sources recorded as `minmax.selector_m.*` bound-evidence report entries (SM-13.5).
  - The `exact_max_selector`/`exact_min_selector` helpers (with a resolved-`Operand` shape) are shared with the Task 17b clamp bridge.
- **`src/compiler/origin.rs`** — `GeneratedRole::{MinMaxEpigraphRow, MinMaxHypographRow, MinMaxSelectorRow, MinMaxSelectorBinary}` (`#[non_exhaustive]` stays).
- **`src/compiler/capability.rs`** — additive `BackendFeature::MinMax` (P32 Task 17a bridge-supported).
- **`src/compiler/session.rs`** — `ConstructKind::MinMax` dispatched through the Plan 01 `BridgeFinalizer` in deterministic construct-id order; `AbsoluteValue`/`BinaryProduct` arms return a typed `UnsupportedFeature` (bridges land in Tasks 17b/17c).
- **`roml-highs/src/session.rs`** — `BackendFeature::MinMax` declared `SupportLevel::Bridge` (no native claim, SM-04.3); the bridge-declaration test iterates the array so it auto-covers the new feature.
- **`src/lib.rs` / `src/advanced.rs`** — A30-pattern re-exports of `MinMaxConstraint`, `MinMaxSense`, `MinMaxRelation`.

### Per-construct evidence (TRACEABILITY.md)

| Construct | Semantic definition | Accepted domain | Rejected domain | Native | Bridge | Origin roles | Reference formulation |
|---|---|---|---|---|---|---|---|
| MinMax (exact max) | `y = max(x_1..x_n)` | ≥2 finite linear operands, all bounded | unbounded exact operands (`UnboundedBigM`); <2 operands; trivially-satisfiable sense/relation | none (P32) | bounded selector (binaries + finite derived M) | `MinMaxSelectorRow`, `MinMaxSelectorBinary` | `y ≥ x_i`; `Σz_i = 1`; `y ≤ x_i + M_i(1−z_i)`, `M_i = u_max − l_i` |
| MinMax (exact min) | `y = min(x_1..x_n)` | ≥2 finite linear operands, all bounded | unbounded exact operands | none | bounded selector | `MinMaxSelectorRow`, `MinMaxSelectorBinary` | `y ≤ x_i`; `Σz_i = 1`; `y ≥ x_i − M_i(1−z_i)`, `M_i = u_i − l_min` |
| MinMax (max epigraph) | `y ≥ max(x_1..x_n)` | ≥2 finite linear operands | trivially-satisfiable `Min`+`Epigraph` | none | zero-binary rows | `MinMaxEpigraphRow` | `x_i ≤ y` (all i) |
| MinMax (min hypograph) | `y ≤ min(x_1..x_n)` | ≥2 finite linear operands | trivially-satisfiable `Max`+`Hypograph` | none | zero-binary rows | `MinMaxHypographRow` | `x_i ≥ y` (all i) |

Unsupported/unbounded failure behavior: unbounded exact operand → `CompileError::UnboundedBigM { construct, expression }` (SM-13.4); `NativeRequired` without native min/max → `CompileError::UnsupportedFeature` (via `select_path`). The D13 difference proof (exact vs one-sided feasible sets differ with NO objective) and the fixed-seed randomized direct-evaluation tests are in `tests/common_constructs.rs`; the reference-vs-portable HiGHS equivalence is in `roml-highs/tests/formulation_equivalence.rs`.

### Focused verification

| Command | Result |
|---|---|
| `cargo test -p roml --test common_constructs minmax` | 0 — **8 passed; 0 failed** |
| `cargo test -p roml --test compiler_bridges` | 0 — **15 passed; 0 failed** |
| `cargo test -p roml-highs --test formulation_equivalence minmax` | 0 — **1 passed; 0 failed** |

### Full verification

| Command | Exit | Result |
|---|---|---|
| `cargo fmt --all -- --check` | 0 | formatting clean |
| `cargo test -p roml --all-targets` | 0 | **742 passed; 0 failed; 0 ignored** (baseline 734 + 8 new) |
| `cargo test -p roml-highs --all-targets` | 0 | **119 passed; 0 failed** (baseline 118 + 1 new) |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 | docs generated, no warnings |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` | 0 | docs generated, no warnings |
| `cargo public-api -p roml` | 0 | **17536 → 18783 items** (+1247 additive) |

### Acceptance criteria

- `src/construct/minmax.rs` defines `MinMaxConstraint` with `MinMaxSense`/`MinMaxRelation`; `ConstructKind::MinMax` carries it and stays `#[non_exhaustive]`; `derive_parameter_dependencies` covers operand parameters — **met**.
- `Model::add_minmax` returns a stable `Construct` handle plus the output-variable handle (SM-12.8) and rejects `<2` operands, `Min`+`Epigraph`, `Max`+`Hypograph`, and non-finite operands with typed `ModelError`s — **met**.
- Exact min/max compile to bounded selector formulations (binary per operand, sum=1, finite derived M) with report entries recording M values, derivations, and bound sources (SM-13.5); no-binary max epigraph and min hypograph rows compile with zero binaries and the distinct `MinMaxEpigraphRow`/`MinMaxHypographRow` origins — **met**.
- Unbounded exact operands → `CompileError::UnboundedBigM { construct, expression }` (SM-13.4, D12) — **met**.
- `tests/common_constructs.rs` proves exact and one-sided feasible sets differ with no objective (D13, SM-12.3); `roml-highs/tests/formulation_equivalence.rs` proves reference-vs-portable feasible-set equality on HiGHS — **met**.

### Deviations

1. **Exact-selector rows use negated operand coefficients.** The first implementation added the operand coefficients directly (`y + x_i`); the four exact-selector row types are `y − x_i` (and `y − x_i ± M_i·z_i`), so a `negate_coefficients` helper flips the operand signs. Caught by the exact-selector feasible-set enumeration and randomized direct-evaluation tests — a Rule 1 fix within the task.
2. **One-sided output-variable bounds are relation-specific.** The initial builder gave the output `[l_min, u_max]` for every relation, which made the min-hypograph output bounded below and broke the D13 proof (y=0 not admitted). The builder now sets exact → `[l_min, u_max]`, max-epigraph → `[l_min, +inf)`, min-hypograph → `(-inf, u_max]` — a Rule 1 fix.
3. **`BackendFeature::MinMax` added and HiGHS bridge-declared** (additive, `#[non_exhaustive]`), matching the Plan 01 logical-construct feature pattern, so `select_path` can gate the min/max bridge and `NativeRequired` rejects cleanly.
4. **`AbsoluteValue`/`BinaryProduct` dispatch arms return `UnsupportedFeature` at this commit** (their bridges land in Tasks 17b/17c) — the `ConstructKind` match must be exhaustive, and a typed error is the honest intermediate behavior.

### Commit trail

- `feat(model): add exact min/max and one-sided relations` — Task 17a implementation + tests + evidence.

---

## Plan 02 — Task 17b: Add exact absolute value, positive part, and clamp

**Phase:** P32 (Plan 02)  **Requirements:** SM-12.4, SM-12.8; exercises SM-13.2/13.3/13.4/13.5
**Status:** complete — committed as `feat(model): add exact absolute value, positive part, and clamp`.

### Scope

Task 17b lands `AbsoluteValueConstraint` with `AbsoluteValueVariant { Absolute, PositivePart, Clamp { lower, upper } }`, the `Model::add_absolute_value` builder (stable `Construct` + output-variable handle, SM-12.8), and the bounded exact bridges: the abs/positive-part decomposition (`z = p + n`, `p - n = x`, `p, n >= 0`, selector binary, `M_p = max(U, 0)`, `M_n = max(-L, 0)`) and the composed clamp (inner exact max `w = max(x, lo)` then outer exact min `z = min(w, hi)`, reusing the Task 17a selector helpers). Every generated entity carries a top-level `EntityOrigin::Construct { construct, role }`; no one-sided relaxation (D13) and no reification/strict-inequality row (D14) is emitted. Clause-level scope: SM-12.1/12.2/12.5 stay closed by Plan 01; SM-12.3 by Task 17a; SM-12.6/12.7 by Task 17c; SM-13 full closure remains P33.

### TDD — RED failures (recorded before implementation)

The new abs tests referenced `roml::construct::AbsoluteValueVariant` and `Model::add_absolute_value` against the Task 17a tree — the bridge was not wired (`ConstructKind::AbsoluteValue` returned `UnsupportedFeature`). Expected failure, recorded verbatim:

```text
---- absolute_value_exact_feasible_set_matches_semantic stdout ----
thread 'absolute_value_exact_feasible_set_matches_semantic' panicked:
snapshot compilation must fail: UnsupportedFeature("absolute value bridge not yet implemented (P32 Task 17b)")
```

### Implementation

- **`src/construct/absolute.rs` (created in Task 17a)** — `AbsoluteValueConstraint { expression, output, variant }`; `AbsoluteValueVariant { Absolute, PositivePart, Clamp { lower, upper } }`; the output variable is created by the builder and stored in the payload (top-level construct origin).
- **`src/model/mod.rs` (builder added in Task 17a)** — `add_absolute_value(expression, variant, preference)` returns `(Construct, VarId)`; validates the expression is bounded (`UnboundedConstructExpression` typed rejection) and clamp bounds are finite `lower <= upper` (`InvalidClampBounds`); output bounds `[0, max(U, -L)]` (abs), `[0, max(U, 0)]` (positive part), `[lo, hi]` (clamp).
- **`src/compiler/bridge/absolute.rs` (create)** — the bounded exact bridge:
  - absolute: `z = p + n`, `p - n = x`, `p, n >= 0`, binary `b`, `p <= M_p·b`, `n <= M_n·(1-b)` with `M_p = max(U, 0)`, `M_n = max(-L, 0)` — `AbsoluteValueDecompositionRow` / `AbsoluteValuePositivePartRow` / `AbsoluteValueNegativePartRow` / `AbsoluteValueSelectorBinary` roles;
  - positive part: `z - n = x`, `z <= M_p·b`, `n <= M_n·(1-b)` (z wired to the positive part);
  - clamp: inner `exact_max_selector` on `{x, lo}` → generated `w`, then outer `exact_min_selector` on `{w, hi}` → `z`, with `ClampInnerSelector*` / `ClampOuterSelector*` roles; all M finite derived from `[L, U]` and the constants;
  - unbounded expression → `CompileError::UnboundedBigM { construct, expression }` (SM-13.4, D12); `M_p`/`M_n` and the clamp selector M values recorded as bound-evidence report entries (SM-13.5).
- **`src/compiler/bridge/minmax.rs`** — the `exact_max_selector`/`exact_min_selector` helpers now emit the COMPLETE selector (base `output <=/>= x_i` rows, sum-binary row, and the Big-M rows) so the clamp bridge can call them directly; the minmax `compile` arms were adjusted to call the helpers (which now own the base rows). This was a Rule 1 bug fix: the clamp's first emission omitted the base rows, making `z = min(w, hi)` unbounded above (caught by the feasible-set enumeration).
- **`src/compiler/session.rs`** — `ConstructKind::AbsoluteValue` now dispatches to the absolute bridge (the `UnsupportedFeature` placeholder is gone).
- **`src/compiler/bridge/mod.rs`** — `pub(crate) mod absolute;`.

### Per-construct evidence (TRACEABILITY.md)

| Construct | Semantic definition | Accepted domain | Rejected domain | Native | Bridge | Origin roles | Reference formulation |
|---|---|---|---|---|---|---|---|
| Absolute value | `z = \|x\|` | bounded linear expr | unbounded expr (`UnboundedBigM` / builder rejection) | none (P32) | exact decomposition | `AbsoluteValueDecompositionRow`, `AbsoluteValuePositivePartRow`, `AbsoluteValueNegativePartRow`, `AbsoluteValueSelectorBinary` | `z = p + n`, `p − n = x`, `p,n ≥ 0`, `p ≤ M_p·b`, `n ≤ M_n·(1−b)`, `M_p = max(U,0)`, `M_n = max(−L,0)` |
| Positive part | `z = max(x, 0)` | bounded linear expr | unbounded expr | none | exact decomposition | same family | `z − n = x`, `z ≤ M_p·b`, `n ≤ M_n·(1−b)` |
| Clamp | `z = clamp(x, lo, hi)` | bounded linear expr, finite `lo ≤ hi` | invalid clamp bounds | none | composed selectors | `ClampInnerSelectorRow/Binary`, `ClampOuterSelectorRow/Binary` | inner `w = max(x, lo)` selector, outer `z = min(w, hi)` selector |

Unsupported/unbounded failure behavior: unbounded expression → `CompileError::UnboundedBigM { construct, expression }` (SM-13.4); builder-level boundedness/`InvalidClampBounds` rejections. No one-sided relaxation and no reification row is emitted (D13/D14). Randomized direct evaluation and the feasible-set enumeration (z = |x|, z = max(x,0), z = clamp(x,lo,hi) across x below/inside/above) are in `tests/common_constructs.rs`; the HiGHS reference-vs-portable equivalence is in `roml-highs/tests/formulation_equivalence.rs`.

### Focused verification

| Command | Result |
|---|---|
| `cargo test -p roml --test common_constructs absolute` | 0 — **7 passed; 0 failed** |
| `cargo test -p roml --test compiler_bridges` | 0 — **15 passed; 0 failed** |
| `cargo test -p roml-highs --test formulation_equivalence absolute` | 0 — **1 passed; 0 failed** |

### Full verification

| Command | Exit | Result |
|---|---|---|
| `cargo fmt --all -- --check` | 0 | formatting clean |
| `cargo test -p roml --all-targets` | 0 | **751 passed; 0 failed; 0 ignored** (baseline 742 + 9 new) |
| `cargo test -p roml-highs --all-targets` | 0 | **120 passed; 0 failed** (baseline 119 + 1 new) |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 | docs generated, no warnings |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` | 0 | docs generated, no warnings |
| `cargo public-api -p roml` | 0 | **18783 items** (unchanged — the abs public surface landed with Task 17a) |

### Acceptance criteria

- `src/construct/absolute.rs` defines `AbsoluteValueConstraint` with `AbsoluteValueVariant { Absolute, PositivePart, Clamp { lower, upper } }`; `ConstructKind::AbsoluteValue` carries it; `derive_parameter_dependencies` covers expression parameters — **met**.
- `Model::add_absolute_value` returns a stable `Construct` handle plus the output-variable handle (SM-12.8) and rejects unbounded expressions and invalid clamp bounds with typed `ModelError`s (SM-12.4) — **met**.
- Abs/positive-part compile to the exact bounded decomposition; clamp compiles to the composed exact max/min selectors; all M finite derived and recorded with bound sources (SM-13.5); unbounded expression → `CompileError::UnboundedBigM { construct, expression }` (SM-13.4, D12) — **met**.
- Every generated entity carries `EntityOrigin::Construct { construct, role }` (SM-02.5) — **met** (`absolute_value_every_generated_entity_carries_construct_origin` + the completeness validator).
- Feasible-set enumeration and HiGHS reference-vs-portable equivalence prove z = |x|, z = max(x,0), z = clamp(x,lo,hi) exactly — **met**.

### Deviations

1. **The selector helpers now emit the complete selector (Rule 1 bug fix).** The first Task 17b implementation called `exact_max_selector`/`exact_min_selector` for the clamp but those helpers only emitted the sum-binary and Big-M rows — the minmax `compile` function had been emitting the base `y >= x_i`/`y <= x_i` rows itself. The clamp's direct helper calls therefore omitted the base rows, so `z = min(w, hi)` was unbounded above. Fixed by moving the base-row emission INTO the helpers and removing the now-duplicate loops from the minmax `compile` arms (caught by the clamp feasible-set enumeration). This also corrected the Task 17a min/max emission to the exact same row set (behavior unchanged, verified by re-running the Task 17a min/max suite).
2. **Clippy `useless_conversion` in a test** (`(p * x).into()` — `p * x` is already `LinExpr`) fixed inline.
3. **`BackendFeature::AbsoluteValue` already declared** in Task 17a (the feature gate and HiGHS bridge declaration were additive then); Task 17b only wires the bridge body behind it.

### Commit trail

- `feat(model): add exact absolute value, positive part, and clamp` — Task 17b implementation + tests + evidence.

---

## Plan 02 — Task 17c: Add exact binary products and reject continuous-times-continuous

**Phase:** P32 (Plan 02)  **Requirements:** SM-12.6, SM-12.7, SM-12.8; exercises SM-13.2/13.4/13.5
**Status:** complete — committed as `feat(model): add algebraic semantic constructs` (the packet Task 17 final commit).

### Scope

Task 17c lands `BinaryProductConstraint` with `ProductOperand { Binary(VarId), Linear(LinExpr) }`, the `Model::add_binary_product` / `Model::add_binary_times_linear` builders (stable `Construct` + output-variable handle, SM-12.8), and the exact product bridge: binary-binary (`w <= a`, `w <= b`, `w >= a + b - 1`, `0 <= w <= 1`) and binary-times-bounded-linear (`w >= L·b`, `w <= U·b`, `w >= f - U·(1-b)`, `w <= f - L·(1-b)`) with finite derived M from the interval. Continuous×continuous exact requests are rejected with a typed `ModelError` (SM-12.7, D23) and produce no compiled entities and no relaxation mislabeled exact. Clause-level scope: SM-12.3/12.4 closed by Tasks 17a/17b; SM-13 full closure remains P33 (this task exercises SM-13.2/13.4/13.5 only).

### TDD — RED failures (recorded before implementation)

The new product tests referenced `Model::add_binary_product`/`ProductOperand` against the Task 17b tree — the bridge was not wired (`ConstructKind::BinaryProduct` returned `UnsupportedFeature`). Expected failure, recorded verbatim:

```text
---- binary_binary_exact_feasible_set_matches_semantic stdout ----
thread 'binary_binary_exact_feasible_set_matches_semantic' panicked:
snapshot compilation must fail: UnsupportedFeature("binary product bridge not yet implemented (P32 Task 17c)")
```

### Implementation

- **`src/construct/product.rs` (created in Task 17a)** — `BinaryProductConstraint { left, right, output }`; `ProductOperand { Binary(VarId), Linear(LinExpr) }`; the output variable is created by the builder and stored in the payload (top-level construct origin).
- **`src/model/mod.rs` (builders added in Task 17a)** — `add_binary_product(left, right, preference)` and `add_binary_times_linear(binary, expression, preference)` each return `(Construct, VarId)`; validate exactly one binary operand (Linear×Linear → `ContinuousTimesContinuousProduct`, SM-12.7) and that each `Binary` operand is a true binary variable (`NonBinaryVariable`, SM-12.6); output bounds `[0,1]` (binary-binary) or `[min(0,L), max(0,U)]` (binary-times-linear).
- **`src/compiler/bridge/product.rs` (create)** — the exact bridge:
  - binary-binary: rows `w <= a`, `w <= b`, `w >= a + b - 1` (`BinaryProductRow`), output bounds `0 <= w <= 1` — no generated binaries, no Big-M;
  - binary-times-bounded-linear: rows `w >= L·b`, `w <= U·b` (`BinaryProductBoundRow`) and `w >= f - U·(1-b)`, `w <= f - L·(1-b)` (`BinaryProductLinearRow`), each M the finite derived interval endpoint (SM-13.2), recorded as `product.m_lower`/`product.m_upper` bound-evidence report entries with sources (SM-13.5); unbounded `f` → `CompileError::UnboundedBigM { construct, expression }` (SM-13.4, D12);
  - continuous×continuous: unreachable (builder rejects); the bridge returns a typed `UnsupportedFeature` defensively — no exact MILP path and no relaxation is emitted (SM-12.7, D23).
- **`src/compiler/session.rs`** — `ConstructKind::BinaryProduct` now dispatches to the product bridge (the `UnsupportedFeature` placeholder is gone).
- **`src/compiler/bridge/mod.rs`** — `pub(crate) mod product;`.

### Per-construct evidence (TRACEABILITY.md)

| Construct | Semantic definition | Accepted domain | Rejected domain | Native | Bridge | Origin roles | Reference formulation |
|---|---|---|---|---|---|---|---|
| Binary product (binary-binary) | `w = a·b` | two binary operands | non-binary operands | none (P32) | exact rows | `BinaryProductRow` | `w ≤ a`, `w ≤ b`, `w ≥ a + b − 1`, `0 ≤ w ≤ 1` |
| Binary product (binary × linear) | `w = b·f` | one binary + one bounded linear operand | unbounded `f` (`UnboundedBigM`); non-binary operand | none | exact rows | `BinaryProductBoundRow`, `BinaryProductLinearRow` | `w ≥ L·b`, `w ≤ U·b`, `w ≥ f − U·(1−b)`, `w ≤ f − L·(1−b)` |
| Binary product (continuous × continuous) | — (not exposed as exact) | — | typed `ContinuousTimesContinuousProduct` (SM-12.7, D23) | none | none — no compiled entities | — | — |

Unsupported/unbounded failure behavior: unbounded linear operand → `CompileError::UnboundedBigM { construct, expression }` (SM-13.4); continuous×continuous and non-binary operands → typed `ModelError`s with no compiled entities and no relaxation labeled exact (SM-12.7, D23). Feasible-set enumeration (binary-binary and binary-times-linear) and fixed-seed randomized direct evaluation are in `tests/common_constructs.rs`; the HiGHS reference-vs-portable equivalence is in `roml-highs/tests/formulation_equivalence.rs`.

### Focused verification

| Command | Result |
|---|---|
| `cargo test -p roml --test common_constructs product` | 0 — **8 passed; 0 failed** |
| `cargo test -p roml --test compiler_bridges` | 0 — **15 passed; 0 failed** |
| `cargo test -p roml-highs --test formulation_equivalence product` | 0 — **1 passed; 0 failed** |

### Full verification

| Command | Exit | Result |
|---|---|---|
| `cargo fmt --all -- --check` | 0 | formatting clean |
| `cargo test -p roml --all-targets` | 0 | **759 passed; 0 failed; 0 ignored** (baseline 751 + 8 new) |
| `cargo test -p roml-highs --all-targets` | 0 | **121 passed; 0 failed** (baseline 120 + 1 new) |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 | docs generated, no warnings |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` | 0 | docs generated, no warnings |
| `cargo public-api -p roml` | 0 | **18783 items** (unchanged — the product public surface landed with Task 17a) |

### Acceptance criteria

- `src/construct/product.rs` defines `BinaryProductConstraint` with `ProductOperand`; `ConstructKind::BinaryProduct` carries it; `derive_parameter_dependencies` covers linear-operand parameters — **met**.
- `Model` exposes the binary-product builders returning stable `Construct` + output-variable handles (SM-12.8); continuous×continuous and non-binary operands are rejected with typed `ModelError`s and produce no compiled entities and no relaxation mislabeled exact (SM-12.6, SM-12.7, D23) — **met**.
- Binary-binary compiles to the four exact rows; binary-times-bounded-linear compiles to the four exact product rows with finite derived M from the interval and recorded bound sources (SM-13.5); unbounded `f` → `CompileError::UnboundedBigM { construct, expression }` (SM-13.4, D12) — **met**.
- Every generated entity carries `EntityOrigin::Construct { construct, role }` (SM-02.5) — **met**.
- Feasible-set enumeration and HiGHS reference-vs-portable equivalence prove w = a·b and w = b·f exactly; the rejected continuous×continuous request emits no rows — **met**.

### Deviations

1. **Test assertion fix (Rule 1).** The `binary_binary_exact_feasible_set_matches_semantic` test initially asserted `compiled.variables.is_empty()` — but the compiled snapshot always includes the user variables; the correct assertion counts only `Construct`-origin generated variables. Fixed to count generated variables only.
2. **`integer().bounds(0, 10)` test literals** were integers; the `Bounds::bounds` signature takes `f64` — fixed to `0.0`/`10.0`.
3. **`BackendFeature::BinaryProduct` already declared** in Task 17a (the feature gate and HiGHS bridge declaration were additive then); Task 17c only wires the bridge body behind it.

### Commit trail

- `feat(model): add algebraic semantic constructs` — Task 17c implementation + tests + evidence (the packet Task 17 final commit). **OR review is requested** per the packet and the plan's review-gate section.

---

## Reviewer findings (phase boundary review, standard depth)

1 critical + 5 warnings + 3 infos — all fixed with TDD:
- CR-01 reification inferred unit gap with a non-integral threshold silently excluded valid integer assignments — fixed `f3002a9` (build-time `ModelError::NonIntegralReificationThreshold`; fractional thresholds require explicit separation; feasible-set regression test).
- WR-01 Boolean/cardinality capability gates `2bb8dd5`; WR-02 Mip gate on generated selector binaries `ea742a6`; WR-03 set-threshold parameter deps `7d0d68e`; WR-04 `validated_explicit_big_m` fails closed `14b7d08`; WR-05 A30 fully met — `Fixture`/`FixturePayload`/`add_construct_fixture` `#[cfg(test)]`-gated, absent from `cargo public-api` (grep 0) `06b6465`.
- IN-01 native-vs-bridge observability `1f58475`; IN-02 `expression_interval` error causes `a957660`; IN-03 construct-add atomicity `0498794`.
- Post-review fix `2520c0b`: the cfg(test) gating left three intra-doc links to `FixturePayload` in non-test builds — reworded to code spans; `RUSTDOCFLAGS='-D warnings' cargo doc` both crates now exit 0 (the phase verifier caught this as a blocker; resolved and re-verified).

Re-verification: `cargo test -p roml --all-targets` **773 pass**, `roml-highs` **121 pass**, clippy `-D warnings` clean, rustdoc `-D warnings` clean, fmt clean. Public API: 18,819 items, fixture-free. 11/12 must-haves verified with the 12th (doc lane) resolved by `2520c0b`.

### Second review round — blocking independent review (PR #30, exactness across model evolution)

Five semantic findings + CI, verified and fixed with TDD:
1. **F1 bridge dependency graph persisted and enforced** (`67447aa`) — `BridgeOutput.dependencies` no longer dropped: persisted on `CurrentCompilation.construct_dependencies`, completed centrally from payloads; `SetParameter`/`SetVariableBounds`/`RemoveVariable` on a construct dependency return `RebuildRequired` before any compiled delta; rejected deltas never advance `CompilationId`; unrelated bounds changes stay incremental. 7 fixed-seed differential tests (incremental-after-mutation == fresh rebuild) for parameterized indicator/reification and bound-derived indicator/minmax/abs/product. P27 fixing/unfixing invalidation lands in the post-rebase pass (generic machinery in place).
2. **F2 construct output bounds decoupled from build-time intervals** (`4d46263`) — `add_minmax`/`add_absolute_value`/`add_binary_product` use static safe domains (UNBOUNDED for exact/epigraph/hypograph minmax, [0,1] binary-binary, UNBOUNDED binary×linear, clamp constants); exact bridge rows enforce the relationship from current intervals at compile time. 5 full-rebuild regressions for finite bound widening + parameter changes.
3. **F3 reification thresholds revalidated at every compilation** (`8ac092a`) — inferred-unit-gap integrality re-checked at compile (and after parameter invalidation); fractional threshold → typed `NonIntegralReificationThreshold` before backend mutation; integral→fractional→integral regressions.
4. **F4 bridge-only construct selection until native payloads exist** (`1b2fef2`) — `select_path` gates native on `native_payloads_available()` (false while `BackendConstraint` is empty); constructs selected/reported only as Bridge; `NativeRequired` rejects; reification now goes through select_path (centralized gating); no native labels on bridge rows. Per-family Auto/Portable/NativeRequired/unqualified tests + unbounded-indicator no-native-escape case.
5. **F5 malformed snapshots never zero-substitute missing parameters** (`d61ccf3`) — `MissingConstructParameter`/`MissingParameter`/`eval_checked`; bridge evaluation surfaces missing parameters as typed errors; `preflight_constructs` validates refs + finiteness before bridge generation; malformed-snapshot tests prove compilation fails before identity advancement.
6. **F6 CI fixed** (`87cdf13`) — the two `nonminimal_bool` expressions rewritten lint-clean (MSRV 1.85 lane).

Re-verification: `cargo test -p roml --all-targets` **798 pass**, `roml-highs` **121 pass**, clippy `-D warnings` clean. Evidence and PR body updated.
