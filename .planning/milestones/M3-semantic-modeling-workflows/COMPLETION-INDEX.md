# M3 Completion Ultra Packet — Index

## Objective

Close ROML M3 through one bounded execution sequence:

```text
P36 MPS write-back
 -> P30 soft constraints / feasibility relaxation
 -> P31 objective policies / lexicographic solve
 -> P34 integrated qualification / M3 closure
 -> M4 quadratic/nonlinear foundation design gate
```

## Packet files

### Program design

- `docs/superpowers/specs/2026-08-09-m3-completion-and-m4-foundation-design.md`
  - architectural rationale;
  - binding invariants;
  - phase-level designs;
  - M4 extension direction;
  - explicit non-goals.

### GSD routing

- `.planning/STATE.md`
  - active phase is P36;
  - P35 is recorded complete/merged;
  - one-active-phase rule;
  - P30/P31/P34 are planned but inactive.
- `.planning/milestones/M3-semantic-modeling-workflows/COMPLETION-ROADMAP.md`
  - NOW/NEXT/LATER;
  - dependency graph;
  - phase gates;
  - deferred backlog;
  - stop conditions.
- `.planning/milestones/M3-semantic-modeling-workflows/COMPLETION-PACKET.md`
  - program invariants;
  - status state machine;
  - review/evidence policy.
- `.planning/milestones/M3-semantic-modeling-workflows/COMPLETION-REQUIREMENTS.md`
  - MPS-W01–W14;
  - M3-C01–C05 execution requirements.

### Executable phase plans

- `.planning/phases/36-mps-writeback/36-PLAN.md`
  - active implementation plan;
  - deterministic free MPS;
  - transaction-safe path writer;
  - semantic/native/solve round trips;
  - full Netlib transcode gate.
- `.planning/phases/30-soft-constraints/30-PLAN.md`
  - persistent soft constraints;
  - exact violation algebra;
  - parameter-aware penalties;
  - solve-scoped portable feasibility relaxation;
  - P29 IIS composition.
- `.planning/phases/31-lexicographic-objectives/31-PLAN.md`
  - canonical objective policies;
  - portable staged executor;
  - exact degradation locks;
  - P30 penalty priority integration;
  - native qualification policy.
- `.planning/phases/34-m3-qualification/34-PLAN.md`
  - integrated workflows;
  - failure/recovery matrix;
  - performance/API/package/fresh-consumer gates;
  - principal OR/backend/NLP-readiness reviews;
  - milestone closure.

### Next milestone preview

- `.planning/milestones/M4-quadratic-nonlinear-foundation/PROJECT.md`
  - quadratic semantic IR hypothesis;
  - QP/QCQP sequence;
  - nonlinear function/evaluation design questions;
  - global/local diagnostic semantics;
  - no M4 implementation authorization.

## Binding decisions

1. P36 is the only active next implementation phase.
2. P30 and P31 preserve their original numbering and SM-10/SM-11 contracts.
3. P34 is a qualification phase, not a feature bucket.
4. P36 writer emits deterministic free MPS; P35 remains fixed+free reader.
5. Mathematical round-trip equivalence is required; source layout/bytes are not preserved.
6. Persistent softening and solve-scoped relaxation are distinct public concepts.
7. Weighted-L1 is the normative first portable feasibility-relaxation objective.
8. Portable lexicographic execution is normative; native execution is optional qualified acceleration.
9. Objective degradation locks must be correct across objective sense and positive/zero/negative optimum values.
10. P34 must prove M4 extension seams concretely before quadratic/nonlinear implementation begins.
11. No new file formats, release/publication, or broad solver-adapter work enters the critical path.
12. Only one implementation phase is active because semantic review/integration capacity is the bottleneck.

## Acceptance flow for this packet

```text
written packet
 -> independent architecture/spec review
 -> owner approval
 -> merge planning PR
 -> create isolated P36 implementation branch/worktree
 -> execute 36-PLAN.md
```

This packet does not itself authorize publication or M4 production code.