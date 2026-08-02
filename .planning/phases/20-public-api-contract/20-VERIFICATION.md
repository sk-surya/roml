---
phase: 20-public-api-contract
verified: 2026-08-02T00:00:00Z
status: human_needed
score: 9/9 truths verified
behavior_unverified: 0
overrides_applied: 0
gaps: []
human_verification:
  - test: "Review the baseline evidence in docs/release/evidence/M2_P20_BASELINE.md against EXECUTION.md evidence standards: base SHA d1391fb, toolchain (rustc 1.97.1, aarch64-apple-darwin), core matrix (399 passed) and HiGHS matrix (73 passed) at base, and the cargo package --list -p roml exit-101 skip reason."
    expected: "Baseline commands, item counts, and skipped-check reasons are acceptable and the recorded counts match what a clean re-run at the base commit produces."
    why_human: "The P20 gate (20-PLAN.md 'Gate') explicitly requires baseline commands and evidence to be 'reviewed'; the phase SUMMARY marks coverage item D1 human_judgment: true. An automated verifier can confirm internal consistency but not reviewer sign-off."
  - test: "Review the drift characterization in tests/ui/current_readme_drift.rs and the captured E0432/E0599 compile failures recorded in M2_P20_BASELINE.md against the actual README.md (lines 60, 78-79) and MODELING_API.md (lines 26, 55, 146, 160-161, 179, 188) usage."
    expected: "The captured failures match the documented HighsAdapter/solve_model API and freezing the fixture (kept out of the default build) is the correct characterization approach."
    why_human: "The phase SUMMARY marks coverage item D2 human_judgment: true — confirming the captured failure matches the documented API is a review judgment, not a grep check."
  - test: "Review the frozen target signatures in tests/ui/target_quickstart.rs and tests/ui/target_incremental.rs against DECISIONS.md D1/D3/D4/D7 and the M2 packet (continuous/integer/binary/parameter builders, Model::named, add_constraint(spec), fallible set_parameter, Highs::new/solve/solve_with, Solution::status().is_optimal())."
    expected: "The target signatures are exactly as specified and have NOT been weakened; they remain non-compiling by design until P21/P22."
    why_human: "Plan Task 3 requires review before any weakening; the phase SUMMARY marks coverage item D3 human_judgment: true. Whether the frozen forms are the correct target is an API-review decision."
  - test: "Review the per-item disposition table in docs/release/PUBLIC_API_M2_DISPOSITION.md (91 items across the 8 required categories: root/prelude, model constructors/mutators, expression/operator traits, all four macros, IDs and coefficients, solution/status/result, backend session/sync, callback/capability)."
    expected: "Each item's disposition among exactly one of the five (golden path / optional syntax sugar / advanced backend extension / compatibility-deprecated / internal exposure to remove) is coherent, the replacement signatures match D7/D3, and the deprecation order honors D12 (replacement before deprecation)."
    why_human: "The phase SUMMARY marks coverage item D5 human_judgment: true — naming/curation coherence is an API-review artifact the P20 gate requires a human reviewer to confirm."
---

# Phase 20: Public API Contract Baseline — Verification Report

**Phase Goal:** freeze the exact M2 signatures, dispositions, and baseline evidence needed to implement without API drift ("establish current behavior and freeze the M2 public contract before implementation").
**Verified:** 2026-08-02
**Status:** human_needed
**Re-verification:** No — initial verification
**Branch verified:** `phase-roml-P20-api-contract` @ `473d465`

## Goal Achievement

