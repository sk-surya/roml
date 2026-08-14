---
gsd_state_version: 1.0
milestone: M3
milestone_name: Semantic Modeling and Solve Workflows
current_phase: 36
current_phase_name: MPS write-back and round-trip qualification
status: pending_merge
implementation_authorized: true
---

# ROML Active State — M3 Completion

This file is the root GSD routing authority. Detailed contracts live under `.planning/milestones/M3-semantic-modeling-workflows/`.

## Routing versus execution

**Current routing target:** P36 — deterministic MPS write-back and round-trip qualification, implementation accepted and pending merge in PR #46.
**Active production implementation:** P36 is complete on the exact candidate head; only closure/merge administration remains. No further Rust, solver, test, dependency, or implementation work is authorized for P36.
**Planning prerequisite:** completion-planning PR #45 merged to `main` as `48fab4db347522cebc786393e5afcbdbcea98f33`.
**Later production phases:** P30, P31, and P34 remain inactive until their explicit predecessor merges; in particular, P30 remains gated until P36/PR #46 actually merges.

A `current_phase` value identifies the next GSD routing target; it is **not** implementation authorization by itself. The current authorization is limited to P36 closure/merge administration; P30, P31, and P34 remain unauthorized.

## Accepted state

- P25–P29: complete/accepted.
- P32–P33: complete/accepted.
- P35 MPS import: complete, merged via PR #44 as `7159fad8830b32f5a9377174e6e57bb24f99de95`.
- P29 design record: merged via PR #38 as `4467797f002c93a1baab638b5e65976fb8492505`.
- P30/P31/P34: planned only.
- P36: implementation accepted, pending merge via PR #46 at exact head `86ee71c685435510be8598337e7ecf8da20b1efd`; required closure artifacts are present in this docs-only follow-up.
- M4 quadratic/nonlinear foundation: preview/design gate only; no production implementation authorization.

## Binding completion sequence

```text
PR #45 written-spec acceptance + merge (`48fab4db`)
  -> P36 implementation accepted / pending merge (PR #46)
  -> P36 actually merged
  -> activate P30
  -> P30 accepted + merged
  -> activate P31
  -> P31 accepted + merged
  -> activate P34
  -> P34 accepted + merged
  -> M3 complete
  -> M4 design gate only
```

P36 is an explicit **program gate** for P30 even though P30's mathematical prerequisites existed earlier. The owner-selected completion sequence is binding for execution.

## Authority map

- Program routing: `.planning/milestones/M3-semantic-modeling-workflows/COMPLETION-ROADMAP.md`
- Shared cross-phase semantics: `.planning/milestones/M3-semantic-modeling-workflows/SHARED-CONTRACTS.md`
- Completion requirements: `.planning/milestones/M3-semantic-modeling-workflows/COMPLETION-REQUIREMENTS.md`
- Historical/original M3 requirements: `.planning/milestones/M3-semantic-modeling-workflows/REQUIREMENTS.md`
- P36 frozen writer contract: `.planning/phases/36-mps-writeback/36-CONTRACT.md`
- P36 exact corpus manifest: `.planning/phases/36-mps-writeback/36-NETLIB-MANIFEST.md`
- P36 execution plan: `.planning/phases/36-mps-writeback/36-PLAN.md`
- P30 plan: `.planning/phases/30-soft-constraints/30-PLAN.md`
- P31 plan: `.planning/phases/31-lexicographic-objectives/31-PLAN.md`
- P34 final qualification contract/plan: `.planning/phases/34-m3-qualification/34-QUALIFICATION-CONTRACT.md`, `34-PLAN.md`

## Phase ledger

| Phase | Status | Production authorization | Gate |
|---|---|---:|---|
| P25 | complete | no new work | accepted evidence |
| P26 | complete | no new work | accepted evidence |
| P27 | complete | no new work | accepted evidence |
| P28 | complete | no new work | accepted evidence |
| P29 | complete | no new work | PR #39 / P29 evidence |
| P30 | planned | **false** | P36 accepted + merged |
| P31 | planned | **false** | P30 accepted + merged |
| P32 | complete | no new work | accepted evidence |
| P33 | complete | no new work | accepted evidence |
| P34 | planned | **false** | P31 accepted + merged |
| P35 | complete | no new work | PR #44 / P35 evidence |
| P36 | implementation accepted / pending merge | **true (closure only)** | PR #46 exact-head evidence, independent review, hosted CI, and owner-authorized merge |

## WIP and update rules

- One production phase at a time.
- Research/review may be prepared ahead; production code may not.
- A phase becomes implementation-active only after its predecessor/packet merge gate is satisfied and this state file is updated on `main`. P36 has passed that prerequisite; its implementation is accepted and awaiting merge.
- Completion requires exact-head evidence, independent review, hosted mandatory CI, and owner-authorized merge.
- P30 remains unauthorized until the P36 merge is complete and this state is updated again.
- A skipped mandatory check never counts as pass.
- Publication/tag/release remain separate owner gates and are not implied by M3 completion.
