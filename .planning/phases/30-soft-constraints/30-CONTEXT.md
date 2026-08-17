# Phase 30: Soft constraints and feasibility relaxation - Context

**Gathered:** 2026-08-14
**Status:** Ready for planning

<domain>
## Phase Boundary

Deliver exact persistent soft constraints and a distinct solve-scoped weighted-L1 feasibility-relaxation workflow. Persistent softening is canonical and revisioned; repair is an isolated overlay workflow that preserves canonical state, reports explicit mathematical outcomes and provenance, composes with supported P29 IIS origins, and preserves primary plus cleanup/rebuild failures. P30 implements only `PenaltyTarget::{None,Objective}`; P31 owns priority targeting and objective-policy/lexicographic execution.

</domain>

<decisions>
## Implementation Decisions

### Persistent softening and algebra
- **D-01:** Model soft constraints as canonical/revisioned semantic constructs with stable opaque handles and violation roles tied back to the original constraint.
- **D-02:** Preserve exact side algebra: upper rows use `f(x) - v_up <= u`, lower rows use `f(x) + v_lo >= l`; equalities and ranged rows receive distinct lower and upper violations.
- **D-03:** Treat finite `max_violation` as a validated nonnegative cap on the generated violation variable; reject invalid, stale, inactive, or unsupported inputs atomically.
- **D-04:** Keep signed correction as a separate explicit API; never infer it from soft-constraint violation values.

### Repair workflow and outcomes
- **D-05:** Keep feasibility relaxation solve-scoped and isolated from canonical model revision; use the existing overlay/apply/rollback/rebuild contract and exact base/relaxation `CompilationId`s.
- **D-06:** Make portable weighted-L1 (`PortableOnly`) normative, with defaults `AllEligible`, `WeightedL1`, `PortableOnly`, and `RequireOptimal`.
- **D-07:** Distinguish `OptimalRepair`, `FeasibleRepair`, `NoRepairFound`, and `Unknown`; only `AcceptFeasible` may return `FeasibleRepair`, while a non-proven feasible incumbent under `RequireOptimal` is `Unknown`.
- **D-08:** Treat preflight, compile, apply, solve, extraction, rollback, verification, and rebuild failures as typed operational errors, never mathematical outcomes; a cleanup failure prevents a success report from escaping.

### Provenance and P29 composition
- **D-09:** Map supported P29 origins all-or-error: primitive/imported constraint sides, declared/imported variable bound sides, and persistent fixings. Preserve imported source provenance in the resulting report.
- **D-10:** Reject temporary locks, overlay-only fixings, grouped semantic constructs, compiler-generated-only members, and stale instance/revision/compilation identities with explicit typed errors; never silently drop IIS members.
- **D-11:** Record evaluated finite nonnegative penalty weights before backend mutation, together with lineage, instance, revision, compilation identities, provider, termination, and honest numerical evidence.
- **D-12:** Treat IIS as scope/diagnostic input only; do not claim minimum-cardinality or minimum-weight repair from an IIS.

### API boundary and provider policy
- **D-13:** Freeze the P30 public API around the plan's `SoftConstraint`, `ViolationPolicy`, `PenaltyPolicy`, `RelaxationRestriction`, `RelaxationScope`, provider, acceptance, outcome, and report concepts without adding `ObjectivePolicy`, `ObjectivePriority`, or a generic unsupported-provider escape hatch.
- **D-14:** `PreferNative` may use a native provider only after exact semantic qualification and differential agreement; otherwise it falls back to portable with an explicit reason. `NativeRequired` rejects before mutation when qualification is unavailable. Native absence must not block portable P30.
- **D-15:** Preserve shared M3 identity, overlay, error-composition, and solve-session contracts; any conflict with those binding contracts is a stop condition requiring reviewed amendment.

### the agent's Discretion
- Exact existing type/module placement and naming adaptations, provided the frozen public semantics are preserved and duplicate wrappers are avoided.
- Internal formulation data structures, deterministic traversal/order choices, test-fixture organization, and the precise error-enum encoding of the binding contract.
- Honest omission of report fields that a provider cannot populate, as required by the API-freeze rule; no fabricated numerical values.
- Native audit evidence format and the portable fallback reason wording, subject to explicit provider semantics and existing diagnostics conventions.

