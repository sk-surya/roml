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
- (Task 2 commit pending; Task 3 pending)

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

## Full verification

(Phase-level verification matrix recorded after Task 3.)

## Native/backend evidence

(HiGHS start/hint API audit — see `docs/knowledge/highs_mip_start_api.md`.)

## Failure/recovery evidence

(Default rejection, conversion recording, no-stale-start determinism.)

## Public API and packaging

(Public API diff and package qualification after the phase.)

## Deviations and decisions

(Auto-fixed issues, forward-declarations, and design amendments.)

## Reviewer findings

(Reserved for the two independent review passes after Task 3.)

## Residual risks

(Version-gated HiGHS behavior, P31 objective_stages population.)

## Gate result

(Completed by the orchestrator after the review gates resolve with no P0/P1.)
