# M3 Completion Ultra Packet

## Program state

M3 has a mature LP/MILP foundation. P25–P29, P32–P33, and P35 are merged/accepted. The remaining completion path is deliberately serialized:

```text
PLANNED ROUTING TARGET: P36 MPS write-back
THEN:                   P30 soft constraints / feasibility relaxation
THEN:                   P31 objective policies / lexicographic solve
FINAL:                  P34 integrated qualification / M3 closure
LATER:                  M4 quadratic/nonlinear foundation design gate
```

**No P36/P30/P31/P34 production implementation is active while PR #45 is unmerged.** Merging an accepted PR #45 authorizes only P36 to begin on an isolated production branch/worktree.

## Program dependency

P36 is an explicit owner-selected program gate for P30:

```text
PR #45 merge -> P36 merge -> P30 merge -> P31 merge -> P34 merge
```

This supersedes older routing text that treated P30 as immediately executable after P28. The reason is deliberate sequencing: close and qualify MPS read/write interchange first, then return to infeasibility repair and multiobjective solve semantics.

## Binding authorities

- routing/authorization: `.planning/STATE.md` and milestone `STATE.md`;
- cross-phase semantics: `SHARED-CONTRACTS.md`;
- completion requirements: `COMPLETION-REQUIREMENTS.md`;
- execution order/gates: `COMPLETION-ROADMAP.md`;
- original stable M3 semantic contracts: `REQUIREMENTS.md`, `DECISIONS.md`;
- P36: `36-CONTRACT.md`, `36-NETLIB-MANIFEST.md`, `36-PLAN.md`;
- P30/P31: their phase plans plus shared contracts;
- P34: `34-QUALIFICATION-CONTRACT.md` + `34-PLAN.md`.

A lower-level artifact cannot weaken a higher-level guarantee silently.

## Shared contract freeze

The completion program now freezes rather than restates these semantics independently:

1. exact roles/lifecycle/equality for lineage, model instance, revision, and CompilationId;
2. overlay apply/rollback, dirty-session/rebuild, and preservation of primary + cleanup failures;
3. parameterized MPS export as one evaluated exact model revision/environment;
4. deterministic export-local names independent of arena/debug IDs;
5. objective degradation lock scale `|z*|` across positive/zero/negative optima;
6. P31 as sole canonical `ObjectivePolicy` owner and sole shared `ObjectivePriority` owner.

## P36 completion gate

P36 public surface is **semantic MPS export only**. A compiled-formulation export is not a decorative P36 option; it is deferred to a future independent design.

Required:
- frozen writer defaults/report/error taxonomy;
- complete representability matrix;
- evaluated-parameter report metadata;
- deterministic free-MPS bytes;
- transactional Linux/macOS/Windows path commit contract + fault seam;
- independent test-local ROML mathematical oracle;
- native HiGHS full-structure + normalized solve oracle;
- exact pinned 94-file manifest with 94/94 required PASS and zero writer rejections;
- all MPS-W01–W14 evidenced;
- exact-head mandatory CI/review and owner merge.

## P30 completion gate

Required:
- persistent soft constraints with exact two-sided algebra/provenance;
- parameter-aware finite penalty evaluation before backend mutation;
- P30-specific `RelaxationProviderPolicy`, not generic SolvePlan unsupported policy;
- portable weighted-L1 normative provider;
- separate `OptimalRepair`, `FeasibleRepair`, `NoRepairFound`, `Unknown`, and operational-error semantics with numerical metadata;
- exact P29 IIS supported-origin mapping and all-or-error rejection of unsupported origins;
- primary + rollback/rebuild failure preservation;
- exact-head qualification/review and owner merge.

P30 does not ship a decorative priority target. P31 adds the shared priority variant when it actually executes.

## P31 completion gate

Required:
- one canonical `ObjectivePolicy` and one `ObjectivePriority`;
- final weighted-level, stage-result, lock-report, and aggregate-result contracts;
- explicit sense normalization;
- portable sequential semantic reference executor;
- frozen `scale=|z*|` degradation locks;
- parameterized P30 penalties resolved before priority execution;
- native provider only on exact semantic equivalence;
- no stage-artifact leak and multi-error cleanup preservation;
- exact-head qualification/review and owner merge.

## P34 final closure gate

P34 is qualification, not a feature bucket. It must execute `34-QUALIFICATION-CONTRACT.md`:
- leaf requirement ledger for every SM-xx.y, MPS-Wxx, M3-Cxx;
- executable fault matrix;
- exact Q01–Q14 fixture corpus;
- backend/version/OS matrix and frozen comparison rules;
- reproducible primitive performance fixture/baseline;
- exact packed consumer/package protocol;
- concrete N1–N4 quadratic/NLP readiness shapes and per-component verdicts;
- positive closure predicate where every mandatory conjunction passes;
- zero unresolved P0/P1, exact-head CI, owner merge.

## WIP policy

- Before PR #45 merges: zero remaining production phases active.
- After PR #45 merges: exactly P36 may be active.
- After each phase merges: root/milestone state explicitly authorizes exactly one successor.
- Review/research ahead is allowed; future production code is not.
- No M4 production implementation begins before P34 closes M3.

## Evidence policy

Each phase evidence records:
- exact base/head SHA;
- leaf requirement IDs;
- actual APIs and public diff;
- commands/results + backend/version/OS scope;
- independent/reference/native/corpus outcomes;
- failure/recovery evidence;
- review IDs/dispositions;
- explicit residual risks/non-goals;
- exact next gate.

Test counts alone never establish acceptance.

## State machine

```text
planned_routing_target
 -> implementation_authorized
 -> implementation_in_progress
 -> qualification_in_progress
 -> pending_independent_review
 -> pending_merge
 -> complete
```

The first transition requires the planning/predecessor merge gate. A roadmap phase label alone never authorizes code.
