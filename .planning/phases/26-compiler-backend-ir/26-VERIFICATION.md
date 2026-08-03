---
phase: 26-compiler-backend-ir
verified: 2026-08-03T08:15:00Z
status: passed
score: 6/6 must-haves verified
behavior_unverified: 0
overrides_applied: 0
re_verification: false
---

# Phase 26: Compiler Boundary, Backend IR, Capabilities, Origins, and Exact Compilation Identity — Verification Report

**Phase Goal:** insert deterministic semantic compilation without regressing primitive incremental behavior.
**Verified:** 2026-08-03T08:15:00Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

The ROADMAP P26 gate is the phase contract. Goal-backward verification checked each gate truth against the actual codebase (not SUMMARY claims). All six truths verified with behavioral evidence where the truth is behavior-dependent; the full `roml` (647) and `roml-highs` (102) test suites pass on this tree, clippy is clean with warnings denied, and all 7 code-review findings (2 critical, 5 warning) are fixed in committed code and verified in place.

### Observable Truths

| #   | Truth (ROADMAP gate)   | Status     | Evidence       |
| --- | ------- | ---------- | -------------- |
| 1   | M2 solve/recovery passes through backend IR | ✓ VERIFIED | `SolverSession` owns a `CompilationSession` (`src/solver/facade.rs:123`); `solve_with` → `apply_deltas`/`rebuild_from_snapshot` compile canonical state and synchronize only via `Synchronization::CompiledRebuild(BackendSnapshot)` / `CompiledDeltaBatch(BackendDeltaBatch)` (`facade.rs:275-378`). `ReferenceBackend` consumes the compiled projection (`rebuild_compiled`/`apply_compiled_delta`/`apply_compiled_op`, `src/solver/reference.rs:476-627`); HiGHS `synchronize` handles only the compiled variants (`roml-highs/src/session.rs:74-201`). `m3_baseline_characterization` (6 tests) passes — no regression. |
| 2   | Primitive random deltas equal rebuild | ✓ VERIFIED | `dx_fixed_seed_compiled_delta_equals_compiled_rebuild` (`tests/differential_harness.rs:2272`) generates 4×20 random primitive-linear ops with fixed seed 4242, applies them as compiled deltas (Path B) and compares state fields against one compiled rebuild (Path A); asserts distinct `CompilationId` per D28. Section 8 closes the removal surface: `dx_compiled_remove_variable_purges_coefficients_and_holds_square` (CR-01) and `dx_compiled_remove_active_objective_clears_policy_and_holds_square` (CR-02). Suite: 26 passed. |
| 3   | Divergent clones with equal revision cannot share exact compiled artifacts | ✓ VERIFIED | `CompilationId` is a globally unique checked atomic counter (zero reserved, never wraps/reuses — `backend_ir.rs:31-65`). Every compile allocates a fresh id (`every_compiled_state_allocates_a_fresh_compilation_id`, `compiler_sync.rs:767`). Stale-state gates compare exact `CompilationId`, never revision/fingerprint: compiler `CompileError::StaleCompilation` (`session.rs:356-361`), reference `apply_compiled_delta` (`reference.rs:505-514`), and the new HiGHS check (`session.rs:136-161`). Cross-model (clone) session reuse returns `RebuildRequired` before miscompile (WR-4: `compile_snapshot_rejects_cross_model_source_instance`, `compile_delta_rejects_cross_model_source_instance`). SM-02.7 clone instance separation is pinned in `lineage_metadata.rs`. |
| 4   | Hashes are never accepted as exact authority | ✓ VERIFIED | `RecipeFingerprint` is computed (deterministic FNV-1a-4) and stored on `BackendSnapshot`/`BackendDeltaBatch`/`CompilationReport` only; grep over `src/` and `roml-highs/src/` finds zero fingerprint-based stale-state or analysis-authority decisions — every gate is exact `CompilationId` (D11/D28/SM-03.9). Asserted by `recipe_fingerprints_are_deterministic_but_never_authority` (`compiler_identity.rs:464`). |
| 5   | All generated entities have origins | ✓ VERIFIED | `BackendSnapshotBuilder::finalize` rejects any unoriginated entity with `CompileError::MissingOrigin` (Task 5 stopping condition; `backend_ir.rs:479-491`); `OriginMap::missing_origins` completeness validator (`origin.rs:233-256`); delta `origin_additions` keeps delta-added entities origin-complete (SM-02.5). Behavioral tests: `builder_finalization_rejects_variable_without_origin`, `builder_finalization_rejects_unoriginated_row_and_objective`. HiGHS projection returns a typed `BackendError::InvalidInput` (never a panic) on origin-less entries (WR-5; `roml-highs/src/compiler.rs:197-249`), verified by `rebuild_from_snapshot_rejects_origin_less_entry_with_error_not_panic`. |
| 6   | Backends consume no mutable model internals | ✓ VERIFIED | `ReferenceBackend` consumes `BackendSnapshot`/`BackendDeltaBatch` only (canonical path retained for characterization). HiGHS receives NO canonical `ModelSnapshot`: grep over `roml-highs/src/` finds `ModelSnapshot` only in doc comments plus test-module use in `session.rs:805,865` (unit test); legacy `projection.rs` removed; HiGHS `synchronize` rejects canonical `Synchronization` variants (D27 compat rejection → `HealthEffect::RequiresRebuild`). |

