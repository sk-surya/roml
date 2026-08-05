# HiGHS MIP start / hint API audit (P28)

Recorded per the M3 "Native API research protocol" (`EXECUTION.md`): inspect
the exact pinned bundled/system official headers, record symbol signatures,
availability, return codes, lifecycle, and documented semantics, then implement
only qualified support.

## Scope

P28 qualifies HiGHS warm-start behavior for the `SolvePlan` executor
(SM-08.7): MIP starts (`MipStart`/`PartialMipStart`), multiple starts
(`MultipleMipStarts`), variable hints (`VariableHints`), and LP-basis warm
starts (`InitialBasis`).

## Pinned artifacts inspected

- Bundled header: `highs-sys 1.15.0`
  `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/highs-sys-1.15.0/HiGHS/highs/interfaces/highs_c_api.h`
  (2665 lines).
- Bundled C API implementation:
  `.../HiGHS/highs/interfaces/highs_c_api.cpp`.
- Bundled C++ implementation: `.../HiGHS/highs/lp_data/Highs.cpp`
  (`Highs::setSolution` overloads).
- Runtime version (from the bundled build): `1.15.0`.
- CI system floor: HiGHS `1.9.0` (the capability declarations are written
  against the bundled version with the floor noted).

## Return codes

The C API returns a `kHighsStatus`:

| Constant | Value |
|----------|-------|
| `kHighsStatusError` | -1 |
| `kHighsStatusOk` | 0 |
| `kHighsStatusWarning` | 1 |

Verified at `highs_c_api.h:28-30`. `roml-highs` already aliases these as
`STATUS_ERROR`/`STATUS_OK`/`STATUS_WARNING` in `src/bindings.rs`.

## Primal start primitives

### `Highs_setSparseSolution` (qualified)

```c
HighsInt Highs_setSparseSolution(void* highs, const HighsInt num_entries,
                                 const HighsInt* index, const double* value);
```

- Header: `highs_c_api.h:1305`.
- Documented semantics: "Set a partial primal solution by passing values for
  a set of variables" (`num_entries`, `index`, `value`).
- Implementation (`highs_c_api.cpp:750`): forwards to
  `Highs::setSolution(num_entries, index, value)`.
- `Highs::setSolution(num_entries, ...)` (`Highs.cpp:2576`):
  - Rejects (returns `kHighsStatusError`) an index outside `[0, num_col)`.
  - Rejects (returns `kHighsStatusError`) a value outside the column bounds
    beyond `primal_feasibility_tolerance` ("User solution value ... is
    infeasible for bounds [...]").
  - Warns (returns `kHighsStatusWarning`) on duplicate indices — the last
    value wins.
  - Builds a full solution vector (`kHighsUndefined` for unset entries) and
    calls the dense `setSolution(const HighsSolution&)`.
- `Highs::setSolution(const HighsSolution&)` (`Highs.cpp:2492`):
  - A new primal solution (`col_value.size() >= num_col`) calls
    `invalidateSolverData()` (clears the previous solution and any basis).
  - Stores the incumbent in `solution_` with `value_valid = true`.
  - Computes row values from the column-wise matrix.

**Verdict:** `Highs_setSparseSolution` is the qualified native partial-MIP-start
primitive. It supports both FULL starts (assign every column) and PARTIAL
starts (assign a subset). It is available in the pinned bundled `1.15.0` and
the CI floor `1.9.0`.

### `Highs_setSolution` (full primal + dual solution, not used in P28)

```c
HighsInt Highs_setSolution(void* highs, const double* col_value,
                           const double* row_value, const double* col_dual,
                           const double* row_dual);
```

- Header: `highs_c_api.h:1291`.
- Documented semantics: "Set a solution by passing the column and row primal
  and dual solution values. For any values that are unavailable, pass NULL."
- Available, but the sparse form is the right match for a `MipStart` value
  map; P28 qualifies and uses `Highs_setSparseSolution` only.

## Basis warm-start primitives (API present, NOT qualified in P28)

```c
HighsInt Highs_setBasis(void* highs, const HighsInt* col_status,
                        const HighsInt* row_status);
HighsInt Highs_setLogicalBasis(void* highs);
```

- Headers: `highs_c_api.h:1264` and `:1274`.
- These APIs EXIST in the pinned header. P28 does NOT qualify `InitialBasis`:
  SM-08.6 keeps the LP-basis warm start a separate future artifact, never
  conflated with a primal-assignment MIP start (D8). The capability
  declaration is `Unsupported` with a note citing this audit record and
  SM-08.6 — a deliberate scope decision, not a guessed absence.

## Absent APIs

The following symbols are ABSENT from the pinned bundled `1.15.0` header
(confirmed by grepping `highs_c_api.h`):

- `Highs_setMipStart` — no dedicated MIP-start API.
- `Highs_clearMipStart` / `Highs_clearSolution` — no solution-clear API.
- any variable-hint symbol (`hint`, `Hint`, `Highs_setMipHint`, ...).

**Verdict:**

- `MultipleMipStarts`: `Unsupported` — there is a single incumbent slot;
  `Highs_setSparseSolution` overwrites the previous solution
  (`invalidateSolverData`). No multi-start API exists.
- `VariableHints`: `Unsupported` — no hint API exists in this version; absent
  hints reject by default (the P28 blocking decision).

## Lifecycle (stale-start question)

`Highs_setSparseSolution` stores the incumbent in the instance's `solution_`.
It persists until:

- the solver data is invalidated by a subsequent `setSolution` /
  `invalidateSolverData` (a model change / rebuild path), or
- a solve replaces `solution_` with its own result.

There is NO `Highs_clearSolution` C API in this version. In the ROML
architecture this is bounded structurally:

1. The `SolvePlan` executor applies starts immediately before the solve, one
   batch per `solve_plan` call (design §12 "apply starts/hints" step), so a
   start is never applied speculatively.
2. A compiled rebuild (`CompiledRebuild`) recreates the native model, clearing
   any incumbent.
3. A set solution is a search HINT, not a constraint: it cannot change a
   proven optimum. A stale incumbent from a previous solve therefore does not
   change the reported optimal objective or the optimal primal solution of an
   unrelated later solve (the no-stale-start determinism invariant).

The executor's no-stale-leak test models the one-shot consumption contract:
a start is consumed by the solve it was applied to.

## Version availability

| Symbol | Bundled 1.15.0 | CI floor 1.9.0 |
|--------|----------------|----------------|
| `Highs_setSparseSolution` | present | present (documented HiGHS API since 1.x) |
| `Highs_setSolution` | present | present |
| `Highs_setBasis` / `Highs_setLogicalBasis` | present | present |
| `Highs_setMipStart` | absent | absent |
| `Highs_clearSolution` | absent | absent |
| variable-hint symbols | absent | absent |

## Capability declarations derived from this audit

| BackendFeature | Level | Evidence |
|----------------|-------|----------|
| `MipStart` | Native | `Highs_setSparseSolution` (full assignment) |
| `PartialMipStart` | Native | `Highs_setSparseSolution` (subset assignment) |
| `MultipleMipStarts` | Unsupported | single incumbent slot; no multi-start API |
| `VariableHints` | Unsupported | no hint API; reject by default |
| `InitialBasis` | Unsupported | API present but out of scope (SM-08.6 separate artifact) |
