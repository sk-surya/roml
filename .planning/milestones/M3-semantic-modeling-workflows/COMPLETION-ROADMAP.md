# M3 Completion Execution Roadmap

**Authority:** current post-P35 execution sequence. The original M3 roadmap remains historical architecture/requirement context; this file controls routing.

## Routing state

**Planned target:** P36.  
**Production implementation active now:** none while PR #45 is under written-spec review.  
**Activation:** accepted + merged PR #45 authorizes an isolated P36 implementation branch/worktree.

```text
PR #45 merge
  -> P36 MPS write-back merge
  -> P30 soft constraints / relaxation merge
  -> P31 objective policies / lexicographic merge
  -> P34 final qualification merge
  -> M3 COMPLETE
  -> M4 DESIGN gate only
```

P36 is an explicit program dependency for P30. The completion program deliberately closes the MPS read/write/differential loop before resuming solve-semantics implementation.

## Status ledger

| Phase | Capability | Status | Production authorization | Gate |
|---|---|---|---:|---|
| P25 | semantic IR / identity | complete | no new work | accepted evidence |
| P26 | compiler/backend IR | complete | no new work | accepted evidence |
| P27 | fixing/locks/overlays | complete | no new work | accepted evidence |
| P28 | SolvePlan/starts/hints | complete | no new work | accepted evidence |
| P29 | IIS/conflict analysis | complete | no new work | PR #39/P29 evidence |
| P30 | soft constraints/relaxation | planned | false | P36 accepted + merged |
| P31 | objective policies | planned | false | P30 accepted + merged |
| P32 | common constructs | complete | no new work | accepted evidence |
| P33 | PWL/bounds | complete | no new work | accepted evidence |
| P34 | M3 qualification | planned | false | P31 accepted + merged |
| P35 | MPS import | complete | no new work | PR #44/P35 evidence |
| P36 | MPS write-back | planning review | false until PR #45 merges | written-spec clearance |

## P36 requirement ownership — MPS-W01 through MPS-W14

There is exactly one P36 requirement namespace:

- **MPS-W01** public solver-free write seam;
- **MPS-W02** deterministic canonical free-MPS bytes;
- **MPS-W03** typed representability/no semantic weakening;
- **MPS-W04** objective sense/coefficient/offset semantics;
- **MPS-W05** row/RHS/RANGES semantics;
- **MPS-W06** continuous/integer/binary/domain semantics;
- **MPS-W07** numeric safety;
- **MPS-W08** transactional path semantics;
- **MPS-W09** independent ROML mathematical round trip;
- **MPS-W10** native HiGHS full structural differential;
- **MPS-W11** normalized solve/status/objective differential;
- **MPS-W12** exact pinned 94-model Netlib transcode gate;
- **MPS-W13** explicit source-layout non-goal;
- **MPS-W14** package/core-solver-separation/exact-head CI qualification.

The detailed text in `COMPLETION-REQUIREMENTS.md` is authoritative. No W01–W08 shorthand is an alternate scheme.

### P36 gate

P36 must satisfy `36-CONTRACT.md`, `36-NETLIB-MANIFEST.md`, and all MPS-W01–W14. The exact pinned manifest must produce 94/94 deterministic writer + independent ROML structure + native HiGHS structure PASS. Missing corpus/file, manifest drift, or writer rejection is failure, not a classified skip. Required solve subset obeys frozen status/objective tolerances/dispositions. Exact-head mandatory CI, independent review, and owner merge are required before P30 activates.

## P30 requirement ownership

P30 owns SM-10 persistent soft constraints and weighted-L1 feasibility relaxation, except the priority-target execution portion of SM-10.6, which P31 closes when it introduces the shared `ObjectivePriority` and actually executes priority penalties.

### P30 gate

- exact upper/lower/equality/ranged algebra and caps;
- stable violation identities/origins;
- finite parameterized penalty evaluation before mutation;
- portable weighted-L1 provider and explicit provider policy;
- separate `OptimalRepair`, `FeasibleRepair`, `NoRepairFound`, `Unknown`, operational-error semantics;
- exact P29 supported-origin mapping; unsupported origins reject all-or-error;
- rollback/rebuild fault matrix preserves primary + cleanup errors;
- exact-head mandatory CI/review + owner merge.

## P31 requirement ownership

P31 is the sole owner of canonical `ObjectivePolicy` and the one shared `ObjectivePriority`. It owns SM-11, SM-07.7 stage-result closure, P27 objective-lock debt, and the priority-target portion of SM-10.6.

### P31 gate

- canonical single/weighted/lexicographic policies;
- deterministic portable sequential executor;
- objective degradation `scale = |z*|` for min/max, positive/zero/negative optima;
- final weighted-level/stage/lock/result schemas;
- parameterized P30 penalty weights resolved before priority stage execution;
- native path only if semantically equivalent;
- no leaked stage artifacts; multi-error cleanup preserved;
- exact-head mandatory CI/review + owner merge.

## P34 requirement ownership

P34 owns SM-15 and the affirmative closure evidence for **every** leaf SM-xx.y, MPS-Wxx, and M3-Cxx.

### P34 gate

`34-QUALIFICATION-CONTRACT.md` is binding. It requires:
- one leaf ledger row per requirement with evidence/review/residual/final state;
- executable fault-injection matrix;
- frozen Q01–Q14 reference/native corpus;
- ReferenceBackend + bundled HiGHS 1.15.0 OS matrix + system HiGHS 1.9.0 compatibility floor;
- frozen structural/objective tolerances and discrepancy dispositions;
- reproducible `P34_PRIMITIVE_PARAMETER_UPDATE_V1` performance comparison to `main@4d111cce...`;
- exact packed-consumer/package protocol;
- N1–N4 quadratic/NLP shapes with per-component readiness verdicts;
- complete positive closure predicate, exact-head review/CI, owner merge.

## Shared contract authority

`SHARED-CONTRACTS.md` freezes for all remaining phases:
- lineage/instance/revision/CompilationId lifecycle and equality;
- solve-scoped apply/rollback/dirty-session/rebuild/error composition;
- parameterized MPS evaluated-snapshot semantics;
- deterministic export naming independent of raw slot/debug IDs;
- objective-lock scale/formula;
- P31 sole `ObjectivePolicy` ownership and shared `ObjectivePriority`.

Any phase contradiction stops execution for reviewed amendment.

## Review/execution protocol

```text
written contract/spec
 -> independent review
 -> owner merge of planning authority
 -> isolated production branch/worktree
 -> TDD task/wave reviews
 -> qualification evidence
 -> independent full PR review
 -> exact-head hosted mandatory CI
 -> owner merge
 -> state update authorizes exactly one successor
```

## Deferred backlog after M3

Keep outside the critical path unless a correctness/user blocker emerges:
- fixed-format MPS writer;
- compiled-formulation MPS export;
- LP/JSON/SMPS format breadth;
- system-native IIS expansion/minimum-cardinality IIS;
- residual `IC-satimage-LB` investigation;
- additional solver adapters merely for breadth;
- release/publication without separate owner authorization;
- any quadratic/nonlinear production work before P34 closure.

## Program stop conditions

Stop before proceeding when a shared contract would change, a portable/native path cannot state exact guarantees, a solve-scoped artifact cannot prove cleanup/recovery, an exact identity/provenance boundary is lost, the frozen P36 corpus cannot pass as declared, P34 has an unpassed leaf/fault/consumer/performance/readiness gate, or future-phase production work starts ahead of its explicit merge authorization.
