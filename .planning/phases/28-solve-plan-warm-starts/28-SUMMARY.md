---
phase: 28-solve-plan-warm-starts
plan: 01
subsystem: solver
tags: [solve-plan, mip-start, hints, effective-plan, capability, warm-start]
dependency_graph:
  requires: [P26 compiler backend IR (CompilationReport, capabilities), P27 overlay/assignment (SolveOverlay, PrimalAssignment), P25 snapshot/delta canonical state]
  provides: [SolvePlan, MipStart, VariableHints, UnsupportedFeaturePolicy, PlanError, ObjectivePolicy, LexStagePolicy, EffectiveSolvePlan, SolveMetadata.effective_plan, Highs start/hint qualification]
  affects: [P29 (IIS session shares the solve-attempt contract), P30 (soft-constraint relaxations via overlay), P31 (objective policies/lexicographic), P34 (qualification)]
tech-stack:
  added: []
  patterns:
    - one explicit SolvePlan combining options, overlay, starts, hints, objective override, lex stage policy, and unsupported-feature policy
    - single plan executor: solve / solve_with / solve_with_overlay / solve_plan all route through one code path (source-asserted, no divergent plain-solve path)
    - effective-plan reporting on every solve (SolveMetadata.effective_plan records applied features, adjustments, rejections)
    - start qualification composed from primitive + multiple gates (never silently overwritten)
    - unsupported features reject by default (UnsupportedFeaturePolicy::Reject); conversions are recorded, never silent
key-files:
  created:
    - src/solver/plan.rs
    - src/solver/effective_plan.rs
    - roml-highs/src/start.rs
    - tests/solve_plan.rs
    - roml-highs/tests/solve_plan.rs
    - docs/knowledge/highs_mip_start_api.md
    - docs/release/evidence/M3_P28_SOLVE_PLAN_STARTS_HINTS.md
  modified:
    - src/solver/facade.rs
    - src/solver/session.rs
    - src/solver/mod.rs
    - src/solution/metadata.rs
    - src/assignment.rs
    - src/lib.rs
    - src/advanced.rs
    - roml-highs/src/session.rs
    - roml-highs/src/lib.rs
decisions:
  - Unsupported features reject by default; conversions (ConvertHintToStart, ConvertStartToTemporaryFixing) are recorded PlanAdjustments/PlanRejections, never silent (SM-07.7)
  - All solve façades route through the one plan executor; backends without OverlaySession get default typed-Unsupported impls (D27 plain-solve compatibility preserved)
  - HiGHS start support qualified from the pinned official header audit (MipStart/PartialMipStart full/partial); hints are unsupported by design on the pinned backend (no hint capability exists in the official API — no silent simulation)
  - Start qualification is composed: primitive && (first || MultipleMipStarts); conversions never bypass the primitive gate
  - ObjectivePolicy is #[non_exhaustive] with Single only — the P31 extension surface
