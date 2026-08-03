---
phase: 26-compiler-backend-ir
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/compiler/mod.rs
  - src/compiler/backend_ir.rs
  - src/compiler/capability.rs
  - src/compiler/origin.rs
  - src/compiler/report.rs
  - src/compiler/session.rs
  - src/solver/backend.rs
  - src/solver/request.rs
  - src/solver/conformance.rs
  - src/solver/session.rs
  - src/solver/facade.rs
  - src/solver/reference.rs
  - src/advanced.rs
  - src/lib.rs
  - roml-highs/src/compiler.rs
  - roml-highs/src/session.rs
  - tests/compiler_identity.rs
  - tests/differential_harness.rs
  - docs/migration/M3_BACKEND_IR.md
  - docs/release/evidence/M3_P26_COMPILER_BACKEND_IR.md
autonomous: false
requirements:
  - SM-02.4
  - SM-02.5
  - SM-02.6
  - SM-03.1
  - SM-03.2
  - SM-03.3
  - SM-03.4
  - SM-03.5
  - SM-03.6
  - SM-03.7
  - SM-03.8
  - SM-03.9
  - SM-04.1
  - SM-04.2
  - SM-04.3
  - SM-04.4
  - SM-04.5
  - SM-13
must_haves:
  truths:
    - M2 solve/recovery passes through backend IR
    - Primitive random deltas equal rebuild
    - Divergent clones with equal revision cannot share exact compiled artifacts
    - Hashes are never accepted as exact authority
    - All generated entities have origins
    - Backends consume no mutable model internals
  artifacts:
    - src/compiler/{mod,backend_ir,capability,origin,report,session}.rs
    - roml-highs/src/compiler.rs
    - tests/compiler_identity.rs
    - docs/migration/M3_BACKEND_IR.md
    - docs/release/evidence/M3_P26_COMPILER_BACKEND_IR.md
  key_links:
    - Exact CompilationId authority chain (snapshot, delta from/to, origins, results) — never a fingerprint
    - Origin completeness — no compiled entity without EntityOrigin
    - A31 delta contract — updates ride ops, not the functions view
    - Capability gating — CompilationSession consults BackendCapabilitySet; unqualified features reject/rebuild
---

# Phase 26 — Compiler Boundary, Backend IR, Capabilities, Origins, and Exact Compilation Identity

> **For agentic workers:** this phase is the M3 compiler boundary. Execute Task 0 (backend-contract amendment review gate) first and alone; it produces the acceptance record that gates every implementation task. Tasks 5 and 6 are content-independent and MAY be dispatched in parallel (see "Waves and parallelization"); Task 7 is strictly serial after both. Follow the TDD protocol from `EXECUTION.md` for every task: write a focused failing test, record the expected failure, implement the smallest correct behavior, run focused then phase tests, commit one coherent unit, update evidence and traceability. Do NOT run `roml-mosek`/`roml-xpress` — they are known-broken against the current facade and out of scope (M2 convention). Stop after Task 7 and request independent review before marking the phase done.

**Goal:** insert deterministic semantic compilation without regressing primitive incremental behavior.

**Requirements:** SM-02.4, SM-02.5, SM-02.6 (foundations), SM-03 (all clauses), SM-04 (all clauses), SM-13 (compiler foundations).

## Requirements

- **SM-02.4** — compiler-generated entities use distinct compiled IDs (`CompiledVariableId`/`CompiledConstraintId`/`CompiledObjectiveId`), never user handles. Closed by Task 5.
- **SM-02.5** — every generated entity maps to a user entity, construct, or overlay role via the mandatory `OriginMap`. Closed by Tasks 5 and 7.
- **SM-02.6** — diagnostics distinguish bound provenance. Per `TRACEABILITY.md`, SM-02.6 closes in P29; P26 delivers the foundation here: compiled bound representation plus an origin map that can already distinguish generated/user-origin bound sources, which the P29 conflict mapping builds on. The evidence file must state this clause-level scope explicitly (do not claim full closure).
- **SM-03.1–SM-03.9** — capability-aware compiler over canonical snapshots/deltas; backends consume backend IR; IR supports linear rows plus the normalized-primitive extension surface; `CompilationPolicy` semantics; compilation artifacts (origin map, report, recipe fingerprint, generated inventory, opaque `CompilationId`); recipe change forces rebuild; primitive incremental equals compiled rebuild; backend-contract migration documented and tested; exact `CompilationId` on all results with fingerprints never authoritative. Closed by Tasks 5, 6, 7.
- **SM-04.1–SM-04.5** — typed `BackendFeature` registry with native/bridge support reported separately, version-aware limitations, unsupported-feature rejection, and per-solve feature recording foundation. Closed by Task 6 (SM-04.5 full per-solve recording lands with `EffectiveSolvePlan` in P28; the typed registry and rejection machinery that make it possible are P26).
- **SM-13 (compiler foundations)** — per `TRACEABILITY.md`, P26 closes only the compiler foundations: the `CompileError` family and the compilation-report infrastructure (error identification, recipe evidence) that P33's deterministic interval analysis and Big-M rules will build on. Full SM-13.1–SM-13.6 close in P33.

## Files

Create:

- `docs/release/evidence/M3_P26_COMPILER_BACKEND_IR.md` — phase evidence file; created empty (with the baseline and backend-contract acceptance sections) before implementation per `EXECUTION.md`, appended as work proceeds.
- `src/compiler/mod.rs` — compiler module root; declares the P26 submodules; `CompileError` family.
- `src/compiler/backend_ir.rs` — `CompilationId`, `RecipeFingerprint`, compiled IDs, `CompiledVariable`/`CompiledLinearRow`/`CompiledObjective`, `CompiledObjectivePolicy`, `BackendConstraint` (extension surface), `BackendSnapshot`, `BackendDeltaBatch`, `BackendOp`, and the backend-IR builder.
- `src/compiler/origin.rs` — `EntityOrigin`, `GeneratedRole`, `OverlayId`, `OriginMap` with bidirectional queries and a completeness validator.
- `src/compiler/report.rs` — `CompilationReport` and `BackendIdentity`.
- `src/compiler/capability.rs` — `BackendFeature`, `SupportLevel`, `FeatureLimitations`, `FeatureSupport`, `BackendCapabilitySet`, `CompilationPolicy`.
- `src/compiler/session.rs` — `CompilationSession` (identity compiler: `compile_snapshot`/`compile_delta`).
- `roml-highs/src/compiler.rs` — HiGHS-side translation of backend IR into native calls.
- `tests/compiler_identity.rs` — compiler identity, origin completeness, and fingerprint-authority tests.
- `docs/migration/M3_BACKEND_IR.md` — backend-author migration guide (SM-03.8).