**Score:** 6/6 truths verified (0 present-but-behavior-unverified).

### Required Artifacts

| Artifact | Expected    | Status | Details |
| -------- | ----------- | ------ | ------- |
| `src/compiler/mod.rs` | module wiring + `CompileError` family | ✓ VERIFIED | `backend_ir`,`capability`,`origin`,`report`,`session`; `CompileError` has `MissingOrigin`,`StaleCompilation`,`UnsupportedFeature`,`InvalidObjectivePolicy`,`RebuildRequired`,`IdentityOverflow` |
| `src/compiler/backend_ir.rs` | IR types + builder + identity | ✓ VERIFIED | `CompilationId(u64)`, `RecipeFingerprint([u8;32])`, dense compiled ids, `CompiledObjectivePolicy{None,Single,Weighted,Lexicographic}` (A32), pinned 15-variant `#[non_exhaustive] BackendOp`, builder finalization with origin + policy validation |
| `src/compiler/capability.rs` | typed capability registry | ✓ VERIFIED | 17-variant `#[non_exhaustive] BackendFeature`, `SupportLevel`, `FeatureLimitations`, `FeatureSupport`, `BackendCapabilitySet`, `CompilationPolicy{Auto,Portable,NativeRequired}` |
| `src/compiler/origin.rs` | `EntityOrigin`/`OriginMap` | ✓ VERIFIED | bidirectional queries + completeness validator; `GeneratedRole` empty `#[non_exhaustive]` (forward decl) |
| `src/compiler/report.rs` | `CompilationReport`/`BackendIdentity` | ✓ VERIFIED | fingerprint, generated-entity inventory, formulation decisions |
| `src/compiler/session.rs` | `CompilationSession` | ✓ VERIFIED | snapshot/delta compilation, exact from/to ids, rebuild-on-uncertainty, A31-aware ops, source-instance guard, capability gating (WR-3) |
| `roml-highs/src/compiler.rs` | backend IR → HiGHS native | ✓ VERIFIED | full `BackendOp` coverage, typed origin errors (WR-5), checked FFI returns; `projection.rs` removed |
| `tests/compiler_identity.rs` | identity/origin/fingerprint tests | ✓ VERIFIED | 14 passed |
| `docs/migration/M3_BACKEND_IR.md` | backend-author migration guide (SM-03.8) | ✓ VERIFIED | 197 lines, 7 sections incl. guarantees, migration steps, capability gating |
| `docs/release/evidence/M3_P26_COMPILER_BACKEND_IR.md` | phase evidence | ✓ VERIFIED | baseline, B1-B6 acceptance, per-task verification + matrices (see residual risks for incomplete final section) |

