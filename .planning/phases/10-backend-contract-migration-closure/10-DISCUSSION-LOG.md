# Phase 10: Backend contract migration closure — Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-18
**Phase:** 10 (M1R-01) — Backend contract migration closure
**Areas discussed:** BackendSession trait surface, Compatibility shim, Pinned test timing, Status/error/solution architecture, Contract freeze process
**Mode:** Headless (autonomous) — no user interaction available; decisions made from context

---

## D1: BackendSession surface — separated bounded traits

| Option | Description | Selected |
|--------|-------------|----------|
| A | Single `BackendSession` trait with all methods (lifecycle, sync, solve, extraction) | |
| B | Separated bounded traits: `BackendSession` (core) + `SessionHealth` + `SolutionView` + `CallbackSession` + `BackendMetadata` | ✓ |
| C | Factory pattern: `BackendSessionFactory` creates `BackendSession` | |

**Rationale:** The phase packet explicitly says "Avoid one giant trait when lifecycle, synchronization, solve, and extraction can be separate bounded interfaces." The existing `SolverAdapter` is a 13-method monolith — the anti-pattern being replaced. The target flow naturally decomposes into synchronize → solve → extract. Each backend can implement only the traits matching its capabilities (e.g., a lint backend may not implement `CallbackSession`). The worktree's `ReferenceBackend` already decomposes this way with separate `apply_batch()`, `rebuild()`, `normalized_view()` methods.

**Option A rejected:** Reproduces the `SolverAdapter` monolith problem. A single trait with 15+ methods is hard to evolve and forces backends to stub-method capabilities they don't support.
**Option C rejected:** Not needed for HiGHS (the sole mandatory v0.1 backend). Deferred to MOSEK qualification if per-session native environments prove necessary.

---

## D2: Compatibility shim

| Option | Description | Selected |
|--------|-------------|----------|
| A | Remove `SolverAdapter`, `SolverModelExt`, `drain_changes()`, `Model.solver_options`, `Change` (public), `ChangeLog`, `SolveOptions`, `LpAlgorithm`, `SolverStatus` from public API. No shim. | ✓ |
| B | Keep a deprecated shim over the safe session: `#[deprecated] impl SolverAdapter for LegacyShim<impl BackendSession>` | |

**Rationale:** This is pre-v0.1 — no stability promise. Every backend will be rewritten against the new contract (HiGHS in M1R-02, MOSEK in M1R-06, Xpress in M1R-07). The 9 pinned tests that use the legacy API are being deleted (D3). M1R-C7 explicitly allows removal. A shim would add maintenance burden for a bridge nobody needs.

**Option B considered and rejected:** Per D-003, a shim "may exist temporarily only if it delegates to the safe protocol, cannot lose replayability, rejects unsupported policy explicitly, and is loudly deprecated." These constraints make the shim non-trivial to implement. Since no code benefits from it (old tests deleted, all backends rewriting against new contract), the cost exceeds the benefit. The `SolverModelExt::sync_model()` shim would need to intercept the destructive drain and journal it — fragile and complex for zero consumer benefit.

**What stays:** `CallbackHandler`/`CallbackData`/`CallbackCut`/`CallbackAction` (solver-agnostic, migrate to new session). `Solution`/`SolutionBuilder`/`SolutionStore` (value types, complemented by `SolutionView`).

---

## D3: Pinned test un-ignore timing

| Option | Description | Selected |
|--------|-------------|----------|
| A | Incremental — each task un-ignores its relevant tests during implementation | |
| B | Final batch — un-ignore all 9 after all contract changes | |
| C | Delete old pinned tests — new contract tests (Task 01.2) supersede them | ✓ |

**Rationale:** The 9 pinned tests import types being deleted (`SolverAdapter`, `SolverModelExt`, `drain_changes()`, `Model.solver_options`). They literally won't compile after this phase. The new contract tests (Task 01.2) cover the same invariants but against the new API. The old tests prove the legacy system is broken — the new tests prove the revisioned system is correct.

**Option A rejected:** Would require keeping the old API alive for tests while removing it for production — circular dependency. A task removing `SolverAdapter` would break tests that another task depends on.
**Option B rejected:** Keeping tests ignored until the end means no regression protection during the 7-task implementation. Also, the tests won't compile mid-phase as types are removed.

**Test scenario preservation mapping:**
- "drained changes lost on error" → "error preserves journal entry" (contract test)
- "second adapter gets nothing" → "two sessions independently catch up" (contract test)
- "no recovery after partial apply" → "ApplyOutcome distinguishes recoverable/terminal" (contract test)
- "reset has no revision check" → "revision mismatch detected and reported" (contract test)
- "no staleness detection" → "AdapterCursor exposes stale/current state" (contract test)
- "solve options on model" → "SolveRequest is immutable and reusable" (contract test)

---

## D4: Status/error/solution architecture

