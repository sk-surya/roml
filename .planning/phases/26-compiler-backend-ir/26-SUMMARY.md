---
phase: 26-compiler-backend-ir
plan: 01
subsystem: compiler
tags: [backend-ir, compilation-identity, origins, fingerprints, compile-error]
dependency_graph:
  requires: [Task 0 backend-contract acceptance record (B1-B6, A32)]
  provides: [src/compiler/{mod,backend_ir,origin,report}.rs, tests/compiler_identity.rs]
  affects: [Task 6 (capability), Task 7 (identity compiler, synchronization), P32/P33 bridges, P29 reports]
tech-stack:
  added: []
  patterns:
    - checked atomic opaque-id allocation mirroring src/identity.rs (zero reserved, typed overflow)
    - deterministic non-cryptographic recipe digest (four FNV-1a passes) for evidence/cache fingerprints
    - builder finalization enforcing origin completeness (D5)
key-files:
  created:
    - src/compiler/mod.rs
    - src/compiler/backend_ir.rs
    - src/compiler/origin.rs
    - src/compiler/report.rs
    - tests/compiler_identity.rs
  modified:
    - src/lib.rs
    - src/advanced.rs
    - docs/release/evidence/M3_P26_COMPILER_BACKEND_IR.md
decisions:
  - CompiledObjectivePolicy::None (A32) is the compiled representation of the M2 no-active-objective case
  - RecipeFingerprint is a deterministic FNV-1a-4 digest over the compiled recipe (evidence/cache only, never authority)
  - CompilationId::allocate stays pub(crate); the builder is the canonical allocation path; overflow covered by a dedicated test-only family
  - UnsupportedFeature carries a feature-name string in P26 (typed BackendFeature registry lands in Task 6)
metrics:
  duration: ~40 min
  completed: 2026-08-03
  tasks: 1 (Task 5 of 4-task phase)
  commits: 1 (plus this summary commit)
status: complete
actuals:
  tokens: 18743   # chars/4 over the realized Task 5 diff (74,972 bytes)
  tasks: 1
  commits: 2
---

# Phase [26] Task [5]: Define backend IR and exact compilation identity

Backend IR and exact compilation identity: `BackendSnapshot` (with compiled objective policy per A32 — including the `None` case), `BackendDeltaBatch` with exact from/to `CompilationId` + from/to `ModelRevision`, the pinned 15-variant `#[non_exhaustive]` `BackendOp` (incl. `RemoveLinearCoefficient`/`RemoveObjectiveCoefficient`/`SetObjectivePolicy`), dense deterministic compiled IDs, unique checked `CompilationId` per compiled state, bidirectional origin queries, structured `CompilationReport`, deterministic `RecipeFingerprint` (evidence/cache only — never authority), and builder finalization that rejects any generated entity without an origin.

## What was built

- **`src/compiler/backend_ir.rs`** — the backend IR shapes from the packet interface contract field-for-field plus A32:
  - `CompilationId(u64)` — opaque, checked atomic allocation (zero reserved, typed overflow, never wraps), mirroring `src/identity.rs`.
  - `RecipeFingerprint([u8; 32])` — deterministic digest of the compiled recipe (four FNV-1a passes packed into 32 bytes); evidence/cache only, never stale-state authority (D28).
  - `CompiledVariableId`/`CompiledConstraintId`/`CompiledObjectiveId` (`pub u32` dense newtypes, SM-02.4).
  - `CompiledVariable`/`CompiledLinearRow`/`CompiledObjective` and `CompiledObjectivePolicy { None, Single, Weighted, Lexicographic }` (A32 adds `None`).
  - Empty `#[non_exhaustive] BackendConstraint` extension surface (F-G: `native_constraints` always empty in P26).
  - `BackendSnapshot` (exact packet shape) and `BackendDeltaBatch` (exact from/to compilation ids + revisions, B2).
  - `#[non_exhaustive] BackendOp` — the B3-pinned 15-variant enumeration including `RemoveLinearCoefficient`, `RemoveObjectiveCoefficient`, `SetObjectivePolicy`.
  - `BackendSnapshotBuilder` — finalization allocates a fresh `CompilationId`, rejects any unoriginated entity (`CompileError::MissingOrigin`), rejects dangling policy objectives (`CompileError::InvalidObjectivePolicy`), and computes the fingerprint + report.