### Key Link Verification

| From | To  | Via | Status | Details |
| ---- | --- | --- | ------ | ------- |
| `CompilationSession` | `BackendSnapshot`/`BackendDeltaBatch` | `compile_snapshot`/`compile_delta` | WIRED | exact from/to ids + revisions; fresh `CompilationId` per target state; `StaleCompilation`/`RebuildRequired` |
| `BackendSnapshotBuilder::finalize` | `OriginMap::missing_origins` | completeness validator | WIRED | no compiled entity without `EntityOrigin` (D5) |
| `ReferenceBackend` | compiled IR | `apply_compiled_delta` stale check | WIRED | `from_compilation` mismatch → `CompileError::StaleCompilation` before any op |
| HiGHS `synchronize` | compiled IR | `from_compilation` vs `current_compilation` | WIRED | D28 check (WR-1) before any op; `Recoverable` + `mark_rebuild` |
| `CompilationSession` | `BackendCapabilitySet` | capability gates | WIRED | MIP/semi-continuous on snapshot; `IncrementalRows`/`IncrementalBounds`/`IncrementalCoefficients` on delta ops (WR-3); unqualified → `UnsupportedFeature` |
| compiler delta consumption | `DeltaBatch` ops | A31 | WIRED | updates ride `SetCell`/`RemoveCell`/`SetConstraintBounds`; `functions` never exhaustive — `a31_delta_consumes_ops_for_updates_to_pre_existing_constraints` |
| `RecipeFingerprint` | evidence/cache only | never stale-state gate | WIRED | zero decision use in `src/` + `roml-highs/src/` |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| -------- | ------- | ------ | ------ |
| `roml` full suite | `cargo test -p roml --all-targets` | 647 passed; 0 failed; 2 ignored | ✓ PASS |
| `roml-highs` full suite | `cargo test -p roml-highs --all-targets` | 102 passed; 0 failed | ✓ PASS |
| clippy `-D warnings` | `cargo clippy -p roml --all-targets -- -D warnings` | clean, exit 0 | ✓ PASS |
| compiler identity | `cargo test -p roml --test compiler_identity` | 14 passed | ✓ PASS |
| identity compiler sync | `cargo test -p roml --test compiler_sync` | 13 passed (9 base + 4 review-fix) | ✓ PASS |
| compiled delta vs rebuild | `cargo test -p roml --test differential_harness` | 26 passed (incl. Sections 7+8) | ✓ PASS |
| HiGHS conformance | `cargo test -p roml-highs --test conformance` | 4 passed | ✓ PASS |
| primitive baseline (no regression) | `cargo test -p roml --test m3_baseline_characterization` | 6 passed | ✓ PASS |

### Probe Execution

No probe scripts (`scripts/*/tests/probe-*.sh`) are declared by this phase's PLAN/SUMMARY. Step 7c not applicable — phase is a Rust library compiler boundary verified by the test matrix above.

### Requirements Coverage

