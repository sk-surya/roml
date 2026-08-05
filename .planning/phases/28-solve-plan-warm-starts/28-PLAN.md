---
phase: 28-solve-plan-warm-starts
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/solver/plan.rs
  - src/solver/effective_plan.rs
  - src/solver/facade.rs
  - src/solver/session.rs
  - src/solver/mod.rs
  - src/solution/metadata.rs
  - src/assignment.rs
  - src/lib.rs
  - src/advanced.rs
  - roml-highs/src/session.rs
  - roml-highs/src/start.rs
  - roml-highs/src/lib.rs
  - tests/solve_plan.rs
  - roml-highs/tests/solve_plan.rs
  - docs/knowledge/highs_mip_start_api.md
  - docs/release/evidence/M3_P28_SOLVE_PLAN_STARTS_HINTS.md
autonomous: false
requirements:
  - SM-07.1
  - SM-07.2
  - SM-07.7
  - SM-08.1
  - SM-08.2
  - SM-08.3
  - SM-08.4
  - SM-08.5
  - SM-08.6
  - SM-08.7
  - SM-04.5
must_haves:
  truths:
    - default unsupported behavior rejects
    - all applications/conversions/rejections are recorded
    - HiGHS behavior is qualified
    - solve and solve_with remain compatible
  artifacts:
    - src/solver/plan.rs
    - src/solver/effective_plan.rs
    - src/solution/metadata.rs (SolveMetadata.effective_plan field)
    - roml-highs/src/start.rs
    - roml-highs/src/session.rs (qualified start/hint capability declarations)
    - tests/solve_plan.rs
    - roml-highs/tests/solve_plan.rs
    - docs/knowledge/highs_mip_start_api.md
    - docs/release/evidence/M3_P28_SOLVE_PLAN_STARTS_HINTS.md
  key_links:
    - Plan executor -> backend typed capabilities (reject before mutation)
    - SolveMetadata.effective_plan <-> EffectiveSolvePlan <-> exact CompilationId
    - C_base / C_overlay / result.compilation_id — starts/hints never alter compiled identity
    - UnsupportedFeaturePolicy -> recorded conversions
    - HiGHS audit findings -> capability declarations -> reject-by-default
---

# Phase 28 — SolvePlan, Starts, Hints, and Effective-Plan Reporting

> **For agentic workers:** this phase is the M3 solve-attempt contract. It translates packet Task 10 verbatim plus the STATE-ledger blocking decision (the pinned HiGHS start-API audit) into three strictly serial tasks. Follow the TDD protocol from `EXECUTION.md` for every task: write a focused failing test, record the expected failure, implement the smallest correct behavior, run focused then phase tests, commit one coherent unit, update evidence and traceability. Do NOT run `roml-mosek`/`roml-xpress` — they are known-broken against the current facade and out of scope (M2 convention). Never use workspace-wide `cargo test` commands; every verification command is `-p roml` or `-p roml-highs` scoped. Stop after Task 3 and request independent review before marking the phase done.
>
> **Blocking decision (STATE ledger):** the pinned HiGHS start API audit is an explicit task item (Task 3). The executor must inspect the exact bundled/system official headers, implement only qualified support, reject absent hints by default, and never simulate unsupported behavior silently.

**Goal:** expose one explicit solve-attempt contract.

**Requirements:** SM-07.1, SM-07.2, SM-07.7, SM-08 (all clauses), SM-04.5.

## Requirements

- **SM-07.1** — `SolvePlan` combines solve options, overlays, starts, hints, objective overrides, and unsupported-feature policy. Closed by Tasks 1 and 2.
- **SM-07.2** — existing `solve` and `solve_with` remain convenience paths over `SolvePlan`. Closed by Task 2 (equivalence proof; M2 source compatibility preserved, D27).
- **SM-07.7** — `Solution` metadata contains the effective solve plan and objective-stage results. Closed by Task 2. `objective_stages` is an empty declaration in P28; P31 populates it.
- **SM-08.1** — `MipStart` accepts full or partial primal assignments with explicit repair policy. Closed by Task 1.
- **SM-08.2** — multiple starts are supported only when declared by the backend; otherwise behavior follows explicit policy. Closed by Task 1 (policy) and Task 3 (`MultipleMipStarts` capability declaration: the audit expects it stays `Unsupported` for the pinned bundled/system HiGHS, so any second start follows explicit policy).
- **SM-08.3** — `VariableHints` stores independent value/priority entries and never changes feasibility. Closed by Task 1 (type) and Task 2 (feasibility-signature proof).
- **SM-08.4** — unsupported starts and hints are rejected by default, never silently ignored. Closed by Tasks 1, 2, 3 (default rejection through `UnsupportedFeaturePolicy` and the backend capability gate).
- **SM-08.5** — explicit conversion policies may convert hints to a start or a start to temporary fixing; conversions are recorded. Closed by Tasks 1 (policy), 2 (recording), 3 (conversion execution against HiGHS).
- **SM-08.6** — LP basis warm starts remain a separate future artifact and are not conflated with primal assignments. Closed by Task 1 (type documentation and distinct semantics) and Task 3 (`InitialBasis` remains `Unsupported`).
- **SM-08.7** — HiGHS start behavior is qualified against pinned official APIs and supported versions. Closed by Task 3 (the audit).
- **SM-04.5** — every solve records selected native features, bridges, adjustments, and rejections. Closed by Task 2 (`EffectiveSolvePlan` in `SolveMetadata`, including the exact `CompilationId`). Per `TRACEABILITY.md`, SM-04.5 was deferred from P26 to P28.

## Files

Create:

- `docs/release/evidence/M3_P28_SOLVE_PLAN_STARTS_HINTS.md` — phase evidence file (TRACEABILITY.md expected path; closes SM-07.1–07.2, SM-07.7, SM-08). Created empty with the baseline section before implementation per `EXECUTION.md`, appended as work proceeds.
- `src/solver/plan.rs` — `SolvePlan`, `MipStart`, `RepairPolicy`, `VariableHints`, `VariableHint`, `HintPriority`, `UnsupportedFeaturePolicy`, `PlanError`, and the minimal `ObjectivePolicy`/`LexStagePolicy` scaffolding that `SolvePlan`'s design-§12 fields require.
- `src/solver/effective_plan.rs` — `EffectiveSolvePlan`, `AppliedFeature`, `PlanAdjustment`, `PlanRejection`, `ObjectiveStageResult`.
- `roml-highs/src/start.rs` — qualified HiGHS start/hint application (only what the audit qualifies; otherwise an explicit typed-unsupported module).
- `tests/solve_plan.rs` — core plan-type, validation, conversion-policy, equivalence, feasibility, and metadata-recording tests.
- `roml-highs/tests/solve_plan.rs` — end-to-end HiGHS solve-plan tests (capability matrix, default rejection, conversions, feasibility, metadata).
- `docs/knowledge/highs_mip_start_api.md` — the pinned official-header audit record (per `EXECUTION.md` "Native API research protocol": symbol signatures, availability, return codes, lifecycle, documented semantics, version availability).

Modify:

- `src/solver/facade.rs` — one plan executor; `solve`/`solve_with`/`solve_with_overlay` route through it; `solve_plan` entry point.
- `src/solver/session.rs` — extend `OverlaySession` with default-reject warm-start methods (`apply_mip_starts`, `apply_variable_hints`).
- `src/solution/metadata.rs` — `SolveMetadata` gains `pub effective_plan: EffectiveSolvePlan` (SM-07.7, SM-04.5).
- `src/assignment.rs` — only if a conversion helper lands there (e.g. extracting hint/start maps from `PrimalAssignment`); the primary conversions live in `src/solver/plan.rs`.
- `src/solver/mod.rs`, `src/lib.rs`, `src/advanced.rs` — module wiring and re-exports for the new public types.
- `roml-highs/src/session.rs` — version-aware capability declarations for `MipStart`/`PartialMipStart`/`MultipleMipStarts`/`VariableHints`/`InitialBasis` per the audit; the warm-start application wiring.
- `roml-highs/src/lib.rs` — re-export the qualified start module if public.

## Task 1 — Add SolvePlan, MipStart, VariableHints, and unsupported/conversion policy

**Phase:** P28  **Requirements:** SM-07.1 (type), SM-08.1, SM-08.2 (policy), SM-08.3 (type), SM-08.4 (policy default), SM-08.5 (policy), SM-08.6 (type semantics)

**Read first:**
- `docs/superpowers/specs/2026-08-02-semantic-modeling-and-solve-workflows-design.md` §11.2 "Distinct semantics", §11.4 "Starts and hints", §12 "SolvePlan and reversible overlays", §19 "Failure semantics".
- `docs/superpowers/plans/2026-08-02-semantic-modeling-and-solve-workflows.md` — "Assignments and solve intent" and "Solve plan and exact result identity" interface contracts (authoritative shapes).
- `src/assignment.rs` (P27 branch content) — `PrimalAssignment::validate_for` and the `AssignmentError` variants (`LineageMismatch`, `StaleVariable`, `ValueOutOfBounds`, `NonFiniteValue`) the plan validation reuses.
- `src/solver/overlay.rs` (P27 branch content) — `SolveOverlay`/`temporary_fixings` (the target of the start-to-fixing conversion).
- `.planning/milestones/M3-semantic-modeling-workflows/DECISIONS.md` — D8 (assignments, starts, hints, and locks are distinct).
- `.planning/milestones/M3-semantic-modeling-workflows/EXECUTION.md` — § "TDD protocol" and § "Commit policy".
- `src/solver/error.rs` — the `SolveError` family the plan errors extend.

**TDD order** (per `EXECUTION.md`):

1. Create the evidence file `docs/release/evidence/M3_P28_SOLVE_PLAN_STARTS_HINTS.md` with the baseline section first: record `git rev-parse HEAD` (branch `phase-roml-P28-solve-plan-warm-starts` from `main@40af9f4`, the repaired main following the P27/P32 restoration), Rust/Cargo versions, target/OS, the untouched `roml` and `roml-highs` baseline matrices, and `cargo public-api`/`cargo package --list` captures for both crates.
2. Write failing tests in `tests/solve_plan.rs`:
   - `SolvePlan` has exactly the design-§12 fields `options: SolveOptions`, `overlay: SolveOverlay`, `mip_starts: Vec<MipStart>`, `hints: VariableHints`, `objective_override: Option<ObjectivePolicy>`, `lex_stage_policy: LexStagePolicy`, `unsupported: UnsupportedFeaturePolicy`; `SolvePlan::new(SolveOptions)` builds an empty plan.
   - `MipStart { assignment: PrimalAssignment, repair: RepairPolicy, name: Option<String> }`; `RepairPolicy { BackendDefault, RejectIncomplete, AllowRepair }`; `MipStart::new(assignment, repair)`.
   - `VariableHints` stores independent value/priority entries (`entries: BTreeMap<Variable, VariableHint>` with accessors); `VariableHint { value: f64, priority: HintPriority }`; `HintPriority(pub i32)`; hints are a pure data record that never claims to change feasibility.
   - `UnsupportedFeaturePolicy` defaults to rejection; the explicit conversion variants `ConvertHintToStart` and `ConvertStartToTemporaryFixing` exist (SM-08.5); default is `Reject` (SM-08.4).
   - `SolvePlan::validate(&Model)` rejects, before any backend call: a `MipStart` from another lineage; a stale variable; a non-finite or out-of-bounds assignment value; a duplicate variable across two starts; a variable present in both a start and the overlay's `temporary_fixings`; a non-finite hint value; and a `RejectIncomplete` start that omits an integer/binary variable.
   - Basis distinctness (SM-08.6): no `MipStart`/`VariableHints` API touches a basis type; a source assertion that the basis features (`InitialBasis`) are not exercised by any start/hint path.