Modify:

- `src/lib.rs` — wire `pub mod compiler;`.
- `src/advanced.rs` — re-export the compiler surface for backend/framework authors (never the ordinary prelude).
- `src/solver/backend.rs`, `src/solver/request.rs`, `src/solver/conformance.rs` — migrate capability handling to the typed set.
- `src/solver/session.rs` — amend `Synchronization` to carry backend IR (design §22).
- `src/solver/facade.rs` — route M2 solve/recovery through compiled IR; preserve compile-before-mutation and one-rebuild-retry.
- `src/solver/reference.rs` — `ReferenceBackend` consumes `BackendSnapshot`/`BackendDeltaBatch` (migrated first).
- `roml-highs/src/session.rs` — version-aware capability set (Task 6) and compiled synchronization path (Task 7).
- `tests/differential_harness.rs` — fixed-seed compiled-delta versus compiled-rebuild equality.

## Task 0 — Backend-contract amendment review (phase gate)

**Phase:** P26  **Requirements:** SM-03.2 (foundation), SM-03.8 (foundation), SM-02.4 (foundation)

This is the STATE.md blocking gate for P26. It is a review pass over the design §8 backend contract and the P25 amendments' implications **before any implementation**. It produces an explicit acceptance record in the phase evidence; no implementation task may start until the record is written and its blockers resolved.

**Read first:**
- `docs/superpowers/specs/2026-08-02-semantic-modeling-and-solve-workflows-design.md` §4 "Identity model", §8 "Compiler and bridge system", §13, §18 "Incremental semantics", §19 "Failure semantics", §22 "Compatibility and migration".
- `.planning/milestones/M3-semantic-modeling-workflows/DECISIONS.md` — D2, D5, D9, D10, D11, D22, D28, and amendments A29–A31 (the exact contract P26 must honor).
- `docs/superpowers/plans/2026-08-02-semantic-modeling-and-solve-workflows.md` — "Backend IR and origins" and "Capabilities and compilation" interface contracts; Task 7's `BackendOp` enumeration note ("the full enumeration is review-gated with the implementation plan (Task 7)").
- `.planning/milestones/M3-semantic-modeling-workflows/EXECUTION.md` — § "Baseline command matrix" and § "Review gates".
- `docs/release/evidence/M3_P25_SEMANTIC_IR.md` — F1/F2/F4 findings and the canonical state P26 compiles from.
- `src/snapshot.rs`, `src/delta.rs`, `src/function/mod.rs` — the canonical snapshot/delta/semantic-entry contract the compiler consumes.

**TDD order** (this task produces no production code — the "test" is the acceptance record and the baseline capture; it does not modify source):

1. Record the P26 baseline in the evidence file: `git rev-parse HEAD` (branch `phase-roml-P26-compiler-backend-ir` from `main@9c2a9df`), Rust/Cargo versions, target/OS; run the untouched `roml` and `roml-highs` baseline matrices from `EXECUTION.md`; capture `cargo public-api -p roml`, `cargo public-api -p roml-highs`, and `cargo package --list` for both crates.
2. Review each backend-contract point below and write its disposition (Confirmed / Amended / Rejected) into the evidence section `## Backend contract amendment acceptance`:
   - **B1 — `BackendSnapshot` compiled objective policy.** Confirm `objective_policy: CompiledObjectivePolicy` (design §8.4). Resolution point: a snapshot with no active objective (M2 reference-backend solves objective-less models — see `objectiveless_rebuild` in `src/solver/reference.rs`). Record how the identity compiler represents the no-active-objective case without regressing M2 behavior; if `CompiledObjectivePolicy` needs a new representation, record a DECISIONS.md amendment before Task 7.
   - **B2 — `BackendDeltaBatch` exact from/to compilation IDs.** Confirm every batch carries exact `from_compilation`/`to_compilation` plus `from_revision`/`to_revision`, and that the compiler allocates a fresh `CompilationId` per target state.
   - **B3 — Full `BackendOp` enumeration.** Review the packet interface-contract enumeration (including `RemoveLinearCoefficient`, `RemoveObjectiveCoefficient`, `SetObjectivePolicy`) and pin it in the acceptance record (design §8.3: the enumeration is review-gated with Task 7).
   - **B4 — `CompilationId`/`RecipeFingerprint` authority rules.** Confirm D28/design §4.3: `RecipeFingerprint` is deterministic evidence/cache aid only, never stale-state authority; exact `CompilationId` is the comparison key.
   - **B5 — A29–A31 implications for what the compiler reads from canonical snapshots/deltas.** A29: `ConstructEntry { id, kind, active, preference }` is the single per-construct authority — the compiler honors `FormulationPreference` (`Auto`/`Portable`/`NativeRequired`) and can narrow the global `CompilationPolicy` per construct without weakening exactness. A30: the construct module is crate-private; only `Construct` and `FormulationPreference` are public; `ModelSnapshot.constructs` is `pub #[doc(hidden)]`. A31: `DeltaBatch.functions`/`constructs` are the view of entities ADDED with final folded bounds, minus removed; updates to pre-existing functions ride the ops (`SetCell`/`SetConstraintBounds`/`RemoveConstraint`) — the P26 compiler MUST consume ops for updates and the semantic entries for added entities, never treating `functions` as exhaustive for pre-existing constraints.
   - **B6 — Compiled synchronization contract amendment.** Design §22 amends the advanced backend synchronization contract: how `Synchronization` in `src/solver/session.rs` carries `BackendSnapshot`/`BackendDeltaBatch`, and how the M2 ordinary `Highs::solve`/`SolverSession` path flows through compiled IR while preserving source compatibility (D27).
3. If any point requires a design/decisions change, record the amendment in `DECISIONS.md` per the amendment protocol BEFORE starting Task 5.
4. Commit the acceptance record + baseline as one unit.

