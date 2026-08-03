# M3 Current-Main Research

## Baseline inspected

Planning was anchored to `main@d1f1ad38cec75abb671729df8efb87736861628c` on 2026-08-02.

Primary files inspected:

- `AGENTS.md`
- `.planning/PROJECT.md`
- `.planning/ROADMAP.md`
- `.planning/STATE.md`
- `.planning/milestones/M2-public-api-ergonomics/*`
- `MODELING_API.md`
- `src/model/mod.rs`
- `src/model/changelog.rs`
- `src/delta.rs`
- `src/snapshot.rs`
- `src/solver/backend.rs`
- `src/solver/session.rs`
- `src/solver/request.rs`
- `src/solution/mod.rs`
- `src/solution/metadata.rs`

## Findings

### 1. The current architecture already has the right persistent-state boundaries

ROML has:

- a solver-independent `Model`;
- canonical entities and coefficient cells;
- a revisioned `DeltaBatch`/`ModelOp` protocol;
- deterministic `ModelSnapshot` rebuilds;
- persistent backend sessions;
- explicit solve requests and effective configuration;
- normalized immutable solutions;
- failure classification and backend health.

M3 should extend these boundaries rather than replace them.

### 2. Canonical snapshots are currently linear-primitive snapshots

`ModelSnapshot` currently contains variables, constraints, objectives, parameters, and coefficient cells. It cannot preserve an indicator, PWL relation, exact max, or soft constraint without eager expansion.

**Consequence:** P25 must extend canonical state; P26 must prevent backend sessions from consuming canonical snapshots directly.

### 3. Canonical and backend operations currently share one vocabulary

`ModelOp` contains operations such as variable/constraint addition, bound changes, cells, objective changes, and semi-continuous bounds. This was correct for a linear canonical model, but semantic constructs require a separate compilation step.

**Consequence:** retain canonical `ModelOp`; introduce compiled/backend operations rather than adding solver-representation choices to `ModelOp`.

### 4. The current backend contract is intentionally decomposed

`BackendSession` has synchronization/solve/close, with optional traits for health, solution views, callbacks, and metadata. This decomposition supports adding optional IIS/start/hint traits without growing one mandatory monolith.

**Consequence:** preserve trait decomposition; amend synchronization payloads only.

### 5. The flat capability record is near its useful limit

The current struct covers LP/MIP, incremental mutations, callbacks, solution data, duals/reduced costs, semi domains, and parameter updates. M3 adds starts, partial starts, multiple starts, hints, IIS, feasibility relaxation, indicator, SOS, PWL, and multiobjective semantics.

**Consequence:** migrate to typed feature queries with limitations and runtime-version sensitivity.

### 6. SolveRequest already establishes the correct policy rule

Current documentation says one solve request is immutable and unsupported options must be applied, adjusted, or rejected rather than ignored.

**Consequence:** `SolvePlan` should contain `SolveOptions` and model-referencing solve instructions; it should not move transient state into `Model`.

### 7. Solution metadata lacks model identity

Solutions record model revision but not independent model lineage. `VarId` generations protect stale entities inside one lineage but do not prevent accidental reuse across independent models.

**Consequence:** add `ModelLineageId` before assignments, starts, locks, or result reuse.

### 8. Variable fixing fits the existing incremental machinery but needs semantic state

A fixing can compile to the existing variable-bound update path. However, storing only effective bounds would lose the declared domain and make `unfix` ambiguous.

**Consequence:** variable state separates declared domain and fixing; compiled effective bounds remain ordinary backend bounds.

### 9. Temporary fixings cannot be journaled and undone safely

Using ordinary model mutations for solution locking would create false revisions and expose temporary state to every adapter. Undo operations could fail after a solve and leave persistent backend/model divergence.

**Consequence:** solve overlays require an explicit reversible backend lifecycle and rebuild fallback.

### 10. Current solution storage can support assignment extraction

`Solution` already owns immutable variable values and metadata. Adding `primal_assignment()` is additive, but it requires lineage metadata.

### 11. Names exist and are first-class but richer diagnostics need metadata/provenance

M2 exposed names and model pretty-printing. IIS and generated-formulation diagnostics also need groups, tags, source keys, bound provenance, and construct roles.

**Consequence:** metadata/provenance is foundational, not a reporting afterthought.

### 12. Current model code still carries a semi-continuous side map

`Model` tracks semi-continuous lower bounds separately. A semantic/compiler redesign is an opportunity to avoid multiplying similar side maps for every future construct.

**Consequence:** constructs live in a dedicated store and compile through recipes; do not add indicator/PWL/minmax maps directly to `Model` fields.

### 13. M2 deliberately deferred this exact feature set

M2's explicit non-goals include IIS, basis/warm-start public APIs, multiobjective policy, nonlinear modeling, and indexed containers. M3 can now address selected items without reopening M2's ordinary API decisions.

### 14. HiGHS system and bundled versions differ materially

Current qualification supports a bundled newer HiGHS and an older system-discovery version. Callback compatibility already required version-portable code.

**Consequence:** starts, IIS, PWL, and multiobjective capabilities must be declared per actual loaded/compiled version. The implementation cannot assume bundled-version fields exist on the minimum system version.

### 15. The current package description is explicitly incremental MILP

M3 should not relabel ROML as an NLP library. Documentation should say MILP-first semantic optimization modeling with an extension-ready architecture.

## Competitive/API research conclusions

The design follows several proven principles without copying a framework wholesale:

- distinguish MIP starts from variable hints;
- use bridge layers for unsupported semantic constraints;
- preserve native capability metadata;
- use sequential objective locking as a portable lexicographic fallback;
- distinguish IIS/conflict analysis from feasibility relaxation;
- use convex epigraph/concave hypograph PWL formulations without binaries;
- use SOS2/native/binary formulations for exact graph relations;
- avoid arbitrary Big-M constants.

M3 should remain smaller than a general MathOptInterface-style universe. The initial construct family is intentionally bounded to common MILP modeling needs.

## Recommended code ownership boundaries

```text
canonical model ownership:
  identity, metadata, function/set, constructs, objectives, fixings

compiler ownership:
  capabilities, backend IR, recipes, bridges, bounds, origins, reports

solver orchestration ownership:
  SolvePlan, overlays, starts/hints, stage execution, rollback

backend ownership:
  official API translation, runtime feature support, native IIS/start/multiobjective

reporting ownership:
  conflict/violation/objective-stage projection and renderers
```

## Characterization tests required before refactoring

1. M2 README/modeling-guide examples compile and solve.
2. primitive snapshots are deterministic.
3. primitive delta application equals rebuild.
4. parameter updates reuse HiGHS and preserve objective constants.
5. bounds, names, solution metadata, duals, and reduced costs retain current semantics.
6. backend failure/rebuild behavior remains bounded to one automatic retry.
7. public API inventories for `roml` and `roml-highs` are captured.

## Non-negotiable conclusion

Do not implement indicators, PWL, slacks, or lexicographic solves directly inside `roml-highs` or as helper functions that eagerly add rows to `Model`. The compiler/origin/capability boundary must land first.
