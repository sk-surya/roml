# M3 Semantic Modeling and Solve Workflows — State

## Routing state

**Milestone:** M3 — Semantic Modeling and Solve Workflows  
**Status:** P30 active
**Current routing target:** P30 — soft constraints and feasibility relaxation
**Active production implementation:** P30 is authorized; P36 is complete and merged via PR #46 as `8838effee84eafdcbc2e502fb417df8d09221248`.
**Implementation authorization:** true for P30 only; P31 and P34 remain unauthorized.
**Planning prerequisite:** PR #45 merged to `main` as `48fab4db347522cebc786393e5afcbdbcea98f33`.
**Completion roadmap:** `COMPLETION-ROADMAP.md`  
**Shared contracts:** `SHARED-CONTRACTS.md`

`Current routing target` identifies the phase at the active merge/authorization gate. It does **not** authorize a successor phase before its predecessor actually merges.

## Accepted phases

| Phase | Capability | State | Evidence / authority |
|---|---|---|---|
| P25 | semantic IR / identity | complete | accepted P25 evidence |
| P26 | compiler/backend IR | complete | accepted P26 evidence |
| P27 | fixing / assignments / locks / overlays | complete | accepted P27 evidence |
| P28 | SolvePlan / starts / hints | complete | accepted P28 evidence |
| P29 | IIS/conflict analysis | complete | PR #39 + `P29_IIS_QUALIFICATION.md` |
| P30 | soft constraints / feasibility relaxation | active | SM-10 + `30-PLAN.md`; P36 merge gate satisfied |
| P31 | objective policies / lexicographic execution | planned, inactive | SM-11 + `31-PLAN.md`; gated by P30 merge |
| P32 | common semantic constructs | complete | accepted P32 evidence |
| P33 | PWL / bound analysis | complete | accepted P33 evidence |
| P34 | integrated M3 qualification | planned, inactive | SM-15 + P34 qualification contract; gated by P31 merge |
| P35 | MPS import / corpus qualification | complete | PR #44 merge `7159fad8830b32f5a9377174e6e57bb24f99de95` |
| P36 | MPS write-back / round trip | complete | PR #46 merged as `8838effe`; exact-head closure evidence |

## Binding completion sequence

```text
PR #45 accepted + merged (`48fab4db`)
  -> P36 merged via PR #46 (`8838effe`)
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
- P30 is active after the P36 merge; P31 and P34 remain inactive until their explicit predecessor merge gates are satisfied.
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
8. Activate exactly one successor after the predecessor implementation PR is actually merged; acceptance alone does not activate a successor.

## M3 stop condition

M3 ends only when P34's positive closure predicate passes and P34 is owner-merged. No publication/tag/release is implied. Quadratic/nonlinear production remains blocked until the separate M4 design gate.
