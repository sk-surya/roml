# M2 — P23 Public API Review

**Phase:** 23-surface-curation (Task 6)
**Requirement IDs:** API-07 (07.1–07.5), API-08 (08.1–08.3)
**Verification head:** `308951a` (docs commit precedes the evidence commit)

## Evidence

- `docs/release/evidence/M2_P23_public_api_roml.txt` (10 737 lines, normalized with `$REPO`)
- `docs/release/evidence/M2_P23_public_api_roml_highs.txt` (106 lines, normalized with `$REPO`)
- Toolchain: `cargo-public-api 0.52.0`, `rustc 1.97.1`, `cargo 1.97.1`, macOS arm64
- Command: `cargo public-api -p roml` / `cargo public-api -p roml-highs`
- Baseline compared against: `docs/release/evidence/M2_P20_public_api_roml.txt` (7431 lines) and `M2_P20_public_api_roml_highs.txt` (80 lines)

## Summary

The P20 → P23 diff is consistent with the curated intent. `roml` grew from
7431 → 10 737 lines and `roml-highs` from 80 → 106 lines. The growth is the
P21/P22/P23 additions (SolverSession/SolveOptions/Solution, definition
builders, names, the `roml::advanced` grouping, the `VarId - VarId`
operator) plus the prelude/advanced re-export metadata. **Every removal is an
intentional, documented pre-1.0 break with migration coverage.**

## Removals (P20 → P23) — all intentional

| Category | Removed item(s) | Disposition / replacement |
|---|---|---|
| Raw arena internals (API-07.4) | `roml::id::IdArena` (whole public surface) | made crate-private (P23 Task 2) |
| Prelude protocol exports (API-07.2) | `roml::prelude::{Change, ValueExpr, SolverStatus}` and the prelude re-export lines of `DeltaBatch`/`ModelOp`/`ModelRevision`/`ModelSnapshot`/`CoeffId`/`AdapterCursor`/`AdapterHealth`/`Synchronization`/`BackendSession`/`SyncReceipt` | absent from prelude; grouped under `roml::advanced` (D9) |
| Canonical status name | `roml::solver::SolverStatus` enum lines | `SolverStatus` is now a type alias of `SolveStatus` (P21) |
| Signature-collision breaks (P21/P22) | `Model::add_constraint(ConstraintBounds) -> ConId`, `Model::add_variable(Bounds, VarType) -> VarId`, `Model::add_integer(Bounds) -> VarId`, `Model::add_parameter(f64) -> ParamId`, infallible `Model::set_parameter(ParamId, f64)` | generic spec/definition forms with fallible return types (D7/D10); documented in `MIGRATION.md` and the P20 signature-collision migration |
| `Solution` constructor/status signatures | `Solution::{new,from_values,status}`, `SolutionBuilder::status` with `SolverStatus` | now take/return `SolveStatus` (alias) |

The `Model::deltas_since(rev)` helper (`/// for testing`, disposition
"internal exposure to remove") remains public and is still used by the P21/P22
suite; its removal is deferred to P24 so the suites stay green (no weaker
surface in P23).

## Additions (P20 → P23) — all intentional

- **P21:** `roml::solver::{facade, options, error}` modules, `SolverSession`,
  `SolveOptions`, `SolveError`, `SolveStatus`, `SolveMetadata`,
  `SynchronizationMode`, `normalize_result`; `roml_highs::Highs`
  (`new`/`solve`/`solve_with`).
- **P22:** `VariableDef`/`ParameterDef`, `continuous`/`integer`/`binary`/
  `parameter`, semantic aliases, `add_constraint`/`minimize`/`maximize`,
  name getters, `add_objective_named`, `add_empty_constraint`, the D11 sparse
  trio (`set_coefficient`/`add_to_coefficient`/`remove_coefficient_at`).
- **P23:** `roml::advanced` grouping (395 reference lines, 60 top-level
  symbols), the curated `roml::prelude`, `pub fn roml::id::VarId::sub` (the
  new `VarId - VarId` operator).

### API-07.2 negative inventory (verified against the P23 output)

Each of the following has **zero** matches under `roml::prelude::` in
`M2_P23_public_api_roml.txt`:

`Change`, `CoeffId`, `DeltaBatch`, `ModelOp`, `ModelRevision`,
`ModelSnapshot`, `AdapterCursor`, `AdapterHealth`, `Synchronization`,
`BackendSession`, `SyncReceipt` (also `ValueExpr`, `ConstraintBounds`).

Each is present under `roml::advanced::`. The curated prelude re-exports only
`Model`, `ModelError`, the semantic aliases, `VariableDef`/`ParameterDef`,
`continuous`/`integer`/`binary`/`parameter`, `LinExpr`/`ConstraintSpec`/
`ObjectiveSpec`/`ConstraintExprExt`/`ObjectiveExprExt`, `Bounds`/`Sense`/
`VarType`, `SolveOptions`/`Solution`/`SolveStatus`/`SolveError`, and the pure
`constraint!` macro (API-07.1).

## cargo-semver-checks — skipped (recorded reason)

`cargo-semver-checks` is **not installed** and no pre-1.0 semver baseline tag
is configured. The plan says to run it "if configured"; the M2 baseline
matrix does not configure it (pre-1.0, `0.1.x`). Skipped and recorded rather
than claimed as passing. Revisit before the first 1.0.0 release (P24
qualification).

## Intentional breakage and migration coverage

Every removal/signature change above has a before/after entry in
`MIGRATION.md` (API-08.2) and the deprecated APIs remain tested
(API-08.3):

- `tests/compatibility_api.rs` — the full deprecated surface still runs.
- `tests/prelude_contract.rs` + the prelude `compile_fail` doctest — the
  curated prelude is sufficient and the protocol types are absent.
- `tests/advanced_surface.rs` — the `roml::advanced` surface is sufficient
  for a backend author.
- P20/P21/P22 suites keep exercising the deprecated wrappers under
  `#[allow(deprecated)]`.

## Independent API review

An independent API-coherence review (per API-10.6) has **not** been performed
by a separate reviewer in this execution. It is recommended before the P23
branch merges, focused on: protocol preservation, error semantics, and the
prelude/advanced split documented here.