- [ ] Record base SHA, tool versions, untouched matrices, public API/package captures in the evidence file.
- [ ] Write a disposition (Confirmed/Amended/Rejected) for each of B1–B6 in `## Backend contract amendment acceptance`.
- [ ] Record any required DECISIONS.md amendment before Task 5.
- [ ] Stop when every review point has a disposition and no P0/P1 blocker remains; the acceptance record gates all implementation.
- [ ] Commit as `docs(m3): record P26 backend contract amendment acceptance`.

**Stopping condition:** the evidence file records the baseline and the backend-contract acceptance record with a disposition for every review point (B1–B6), any required amendments are recorded in `DECISIONS.md`, and no implementation task has begun.

**Commit:** `docs(m3): record P26 backend contract amendment acceptance`

**Verification:**

```bash
test -s docs/release/evidence/M3_P26_COMPILER_BACKEND_IR.md \
  && grep -q "## Backend contract amendment acceptance" docs/release/evidence/M3_P26_COMPILER_BACKEND_IR.md
```

**Acceptance criteria:**
- `docs/release/evidence/M3_P26_COMPILER_BACKEND_IR.md` exists and contains the `## Backend contract amendment acceptance` section with an explicit disposition for each of B1–B6.
- Each of A29–A31's compiler-facing consequences (preference honored; crate-private constructs respected; delta `functions` not treated as exhaustive) is explicitly addressed in the record.
- The untouched baseline matrices and public API/package captures for `roml` and `roml-highs` are recorded (base `main@9c2a9df`).
- Any required `DECISIONS.md` amendment is committed before Task 5 begins.
- No production source file is modified by this task.

## Task 5 — Define backend IR and exact compilation identity

**Phase:** P26  **Requirements:** SM-02.4, SM-02.5, SM-03.3 (extension surface), SM-03.5, SM-03.6 (fingerprint + identity foundation), SM-03.9, SM-13 (compiler foundations)

**Read first:**
- `docs/superpowers/specs/2026-08-02-semantic-modeling-and-solve-workflows-design.md` §4 "Identity model", §5 "Metadata and provenance", §8.3 "Backend snapshot", §8.4 "Objective policy in backend IR", §8.5 "Bridge contract".
- `docs/superpowers/plans/2026-08-02-semantic-modeling-and-solve-workflows.md` — "Identity" and "Backend IR and origins" interface contracts (authoritative shapes; `BackendSnapshot` includes `recipe_fingerprint`).
- `src/identity.rs` — the checked atomic-counter opaque-id pattern (zero reserved, typed overflow) that `CompilationId` must mirror.
- `src/snapshot.rs`, `src/delta.rs` — the input types (`ModelSnapshot`, `DeltaBatch`, `ModelRevision`, `ModelInstanceId`) the backend IR projects from.
- `src/advanced.rs`, `src/lib.rs` — the surfaces that will carry the compiler exports.

**TDD order** (per `EXECUTION.md`):

1. Write failing tests in `tests/compiler_identity.rs`:
   - Compiled IDs are dense and deterministic; `CompilationId` is unique and checked (zero reserved; overflow is a typed error, never a wrap).
   - Builder finalization rejects any generated entity without an origin: building a `BackendSnapshot` with a `CompiledVariable`/`CompiledLinearRow`/`CompiledObjective` that has no `OriginMap` entry returns a typed `CompileError`.
   - `BackendSnapshot` stores `compilation_id`, `source_instance`, `source_revision`, and `objective_policy: CompiledObjectivePolicy`.
   - Every `BackendDeltaBatch` requires and carries exact `from_compilation`/`to_compilation` and `from_revision`/`to_revision`.
   - `OriginMap` supports bidirectional queries (compiled → origin and origin → compiled) and a completeness validator flags any unoriginated compiled entity.
   - Recipe fingerprints are deterministic (equal compiled states → equal fingerprint) but never authority: a test asserts stale-state comparisons use `CompilationId`, not the fingerprint (D28).
   - `CompilationReport` records the recipe fingerprint, a generated-entity inventory, and formulation decisions.
2. Run the tests and record the expected failures (missing types).
3. Implement:
   - `src/compiler/backend_ir.rs`: `pub struct CompilationId(u64);`, `pub struct RecipeFingerprint([u8; 32]);` (opaque, checked atomic allocation mirroring P25 `src/identity.rs`); `CompiledVariableId(pub u32)`, `CompiledConstraintId(pub u32)`, `CompiledObjectiveId(pub u32)`; `CompiledVariable { id, bounds, var_type, name }`; `CompiledLinearRow { id, bounds, coefficients: Vec<(CompiledVariableId, f64)>, name }`; `CompiledObjective { id, sense, coefficients, constant, name }`; `CompiledObjectivePolicy { Single(CompiledObjectiveId), Weighted(Vec<CompiledWeightedObjective>), Lexicographic(Vec<CompiledObjectiveLevel>) }`; `CompiledWeightedObjective`, `CompiledObjectiveLevel`; `#[non_exhaustive] pub enum BackendConstraint {}` declared as the normalized-native-primitive extension surface (indicator/SOS1/SOS2/PWL payloads land with the P32/P33 bridge tasks, mirroring the P25 `ConstructKind`/A30 pattern — `native_constraints: Vec<BackendConstraint>` is always empty in P26); `BackendSnapshot { compilation_id, source_instance, source_revision, variables, linear_rows, native_constraints, objectives, objective_policy, origin_map, report, recipe_fingerprint }`; `BackendDeltaBatch { from_compilation, to_compilation, from_revision, to_revision, operations, recipe_fingerprint }`; `#[non_exhaustive] pub enum BackendOp` with the full packet enumeration including `RemoveLinearCoefficient { constraint, variable }`, `RemoveObjectiveCoefficient { objective, variable }`, `SetObjectivePolicy(CompiledObjectivePolicy)`; a builder whose finalization rejects any generated entity without a recorded origin.
   - `src/compiler/origin.rs`: `EntityOrigin { UserVariable(Variable), UserConstraint(Constraint), UserObjective(Objective), Construct { construct: Construct, role: GeneratedRole }, SolveOverlay { overlay: OverlayId, role: GeneratedRole } }`; `GeneratedRole` (`#[non_exhaustive]` role marker; roles refined with the bridge tasks); `OverlayId(u64)` (opaque overlay identity, design §4.4); `OriginMap` with bidirectional queries and a completeness validator.
   - `src/compiler/report.rs`: `CompilationReport` (recipe fingerprint, generated-entity inventory, formulation decisions); `BackendIdentity` (backend name/version pair for report provenance — packet gloss, consumed by P29).
   - `src/compiler/mod.rs`: module wiring plus the `CompileError` family (design §19): the P26 variants needed now (missing-origin rejection; stale-compilation and unsupported-feature rejections used by Task 7). P32 adds `UnboundedBigM` etc.
   - Wire `pub mod compiler;` in `src/lib.rs`; re-export the compiler surface through `src/advanced.rs` (compiler internals are NOT added to the ordinary prelude — SM-03.x/API-07.2).
