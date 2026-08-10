# M3 Semantic Modeling and Solve Workflows — State

## Routing state

**Milestone:** M3 — Semantic Modeling and Solve Workflows  
**Status:** completion planning under review  
**Planned routing target:** P36 — MPS write-back  
**Active production implementation:** **none** until PR #45 is accepted and merged  
**Implementation authorization:** false  
**Current authoritative main for this planning branch:** `main@4467797f002c93a1baab638b5e65976fb8492505`  
**Completion roadmap:** `COMPLETION-ROADMAP.md`  
**Shared contracts:** `SHARED-CONTRACTS.md`

`Planned routing target` means the phase GSD should activate next after its planning gate. It does **not** mean a production branch may exist yet.

## Accepted phases

| Phase | Capability | State | Evidence / authority |
|---|---|---|---|
| P25 | semantic IR / identity | complete | accepted P25 evidence |
| P26 | compiler/backend IR | complete | accepted P26 evidence |
| P27 | fixing / assignments / locks / overlays | complete | accepted P27 evidence |
| P28 | SolvePlan / starts / hints | complete | accepted P28 evidence |
| P29 | IIS/conflict analysis | complete | PR #39 + `P29_IIS_QUALIFICATION.md` |
| P30 | soft constraints / feasibility relaxation | planned, inactive | SM-10 + `30-PLAN.md`; gated by P36 merge |
| P31 | objective policies / lexicographic execution | planned, inactive | SM-11 + `31-PLAN.md`; gated by P30 merge |
| P32 | common semantic constructs | complete | accepted P32 evidence |
| P33 | PWL / bound analysis | complete | accepted P33 evidence |
| P34 | integrated M3 qualification | planned, inactive | SM-15 + P34 qualification contract; gated by P31 merge |
| P35 | MPS import / corpus qualification | complete | PR #44 merge `7159fad8830b32f5a9377174e6e57bb24f99de95` |
| P36 | MPS write-back / round trip | planning review | gated by acceptance + merge of PR #45 |

## Binding completion sequence

```text
PR #45 accepted + merged
  -> P36 implementation active
  -> P36 accepted + merged
  -> P30 implementation active
  -> P30 accepted + merged
  -> P31 implementation active
  -> P31 accepted + merged
  -> P34 qualification active
  -> P34 accepted + merged
  -> M3 complete
  -> M4 design gate only
```

P36 is an explicit **program dependency** for P30. The older statement that P30 was already the next coding phase is superseded by this completion-program routing decision.

## Shared contract freeze

P36/P30/P31/P34 must use `SHARED-CONTRACTS.md` for:
- lifecycle/equality of `ModelLineageId`, `ModelInstanceId`, `ModelRevision`, and `CompilationId`;
- overlay apply/rollback, dirty session/rebuild, and multi-error preservation;
- parameterized MPS export as one evaluated snapshot;
- deterministic export naming independent of slot/debug IDs;
- objective degradation lock formula including zero/negative optima;
- P31 sole ownership of `ObjectivePolicy` and `ObjectivePriority`.

No phase plan may silently redefine those semantics.

## Phase-specific hard gates

### P36

Binding artifacts:
- `.planning/phases/36-mps-writeback/36-CONTRACT.md`
- `.planning/phases/36-mps-writeback/36-NETLIB-MANIFEST.md`
- `.planning/phases/36-mps-writeback/36-PLAN.md`

MPS-W01–W14 are all binding. Frozen Netlib qualification is 94 exact files; a missing file or writer rejection is a failure, not a skip.

### P30

Portable weighted-L1 is normative. Provider choice, mathematical outcome, and operational failure are distinct. P29 IIS composition is all-or-error over explicitly supported original origins.

### P31

P31 owns canonical `ObjectivePolicy` and `ObjectivePriority`; objective locks use `scale = |z*|`. P31 adds the actual priority-target penalty integration only when it executes.

### P34

`34-QUALIFICATION-CONTRACT.md` is the final closure authority: leaf requirement ledger, executable fault matrix, native/portable corpus, performance baseline, packed consumers, concrete NLP readiness, positive closure predicate.

## WIP policy

- One production phase active at a time.
- A planning/review branch may coexist with one production phase only when it does not authorize or accumulate future production code.
- While PR #45 is unmerged, **zero** P36/P30/P31/P34 production phases are active.
- P34 starts only after all feature phases are merged and cross-phase contracts are stable.

## Evidence/update protocol

For each phase:
1. record exact base/head SHAs;
2. map leaf requirement IDs to tests/evidence;
3. record exact local/hosted commands and backend/version/OS scope;
4. record public API changes, residuals, and review dispositions;
5. require exact-head independent review and hosted mandatory CI;
6. owner-authorized merge;
7. only then update root/milestone state to authorize the next phase.

## M3 stop condition

M3 ends only when P34's positive closure predicate passes and P34 is owner-merged. No publication/tag/release is implied. Quadratic/nonlinear production remains blocked until the separate M4 design gate.
