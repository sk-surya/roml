# Phase 29 Context — IIS/Conflict Analysis

**Status:** owner-directed discussion resolved; implementation complete and accepted
**Binding input:** the packet fetched with `git fetch origin docs/p29-iis-design-packet` and read from `FETCH_HEAD:.planning/phases/29-iis-conflict-analysis/29-DESIGN-PACKET.md` (PR #38, dated 2026-08-06).
**Repository baseline for implementation:** `main@82e2ed95545635b628187ba0081fe8c8b03eaafb`, as required by repository instructions. The packet header also names `b04f0f516b159548327b868b5011fe2c24fe420a`; that is retained as packet provenance, not substituted for the authoritative repository baseline.
**Scope:** LP infeasibility only; this discussion/planning pass makes no production implementation changes.

## Resolved owner decisions

### Provider tiers and guarantees

- Phase 29 ships a first-class, solver-agnostic ROML LP IIS engine.
- Backend-native IIS is an optional, separately labeled provider. `Auto` prefers a qualified native seed, then maps it to semantic atoms and runs ROML reduction and fresh verification. If native support is unavailable, `Auto` falls back to ROML-only analysis.
- The explicit modes are `Auto`, `RomlPortable`, `NativeOnly`, and `NativeThenRoml`. `NativeOnly` never falls back. Feasibility relaxation is a separate API.
- A complete ROML result may claim semantic irreducibility only relative to the recorded candidate universe, grouping, feasibility oracle, and numerical policy. It never claims native support or minimum cardinality. Native evidence remains native evidence unless semantic reduction and verification establish the stronger ROML claim.

### Orchestration and isolation

- The primary entry point is plan-driven orchestration on `SolverSession`:

  ```rust
  pub fn analyze_infeasibility(
      &mut self,
      model: &Model,
      plan: &InfeasibilityPlan,
  ) -> Result<InfeasibilityReport, InfeasibilityError>;
  ```

- It is not a `Model` method. The operation compiles the requested scope exactly once, creates an isolated analysis session from the exact `BackendSnapshot`, and preserves the persistent solve session and its compiled state.
- Normal operation uses one isolated build, stable compiled indices, incremental restriction toggles, and basis reuse where the backend audit qualifies it. Rebuilding for each candidate is forbidden. Uncertain mutation or rollback marks only the analysis session for recovery/rebuild.

### Semantic atoms and mapping

- The public conflict unit is a semantic restriction atom, not a compiled row.
- Add a restriction-level origin map beside the existing entity-level `OriginMap`. It covers individual constraint sides, variable lower/upper bounds, persistent fixings, solve locks, temporary fixings, and grouped semantic constructs. The abstraction remains function-in-set so future convex/nonlinear members need not become matrices.
- Bound contributions are a stack: declared bounds → persistent fixing → solve-scoped fixing/lock. Disabling a layer restores the next lower contribution; it never blindly restores infinity.
- Every mapping, toggle, cache lookup, native conversion, and report re-resolution requires the exact `CompilationId`; stale mappings reject with a typed error.
- Native row/column/bound-side conflicts are compiled evidence. They must map to semantic atoms and be semantically re-reduced before an irreducibility claim.

### Report and oracle contract

- `InfeasibilityReport` is the one canonical structured result. Dedicated deterministic text and Markdown renderers are stable views; `Display` is concise and not the full rendering contract.
- The report records model lineage, model instance, model revision, mandatory exact `CompilationId`, backend identity/version, provider chain, scope (`OriginalLp` or explicitly requested `LpRelaxation`), candidate universe and grouping, numerical policy, completion, oracle strength, guarantee, semantic members, compiled/native evidence, statistics, and warnings.
- Completion and guarantee are independent. No minimum-cardinality claim is representable. Unknown or limited oracle checks prevent irreducibility.
- The feasibility oracle has exactly three outcomes: `ProvenFeasible`, `ProvenInfeasible`, and `Unknown`. Ambiguous, numerical, interrupted, and limit statuses map to `Unknown`; none may be coerced to infeasible.

## Algorithm baseline

Implementation slices must preserve this order:

```text
exact compilation
-> semantic conflict universe
-> isolated tri-state feasibility oracle
-> cheap contradiction scan
-> native/Farkas/elastic/full-universe seed
-> adaptive chunk deletion
-> exact single-atom deletion polish
-> fresh final verification
-> semantic origin mapping and deterministic rendering
```

The default reducer is deterministic and hybrid. It is not a minimum-cardinality optimizer and must remain behind an internal reducer seam so later evidence can compare a divide-and-conquer strategy without changing the public contract.

## Scope and model-class decisions

- A continuous linear infeasible model is analyzed as `OriginalLp`.
- A MIP LP relaxation is analyzed only when the caller explicitly requests `LpRelaxation`; the report must say so and may not call it an original-MIP IIS.
- If the LP relaxation is feasible but the MIP is infeasible, return typed `Unsupported` or `NoConflictInRequestedScope`; defer the MIP conflict phase.
- Feasible models return `NoConflict` in the canonical report shape. Unknown initial feasibility returns an incomplete result without an irreducibility claim.
- Local NLP restoration failure is never an IIS. Nonlinear/MINLP claims require a future globally conclusive or qualified native provider; P29 only lands the extension seam.

## Existing code and partial-work disposition

The following are extension points, not approvals of their current implementation:

- Compiler identity/provenance: `src/compiler/{backend_ir,capability,origin,report,session}.rs`.
- Bound/domain and semantic state: `src/compiler/bounds.rs`, `src/model/{constraint,transaction,variable}.rs`, `src/solver/overlay.rs`.
- Orchestration/contracts: `src/solver/{backend,facade,reference,session}.rs`; the existing `src/solver/infeasibility.rs` is a characterization stub and must be reconciled with this contract.
- Public exports: `src/advanced.rs`, `src/lib.rs`, `src/solver/mod.rs`.
- HiGHS session/projection: `roml-highs/src/{bindings,compiler,error,ffi,lib,session,start}.rs`; native IIS belongs in the new `roml-highs/src/iis.rs`, not the warm-start `start.rs` module.
- Existing tests to extend: `tests/{compiler_identity,compiler_sync,differential_harness,end_to_end_equivalence,fixing_assignment,model_characterization,semicontinuous_recovery,solve_overlay,status_mapping,typed_capabilities}.rs` and `roml-highs/tests/{conformance,contract_tests,solve_plan}.rs`.
- Existing `roml-highs/src/ffi.rs` contains handwritten declarations despite the broader release policy. P29 native IIS code may not use it; the Phase 29 audit must record whether the remaining declarations are removed/migrated in this phase or are an explicit P3 dependency. No new handwritten declaration may be added.

## Requirement and decision traceability

| Phase 29 obligation | Governing source | Planned closure |
|---|---|---|
| Separate portable/native providers and explicit unsupported behavior | Packet D1; D-005; D-014; R4.1, R4.6 | Plans 29-01, 29-04, 29-06 |
| Session plan entry point and persistent-session preservation | Packet D2; D-002, D-004, D-018 | Plans 29-01, 29-03, 29-04 |
| Semantic atoms, exact compilation identity, origin completeness | Packet D3; D28; D-003, D-015; R2.6, R2.9 | Plans 29-01, 29-02, 29-05 |
| Tri-state status and typed recovery | Packet §5/§8; R4.2, R4.3; audit C1/C2 | Plans 29-01, 29-03, 29-04 |
| Native generated-binding boundary and checked returns | Packet §6; D-006, D-009, D-010; R5.1, R5.2, R5.9–R5.11, R6.1 | Plan 29-06 |
| Deterministic report/rendering and docs | Packet D4; D-012, D-014; R9.1–R9.3, R9.6 | Plan 29-05 and 29-07 |
| Incremental/rebuild equivalence and evidence | D-015, D-017; R3.5, R8.1–R8.3 | Plans 29-03, 29-04, 29-07 |
| Bundled/system HiGHS qualification | Packet §6/§10; R7.4 | Plans 29-06, 29-07 |

## Non-goals

P29 does not enumerate all IISs, optimize minimum cardinality, implement feasibility relaxation, diagnose integrality-only MIP infeasibility, claim exact-rational proof, expose raw backend row IDs as the ordinary report, mutate canonical model state or the persistent solve session, implement nonlinear feasibility, or publish/tag/release crates.

## Planning/execution gates

The GSD routing gate is now closed: root `.planning/ROADMAP.md` and `.planning/STATE.md`
project the active M3 milestone, `.planning/phases/29-iis-conflict-analysis/` is the
canonical phase directory, and the confirmed-empty duplicate directory was removed.
The GSD phase resolver now identifies Phase 29 without ambiguity.

The implementation gate was satisfied for bundled HiGHS 1.15.0 by the generated-binding/header/source audit and compile-gated native provider. System HiGHS 1.9 portable qualification passed; system-native IIS remains explicitly `Unsupported` pending a separate exact header/library matrix. Phase 29 was independently reviewed CLEAR and merged in PR #39 at `main@19c8c70e3f463fc96b2b723537deb71759b825f5`.