4. Run `cargo test -p roml --test compiler_identity` (must pass), then `cargo test -p roml --all-targets` (must pass), `cargo clippy -p roml --all-targets -- -D warnings` (must pass), and `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` (must pass).
5. Update evidence and traceability.
6. Commit one coherent unit.

- [ ] Write builder-finalization tests that reject any generated entity without origin.
- [ ] Allocate dense deterministic compiled IDs and a unique checked `CompilationId` per compiled state.
- [ ] Store source instance/revision and objective policy in `BackendSnapshot`.
- [ ] Define backend deltas with exact from/to compilation IDs.
- [ ] Implement bidirectional origin queries and structured compilation reports.
- [ ] Implement deterministic recipe fingerprinting solely for evidence/cache use.
- [ ] Stop when no generated entity can be finalized without a recorded origin.
- [ ] Commit as `feat(compiler): define backend IR and compilation identity`.

**Stopping condition (packet, verbatim):** no generated entity can be finalized without a recorded origin.

**Commit:** `feat(compiler): define backend IR and compilation identity`

**Verification:**

```bash
cargo test -p roml --test compiler_identity
cargo test -p roml --all-targets
cargo clippy -p roml --all-targets -- -D warnings
```

**Acceptance criteria:**
- All three commands exit 0.
- `src/compiler/backend_ir.rs` defines `pub struct CompilationId(u64)` and `pub struct RecipeFingerprint([u8; 32])` (opaque; checked atomic allocation, zero reserved).
- `src/compiler/backend_ir.rs` defines `pub struct BackendSnapshot { pub compilation_id: CompilationId, pub source_instance: ModelInstanceId, pub source_revision: ModelRevision, pub variables: Vec<CompiledVariable>, pub linear_rows: Vec<CompiledLinearRow>, pub native_constraints: Vec<BackendConstraint>, pub objectives: Vec<CompiledObjective>, pub objective_policy: CompiledObjectivePolicy, pub origin_map: OriginMap, pub report: CompilationReport, pub recipe_fingerprint: RecipeFingerprint }`.
- `src/compiler/backend_ir.rs` defines `pub struct BackendDeltaBatch { pub from_compilation: CompilationId, pub to_compilation: CompilationId, pub from_revision: ModelRevision, pub to_revision: ModelRevision, pub operations: Vec<BackendOp>, pub recipe_fingerprint: RecipeFingerprint }` — every `BackendDeltaBatch` carries exact from/to compilation IDs.
- `BackendOp` is `#[non_exhaustive]` and includes `RemoveLinearCoefficient { constraint, variable }`, `RemoveObjectiveCoefficient { objective, variable }`, and `SetObjectivePolicy(CompiledObjectivePolicy)`.
- `src/compiler/origin.rs` defines `EntityOrigin`, `GeneratedRole`, `OverlayId(u64)`, and `OriginMap` with bidirectional queries and a completeness validator (SM-02.5, D5).
- `src/compiler/report.rs` defines `CompilationReport` (recipe fingerprint, generated inventory, formulation decisions) and `BackendIdentity`.
- The compiler surface is exported through `src/advanced.rs`, not the prelude.

## Task 6 — Implement typed capabilities

**Phase:** P26  **Requirements:** SM-04.1, SM-04.2, SM-04.3, SM-04.4, SM-04.5 (foundation), SM-03.4

**Read first:**
- `docs/superpowers/specs/2026-08-02-semantic-modeling-and-solve-workflows-design.md` §8.1 "Compilation policy", §8.2 "Backend capability model".
- `docs/superpowers/plans/2026-08-02-semantic-modeling-and-solve-workflows.md` — "Capabilities and compilation" interface contract (`BackendFeature`, `SupportLevel`, `FeatureLimitations`, `FeatureSupport`, `CompilationPolicy`).
- `src/solver/backend.rs` — the current flat `BackendCapabilities` struct and `all()`.
- `src/solver/request.rs` — `validate_request` (the capability-check migration target).
- `src/solver/conformance.rs` — the shared sync suite that must keep passing through the migration.
- `roml-highs/src/session.rs` — `BackendMetadata::capabilities()` (~lines 324–343) and `negotiate_options`.
- `docs/release/evidence/M3_P25_SEMANTIC_IR.md` — the public-API diff; `BackendCapabilities` is a public type re-exported from `advanced.rs`/`lib.rs`, so D27 (M2 surface source-compatible) constrains the migration.

**TDD order** (per `EXECUTION.md`):

1. **Characterize every legacy capability mapping before replacement.** Write a characterization test asserting the current `BackendCapabilities::all()` and HiGHS `capabilities()` values map onto the typed `BackendFeature` set as expected, and that `validate_request`'s current rejections correspond to typed-feature checks. Run it and record the result (this is characterization on the current tree — it passes before the migration and pins the intended mapping).
2. Write failing tests (in `src/compiler/capability.rs` `#[cfg(test)]` and `src/solver/request.rs`):
   - `BackendCapabilitySet::supports(BackendFeature::Lp)` and friends.
   - `validate_request` rejects MIP options against a set lacking `BackendFeature::Mip`.
   - `FeatureSupport { level, limitations }` carries `minimum_version`, `model_classes`, `maximum_count`, and `notes` (SM-04.3).
   - The HiGHS capability set declares the M2-native features `Native` and every unqualified M3 feature `Unsupported` (`MipStart`, `PartialMipStart`, `MultipleMipStarts`, `VariableHints`, `InitialBasis`, `Iis`, `FeasibilityRelaxation`, `Indicator`, `Sos1`, `Sos2`, `NativePiecewiseLinear`, `NativeMultiObjective`).