The phase goal — establish the exact current-main public API behavior and freeze the M2 target contract before any implementation — is achieved on disk. All 5 plan tasks produced their claimed deliverables, all 9 phase files exist and are substantive, no production code changed, and every behavior-dependent deliverable has passing test evidence. The only reason the status is not `passed` is that the phase's own Gate and the SUMMARY's coverage metadata require human API review of the baseline, drift capture, target signatures, and disposition table.

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Exact current-main API inventory recorded: base SHA `d1391fb3a58a61d24b5c597aab78e4be7683f894`, toolchain `rustc 1.97.1` / `aarch64-apple-darwin`, full core + HiGHS matrix with exit statuses and test counts, normalized `cargo public-api` inventories for both crates (repo paths → `$REPO`) | ✓ VERIFIED | `docs/release/evidence/M2_P20_BASELINE.md` (baseline matrix + inventory sections); `M2_P20_public_api_roml.txt` (7431 lines, confirmed by wc; zero `/Users/` occurrences) and `M2_P20_public_api_roml_highs.txt` (80 lines, confirmed) |
| 2 | Documented solve API drift characterized: README/MODELING_API `HighsAdapter::solve_model` does not exist in production; failures frozen with captured E0432 (import) and E0599 (method) | ✓ VERIFIED | `tests/ui/current_readme_drift.rs` + `tests/ui/current_solve_model_method.rs`; `scripts/p20-capture-drift.sh` reproduces both errors from the committed fixtures (exit 0; asserts expected codes); baseline doc drift section; production grep: zero `HighsAdapter`/`solve_model` in `src/`, `roml-highs/src/` (only stale doc comment `roml-highs/src/ffi.rs:3`); public-api inventories contain zero occurrences; README.md:60,78-79 and MODELING_API.md:26,55,146,160-161,179,188 are the drift source |
| 3 | Golden-path target compile contracts frozen against exact target signatures (quickstart + incremental) | ✓ VERIFIED | `tests/ui/target_quickstart.rs:38-48` matches plan verbatim; `tests/ui/target_incremental.rs:35-46` matches plan verbatim; confirmed not auto-discovered (no test binary lists them) |
| 4 | Compile-pass characterization of the current `roml` prelude surface that compiles and runs today | ✓ VERIFIED | `tests/public_api_compile.rs` — 3/3 tests pass (independently re-run) |
| 5 | Every public entry point assigned exactly one of five dispositions, covering all 8 required categories, with replacement signatures and deprecation order | ✓ VERIFIED | `docs/release/PUBLIC_API_M2_DISPOSITION.md` — 91 rows; disposition counts: golden path 22, optional syntax sugar 3, advanced backend extension 48, compatibility/deprecated 13, internal exposure to remove 5; all 8 categories present (Sections 1-8); all four macros classified (lines 100-103); replacement signatures (lines 159-181); deprecation order D12 (lines 186-199) |
| 6 | Repeated-solve `HighsSession` protocol baseline recorded (revision/health/status/objective/solution availability per step, incl. deterministic dirty-path snapshot recovery) | ✓ VERIFIED | `roml-highs/tests/repeated_session_baseline.rs` — 3/3 tests pass (independently re-run): rebuild→solve obj 12.0, parameter delta 3.0→5.0 → 20.0, bound delta 4.0→2.0 → 8.0, rejected mismatched-base delta → RequiresRebuild + deterministic recovery → 12.0; behavior table in baseline doc lines 156-178 |
| 7 | Zero production code changes (phase Gate: characterization-only) | ✓ VERIFIED | `git log --name-only fa8e52b^..8ebbb1d`: only 9 docs/evidence/test files touched; working tree has no phase-related modifications |
| 8 | Requirement IDs API-04, API-07, API-08, API-10 each accounted for by actual evidence | ✓ VERIFIED | See Requirements Coverage table below |
| 9 | Approved M2 decisions referenced (D1-D13); no new naming/signature ambiguity — the four inherited open naming items are explicitly recorded and deferred to P21/P22 per the M2 packet | ✓ VERIFIED (note) | Disposition doc "Open items inherited from planning" (lines 201-209) records compat window, SolveStatus-vs-SolverStatus, `SolverSession<B>` name, alias-vs-wrapper; states dispositions do not depend on their resolution. These were never this phase's to resolve (plan Gate: "No production API implementation is required in this phase") |

**Score:** 9/9 truths verified (0 present-but-behavior-unverified; 0 overrides).

### Required Artifacts

