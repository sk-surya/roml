---
gsd_state_version: 1.0
milestone: M3
milestone_name: Semantic Modeling and Solve Workflows
current_phase: 36
current_phase_name: MPS write-back and round-trip qualification
status: planned
---

# ROML Active State — M3 Semantic Modeling and Solve Workflows

The detailed M3 ledger is retained at
`.planning/milestones/M3-semantic-modeling-workflows/STATE.md`. This root state
frontmatter is the active GSD routing authority.

## Active execution state

**Current phase:** P36 — deterministic MPS write-back and round-trip qualification.  
**Status:** planned; design/plan review is the next gate.  
**Implementation base:** current `main` after merged P35 PR #44 (`7159fad8830b32f5a9377174e6e57bb24f99de95`) and merged P29 architecture record PR #38 (`4467797f002c93a1baab638b5e65976fb8492505`).  
**Execution plan:** `.planning/phases/36-mps-writeback/36-PLAN.md`.  
**Program roadmap:** `.planning/milestones/M3-semantic-modeling-workflows/COMPLETION-ROADMAP.md`.  
**Next review gate:** review/approve this completion packet, then implement P36 from an isolated phase branch/worktree. P30/P31/P34 production implementation remains inactive.

## M3 completion sequence

```text
P36 MPS write-back
  -> P30 soft constraints / feasibility relaxation
  -> P31 objective policies / lexicographic solve
  -> P34 M3 integration / qualification / NLP-readiness
  -> M3 complete
  -> M4 quadratic/nonlinear foundation design gate
```

Accepted/merged prerequisites: P25–P29, P32–P33, P35. P30 and P31 retain their original phase numbers and requirement ownership; P36 is the pulled-forward I/O closure phase after P35.

## Current phase ledger

| Phase | Status | Evidence / authority | Blocking decision |
|---|---|---|---|
| P25 | Complete | accepted phase evidence | none |
| P26 | Complete | accepted phase evidence | none |
| P27 | Complete | accepted phase evidence | none |
| P28 | Complete | accepted phase evidence | none |
| P29 | Complete | `docs/release/evidence/P29_IIS_QUALIFICATION.md`, PR #39 (`19c8c70`), design record PR #38 | system-native IIS/performance remain follow-up, not M3 blockers |
| P30 | Planned | SM-10 + `30-PLAN.md` | P36 must merge first |
| P31 | Planned | SM-11 + `31-PLAN.md` | P30 must merge first |
| P32 | Complete | accepted phase evidence | none |
| P33 | Complete | accepted phase evidence | none |
| P34 | Planned | SM-15 + `34-PLAN.md` | P31 must merge first |
| P35 | Complete | `docs/release/evidence/P35_MPS_QUALIFICATION.md`, merged PR #44 (`7159fad`) | residual `IC-satimage-LB` qualification is follow-up |
| P36 | **Planned / active** | completion packet + `36-PLAN.md` | design/plan review then implementation |

## WIP rule

Only one P36/P30/P31/P34 implementation phase may be active. Research/review can be prepared ahead, but production code for the next phase does not accumulate before the current phase is accepted and merged.

## Archived v0.1 Release-Hardening State

The following section is historical release-roadmap state. It is not the active GSD execution route above.

### Historical milestone

**Milestone:** crates.io production-readiness  
**Status:** planned/previously executed in its own roadmap context  
**Authoritative implementation base:** `82e2ed95545635b628187ba0081fe8c8b03eaafb`  
**Historical audit base:** `f9ba1921e650b5057bbc4de090a78391f7932a53`

### Historical planning branch

`docs/public-release-production-roadmap`

Current `main` was reconciled into this branch through PR #2, merge commit `083cc6d890c59efab9da74c687031cb9ecf27d5b`. The branch contains planning/governance documents and is not an implementation branch or release candidate.

### Accepted planning assumptions

- First release train prioritizes `roml` and `roml-highs`.
- `roml-mosek` and `roml-xpress` graduate independently and may remain unpublished/experimental.
- Recommended project license is `MIT OR Apache-2.0`; implementation must obtain owner confirmation before adding license files or changing package metadata.
- Core remains solver-free.
- HiGHS should use maintained/generated official bindings where required APIs are available.
- MOSEK should use the official `mosek` Rust crate/API.
- Xpress needs a separate binding/licensing investigation before selecting generated link-time bindings versus runtime loading.
- Language wrappers are post-v0.1.

### Open owner decisions retained from release roadmap

1. Confirm `MIT OR Apache-2.0` or choose another license before publication metadata completion.
2. Confirm whether crates.io names `roml`, `roml-highs`, `roml-mosek`, and `roml-xpress` are owned/available before publication.
3. Select which commercial adapters, if any, are included in the first published release.
4. Approve use of protected self-hosted CI runners for licensed solver tests.
5. Approve publication only after the separate release evidence gate.

### Historical progress ledger

| Phase | Status | Evidence | Blocking decision |
|---|---|---|---|
| P0 | Complete | `docs/release/evidence/P0_BASELINE.md` (HEAD: c1fe456) | license confirmation before metadata merge |
| P1 | Complete | canonical cells, validation, invariant checker (HEAD: c1fe456) | none remaining |
| P2 | Complete | revision, snapshot, delta, journal, cursor, ref backend, atomic tx, sync characterization (HEAD: c1fe456) | journal retention policy decided |
| P3 | Complete | backend errors, SolveRequest/Result, capabilities, Xpress decision doc (HEAD: c1fe456) | Xpress binding/licensing deferred |
| P4 | Complete | core CI, policy CI, workspace lints | commercial CI runner approval future |
| P5 | Complete | examples, CHANGELOG, RELEASE_CHECKLIST, SUPPORT_MATRIX, PACKAGING.md | public support labels documented |
| P6 | Complete | release evidence, foundry verification, steward audit | explicit publish authorization external gate |
| P7 | Deferred | none | post-v0.1 |

### Completed historical requirement IDs

- R0 package metadata: P0
- R1 repository hygiene: P0
- R2 canonical semantics/cells/validation: P1
- R3 revisioned synchronization/journal/cursor/atomicity: P2
- R4 solver boundaries/backend errors/capabilities: P3
- R5 Xpress binding decision documented: P3
- R6 cross-platform CI design: P4
- R7.1–R7.3 CI lanes: P4
- R8.1 reference backend: P2
- R8.3 validation: P1
- R9.1–R9.4 public API/docs: P5
- R9.5–R9.6 publication controls: P0
- R10 language ABI: deferred P7

## State update protocol

After each active M3 phase:

- set status and completion commit;
- link the evidence report and independent review;
- record requirement IDs closed;
- record deviations/ADR amendments;
- identify exactly one next unblocked implementation phase;
- never mark a skipped mandatory check as passing;
- never treat a future milestone preview as active implementation authorization.