3. Run the tests and record the expected failures.
4. Implement:
   - `src/compiler/capability.rs`: `#[non_exhaustive] pub enum BackendFeature` (all 17 variants from the interface contract); `SupportLevel { Native, Unsupported }`; `FeatureLimitations { minimum_version: Option<String>, model_classes: Vec<String>, maximum_count: Option<usize>, notes: Vec<String> }`; `FeatureSupport { level: SupportLevel, limitations: FeatureLimitations }`; `BackendCapabilitySet` keyed by `BackendFeature` (native support and ROML bridge support reported separately per SM-04.2); `CompilationPolicy { Auto, Portable, NativeRequired }` co-located per the packet's "Capabilities and compilation" grouping.
   - Migrate `validate_request` in `src/solver/request.rs` to validate against the typed set; migrate `src/solver/conformance.rs` wherever it touches capabilities.
   - Build the version-aware HiGHS capability set in `roml-highs/src/session.rs` from pinned `highs-sys` facts; keep `BackendMetadata::capabilities()` source-compatible (D27) by returning a `BackendCapabilities` compat view derived from the typed set.
   - **Remove the transitional flat→typed conversion helper before the P26 merge** (packet verbatim).
5. Run `cargo test -p roml-highs --test conformance` (the conformance fix), then `cargo test -p roml-highs --all-targets`, `cargo test -p roml --all-targets`, and clippy for both crates.
6. Update evidence and traceability.
7. Commit one coherent unit.

- [ ] Characterize every legacy capability mapping before replacement.
- [ ] Implement `BackendCapabilitySet` keyed by `BackendFeature`.
- [ ] Migrate request validation and conformance tests.
- [ ] Build version-aware HiGHS capability sets; unqualified M3 features remain unsupported.
- [ ] Remove the transitional flat conversion before P26 merge.
- [ ] Commit as `feat(backend): add typed feature capabilities`.

**Stopping condition:** every legacy capability mapping is characterized, the typed `BackendCapabilitySet` is authoritative for request validation and HiGHS capability declarations, the transitional flat conversion is removed before merge, and the conformance test passes.

**Commit:** `feat(backend): add typed feature capabilities`

**Verification:**

```bash
cargo test -p roml-highs --test conformance
cargo test -p roml-highs --all-targets
cargo test -p roml --all-targets
cargo clippy -p roml --all-targets -- -D warnings
cargo clippy -p roml-highs --all-targets -- -D warnings
```

**Acceptance criteria:**
- All five commands exit 0.
- `src/compiler/capability.rs` defines `#[non_exhaustive] pub enum BackendFeature` with the 17 interface-contract variants; `SupportLevel { Native, Unsupported }`; `FeatureLimitations`; `FeatureSupport`; `BackendCapabilitySet` keyed by `BackendFeature`; `CompilationPolicy { Auto, Portable, NativeRequired }` (SM-04.1, SM-03.4).
- `validate_request` validates against the typed set; unsupported features are rejected, never silently ignored (SM-04.4).
- HiGHS `BackendMetadata::capabilities()` reports version-aware support with unqualified M3 features `Unsupported` (SM-04.2, SM-04.3).
- The transitional flat→typed conversion helper does not exist at merge (removed before P26 merge).
- M2 source compatibility preserved (D27): `cargo test -p roml --all-targets` and `cargo test -p roml-highs --all-targets` still green.

## Task 7 — Add identity compiler and migrate synchronization

**Phase:** P26  **Requirements:** SM-02.4, SM-02.5, SM-02.6 (foundation), SM-03.1, SM-03.2, SM-03.4, SM-03.5, SM-03.6, SM-03.7, SM-03.8, SM-03.9, SM-13 (compiler foundations)

**Read first:**
- `docs/superpowers/specs/2026-08-02-semantic-modeling-and-solve-workflows-design.md` §8 "Compiler and bridge system", §18 "Incremental semantics", §19 "Failure semantics", §22 "Compatibility and migration".
- `docs/superpowers/plans/2026-08-02-semantic-modeling-and-solve-workflows.md` — Task 7 (verbatim translation target).
- Task 5 artifacts (`src/compiler/backend_ir.rs`, `origin.rs`, `report.rs`), Task 6 artifact (`src/compiler/capability.rs`).
- `src/solver/session.rs` — `Synchronization` (the amended contract), `src/solver/facade.rs` — `SolverSession`, `src/solver/reference.rs` — `ReferenceBackend::apply_op`/`rebuild`, `src/solver/conformance.rs`, `tests/differential_harness.rs` — the commuting-square harness to extend, `roml-highs/src/session.rs` + `roml-highs/src/projection.rs`.
- `docs/release/evidence/M3_P25_SEMANTIC_IR.md` — F2/A31 delta `functions` narrowed contract (the compiler consumes ops for updates, semantic entries for added entities).

**TDD order** (per `EXECUTION.md`):

1. Write failing tests first:
   - **Identity compile:** a primitive linear `ModelSnapshot` compiles one-to-one into a `BackendSnapshot` — each variable → one `CompiledVariable`, each constraint → one `CompiledLinearRow` with dense deterministic compiled IDs, each objective → one `CompiledObjective`, and the active objective → `CompiledObjectivePolicy::Single(compiled id)`.
   - **Compiled delta:** a `DeltaBatch` compiles into a `BackendDeltaBatch` with exact `from_compilation`/`to_compilation` and `from_revision`/`to_revision`; `SetCell` → `SetLinearCoefficient`/`SetObjectiveCoefficient`, `RemoveCell` → `RemoveLinearCoefficient`/`RemoveObjectiveCoefficient`, `SetConstraintBounds` → `SetLinearRowBounds`, `RemoveConstraint` → `RemoveLinearRow`, objective changes → the objective `BackendOp` variants, active-objective changes → `SetObjectivePolicy`.
   - **Rebuild on uncertainty:** a delta containing a semantic construct op (or any op the identity compiler cannot prove incrementally equivalent) forces a deterministic rebuild — no compiled delta is emitted (design §18, D22).
   - **A31-aware delta consumption:** updates to pre-existing functions ride the ops; the compiled delta derives its row/objective coefficients from the `SetCell`/`RemoveCell` ops, never from treating `DeltaBatch.functions` as exhaustive for pre-existing constraints.
   - **Fixed-seed compiled-delta equals compiled-rebuild:** random primitive op sequences applied incrementally to a compiled backend produce the same normalized state as one compiled rebuild (`tests/differential_harness.rs` extension).
   - **ReferenceBackend recovery:** after migration, the existing recovery/differential tests pass on the compiled path.
   - **HiGHS migration:** `roml-highs` consumes `BackendSnapshot`/`BackendDeltaBatch`; a `ModelSnapshot` is never passed to the HiGHS session after migration (source-level assertion).
   - **Compile-before-mutation and one-rebuild-retry:** the facade compiles the full canonical state before any backend mutation, and on failure performs one deterministic rebuild retry.