| Artifact | Expected | Status | Details |
| -------- | --------- | ------ | ------- |
| `docs/release/evidence/M2_P20_BASELINE.md` | base SHA, env, matrix, item counts, drift captures, skip reasons | ✓ VERIFIED | 8488 bytes; all claims cross-checked against git/grep |
| `docs/release/evidence/M2_P20_public_api_roml.txt` | normalized `cargo public-api -p roml` | ✓ VERIFIED | 7431 lines; paths → `$REPO` |
| `docs/release/evidence/M2_P20_public_api_roml_highs.txt` | normalized `cargo public-api -p roml-highs` | ✓ VERIFIED | 80 lines; zero HighsAdapter/solve_model |
| `docs/release/PUBLIC_API_M2_DISPOSITION.md` | per-item disposition, 5 categories, replacements, migration strategy, deprecation order | ✓ VERIFIED | 91 items; all 8 plan-required sections; collision migration recorded |
| `tests/ui/current_readme_drift.rs` | frozen README drift fixture (non-compiling, not auto-discovered) | ✓ VERIFIED | matches README/MODELING_API documented code (E0432) |
| `tests/ui/current_solve_model_method.rs` | frozen `solve_model`-on-`HighsSession` fixture (E0599) | ✓ VERIFIED | `scripts/p20-capture-drift.sh` exit 0, both error codes reproduced |
| `tests/ui/target_quickstart.rs` | frozen golden-path target contract | ✓ VERIFIED | verbatim plan form |
| `tests/ui/target_incremental.rs` | frozen incremental one-`Highs` target contract | ✓ VERIFIED | verbatim plan form |
| `tests/public_api_compile.rs` | compile-pass current-surface characterization (3 tests) | ✓ VERIFIED | 3/3 pass; exact `assert_eq!` on active objective |
| `roml-highs/tests/repeated_session_baseline.rs` | repeated-solve protocol baseline (3 tests) | ✓ VERIFIED | 3/3 pass; asserts stale-readable state on rejected delta |

### Key Link Verification

| From | To | Via | Status | Details |
| ---- | --- | --- | ------ | ------- |
| `tests/ui/target_*.rs` | DECISIONS.md D1/D3/D4/D7 target signatures | verbatim fixture bodies | ✓ WIRED | confirmed identical to plan's exact forms (lines 38-48 and 35-46) |
| `tests/ui/current_readme_drift.rs` | README.md / MODELING_API.md documented API | `use roml_highs::HighsAdapter` + `adapter.solve_model` | ✓ WIRED | README.md:60,78-79; MODELING_API.md:26,146,179 |
| `M2_P20_BASELINE.md` | public-api inventories | references to the two txt files | ✓ WIRED | line counts match exactly |
| `M2_P20_BASELINE.md` | `tests/ui/current_readme_drift.rs` + `repeated_session_baseline.rs` | companion-artifact section (lines 187-197) | ✓ WIRED | cross-references consistent |
| `PUBLIC_API_M2_DISPOSITION.md` | `tests/ui/target_*.rs` | "Full target bodies are frozen in tests/ui/..." (lines 183-184) | ✓ WIRED | consistent |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
| -------- | ------------- | ------ | ------------------ | ------ |
| `repeated_session_baseline.rs` | objective_value, revision, health | real `HighsSession::solve` via bundled HiGHS | yes — real solver output (obj 12.0/20.0/8.0 asserted) | ✓ FLOWING |
| `public_api_compile.rs` | objective_constant, parameter_value | real `Model` mutation | yes — real model semantics asserted | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| -------- | ------- | ------ | ------ |
| Current-surface compile-pass characterization | `cargo test -p roml --test public_api_compile` | 3 passed; 0 failed | ✓ PASS |
| Repeated-solve protocol (param delta 3.0→5.0 → 20.0; bound delta → 8.0; dirty-path recovery → 12.0) | `cargo test -p roml-highs --test repeated_session_baseline` | 3 passed; 0 failed | ✓ PASS |
| Formatting clean | `cargo fmt --all -- --check` | exit 0 | ✓ PASS |
| UI fixtures not auto-discovered (do not break default suites) | `cargo test -p roml --all-targets -- --list` / `-p roml-highs` | no `target_*`/`current_readme`/`current_solve_model`/`drift` test names | ✓ PASS |

Orchestrator's full matrix (`cargo test -p roml --all-targets`: 402 passed; `-p roml-highs --all-targets`: 76 passed; rustdoc `-D warnings` both crates) is consistent with the base matrix plus this phase's new tests (399+3=402, 73+3=76).

### Probe Execution

