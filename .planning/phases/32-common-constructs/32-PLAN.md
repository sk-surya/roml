---
phase: 32-common-constructs
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/compiler/bounds.rs
  - src/compiler/bridge/mod.rs
  - src/compiler/bridge/indicator.rs
  - src/compiler/bridge/reification.rs
  - src/compiler/bridge/boolean.rs
  - src/compiler/bridge/cardinality.rs
  - src/compiler/mod.rs
  - src/compiler/session.rs
  - src/compiler/report.rs
  - src/compiler/origin.rs
  - src/compiler/capability.rs
  - src/construct/mod.rs
  - src/construct/indicator.rs
  - src/construct/reification.rs
  - src/construct/boolean.rs
  - src/construct/cardinality.rs
  - src/model/mod.rs
  - src/lib.rs
  - src/advanced.rs
  - roml-highs/src/session.rs
  - tests/compiler_bridges.rs
  - tests/common_constructs.rs
  - roml-highs/tests/formulation_equivalence.rs
  - docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md
autonomous: false
requirements:
  - SM-13.1
  - SM-13.2
  - SM-13.3
  - SM-13.4
  - SM-13.5
  - SM-13.6
  - SM-01.3
  - SM-02.3
  - SM-02.5
must_haves:
  truths:
    - "Every construct has one semantic definition"
    - "Complete origins: every generated bridge entity has an EntityOrigin::Construct { construct, role }"
    - "Exact portable formulation: every construct compiles to an exact native or bridge formulation, never a silent relaxation"
    - "Explicit failure when bounds/support are insufficient: construct-aware UnboundedBigM / UnsupportedFeature typed errors, never a silent default constant"
    - "Deterministic bound analysis: linear interval propagation is deterministic, rejects NaN, and derives Big-M only from finite bounds or explicit validated user values"
    - "Logical constructs reject invalid inputs: non-binary activators, duplicate cardinality inputs, invalid k, and continuous exact reification without separation"
  artifacts:
    - src/compiler/bounds.rs
    - src/compiler/bridge/{mod,indicator,reification,boolean,cardinality}.rs
    - src/construct/{mod,indicator,reification,boolean,cardinality}.rs
    - src/lib.rs (A30 public construct exports)
    - tests/compiler_bridges.rs
    - tests/common_constructs.rs
    - roml-highs/tests/formulation_equivalence.rs
    - docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md
  key_links:
    - "Origin completeness: bridge finalizer records every generated entity's construct origin (SM-02.5)"
    - "UnboundedBigM: no finite Big-M for the construct's bounds is a typed CompileError naming the construct and expression, never a silent constant (SM-13.2/SM-13.4, D12)"
    - "Capability/bridge selection: Auto prefers a qualified native BackendFeature::Indicator, otherwise a finite-bound exact bridge; NativeRequired rejects (design §8.1)"
    - "A30 public surface: ConstructKind/ConstructEntry public exports; Fixture/FixturePayload/add_construct_fixture stay crate-private"
---

# Phase 32 — Common Semantic Modeling Constructs