2. Run the tests and record the expected failures.
3. Implement:
   - `src/compiler/session.rs`: `CompilationSession` with `compile_snapshot(&ModelSnapshot, &CompilationPolicy, &BackendCapabilitySet) -> Result<BackendSnapshot, CompileError>` and `compile_delta(&DeltaBatch, from_compilation: CompilationId, policy: &CompilationPolicy, capabilities: &BackendCapabilitySet) -> Result<BackendDeltaBatch, CompileError>`; one-to-one identity mapping for primitive linear state; active objective compiled into `CompiledObjectivePolicy`; rebuild-on-uncertainty (per the Task 0 acceptance record B1–B6, including the no-active-objective disposition); A31-aware delta consumption.
   - Amend `Synchronization` in `src/solver/session.rs` to carry backend IR (design §22). Recommended variant names, to be pinned by the Task 0 acceptance record B6: `CompiledRebuild(BackendSnapshot)` and `CompiledDeltaBatch(BackendDeltaBatch)`.
   - Migrate `src/solver/reference.rs` (`ReferenceBackend`) FIRST to consume compiled IR; run recovery + differential tests.
   - Create `roml-highs/src/compiler.rs` (backend IR → HiGHS native translation) and migrate `roml-highs/src/session.rs` `synchronize()` to the compiled path. After migration the HiGHS session receives no canonical `ModelSnapshot`.
   - Migrate `src/solver/facade.rs` (`SolverSession`/`Highs` path) so M2 solve/recovery flows through compiled IR; preserve compile-before-mutation and one-rebuild-retry invariants.
   - Update `src/advanced.rs` exports for the compiler surface.
   - Extend `tests/differential_harness.rs` with the fixed-seed compiled-delta versus rebuild equality.
   - Write `docs/migration/M3_BACKEND_IR.md` (SM-03.8 backend-author migration guide).
4. Run the phase verification matrix (below), update evidence, commit, and request architecture review at the phase boundary.

- [ ] Compile primitive linear snapshots one-to-one, including active compiled objective policy.
- [ ] Compile primitive deltas with exact from/to compilation IDs; use rebuild on uncertainty.
- [ ] Add explicit coefficient-removal and objective-policy backend operations.
- [ ] Migrate ReferenceBackend first and run recovery/differential tests.
- [ ] Migrate HiGHS; it receives no canonical `ModelSnapshot` afterward.
- [ ] Preserve compile-before-mutation and one-rebuild-retry invariants.
- [ ] Add fixed-seed compiled-delta versus rebuild equality.
- [ ] Commit as `feat(sync): compile canonical state into backend IR` and request architecture review.

**Stopping condition (packet, verbatim):** ReferenceBackend migrates first and its recovery/differential tests pass; HiGHS migrates and receives no canonical `ModelSnapshot` afterward; compile-before-mutation and one-rebuild-retry hold; fixed-seed compiled-delta equals rebuild.

**Commit:** `feat(sync): compile canonical state into backend IR` (then request architecture review).

**Verification:**

```bash
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
cargo clippy -p roml --all-targets -- -D warnings
cargo clippy -p roml-highs --all-targets -- -D warnings
```

**Acceptance criteria:**
- All four commands exit 0 (including `tests/m3_baseline_characterization.rs` — primitive incremental behavior not regressed, SM-03.7 / the P26 gate).
- `src/compiler/session.rs` defines `CompilationSession` with snapshot and delta compilation; primitive linear snapshots compile one-to-one including the active `CompiledObjectivePolicy` (SM-03.1, SM-03.5).
- Every `BackendDeltaBatch` emitted by the compiler has exact `from_compilation`/`to_compilation`; any uncertainty selects rebuild (SM-03.6, SM-03.7).
- `roml-highs/src/compiler.rs` translates backend IR into HiGHS native state; the HiGHS session synchronizes through the compiled path — source assertion: no `ModelSnapshot` reaches the HiGHS session after migration (SM-03.2).
- `src/solver/reference.rs` consumes compiled IR and its recovery/differential tests pass (SM-03.8).
- `tests/differential_harness.rs` proves fixed-seed compiled-delta equals compiled-rebuild (SM-03.7).
- M2 solve/recovery passes through backend IR (the P26 gate) with `Highs::solve`/`SolverSession` source-compatible (D27).
- `docs/migration/M3_BACKEND_IR.md` documents the backend-contract migration for backend authors (SM-03.8).

## Verification

Phase-level checks (all must exit 0):

```bash
cargo fmt --all -- --check
cargo test -p roml --test compiler_identity
cargo test -p roml-highs --test conformance
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
cargo clippy -p roml --all-targets -- -D warnings
cargo clippy -p roml-highs --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps
cargo public-api -p roml
```

Baseline matrix (untouched tree, recorded in Task 0 evidence; `roml-mosek`/`roml-xpress` are out of scope — never use workspace-wide commands):

```bash
cargo fmt --all -- --check
cargo check -p roml --all-targets
cargo check -p roml-highs --all-targets
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
cargo package --list -p roml
cargo package --list -p roml-highs
```

Per P26 mandatory checks in `EXECUTION.md`: compiler determinism; origin completeness; ReferenceBackend/HiGHS conformance; randomized compiled delta versus rebuild; recovery/failure tests. Public API/package qualification:

```bash
cargo public-api -p roml
cargo public-api -p roml-highs
cargo package -p roml --locked
cargo package -p roml-highs --locked
```

Skips must be recorded in the evidence file, never treated as passing.

## Waves and parallelization

The four tasks map to three execution waves:

- **Wave 1 — Task 0 (backend-contract amendment review gate).** Runs first and alone: its acceptance record (B1–B6) gates all implementation and it touches no source file.
- **Wave 2 — Task 5 and Task 6 (parallelizable).**
- **Wave 3 — Task 7 (strictly serial).**

**Can Tasks 5 and 6 run in parallel? Yes, with one ordered-line convention.**