metrics:
  duration: ~2-3 days (multi-review-round phase)
  completed: 2026-08-04
  tasks: 3 (SolvePlan/starts/hints types; one plan executor; HiGHS start/hint qualification)
  commits: 3 task commits + evidence + review-fix commits (rebase note: all SHAs changed on 2026-08-04 after PR #33 merge)
status: complete
actuals:
  tokens: ~45000   # evidence doc is ~40KB; verify against diff if precision needed
  tasks: 3
  commits: 7 (3 task + evidence + fix commits; post-rebase)
---

# Phase [28] Task [1-3]: SolvePlan, starts, hints, and effective-plan reporting

One explicit `SolvePlan` type combining options, overlays, starts, hints, objective overrides, and unsupported-feature policy; one plan executor routing `solve`/`solve_with`/`solve_with_overlay`/`solve_plan`; effective-plan reporting on every solve; and a qualified HiGHS start/hint implementation derived from the pinned official header audit. Requirements closed: SM-07.1, SM-07.2, SM-07.7, SM-08.1–08.7, SM-04.5 (deferred from P26 per TRACEABILITY.md).

## What was built

- **`src/solver/plan.rs`** — `SolvePlan { options, overlay, mip_starts, hints, objective_override, lex_stage_policy, unsupported }` + `SolvePlan::new(SolveOptions) -> Result<Self, IdentityOverflow>` + `SolvePlan::validate(&Model) -> Result<(), PlanError>`; `MipStart { assignment, repair, name }`; `RepairPolicy { BackendDefault, RejectIncomplete, AllowRepair }`; `VariableHints` (private `BTreeMap<Variable, VariableHint>`); `VariableHint { value, priority }`; `HintPriority(pub i32)`; `UnsupportedFeaturePolicy { #[default] Reject, ConvertHintToStart, ConvertStartToTemporaryFixing }`; `PlanError` (wraps `AssignmentError`; adds `DuplicateStartVariable`, `OverlayConflict`, `NonFiniteHintValue`, `IncompleteStart`, `UnsupportedFeature`); `ObjectivePolicy` (`#[non_exhaustive]`, `Single` only — P31 extension surface); `LexStagePolicy { RequireOptimal, UseBestFeasible }`.
- **`src/solver/effective_plan.rs`** — `EffectiveSolvePlan`, `AppliedFeature`, `PlanAdjustment`, `PlanRejection`; `SolveMetadata::effective_plan` field reporting on every solve.
- **`src/solver/facade.rs` / `session.rs`** — single plan executor; `solve` → `solve_with` → `solve_plan` delegations (source-asserted: exactly three delegations, no divergent plain-solve path); `solve_with_overlay` routes through the same executor. Backends without `OverlaySession` received default typed-`Unsupported` impls (D27: plain `solve`/`solve_with` remain callable).
- **`roml-highs/src/start.rs` / `session.rs`** — HiGHS start/hint implementation derived from the pinned official header audit: `MipStart`/`PartialMipStart` capability qualification (full = `MipStart && (first || MultipleMipStarts)`, partial = `PartialMipStart && …`); the starts loop gates `index >= 1` on `MultipleMipStarts` through the policy ladder; hints reported unsupported by design (no official hint API — no silent simulation); `force_rebuild_on_next_sync()` on the warm-start failure path.
- **`src/solution/metadata.rs`** — `SolveMetadata::effective_plan`; `SolveOptions` gained `#[derive(PartialEq)]` (required by `SolvePlan: PartialEq`).
- **`tests/solve_plan.rs`** (34 tests) — types, validation, conversion policy, basis distinctness, equivalence (solve == solve_with == empty solve_plan), metadata recording, feasibility signature, no-stale-start, default-rejection; **`roml-highs/tests/solve_plan.rs`** — capability declarations, qualified starts, no stale-start leakage, cross-product capability gates.

## Verification

| Command | Result |
|---|---|
| `cargo test -p roml --test solve_plan` | 0 — 34 passed (incl. compile-time D27 regression + 3 cross-product capability tests) |
| `cargo test -p roml --all-targets` | 0 — green (35 suites) |
| `cargo test -p roml-highs --all-targets` | 0 — green (19 suites) |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 — clean |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 — clean |
| `RUSTDOCFLAGS='-D warnings' cargo doc` (both crates) | 0 — clean |
| `cargo fmt --all -- --check` | 0 — clean |
| `cargo public-api` (both crates) | 0 — SolvePlan surface present; baseline captures in evidence |

## Deviations from plan

1. **`solve`/`solve_with` moved back into the unbounded impl block** (`B: BackendSession + SessionHealth + BackendMetadata`), implemented through the shared plain core `solve_base`; `solve_plan` delegates to it for empty-content plans. The three boilerplate `OverlaySession` impls added to legacy test backends were removed — their call sites compile unchanged. Compile-time regression `d27_plain_solve_callable_without_overlay_session` (4 E0599s if re-bounded) guards this.
2. **Start capabilities composed, not flat:** `primitive && (first || MultipleMipStarts)`; `ConvertHintToStart` classifies the generated assignment full/partial and applies the same composed gates — a conversion never bypasses the primitive and never silently overwrites the plan's own start.
3. **`model_classes: ["mip"]` limitation dropped** after the audit showed `setSparseSolution` accepts any model class; the declaration now traces to the audit.
4. **Accepted design decisions (documented, no code change):** P2-05 empty-overlay equivalence design; P2-06 hints vacuous-by-design on the pinned backend; P2-08 `kWarning` unreachable through validated plans.

## Known Stubs

- **HiGHS hints:** `VariableHints` is a full public type and validated in `SolvePlan::validate` (stale hint variables rejected before any backend mutation), but HiGHS rejects hint-bearing plans with typed `Unsupported` — no official hint API exists on the pinned backend (audit: `docs/knowledge/highs_mip_start_api.md`). No silent simulation.
- **ObjectivePolicy::Single only:** lexicographic stages are declared (`LexStagePolicy`) but orchestration lands in P31.

## Self-Check: PASSED

Merged via PR #32 (`2fa0596`) onto main. Created files verified present: `src/solver/plan.rs`, `src/solver/effective_plan.rs`, `roml-highs/src/start.rs`, `tests/solve_plan.rs`, `roml-highs/tests/solve_plan.rs`, `docs/knowledge/highs_mip_start_api.md`, `docs/release/evidence/M3_P28_SOLVE_PLAN_STARTS_HINTS.md`. Post-fix verification matrix all exit 0 (solve_plan 34/34; roml 35 suites; roml-highs 19 suites; clippy/rustdoc `-D warnings`; fmt). Full review dispositions (P1-01, P2-01..08, P1-1, P1-2) in `REVIEW.md`; integration verdict MERGEABLE with 0 P0/P1.