- **`src/compiler/origin.rs`** — `EntityOrigin` (`UserVariable`/`UserConstraint`/`UserObjective`/`Construct`/`SolveOverlay`), empty `#[non_exhaustive] GeneratedRole` marker, `OverlayId(u64)` (opaque), `OriginMap` with bidirectional queries and a completeness validator (D5, SM-02.5).
- **`src/compiler/report.rs`** — `CompilationReport` (recipe fingerprint, generated-entity inventory, formulation decisions) and `BackendIdentity` (name/version pair).
- **`src/compiler/mod.rs`** — module wiring and the `CompileError` family (design §19): `MissingOrigin`, `StaleCompilation`, `UnsupportedFeature`, `InvalidObjectivePolicy`, `IdentityOverflow`.
- **`src/lib.rs`** — `pub mod compiler;`; **`src/advanced.rs`** — compiler surface re-exported through `advanced`, never the ordinary prelude.
- **`tests/compiler_identity.rs`** — 14 integration tests (TDD: RED → recorded → green).

## Verification

| Command | Result |
|---|---|
| `cargo test -p roml --test compiler_identity` | 0 — 14 passed |
| `cargo test -p roml --all-targets` | 0 — 618 passed; 0 failed; 2 ignored (baseline 600 + 18 new) |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 — clean |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 — clean |
| `cargo fmt --all -- --check` | 0 — clean |

## Deviations from plan

None — the plan was executed exactly as written. Minor implementation notes (not deviations):

- The recipe-fingerprint digest is a deterministic FNV-1a-4 construction (the plan specified the `[u8; 32]` type and determinism; the exact hash construction is an implementation detail documented in `backend_ir.rs`).
- `CompilationId::allocate()` is `pub(crate)` (mirroring `src/identity.rs`); the overflow/zero-reserved coverage uses a dedicated test-only family so concurrent unit tests do not race the shared counter.
- `CompileError::UnsupportedFeature` carries a feature-name `String` in P26; the typed `BackendFeature` registry lands in Task 6 and Task 7 wires the typed rejection.

## Known Stubs

- `BackendConstraint` is an empty `#[non_exhaustive]` enum and `native_constraints` is always empty in P26 — intentional extension-surface declaration (evidence finding F-G; payloads land with P32/P33 bridge tasks, mirroring the P25 `ConstructKind`/A30 pattern).
- `GeneratedRole` is empty and `EntityOrigin::Construct`/`SolveOverlay` are unconstructible in P26 — intentional forward declarations refined by P27 overlays and P32/P33 bridges.
- `CompileError::StaleCompilation`/`UnsupportedFeature` are declared now but exercised by Task 7.

## Self-Check: PASSED

Created files exist; commit `cc743a8` exists in `git log`; all verification commands exit 0.
||||||| 9c2a9df
# Phase 26 — Task 6 Summary: Implement typed capabilities

**Plan:** `26-PLAN.md` — Task 6 (implement typed capabilities)
**Status:** complete
**Requirements:** SM-04.1, SM-04.2, SM-04.3, SM-04.4, SM-04.5 (foundation), SM-03.4
**Branch (executor worktree):** `worktree-agent-a6f5ef61b9775b791` (base `main@9c2a9df254321792ef0e3cae275a662c798572c9`)
**Commit:** `feat(backend): add typed feature capabilities`

## One-liner

Typed `BackendCapabilitySet` keyed by the 17-variant `BackendFeature` replaces the flat Boolean capability record as the authority for request validation and HiGHS capability declarations, with a version-aware HiGHS typed set and the transitional flat→typed conversion removed.

## What was built

- **`src/compiler/capability.rs`** — the typed capability registry (D10, SM-04):
  - `#[non_exhaustive] pub enum BackendFeature` with the 17 packet variants.
  - `SupportLevel { Unsupported, Native }` (default `Unsupported`).
  - `FeatureLimitations { minimum_version, model_classes, maximum_count, notes }` (SM-04.3).
  - `FeatureSupport { level, limitations }` with `native`/`unsupported`/`is_native` constructors.
  - `BackendCapabilitySet` keyed by `BackendFeature` (`supports`, `support`, `set`, `native_features`, `unsupported_features`, `len`, `is_empty`).
  - `CompilationPolicy { Auto, Portable, NativeRequired }` (SM-03.4, design §8.1).