- *Disjoint file ownership.* Task 5 owns `src/compiler/{mod,backend_ir,origin,report}.rs`, `tests/compiler_identity.rs`, `src/advanced.rs`, `src/lib.rs`. Task 6 owns `src/compiler/capability.rs`, `src/solver/{backend,request,conformance}.rs`, `roml-highs/src/session.rs`. The two share exactly one file: `src/compiler/mod.rs` (Task 5 creates the module tree; Task 6 appends the single line `pub mod capability;`).
- *Content independence.* Task 5 introduces `CompilationId`/`BackendSnapshot`/`BackendDeltaBatch`/`BackendOp`/`OriginMap`; Task 6 introduces `BackendFeature`/`BackendCapabilitySet`/`CompilationPolicy`. Neither needs the other's types, so the two can be authored, implemented, and committed independently.
- *Merge-conflict minimization.* The single shared line is resolved by **ordered application**: Task 5 creates `src/compiler/mod.rs` declaring `backend_ir`, `origin`, `report`; Task 6's `pub mod capability;` is an additive append applied after Task 5's commit lands. If `gsd-execute-phase` dispatches the two plans to separate agents, the executor must serialize Task 6's `mod.rs` edit to follow Task 5's commit — never two agents editing `mod.rs` simultaneously. With ordered application there are zero conflicting text edits; with a single executor running the tasks in order there is zero conflict by construction.

**Why Task 7 is Wave 3 (never parallel with 5 or 6).** Task 7 depends on BOTH Task 5 (uses `BackendSnapshot`/`BackendDeltaBatch`/`BackendOp`/`CompilationReport`) and Task 6 (uses `BackendCapabilitySet`/`BackendFeature`/`CompilationPolicy`). It also shares files with both: it modifies `src/solver/conformance.rs` and `roml-highs/src/session.rs` (also touched by Task 6) and `src/advanced.rs` plus `src/compiler/mod.rs` (also touched by Task 5). Running Task 7 in parallel with either Task 5 or Task 6 would produce overlapping edits on the same files. **Do NOT run Task 7 in parallel with Task 5 or Task 6.**

**Recommended structure for `gsd-execute-phase`:** Wave 2 = Task 5 and Task 6 dispatched in parallel with the ordered-`mod.rs` convention; Wave 3 = Task 7. If the executor prefers serial execution, the order Task 0 → Task 5 → Task 6 → Task 7 is equally correct and fully conflict-free (matching the P25 serial precedent and `D26 — One active implementation phase by default`).

## Review gates

Per `EXECUTION.md` § "Review gates", P26 receives two independent review passes at the phase boundary (after Task 7).

- **Pass 1 — Specification and correctness:** requirement coverage (SM-02.4–02.6 foundations, SM-03.1–03.9, SM-04.1–04.5, SM-13 compiler foundations); semantic correctness of the compiled IR and identity rules; invariant preservation (exact `CompilationId` authority, `RecipeFingerprint` non-authority, origin completeness, rebuild-on-uncertainty); unsupported/error behavior (typed `CompileError`, typed capability rejections); origin completeness (no generated entity without `EntityOrigin`); API coherence (compiler surface via `advanced`, not the prelude); official backend evidence (HiGHS capability set from pinned `highs-sys`); test quality.
- **Pass 2 — Integration and operations:** incremental/rebuild behavior (compiled delta versus rebuild equality); failure recovery (ReferenceBackend recovery/differential tests); cross-platform/version behavior (HiGHS capability declarations); public API diff (`cargo public-api -p roml`); package/docs impact (`docs/migration/M3_BACKEND_IR.md`); performance evidence (the P34 benchmark fixture is untouched by P26 — no regression to the primitive incremental path); migration accuracy (backend-author guide).

**Blocking rules:**

- P0/P1 findings **block merge**.
- P2 findings may merge only when explicitly accepted and scheduled.
- `autonomous: false` — the executor pauses after Task 7 and does not declare the phase complete until both review passes resolve to no P0/P1 findings.
- The backend-contract acceptance record (Task 0) is itself review input: the reviewer verifies B1–B6 dispositions are consistent with the design and with the implemented IR.

Evidence requirement: `docs/release/evidence/M3_P26_COMPILER_BACKEND_IR.md` must record the baseline, the backend-contract acceptance record, per-task verification with RED failures, the full verification matrix, the public API diff, and reviewer findings and dispositions before the gate result is marked pass (per `EXECUTION.md` § "Evidence file structure").

## Artifacts this phase produces

