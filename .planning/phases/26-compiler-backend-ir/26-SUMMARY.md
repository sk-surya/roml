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