- **`src/solver/request.rs`** — `validate_request` migrated to `&BackendCapabilitySet`; MIP options gate on `BackendFeature::Mip`; unsupported options rejected, never silently ignored (SM-04.4).
- **`src/solver/backend.rs`** — module doc records the typed migration; `BackendCapabilities` retained unchanged for M2 source compatibility (D27); characterization test pins the flat→typed feature correspondence.
- **`roml-highs/src/session.rs`** — `highs_capability_set(major, minor, patch)` builds the version-aware typed set from pinned `highs-sys` facts: M2-native surface (`Lp`, `Mip`, `IncrementalBounds`, `IncrementalRows`, `IncrementalCoefficients`) `Native` with `minimum_version` = runtime version; 12 unqualified M3 features `Unsupported`. `BackendMetadata::capabilities()` now returns a flat compat view derived from the typed set.
- **`roml-highs/src/lib.rs`** — re-exports `highs_capability_set`.
- **Module wiring** — `src/lib.rs` gains `pub mod compiler;`; `src/compiler/mod.rs` created with the single `pub mod capability;` line (orchestrator resolves the shared line with Task 5 on merge).

## Key files

| File | Change |
|------|--------|
| `src/compiler/capability.rs` | created — typed capability registry |
| `src/compiler/mod.rs` | created — `pub mod capability;` (Task 6 line only) |
| `src/solver/request.rs` | modified — `validate_request` typed migration |
| `src/solver/backend.rs` | modified — doc + characterization test |
| `roml-highs/src/session.rs` | modified — `highs_capability_set` + compat view |
| `roml-highs/src/lib.rs` | modified — re-export `highs_capability_set` |
| `tests/typed_capabilities.rs` | created — 7 typed capability + characterization tests |
| `roml-highs/tests/conformance.rs` | modified — HiGHS typed set + flat characterization |
| `tests/status_negotiation_tests.rs` | modified — `validate_request` callers migrated |
| `tests/advanced_surface.rs` | modified — `validate_request` caller migrated |
| `docs/release/evidence/M3_P26_COMPILER_BACKEND_IR.md` | modified — Task 6 evidence section |

## TDD record

- **Characterization (untouched tree, before migration):** flat `BackendCapabilities::all()` and HiGHS `capabilities()` values characterized and recorded passing (3 + 1 tests).
- **RED:** unresolved imports for `BackendCapabilitySet`/`BackendFeature`/`FeatureSupport` in `tests/typed_capabilities.rs`, `src/solver/request.rs`, and `src/compiler/capability.rs`.
- **GREEN:** all typed tests pass; full verification matrix green.

## Verification

| Command | Exit | Result |
|---|---|---|
| `cargo test -p roml-highs --test conformance` | 0 | 4 passed |
| `cargo test -p roml-highs --all-targets` | 0 | 104 passed |
| `cargo test -p roml --all-targets` | 0 | 613 passed; 2 ignored |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 | clean |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 | clean |
| `cargo fmt --all -- --check` | 0 | clean |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 | clean |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` | 0 | clean |

## Deviations

1. `validate_request`'s public signature change (`&BackendCapabilities` → `&BackendCapabilitySet`) required migrating existing test callers in `tests/status_negotiation_tests.rs` and `tests/advanced_surface.rs` (not in the plan's file list, but required for the `cargo test -p roml --all-targets` gate).
2. `roml-highs/src/lib.rs` re-export added (not in the plan's file list) so the version-aware HiGHS typed set is publicly inspectable.
3. `src/lib.rs` `pub mod compiler;` and `src/compiler/mod.rs` wired for local compilation; orchestrator resolves the shared `mod.rs` line with Task 5 on merge.
4. `src/solver/conformance.rs` required no change (touches no capability code).

## Acceptance criteria

All five Task 6 acceptance criteria met: 17-variant `BackendFeature`; `validate_request` against the typed set with unsupported rejected; HiGHS version-aware capabilities with M3 features `Unsupported`; transitional flat→typed conversion removed (grep-confirmed absent); M2 source compatibility preserved (D27) with both `--all-targets` suites green.

## Transitional conversion removal

No `From<&BackendCapabilities> for BackendCapabilitySet` / `from_flat` helper exists in the committed tree. The typed set is the sole authority for request validation and HiGHS capability declarations; `BackendCapabilities` remains only as the D27 compat output view.
