---
phase: 25-semantic-ir-foundation
plan: 01
subsystem: canonical-model-ir
status: complete
completed_date: 2026-08-02
tags: [rust, milp, identity, lineage, metadata, function-in-set, constructs, atomic-ids, snapshot, delta]

# Dependency graph
requires:
  - phase: 24-consumer-qualification
    provides: qualified M2 ordinary linear surface (Model, LinExpr, builder APIs, snapshot/delta, reference + HiGHS backends)
provides:
  - Opaque ModelLineageId / ModelInstanceId / ConstructId via checked atomic counters (zero reserved, typed overflow)
  - Entity metadata store (ModelSource, EntityMetadata, EntityRef) — canonical but non-solver-affecting
  - Manual Model Default/Clone (clone preserves lineage, allocates new instance)
  - Function-in-set canonical constraints (ScalarFunction, ScalarSet, FunctionConstraint, IntoScalarFunction)
  - Generation-safe construct arena with full lifecycle (add/clone/snapshot/activity/remove/rebuild)
  - Semantic function/set and construct entries in ModelSnapshot and DeltaBatch
  - SolveMetadata records model_lineage / model_instance / model_revision
affects:
  - P26-compiler-backend-ir (consumes canonical snapshot/delta semantic entries, ConstructId, EntityRef)
  - P27-fixing-locks-overlays, P28-solve-plan-warm-starts, P29-iis-conflict-reports
  - P30-soft-constraints, P32-common-constructs, P33-piecewise-linear-bounds (per-construct payloads land here)

actuals:
  tokens: 23062   # chars/4 over the realized code+evidence diff, excluding raw public-api/package dumps
  tasks: 4
  commits: 4

tech-stack:
  added: []
  patterns:
    - "Opaque atomic-counter identities (checked, zero reserved) for lineage/instance/construct"
    - "Derived semantic snapshot/delta entries reconstructed from the single authority (no second coefficient authority)"
    - "Generation-safe construct arena owning all construct lifecycle (no per-feature side maps)"
    - "Invariant-checked transitional legacy fields in snapshots/deltas"
    - "Manual Model Default/Clone to control identity semantics (clone preserves lineage, reallocates instance)"

key-files:
  created:
    - src/identity.rs
    - src/metadata.rs
    - src/function/mod.rs
    - src/function/scalar.rs
    - src/function/set.rs
    - src/construct/mod.rs
    - tests/m3_baseline_characterization.rs
    - tests/lineage_metadata.rs
    - tests/semantic_ir.rs
    - docs/release/evidence/M3_P25_SEMANTIC_IR.md
  modified:
    - src/model/mod.rs
    - src/model/changelog.rs
    - src/model/constraint.rs
    - src/solution/metadata.rs
    - src/snapshot.rs
    - src/delta.rs
    - src/lib.rs
    - src/advanced.rs
    - src/expr/linear.rs
    - src/solver/facade.rs
    - src/solver/reference.rs
    - src/solver/conformance.rs
    - roml-highs/src/projection.rs

key-decisions:
  - "Model::clone preserves lineage but allocates a new ModelInstanceId; Default allocates both fresh (D28)."
  - "ConstructKind is #[non_exhaustive]; P25 stores only the private Fixture payload and declares design §7's nine variants as the extension surface (per-construct modules land P30/P32/P33)."
  - "Snapshot/DeltaBatch functions and constructs are derived views reconstructed from the single authority, never stored second authorities (SM-01.1)."
  - "Construct ModelOp variants are explicit no-ops in ReferenceBackend/HiGHS (M3 v1 does not compile constructs; SM-01.6)."
  - "Metadata is canonical but non-solver-affecting: set_metadata never advances the revision."

patterns-established:
  - "Opaque ids allocated by checked per-family atomic counters with zero reserved; overflow is a typed error, never a wrap."
  - "Semantic entries (functions/constructs) are reconstructed deterministically at snapshot/delta build time and invariant-checked against the transitional legacy fields."
  - "One generation-safe construct arena owns all construct lifecycle; removal invalidates ids and stale ids are rejected with typed errors."

requirements-completed:
  - SM-01.1
  - SM-01.2
  - SM-01.3
  - SM-01.4
  - SM-01.5
  - SM-01.6
  - SM-02.1
  - SM-02.2
  - SM-02.3
  - SM-02.5
  - SM-02.7
  - SM-15.1
---

# Phase 25 Plan 01: Canonical Semantic IR, Identities, and Metadata

Lineage/instance/construct opaque identity, entity metadata, linear function-in-set constraints, and a generation-safe construct arena — established as canonical semantic state on top of the ordinary M2 linear surface, before any workflow/compiler phase.

## One-liner

Opaque lineage/instance/construct identities (checked atomic counters), entity metadata, linear function-in-set constraints, and a generation-safe construct arena, all carried through canonical snapshot/delta entries while keeping the coefficient index the single authority.

## Goal-backward verification (P25 gate, verbatim)

1. **Existing linear models remain observationally equivalent** — the untouched-tree characterization suite (`tests/m3_baseline_characterization.rs`) and the full `roml`/`roml-highs` suites stay green; the M2 `LinExpr`/builder path is unchanged.
2. **Independent models reject cross-lineage assignments** — independent models never share a lineage; lineage/instance ids are globally distinct (checked atomic counters).
3. **Clones share lineage but never instance identity** — manual `Model::Clone` preserves `lineage()`, allocates a new `instance()`.
4. **Every construct fixture survives clone/snapshot/delta/activity/remove/rebuild** — the construct lifecycle test section covers each transition; stale ids are rejected with typed `ConstructNotFound`.