3. Run the tests and record the expected failures (missing types).
4. Implement:
   - `src/solver/plan.rs`: `pub struct SolvePlan { pub options: SolveOptions, pub overlay: SolveOverlay, pub mip_starts: Vec<MipStart>, pub hints: VariableHints, pub objective_override: Option<ObjectivePolicy>, pub lex_stage_policy: LexStagePolicy, pub unsupported: UnsupportedFeaturePolicy }` (packet shape, design §12); `SolvePlan::new(options) -> Result<Self, IdentityOverflow>` building the empty overlay via `SolveOverlay::new(empty temporary_fixings, empty locks, empty objective_locks, empty cutoffs)`; `MipStart { assignment, repair, name }` + `MipStart::new`; `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum RepairPolicy { BackendDefault, RejectIncomplete, AllowRepair }`; `VariableHints` (private `BTreeMap<Variable, VariableHint>` per the packet; `Default`, `get`, `insert`, `iter`, `is_empty`, `len`); `VariableHint { value: f64, priority: HintPriority }`; `pub struct HintPriority(pub i32)`; `#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)] pub enum UnsupportedFeaturePolicy { #[default] Reject, ConvertHintToStart, ConvertStartToTemporaryFixing }`; a `PlanError` enum (design §19 "solve-plan conflict" family) whose variants wrap `AssignmentError` and add `DuplicateStartVariable { variable }`, `OverlayConflict { variable }`, `NonFiniteHintValue { variable, value }`, `IncompleteStart { missing: Vec<Variable> }`, `UnsupportedFeature { feature: &'static str, policy: UnsupportedFeaturePolicy }`; and `SolvePlan::validate(&self, model: &Model) -> Result<(), PlanError>` that reuses `PrimalAssignment::validate_for` and adds the conflict/duplicate/completeness checks.
   - The minimal forward-declared scaffolding `SolvePlan` needs for its design-§12 fields: `#[non_exhaustive] pub enum ObjectivePolicy { Single(Objective) }` and `pub enum LexStagePolicy { RequireOptimal, UseBestFeasible }` (packet "Assignments and solve intent" shapes). P31 owns the full policy semantics in `src/objective_policy.rs` and extends `ObjectivePolicy` with `Weighted`/`Lexicographic` — record this forward-declaration (A30/A32 extension-surface precedent) in the evidence so P31 does not collide.
   - Wire `pub mod plan;` in `src/solver/mod.rs`; re-export `SolvePlan`, `MipStart`, `VariableHints`, `VariableHint`, `HintPriority`, `RepairPolicy`, `UnsupportedFeaturePolicy`, and `PlanError` through `src/lib.rs`/`src/advanced.rs` (public types per SM-07.1; keep the ordinary prelude minimal — mirror how `solver::overlay` and `assignment` are exported).
5. Run the Task 1 verification commands (below); update evidence and traceability; commit one coherent unit.

- [ ] Prove `SolvePlan` combines options, overlay, starts, hints, objective override, continuation, and unsupported policy (SM-07.1).
- [ ] Validate lineages, entities, finite values, conflicts, duplicates, and conversion policy before backend mutation (packet Task 10 bullet, verbatim).
- [ ] Implement `MipStart` (full or partial) with explicit repair policy (SM-08.1) and `VariableHints` with priorities (SM-08.3).
- [ ] Make default unsupported behavior reject (SM-08.4) and conversions explicit (SM-08.5).
- [ ] Keep starts/hints/locks/fixings/basis semantically distinct (D8, SM-08.6).
- [ ] Commit as `feat(solve): add solve plan starts and hints types`.

**Stopping condition (packet, verbatim):** validate lineages, entities, finite values, conflicts, duplicates, and conversion policy before backend mutation — a `SolvePlan::validate` path rejects every listed class with a typed `PlanError` and no backend call is reachable with an invalid plan.

**Commit:** `feat(solve): add solve plan starts and hints types`

**Verification:**

```bash
cargo fmt --all -- --check
cargo test -p roml --test solve_plan
cargo test -p roml --all-targets
cargo clippy -p roml --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
cargo public-api -p roml
```

**Acceptance criteria:**
- All six commands exit 0.
- `src/solver/plan.rs` defines `pub struct SolvePlan { pub options: SolveOptions, pub overlay: SolveOverlay, pub mip_starts: Vec<MipStart>, pub hints: VariableHints, pub objective_override: Option<ObjectivePolicy>, pub lex_stage_policy: LexStagePolicy, pub unsupported: UnsupportedFeaturePolicy }` with the exact design-§12 field identifiers.
- `src/solver/plan.rs` defines `MipStart { assignment: PrimalAssignment, repair: RepairPolicy, name: Option<String> }`, `RepairPolicy { BackendDefault, RejectIncomplete, AllowRepair }`, `VariableHints`, `VariableHint { value: f64, priority: HintPriority }`, `HintPriority(pub i32)`, and `UnsupportedFeaturePolicy` whose default is `Reject` (source assertion: `#[default]` on `Reject`).
- `SolvePlan::validate(&Model)` returns a typed `PlanError` for lineage mismatch, stale variable, non-finite/out-of-bounds value, duplicate variable across starts, overlay/start conflict, non-finite hint value, and `RejectIncomplete` partial integer coverage — each covered by a test in `tests/solve_plan.rs`.
- The forward-declared `ObjectivePolicy::Single`/`LexStagePolicy` scaffolding is documented in the evidence as a P31 extension surface.

## Task 2 — Route all solve façades through one plan executor and record effective plans

**Phase:** P28  **Requirements:** SM-07.1 (executor), SM-07.2, SM-07.7, SM-08.3 (feasibility proof), SM-08.5 (recording), SM-04.5

**Read first:**
- `docs/superpowers/specs/2026-08-02-semantic-modeling-and-solve-workflows-design.md` §12 "SolvePlan and reversible overlays" (the lifecycle block: commit → compile/synchronize → validate plan → compile overlay → apply overlay → apply starts/hints → execute objective stages → extract result → rollback → verify), §15.2 "Lexicographic policy" (effective-plan reporting), §19 "Failure semantics".
- `docs/superpowers/plans/2026-08-02-semantic-modeling-and-solve-workflows.md` — "Solve plan and exact result identity" interface contract (`EffectiveSolvePlan { applied_features, adjustments, rejections, objective_stages }`; `SolveMetadata` adds lineage/instance/revision/compilation identity).
- `src/solver/facade.rs` (P27 branch content) — `solve`/`solve_with`/`solve_with_overlay`, the `C_base`→`C_overlay` lifecycle, the extraction `CompilationId` gate, and `normalize_result`.
- `src/solver/session.rs` (P27 branch content) — `OverlaySession` (the trait the warm-start methods extend).
- `src/solution/metadata.rs` — `SolveMetadata` (the record that gains `effective_plan`).
- `src/solver/error.rs` — `SolveError` (add the plan-validation error mapping).
- Task 1 artifacts (`src/solver/plan.rs`).

