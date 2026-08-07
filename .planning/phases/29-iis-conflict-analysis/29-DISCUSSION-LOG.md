# Phase 29 Discussion Log

## Input and resolution

The owner-approved IIS design packet was fetched without changing branches:

```text
git fetch origin docs/p29-iis-design-packet
git show FETCH_HEAD:.planning/phases/29-iis-conflict-analysis/29-DESIGN-PACKET.md
```

The packet is binding owner direction. The four discussion areas were deliberately not re-asked because the user supplied their settled choices. Those choices are recorded in `29-CONTEXT.md` and are the contract consumed by the seven implementation slices.

## Decisions encoded

1. **IIS tiers:** portable ROML LP IIS is first-class; native IIS is a separately labeled provider; `Auto` is native-seed → semantic ROML reduction → verification with ROML fallback; `RomlPortable`, `NativeOnly`, and `NativeThenRoml` are explicit; feasibility relaxation is separate; no minimum-cardinality claim.
2. **Entry point and lifecycle:** `SolverSession::analyze_infeasibility(&mut self, &Model, &InfeasibilityPlan)`; no primary `Model` method; exact compilation and snapshot; isolated repeated-solve session; incremental toggles/basis reuse; persistent solve session preserved.
3. **Origin mapping:** semantic restriction atoms and restriction-level origin map coexist with entity `OriginMap`; sides, bound layers, fixings, locks, temporary fixings, and grouped constructs are covered; exact `CompilationId` is mandatory; native conflicts are semantically mapped and re-reduced.
4. **Report/oracle:** one canonical report with deterministic text/Markdown renderers and concise `Display`; completion and guarantee are independent; exact lineage/identity/provider/scope/universe/policy/evidence/statistics/warnings; the oracle returns only the three permitted outcomes; Unknown blocks irreducibility.
5. **Scope seam:** explicit `LpRelaxation` is allowed and labeled; LP-feasible/MIP-infeasible analysis is next phase; function-in-set semantic members preserve the future nonlinear seam; local restoration failure is never an IIS.

## Internal review checklist completed

- The seven plans align one-to-one with the packet slices: contract/characterization, semantic universe, oracle, reducer/verifier, report/renderers, native HiGHS, and qualification/performance evidence.
- The plans do not add feasibility relaxation, MIP-only conflict analysis, nonlinear feasibility, all-IIS enumeration, or minimum-cardinality optimization.
- The primary API is session orchestration, and the plan forbids canonical/persistent-session mutation during analysis.
- Every strong guarantee requires proven tri-state outcomes and a fresh final verification pass.
- Exact `CompilationId` is threaded through compilation, toggles, cache identity, native evidence, mapping, and report re-resolution.
- The HiGHS slice is blocked on authoritative generated binding/header/source and system-version audit. No signature, layout, status, or strategy mask is guessed.
- Tests cover correctness, bound-stack mutation, stale mapping, rollback recovery, differential native/portable behavior, bundled/system qualification, mutation resistance, and planted-IIS performance.

## Concrete blockers and stop conditions

1. **GSD routing — resolved:** root `.planning/ROADMAP.md` and `.planning/STATE.md` now project active M3 execution, `.planning/phases/29-iis-conflict-analysis/` is canonical, and the confirmed-empty duplicate directory is gone. `init.phase-op 29` resolves exactly one phase directory.
2. **Native qualification:** the exact pinned `highs-sys` 1.15.0 generated API, matching official HiGHS header/source, and every supported system version are not yet attached as evidence. Native implementation stops at typed `Unsupported` if compile-time-safe qualification cannot be established.
3. **Contract reconciliation:** the existing uncommitted `src/solver/infeasibility.rs` and placeholder facade method are partial work. Implementation must reconcile them with this packet before tests are treated as evidence; the current optional `CompilationId` and string native membership are not accepted as final contract.
4. **Baseline provenance:** the packet header names `b04f0f5`, while repository instructions require `main@82e2ed9`. The latter governs implementation; the exact implementation base must be recorded in evidence.

No owner/legal release action is required for P29, and publication/tagging is explicitly out of scope.

## Next review gate

Review `29-PLAN.md` and `29-01-PLAN.md` through `29-07-PLAN.md` for executable file/interface/test consistency. Close the GSD routing ambiguity before execution. Then begin the phase in serial dependency order at Slice 1; the HiGHS provider cannot be accepted until its header/version audit and compile-gated qualification matrix are attached. Independent specification/correctness and integration/operations reviews are required before Phase 29 can be marked complete.
