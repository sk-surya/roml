---
phase: 25-semantic-ir-foundation
verified: 2026-08-02T23:40:00Z
status: passed
score: 4/4 must-haves verified
behavior_unverified: 0
overrides_applied: 0
gaps: []
deferred:
  - truth: "PrimalAssignment cross-lineage rejection (full assignment-reuse mechanism)"
    addressed_in: "Phase 27 (P27-fixing-locks-overlays)"
    evidence: "P27 goal: 'support hard solution reuse'; deliverable 'PrimalAssignment with lineage compatibility and instance/revision provenance'; gate: 'exact compilation mismatches reject before mutation'. P25 delivers only the lineage identity + semantics foundation (SM-02.1, SM-02.2 foundation)."
---

# Phase 25: Canonical Semantic IR, Identities, and Metadata — Verification Report

**Phase Goal:** establish semantic canonical state before adding workflows.
**Verified:** 2026-08-02T23:40:00Z
**Status:** passed
**Re-verification:** No (initial verification)

## Goal Achievement

The P25 gate (ROADMAP, verbatim) requires four observable behaviors. All four are
established in the codebase and exercised by passing behavioral tests. The solve-path
binding defect (CR-02) and the constant-folding delta divergence (CR-01) found by
independent review were fixed and are covered by TDD tests that failed before the fix.

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Existing linear models remain observationally equivalent | ✓ VERIFIED | `tests/m3_baseline_characterization.rs` (6 behavioral tests: fluent modeling, deterministic snapshot round-trip, parameter update, objective constant, solution metadata, one-rebuild-retry) pass. `cargo test -p roml --all-targets` exit 0; `cargo test -p roml-highs --all-targets` exit 0. Public API diff from base `7b124ad`: +1571 / −6, the −6 being `Model::clone`/`Model::default` moving derived→manual with identical public signatures (M2 surface preserved, SM-01.5/SM-15.1). |
| 2 | Independent models reject cross-lineage assignments | ✓ VERIFIED | `src/identity.rs` defines opaque `ModelLineageId` (checked atomic counter, zero reserved). `tests/lineage_metadata.rs`: `independent_models_never_share_lineage_or_instance`, `lineage_and_instance_ids_are_unique_across_many_models` pass. Lineage-governs-reuse semantics documented (identity.rs §4.1, SM-02.1/SM-02.2 foundation). Full `PrimalAssignment` mechanism is explicitly P27 (see Deferred). |
| 3 | Clones share lineage but never instance identity | ✓ VERIFIED | Manual `impl Clone for Model` (`src/model/mod.rs:209-231`) preserves `lineage`, allocates new `instance` (D28 — a derived Clone would silently copy the instance). CR-02 fixed: `SolverSession::solve_with` threads `model.lineage()`/`model.instance()` into `normalize_result` (`src/solver/facade.rs:226-234, 47-59`), so the SOLVED model's ids are bound — no fresh unrelated counter ids. Tests: `clone_preserves_lineage_but_allocates_new_instance`, `real_solve_binds_model_lineage_and_instance_into_metadata` pass (the latter failed before CR-02). |
| 4 | Every construct fixture survives clone/snapshot/delta/activity/remove/rebuild | ✓ VERIFIED | Generation-safe `ConstructStore` (`src/construct/mod.rs`), typed `ConstructNotFound` for stale ids. `tests/semantic_ir.rs` construct section (7 tests): add/stable-id, clone preserves ids+activity, snapshot/delta round-trip, activity in snapshot, remove-invalidates + stale-id typed rejection, rebuild restores equal content (by-id full-entry assertions, IN-03), metadata-via-EntityRef, remove-cascades-metadata (WR-06). All pass. |

**Score:** 4/4 truths verified (0 present-but-behavior-unverified — every truth is
exercised by a passing behavioral test, not symbol presence alone).

### Deferred Items

Items not yet met but explicitly addressed in later milestone phases.

| # | Item | Addressed In | Evidence |
|---|------|-------------|----------|
| 1 | `PrimalAssignment` lineage/instance validation (assignment-reuse) | Phase 27 | P27 goal "support hard solution reuse"; deliverable "`PrimalAssignment` with lineage compatibility and instance/revision provenance"; gate "exact compilation mismatches reject before mutation". P25 contributes the lineage identity and its reuse semantics foundation only. |

### Required Artifacts