> **For agentic workers:** this phase is P32 Plan 01 — the P32 foundation and the logical-construct family. Execute Task 15 first and alone (it produces the bound-analysis and bridge framework that Task 16's bridges are written against), then Task 16 strictly serially. Follow the TDD protocol from `EXECUTION.md` for every task: write a focused failing test, record the expected failure, implement the smallest correct behavior, run focused then phase tests, commit one coherent unit, update evidence and traceability. Do NOT run `roml-mosek`/`roml-xpress` — they are known-broken against the current facade and out of scope (M2 convention); never use workspace-wide commands. Stop after Task 16 and request independent review before marking the phase done. Do not touch git branches (`phase-roml-P32-common-constructs` exists; work on it).

**Goal:** deliver a bounded high-value exact MILP construct library over the frozen compiler contract — establish exact, deterministic formulations for the common logical semantic constructs (indicator, reification, Boolean, cardinality) on the P26 backend-IR foundation, with the shared bound-analysis and bridge framework they need.

**Requirements:** SM-13 (all clauses at the level the construct bridges need), SM-01.3, SM-02.3, SM-02.5 (P32 closures per `TRACEABILITY.md`).

## Requirements

- **SM-13.1** — deterministic interval analysis computes bounds for linear scalar functions. Closed by Task 15 (`src/compiler/bounds.rs`).
- **SM-13.2** — a Big-M bridge requires a finite derived value or explicit user value. Closed by Task 15 (one-sided Big-M helpers returning finite derived/validated values or the construct-aware `UnboundedBigM` marker).
- **SM-13.3** — explicit M values are validated against known bounds where possible. Closed by Task 15.
- **SM-13.4** — compilation errors identify the construct and missing/unbounded expression. Closed by Task 15 (`CompileError::UnboundedBigM { construct, expression }`) and exercised by Task 16's bridges.
- **SM-13.5** — compilation reports record M values, derivations, and bound sources. Closed by Task 15 (bound-evidence report entries).
- **SM-13.6** — M3 does not silently run auxiliary optimization problems for bound tightening. Closed by Task 15 (derivation from declared bounds/coefficients only — no auxiliary LP).
- **SM-01.3** — every construct has a stable handle, metadata, activity state, and parameter-dependency information. Closed by Task 16 (the real per-construct payloads participate in the P25 construct arena; stable `Construct` handles from the public builders; `derive_parameter_dependencies` extended).
- **SM-02.3** — constructs support names, descriptions, groups, tags, and optional source metadata. Closed by Task 16 (payloads are addressable through the existing `EntityMetadata` store keyed by `EntityRef::Construct`).
- **SM-02.5** — every generated entity maps to a user entity, construct, or overlay role. Closed by Task 15/16 (bridge finalizer records `EntityOrigin::Construct { construct, role }` for every generated variable/row; `GeneratedRole` gains deterministic role variants).

**Clause-level scope (must be stated in the evidence file):**

- **SM-13** is closed by P32 only at the level the common-construct bridges need (interval analysis over linear scalar functions; finite-bound/validated Big-M; construct-identifying errors; M-value/bound-source reporting; no silent auxiliary tightening). Per `TRACEABILITY.md`, full SM-13 closure (PWL interval analysis extended to curvature, bound-source traces, `BigM` evidence on the P33 fixtures) remains **P33**. Do not claim full SM-13 closure in `M3_P32_COMMON_CONSTRUCTS.md`.
- **SM-12** is NOT a mandate of this plan's requirement list. Task 16 advances **SM-12.1, SM-12.2, SM-12.5, SM-12.8** (indicator one-way semantics with native/bridge selection; reification separation semantics; Boolean/cardinality exact rows; stable handles + formulation diagnostics). The remaining SM-12 clauses (**SM-12.3** exact min/max, **SM-12.4** absolute value/positive part/clamp, **SM-12.6** binary products, **SM-12.7** no continuous×continuous exactness) and the corresponding ROADMAP P32 deliverables (min/max, abs/clamp, products) land in the **Task 17 follow-up plan** (Plan 02 of P32), which this plan does NOT cover (see "Source coverage audit").

## Files

Create:

- `docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md` — phase evidence file; created empty (with baseline + environment) before implementation per `EXECUTION.md`, appended as work proceeds.
- `src/compiler/bounds.rs` — `Interval`, `BoundTrace`, `BoundSource`, `BoundAnalyzer`, one-sided Big-M helpers, `UnboundedBigM` (Task 15).
- `src/compiler/bridge/mod.rs` — bridge contract + `BridgeFinalizer` (deterministic generated order, dependency capture, origins, report entries) (Task 15).
- `src/compiler/bridge/{indicator,reification,boolean,cardinality}.rs` — the four exact bridge modules (Task 16).
- `src/construct/{indicator,reification,boolean,cardinality}.rs` — the four exact semantic payload types (Task 16).
- `tests/compiler_bridges.rs` — interval-analysis, Big-M, and bridge-finalization tests (Task 15).
- `tests/common_constructs.rs` — construct validation, payload storage, compilation, and feasible-set enumeration tests (Task 16).
- `roml-highs/tests/formulation_equivalence.rs` — native/portable feasible-set equivalence on HiGHS (Task 16).

Modify:

- `src/compiler/mod.rs` — declare `pub mod bounds; pub mod bridge;`; add `CompileError::UnboundedBigM { construct, expression }` (and any bridge error variants the design §19 families need).
- `src/compiler/session.rs` — integrate the bridge framework and construct compilation (Task 16 wires active constructs through the bridges).
- `src/compiler/report.rs` — bound-evidence report entries (M values, derivations, bound sources — SM-13.5).
- `src/compiler/origin.rs` — fill `GeneratedRole` with deterministic role variants for the four constructs.
- `src/compiler/capability.rs` — support `SupportLevel::Bridge` declarations (SM-04.2) via a `FeatureSupport::bridge` constructor.
- `src/construct/mod.rs` — add the four `ConstructKind` variants; extend `derive_parameter_dependencies`; keep the `Fixture` variant crate-private (A30).
- `src/model/mod.rs` — public builders `add_indicator`, `add_reify`, `add_boolean`, `add_cardinality`.
- `src/lib.rs` — **A30:** `pub(crate) mod construct;` → `pub mod construct;`; re-export `ConstructKind`/`ConstructEntry` and the four payload types.
- `src/advanced.rs` — re-export the new construct payload and bridge surface for framework/backend authors (never the ordinary prelude).
- `roml-highs/src/session.rs` — `highs_capability_set` declares the P32 construct features as `SupportLevel::Bridge` (no unqualified native claims).

## Task 15 — Add interval bounds and bridge framework

**Phase:** P32  **Requirements:** SM-13.1, SM-13.2, SM-13.3, SM-13.4, SM-13.5, SM-13.6, SM-02.5 (foundation)

**Read first:**
- `docs/superpowers/specs/2026-08-02-semantic-modeling-and-solve-workflows-design.md` §8.5 "Bridge contract", §9 "Bound analysis and Big-M safety", §19 "Failure semantics".
- `docs/superpowers/plans/2026-08-02-semantic-modeling-and-solve-workflows.md` — Task 15 (verbatim translation target).
- `src/compiler/mod.rs` — the `CompileError` family the new variants extend.
- `src/compiler/report.rs` — `CompilationReport`/`FormulationDecision` (the report-entry extension point).
- `src/compiler/origin.rs` — `EntityOrigin::Construct { construct, role }` and the empty `GeneratedRole` boundary the bridge finalizer fills.
- `src/compiler/session.rs` — `CompilationSession::compile_snapshot` (the compiler entry the framework integrates into).
- `src/snapshot.rs` — `ModelSnapshot.constructs` and the variable/constraint entry shapes the bound analyzer consumes.

**TDD order** (per `EXECUTION.md`):

1. **Baseline + evidence.** Record in `docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md` the branch (`phase-roml-P32-common-constructs` from `main@192cd00`), exact base SHA, Rust/Cargo versions, target/OS, and the untouched `roml`/`roml-highs` baseline matrices from `EXECUTION.md` (fmt/check/clippy/test/doc), plus `cargo public-api -p roml` and `cargo package --list` for both crates.
2. Write failing tests in `tests/compiler_bridges.rs`:
   - **Interval analysis:** `BoundAnalyzer::interval_of(linear)` over coefficient signs (positive/negative/mixed), constant terms, fixed variables (equal lower/upper bounds — the fixing representation), infinite bounds (free variables), and parameters (evaluated value). Assert deterministic interval results.
   - **NaN rejection:** a non-finite coefficient or bound returns a typed error; no NaN propagates silently.
   - **One-sided Big-M:** a finite-bound implication derives a finite M; a free/unbounded side returns the construct-aware `UnboundedBigM` marker (never a silent default constant — D12); an explicit user M is validated against known bounds where possible (SM-13.3) and rejected when inconsistent.
   - **Bridge finalization:** generated entities appear in deterministic order (dense ids, ordered by construct id then role sequence); the finalizer captures bridge dependencies; every generated compiled variable/row has an `EntityOrigin::Construct { construct, role }`; report entries record M values, derivations, and bound sources (SM-13.5).
3. Run the tests and record the expected failures (missing `roml::compiler::{bounds, bridge}` surface).
4. Implement:
   - `src/compiler/bounds.rs`: `pub struct Interval { pub lower: f64, pub upper: f64 }`; `pub struct BoundTrace { pub sources: Vec<BoundSource>, pub result: Interval }`; `BoundSource` as an implementation-detail provenance marker enum (declared variable bounds / fixed value / parameter value / constant) — crate-private per design §9 gloss; `BoundAnalyzer` with deterministic linear interval propagation over `ScalarFunction::Linear` handling coefficient signs, constants, fixed/equal-bound variables, infinite bounds, and evaluated parameters, rejecting NaN and non-finite arithmetic with a typed error (SM-13.1, SM-13.6); one-sided Big-M helpers (`bound_big_m_implied`, `validated_explicit_big_m`) returning a finite derived M, a validated explicit M, or the construct-aware `UnboundedBigM` marker; `pub(crate) struct UnboundedBigM { construct: Construct, expression: String }` — the implementation-detail marker that no finite Big-M exists for the construct's bounds, never silently substituted.
   - `src/compiler/bridge/mod.rs`: the bridge contract (every bridge produces compiled variables/rows, mandatory origins, representation kind, dependencies, bound/Big-M evidence, and report entries — design §8.5); `BridgeFinalizer` with deterministic generated order (dense compiled ids appended in a fixed per-construct, per-role sequence), dependency capture, origin completeness (every generated entity is `EntityOrigin::Construct { construct, role }`), and report entries appended to `CompilationReport.formulation_decisions`; explicit failure (`CompileError::UnboundedBigM` / `UnsupportedFeature`) when an exact representation is unavailable.
   - `src/compiler/mod.rs`: declare `pub mod bounds; pub mod bridge;`; add `CompileError::UnboundedBigM { construct: Construct, expression: String }` (SM-13.4, design §19).
   - `src/compiler/report.rs`: bound-evidence report entries recording M values, derivations, and bound sources (SM-13.5).
5. Run `cargo test -p roml --test compiler_bridges` (must pass), then `cargo test -p roml --all-targets`, `cargo clippy -p roml --all-targets -- -D warnings`, and `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps`.
6. Update evidence and traceability.
7. Commit one coherent unit.

- [ ] Test interval arithmetic over coefficient signs, constants, fixed variables, infinite bounds, and parameters.
- [ ] Implement deterministic linear propagation and reject NaN.
- [ ] Implement one-sided Big-M helpers returning construct-aware `UnboundedBigM`.
- [ ] Implement bridge finalization with deterministic generated order, dependency capture, origins, and report entries.
- [ ] Stop when all four packet bullets hold and `compiler_bridges` is green.
- [ ] Commit as `feat(compiler): add safe bridge infrastructure`.

**Stopping condition:** deterministic linear interval propagation over the five coefficient/bound classes rejects NaN; every Big-M request returns a finite derived/validated value or the construct-aware `UnboundedBigM` marker; bridge finalization produces deterministic generated order, dependency capture, complete construct origins, and report entries; `cargo test -p roml --test compiler_bridges` exits 0.

**Commit:** `feat(compiler): add safe bridge infrastructure`

**Verification:**

```bash
cargo test -p roml --test compiler_bridges
cargo test -p roml --all-targets
cargo clippy -p roml --all-targets -- -D warnings
```

**Acceptance criteria:**
- All three commands exit 0.
- `src/compiler/bounds.rs` defines `Interval { lower, upper }`, `BoundTrace { sources, result }`, and a `BoundAnalyzer` whose deterministic linear propagation handles coefficient signs, constants, fixed/equal-bound variables, infinite bounds, and evaluated parameters, and rejects NaN/non-finite input with a typed error (SM-13.1, SM-13.6). No auxiliary LP is ever run for bound tightening (SM-13.6).
- One-sided Big-M helpers return a finite derived value, a validated explicit user value, or the construct-aware `UnboundedBigM` marker — never a silent default constant (SM-13.2, D12); explicit M is validated against known bounds where possible (SM-13.3).
- `src/compiler/bridge/mod.rs` implements bridge finalization with deterministic generated order, dependency capture, complete origins (`EntityOrigin::Construct { construct, role }`, SM-02.5), and report entries recording M values/derivations/bound sources (SM-13.5).
- `src/compiler/mod.rs` declares `CompileError::UnboundedBigM { construct, expression }` identifying the construct and the missing/unbounded expression (SM-13.4, design §19).

## Task 16 — Add indicators, reification, Boolean, and cardinality

**Phase:** P32  **Requirements:** SM-01.3, SM-02.3, SM-02.5, SM-13.4 (exercised), and advances SM-12.1/SM-12.2/SM-12.5/SM-12.8 (full SM-12 closure in the Task 17 follow-up plan)

**Read first:**
- `docs/superpowers/specs/2026-08-02-semantic-modeling-and-solve-workflows-design.md` §7 "Canonical semantic constructs", §8.1 "Compilation policy", §8.2 "Backend capability model", §16.1 "Indicators", §16.2 "Reification", §16.4 "Boolean and cardinality", §19 "Failure semantics".
- `docs/superpowers/plans/2026-08-02-semantic-modeling-and-solve-workflows.md` — Task 16 (verbatim translation target).
- Task 15 artifacts (`src/compiler/bounds.rs`, `src/compiler/bridge/mod.rs`) — the framework the four bridges call into.
- `src/compiler/session.rs` — the construct-compilation integration point.
- `src/compiler/origin.rs` — `GeneratedRole` (fill with role variants).
- `src/compiler/capability.rs` — `BackendFeature::Indicator`, `SupportLevel::Bridge`, `FeatureSupport`.
- `src/construct/mod.rs` — `ConstructKind`/`ConstructEntry` (A30 visibility) and `derive_parameter_dependencies`.
- `src/model/mod.rs` — the `add_construct_fixture` pattern and `Change::ConstructAdded`/`ModelOp::AddConstruct` the public builders mirror.
- `src/lib.rs` — the A30 export change.
- `roml-highs/src/session.rs` — `highs_capability_set` (declare bridge support).
- `docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md` — this phase's evidence file.

### A30 — public-surface step (MANDATORY task bullet)

Amendment A30 is activated by this task: the real per-construct variants land here, so `ConstructKind`/`ConstructEntry` and the construct module become **public exports**.

- `src/lib.rs`: change `pub(crate) mod construct;` → `pub mod construct;` and add crate-root re-exports for `ConstructKind`, `ConstructEntry`, and the four payload types (`IndicatorConstraint`, `ReificationConstraint`, `BooleanConstraint`, `CardinalityConstraint`). Keep the existing `Construct`/`ConstructId` and `FormulationPreference` re-exports. Keep the `#[non_exhaustive]` extension boundary on `ConstructKind` (A30: "the `#[non_exhaustive]` extension boundary stays").
- **Stay crate-private:** the `Fixture` variant, `FixturePayload` (make its fields private so external code cannot construct it; add a `pub(crate)` constructor for the in-crate `#[cfg(test)]` fixture helper — the existing `fixture()` helper at `src/model/mod.rs` builds `FixturePayload` via struct literal from a sibling module, which breaks if the fields become private), and `Model::add_construct_fixture`/`Model::construct` (both stay `pub(crate)`). In-crate `#[cfg(test)]` fixture lifecycle tests remain (adjusted to the constructor).
- **Public-API diff expectation:** `cargo public-api -p roml` grows by the construct-module surface (`roml::construct::{ConstructKind, ConstructEntry, IndicatorConstraint, ReificationConstraint, BooleanConstraint, CardinalityConstraint, ...}`), the crate-root re-exports, the four `Model` builder methods, and the `src/advanced.rs` re-exports. The `Fixture` variant, `FixturePayload`, and `add_construct_fixture` MUST NOT appear in the public-api output — verify with `cargo public-api -p roml` and record the diff in the evidence file.

**TDD order** (per `EXECUTION.md`):

1. Write failing tests in `tests/common_constructs.rs` (core) and `roml-highs/tests/formulation_equivalence.rs` (HiGHS):
   - **Validation rejections:** non-binary activator (continuous or integer variable) → typed `ModelError`; duplicate variables in a cardinality input → typed error; invalid `k` (negative, non-integral, or greater than the input length) → typed error; continuous exact reification without an explicit separation tolerance → typed error (SM-12.2, D14).
   - **Payload storage:** each builder returns a stable `Construct` handle; the arena stores the exact semantic payload and the per-construct `FormulationPreference` (A29 single authority); snapshot/delta round-trip preserves payload and preference (SM-01.3, SM-01.4).
   - **Compilation:** active constructs compile through the Task 15 framework to exact rows/variables; every generated entity carries `EntityOrigin::Construct { construct, role }` (SM-02.5); `Auto` selects a qualified native `BackendFeature::Indicator` when the backend declares it, otherwise the finite-bound exact bridge; `Portable` forces the bridge; `NativeRequired` rejects non-native (SM-12.1, design §8.1); insufficient bounds → `CompileError::UnboundedBigM`; unqualified feature → `CompileError::UnsupportedFeature`.
   - **Reification semantics:** compiled as two implications; unit gap inferred only when the expression is proven integer-valued; explicit separation tolerance honored (SM-12.2, D14).
   - **Boolean/cardinality:** exact linear rows for implication, equivalence, any/all, and exactly/at-most/at-least-k (SM-12.5).
   - **Feasible-set enumeration:** for small binary domains (≤3 variables) enumerate the semantic truth set and the compiled feasible set (reference formulation, native where declared, portable bridge) and assert equality (packet verbatim: "Enumerate small binary domains and compare semantic/reference/native/portable feasible sets").
2. Run the tests and record the expected failures (missing `roml::construct::{indicator,reification,boolean,cardinality}` payload types and `Model::add_*` builders).
3. Implement (in order):
   - **A30 public-surface step** (bullets above) — land it first so the rest of the task compiles against the public module.
   - `src/construct/indicator.rs` — `IndicatorConstraint` payload (binary activator, one-way implication direction, scalar-function-in-set relation); `src/construct/reification.rs` — `ReificationConstraint` payload (scalar-function-in-set relation, separation tolerance, proven-integrality flag); `src/construct/boolean.rs` — `BooleanConstraint` payload (implication/equivalence/any/all over binary variables); `src/construct/cardinality.rs` — `CardinalityConstraint` payload (exactly/at-most/at-least, `k`, binary variable list). All payload shapes follow design §7.
   - `src/construct/mod.rs` — add the four `ConstructKind` variants; extend `derive_parameter_dependencies` for the new payloads; keep the `Fixture` variant crate-private.
   - `src/model/mod.rs` — public builders `add_indicator`, `add_reify`, `add_boolean`, `add_cardinality`, each returning `Result<Construct, ModelError>` (stable generation-safe handle, SM-01.3/SM-12.8), validating the four invalid-input classes, accepting an optional per-construct `FormulationPreference`, and recording `Change::ConstructAdded` with the exact payload + preference (A29 single authority).
   - `src/compiler/origin.rs` — fill `GeneratedRole` with deterministic role variants for the four constructs (e.g. `IndicatorImplicationRow`, `IndicatorBigM`, `ReificationImplicationRow`, `ReificationComplement`, `BooleanAuxiliary`, `CardinalityRow`), keeping `#[non_exhaustive]`.
   - `src/compiler/bridge/{indicator,reification,boolean,cardinality}.rs` — the four exact bridge modules: indicator (finite-bound Big-M bridge over the one-sided implication; a qualified native `BackendFeature::Indicator` when the backend declares it); reification (two implications; unit gap only from proven integrality; separation tolerance); boolean (exact linear rows for implication/equivalence/any/all); cardinality (exact linear rows for exactly/at-most/at-least-k). Each bridge produces its generated entities through the Task 15 `BridgeFinalizer` (deterministic order, dependency capture, origins, report entries).
   - `src/compiler/session.rs` — `compile_snapshot` iterates active constructs in the snapshot's deterministic construct-id order, dispatches each to native (qualified `BackendFeature::Indicator`) or the matching bridge per the `CompilationPolicy` narrowed by the per-construct `FormulationPreference`, appends generated entities and origins, and returns typed errors on unbounded (`CompileError::UnboundedBigM`) or unsupported (`CompileError::UnsupportedFeature`).
   - `src/compiler/capability.rs` — add `FeatureSupport::bridge(limitations)` so backends can report exact ROML bridge support separately from native (SM-04.2).
   - `roml-highs/src/session.rs` — `highs_capability_set` declares `BackendFeature::Indicator` and the logical-construct features as `SupportLevel::Bridge` (exact ROML formulations). Do NOT declare native indicator support unless an official-header/API audit qualifies it; unqualified native claims are forbidden (SM-04.3, the native-research protocol).
   - `src/advanced.rs` — re-export the four payload types and the bridge surface for framework/backend authors.
4. Run `cargo test -p roml --test common_constructs indicator` and `cargo test -p roml-highs --test formulation_equivalence indicator` (must pass), then the phase verification matrix below.
5. Update the evidence file with the per-construct requirements from `TRACEABILITY.md` (semantic definition; accepted/rejected domains; native and bridge representation; origin map; explicit reference formulation; randomized solver equivalence; formulation report output; unsupported/unbounded failure behavior) and record the `cargo public-api -p roml` diff (A30).
6. Commit one coherent unit.

- [ ] **A30 public-surface step:** `ConstructKind`/`ConstructEntry` and the construct module become public exports in `src/lib.rs`; `Fixture`/`FixturePayload`/`add_construct_fixture` stay crate-private; in-crate fixture lifecycle tests remain. Record the `cargo public-api -p roml` diff.
- [ ] Reject non-binary activators, duplicate cardinality inputs, invalid `k`, and continuous exact reification without separation.
- [ ] Store exact semantic payloads and per-construct formulation preference.
- [ ] Select qualified native indicators or finite-bound exact bridges.
- [ ] Implement reification as two implications; infer unit gap only from proven integrality.
- [ ] Implement exact Boolean/cardinality rows.
- [ ] Enumerate small binary domains and compare semantic/reference/native/portable feasible sets.
- [ ] Stop when all packet bullets hold and both focused test targets are green.
- [ ] Commit as `feat(model): add logical semantic constructs`.

**Stopping condition (packet, verbatim, plus A30):** reject non-binary activators, duplicate cardinality inputs, invalid `k`, and continuous exact reification without separation; store exact semantic payloads and per-construct formulation preference; select qualified native indicators or finite-bound exact bridges; implement reification as two implications with unit gap only from proven integrality; implement exact Boolean/cardinality rows; enumerate small binary domains and compare semantic/reference/native/portable feasible sets; `ConstructKind`/`ConstructEntry` are public exports with `Fixture`/`FixturePayload`/`add_construct_fixture` crate-private (A30).

**Commit:** `feat(model): add logical semantic constructs`

**Verification:**

```bash
cargo test -p roml --test common_constructs indicator
cargo test -p roml-highs --test formulation_equivalence indicator
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
cargo clippy -p roml --all-targets -- -D warnings
cargo clippy -p roml-highs --all-targets -- -D warnings
```

**Acceptance criteria:**
- All six commands exit 0.
- `src/construct/{indicator,reification,boolean,cardinality}.rs` define `IndicatorConstraint`, `ReificationConstraint`, `BooleanConstraint`, and `CardinalityConstraint` (design §7 identifiers); `ConstructKind` carries them and remains `#[non_exhaustive]`.
- `src/lib.rs` publicly exports `ConstructKind`/`ConstructEntry` and the four payload types; the `Fixture` variant, `FixturePayload`, and `add_construct_fixture` are crate-private (A30) — verified by `cargo public-api -p roml` absence plus source assertions.
- `Model` exposes `add_indicator`, `add_reify`, `add_boolean`, and `add_cardinality` returning stable `Construct` handles and rejecting non-binary activators, duplicate cardinality inputs, invalid `k`, and continuous exact reification without separation (SM-01.3, SM-12.8).
- Compilation of each active construct yields exact rows/variables whose origins are `EntityOrigin::Construct { construct, role }` (SM-02.5); selection honors `Auto`/`Portable`/`NativeRequired` narrowed by per-construct preference (SM-12.1, design §8.1); insufficient bounds → `CompileError::UnboundedBigM`; unqualified native under `NativeRequired` → `CompileError::UnsupportedFeature`.
- Reification compiles as two implications with unit gap only from proven integrality and honors the explicit separation tolerance (SM-12.2, D14).
- Boolean/cardinality rows are exact (SM-12.5).
- `tests/common_constructs.rs` and `roml-highs/tests/formulation_equivalence.rs` enumerate small binary domains and prove semantic/reference/native/portable feasible-set equality.

## Verification

Phase-level checks (all must exit 0; per-crate only — never workspace-wide):

```bash
cargo fmt --all -- --check
cargo test -p roml --test compiler_bridges
cargo test -p roml --test common_constructs indicator
cargo test -p roml-highs --test formulation_equivalence indicator
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
cargo clippy -p roml --all-targets -- -D warnings
cargo clippy -p roml-highs --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps
cargo public-api -p roml
```

Baseline matrix (untouched tree, recorded in the Task 15 evidence step; `roml-mosek`/`roml-xpress` are out of scope — never use workspace-wide commands):

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

Per P30–P33 mandatory checks in `EXECUTION.md`: algebra/reference formulation tests; native/portable equivalence; generated-origin completeness; no-silent-relaxation tests; parameter dependency/update tests. Skips must be recorded in the evidence file, never treated as passing.

## Waves and parallelization

This plan has two tasks and two execution waves:

- **Wave 1 — Task 15** (bound analyzer + bridge framework).
- **Wave 2 — Task 16** (logical construct payloads + bridges + A30 public surface).

**Can Task 15 and Task 16 run in parallel? No — strictly serial. Two independent reasons:**

1. **Type dependency.** Task 16's four bridge modules (`src/compiler/bridge/{indicator,reification,boolean,cardinality}.rs`) are *written against* Task 15's `BridgeFinalizer`, `BoundAnalyzer`, one-sided Big-M helpers, and `UnboundedBigM`. They cannot compile before Task 15 lands. This differs from P26 Tasks 5/6, which were type-independent (`BackendFeature` needed none of `BackendSnapshot`) and therefore parallelizable. Task 16's bridges *call into* the Task 15 framework, so the compile-time dependency is real, not a reviewer convention.
2. **Shared files.** Both tasks modify `src/compiler/mod.rs` (Task 15 adds `pub mod bounds; pub mod bridge;`) and `src/compiler/session.rs` (Task 15 wires framework plumbing; Task 16 adds construct compilation). Running them in parallel would produce overlapping edits on the same files — a merge conflict by construction.

**Why the packet orders 15 then 16.** The packet's serial order (Tasks 15 → 16 → 17) matches the compile-time dependency: construct bridges consume the bound/Big-M machinery. Do not reorder.

**Why the P30 soft bridge was declared self-contained.** Task 13 (P30) was deliberately scoped to "violation rows and objective penalties only" — it does NOT depend on Task 15's bound-analysis/Big-M framework — precisely so P30 could proceed without waiting for the P32 foundation. That self-containment made P30's file ownership disjoint from P32's. It does NOT extend to Task 16, whose bridges are defined in terms of the Task 15 framework.

**Merge-conflict rationale:** run Task 15 then Task 16 with the same executor (Wave 1 then Wave 2), or with two agents only if Task 16 is explicitly blocked on Task 15's commit (`feat(compiler): add safe bridge infrastructure`). With ordered application — Task 16 edits `session.rs`/`origin.rs`/`capability.rs`/`lib.rs`/`model/mod.rs` only after Task 15's commit lands — there are zero conflicting text edits. This plan therefore exposes no cross-task parallelism within the phase; D26 (one active implementation phase) and the M3 WIP limit already bound the phase to a single branch.

## Source coverage audit

| Source | Item | Coverage |
|---|---|---|
| GOAL (ROADMAP P32) | exact deterministic formulations for common semantic constructs over the frozen compiler contract | This plan: foundation + logical constructs (Tasks 15-16) |
| REQ (phase mandate) | SM-13.1–13.6 | Task 15 |
| REQ | SM-01.3, SM-02.3, SM-02.5 | Task 16 |
| REQ (advance only) | SM-12.1, SM-12.2, SM-12.5, SM-12.8 | Task 16 |
| RESEARCH | bridge layers for unsupported semantic constraints; avoid arbitrary Big-M | Task 15/16 (exact bridges; `UnboundedBigM`; no default constants) |
| CONTEXT (D12, D14, A29, A30) | Big-M requires proof; reification separation; preference single authority; A30 public surface | Task 15 (Big-M), Task 16 (reification, preference, A30) |
| GOAL (ROADMAP P32 residual) | exact vs epigraph/hypograph min/max; absolute value/positive part/clamp; binary-binary and binary-times-bounded-linear products | **NOT in this plan** — Task 17 (min/max/abs/clamp/products) lands in the follow-up Plan 02 of P32. Flagged for the orchestrator; do not silently claim SM-12.3/12.4/12.6/12.7 here. |

## Review gates

Per `EXECUTION.md` § "Review gates", P32 receives two independent review passes at the phase boundary (after Task 16).

- **Pass 1 — Specification and correctness:** requirement coverage (SM-13.1–13.6 clause-level, SM-01.3, SM-02.3, SM-02.5); semantic correctness of the interval analysis and the four construct formulations (indicator one-way implication, reification as two implications, exact Boolean/cardinality rows); invariant preservation (origin completeness for every generated bridge entity, exactness never relaxed, deterministic generated order); unsupported/error behavior (construct-aware `UnboundedBigM`, `UnsupportedFeature`, validation rejections); API coherence (A30 public construct exports; construct module public, fixture scaffolding crate-private); test quality (feasible-set enumeration comparing semantic/reference/native/portable).
- **Pass 2 — Integration and operations:** incremental/rebuild behavior (construct compilation through the frozen P26 compiler contract; no compiler-contract change without an amendment); failure recovery; cross-platform/version behavior (HiGHS bridge-capability declarations); public API diff (`cargo public-api -p roml`, A30 additions); package/docs impact (`docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md`); migration accuracy (backend authors consume `SupportLevel::Bridge`).

**Blocking rules:**

- P0/P1 findings **block merge**.
- P2 findings may merge only when explicitly accepted and scheduled.
- `autonomous: false` — the executor pauses after Task 16 and does not declare the phase complete until both review passes resolve to no P0/P1 findings.
- Reviewers verify the SM-13 clause-level scope statement (P33 completes SM-13) and the SM-12 follow-up-plan disclosure (Task 17 covers min/max/abs/products) are explicit in the evidence file.

Evidence requirement: `docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md` must record the baseline, per-task verification with RED failures, the full verification matrix, the public API diff (A30), the per-construct evidence items from `TRACEABILITY.md`, reviewer findings and dispositions, and the residual risks before the gate result is marked pass (per `EXECUTION.md` § "Evidence file structure").

## Artifacts this phase produces

New modules and symbols (all names/signatures from the approved design and the packet's interface contract):

- `src/compiler/bounds.rs` — `Interval { lower, upper }`, `BoundTrace { sources, result }`, `BoundSource` (implementation-detail provenance marker), `BoundAnalyzer`, one-sided Big-M helpers, `UnboundedBigM` (construct-aware marker) (SM-13).
- `src/compiler/bridge/mod.rs` — bridge contract + `BridgeFinalizer` (deterministic generated order, dependency capture, origins, report entries) (design §8.5).
- `src/compiler/bridge/{indicator,reification,boolean,cardinality}.rs` — the four exact bridge modules.
- `src/construct/{indicator,reification,boolean,cardinality}.rs` — `IndicatorConstraint`, `ReificationConstraint`, `BooleanConstraint`, `CardinalityConstraint` payloads (design §7).
- `src/compiler/origin.rs` — `GeneratedRole` role variants for the four constructs (design §5).
- `src/compiler/report.rs` — bound-evidence report entries (M values, derivations, bound sources).
- `src/compiler/mod.rs` — `CompileError::UnboundedBigM { construct, expression }` (design §19, SM-13.4).
- `src/compiler/capability.rs` — `FeatureSupport::bridge` (SM-04.2).
- `src/compiler/session.rs` — construct compilation through the Task 15 framework.
- `src/model/mod.rs` — public builders `add_indicator`, `add_reify`, `add_boolean`, `add_cardinality`.
- `src/lib.rs` — **A30:** `pub mod construct;` + public re-exports of `ConstructKind`/`ConstructEntry` and the four payload types.
- `src/advanced.rs` — re-exported construct payload and bridge surface.
- `roml-highs/src/session.rs` — P32 construct features declared `SupportLevel::Bridge`.
- Test files: `tests/compiler_bridges.rs`, `tests/common_constructs.rs`, `roml-highs/tests/formulation_equivalence.rs`.
- Evidence: `docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md`.
- Modified: `src/construct/mod.rs` (four `ConstructKind` variants + `derive_parameter_dependencies`).

## must_haves

Goal-backward verification (the ROADMAP P32 gate, verbatim):

> **Gate:** every construct has one semantic definition, complete origins, exact portable formulation, and explicit failure when bounds/support are insufficient.

**Truths (observable behaviors):**

1. **Every construct has one semantic definition.** `ConstructKind` carries exactly one exact payload per construct (`IndicatorConstraint`, `ReificationConstraint`, `BooleanConstraint`, `CardinalityConstraint`); there is no second representation stored in canonical state.
2. **Complete origins.** Every generated bridge variable/row carries `EntityOrigin::Construct { construct, role }`; the bridge finalizer and `BackendSnapshot::validate` reject any unoriginated entity (D5, SM-02.5).
3. **Exact portable formulation.** Each construct compiles to an exact formulation — a qualified native primitive or a finite-bound exact bridge — never a silent relaxation (D13, SM-12.1).
4. **Explicit failure when bounds/support are insufficient.** No finite Big-M → `CompileError::UnboundedBigM { construct, expression }`; unqualified feature → `CompileError::UnsupportedFeature`; no silent default constant exists (SM-13.2/13.4, D12).
5. **Deterministic bound analysis.** Linear interval propagation is deterministic over coefficient signs, constants, fixed variables, infinite bounds, and parameters; NaN is rejected; M3 runs no auxiliary LP for bound tightening (SM-13.1, SM-13.6).
6. **Logical constructs reject invalid inputs.** Non-binary activators, duplicate cardinality inputs, invalid `k`, and continuous exact reification without separation are typed errors (SM-12.2, SM-12.5).

**Artifacts (files that must exist):**

- `src/compiler/bounds.rs`
- `src/compiler/bridge/{mod,indicator,reification,boolean,cardinality}.rs`
- `src/construct/{mod,indicator,reification,boolean,cardinality}.rs`
- `src/lib.rs` (A30 public construct exports)
- `tests/compiler_bridges.rs`, `tests/common_constructs.rs`, `roml-highs/tests/formulation_equivalence.rs`
- `docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md`

**Key links (critical connections where breakage cascades):**

- **Origin completeness** — a generated bridge entity without an `EntityOrigin::Construct` is unfinalizable; a missing origin at finalization is a typed `CompileError` (D5, SM-02.5).
- **`UnboundedBigM`** — the absence of a finite Big-M for a construct's bounds must surface as the construct-aware typed error, never as a silent default constant (SM-13.2/13.4, D12).
- **Capability/bridge selection** — `Auto` prefers a qualified native `BackendFeature::Indicator`, otherwise a finite-bound exact bridge; `NativeRequired` rejects; per-construct `FormulationPreference` narrows but never weakens exactness (design §8.1).
- **A30 public surface** — `ConstructKind`/`ConstructEntry` are public exports; `Fixture`/`FixturePayload`/`add_construct_fixture` remain crate-private; the `#[non_exhaustive]` boundary stays.

## Threat model

This is a modeling library; P32 introduces no network, filesystem, auth, or untrusted-input surface. The relevant trust boundaries are integrity/invariant boundaries:

| Boundary | Description | Mitigation in this phase |
|----------|-------------|--------------------------|
| canonical construct state → compiler | the compiler reads immutable snapshot construct entries only | `compile_snapshot` iterates `ModelSnapshot.constructs` deterministically; no mutable `Model` access (SM-03.2) |
| compiler → generated bridge entities | generated state must be complete, deterministic, and traceable | `BridgeFinalizer` enforces deterministic generated order and `EntityOrigin::Construct` completeness; `BackendSnapshot::validate` re-checks (D5) |
| bound analysis → Big-M | a wrong or invented M silently changes feasibility | finite derived/validated values only; `UnboundedBigM` is a typed error, never a default constant (D12, SM-13) |
| capability gating | a native claim without qualification misleads backend selection | P32 declares `SupportLevel::Bridge` for HiGHS; native indicator is declared only after an official-header audit (SM-04.3) |
| numeric arithmetic | NaN/infinity in interval propagation | deterministic propagation rejects non-finite input with a typed error (SM-13.1) |

**STRIDE register:**

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-32-01 | Tampering | `BoundAnalyzer`/Big-M helpers | high | mitigate | finite derived/validated M only; `UnboundedBigM` typed error; no default constant (SM-13.2/13.4) |
| T-32-02 | Information disclosure | `CompileError::UnboundedBigM` message | low | mitigate | error names the construct and expression (SM-13.4) — internal model identifiers only, no secrets |
| T-32-03 | Tampering | generated bridge entities | high | mitigate | `BridgeFinalizer` origin completeness + `BackendSnapshot::validate` (D5, SM-02.5) |
| T-32-04 | Spoofing | capability declaration (`SupportLevel::Bridge` vs `Native`) | medium | mitigate | HiGHS declares bridge only; native only after official audit (SM-04.3, D10) |
| T-32-05 | Elevation of privilege | A30 public construct surface | low | mitigate | `Fixture`/`FixturePayload`/`add_construct_fixture` stay crate-private; `#[non_exhaustive]` stays |
| T-32-SC | Tampering | npm/pip/cargo installs | low | accept | no new dependencies this phase (stdlib + existing workspace only); no package install tasks |

No new `unsafe`, environment mutation, filesystem scan, or stdout output is introduced by this phase.

## Gate

P32 passes when:

- Task 15 provides deterministic interval analysis, construct-aware `UnboundedBigM`, and bridge finalization with deterministic generated order, dependency capture, origins, and report entries (SM-13.1–13.6 at clause level; full SM-13 closure remains P33);
- Task 16 lands indicator, reification, Boolean, and cardinality constructs with exact payloads, validation rejections, native-or-bridge selection, two-implication reification, exact Boolean/cardinality rows, and semantic/reference/native/portable feasible-set equality (SM-01.3, SM-02.3, SM-02.5; advances SM-12.1/12.2/12.5/12.8);
- the **A30 public-surface step** is explicit: `ConstructKind`/`ConstructEntry` are public exports, fixture scaffolding stays crate-private, and the `cargo public-api -p roml` diff is recorded;
- the frozen P26 compiler contract is unchanged (no compiler-contract amendment without a reviewed `DECISIONS.md` change);
- all phase-level verification commands exit 0 (including `cargo test -p roml --test compiler_bridges`, `cargo test -p roml --test common_constructs indicator`, `cargo test -p roml-highs --test formulation_equivalence indicator`, both `--all-targets` suites, both clippy lanes, rustdoc with warnings denied, and `cargo public-api -p roml`);
- the P32 gate holds verbatim: every construct has one semantic definition, complete origins, exact portable formulation, and explicit failure when bounds/support are insufficient;
- the evidence bundle, public API diff, clause-level scope statements (SM-13 → P33; SM-12.3/12.4/12.6/12.7 → Task 17 follow-up plan), and residual risks are recorded; and
- both independent review passes resolve with no P0/P1 findings.

No crate publication, tag, or release is part of this phase (SM-15.8 / M3 stopping condition).

## Output

Create `.planning/phases/32-common-constructs/32-SUMMARY.md` when done, per the phase completion protocol.
