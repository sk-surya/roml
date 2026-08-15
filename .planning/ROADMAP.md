# ROML Active Roadmap — M3 Completion

This root file is the concise GSD routing projection. Historical milestone design remains under `.planning/milestones/`; it is not duplicated here.

## Current routing gate

```text
PR #45 merged (`48fab4db`)
  -> P36 merged via PR #46 (`8838effe`)
  -> P30 soft constraints / feasibility relaxation
  -> P31 objective policies / lexicographic solves
  -> P34 integrated M3 qualification / NLP readiness
  -> M3 complete
  -> M4 design gate only
```

**Current routing target:** P30 — soft constraints and feasibility relaxation.
**Active production implementation:** P30 is authorized. P36 merged via PR #46 as `8838effee84eafdcbc2e502fb417df8d09221248` after exact-head qualification and review.
**Hard rule:** `current_phase`/roadmap position is not authorization to create production code. `.planning/STATE.md` carries the explicit authorization flag.

P36 was an owner-selected **program dependency** of P30. That dependency is now satisfied, so solve-semantics work may resume at P30.

## Phase status

| Phase | Capability | Status | Next gate |
|---|---|---|---|
| P25 | semantic IR / identity | complete | none |
| P26 | compiler/backend IR | complete | none |
| P27 | fixing / locks / overlays | complete | none |
| P28 | SolvePlan / starts / hints | complete | none |
| P29 | IIS / conflict analysis | complete | follow-up native/perf work is nonblocking |
| P30 | soft constraints / relaxation | remediation-review-pending | remediation evidence/review on final head; owner merge |
| P31 | objective policies / lexicographic | planned, inactive | P30 merge |
| P32 | semantic constructs | complete | none |
| P33 | PWL / bound analysis | complete | none |
| P34 | M3 final qualification | planned, inactive | P31 merge |
| P35 | MPS import / corpus qualification | complete | residual broad Chinneck work is nonblocking |
| P36 | deterministic MPS write-back | complete | PR #46 merged; exact-head evidence retained |

## Binding authorities

- Root execution state: `.planning/STATE.md`
- Original M3 requirements: `.planning/milestones/M3-semantic-modeling-workflows/REQUIREMENTS.md`
- Original M3 architecture decisions: `.planning/milestones/M3-semantic-modeling-workflows/DECISIONS.md`
- Current completion routing: `.planning/milestones/M3-semantic-modeling-workflows/COMPLETION-ROADMAP.md`
- Completion requirements: `.planning/milestones/M3-semantic-modeling-workflows/COMPLETION-REQUIREMENTS.md`
- Shared identity/transaction/objective contracts: `.planning/milestones/M3-semantic-modeling-workflows/SHARED-CONTRACTS.md`
- Detailed milestone state: `.planning/milestones/M3-semantic-modeling-workflows/STATE.md`

## P36 — MPS write-back

**Goal:** export the evaluated active primitive LP/MILP mathematical model as deterministic free MPS and prove independent ROML/native-HiGHS equivalence.

**Binding requirements:** MPS-W01–MPS-W14.

**Frozen artifacts:**

- `.planning/phases/36-mps-writeback/36-CONTRACT.md`
- `.planning/phases/36-mps-writeback/36-NETLIB-MANIFEST.md`
- `.planning/phases/36-mps-writeback/36-PLAN.md`

**Gate to P30:** all MPS-W01–W14 evidenced; exact pinned 94-model manifest has 94/94 writer/determinism/ROML-structure/HiGHS-structure PASS; required solve comparisons, CI, review, and owner merge pass.

## P30 — soft constraints and feasibility relaxation

**Goal:** persistent semantic softening plus a distinct portable weighted-L1 solve-scoped repair workflow that composes honestly with P29.

**Ownership:** SM-10.1–SM-10.5, SM-10.7–SM-10.9 and the `None`/`Objective` portion of SM-10.6. P31 closes priority targeting.

**Gate to P31:** portable algebra/outcomes/provenance, P29 supported-origin mapping, parameterized weights, fault cleanup/error composition, exact-head qualification/review/merge.

**Plans:** 4 plans

Plans:

- [x] 30-01-PLAN.md — freeze the persistent soft-constraint API and canonical lifecycle
- [x] 30-02-PLAN.md — compile exact algebra, penalties, origins, and violation accessors
- [x] 30-03-PLAN.md — execute portable weighted-L1 repair with outcome and cleanup semantics
- [x] 30-04-PLAN.md — compose P29 origins and complete qualification/evidence handoff

## P31 — objective policies and lexicographic solves

**Goal:** one canonical `ObjectivePolicy`, one `ObjectivePriority`, deterministic portable weighted/lexicographic execution, and exact degradation locks.

**Ownership:** SM-11.1–SM-11.8, SM-07.7 objective-stage closure, P27 lock debt, and priority-target SM-10.6.

**Gate to P34:** independent reference equivalence, `|z*|` lock formula across positive/zero/negative optima, P30 priority integration, cleanup/fault evidence, exact-head qualification/review/merge.

## P34 — M3 final qualification

**Goal:** prove the milestone as one coherent system; no adjacent feature expansion.

**Frozen closure protocol:** `.planning/phases/34-m3-qualification/34-QUALIFICATION-CONTRACT.md`.

P34 requires a leaf-level requirement ledger, executable fault matrix, frozen native/portable corpus and backend/OS/version matrix, reproducible performance fixture/baseline, packed-consumer protocol, concrete quadratic/NLP readiness verdicts, and the complete positive closure predicate.

## M4 preview

Quadratic/nonlinear production implementation is forbidden before P34 closes. M4 begins with a fresh design gate and must extend rather than replace the M3 identity, provenance, compiler/backend IR, SolvePlan, objective, and reporting seams.

## Deferred, noncritical work

Unless a correctness/user blocker emerges, keep these outside M3's critical path:

- fixed-format MPS writer;
- compiled-formulation MPS export;
- LP/JSON/SMPS format breadth;
- broad system-native IIS expansion;
- minimum-cardinality IIS;
- residual `IC-satimage-LB` investigation;
- additional solver-adapter breadth;
- release/publication work without separate owner authorization.