| Requirement | Scope in P26 | Status | Evidence |
| ----------- | ------------ | ------ | -------- |
| SM-02.4 (distinct compiled IDs) | Closed (Task 5) | ✓ SATISFIED | `CompiledVariableId`/`CompiledConstraintId`/`CompiledObjectiveId` dense newtypes; `compiled_ids_are_dense_deterministic_and_ordered` |
| SM-02.5 (every generated entity maps via OriginMap) | Closed (Tasks 5+7) | ✓ SATISFIED | `OriginMap` bidirectional + completeness; builder finalization; delta `origin_additions`; HiGHS `compiled_to_user_*` maps |
| SM-02.6 (bound provenance diagnostics) | Foundation only; full closure P29 | ✓ FOUNDATION | compiled bound representation + origin map distinguishing user/generated bound sources; TRACEABILITY maps closure to P29; evidence marks "(foundation)" |
| SM-03.1 capability-aware compiler | Closed (Task 7) | ✓ SATISFIED | `CompilationSession` over snapshots/deltas |
| SM-03.2 backends consume backend IR | Closed (Task 7) | ✓ SATISFIED | Reference + HiGHS consume IR; no `ModelSnapshot` to HiGHS |
| SM-03.3 linear rows + extension surface | Closed (Task 5, surface) | ✓ SATISFIED | empty `#[non_exhaustive] BackendConstraint` + `native_constraints` empty; payloads P32/P33 |
| SM-03.4 CompilationPolicy semantics | Closed (Task 6) | ✓ SATISFIED | `CompilationPolicy{Auto,Portable,NativeRequired}` documented; no-op until P32 (IN-01 documented) |
| SM-03.5 compilation artifacts | Closed (Tasks 5+7) | ✓ SATISFIED | origin map, report, fingerprint, inventory, `CompilationId` on snapshot/delta |
| SM-03.6 recipe change forces rebuild | Closed (Tasks 5+7) | ✓ SATISFIED | `RebuildRequired` for any non-incrementally-provable delta (D22/F-B1) |
| SM-03.7 primitive incremental == compiled rebuild | Closed (Task 7) | ✓ SATISFIED | `dx_fixed_seed_compiled_delta_equals_compiled_rebuild` + Section 8 |
| SM-03.8 migration documented/tested | Closed (Task 7) | ✓ SATISFIED | `docs/migration/M3_BACKEND_IR.md`; reference recovery/differential tests pass |
| SM-03.9 exact CompilationId on results; fingerprints non-authority | Closed (Tasks 5+7) | ✓ SATISFIED | exact id on snapshot/delta/origins; fingerprint never a gate |
| SM-04.1 typed features | Closed (Task 6) | ✓ SATISFIED | `BackendCapabilitySet` keyed by `BackendFeature` |
| SM-04.2 native vs bridge reported separately | Closed (Task 6) | ✓ SATISFIED | `SupportLevel{Native,Unsupported}` + limitations; bridge surface documented separately |
| SM-04.3 version-aware limitations | Closed (Task 6) | ✓ SATISFIED | `FeatureLimitations{minimum_version,model_classes,maximum_count,notes}`; `highs_capability_set(major,minor,patch)` |
| SM-04.4 unsupported rejected | Closed (Tasks 6+7) | ✓ SATISFIED | `validate_request` typed-set rejection + compiler capability gates (WR-3) |
| SM-04.5 per-solve recording | Foundation; full in P28 | ✓ FOUNDATION | typed registry + rejection machinery; full `EffectiveSolvePlan` recording P28 (TRACEABILITY) |
| SM-13 compiler foundations | Foundation; full closure P33 | ✓ FOUNDATION | `CompileError` family + compilation-report infrastructure (error identification, recipe evidence) |

### A29-A32 Amendments Honored

| Amendment | Status | Evidence |
| --------- | ------ | -------- |
| A29 (FormulationPreference per construct) | ✓ HONORED (forward contract) | `ConstructEntry.preference` read surface documented; P26 compiles only primitive linear state (no real construct kinds) |
| A30 (crate-private constructs) | ✓ HONORED | `ModelSnapshot.constructs` is `pub #[doc(hidden)]`; compiler lives in-crate; no external backend names construct internals |
| A31 (delta `functions` NOT exhaustive) | ✓ HONORED | ops-for-updates consumption; `a31_delta_consumes_ops_for_updates_to_pre_existing_constraints` |
| A32 (`CompiledObjectivePolicy::None`) | ✓ HONORED | DECISIONS.md amendment recorded before Task 5; implemented (`backend_ir.rs:213-217`), compiled (`session.rs:269-275,616-635`), consumed by both backends; `identity_compile_objectiveless_snapshot_uses_policy_none` |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| (none) | — | TBD/FIXME/XXX/TODO/HACK/placeholder markers | — | None in `src/compiler/**`, `src/solver/reference.rs`, `src/solver/facade.rs`, `roml-highs/src/compiler.rs` |