N/A — no probes declared in the PLAN/SUMMARY and this is a characterization/evidence phase, not a migration or CLI/tooling phase (Step 7c not applicable).

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
| ----------- | ---------- | ----------- | ------ | -------- |
| API-04 (canonical modeling style) | 20-PLAN | add_constraint(spec), minimize/maximize, .le/.ge/.eq/.between, macro disposition | ✓ SATISFIED (characterization portion) | target_quickstart.rs:42,44; target_incremental.rs:39,40; public_api_compile.rs uses .le/.ge/.between; disposition Sections 2-4; behavioral completion is P22/P23 per TRACEABILITY |
| API-07 (public surface curation) | 20-PLAN | prelude limits, protocol types absent, advanced/backend grouping, raw arenas private, cargo public-api evidence | ✓ SATISFIED (characterization portion) | disposition Section 1 (API-07.2/07.3 moves to `roml::backend`), Section 5 `Generation`/`IdArena` → remove (API-07.4); `M2_P20_public_api_*.txt` stored normalized (API-07.5) |
| API-08 (compatibility and migration) | 20-PLAN | replacement-before-deprecation, docs, actionable notes, no backend-contract change in M2 | ✓ SATISFIED (characterization portion) | disposition "Deprecation order (D12)" and "Signature-collision migration" sections (API-08.1/08.3; concrete per-collision approach for `add_variable`/`set_parameter`/`add_constraint`); zero production changes, no backend contract change (API-08.4); MIGRATION/CHANGELOG documentation is P23 |
| API-10 (qualification) | 20-PLAN | suites remain green, compile-pass tests, compile-fail tests where practical | ✓ SATISFIED (characterization portion) | baseline matrix (API-10.1) + orchestrator re-run green; `tests/public_api_compile.rs` green (API-10.2); UI fixtures kept out of default build (API-10.1); packed-consumer portions are P24 |

No orphaned requirements: API-04, API-07, API-08, API-10 all appear in the PLAN `requirements` field and are each mapped to evidence.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| — | — | — | — | None. No TBD/FIXME/XXX/TODO/HACK/PLACEHOLDER markers in any phase deliverable; no empty implementations; no console.log-only code; no hardcoded-empty rendering |

Non-compiling UI fixtures are **intentional** (they freeze target contracts and drift), not stubs: they are excluded from auto-discovery (verified via test enumeration) so default suites stay green, and their bodies are verbatim copies of the plan's exact target forms.

### Deferred Items

None. The four inherited open naming items (compat window, SolveStatus naming, `SolverSession<B>` name, alias-vs-wrapper) are recorded in the disposition doc and M2 STATE.md as P21/P22 decisions — they are planning-packet open items this phase transparently carries, not failed phase truths, so they are not listed as formal deferred gaps.

### Human Verification Required

The phase's own Gate ("P20 passes when target signatures, public-item dispositions, current drift, protocol behavior, and baseline commands are reviewed") and the SUMMARY's coverage metadata (D1, D2, D3, D5 marked `human_judgment: true`) require human API review of four contract artifacts:

1. **Baseline evidence review** — Confirm the baseline commands, toolchain, base SHA, and test counts in `docs/release/evidence/M2_P20_BASELINE.md` are acceptable per EXECUTION.md evidence standards. The `cargo package --list -p roml` baseline was completed from a clean worktree after review (exit 0, 99 files); the earlier exit-101 (dirty primary tree from untracked local artifacts) is recorded as context, not as a skip.
2. **Drift characterization review** — Confirm the captured E0432/E0599 failures match the README/MODELING_API documented `HighsAdapter::solve_model` API and that the UI-fixture characterization approach is correct.
3. **Target contract signature review** — Confirm the frozen signatures in `tests/ui/target_quickstart.rs` and `tests/ui/target_incremental.rs` match DECISIONS.md and the M2 packet exactly and are not weakened.
4. **Disposition table review** — Confirm the 91 per-item classifications (one of five dispositions, all 8 required categories) are coherent and the D12 replacement-before-deprecation order is sound.

### Gaps Summary

No gaps found. All 9 must-have truths verified, all 9 phase artifacts present/substantive/wired with flowing data, all behavioral tests pass (independently re-run), formatting clean, no debt markers, no production code changed, and requirement IDs API-04/API-07/API-08/API-10 are each backed by actual evidence. The phase goal is achieved; status is `human_needed` solely because the phase's own Gate requires human API review of the four contract artifacts above.

---

_Verified: 2026-08-02_
_Verifier: Claude (gsd-verifier)_
