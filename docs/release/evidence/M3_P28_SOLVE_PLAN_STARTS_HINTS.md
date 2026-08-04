# P28 Evidence — SolvePlan, Starts, Hints, and Effective-Plan Reporting

## Scope and requirements

Phase P28 closes the M3 solve-attempt contract (packet Task 10): one explicit
`SolvePlan` type combining options, overlays, starts, hints, objective overrides,
and unsupported-feature policy; one plan executor routing `solve`/`solve_with`/
`solve_with_overlay`/`solve_plan`; effective-plan reporting on every solve; and
a qualified HiGHS start/hint implementation derived from the pinned official
header audit.

Requirements closed in this phase: SM-07.1, SM-07.2, SM-07.7, SM-08.1–08.7,
SM-04.5 (deferred from P26 per TRACEABILITY.md).

## Baseline and environment

- Branch: `phase-roml-P28-solve-plan-warm-starts`
- Base: `main@40af9f4` (repaired main following the P27/P32 restoration)
- HEAD at execution start: `d2fdbf0` (`docs(28): add solve plan warm starts phase plan` — docs-only plan commit on top of the base)
- Rust: `rustc 1.97.1 (8bab26f4f 2026-07-14)`, `cargo 1.97.1 (c980f4866 2026-06-30)`
- Toolchain: `stable-aarch64-apple-darwin` (default)
- Target/OS: `aarch64-apple-darwin` on Darwin 25.4.0 (arm64)
- HiGHS: bundled `highs-sys 1.15.0` (default feature); CI system floor 1.9.0

### Untouched baseline captures

| Capture | Command | Result |
|---------|---------|--------|
| roml public API | `cargo public-api -p roml` | exit 0 (raw artifact `M3_P28_public_api_roml_baseline.txt`) |
| roml-highs public API | `cargo public-api -p roml-highs` | exit 0 (raw artifact `M3_P28_public_api_roml_highs_baseline.txt`) |
| roml package list | `cargo package --list -p roml` | exit 0 (`M3_P28_package_roml_baseline.txt`) |
| roml-highs package list | `cargo package --list -p roml-highs` | exit 0 (`M3_P28_package_roml_highs_baseline.txt`) |
| roml check | `cargo check -p roml --all-targets` | exit 0 |

Baseline public API: the core crate exports the P27 solve-overlay surface
(`SolveOverlay`, `OverlayError`, `PrimalAssignment`, `SolutionLock`, lock
selectors/continuous locks) plus the M2 solver façade surface
(`SolverSession::solve`/`solve_with`/`solve_with_overlay`, `SolveOptions`,
`SolveError`, `SolveStatus`). roml-highs exports `Highs`, `HighsError`,
`HighsSession`, `highs_capability_set`, and `HighsInt`. No `SolvePlan`,
`MipStart`, `VariableHints`, `UnsupportedFeaturePolicy`, `EffectiveSolvePlan`,
or warm-start capability declarations exist in the baseline surface.

## Commit trail

- `40ef027` — `feat(solve): add solve plan starts and hints types` (Task 1)
- `286c6c7` — `feat(solve): route solve façades through one plan executor` (Task 2)
- `cd46ebb` — `feat(highs): qualify mip start support and reject absent hints` (Task 3)

## Public interfaces

### Task 1 — SolvePlan, starts, hints, policy (landed)

New public types (exported from `roml` root and `roml::advanced`, mirroring the
overlay/assignment export convention):

- `SolvePlan { options, overlay, mip_starts, hints, objective_override, lex_stage_policy, unsupported }` + `SolvePlan::new(SolveOptions) -> Result<Self, IdentityOverflow>` + `SolvePlan::validate(&Model) -> Result<(), PlanError>`
- `MipStart { assignment, repair, name }` + `MipStart::new(PrimalAssignment, RepairPolicy)`
- `RepairPolicy { BackendDefault, RejectIncomplete, AllowRepair }`
- `VariableHints` (private `BTreeMap<Variable, VariableHint>` + `get`/`insert`/`iter`/`is_empty`/`len`)
- `VariableHint { value: f64, priority: HintPriority }`
- `HintPriority(pub i32)`
- `UnsupportedFeaturePolicy { #[default] Reject, ConvertHintToStart, ConvertStartToTemporaryFixing }`
- `PlanError` (wraps `AssignmentError`; adds `DuplicateStartVariable`, `OverlayConflict`, `NonFiniteHintValue`, `IncompleteStart`, `UnsupportedFeature`)
- `ObjectivePolicy` (`#[non_exhaustive]`, `Single` only — P31 extension surface) and `LexStagePolicy { RequireOptimal, UseBestFeasible }`