</decisions>

<specifics>
## Specific Ideas

- The accepted P30 implementation plan is the binding implementation contract; this autonomous discussion resolves its grey areas rather than reopening the design.
- P29's restriction-level origin map and exact `CompilationId` freshness checks are the intended composition seam.
- The workflow should preserve the distinction between a mathematical result and an operationally unhealthy session, including successful mathematics followed by failed rollback.

</specifics>

<canonical_refs>
## Canonical References

### Phase contract and routing
- `.planning/phases/30-soft-constraints/30-PLAN.md` — frozen P30 API, algebra, outcomes, P29 mapping, tasks, gates, and stop conditions.
- `.planning/STATE.md` — P30 is the sole authorized active phase after P36 merge.
- `.planning/ROADMAP.md` — P30 ownership and gate to P31.

### M3 binding semantics
- `.planning/milestones/M3-semantic-modeling-workflows/COMPLETION-ROADMAP.md` — authoritative P30 ownership and completion gate.
- `.planning/milestones/M3-semantic-modeling-workflows/STATE.md` — active-phase authorization and shared-contract freeze.
- `.planning/milestones/M3-semantic-modeling-workflows/SHARED-CONTRACTS.md` §2 — overlay apply/rollback, dirty-session/rebuild, and multi-error preservation.
- `.planning/milestones/M3-semantic-modeling-workflows/COMPLETION-REQUIREMENTS.md` — M3 leaf requirements and qualification expectations.
- `.planning/milestones/M3-semantic-modeling-workflows/DECISIONS.md` — accepted M3 semantic decisions.

### Prior phase composition
- `.planning/phases/29-iis-conflict-analysis/29-CONTEXT.md` — semantic IIS origins, exact identity, scope, and no-minimum-repair boundary.
- `.planning/phases/35-mps-import/35-CONTEXT.md` — imported bound/row provenance and independent-reader qualification context.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `src/model/{constraint,variable,objective,parameter,transaction}.rs` — canonical entities, bounds/fixings, parameter evaluation, objectives, and mutation/revision seams.
- `src/compiler/{bounds,origin,restriction,backend_ir,session,report}.rs` — effective-bound analysis, semantic origin maps, compiled identities, and backend projection/reporting.
- `src/solver/{overlay,session,facade,backend,reference,infeasibility}.rs` — solve-scoped overlays, session lifecycle, backend contract, portable reference path, and P29 analysis types.
- `roml-highs/src/session.rs` and `roml-highs/src/iis.rs` — existing overlay lifecycle, exact compilation checks, and native IIS composition points.

### Established Patterns
- Exact `CompilationId` is the stale-state authority for compiled mappings and solution metadata; model revision alone is insufficient.
- Overlay rollback returns a clean/restored result or `RequiresRebuild`; primary and cleanup failures are retained.
- Semantic restriction origins coexist with entity origins, and deterministic reports are structured data with stable renderings.
- The solver-free `roml` core owns mathematical semantics; backend crates own native capability and qualification decisions.

### Integration Points
- Public exports flow through `src/advanced.rs`, `src/solver/mod.rs`, and `src/lib.rs`.
- Canonical mutation and revision/delta behavior must connect through model transactions/change tracking rather than destructive ad hoc state.
- Portable repair must compile/apply/solve/extract/rollback through the existing `SolverSession`/overlay seams; HiGHS qualification belongs in `roml-highs` and cross-crate tests.
- P29 report members and imported MPS source-aware origins provide the input and provenance path for P30 composition tests.

</code_context>

<deferred>
## Deferred Ideas

- `PenaltyTarget::Priority` and all priority execution belong to P31 with the shared `ObjectivePriority` and lexicographic workflow.
- Canonical `ObjectivePolicy`, objective-stage locking, and lexicographic/weighted objective execution belong to P31.
- Native feasibility-relaxation execution remains optional and cannot expand P30's portable contract; broader native qualification is deferred unless exact equivalence is demonstrated.
- Minimum-cardinality/minimum-weight IIS or repair optimization, broader IIS enumeration, nonlinear/MINLP repair, and additional solver-adapter breadth remain outside M3's critical path.

</deferred>

---

*Phase: 30-soft-constraints*
*Context gathered: 2026-08-14*
