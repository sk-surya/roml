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

## Task 5 — Define backend IR and exact compilation identity

**Phase:** P26  **Requirements:** SM-02.4, SM-02.5, SM-03.3 (extension surface), SM-03.5, SM-03.6, SM-03.9, SM-13 (compiler foundations)
**Status:** complete

### TDD — RED failures (recorded before implementation)

`cargo test -p roml --test compiler_identity` failed to compile against the untouched tree — the `roml::compiler` module and the entire compiler surface did not exist. Expected failure, recorded verbatim:

```text
error[E0432]: unresolved import `roml::advanced::CompiledObjectiveId`
  --> tests/compiler_identity.rs:18:5
   |
18 |     CompiledObjectiveId, CompiledObjectiveLevel, CompiledObjectivePolicy, CompiledVariable,
   |     ^^^^^^^^^^^^^^^^^^^  ^^^^^^^^^^^^^^^^^^^^^^  ^^^^^^^^^^^^^^^^^^^^^^^  ^^^^^^^^^^^^^^^^
   |     no `CompiledObjectiveId` in `advanced`
...
error: could not compile `roml` (test "compiler_identity") due to 1 previous error
```

No production source existed; every new test failed at compile time with unresolved imports for the compiler surface (and, once the surface was wired, one assertion defect in the test harness was fixed — the snapshot-source-identity test was tightened to compare against the actual compiled-from `Model` instance).

### Implementation

- `src/compiler/mod.rs` — module wiring plus the `CompileError` family (design §19): `MissingOrigin`, `StaleCompilation`, `UnsupportedFeature`, `InvalidObjectivePolicy`, `IdentityOverflow`.
- `src/compiler/backend_ir.rs` — `CompilationId(u64)` (opaque, checked atomic allocation, zero reserved, typed overflow); `RecipeFingerprint([u8; 32])` (deterministic FNV-1a-4 digest, evidence/cache only, never authority — D28); dense `CompiledVariableId`/`CompiledConstraintId`/`CompiledObjectiveId` (`pub u32`); `CompiledVariable`/`CompiledLinearRow`/`CompiledObjective`; `CompiledObjectivePolicy { None, Single, Weighted, Lexicographic }` (A32 adds `None`); `CompiledWeightedObjective`/`CompiledObjectiveLevel`; empty `#[non_exhaustive] BackendConstraint` extension surface (F-G: `native_constraints` always empty in P26); `BackendSnapshot`; `BackendDeltaBatch` (exact from/to compilation ids + revisions); the pinned 15-variant `#[non_exhaustive] BackendOp` (including `RemoveLinearCoefficient`, `RemoveObjectiveCoefficient`, `SetObjectivePolicy`); and `BackendSnapshotBuilder` whose finalization rejects any generated entity without an origin and any objective policy with a dangling objective, allocates a fresh `CompilationId` per state, and computes the deterministic recipe fingerprint + structured report.
- `src/compiler/origin.rs` — `EntityOrigin` (`UserVariable`/`UserConstraint`/`UserObjective`/`Construct`/`SolveOverlay`), empty `#[non_exhaustive] GeneratedRole` marker, `OverlayId(u64)` (opaque, design §4.4), `OriginMap` with bidirectional queries (compiled → origin and origin → compiled) and a completeness validator.
- `src/compiler/report.rs` — `CompilationReport` (recipe fingerprint, generated-entity inventory, formulation decisions) and `BackendIdentity` (name/version pair).
- `src/lib.rs` — `pub mod compiler;`. `src/advanced.rs` — the compiler surface is re-exported through `advanced`, never the ordinary prelude (SM-03.x / API-07.2).
- `tests/compiler_identity.rs` — 14 integration tests covering the plan bullets.

### Focused verification

| Command | Result |
|---|---|
| `cargo test -p roml --test compiler_identity` | 0 — **14 passed; 0 failed** |
| `cargo test -p roml --lib` | 0 — all pass, including 4 in-crate `backend_ir` tests (overflow saturation, zero-reserved, fingerprint determinism) |

### Full verification

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | 0 (clean) |
| `cargo test -p roml --all-targets` | 0 — **618 passed; 0 failed; 2 ignored** (baseline 600 + 18 new tests) |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 (clean, warnings denied) |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 (docs generated, no warnings) |
| `cargo check -p roml-highs --all-targets` | 0 (roml public-surface change is additive; `roml-highs` still compiles) |

### Acceptance criteria

All Task 5 acceptance criteria met:

- `BackendSnapshot`/`BackendDeltaBatch` shapes match the packet interface contract field-for-field, including the packet's `recipe_fingerprint` field and **A32** (`CompiledObjectivePolicy::None` — B1 resolution).
- `BackendOp` is `#[non_exhaustive]` and includes `RemoveLinearCoefficient { constraint, variable }`, `RemoveObjectiveCoefficient { objective, variable }`, and `SetObjectivePolicy(CompiledObjectivePolicy)` (B3 pinned enumeration).
- `OriginMap` supports bidirectional queries and a completeness validator; builder finalization enforces the Task 5 stopping condition — **no generated entity can be finalized without a recorded origin** (D5, SM-02.5).
- `RecipeFingerprint` is deterministic (equal compiled states → equal fingerprint) but never stale-state authority; exact `CompilationId` is the comparison key (D28, SM-03.9) — asserted by `recipe_fingerprints_are_deterministic_but_never_authority`.
- The compiler surface is exported through `src/advanced.rs`, not the prelude.

### Commit trail

| # | SHA | Message |
|---|---|---|
| 1 | `cc743a8` | `feat(compiler): define backend IR and compilation identity` |

<!-- Phase-level gate result (P26 boundary) filled after Task 7 review passes. -->
