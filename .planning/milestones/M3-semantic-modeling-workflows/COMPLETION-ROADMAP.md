# M3 Completion Execution Roadmap

**Authority:** this roadmap governs the post-P35 completion sequence. The original M3 roadmap remains the historical phase contract. Where execution order differs, this file is the current owner-approved routing decision.

## NOW / NEXT / LATER

### NOW — P36 MPS write-back

**State:** planned; next implementation phase.

**Why now:** P35 established a trustworthy external-model ingestion and differential harness. Write-back closes that loop and creates a strong commuting-square correctness oracle before work returns to solve semantics.

**End state:** deterministic free MPS output for supported linear LP/MILP, ROML semantic round trips, native HiGHS structure/solve equivalence, Netlib transcode qualification.

**Plan:** `.planning/phases/36-mps-writeback/36-PLAN.md`

### NEXT — P30 soft constraints / feasibility relaxation

**State:** planned, inactive until P36 accepted.

**Why:** converts P29's “why infeasible?” into “what controlled violation repairs feasibility?” while maintaining semantic/provenance boundaries.

**End state:** persistent soft constraints + separate solve-scoped weighted-L1 feasibility relaxation + P29 composition.

**Plan:** `.planning/phases/30-soft-constraints/30-PLAN.md`

### NEXT — P31 objective policies / lexicographic solve

**State:** planned, inactive until P30 accepted.

**Why:** closes the real operational objective workflow and activates soft-penalty priority semantics without solver-specific behavior.

**End state:** canonical weighted/lexicographic policy, portable sequential executor, exact objective locks, stage results, P30 priority integration.

**Plan:** `.planning/phases/31-lexicographic-objectives/31-PLAN.md`

### NEXT — P34 M3 closure

**State:** planned, inactive until P31 accepted.

**Why:** M3 must stop accumulating features and be qualified as one system.

**End state:** all SM requirements evidenced, integrated workflows, performance/API/package/fresh-consumer qualification, NLP-readiness review, M3 closed.

**Plan:** `.planning/phases/34-m3-qualification/34-PLAN.md`

### LATER — M4 quadratic/nonlinear foundation

**State:** preview only; no implementation branch until P34 closes.

**Why:** nonlinear support changes mathematical/function/backend semantics and deserves a separate milestone.

**Preview:** `.planning/milestones/M4-quadratic-nonlinear-foundation/PROJECT.md`

---

## Status ledger

| Phase | Capability | Status | Merge/evidence authority | Remaining gate |
|---|---|---|---|---|
| P25 | semantic IR / identity | complete | accepted phase evidence | none |
| P26 | compiler/backend IR | complete | accepted phase evidence | none |
| P27 | fixing/locks/overlays | complete | accepted phase evidence | none |
| P28 | SolvePlan/starts/hints | complete | accepted phase evidence | none |
| P29 | IIS/conflict analysis | complete | PR #39 + P29 evidence; design record #38 | follow-up system-native/perf not M3 blocker |
| P30 | soft constraints/relaxation | planned | this completion packet + SM-10 | P36 merge then implementation |
| P31 | objective policies | planned | this completion packet + SM-11 | P30 merge then implementation |
| P32 | common constructs | complete | accepted phase evidence | none |
| P33 | PWL/bounds | complete | accepted phase evidence | none |
| P34 | M3 qualification | planned | this completion packet + SM-15 | P31 merge then implementation |
| P35 | MPS import | complete | PR #44 / `7159fad...` + P35 evidence | residual satimage qualification is follow-up |
| P36 | MPS write-back | **NOW** | this completion packet | design/plan review -> implementation |

## Critical path

```text
P36 -> P30 -> P31 -> P34 -> M3 COMPLETE -> M4 DESIGN
```

No parallel implementation is authorized on this path. Research notes and reviews may be prepared ahead, but no production branch for a later phase should accumulate code.

## Requirement ownership

### P36

P36 is an additive post-original-M3 phase. Its requirements are program-specific:

- MPS-W01 deterministic semantic export;
- MPS-W02 typed representability;
- MPS-W03 exact objective/range/bound/domain semantics;
- MPS-W04 semantic round-trip equivalence;
- MPS-W05 native HiGHS structure/solve differential;
- MPS-W06 Netlib transcode qualification;
- MPS-W07 transactional path output / typed diagnostics;
- MPS-W08 no source-layout preservation claim.

### P30

Owns SM-10.1–SM-10.9 and closes related provenance/overlay evidence.

### P31

Owns SM-11.1–SM-11.8, SM-07.7 objective-stage closure, and P27 real objective-lock optimum/tolerance debt.

### P34

Owns SM-15.1–SM-15.8 plus all residual integration evidence not already fully established in earlier phases.

## Review gates

Every phase follows:

```text
context/spec approval
  -> executable plan
  -> isolated implementation branch/worktree
  -> TDD slices with task reviews
  -> qualification evidence
  -> independent full PR review
  -> exact-head hosted CI
  -> owner merge
  -> routing update
```

A phase cannot become `complete` merely because implementation tests pass locally.

## Branch conventions

Recommended:

```text
phase-roml-P36-mps-writeback
phase-roml-P30-soft-constraints
phase-roml-P31-lexicographic-objectives
phase-roml-P34-m3-qualification
```

Plans/specs may use `docs/...` branches, but production changes stay on the phase branch.

## Completion metrics that matter

Prefer evidence that closes invariants:

- semantic equivalence, not parser/writer line counts;
- reference/native differential outcomes, not raw test count;
- rollback/rebuild correctness, not happy-path success;
- exact origin mapping, not presence of metadata;
- guarantees actually enforced by execution, not public configuration types;
- fresh-consumer usability, not internal examples only;
- measured performance, not complexity claims alone.

## Deferred backlog after M3

Keep these out of the critical path unless a correctness/user blocker emerges:

- fixed-format MPS writer;
- LP file format;
- JSON/model serialization;
- system-native IIS expansion;
- minimum-cardinality IIS;
- additional solver adapters/features;
- release-grade broad IIS performance campaign;
- `IC-satimage-LB.mps` P29 `Unknown(Unclassified)` investigation;
- quadratic/nonlinear implementation before M4.

## Program stop conditions

Escalate before proceeding when:

- a phase requires changing an accepted earlier semantic contract;
- a native backend cannot express the declared semantics and no exact portable path exists;
- an option/report field does not govern or describe actual execution;
- test evidence depends on one solver being treated as the format/semantic authority;
- a temporary solve artifact cannot be proven removed or recovered;
- exact identity/provenance is lost;
- review capacity becomes the bottleneck and parallel work is accumulating unreviewed;
- implementation begins to expand M4 before P34 closure.