**TDD order** (per `EXECUTION.md`):

1. Write failing tests in `tests/solve_plan.rs`:
   - **Equivalence (packet bullet):** on one reference-backend fixture and one HiGHS fixture, `solve()` == `solve_with(SolveOptions::default())` == `solve_plan(SolvePlan::new(SolveOptions::default())?)` — identical status, objective value, primal values, `SynchronizationMode`, revision, and `compilation_id` in metadata.
   - **Single executor:** a source assertion that `solve` and `solve_with` delegate to one `solve_plan`/executor path (no divergent code path — grep the facade for a single plan-execution entry point).
   - **Metadata recording (SM-07.7, SM-04.5):** every real solve's `SolveMetadata` carries `effective_plan: EffectiveSolvePlan` with the exact `CompilationId`; applied starts and conversions appear in `applied_features`/`adjustments`; default rejections appear in `rejections`; `objective_stages` is present and empty in P28.
   - **Feasibility signature (SM-08.3):** solve a MIP with a valid `MipStart` and a `VariableHints` set, and again without them — the optimal objective and status are identical, `model.current_revision()` is unchanged, and the compiled base `CompilationId` reported by the backend is unchanged (starts/hints never alter canonical or compiled feasible-region signatures).
   - **No stale-start leakage:** solve with a start, then modify the model and solve without a start; the second solve is deterministic and equal to a fresh no-start solve of the modified model (a stale incumbent must not seed an unrelated solve).
   - **Conversion recording:** with `UnsupportedFeaturePolicy::ConvertStartToTemporaryFixing`, a start against a backend without native `MipStart` converts into overlay `temporary_fixings` and the conversion is recorded in the effective plan; `ConvertHintToStart` behaves symmetrically where the backend qualifies `MipStart`.
2. Run the tests and record the expected failures.
3. Implement:
   - `src/solver/effective_plan.rs`: `pub struct EffectiveSolvePlan { pub applied_features: Vec<AppliedFeature>, pub adjustments: Vec<PlanAdjustment>, pub rejections: Vec<PlanRejection>, pub objective_stages: Vec<ObjectiveStageResult> }` (packet shape, `#[derive(Clone, Debug, Default, PartialEq)]`); `AppliedFeature { feature: String, detail: String }` (native features/bridges selected this solve — SM-04.5); `PlanAdjustment { key: String, requested: String, applied: String, reason: String }` (conversions and adjustments, SM-08.5); `PlanRejection { key: String, reason: String }`; `ObjectiveStageResult { stage: usize, objective: Option<ObjId>, value: Option<f64>, status: SolveStatus }` (empty in P28; P31 populates).
   - `src/solver/session.rs`: extend `OverlaySession` with two default methods that reject unless overridden — `fn apply_mip_starts(&mut self, starts: &[MipStart]) -> Result<(), BackendError>` and `fn apply_variable_hints(&mut self, hints: &VariableHints) -> Result<(), BackendError>`, each returning `BackendError::new(..., ErrorCategory::Unsupported, HealthEffect::Recoverable)` by default. Default-reject means a backend that does not qualify starts/hints needs no change and can never silently ignore them (SM-08.4).
   - `src/solver/facade.rs`: one plan executor `SolverSession::solve_plan(&mut self, model: &mut Model, plan: SolvePlan) -> Result<Solution, SolveError>` implementing the design §12 lifecycle: (0) validate options and request against the typed capability set exactly as `solve_with` does today; (1) `plan.validate(model)` → typed `SolveError::Plan` before any synchronization; (2) resolve starts/hints against the backend's `typed_capabilities()` and the plan's `UnsupportedFeaturePolicy` — reject with a typed error by default, or apply an explicit conversion and record it; (3) commit + synchronize to `C_base` (reuse the existing path); (4) compile the overlay (if non-empty) against the exact base and apply it; (5) apply qualified starts/hints through the new `OverlaySession` methods (record applied features); (6) solve; (7) enforce the exact `CompilationId` extraction gate (a result tagged with anything other than the overlay-compounded id or `C_base` is a typed `CompilationMismatch`); (8) rollback the overlay with the existing `rollback_and_verify`; (9) build the `EffectiveSolvePlan` and thread it through `normalize_result` into `SolveMetadata`. Refactor `solve`/`solve_with`/`solve_with_overlay` to construct a `SolvePlan` and call the single executor (D27: keep signatures and observable behavior source-compatible).
   - `src/solution/metadata.rs`: add `pub effective_plan: EffectiveSolvePlan` to `SolveMetadata` (default `EffectiveSolvePlan::default()`); thread it through `normalize_result`.
   - `src/solver/mod.rs`/`src/lib.rs`/`src/advanced.rs`: wire `pub mod effective_plan;` and re-export `EffectiveSolvePlan`/`AppliedFeature`/`PlanAdjustment`/`PlanRejection`/`ObjectiveStageResult`.
4. Run the Task 2 verification commands; update evidence (record RED failures, equivalence results, feasibility proof); commit.

- [ ] Prove `solve`, `solve_with`, and empty `solve_plan` equivalence (packet Task 10 bullet, verbatim).
- [ ] Route all solve façades through one plan executor (packet Task 10 bullet, verbatim).
- [ ] Record applied/converted/rejected features and exact compilation ID in solution metadata (packet Task 10 bullet, verbatim; SM-04.5, SM-07.7).
- [ ] Prove starts/hints leave canonical and compiled feasible-region signatures unchanged (packet Task 10 bullet, verbatim; SM-08.3).
- [ ] Commit as `feat(solve): route solve façades through one plan executor`.