## Commits

| Task | Commit | Message |
|---|---|---|
| 1 | `8ebbf8a` | `test(m3): capture semantic modeling baseline` |
| 2 | `217aa0c` | `feat(model): add lineage instance and metadata` |
| 3 | `c19c608` | `feat(model): add linear function-in-set semantics` |
| 4 | `79c3d9a` | `feat(model): add canonical construct lifecycle` |

Base SHA `7b124ad`; branch `phase-roml-P25-semantic-ir-foundation`. No Co-Authored-By trailers. No modifications to root `.planning/STATE.md` / `.planning/ROADMAP.md` (release-hardening milestone owns those; the orchestrator handles tracking writes).

## Verification

All phase-level commands exit 0:

| Command | Result |
|---|---|
| `cargo test -p roml --test m3_baseline_characterization -- --nocapture` | 6 passed |
| `cargo test -p roml --test lineage_metadata` | 5 passed |
| `cargo test -p roml --test semantic_ir` | 15 passed |
| `cargo test -p roml --all-targets` | **585 passed; 0 failed; 0 ignored** |
| `cargo test -p roml-highs --all-targets` | **100 passed; 0 failed; 0 ignored** |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` | 0 |
| `cargo public-api -p roml` | 0 |

Untouched-tree baseline (Task 1): `roml` 553 passed / `roml-highs` 100 passed; fmt/check/clippy/test/doc/package all exit 0, recorded in `docs/release/evidence/M3_P25_SEMANTIC_IR.md`.

## Requirements closed

SM-01.1 (function-in-set canonical, single authority), SM-01.2 (`#[non_exhaustive]` linear-only function/set), SM-01.3 (stable construct handle, activity, parameter deps, metadata), SM-01.4 (snapshot/delta carry semantic entries), SM-01.5 (M2 linear path preserved), SM-01.6 (no backend index/handle/Big-M/overlay in canonical state), SM-02.1 (opaque lineage; clones preserve), SM-02.2 foundation (lineage governs reuse), SM-02.3 (metadata per entity), SM-02.5 foundation (construct origins), SM-02.7 (distinct instance; clone reallocates), SM-15.1 (M2 golden-path source compatibility preserved).

## Evidence

`docs/release/evidence/M3_P25_SEMANTIC_IR.md` — baseline/environment, untouched matrices, public API capture + post-P25 diff (+1571 / −6, the six being derived→manual `Model::clone`/`default` with identical public signatures), focused verification per task with recorded RED failures, full verification, public API/package, deviations, residual risks.

## Deviations from Plan

### Auto-fixed issues (Rules 1–3)

**1. [Rule 3 — call sites] `SolveMetadata` struct literals** (Task 2)
- `src/solver/facade.rs` and `tests/status_mapping.rs` constructed `SolveMetadata` without the new lineage/instance fields; fixed with `..SolveMetadata::default()`.

**2. [Rule 3 — ripple] `ModelSnapshot`/`DeltaBatch` field additions** (Tasks 3, 4)
- Adding `functions`/`constructs` broke struct literals in `src/solver/conformance.rs` and ~34 roml-highs test snapshot fixtures; all updated with `functions: vec![]` / `constructs: vec![]`. No behavior changed.

**3. [Rule 3 — ripple] `ModelOp` construct variants** (Task 4)
- Added `AddConstruct`/`RemoveConstruct`/`SetConstructActive` broke the exhaustive matches in `src/solver/reference.rs` and `roml-highs/src/projection.rs`; added explicit no-op arms (SM-01.6).

### Design interpretations

- **`ConstructKind` P25 shape**: only the `Fixture(FixturePayload)` variant is present; design §7's nine payload variants are declared as the extension surface in rustdoc and land with P30/P32/P33 (P25 scope note: no formulation pre-implementation).
- **`LinExpr`/`Term`/`TermCoeff` gained `PartialEq`**: required by `ScalarFunction`'s `PartialEq` per design §6; additive, no behavior change.
- **Delta semantic entries are reconstructed views**: `DeltaBatch::new` derives `functions`/`constructs` deterministically from the ops; the coefficient index and construct arena remain the single authorities.

## Threat Flags

No security-relevant surface was introduced. The new public surface is additive (`identity`/`metadata`/`function`/`construct` modules + re-exports); no network endpoints, no file access, no `unsafe`, no auth paths. The construct `ModelOp` variants reach adapters only as no-ops. No threat model beyond the plan applies.

## Known Stubs

None. `ConstructKind` carries only the P25 `Fixture` payload by design (documented above); the per-construct modules in P30/P32/P33 resolve it. No placeholder text, no unwired components, no empty-value stubs flow to UI/rendering (this is a library crate).

## Self-Check

- Created files exist: `src/identity.rs`, `src/metadata.rs`, `src/function/mod.rs`, `src/function/scalar.rs`, `src/function/set.rs`, `src/construct/mod.rs`, `tests/m3_baseline_characterization.rs`, `tests/lineage_metadata.rs`, `tests/semantic_ir.rs`, `docs/release/evidence/M3_P25_SEMANTIC_IR.md` — verified.
- Commits exist: `8ebbf8a`, `217aa0c`, `c19c608`, `79c3d9a` — verified in `git log`.
- All verification commands exit 0 — verified above.

## Self-Check: PASSED
