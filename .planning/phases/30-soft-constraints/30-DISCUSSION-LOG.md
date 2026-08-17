# Phase 30: Soft constraints and feasibility relaxation - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in `30-CONTEXT.md`; this log preserves the alternatives considered.

**Date:** 2026-08-14
**Phase:** 30-soft-constraints
**Discussion agent:** Luna (autonomous smart-discuss)
**Mode:** Non-interactive; accepted project decisions and the existing owner-approved P30 plan supplied the defaults.
**Areas discussed:** persistent softening, repair workflow/outcomes, provenance and P29 composition, API/provider boundary

---

## Persistent softening and algebra

| Option | Description | Selected |
|--------|-------------|----------|
| Canonical semantic softening | Persist soft constructs in model state/revision, with stable violation roles and exact side algebra | ✓ |
| Solve-only penalty overlay | Represent all softening only as temporary solve state | |
| Backend-native row mutation | Let each adapter define softening algebra independently | |

**Autonomous choice:** Canonical semantic softening, as frozen by the P30 plan.
**Notes:** Upper/lower/equality/ranged rows retain distinct violation roles. Finite caps are nonnegative and validated atomically. Signed correction remains a separate API.

## Repair workflow and outcomes

| Option | Description | Selected |
|--------|-------------|----------|
| Portable weighted-L1 normative provider | Isolated solve-scoped repair with explicit provider policy and exact outcome classification | ✓ |
| Native provider normative | Require backend-native relaxation semantics as the primary contract | |
| Reuse generic unsupported-feature policy | Hide provider choice behind existing unsupported handling | |

**Autonomous choice:** Portable weighted-L1 with defaults `AllEligible`, `WeightedL1`, `PortableOnly`, and `RequireOptimal`.
**Notes:** `OptimalRepair`, `FeasibleRepair`, `NoRepairFound`, and `Unknown` remain distinct. Feasible-but-unproven is `Unknown` under `RequireOptimal`; operational errors never become mathematical outcomes.

## Provenance and P29 composition

| Option | Description | Selected |
|--------|-------------|----------|
| Exact all-or-error origin mapping | Convert only explicitly supported P29 origins and reject the entire conversion on an unsupported member | ✓ |
| Best-effort mapping | Drop unsupported members and continue with a partial repair scope | |
| Raw backend-ID mapping | Treat native row/column IDs as the ordinary public provenance | |

**Autonomous choice:** Exact semantic mapping with imported source provenance retained and exact instance/revision/`CompilationId` freshness checks before mutation.
**Notes:** IIS scopes/diagnoses; it does not prove minimum repair. Temporary locks, overlay-only fixings, grouped constructs, compiler-only members, and stale identities are explicit rejection cases.

## API and provider boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Frozen P30 boundary | Implement `None`/`Objective` penalties and portable repair; leave priority/objective policy to P31 | ✓ |
| Pull priority and lexicographic objectives into P30 | Close the whole solve-policy surface in one phase | |
| Require native qualification for completion | Make commercial/native capability a P30 prerequisite | |

**Autonomous choice:** Keep P31 ownership and portable completion independent from native availability. `PreferNative` is allowed only after exact equivalence; `NativeRequired` rejects before mutation when unqualified.
**Notes:** No generic unsupported-provider field, `ObjectivePolicy`, or `ObjectivePriority` is introduced by P30.

## the agent's Discretion

- Exact internal names, module placement, formulation representation, traversal ordering, test fixture structure, and error encoding remain flexible within the binding contracts.
- Report fields unsupported by a provider may be omitted at API freeze; fabricated values are not acceptable.

## Deferred Ideas

- Priority-target penalties, objective policy, and lexicographic execution — P31.
- Broader native relaxation, minimum-repair optimization, IIS enumeration, nonlinear repair, and extra adapter breadth — later work or outside M3.

---

*Phase: 30-soft-constraints*
*Discussion log generated: 2026-08-14*