**Stopping condition (packet, verbatim):** `solve`, `solve_with`, and empty `solve_plan` are proven equivalent; all solve façades route through one plan executor; applied/converted/rejected features and the exact `CompilationId` are recorded in solution metadata; starts/hints leave canonical and compiled feasible-region signatures unchanged.

**Commit:** `feat(solve): route solve façades through one plan executor`

**Verification:**

```bash
cargo fmt --all -- --check
cargo test -p roml --test solve_plan
cargo test -p roml --all-targets
cargo clippy -p roml --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
cargo public-api -p roml
```

**Acceptance criteria:**
- All six commands exit 0 (including the P27 and M2 suites — overlay and convenience behavior not regressed, D27).
- `src/solver/facade.rs` defines `pub fn solve_plan(&mut self, model: &mut Model, plan: SolvePlan) -> Result<Solution, SolveError>` and `solve`/`solve_with`/`solve_with_overlay` delegate to it (source assertion: each convenience method constructs a `SolvePlan` and calls the executor).
- `src/solver/effective_plan.rs` defines `EffectiveSolvePlan { applied_features, adjustments, rejections, objective_stages }` with the packet field identifiers.
- `src/solution/metadata.rs` `SolveMetadata` has `pub effective_plan: EffectiveSolvePlan` (SM-07.7).
- `tests/solve_plan.rs` proves equivalence, feasibility-signature invariance (same optimum, same canonical revision, same compiled base id), and no-stale-start determinism.
- Conversions are recorded in `EffectiveSolvePlan` (`adjustments`/`applied_features`), never silent (SM-08.5).

## Task 3 — Audit pinned HiGHS start/hint APIs and implement only qualified support

**Phase:** P28  **Requirements:** SM-08.7, SM-08.2, SM-08.4, SM-08.5 (execution), SM-08.6 (basis untouched), SM-04.5 (backend recording)

**Read first:**
- The pinned official header: `~/.cargo/registry/src/index.crates.io-*/highs-sys-1.15.0/HiGHS/highs/interfaces/highs_c_api.h` — the audit target. The exact line numbers are version-bound; the symbols to inspect are `Highs_setSolution` ("Set a solution by passing the column and row primal and dual solution values. For any values that are unavailable, pass NULL."), `Highs_setSparseSolution` ("Set a partial primal solution by passing values for a set of variables" — `num_entries`, `index`, `value`), `Highs_setBasis`/`Highs_setLogicalBasis`, and the confirmed ABSENCE of `Highs_setMipStart`/`Highs_clearMipStart`/any variable-hint symbol in this bundled version. Return convention: `HighsInt` = `kHighsStatus` (`kHighsStatusError` = -1, `kHighsStatusOk` = 0, `kHighsStatusWarning` = 1).
- `.planning/milestones/M3-semantic-modeling-workflows/EXECUTION.md` — § "Native API research protocol" (record in `docs/knowledge/`; add compile-time/version characterization tests; implement through existing official/generated binding boundaries; qualify absence as typed unsupported; never infer signatures).
- `.planning/milestones/M3-semantic-modeling-workflows/DECISIONS.md` — D10 (typed version-aware capabilities), D19 (qualify from pinned official APIs; unsupported versions return typed `Unsupported`), D8, and SM-08.6/SM-08.7 text in `REQUIREMENTS.md`.
- `src/compiler/capability.rs` — `BackendFeature::{MipStart, PartialMipStart, MultipleMipStarts, VariableHints, InitialBasis}`, `FeatureSupport`, `FeatureLimitations`, `SupportLevel`.
- `roml-highs/src/session.rs` (P27 branch content) — `highs_capability_set`, `M2_NATIVE_FEATURES`, `UNQUALIFIED_M3_FEATURES`, the `OverlaySession` impl (the apply/rollback/verify pattern the start path mirrors), and the compiled-keyed maps (`compiled_to_user_*`) the start application maps through.
- `roml-highs/src/compiler.rs` and `docs/migration/M3_BACKEND_IR.md` — how compiled IR maps to native column indices (the mapping `MipStart` values must traverse: user `Variable` → compiled id → native index).
- `src/solver/plan.rs` (Task 1) and `src/solver/facade.rs` (Task 2) — the executor's warm-start call site and the `EffectiveSolvePlan` the backend contributes to.

**TDD order** (per `EXECUTION.md`):

1. **Audit (explicit task item — the STATE-ledger blocking decision).** Inspect the exact pinned bundled/system official headers and record in `docs/knowledge/highs_mip_start_api.md`: symbol signatures, availability, return codes, lifecycle, documented semantics, and version availability for: full MIP starts, partial starts, clearing starts, multiple starts, hints, and basis warm starts. Preliminary finding to verify, not to assume: the bundled `highs-sys` 1.15.0 header exposes `Highs_setSolution`/`Highs_setSparseSolution` as the primal warm-start primitives and contains NO `Highs_setMipStart`/`Highs_clearMipStart`/hint symbols; the CI system floor is HiGHS 1.9.0. Record the lifecycle question explicitly: whether a set solution persists across solves and how it is cleared, because the executor's no-stale-leak test depends on the answer. Do NOT write any production code against an API you have not read in the header.
2. Write failing characterization and behavior tests in `roml-highs/tests/solve_plan.rs`:
   - End-to-end: a MIP solved via `SolvePlan::new(SolveOptions::default())` equals `solve_with` on the HiGHS backend (equivalence at the backend).
   - Capability matrix: `highs_capability_set` declares the start/hint features exactly as the audit qualifies them — e.g. `MipStart`/`PartialMipStart` qualified as `Native` with `FeatureLimitations` notes (version range, sparse-semantics note) if the audit approves `Highs_setSparseSolution`, and `MultipleMipStarts`/`VariableHints`/`InitialBasis` remain `Unsupported` with the audit note (SM-08.2, SM-08.6, SM-08.7).
   - Default rejection (SM-08.4): a plan requesting starts or hints against the pinned backend returns a typed `SolveError` unless the feature is qualified — nothing is silently ignored. In particular, `VariableHints` requests reject by default (absent hints reject by default — the blocking decision).
   - Conversions (SM-08.5): `ConvertStartToTemporaryFixing` applies the start values as overlay `temporary_fixings` and the conversion appears in the returned `EffectiveSolvePlan`; `ConvertHintToStart` converts hints into a `MipStart` only when `MipStart` is qualified, and records the conversion; otherwise the request rejects.
   - Feasibility (SM-08.3): a HiGHS MIP solved with and without a qualified start yields the same optimal objective, the same canonical revision, and the same compiled base id.
   - Return codes: a failed `Highs_setSparseSolution` (e.g. an index/value the backend rejects) maps to a typed `BackendError`, never a panic or unchecked return.
   - No stale start: after a solve with a start, a subsequent solve of a changed model is deterministic and equal to a fresh no-start solve.
