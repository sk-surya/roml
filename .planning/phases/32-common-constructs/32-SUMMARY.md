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

---

# Phase [32] Task [16]: Add indicators, reification, Boolean, and cardinality

Logical semantic constructs (design §7, §16) with exact payloads, the four validation rejections, native-or-bridge compilation through the Task 15 framework, reification as two implications, exact Boolean/cardinality rows, small-binary-domain feasible-set equivalence (semantic/reference/native/portable), and the A30 public-surface activation (`ConstructKind`/`ConstructEntry` public; `Fixture`/`FixturePayload`/`add_construct_fixture` crate-private).

## What was built

- **`src/construct/{indicator,reification,boolean,cardinality}.rs`** — `IndicatorConstraint` (binary activator, one-way direction, function-in-set), `ReificationConstraint` (function-in-set, separation tolerance, proven-integrality, builder-created binary `activator`), `BooleanConstraint` (implication/equivalence/any/all), `CardinalityConstraint` (exactly/at-most/at-least-k).
- **`src/model/mod.rs`** — public builders `add_indicator`/`add_reify`/`add_boolean`/`add_cardinality` returning stable `Construct` handles with optional per-construct `FormulationPreference` (A29 single authority), recording `Change::ConstructAdded`; typed `ModelError` rejections for non-binary variables, duplicate cardinality inputs, invalid `k`, and continuous exact reification without separation.
- **A30 (lib.rs)** — `pub mod construct;` + crate-root re-exports; `Fixture`/`FixturePayload`/`add_construct_fixture`/`ConstructData` crate-private; `cargo public-api` diff recorded (15983 → 17536; fixture scaffolding absent).
- **`src/compiler/bridge/{indicator,reification,boolean,cardinality}.rs`** — exact bridges on the Task 15 framework: indicator native-or-finite-bound Big-M; reification two implications (unit gap iff proven integral, D14); boolean/cardinality exact rows.
- **`src/compiler/{origin,capability,bounds,session,backend_ir,report,mod}.rs`** — per-construct `GeneratedRole`s, `FeatureSupport::bridge` + `BackendCapabilitySet::is_bridge`, additive `BackendFeature::{Reification,Boolean,Cardinality}`, `bound_big_m_implied_snapshot`, construct dispatch in `compile_snapshot`, report decision plumbing, `CompileError::MissingConstructReference`.
- **`roml-highs/src/session.rs`** — logical-construct features declared `SupportLevel::Bridge` (P32's first bridge declarations, SM-04.2; no native claims, SM-04.3).
- **Tests** — `tests/common_constructs.rs` (28) + `roml-highs/tests/formulation_equivalence.rs` (3) + in-crate highs bridge-declaration test (1).

## Verification

| Command | Result |
|---|---|
| `cargo test -p roml --test common_constructs indicator` | 0 — 9 passed |
| `cargo test -p roml-highs --test formulation_equivalence indicator` | 0 — 1 passed |
| `cargo test -p roml --all-targets` | 0 — 734 passed; 0 failed (baseline 706 + 28) |
| `cargo test -p roml-highs --all-targets` | 0 — 118 passed; 0 failed (baseline 114 + 4) |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 — clean |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 — clean |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 — clean |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` | 0 — clean |
| `cargo public-api -p roml` | 0 — 15983 → 17536 items (+1553 additive); fixture scaffolding absent |

## Deviations from plan

1. `ReificationConstraint.activator` added (the reify builder creates the binary result variable; the construct cannot compile without referencing it).
2. `add_cardinality` takes `k: f64` (validated), stores `k: usize` — makes negative/non-integral `k` rejections testable (the plan's typed-error list).
3. Native indicator selection emits the exact finite-bound row with role `IndicatorNative` + a `FormulationDecision` (the P26 IR has no native-constraint representation; no compiler-contract amendment).
4. Reification of equality/interval relations is a typed build-time rejection (`UnsupportedReificationSet`) — the P32 two-implication contract covers le/ge thresholds.
5. `BackendFeature::{Reification, Boolean, Cardinality}` added (additive, `#[non_exhaustive]`) so HiGHS can declare each construct's bridge support.
6. `ConstructData` tightened to `pub(crate)` (A30 absent-from-public-api).

Full detail (RED failures, per-construct evidence table, verification matrix, A30 diff, deviations, commit trail) is in `docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md`.

## Commit trail (Task 16)

| # | SHA | Message |
|---|---|---|
| 3 | `5f4d972` | `feat(model): add logical semantic constructs` |
| 4 | `55f429d` | `docs(32): summarize Task 16 evidence and deviations` |

## Self-Check: PASSED

- Created files verified: all four payload modules, all four bridge modules, `tests/common_constructs.rs`, `roml-highs/tests/formulation_equivalence.rs`, and the evidence file are present in commit `5f4d972`.
- Commits verified: `5f4d972` (feat) and `55f429d` (docs) exist in the worktree history.
- Verification matrix: roml 734 tests / highs 118 tests / both clippy lanes clean / both doc lanes clean / fmt clean / `cargo public-api -p roml` 17536 items with `Fixture`/`FixturePayload`/`add_construct_fixture`/`ConstructData` absent.

---

# Phase [32] Plan [02]: Algebraic semantic constructs (packet Task 17)

Exact min/max with one-sided epigraph/hypograph relations, exact absolute value / positive part / clamp, and exact binary products (binary-binary and binary-times-bounded-linear), each as one semantic definition with complete top-level construct origins, a bounded exact portable formulation, and explicit failure when bounds/support are insufficient. Exactness is never inferred from objective context (D13): a proof test shows exact and one-sided feasible sets differ with no objective. Continuous×continuous product requests are typed rejections (SM-12.7, D23).

## What was built

- **`src/construct/{minmax,absolute,product}.rs`** — `MinMaxConstraint` (`MinMaxSense`, `MinMaxRelation`), `AbsoluteValueConstraint` (`AbsoluteValueVariant::{Absolute,PositivePart,Clamp}`), `BinaryProductConstraint` (`ProductOperand::{Binary,Linear}`). Each builder creates the output variable and stores it in the payload (top-level construct origin); `derive_parameter_dependencies` covers the operand/expression parameters.
- **`src/model/mod.rs`** — `add_minmax`, `add_absolute_value`, `add_binary_product`, `add_binary_times_linear` builders returning the stable `Construct` handle plus the output-variable handle (SM-12.8); typed rejections for `<2` minmax operands, trivially-satisfiable `Min`+`Epigraph`/`Max`+`Hypograph`, unbounded abs expressions, invalid clamp bounds, continuous×continuous products, and non-binary product operands.
- **`src/compiler/bridge/{minmax,absolute,product}.rs`** — the bounded exact bridges on the Plan 01 `BridgeFinalizer`:
  - minmax: zero-binary max-epigraph / min-hypograph rows; exact min/max bounded selector formulations with finite derived `M_i` and bound-source report entries (SM-13.5);
  - absolute: exact `z = p + n`, `p - n = x` decomposition with `M_p = max(U,0)`, `M_n = max(-L,0)`; composed clamp (inner max selector on `{x, lo}`, outer min selector on `{w, hi}`); no one-sided relaxation (D13), no reification row (D14);
  - product: binary-binary (`w <= a`, `w <= b`, `w >= a+b-1`) and binary-times-bounded-linear (`w >= L·b`, `w <= U·b`, `w >= f-U(1-b)`, `w <= f-L(1-b)`) exact rows; no continuous×continuous path (SM-12.7).
- **`src/compiler/{origin,session,capability}.rs`** — per-construct `GeneratedRole`s; `ConstructKind::{MinMax,AbsoluteValue,BinaryProduct}` dispatch through the bridge framework in deterministic construct-id order; additive `BackendFeature::{MinMax,AbsoluteValue,BinaryProduct}`.
- **`src/lib.rs` / `src/advanced.rs`** — A30-pattern re-exports of the three payload types and helper enums.
- **`roml-highs/src/session.rs`** — the three algebraic features declared `SupportLevel::Bridge` (no native claims, SM-04.3).
- **Tests** — `tests/common_constructs.rs` (17 new: D13 difference proof, zero-binary rows, exact-selector feasible-set enumeration, abs/positive-part/clamp enumeration, product enumeration, fixed-seed randomized direct evaluation, `UnboundedBigM` / typed-rejection tests) + `roml-highs/tests/formulation_equivalence.rs` (3 new reference-vs-portable HiGHS tests).

## Verification

| Command | Result |
|---|---|
| `cargo test -p roml --test common_constructs minmax` | 0 — 8 passed |
| `cargo test -p roml --test common_constructs absolute` | 0 — 7 passed |
| `cargo test -p roml --test common_constructs product` | 0 — 6 passed (the `binary_binary`/`binary_times_linear` enumeration tests are included in the full-suite run; 8 product tests total) |
| `cargo test -p roml --test compiler_bridges` | 0 — 15 passed |
| `cargo test -p roml-highs --test formulation_equivalence minmax/absolute/product` | 0 — 1 passed each |
| `cargo test -p roml --all-targets` | 0 — **759 passed; 0 failed** |
| `cargo test -p roml-highs --all-targets` | 0 — **121 passed; 0 failed** |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 — clean |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 — clean |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 — clean |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` | 0 — clean |
| `cargo public-api -p roml` | 0 — **18783 items** (+1247 over the Task 16 baseline 17536) |

## Deviations from plan

1. **Selector helpers emit the complete selector (Rule 1 bug fix, Task 17b).** The `exact_max_selector`/`exact_min_selector` helpers originally emitted only the sum-binary and Big-M rows (the minmax `compile` emitted the base `y >= x_i` rows itself), so the clamp bridge's direct helper calls omitted the base rows and `z = min(w, hi)` was unbounded above. Moved the base-row emission into the helpers and removed the duplicate loops from the minmax arms. Behavior of the Task 17a min/max is unchanged (re-verified).
2. **Exact-selector rows need negated operand coefficients (Rule 1 bug fix, Task 17a).** `y - x_i` requires `-x_i` coefficients; the first implementation added them positively. Caught by the feasible-set enumeration and randomized direct-evaluation tests.
3. **One-sided minmax output bounds are relation-specific (Rule 1 bug fix, Task 17a).** Exact → `[l_min, u_max]`, max-epigraph → `[l_min, +inf)`, min-hypograph → `(-inf, u_max]`; the initial `[l_min, u_max]` for every relation broke the D13 proof for the one-sided models.
4. **Dispatch arms for not-yet-wired constructs returned `UnsupportedFeature`** at the Task 17a/17b intermediate commits (the `ConstructKind` match must be exhaustive); each was replaced by the real bridge as its task landed.
5. **Test-surface fixes (Rule 1):** `compiled.variables.is_empty()` in the binary-binary test was corrected to count only `Construct`-origin generated variables (the snapshot always includes user variables); `integer().bounds(0, 10)` integer literals fixed to `f64`.
6. **Public API was frozen by Task 17a.** The three payload types, builders, `GeneratedRole`s, and `BackendFeature`s all landed with the Task 17a commit (they were needed to keep the crate compiling); Tasks 17b/17c wired the bridge bodies behind them, so the public API count (18783) did not change between 17a→17b→17c.
7. **Reference-backend solve adaptation.** The plan's "randomized direct evaluation" bullet names both the reference backend and HiGHS as solvers, but `ReferenceBackend` is a projection/state-tracking backend and does not solve MILPs. The core-crate randomized direct-evaluation tests therefore verify the exact formulation algebraically (for sampled operands, `y = max/min/|x|/clamp/b·f` is feasible and `±0.5` away is not — an existential check over the generated binaries), and the actual solve-based randomized/reference-vs-portable equivalence runs on HiGHS (`roml-highs/tests/formulation_equivalence.rs`). This preserves the intent (fixed-input randomized direct evaluation) while respecting the reference backend's non-solving contract.

## Commit trail (Plan 02)

| # | SHA | Message |
|---|---|---|
| 5 | `cd204be` | `feat(model): add exact min/max and one-sided relations` |
| 6 | `3be5093` | `feat(model): add exact absolute value, positive part, and clamp` |
| 7 | `109e0d8` | `feat(model): add algebraic semantic constructs` |
| 8 | (this commit) | `docs(32): summarize Plan 02 evidence and deviations` |

## Self-Check: PASSED

- Created files verified: `src/construct/{minmax,absolute,product}.rs`, `src/compiler/bridge/{minmax,absolute,product}.rs`, and the evidence/SUMMARY appends are present in the worktree history.
- Commits verified: `cd204be`, `3be5093`, `109e0d8` exist in the worktree history.
- Verification matrix: roml 759 / highs 121 / both clippy lanes clean / both doc lanes clean / fmt clean / `cargo public-api -p roml` 18783 items. OR review is requested per the packet (Task 17 final commit).
