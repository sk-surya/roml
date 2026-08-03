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
