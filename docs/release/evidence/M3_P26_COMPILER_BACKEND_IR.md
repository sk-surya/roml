# P26 Evidence — Compiler Boundary, Backend IR, Capabilities, Origins, and Exact Compilation Identity

**Phase:** 26-compiler-backend-ir
**Plan:** `26-PLAN.md` — Task 0 (backend-contract amendment review gate)
**Requirements (Task 0):** SM-03.2 (foundation), SM-03.8 (foundation), SM-02.4 (foundation)
**Branch:** `phase-roml-P26-compiler-backend-ir`
**Base:** `main@9c2a9df254321792ef0e3cae275a662c798572c9`
**Status:** Task 0 complete — acceptance record written, baseline captured, no source code modified.

This document records the P26 Task 0 deliverables per `EXECUTION.md` § "Evidence file structure": the untouched baseline matrix and the backend-contract amendment acceptance record (the STATE.md blocking gate for P26). Later tasks (5, 6, 7) append their per-task verification, RED failures, the full phase verification matrix, the public API diff, and reviewer dispositions.

## Scope and requirements

Task 0 is a review pass over the design §8 backend contract and the packet Tasks 5–7 interface contract **before any implementation**, plus the untouched baseline capture. It produces an explicit acceptance record with a disposition for every review point (B1–B6) and records the implications of the P25 amendments A29–A31 for what the P26 compiler reads from canonical snapshots/deltas. Task 0 modifies no production source file; the only artifacts are this evidence file and the required `DECISIONS.md` amendment (A32, recorded below).

## Baseline and environment