Also: `SolveOptions` gained `#[derive(PartialEq)]` (required by the packet's
`SolvePlan: PartialEq`; `SolveRequest` was already `PartialEq`).

New test surface: `tests/solve_plan.rs` (16 Task 1 tests).

## Focused verification

### Task 1 — RED (expected failures)

`cargo test -p roml --test solve_plan` fails to compile with E0432/E0433:
the Task 1 types do not yet exist.

- `E0432: unresolved import 'roml::SolvePlan'` (and the other new names in
  `tests/solve_plan.rs`'s `use roml::{...}` list);
- `E0433: could not find 'ObjectivePolicy'/'LexStagePolicy' in 'roml'` at every
  direct-construction test site.

This is the expected missing-types RED; no behavioral test can run until the
Task 1 types are implemented.

### Task 1 — GREEN

- `cargo test -p roml --test solve_plan`: 16 passed (types, validation,
  conversion policy, basis distinctness).
- `cargo test -p roml --all-targets`: pass.
- `cargo clippy -p roml --all-targets -- -D warnings`: pass.
- `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps`: pass.
- `cargo public-api -p roml`: exit 0 (new types present; raw diff recorded
  under "Public API and packaging").
- `cargo fmt --all -- --check`: pass.

### Task 2 — RED (expected failures)

Task 2 tests (equivalence, metadata recording, feasibility signature,
no-stale-start, conversions) were added to `tests/solve_plan.rs` before the
executor existed. Expected RED: `E0599`/`E0433` — `SolverSession::solve_plan`
does not exist, `EffectiveSolvePlan`/`AppliedFeature`/`PlanAdjustment`/
`PlanRejection` are not exported, and `SolveMetadata::effective_plan` is
absent. Recorded before the Task 2 implementation.

### Task 2 — GREEN

- `cargo test -p roml --test solve_plan`: 24 passed (16 Task 1 + 8 Task 2:
  equivalence, metadata recording, two conversions, two default-rejections,
  feasibility-signature, no-stale-start).
- `cargo test -p roml --all-targets`: pass (P27 overlay + M2 suites not
  regressed — D27).
- `cargo clippy -p roml --all-targets -- -D warnings`: pass.
- `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps`: pass.
- `cargo public-api -p roml`: exit 0 (`solve_plan`, `EffectiveSolvePlan`,
  `SolveMetadata::effective_plan` present).
- `cargo fmt --all -- --check`: pass.

**Single-executor source assertion:** `grep -n "self.solve_plan" src/solver/facade.rs`
shows exactly three delegations — `solve` → `solve_with` → `solve_plan`,
`solve_with` → `solve_plan`, and `solve_with_overlay` → `solve_plan`. There is
no divergent plain-solve path: all solve façades route through the one plan
executor. Backends that do not implement `OverlaySession` were given default
`OverlaySession` impls (typed `Unsupported` on the required overlay methods) so
`SolverSession::solve`/`solve_with` remain available to them while still
routing through the executor.

### Task 3 — RED (expected failures)

`roml-highs/tests/solve_plan.rs` added before the backend implementation. 6 of
8 tests failed as expected:

- `highs_capability_set_declares_start_hint_features_per_audit` — `MipStart`/
  `PartialMipStart` still declared `Unsupported` (P26-era capability set);
- the qualified-start tests (`...applies_natively...`,
  `...leaves_feasible_region_signature_unchanged`,
  `highs_no_stale_start_leakage...`) — `SolveError::Plan(UnsupportedFeature)`:
  starts reject by default because the backend capability set did not yet
  qualify `MipStart`;
- `highs_convert_hint_to_start_is_recorded` — the conversion adjustment
  appeared but the applied feature was not yet recorded;
- `highs_failed_sparse_solution_maps_to_typed_backend_error` — the start was
  rejected at capability time, not at the native call.

`highs_solve_solve_with_and_empty_solve_plan_are_equivalent` and
`highs_variable_hints_reject_by_default` already passed (empty-plan equivalence
needs no backend start support; hints were already unqualified).

### Task 3 — GREEN

- `cargo test -p roml-highs --test solve_plan`: 8 passed (capability matrix,
  equivalence, default rejection, native qualified path, hint->start
  conversion, feasibility signature, checked return code, no-stale-start).
- `cargo test -p roml-highs --all-targets`: pass (36+ across suites).
- `cargo test -p roml --all-targets`: pass.
- `cargo clippy -p roml-highs --all-targets -- -D warnings`: pass.
- `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps`: pass.
- `cargo public-api -p roml-highs`: exit 0 (adds `HighsSession::apply_mip_starts`
  and `apply_variable_hints` — the two `OverlaySession` trait methods).
- `cargo fmt --all -- --check`: pass.

**Implementation notes (Task 3):**

- `roml-highs/src/start.rs` — `apply_mip_starts` maps each start's user
  `Variable` values through the compiled-keyed origin maps
  (`compiled_to_user_variable` → `col_map`) to native column indices and
  applies via `Highs_setSparseSolution` with every return code checked through
  `check_highs_status` (T-28-01). Hints are never simulated:
  `unsupported_hint_error()` is a typed `Unsupported` `BackendError`.
- `roml-highs/src/session.rs` — `highs_capability_set` now declares
  `MipStart`/`PartialMipStart` `Native` (audit-cited, `mip` model class) and
  keeps `MultipleMipStarts`/`VariableHints`/`InitialBasis` `Unsupported` with
  audit-citing notes. `HighsSession` implements the two `OverlaySession`
  warm-start methods.
- `roml-highs/tests/conformance.rs` — the P26-era capability test updated:
  `MipStart`/`PartialMipStart` are now asserted `Native` per SM-08.7.

## Full verification

Phase-level verification matrix (per-crate only; never workspace-wide). All
commands exit 0:

| # | Command | Result |
|---|---------|--------|
| 1 | `cargo fmt --all -- --check` | exit 0 |
| 2 | `cargo test -p roml --test solve_plan` | 24 passed |
| 3 | `cargo test -p roml --all-targets` | 911 passed |
| 4 | `cargo test -p roml-highs --test solve_plan` | 8 passed |
| 5 | `cargo test -p roml-highs --all-targets` | 139 passed |
| 6 | `cargo clippy -p roml --all-targets -- -D warnings` | exit 0 |
| 7 | `cargo clippy -p roml-highs --all-targets -- -D warnings` | exit 0 |
| 8 | `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | exit 0 |
| 9 | `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` | exit 0 |
| 10 | `cargo public-api -p roml` | exit 0 |
| 11 | `cargo public-api -p roml-highs` | exit 0 |

Package qualification (after the Task 3 commit; clean tree required by
`cargo package`):

| # | Command | Result |
|---|---------|--------|
| 12 | `cargo check -p roml --all-targets` | exit 0 |
| 13 | `cargo check -p roml-highs --all-targets` | exit 0 |
| 14 | `cargo package --list -p roml` | exit 0 (113 files; `src/solver/plan.rs` and `src/solver/effective_plan.rs` included) |
| 15 | `cargo package --list -p roml-highs` | exit 0 (35 files; `src/start.rs` included) |

P27–P28 mandatory checks (per `EXECUTION.md`) are covered by the passing
suites: overlay failure injection (`tests/solve_overlay.rs`), model revision
invariance under temporary operations (`tests/solve_overlay.rs`), subsequent-
solve leak tests (`no_stale_start_leakage_into_subsequent_solve` in both
`tests/solve_plan.rs` and `roml-highs/tests/solve_plan.rs`), and
capability/effective-plan assertions (`tests/solve_plan.rs`). No skips.

## Native/backend evidence

Full audit record: `docs/knowledge/highs_mip_start_api.md` (pinned bundled
`highs-sys 1.15.0` header `highs_c_api.h`, its C API implementation, and the
`Highs::setSolution` C++ implementation; CI floor 1.9.0).

Summary:

- **`Highs_setSparseSolution`** (`highs_c_api.h:1305`) — qualified native
  partial-MIP-start primitive; rejects an out-of-range index or an
  out-of-bounds value with `kHighsStatusError`; warns (last value wins) on
  duplicate indices; `kHighsStatusError=-1`, `kHighsStatusOk=0`,
  `kHighsStatusWarning=1` (`:28-30`).
- **`Highs_setSolution`** (`:1291`) — full primal+dual solution setter;
  available but not used in P28.
- **`Highs_setBasis`** (`:1264`) / **`Highs_setLogicalBasis`** (`:1274`) —
  present, but `InitialBasis` stays `Unsupported` in P28 (SM-08.6 separate
  artifact).
- **Absent:** `Highs_setMipStart`, `Highs_clearMipStart`,
  `Highs_clearSolution`, and any variable-hint symbol.
- **Lifecycle:** a set solution persists on the instance as the incumbent
  until invalidated (model change / next `setSolution`) or replaced by a
  solve; there is no clear API. Bounded structurally: the executor applies
  starts immediately before solve (one batch per `solve_plan`), a compiled
  rebuild clears the incumbent, and a start is a search hint that cannot
  change a proven optimum.

Capability declarations (all audit-cited in `FeatureLimitations.notes`):

| BackendFeature | Level | Evidence |
|----------------|-------|----------|
| `MipStart` | Native | `Highs_setSparseSolution` (full assignment) |
| `PartialMipStart` | Native | `Highs_setSparseSolution` (subset assignment) |
| `MultipleMipStarts` | Unsupported | single incumbent slot; no multi-start API |
| `VariableHints` | Unsupported | no hint API; reject by default |
| `InitialBasis` | Unsupported | API present but out of scope (SM-08.6) |

## Failure/recovery evidence

(Default rejection, conversion recording, no-stale-start determinism.)

## Public API and packaging

### Public API diff — `roml` (baseline -> Tasks 1–3)

Added (raw diff in `M3_P28_public_api_roml_baseline.txt` vs the final capture;
the important new surface):

- `SolvePlan { options, overlay, mip_starts, hints, objective_override, lex_stage_policy, unsupported }`, `SolvePlan::new`, `SolvePlan::validate`
- `MipStart`/`RepairPolicy`/`VariableHints`/`VariableHint`/`HintPriority`
- `UnsupportedFeaturePolicy`, `PlanError` (from `AssignmentError`)
- `ObjectivePolicy` (`#[non_exhaustive]`, `Single`) and `LexStagePolicy`
- `EffectiveSolvePlan`/`AppliedFeature`/`PlanAdjustment`/`PlanRejection`/`ObjectiveStageResult`
- `SolverSession::solve_plan`
- `OverlaySession::apply_mip_starts` / `apply_variable_hints` (default-reject)
- `SolveMetadata::effective_plan`
- `SolveOptions` gained `PartialEq`

### Public API diff — `roml-highs` (baseline -> Task 3)

Only two additions:

- `HighsSession::apply_mip_starts`
- `HighsSession::apply_variable_hints`

No existing public symbol changed signature or was removed in either crate.

### Package qualification

`cargo package --list` exit 0 for both crates; the new modules
(`src/solver/plan.rs`, `src/solver/effective_plan.rs`, `roml-highs/src/start.rs`)
are included in the packed file lists. No crate was published (SM-15.8 / M3
stopping condition).

## Deviations and decisions

### Forward declarations (A30/A32 extension-surface precedent)

- `ObjectivePolicy` is `#[non_exhaustive]` with only `Single` in P28; P31 owns
  `Weighted`/`Lexicographic` in `src/objective_policy.rs`. The `#[non_exhaustive]`
  boundary makes the P31 extension a non-breaking change.
- `LexStagePolicy` is declared with both variants in P28; P31 executes the
  stages and populates `EffectiveSolvePlan::objective_stages` (declared empty in
  P28 per SM-07.7).

### Auto-fixed issues (Rule 1/2)

1. **[Rule 2 - Recording gap] Converted hint->start applied feature.** During
   Task 3 GREEN, `highs_convert_hint_to_start_is_recorded` failed because a
   `ConvertHintToStart` conversion pushed the converted start into the
   application list and a `PlanAdjustment`, but NOT an `AppliedFeature`. Fixed
   in `resolve_plan_features` to also record the applied feature (SM-04.5: all
   applications recorded). Files: `src/solver/facade.rs`.

2. **[Rule 1 - Stale assertion] P26-era conformance capability test.**
   `roml-highs/tests/conformance.rs` asserted `MipStart`/`PartialMipStart` are
   `Unsupported`. P28 qualifies them `Native` per SM-08.7, so the test's
   `unqualified_m3` list was updated (removed the two features and added an
   explicit Native assertion). This is a test expectation update mandated by
   the P28 capability change, not a weakened test.

### Design decisions recorded

- **Qualified-wins over conversion:** when a feature is qualified by the
  backend, an explicit conversion policy is NOT applied (conversion is a
  fallback for unqualified features). Proven by
  `highs_qualified_start_applies_natively_even_under_conversion_policy`.
- **Unconvertible requests under a conversion policy** are recorded as
  `PlanRejection`s (never silent), while the default `Reject` policy returns a
  typed `SolveError::Plan` before any backend mutation (SM-08.4).
- **Test backends** (`RecordingBackend`, `LineageTestBackend`, `TestBackend`)
  gained minimal `OverlaySession` impls (typed `Unsupported` on the three
  required methods) so `SolverSession::solve`/`solve_with` remain available to
  them while still routing through the single `solve_plan` executor (D27).

## Reviewer findings

(Reserved for the two independent review passes after Task 3.)

## Residual risks

(Version-gated HiGHS behavior, P31 objective_stages population.)

## Gate result

(Completed by the orchestrator after the review gates resolve with no P0/P1.)

## Review gates (executed)

Two independent review passes at the phase boundary.

- **Pass 1 (spec/correctness) — HOLD (P1-01 blocking):** the single-executor, C_overlay lifecycle, exact-`CompilationId` gate, default rejection, recorded conversions, and the audit record all verified sound. One P1: the executor never consulted `MultipleMipStarts`, so a second+ start was silently dropped by HiGHS's overwriting `Highs_setSparseSolution` while recorded as applied (SM-08.2). Seven P2 + one additional P2 from Pass 2.
- **Pass 2 (integration/ops) — MERGEABLE (0 P0/P1):** incremental/rebuild behavior, failure recovery, cross-platform declarations, public API, docs, migration, and E2E all wired; three P2s (one substantive: warm-start failure state).

### Review-fix round (orchestrator, after Pass 1)

**P1-01 — FIXED (TDD, 3 tests):** the starts loop gates `index >= 1` on `MultipleMipStarts` through the policy ladder (Reject → typed `PlanError`; ConvertStartToTemporaryFixing → recorded conversion; ConvertHintToStart → recorded rejection when it would create a second start). **P2-03 — FIXED (TDD):** `force_rebuild_on_next_sync()` on the warm-start failure path (CR-02 mirror) + test proving the next solve rebuilds and the failed start cannot seed it. **P2-01 — FIXED:** the three `OverlaySession` lifecycle methods defaulted with typed-Unsupported default-reject implementations (M2 backend migration = one empty impl line; D27). **P2-02 — FIXED (TDD):** `SolvePlan::validate` rejects stale hint variables before any backend mutation (`solves == 0` asserted). **P2-04 — FIXED (TDD):** `objective_override` recorded as a `PlanAdjustment`. **P2-07 — FIXED:** `model_classes: ["mip"]` limitation dropped (the audit shows `setSparseSolution` accepts any model class; the declaration now traces to the audit). **P2-05/06/08 — ACCEPTED, documented** (empty-overlay equivalence design; hints vacuous-by-design on the pinned backend; `kWarning` unreachable through validated plans). Full dispositions in `.planning/phases/28-solve-plan-warm-starts/REVIEW.md`.

Post-fix verification (all exit 0): `cargo test -p roml --test solve_plan` **30/30**; roml all-targets green (35 suites); roml-highs all-targets green (19 suites); clippy both crates `-D warnings`; rustdoc both crates `-D warnings`; fmt clean.

**Gate status:** P1-01 and the substantive P2s resolved; Pass-1 re-review pending on the fix commit; Pass 2 unaffected (no integration-surface change beyond the trait defaults and declaration notes).

## Second review round (owner disposition, PR #32)

**P1-1 — D27 source compatibility broken — FIXED (compile-time regression first).** `solve`/`solve_with` moved back into the unbounded impl block (`B: BackendSession + SessionHealth + BackendMetadata`), implemented through the shared plain core `solve_base`; `solve_plan` delegates to it for empty-content plans (the `solve == solve_with == empty solve_plan` equivalence stays one code path). The three boilerplate `OverlaySession` impls added to the legacy test backends (`RecordingBackend`, `LineageTestBackend`, `TestBackend`) are removed — their solve/solve_with call sites compile unchanged. New regression `d27_plain_solve_callable_without_overlay_session` (a `PreP28Backend` with exactly the pre-P28 traits): generic bound-checked `solve`/`solve_with` calls + an end-to-end optimal solve; the test fails to compile (verified: 4 E0599s) if the plain paths are re-bounded.

**P1-2 — start capabilities not composed — FIXED (3 cross-product tests first).** Start qualification is now `primitive && (first || MultipleMipStarts)` (full = `MipStart && …`, partial = `PartialMipStart && …`); the error names `MultipleMipStarts` when the primitive holds and the multiple gate fails, else the primitive. `ConvertHintToStart` classifies the generated assignment full/partial and applies the same composed gates — a conversion never bypasses the primitive and never silently overwrites the plan's own start. Cross-product tests cover MultipleMipStarts without each primitive, two full starts under both gates, and partial/full converted starts.

Post-fix verification: solve_plan **34/34** (incl. the compile-time regression and 3 cross-product tests); roml all-targets green (35 suites, incl. the three legacy-backend files); roml-highs all-targets green (19 suites); clippy both crates `-D warnings`; rustdoc `-D warnings`; fmt clean.