3. Run the tests and record the expected failures.
4. Implement only qualified support:
   - `roml-highs/src/start.rs`: apply qualified starts through `Highs_setSparseSolution` (mapping each start's user `Variable` values through the compiled-keyed origin maps to native column indices), checking every return code via the existing `check_highs_status` pattern; partial starts only when the audit qualifies `PartialMipStart`; `RepairPolicy::RejectIncomplete` enforced in the executor/Task 1 validation, `AllowRepair`/`BackendDefault` passed through per the audit's documented HiGHS behavior.
   - `roml-highs/src/session.rs`: update `highs_capability_set` so the qualified features carry `FeatureSupport::native`/`unsupported` with `FeatureLimitations` notes citing the audit record and version range; keep unqualified features typed `Unsupported`; implement `apply_mip_starts`/`apply_variable_hints` (from the Task 2 contract) — hints stay default-reject unless a conversion applies.
   - `roml-highs/src/lib.rs`: re-export the start module surface if public.
   - Wire the backend's applied/converted/rejected records into the `EffectiveSolvePlan` the facade returns (SM-04.5, SM-08.5).
5. Run the Task 3 verification commands; update the evidence file with the audit findings, capability table, RED failures, and full HiGHS lane results; commit.

- [ ] Audit exact bundled/system official HiGHS APIs for starts, partial starts, clearing, multiple starts, hints, return codes, and version availability (packet Task 10 bullet, verbatim — the STATE-ledger blocking decision).
- [ ] Implement only qualified support; absent hints reject by default (packet Task 10 bullet, verbatim).
- [ ] Prove starts/hints leave canonical and compiled feasible-region signatures unchanged on HiGHS (packet Task 10 bullet, verbatim).
- [ ] Never simulate silently (blocking decision): an unqualified feature is typed `Unsupported`, never faked.
- [ ] Record applied/converted/rejected features in solution metadata at the backend (SM-04.5, SM-08.5).
- [ ] Commit as `feat(highs): qualify mip start support and reject absent hints`.

**Stopping condition (packet, verbatim):** audit exact bundled/system official HiGHS APIs for starts, partial starts, clearing, multiple starts, hints, return codes, and version availability; implement only qualified support; absent hints reject by default; prove starts/hints leave canonical and compiled feasible-region signatures unchanged.

**Commit:** `feat(highs): qualify mip start support and reject absent hints`

**Verification:**

```bash
cargo fmt --all -- --check
cargo test -p roml-highs --test solve_plan
cargo test -p roml-highs --all-targets
cargo test -p roml --all-targets
cargo clippy -p roml-highs --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps
cargo public-api -p roml-highs
```

**Acceptance criteria:**
- All seven commands exit 0.
- `docs/knowledge/highs_mip_start_api.md` exists and records, for the pinned bundled/system headers: symbol signatures, availability, return codes, lifecycle (including start persistence/clearing), and version availability for starts/partial starts/multiple starts/hints/basis (SM-08.7).
- `highs_capability_set` declares the start/hint features exactly per the audit — unqualified features (`VariableHints`, `MultipleMipStarts`, `InitialBasis` when the audit finds no API) are `Unsupported` with a note citing the audit record (SM-08.2, SM-08.6).
- `roml-highs/tests/solve_plan.rs` proves default rejection, the qualified-start path (if the audit approves it), conversion recording, feasibility-signature invariance, and checked return codes.
- A request for absent hints rejects by default with a typed error (source assertion: the default `UnsupportedFeaturePolicy::Reject` + backend `Unsupported` capability path); nothing is silently simulated.

## Verification

Phase-level checks (all must exit 0; per-crate only — never workspace-wide):

```bash
cargo fmt --all -- --check
cargo test -p roml --test solve_plan
cargo test -p roml --all-targets
cargo test -p roml-highs --test solve_plan
cargo test -p roml-highs --all-targets
cargo clippy -p roml --all-targets -- -D warnings
cargo clippy -p roml-highs --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps
cargo public-api -p roml
cargo public-api -p roml-highs
```

Per `EXECUTION.md` P27–P28 mandatory checks: overlay failure injection (inherited from P27 — must still pass); model revision invariance under temporary operations; subsequent-solve leak tests (Task 2/3 no-stale-start); capability/effective-plan assertions. Baseline matrix and package qualification:

```bash
cargo check -p roml --all-targets
cargo check -p roml-highs --all-targets
cargo package --list -p roml
cargo package --list -p roml-highs
```

Skips must be recorded in the evidence file, never treated as passing.

## Waves and parallelization

P28 is one phase plan with three strictly serial tasks. The three tasks share files (`src/solver/mod.rs`, `src/lib.rs`, `src/advanced.rs` across Tasks 1–2; `src/solver/facade.rs` and `src/solver/session.rs` are Task 2 only), and each task's output is a compile-time input to the next:

- **Task 1 (types) → Task 2 (executor) → Task 3 (HiGHS).** Task 2 uses Task 1's `SolvePlan`/`MipStart`/`VariableHints`/`UnsupportedFeaturePolicy` types; Task 3 implements Task 2's `OverlaySession` warm-start methods and consumes the `EffectiveSolvePlan` metadata contract. Running any of them in parallel produces overlapping edits on shared files or compiles against types that do not exist yet.

**The one permitted parallel sub-step:** the *research* half of Task 3 — inspecting the pinned headers and drafting `docs/knowledge/highs_mip_start_api.md` — is content-independent and MAY run in parallel with Tasks 1–2, exactly as `EXECUTION.md` permits ("research/header audits may run in parallel but may not land production code against speculative APIs"). The *implementation* half of Task 3 (capability-declaration changes, `roml-highs/src/start.rs`, and the end-to-end `roml-highs/tests/solve_plan.rs`) is strictly after Task 2.

If the executor prefers fully serial execution, Task 1 → Task 2 → Task 3 is correct and conflict-free (matching `D26 — One active implementation phase by default`).

## Review gates

Per `EXECUTION.md` § "Review gates", P28 receives two independent review passes at the phase boundary (after Task 3).

- **Pass 1 — Specification and correctness:** requirement coverage (SM-07.1–07.2, SM-07.7, SM-08.1–08.7, SM-04.5); semantic correctness of the distinct start/hint/lock/fixing/basis semantics (D8); invariant preservation (default rejection SM-08.4; conversions explicit and recorded SM-08.5; exact `CompilationId` extraction gate; starts/hints never change the feasible region); unsupported/error behavior (typed `PlanError`/`SolveError`, typed capability rejections); official backend evidence (`docs/knowledge/highs_mip_start_api.md` derived from the pinned header, never inferred); test quality (equivalence, feasibility, leak, conversion tests).
- **Pass 2 — Integration and operations:** incremental/rebuild behavior (no canonical revision advance, no compiled-identity change from starts/hints); failure recovery (start-application failure maps to typed error; no stale-start leak); cross-platform/version behavior (HiGHS capability declarations with version notes; bundled 1.15.0 vs CI floor 1.9.0); public API diff (`cargo public-api -p roml`, `-p roml-highs`); package/docs impact (`docs/knowledge/`, evidence file); migration accuracy; `solve`/`solve_with` M2 source compatibility (D27).

**Blocking rules:**

- P0/P1 findings **block merge**.
- P2 findings may merge only when explicitly accepted and scheduled.
- `autonomous: false` — the executor pauses after Task 3 and does not declare the phase complete until both review passes resolve to no P0/P1 findings.
- The HiGHS audit record is itself review input: the reviewer verifies every capability declaration traces to a symbol read in the pinned header and that no unqualified feature is simulated.

Evidence requirement: `docs/release/evidence/M3_P28_SOLVE_PLAN_STARTS_HINTS.md` must record the baseline, the audit findings, per-task verification with RED failures, the full verification matrix, the public API diff, and reviewer findings and dispositions before the gate result is marked pass (per `EXECUTION.md` § "Evidence file structure"). The evidence must also close, at clause level, SM-07.1, SM-07.2, SM-07.7, SM-08.1–08.7, and SM-04.5 (SM-04.5 was deferred from P26 to P28 per `TRACEABILITY.md`).

## Artifacts this phase produces

New modules and symbols (names/signatures from the approved design §12, §15 and the packet's interface contract):

- `src/solver/plan.rs` — `SolvePlan { options, overlay, mip_starts, hints, objective_override, lex_stage_policy, unsupported }`, `MipStart { assignment, repair, name }`, `RepairPolicy { BackendDefault, RejectIncomplete, AllowRepair }`, `VariableHints` (private `BTreeMap<Variable, VariableHint>` + accessors), `VariableHint { value, priority }`, `HintPriority(pub i32)`, `UnsupportedFeaturePolicy { Reject (default), ConvertHintToStart, ConvertStartToTemporaryFixing }`, `PlanError` (assignment wrap + duplicate/conflict/non-finite/incomplete/unsupported variants), `SolvePlan::validate(&Model)`, forward-declared `#[non_exhaustive] ObjectivePolicy::Single` and `LexStagePolicy { RequireOptimal, UseBestFeasible }` (P31 extension surface).
- `src/solver/effective_plan.rs` — `EffectiveSolvePlan { applied_features, adjustments, rejections, objective_stages }`, `AppliedFeature`, `PlanAdjustment`, `PlanRejection`, `ObjectiveStageResult`.
- `src/solver/session.rs` — `OverlaySession` default-reject methods `apply_mip_starts(&[MipStart])` and `apply_variable_hints(&VariableHints)`.
- `src/solver/facade.rs` — `SolverSession::solve_plan` (the single plan executor); `solve`/`solve_with`/`solve_with_overlay` route through it.
- `src/solution/metadata.rs` — `SolveMetadata::effective_plan: EffectiveSolvePlan`.
- `roml-highs/src/start.rs` — qualified start application through `Highs_setSparseSolution` (audit-approved) with checked return codes.
- `roml-highs/src/session.rs` — audit-derived capability declarations for `MipStart`/`PartialMipStart`/`MultipleMipStarts`/`VariableHints`/`InitialBasis`.
- Modified wiring/exports: `src/solver/mod.rs`, `src/lib.rs`, `src/advanced.rs`, `roml-highs/src/lib.rs`; `src/assignment.rs` only if a conversion helper lands there.
- Test files: `tests/solve_plan.rs`; `roml-highs/tests/solve_plan.rs`.
- Docs/evidence: `docs/knowledge/highs_mip_start_api.md`; `docs/release/evidence/M3_P28_SOLVE_PLAN_STARTS_HINTS.md`.

Conversion-policy records (SM-08.5) are realized as `PlanAdjustment`/`AppliedFeature` entries in the `EffectiveSolvePlan` carried by `SolveMetadata`; they are never silent.

## must_haves

Goal-backward verification (from the ROADMAP P28 gate, verbatim):

**Truths (observable behaviors):**

1. **default unsupported behavior rejects** — a plan that requests starts or hints the backend does not qualify returns a typed error by default; nothing is silently ignored (SM-08.4).
2. **all applications/conversions/rejections are recorded** — every solve's `Solution` metadata carries an `EffectiveSolvePlan` (applied features, adjustments/conversions, rejections) plus the exact `CompilationId` (SM-04.5, SM-07.7).
3. **HiGHS behavior is qualified** — every start/hint capability declaration traces to the pinned official header audit; unqualified features are typed `Unsupported`; absent hints reject (SM-08.7).
4. **solve and solve_with remain compatible** — `solve()`/`solve_with()` and an empty `SolvePlan` are proven equivalent and M2 source usage compiles unchanged (D27, SM-07.2).

**Artifacts (files that must exist):**

- `src/solver/plan.rs`, `src/solver/effective_plan.rs`
- `src/solution/metadata.rs` with `SolveMetadata::effective_plan`
- `src/solver/facade.rs` with `solve_plan` (single executor)
- `roml-highs/src/start.rs`, `roml-highs/src/session.rs` (qualified declarations)
- `tests/solve_plan.rs`, `roml-highs/tests/solve_plan.rs`
- `docs/knowledge/highs_mip_start_api.md`
- `docs/release/evidence/M3_P28_SOLVE_PLAN_STARTS_HINTS.md`

**Key links (critical connections where breakage cascades):**

- **Plan executor ↔ backend typed capabilities** — an unqualified start/hint request is rejected before any backend mutation; the executor consults `typed_capabilities()` plus `UnsupportedFeaturePolicy` (SM-04.4, SM-08.4).
- **SolveMetadata.effective_plan ↔ EffectiveSolvePlan ↔ exact CompilationId** — the recorded plan and the compiled-state identity must agree (SM-04.5, SM-07.7); a result tagged with the wrong id is a typed `CompilationMismatch`.
- **C_base / C_overlay / result.compilation_id** — starts/hints never alter the compiled identity or canonical revision; the feasibility proof asserts the same optimum, revision, and compiled base id (SM-08.3).
- **UnsupportedFeaturePolicy → recorded conversions** — a conversion is applied only through an explicit policy and is recorded in the effective plan; never silent (SM-08.5).
- **HiGHS audit findings → capability declarations → reject-by-default** — every declaration is derived from a pinned official header; absence is typed unsupported, never simulated (SM-08.7, D19).

## Threat model

This is a modeling library; P28 introduces no network, filesystem, auth, or untrusted-input surface. The relevant boundaries are integrity/invariant boundaries:

| Boundary | Description | Mitigation in this phase |
|----------|-------------|--------------------------|
| solve intent → plan executor | an invalid or unsupported plan must not reach the backend | `SolvePlan::validate` + typed-capability preflight before any mutation; default rejection (SM-08.4) |
| plan executor → `OverlaySession` warm-start methods | a backend without qualified support must not silently drop starts/hints | default methods return `BackendError` with `ErrorCategory::Unsupported`; conversions only via explicit policy and recorded (SM-08.5) |
| roml-highs start/hint FFI calls | no panic/UB across C; every native return code checked; no inferred signatures | `roml-highs/src/start.rs` routes through `check_highs_status`; signatures derived from the pinned official header audit, never guessed (D19) |
| stale solve intent | a start from one solve must not silently seed an unrelated later solve | no-stale-start determinism test; start lifecycle recorded in the audit |
| compiled-identity authority | a result from a different compiled state must never be accepted | exact `CompilationId` extraction gate unchanged (F2); starts/hints never alter `C_base`/`C_overlay` identity |

STRIDE register:

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-28-01 | Tampering | roml-highs `Highs_setSparseSolution` FFI call | high | mitigate | checked return codes via existing `check_highs_status`; arguments (indices/values) mapped through compiled-keyed origin maps; no `unsafe` added beyond the existing `highs-sys` boundary |
| T-28-02 | Spoofing | `SolveResult.compilation_id` vs `C_base`/`C_overlay` | high | mitigate | existing exact-`CompilationId` mismatch gate; starts/hints apply after the compiled base is established and never change it |
| T-28-03 | Repudiation | conversion/rejection records in metadata | medium | mitigate | every applied/converted/rejected feature is recorded in `EffectiveSolvePlan` (SM-04.5); no silent conversion path |
| T-28-04 | Elevation | unsupported feature bypass | medium | mitigate | default `UnsupportedFeaturePolicy::Reject`; capability preflight before backend mutation; unqualified features typed `Unsupported` |
| T-28-05 | Tampering | unsupported/implied FFI signatures | high | mitigate | audit-driven declarations only; `docs/knowledge/highs_mip_start_api.md` records every symbol; never infer layouts/constants from another version |

No new `unsafe`, environment mutation, filesystem scan, or unsolicited stdout is introduced by this phase.

## Gate

P28 passes when:

- Task 1 defines `SolvePlan`, `MipStart`, `VariableHints`, `HintPriority`, `UnsupportedFeaturePolicy`, and plan validation (lineages, entities, finite values, conflicts, duplicates, conversion policy) before backend mutation (SM-07.1, SM-08.1–08.6, SM-04.5 foundations);
- Task 2 routes `solve`/`solve_with`/`solve_with_overlay` through one plan executor, proves `solve`/`solve_with`/empty-`solve_plan` equivalence, records `EffectiveSolvePlan` plus exact `CompilationId` in `SolveMetadata`, and proves starts/hints leave canonical and compiled feasible-region signatures unchanged (SM-07.2, SM-07.7, SM-04.5, SM-08.3);
- Task 3 audits the pinned bundled/system official HiGHS start/hint APIs into `docs/knowledge/highs_mip_start_api.md`, implements only qualified support, rejects absent hints by default, and never simulates silently (SM-08.7, SM-08.2, SM-08.4);
- all phase-level verification commands exit 0 (both `solve_plan` integration tests, both `--all-targets` suites, both clippy lanes, rustdoc with warnings denied, both `cargo public-api` captures);
- the ROADMAP P28 gate holds: default unsupported behavior rejects; all applications/conversions/rejections are recorded; HiGHS behavior is qualified; `solve` and `solve_with` remain compatible;
- SM-04.5 is closed in P28 (deferred from P26 per `TRACEABILITY.md`), and SM-07.7's `objective_stages` is explicitly declared empty pending P31;
- the evidence bundle, the HiGHS audit record, and the public API diff are recorded; and
- both independent review passes resolve with no P0/P1 findings.

No crate publication, tag, or release is part of this phase (SM-15.8 / M3 stopping condition).

## Output

Create `.planning/phases/28-solve-plan-warm-starts/28-SUMMARY.md` when done, per the phase completion protocol.