New modules and symbols (all names/signatures from the approved design and the packet's interface contract):

- `src/compiler/backend_ir.rs` — `CompilationId(u64)`, `RecipeFingerprint([u8; 32])`, `CompiledVariableId(pub u32)`, `CompiledConstraintId(pub u32)`, `CompiledObjectiveId(pub u32)`, `CompiledVariable`, `CompiledLinearRow`, `CompiledObjective`, `CompiledObjectivePolicy { Single, Weighted, Lexicographic }`, `CompiledWeightedObjective`, `CompiledObjectiveLevel`, `#[non_exhaustive] BackendConstraint` (extension surface), `BackendSnapshot`, `BackendDeltaBatch` (exact from/to compilation IDs), `#[non_exhaustive] BackendOp` (including `RemoveLinearCoefficient`, `RemoveObjectiveCoefficient`, `SetObjectivePolicy`).
- `src/compiler/origin.rs` — `EntityOrigin { UserVariable, UserConstraint, UserObjective, Construct { construct, role }, SolveOverlay { overlay, role } }`, `GeneratedRole`, `OverlayId(u64)`, `OriginMap` (bidirectional queries + completeness validator).
- `src/compiler/report.rs` — `CompilationReport` (recipe fingerprint, generated inventory, formulation decisions), `BackendIdentity` (name/version pair).
- `src/compiler/capability.rs` — `#[non_exhaustive] BackendFeature` (17 variants), `SupportLevel`, `FeatureLimitations`, `FeatureSupport`, `BackendCapabilitySet`, `CompilationPolicy { Auto, Portable, NativeRequired }`.
- `src/compiler/session.rs` — `CompilationSession` (`compile_snapshot`, `compile_delta`); identity compilation and rebuild-on-uncertainty.
- `src/compiler/mod.rs` — module wiring; `CompileError` family.
- `roml-highs/src/compiler.rs` — backend IR → HiGHS native translation.
- Modified: `src/solver/backend.rs`, `src/solver/request.rs`, `src/solver/conformance.rs` (typed capability migration); `src/solver/session.rs` (amended `Synchronization` carrying backend IR); `src/solver/facade.rs` (M2 path through compiled IR); `src/solver/reference.rs` (compiled IR); `roml-highs/src/session.rs` (version-aware capabilities + compiled sync); `src/advanced.rs`, `src/lib.rs` (compiler surface exports).
- Test files: `tests/compiler_identity.rs`; `tests/differential_harness.rs` (fixed-seed compiled-delta vs rebuild).
- Docs/evidence: `docs/migration/M3_BACKEND_IR.md`; `docs/release/evidence/M3_P26_COMPILER_BACKEND_IR.md`.

## must_haves

Goal-backward verification (from the ROADMAP P26 gate, verbatim):

**Truths (observable behaviors):**

1. **M2 solve/recovery passes through backend IR.** The ordinary `Highs::solve`/`SolverSession` path compiles canonical state to `BackendSnapshot`/`BackendDeltaBatch`; no backend parses mutable `Model` internals; the M2 characterization suite stays green.
2. **Primitive random deltas equal rebuild.** Fixed-seed compiled-delta application on a compiled backend produces the same normalized state as one compiled rebuild (the commuting square on backend IR).
3. **Divergent clones with equal revision cannot share exact compiled artifacts.** Two clones preserving lineage but with distinct `ModelInstanceId` (D28) allocate distinct `CompilationId`s; stale-state checks compare exact `CompilationId`, never `ModelRevision`/fingerprint.
4. **Hashes are never accepted as exact authority.** `RecipeFingerprint` is deterministic evidence/cache support only; no result, overlay, or analysis mapping is authorized by a fingerprint.
5. **All generated entities have origins.** Builder finalization rejects any compiled entity without an `EntityOrigin`; `OriginMap` completeness holds.
6. **Backends consume no mutable model internals.** ReferenceBackend and HiGHS consume backend IR only; HiGHS receives no `ModelSnapshot` after migration.

**Artifacts (files that must exist):**

- `src/compiler/{mod,backend_ir,capability,origin,report,session}.rs`
- `roml-highs/src/compiler.rs`
- `tests/compiler_identity.rs`, `tests/differential_harness.rs`
- `docs/migration/M3_BACKEND_IR.md`
- `docs/release/evidence/M3_P26_COMPILER_BACKEND_IR.md`

**Key links (critical connections where breakage cascades):**

- **`CompilationId` authority chain** — `BackendSnapshot.compilation_id`, `BackendDeltaBatch.from_compilation`/`to_compilation`, origin maps, and (from P28 onward) solution metadata must agree; divergent clones with equal revision must never share a `CompilationId` (D28).
- **Origin completeness** — a compiled entity without an `EntityOrigin` must be unfinalizable; a missing origin at finalization is a typed `CompileError` (D5, SM-02.5).
- **Fingerprint non-authority** — `RecipeFingerprint` equality must never be used as a stale-state gate; only exact `CompilationId` may (SM-03.9).
- **A31 delta contract** — the compiler derives row/objective updates from the `SetCell`/`RemoveCell`/`SetConstraintBounds` ops, not from `DeltaBatch.functions` treated as exhaustive for pre-existing constraints.
- **Capability gating** — `CompilationSession` consults `BackendCapabilitySet`; unqualified features are rejected or rebuild, never silently ignored (SM-04.4).

## Threat model

This is a modeling library; P26 introduces no network, filesystem, auth, or untrusted-input surface. The relevant trust boundaries are integrity/invariant boundaries:

| Boundary | Description | Mitigation in this phase |
|----------|-------------|--------------------------|
| canonical model state → compiler | compiler reads immutable snapshot/delta only | `CompilationSession` consumes `ModelSnapshot`/`DeltaBatch`; no mutable `Model` access (SM-03.2); A31-narrowed delta contract honored |
| compiler → backend IR | generated state must be complete and traceable | builder finalization rejects unoriginated entities; `OriginMap` completeness validator (D5) |
| backend IR → HiGHS native (via `highs-sys`) | no panic/UB across FFI; every native return code checked | `roml-highs/src/compiler.rs` routes through existing checked `highs-sys` call pattern (`check_highs_status`); unchanged from M2 binding policy |
| stale-state authority | hashes/fingerprints must never authorize correctness | exact `CompilationId` is the only stale-state key; `RecipeFingerprint` is evidence/cache only (D28) |

No new `unsafe`, environment mutation, filesystem scan, or stdout output is introduced by this phase.

## Gate

P26 passes when:

- Task 0's backend-contract acceptance record is written (B1–B6 dispositions), any required DECISIONS.md amendment is recorded, and the untouched baseline is captured in `M3_P26_COMPILER_BACKEND_IR.md`;
- Task 5 defines backend IR and exact compilation identity with origin-complete finalization (SM-02.4, SM-02.5, SM-03.3 extension surface, SM-03.5, SM-03.6, SM-03.9, SM-13 foundations);
- Task 6 implements typed version-aware capabilities with the transitional flat conversion removed (SM-04.1–04.5 foundations, SM-03.4);
- Task 7 adds the identity compiler and migrates ReferenceBackend then HiGHS to backend IR (SM-03.1–03.8), preserving compile-before-mutation and one-rebuild-retry;
- all phase-level verification commands exit 0 (including `cargo test -p roml --test compiler_identity`, `cargo test -p roml-highs --test conformance`, both `--all-targets` suites, both clippy lanes, rustdoc with warnings denied, and `cargo public-api -p roml`);
- the P26 gate holds: M2 solve/recovery passes through backend IR; primitive random deltas equal rebuild; divergent clones with equal revision cannot share exact compiled artifacts; hashes are never accepted as exact authority; all generated entities have origins; backends consume no mutable model internals;
- SM-02.6 is closed only at its clause level (foundation delivered here; full closure in P29) — the evidence file states this explicitly;
- the P26 evidence bundle, backend-contract acceptance record, public API diff, and migration guide are recorded; and
- both independent review passes resolve with no P0/P1 findings.

No crate publication, tag, or release is part of this phase (SM-15.8 / M3 stopping condition).

## Output

Create `.planning/phases/26-compiler-backend-ir/26-SUMMARY.md` when done, per the phase completion protocol.