| Artifact | Expected | Status | Details |
| -------- | -------- | ------ | ------- |
| `src/identity.rs` | Opaque `ModelLineageId`/`ModelInstanceId`/`ConstructId` | ✓ VERIFIED | Checked per-family atomic counters, zero reserved, saturating overflow (WR-03 fixed), testable overflow seam (IN-02). |
| `src/metadata.rs` | `ModelSource`, `EntityMetadata`, `EntityRef` | ✓ VERIFIED | Exact design §5 fields; `EntityRef::Construct` usable via arena. |
| `src/function/mod.rs`, `scalar.rs`, `set.rs` | `#[non_exhaustive]` `ScalarFunction::Linear`, `ScalarSet` (4 variants), `FunctionConstraint`, `IntoScalarFunction` | ✓ VERIFIED | Design §6 shapes verbatim. |
| `src/construct/mod.rs` | `Construct`, `#[non_exhaustive]` `ConstructKind`, `ConstructEntry`, `FormulationPreference`, generation-safe arena | ✓ VERIFIED | Fixture-only kind by P25 scope note (per-construct modules P30/P32/P33); `#[non_exhaustive]` extension boundary declared. |
| `tests/m3_baseline_characterization.rs` | 6 M2 characterization tests | ✓ VERIFIED | 6 passed. |
| `tests/lineage_metadata.rs` | Lineage/instance/metadata + solve-path binding tests | ✓ VERIFIED | 8 passed (incl. CR-02, WR-05). |
| `tests/semantic_ir.rs` | Function-in-set + construct lifecycle tests | ✓ VERIFIED | 19 passed (incl. CR-01, WR-01, WR-06). |
| `docs/release/evidence/M3_P25_SEMANTIC_IR.md` | Baseline matrix, focused verification, public API diff, reviewer dispositions | ✓ VERIFIED | Present; records base SHA `7b124ad`, untouched matrices, +1571/−6 public API diff, CR/WR dispositions. |

### Key Link Verification

| From | To | Via | Status | Details |
| ---- | --- | --- | ------ | ------- |
| `Model::default` / `Model::clone` | `ModelLineageId` / `ModelInstanceId` | manual impls (`src/model/mod.rs:185-232`) | ✓ WIRED | Default allocates both fresh; Clone preserves lineage, reallocates instance. |
| `SolverSession::solve_with` | `SolveMetadata` | `normalize_result(model.lineage(), model.instance())` (`src/solver/facade.rs`) | ✓ WIRED | CR-02: no fallback to fresh default ids; test `real_solve_binds_model_lineage_and_instance_into_metadata` asserts equality. |
| `Model::take_snapshot` / `DeltaBatch::new` | semantic `functions`/`constructs` entries | reconstruction from coefficient index / arena ops (`src/snapshot.rs:174-278`, `src/delta.rs:299-401`) | ✓ WIRED | Derived views, never second authorities (SM-01.1). CR-01: delta `set` folds last same-batch `SetConstraintBounds`; invariant checks compare `set` AND `function` against `constraint_function` (`src/model/mod.rs:1632-1649`). |
| `set_metadata` | metadata store | liveness check then `metadata.insert` (`src/model/mod.rs:313-338`) | ✓ WIRED | Stale entities rejected (typed `*NotFound`); no changelog push / no revision advance (non-solver-affecting). |
| Construct arena | snapshot/delta/changelog | `ConstructStore` + `Change::ConstructAdded/Removed/ActivityChanged` + `ModelOp::Add/Remove/SetConstructActive` | ✓ WIRED | Rebuild restores equal content; stale ids rejected with typed `ConstructNotFound`. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
| -------- | ------------- | ------ | ------------------ | ------ |
| `Snapshot.functions` | `FunctionEntry` | reconstructed from coefficient index (`constraint_function`) | ✓ real (round-trip tests assert equality with canonical view) | ✓ FLOWING |
| `DeltaBatch.functions` | `FunctionEntry` | reconstructed from ops incl. folded bounds (CR-01) | ✓ real (`delta_set_reflects_bounds_folded_from_expression_constant` asserts `entry.set == model.constraint_function(con).set`) | ✓ FLOWING |
| `SolveMetadata.model_lineage/instance` | opaque ids | `model.lineage()` / `model.instance()` on real solve path | ✓ real (no static/empty fallback) | ✓ FLOWING |
| `ModelSnapshot.constructs` / `DeltaBatch.constructs` | `ConstructEntry` | construct arena / ops | ✓ real (lifecycle tests verify kind + activity through each transition) | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| -------- | ------- | ------ | ------ |
| roml full regression suite | `cargo test -p roml --all-targets` | exit 0 | ✓ PASS |
| roml-highs full regression suite (M2-era HiGHS) | `cargo test -p roml-highs --all-targets` | exit 0 | ✓ PASS |
| P25 baseline characterization | `cargo test -p roml --test m3_baseline_characterization` | 6 passed | ✓ PASS |
| P25 lineage/metadata | `cargo test -p roml --test lineage_metadata` | 8 passed | ✓ PASS |
| P25 semantic IR + construct lifecycle | `cargo test -p roml --test semantic_ir` | 19 passed | ✓ PASS |

### Requirements Coverage

All 12 requirement IDs declared for P25 are accounted for with implementation + test
evidence. No orphaned requirements (TRACEABILITY.md P25 closure list matches the plan's
`requirements` field exactly).