| Item | Value |
|---|---|
| Base commit (`main`) | `9c2a9df254321792ef0e3cae275a662c798572c9` (P25 merged, PR #27) |
| HEAD at baseline capture | `ecb75a01018614c0156156dd5ed2bb15dc168148` (`docs(26): plan P26 compiler boundary and backend IR`) |
| Branch | `phase-roml-P26-compiler-backend-ir` |
| `rustc --version` | `rustc 1.97.1 (8bab26f4f 2026-07-14)` |
| `cargo --version` | `cargo 1.97.1 (c980f4866 2026-06-30)` |
| `rustc -vV` host | `aarch64-apple-darwin` |
| OS | `Darwin 25.4.0 arm64` |
| `cargo public-api --version` | `cargo-public-api 0.52.0` |
| HiGHS build | bundled via `highs-sys 1.15.0` (cmake); no system HiGHS |

All commands ran on the platform above with the toolchain above at the HEAD above, on the **untouched** tree (no P26 source modification; working tree clean except pre-existing untracked `.planning/config.json`, `.planning/graphs/`, `graphify-out/`).

### Untouched baseline matrix — `roml`

| Command | Exit | Result |
|---|---|---|
| `cargo fmt --all -- --check` | 0 | formatting clean |
| `cargo check -p roml --all-targets` | 0 | clean |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `cargo test -p roml --all-targets` | 0 | **600 passed; 0 failed; 2 ignored** (27 test targets) |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 | docs generated, no warnings |
| `cargo package --list -p roml` | 0 | 79 files |
| `cargo public-api -p roml` | 0 | 12019 public items |
| `cargo package -p roml --locked` | 0 | package verified |

### Untouched baseline matrix — `roml-highs`

| Command | Exit | Result |
|---|---|---|
| `cargo check -p roml-highs --all-targets` | 0 | clean |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `cargo test -p roml-highs --all-targets` | 0 | **100 passed; 0 failed; 0 ignored** (17 test targets) |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` | 0 | docs generated, no warnings |
| `cargo package --list -p roml-highs` | 0 | 32 files |
| `cargo public-api -p roml-highs` | 0 | 106 public items |
| `cargo package -p roml-highs --locked` | 1 | **SKIP (pre-existing):** fails with `no matching package named 'roml' found` — `roml` is an unpublished path dependency, so the packed `roml-highs` cannot resolve it from the crates.io index. Pre-existing workspace packaging limitation, unrelated to P26 (recorded per the plan's "skips must be recorded, never treated as passing"). |

The `roml` `--all-targets` total (600 passed) matches the P26 baseline expectation; `roml-highs` (100 passed) is unchanged from P25. The 2 ignored `roml` tests and the package-list composition are unchanged from P25.

### Public API / package capture

Raw captures at the P26 base were recorded to temporary artifacts (paths absolute; per the M2/M3 convention the phase-end diff normalizes repository paths to the `$REPO` token):

- `cargo public-api -p roml` — **12019** items (raw).
- `cargo public-api -p roml-highs` — **106** items (raw), identical count to the committed `M3_P25_public_api_roml_highs.txt`.
- `cargo package --list -p roml` — **79** files.
- `cargo package --list -p roml-highs` — **32** files.

**Finding F-C (WARNING, evidence consistency):** the committed `docs/release/evidence/M3_P25_public_api_roml_final.txt` (12016 lines) is **stale** relative to merged `main@9c2a9df`. A normalized diff against the P26 base capture (12019 items) shows the P25 final file records the intermediate F1 shape from the first `be67053` fix (`pub roml::advanced::FunctionEntry::terms`/`::dependencies` and `pub roml::function::FunctionConstraint::terms`/`::dependencies`), which the F1 re-fix `791708f` removed in favor of the derived `parameter_dependencies()` methods now present on `LinExpr`, `ScalarFunction`, and `FunctionConstraint`. The P25 final capture was not refreshed during the P25 re-verification round. **Disposition: accepted** — the P26 phase-end public-API diff must be computed against this P26 base capture (and/or a fresh head capture), not the stale P25 final file; the discrepancy is evidence-documentation drift, not a P26 implementation issue.

## Backend contract amendment acceptance

**Verdict: PASSED** — no P0/P1 (BLOCKER) findings. 1 review point amended (B1, via `DECISIONS.md` A32 recorded below), 5 review points confirmed, 1 evidence-consistency WARNING accepted, 5 INFO findings disposed. Every review point B1–B6 carries an explicit disposition. No implementation task may start until this record is written and its blockers (none) resolved — this record satisfies the P26 STATE.md blocking gate.

### B1 — `BackendSnapshot` compiled objective policy — **AMENDED**

`objective_policy: CompiledObjectivePolicy` on `BackendSnapshot` (design §8.4, packet interface contract) is **Confirmed**. The resolution point is real and verified against the code: M2 supports objective-less models — `objectiveless_rebuild` in `src/solver/reference.rs` (lines 552–583) rebuilds a snapshot with an empty objective set; `ReferenceBackend` tracks `active_objective: Option<ObjId>`; and `ModelOp::SetActiveObjective { obj: None }` is part of the primitive operation set. `CompiledObjectivePolicy { Single, Weighted, Lexicographic }` has **no representation for "no active objective"**, and `Single(id)` cannot be reused because the id must reference a compiled objective that does not exist. A new representation is therefore required.

**Disposition: Amended** — `DECISIONS.md` amendment **A32** adds `CompiledObjectivePolicy::None` as the compiled representation of no-active-objective. The identity compiler maps a snapshot with no active objective to `None`; `SetActiveObjective { obj: None }` compiles to `BackendOp::SetObjectivePolicy(CompiledObjectivePolicy::None)`. This preserves the M2 reference-backend objective-less behavior through the compiled path. A32 is committed before Task 5. `Weighted`/`Lexicographic` remain reachable only from the P31 canonical `ObjectivePolicy` (design §15) — in P26 only `Single`/`None` are reachable.

### B2 — `BackendDeltaBatch` exact from/to compilation IDs — **CONFIRMED**

Design §8.3, the packet interface contract, and the plan Task 5 acceptance criterion agree: `BackendDeltaBatch { from_compilation: CompilationId, to_compilation: CompilationId, from_revision: ModelRevision, to_revision: ModelRevision, operations: Vec<BackendOp>, recipe_fingerprint: RecipeFingerprint }`. Every batch carries exact from/to compilation IDs and revisions; the compiler allocates a fresh `CompilationId` per target state. Divergent clones with equal `ModelRevision` never share a `CompilationId` (D28; must-have truth 3) — canonical state identity is `(ModelInstanceId, ModelRevision)`, and the compiled state identity is the `CompilationId`. `BackendSnapshot.source_instance`/`source_revision` tie the compiled artifact back to exact canonical state.

### B3 — Full `BackendOp` enumeration — **CONFIRMED** (with rebuild-on-uncertainty carve-out)

The packet interface-contract enumeration is pinned in this acceptance record (design §8.3: the full enumeration is review-gated with the implementation plan — Task 7; this Task 0 record is that gate). The pinned 15-variant enumeration is:

`AddVariable`, `RemoveVariable`, `SetVariableBounds`, `AddLinearRow`, `RemoveLinearRow`, `SetLinearRowBounds`, `SetLinearCoefficient`, `RemoveLinearCoefficient`, `AddObjective`, `RemoveObjective`, `SetObjectiveCoefficient`, `RemoveObjectiveCoefficient`, `SetObjectiveConstant`, `SetObjectiveSense`, `SetObjectivePolicy(CompiledObjectivePolicy)`.

This includes the three explicitly required variants: `RemoveLinearCoefficient { constraint, variable }`, `RemoveObjectiveCoefficient { objective, variable }`, and `SetObjectivePolicy(CompiledObjectivePolicy)`, plus `SetObjectiveConstant`/`SetObjectiveSense` (needed to keep objective constants and senses flowing incrementally; `SetObjectiveCell` in `ModelOp` reports the constant exactly once, API-03.5). `BackendOp` is `#[non_exhaustive]`.

**Finding F-B1 (INFO):** the enumeration does not include explicit ops for `SetVariableType`, `SetVariableActive`, `SetConstraintActive`, `SetParameter`, and `SetSemiContinuousBound` — all real M2 `ModelOp`s handled by `ReferenceBackend::apply_op`. **Disposition: accepted** — design §18 "any uncertainty selects rebuild": any delta containing an op the identity compiler cannot prove incrementally equivalent (including these) forces a deterministic rebuild; no `BackendDeltaBatch` is emitted for it. No enumeration gap blocks implementation.

### B4 — `CompilationId`/`RecipeFingerprint` authority rules — **CONFIRMED**

D28 and design §4.3 hold as they apply to the compiler: `ModelLineageId` governs assignment compatibility across clones; `ModelInstanceId + ModelRevision` identifies exact canonical state; `CompilationId` is the exact comparison key for stale-state safety and is carried by every `BackendSnapshot`, `BackendDeltaBatch` (from/to), origin map, and (P28 onward) result/overlay metadata; `RecipeFingerprint` is deterministic evidence/cache support only and is **never** stale-state authority (D11/D28, must-have truth 4). `CompilationId(u64)` mirrors the checked atomic-counter opaque-id pattern in `src/identity.rs` (zero reserved, typed overflow). Recipe fingerprints are equal only for equal compiled states and are never used as a gate.

### B5 — A29–A31 implications for what the compiler reads from canonical snapshots/deltas — **CONFIRMED**

All three P25 amendments are verified against the implemented canonical state and their compiler-facing consequences are recorded:

- **A29 (preference honored):** `ConstructEntry { id, kind, active, preference }` is verified as the single per-construct authority (`src/construct/mod.rs` lines 64–80); `preference` threads through `Change::ConstructAdded` (`src/model/changelog.rs` lines 217–228), `ModelOp::AddConstruct` (`src/delta.rs` lines 190–204), and the snapshot/delta reconstruction (`reconstruct_construct_entries`). The P26 compiler reads `FormulationPreference` (`Auto`/`Portable`/`NativeRequired`) from the canonical entry and may narrow the global `CompilationPolicy` per construct without weakening exactness (design §8.1). In P26 no real construct kinds exist (only the crate-private `Fixture`), so preference-honoring is the forward contract P32's construct bridges will exercise; the compiler's read surface must not depend on any removed `ConstructData.preference`.
- **A30 (crate-private constructs):** the construct module is crate-private (`pub(crate)` `ConstructStore`; only `Construct` and `FormulationPreference` are public — `src/construct/mod.rs` lines 15–23); `ModelSnapshot.constructs` is `pub #[doc(hidden)]` (`src/snapshot.rs` lines 62–68). The P26 compiler lives in-crate (`src/compiler/**`) and therefore works through model-internal APIs to read `ConstructEntry`/`ConstructKind`; external backends cannot name these types and must treat constructs opaquely. This is the intended A30 consequence.
- **A31 (delta `functions` is NOT exhaustive for pre-existing functions):** `DeltaBatch.functions`/`constructs` are the view of entities **added** by the batch with final folded bounds, minus entities removed by the same batch (`src/delta.rs` lines 230–269 and `reconstruct_function_entries`, lines 332–402). Updates to pre-existing functions ride the ops (`SetCell`/`SetConstraintBounds`/`RemoveConstraint`). The P26 compiler **MUST** consume the ops for updates and the semantic entries for added entities — never treat `functions` as exhaustive for pre-existing constraints. Task 7 includes an explicit A31-aware delta-consumption test.

### B6 — Compiled synchronization contract amendment — **CONFIRMED**

Design §22 amends the advanced backend synchronization contract: the current `Synchronization { DeltaBatch(DeltaBatch), Rebuild(ModelSnapshot) }` (`src/solver/session.rs` lines 30–35) carries only canonical state and cannot cleanly pass semantic compilation through. The amended variants are **pinned** in this acceptance record (per the plan Task 7 recommendation): `CompiledRebuild(BackendSnapshot)` and `CompiledDeltaBatch(BackendDeltaBatch)`.

**Finding F-B6 (INFO):** amending public `Synchronization` variants is a source-level API change for advanced backend authors. **Disposition: accepted** — design §22 explicitly sanctions the amendment; D27 protects only the ORDINARY M2 API (`Model`, `LinExpr`, `Highs::solve`, `solve_with`, `Solution`), which remains source-compatible. Backend authors receive the migration guide (`docs/migration/M3_BACKEND_IR.md`, SM-03.8), and `ReferenceBackend` migrates first (Task 7) so the conformance/recovery surface stays green during migration.

### Findings summary

| ID | Severity | Finding | Disposition |
|----|----------|---------|-------------|
| B1 | WARNING | `CompiledObjectivePolicy` cannot represent no-active-objective (real M2 behavior) | **Amended** — A32 adds `CompiledObjectivePolicy::None` |
| F-B1 | INFO | `BackendOp` enumeration omits var-type/activity/semi-continuous/parameter ops | Accepted — rebuild-on-uncertainty (design §18) |
| F-B6 | INFO | Amending public `Synchronization` variants | Accepted — sanctioned by design §22; migration guide + ReferenceBackend-first |
| F-C | WARNING | Committed `M3_P25_public_api_roml_final.txt` is stale vs merged `main@9c2a9df` (F1 intermediate shape) | Accepted — P26 diff computed against this P26 base capture |
| F-D | INFO | `cargo package -p roml-highs --locked` fails (unpublished `roml` path dep) | Accepted — pre-existing packaging limitation, recorded skip |
| F-E | INFO | design §8.3 `BackendSnapshot`/`BackendDeltaBatch` omit `recipe_fingerprint`; packet/plan add it | Accepted — resolved-by (packet is authoritative; plan Task 5 lists the field) |
| F-G | INFO | packet `BackendConstraint` payloads (`Indicator`/`Sos1`/`Sos2`/`PiecewiseLinear`) not defined in P26 | Accepted — deferred-to P32/P33 bridge tasks; empty `#[non_exhaustive]` boundary now, `native_constraints` always empty in P26 (mirrors A30 pattern) |

No BLOCKER (P0/P1) findings. The acceptance record is therefore **PASSED**; B1's required amendment (A32) is recorded in `DECISIONS.md` and committed with this task, satisfying the plan's precondition that any design/decisions change is recorded before Task 5.

## Deviations and decisions

- No production source file was modified by Task 0 (review + baseline only).
- **A32** recorded in `.planning/milestones/M3-semantic-modeling-workflows/DECISIONS.md` (new no-active-objective representation for `CompiledObjectivePolicy`), per the plan Task 0 step 3 and the amendment protocol.
- Baseline captures (public API, package lists) are recorded as summaries here with raw artifacts kept at the capture time; the phase-end public-API diff uses this P26 base capture as its baseline (see F-C).

## Reviewer findings

Task 0 is itself the review: the backend-contract acceptance record above is the reviewer's pass over design §8, the packet Tasks 5–7 contract, and the P25-implemented canonical state, with code verification against `src/snapshot.rs`, `src/delta.rs`, `src/function/mod.rs`, `src/construct/mod.rs`, `src/model/changelog.rs`, `src/solver/reference.rs`, `src/solver/session.rs`, and the P25 evidence. All findings and dispositions are in the table above; no finding required source changes.

## Residual risks

- `cargo package -p roml-highs --locked` cannot be verified until `roml` is published; recorded as a pre-existing skip (F-D), unchanged from M2/P25 convention.
- The P26 public-API diff baseline is this P26 base capture (12019 items), not the stale P25 final file (F-C); the P26 head capture at phase end will be diffed against it.
- Construct preference-honoring (A29) and `BackendConstraint` payloads (F-G) are exercised only from P32/P33; P26 declares the boundaries and the compiler's read surface without landing formulations.
- `CompiledObjectivePolicy::None` (A32) must be accepted by `ReferenceBackend`/HiGHS compiled paths in Task 7 without regressing the M2 objective-less solve behavior (`objectiveless_rebuild`).

## Gate result

Task 0 (backend-contract amendment review gate): **PASSED** — B1–B6 dispositions recorded, A29–A31 compiler-facing consequences addressed, A32 amendment recorded before Task 5, untouched baseline captured at `main@9c2a9df`, no source code modified. Implementation tasks may begin.

---

# Task 6 — Implement typed capabilities

**Plan:** `26-PLAN.md` — Task 6 (implement typed capabilities)
**Requirements:** SM-04.1, SM-04.2, SM-04.3, SM-04.4, SM-04.5 (foundation), SM-03.4
**Status:** Complete — committed as `feat(backend): add typed feature capabilities`.
**Branch (executor worktree):** `worktree-agent-a6f5ef61b9775b791` (base `main@9c2a9df254321792ef0e3cae275a662c798572c9`)

## Scope

Task 6 replaces the flat `BackendCapabilities` Boolean record with a typed `BackendCapabilitySet` keyed by `BackendFeature` (D10, SM-04.1), migrates request validation to the typed set (SM-04.4), builds a version-aware HiGHS typed capability set from pinned `highs-sys` facts (SM-04.2, SM-04.3), and removes the transitional flat→typed conversion before the P26 merge. `CompilationPolicy { Auto, Portable, NativeRequired }` is co-located in `src/compiler/capability.rs` per the packet's "Capabilities and compilation" grouping (SM-03.4).

## TDD record

1. **Characterization (untouched tree — passes before the migration):**
   - `tests/typed_capabilities.rs` — `characterize_legacy_all_flat_capabilities`, `characterize_validate_request_rejects_mip_when_flat_mip_false`, `characterize_validate_request_accepts_mip_when_flat_mip_true` (flat API only). **3 passed** on the untouched tree.
   - `roml-highs/tests/conformance.rs` — `characterize_highs_flat_capabilities` (HiGHS flat `capabilities()` values). **1 passed** on the untouched tree.
   - `src/solver/backend.rs` — `characterize_legacy_flat_mapping_onto_typed_features` pins the flat→typed feature correspondence (`lp`/`mip` → `Lp`/`Mip`; `add_variable`/`add_constraint`/`set_coefficient`/`set_bounds` → `IncrementalRows`/`IncrementalCoefficients`/`IncrementalBounds`; flat-only fields preserved flat-only).
2. **RED failures (recorded before implementation):**
   - `tests/typed_capabilities.rs`: `error[E0432] unresolved imports roml::compiler::capability::{BackendCapabilitySet, BackendFeature, FeatureLimitations, FeatureSupport, SupportLevel}`.
   - `src/solver/request.rs` + `src/compiler/capability.rs` `#[cfg(test)]`: same unresolved imports / missing types (`error[E0422]`, `error[E0425]`, `error[E0433]`).
3. **GREEN:** all typed tests pass after implementation (see verification matrix).

## Implementation

- **`src/compiler/capability.rs` (create):** `#[non_exhaustive] pub enum BackendFeature` (all 17 interface-contract variants), `SupportLevel { Unsupported, Native }` (default `Unsupported`), `FeatureLimitations { minimum_version, model_classes, maximum_count, notes }`, `FeatureSupport { level, limitations }` (+ `native`/`unsupported`/`is_native`), `BackendCapabilitySet` keyed by `BackendFeature` (`supports`, `support`, `set`, `native_features`, `unsupported_features`, `len`, `is_empty`), `CompilationPolicy { Auto, Portable, NativeRequired }`.
- **`src/solver/request.rs`:** `validate_request` migrated to `&BackendCapabilitySet`; MIP options gate on `capabilities.supports(BackendFeature::Mip)`. Unsupported features are rejected, never silently ignored (SM-04.4).
- **`src/solver/backend.rs`:** module doc documents the typed migration; characterization test pins the flat→typed feature correspondence. `BackendCapabilities` retained unchanged for M2 source compatibility (D27).
- **`roml-highs/src/session.rs`:** `pub fn highs_capability_set(major, minor, patch) -> BackendCapabilitySet` — the M2-native surface (`Lp`, `Mip`, `IncrementalBounds`, `IncrementalRows`, `IncrementalCoefficients`) declared `Native` with `minimum_version` = runtime version; every unqualified M3 feature (`MipStart`, `PartialMipStart`, `MultipleMipStarts`, `VariableHints`, `InitialBasis`, `Iis`, `FeasibilityRelaxation`, `Indicator`, `Sos1`, `Sos2`, `NativePiecewiseLinear`, `NativeMultiObjective`) declared `Unsupported`. `BackendMetadata::capabilities()` returns the flat compat view derived from the typed set (typed-mappable fields from the set; flat-only facts preserved).
- **`roml-highs/src/lib.rs`:** re-exports `highs_capability_set` so the version-aware declaration is publicly inspectable.
- **Module wiring:** `src/lib.rs` gains `pub mod compiler;` and `src/compiler/mod.rs` is created with the single `pub mod capability;` line (Task 6's contribution; Task 5's `backend_ir`/`origin`/`report` declarations are merged by the orchestrator per the plan's ordered-line convention).
- **Tests:** new `tests/typed_capabilities.rs` (7 tests), HiGHS `session.rs` unit tests (2), HiGHS `conformance.rs` (2), `src/solver/backend.rs` (1). Existing `tests/status_negotiation_tests.rs` and `tests/advanced_surface.rs` `validate_request` callers migrated to the typed set.

## Transitional flat→typed conversion

**Removed before merge.** No `From<&BackendCapabilities> for BackendCapabilitySet` / `from_flat` helper exists in the committed tree (verified by grep). The typed set is the sole authority for request validation and HiGHS capability declarations; `BackendCapabilities` remains only as the D27 source-compatible compat output view.

## Verification matrix (Task 6)

| Command | Exit | Result |
|---|---|---|
| `cargo test -p roml-highs --test conformance` | 0 | **4 passed** |
| `cargo test -p roml-highs --all-targets` | 0 | **104 passed; 0 failed** |
| `cargo test -p roml --all-targets` | 0 | **613 passed; 0 failed; 2 ignored** |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `cargo fmt --all -- --check` | 0 | formatting clean |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 | docs generated, no warnings |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` | 0 | docs generated, no warnings |

Baseline comparison: `roml` grew from 600 to 613 passing tests (+13: 5 capability unit tests + 7 typed-capability integration tests + 1 backend characterization test); `roml-highs` grew from 100 to 104 (+2 session unit tests + 2 conformance tests). No existing test weakened or deleted.

## Acceptance criteria

- `src/compiler/capability.rs` defines `#[non_exhaustive] pub enum BackendFeature` with the 17 interface-contract variants; `SupportLevel { Native, Unsupported }`; `FeatureLimitations`; `FeatureSupport`; `BackendCapabilitySet` keyed by `BackendFeature`; `CompilationPolicy { Auto, Portable, NativeRequired }` — **met** (SM-04.1, SM-03.4).
- `validate_request` validates against the typed set; unsupported features rejected, never silently ignored — **met** (SM-04.4).
- HiGHS `BackendMetadata::capabilities()` reports version-aware support with unqualified M3 features `Unsupported` — **met** (SM-04.2, SM-04.3).
- The transitional flat→typed conversion helper does not exist at merge — **met** (grep-confirmed).
- M2 source compatibility preserved (D27): `BackendCapabilities` retained; both `--all-targets` suites green — **met**.

## Deviations

1. **`validate_request` signature change required migrating existing test callers** (`tests/status_negotiation_tests.rs`, `tests/advanced_surface.rs`) — these are not in the plan's Task 6 file list, but the public signature change (`&BackendCapabilities` → `&BackendCapabilitySet`) mandated by the plan's "migrate request validation to the typed set" requires updating them for the `cargo test -p roml --all-targets` gate to pass. Documented here; minimal mechanical updates.
2. **`roml-highs/src/lib.rs` re-export** (`pub use session::highs_capability_set;`) — not in the plan's file list, added so the version-aware typed HiGHS declaration is publicly inspectable (M3 official-backend-evidence requirement).
3. **`src/lib.rs` `pub mod compiler;` and `src/compiler/mod.rs`** — wired by Task 6 so the capability module compiles and tests run in this worktree; the orchestrator resolves the shared `mod.rs` line and dedupes the identical `pub mod compiler;` line with Task 5 during merge.
4. **`src/solver/conformance.rs`** — listed in the plan as a migration target, but it touches no capability code (verified); no change required.

## Commit trail

- `feat(backend): add typed feature capabilities` — Task 6 implementation + tests + this evidence section.

<!-- Phase-level gate result (P26 boundary) filled after Task 7 review passes. -->
