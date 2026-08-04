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

(Tasks 1–3 commits recorded as they land.)

## Public interfaces

(Task 1/2/3 public surface recorded as it lands.)

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