| Option | Description | Selected |
|--------|-------------|----------|
| A | Single unified types covering all cases in one flat struct/enum | |
| B | Separate type hierarchy per concern (Status enum, Error enum, Solution struct — independently returned) | |
| C | Hybrid — unified `SolveResult` containing typed sub-structures (`SolveRequest`, `EffectiveConfig`, `TerminationStatus`, `SolveSolution`) | ✓ |

**Rationale:** The worktree already prototypes this exact structure (`SolveResult` at `request.rs:84-94`). It's the designed shape from the target flow: `SolveResult { requested, effective, termination, solution_view }`. The hybrid pattern keeps the API simple (one return type) while keeping concerns properly separated internally.

**Option A rejected:** A flat struct with 30 fields is unreadable and hard to evolve. Adding a field to `SolveSolution` shouldn't require touching `SolveResult`.
**Option B rejected:** Forcing consumers to match on multiple return types is unergonomic. The current `SolverAdapter` already does this (separate `solve() -> SolverStatus`, `solution_values() -> Option<HashMap>`, etc.) and it's fragile — callers must remember to call all the getters in the right order.

**Key enrichments over worktree:**
- `SolveResult` gains a `requested: SolveRequest` field (currently only `effective_configuration`)
- New `SolutionView` type for borrowed/indexed access (replaces cloned HashMap pattern)
- `TerminationStatus` already has 11 variants covering M1R-C6 requirements (incumbent, proof, limits, interruption, ambiguity)

**`TerminationStatus` information lattice** (4 tiers):
1. `Optimal` — proven optimal
2. `Feasible` — incumbent found, not proven
3. `Infeasible` / `Unbounded` / `InfeasibleOrUnbounded` — proven infeasibility/unboundedness (or ambiguity)
4. `TimeLimit` / `IterationLimit` / `NodeLimit` / `SolutionLimit` / `Interrupted` / `NumericalIssue` / `Error` — termination without proof

---

## D5: Contract freeze process

| Option | Description | Selected |
|--------|-------------|----------|
| A | Signed ADR at `.planning/adr/ADR-001-backend-contract-freeze.md` with frozen trait/type signatures, freeze SHA, and change process | ✓ |
| B | Light checkpoint in STATE.md + ROADMAP.md only | |
| C | No formal freeze — downstream workers rebase as needed | |

**Rationale:** The gate says "Contract edits after freeze require a recorded decision and coordinated rebase." M1R-02 (HiGHS), M1R-06 (MOSEK), and M1R-07 (Xpress) all depend on a stable contract. Without a freeze, a worker starts implementing against a moving target. The ADR format is already established by `DECISIONS.md` (D-001 through D-012). This is consistent with D-009: "Shared contract files have one integration owner."

**Option B considered but insufficient:** STATE.md tracks phase state, not interface contracts. An ADR gives the contract its own canonical location — one file where downstream workers find the exact trait signatures and the process for proposing changes.
**Option C rejected:** "Coordinated rebase" without a freeze means every worker is chasing a moving target. HiGHS, MOSEK, and Xpress workers need a single SHA to point at as their contract baseline.

**ADR contents (minimal):**
1. Frozen trait signatures (`BackendSession`, `SessionHealth`, `SolutionView`, `CallbackSession`, `BackendMetadata`)
2. Frozen type signatures (`Synchronization`, `SyncReceipt`, `SolveRequest`, `SolveResult`, `TerminationStatus`, `EffectiveConfig`, `SolveSolution`, `BackendError`, `DeltaBatch`, `ModelOp`, `ApplyOutcome`, `AdapterCursor`, `AdapterHealth`, `ModelRevision`, `ModelSnapshot`, `Journal`)
3. Freeze commit SHA
4. Change process: edit → recorded decision (ADR update) → notify downstream workers → coordinated rebase
5. Cross-reference in STATE.md

---

## Claude's Discretion

All 5 decisions made autonomously. No user interaction was available (headless mode). Decisions are grounded in:
- Phase packet requirements and constraints
- Existing program decisions (D-002, D-003, D-006, D-008, D-009, D-010)
- Phase 9 truth-reset findings (M1.1 FAILED, legacy patterns documented, pinned test inventory)
- Worktree analysis (existing revisioned protocol types, their shape and completeness)
- Current source analysis (legacy surface to remove, migration path)

No user preferences overridden. All decisions align with the explicit program constraints.

## Deferred Ideas

1. **Factory pattern** (D1, Option C) — deferred to M1R-06 (MOSEK) if per-session native environments prove necessary
2. **Async solve** — deferred to M3 (persistent incremental runtime); synchronous `solve()` for now
3. **Internal Change → ModelOp conversion** — implementation detail, not a contract decision. The external contract only sees `ModelOp`.
4. **SolverStatus deprecation** — HiGHS references to `SolverStatus` migrate in M1R-02, not M1R-01
5. **semver-checks baseline** — establish after contract freeze; v0.1 has no prior version to compare against