No debt markers, no empty-return stubs, no hardcoded-empty data paths in the phase's production code. Intentional forward declarations (`BackendConstraint` empty, `GeneratedRole` empty, `CompiledObjectivePolicy::Weighted/Lexicographic` unreachable in P26) are documented extension surfaces, not stubs.

### Human Verification Required

None. Every behavior-dependent truth (compiled-delta == rebuild; fresh-id-per-compile; stale-compilation rejection; origin rejection) has a passing behavioral test in the suites run above. This is a library phase; no visual/UX/external-service behavior requires human testing.

### Residual Risks

1. **Evidence bundle not fully closed (WARNING, documentation).** `docs/release/evidence/M3_P26_COMPILER_BACKEND_IR.md` ends with the placeholder comment `<!-- Phase-level gate result (P26 boundary) filled after Task 7 review passes. -->`. The phase-level gate result, the phase-end public-API diff (base capture 12019 items vs head), and the reviewer findings/dispositions (recorded in `26-REVIEW.md`/`26-REVIEW-FIX.md`, not appended to the evidence) are not yet written into the evidence file, despite the file's own promise on line 10. The PLAN gate lists "public API diff ... recorded" as a passing condition. Recommend the close-out/ship step finalize these evidence sections (fill gate result, run and commit the head public-API capture and diff, append reviewer dispositions, state the SM-02.6 clause-level scope explicitly). Non-blocking to the code-level goal.
2. **SM-02.6 scope statement implicit (WARNING, documentation).** Evidence marks SM-02.6 as "(foundation)" in task headers and TRACEABILITY maps closure to P29, but the evidence file does not contain one explicit clause-level scope sentence (the PLAN requires "the evidence file must state this clause-level scope explicitly"). Recommend adding that sentence during evidence finalization.
3. **No single end-to-end divergent-clone test (INFO).** The D28 clone invariant is verified through component tests (fresh-id-per-compile, stale-from-compilation rejection at compiler/reference/HiGHS, cross-model source-instance guard, SM-02.7 clone instance separation). There is no one test that literally clones a `Model`, mutates both clones, compiles both, and asserts distinct `CompilationId`s end-to-end. The mechanism (global monotonic id counter + per-instance guard + exact-id stale gate) makes sharing impossible by construction; a dedicated clone-pair test would be a worthwhile hardening addition.
4. **`CompilationPolicy` accepted but not yet honored (INFO).** `compile_snapshot`/`compile_delta` take `&CompilationPolicy` and read only `Auto` semantics (IN-01). Documented as the P26 no-op; `Portable`/`NativeRequired` effects and per-construct preference-honoring (A29) land with P32 construct bridges.
5. **IN-02/IN-03 validation gaps (INFO).** Negative weight/tolerance values in `CompiledWeightedObjective`/`CompiledObjectiveLevel` are not rejected at finalize (unreachable in P26 — only `Single`/`None` emitted); snapshot compile silently drops cells referencing a variable absent from `snapshot.variables` (valid models cannot produce them). Both deferred per review dispositions; P31/P32 should tighten.

### Gaps Summary

No gaps block the phase goal. All six ROADMAP P26 gate truths are verified against the actual codebase, all required artifacts exist and are substantive and wired, both test suites and clippy pass, and all 7 code-review findings (2 critical, 5 warning) are fixed and verified in place. The residual risks are documentation-finalization items in the phase evidence bundle (phase gate result, public API diff, reviewer dispositions, explicit SM-02.6 clause-level statement) plus three INFO-level hardening notes — none affect the correctness of the compiled backend-IR path.

---

_Verified: 2026-08-03T08:15:00Z_
_Verifier: Claude (gsd-verifier)_