| Requirement | Description | Status | Evidence |
| ----------- | ----------- | ------ | -------- |
| SM-01.1 | Canonical function-in-set state; coefficient index single authority | ✓ SATISFIED | `constraint_function`, derived snapshot/delta views, invariant checks (`model/mod.rs:1632-1649`); tests `ordinary_builder_round_trips_through_coefficient_index`, `model_invariants_verify_legacy_fields_against_semantic_view`. |
| SM-01.2 | `#[non_exhaustive]` `ScalarFunction`/`ScalarSet`; linear only | ✓ SATISFIED | `src/function/scalar.rs`, `src/function/set.rs`. |
| SM-01.3 | Construct stable handle, metadata, activity, parameter deps | ✓ SATISFIED | `ConstructEntry { id, kind, active }`, `ConstructData { metadata, parameter_dependencies }`; tests `construct_add_returns_stable_id...`, `construct_parameter_dependencies` API. |
| SM-01.4 | Snapshots/deltas include semantic constructs | ✓ SATISFIED | `ModelSnapshot.functions/constructs`, `DeltaBatch.functions/constructs`; round-trip tests. |
| SM-01.5 | M2 `LinExpr`/builder path stays canonical | ✓ SATISFIED | `tests/m3_baseline_characterization.rs` (6 passed); full roml + roml-highs suites exit 0. |
| SM-01.6 | No backend index/Big-M/handle/overlay in canonical state | ✓ SATISFIED | `Model` fields all canonical (no backend handle); construct `ModelOp` variants are explicit no-op arms in `src/solver/reference.rs:215-217` and `roml-highs/src/projection.rs:852-854`. |
| SM-02.1 | Opaque `ModelLineageId`; clones preserve lineage | ✓ SATISFIED | `src/identity.rs`; tests `independent_models_never_share_lineage_or_instance`, `clone_preserves_lineage_but_allocates_new_instance`. |
| SM-02.2 (foundation) | Lineage identity semantics for reuse compatibility | ✓ SATISFIED (foundation) | Lineage governs reuse documented; distinct-lineage tests. Full assignment validation is P27 (deferred). |
| SM-02.3 | Metadata: description/group/tags/source per entity | ✓ SATISFIED | `metadata_round_trips_per_entity` (all four entity kinds); `set_metadata` liveness + cascade removal (WR-05). |
| SM-02.5 (foundation) | Generated entities map to construct/user/overlay (identity surface) | ✓ SATISFIED (foundation) | Stable `ConstructId` + `EntityRef::Construct` identity surface exists (P26 compiler will map generated entities to it); `construct_metadata_usable_via_entity_ref`. |
| SM-02.7 | Distinct `ModelInstanceId`; clone preserves lineage, new instance | ✓ SATISFIED | Manual `Clone`; `real_solve_binds_model_lineage_and_instance_into_metadata` (CR-02). |
| SM-15.1 (foundation) | M2 golden-path source compatibility preserved | ✓ SATISFIED | Baseline characterization green; public API diff −6 = derived→manual with identical signatures; full suites exit 0. |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| — | — | None | — | No TBD/FIXME/XXX/TODO/HACK/PLACEHOLDER/unimplemented markers in any P25 file. `ConstructKind::Fixture` is a deliberate, documented P25 scope decision (per-construct modules P30/P32/P33), not a stub. |

### Human Verification Required

None. All four gate truths are behavior-dependent and are exercised by passing
behavioral tests I ran (not symbol presence alone). No visual/realtime/external-service
surface exists (library crate).

### Gaps Summary

No gaps. The phase goal — semantic canonical state (identity, metadata, function-in-set,
construct arena) established before workflows — is achieved and evidenced.

### Residual Risks

1. **CR-02 test backend is minimal** — `real_solve_binds_model_lineage_and_instance_into_metadata` drives the real `SolverSession::solve` facade through a deterministic in-memory `BackendSession` (deliberately identity-free). The binding lives in the backend-agnostic `normalize_result` path, so the Reference/HiGHS paths share it; a HiGHS-level assertion is not present.
2. **Construct `ModelOp` no-ops** — constructs are canonical entities only until P26 compiles them to backend rows. SM-01.6 holds by construction.
3. **Counter-exhaustion panic boundary** — `Model::default`/`Clone`/`SolveMetadata::default` panic when a family counter is truly exhausted (documented, WR-04); the counters saturate so ids are never re-issued. Unreachable at 2^64 allocations.
4. **Assignment-reuse mechanism deferred** — the full `PrimalAssignment` lineage/revision validation is P27; P25 delivers only the lineage identity + semantics.
5. **Known-broken adapters** — `roml-mosek`/`roml-xpress` remain broken against the current facade (pre-existing, out of P25 scope, matching M2 convention).

---

_Verified: 2026-08-02T23:40:00Z_
_Verifier: Claude (gsd-verifier)_
