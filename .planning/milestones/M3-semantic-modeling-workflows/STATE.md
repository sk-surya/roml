# M3 Semantic Modeling and Solve Workflows State

## Current state

**Milestone:** M3 — Semantic Modeling and Solve Workflows  
**Status:** planned; implementation not started  
**Current phase:** P25 — semantic IR, lineage, metadata  
**Planning branch:** `docs/m3-semantic-modeling-workflows`  
**Authoritative baseline:** `main@d1f1ad38cec75abb671729df8efb87736861628c`  
**Design:** `docs/superpowers/specs/2026-08-02-semantic-modeling-and-solve-workflows-design.md`

## Approved owner direction

- Preserve high-level semantic constructs in canonical model state.
- Design abstractions so later nonlinear programming extends rather than replaces the architecture.
- Implement MILP workflows first; do not implement NLP in M3.
- Use bound tightening as the default representation of variable fixing.
- Proceed with programming design and an implementation-ready planning packet.

## Immediate next gate

P25 may begin only after review of this planning branch and confirmation that the design/requirements/roadmap are internally consistent. Implementation starts from current reviewed `main`, not from an outdated planning SHA.

Before P25 code:

1. fetch current refs and record exact base SHA;
2. read `AGENTS.md`, the M3 packet, and the approved design;
3. create an isolated worktree/branch `phase-roml-P25-semantic-ir-foundation`;
4. capture untouched baseline commands and public API;
5. write characterization/failing tests first.

## Phase ledger

| Phase | Status | Prerequisite | Evidence | Blocking decision |
|---|---|---|---|---|
| P25 Semantic IR/identity | Planned | M3 plan approval | none | none |
| P26 Compiler/backend IR | Blocked | P25 accepted | none | backend contract amendment review |
| P27 Fixing/locks/overlays | Blocked | P26 accepted | none | none |
| P28 Starts/hints/SolvePlan | Blocked | P27 accepted | none | pinned HiGHS start API audit |
| P29 IIS/conflicts | Blocked | P28 accepted | none | pinned/system HiGHS IIS capability |
| P30 Soft constraints | Blocked | P28 accepted | none | none |
| P31 Lexicographic objectives | Blocked | P28 accepted | none | native HiGHS semantics audit |
| P32 Common constructs | Blocked | P26 accepted | none | none |
| P33 PWL/bounds | Blocked | P32 accepted | none | none |
| P34 Qualification | Blocked | P29–P33 accepted | none | no publication in M3 |

## Bounded technical decisions to resolve during execution

These are not open architecture questions; each has a prescribed evidence gate.

1. **HiGHS IIS support:** inspect pinned bundled and minimum system official headers. Implement official API where present; otherwise qualify version-gated unsupported behavior or upgrade through a separate reviewed dependency change.
2. **HiGHS native multiobjective:** compare official priority/tolerance semantics to ROML's policy. Use native execution only on an exact match; otherwise use sequential overlay execution.
3. **HiGHS variable hints:** if no official hint capability exists, report unsupported. Do not simulate hints silently.
4. **Backend IR native PWL:** expose the normalized primitive only after at least one backend implementation and a portable fallback exist.
5. **Metadata source capture:** M3 stores user-supplied source metadata. Automatic procedural-macro source capture remains deferred unless it can be additive and optional.

## WIP policy

- Default: one coding phase active.
- Maximum: one coding phase plus one review/fix branch.
- P29/P30/P31 parallelism is allowed only after P28, with separate owners and an integration reviewer.
- P32 may overlap only when P26's compiler contract is frozen and unchanged.
- P34 begins only after all feature branches are merged and no unresolved cross-phase contract changes remain.

## Evidence protocol

Each phase evidence file must record:

- base and head SHAs;
- requirement IDs closed;
- exact commands and results;
- public API changes;
- backend/version/feature matrix;
- skipped checks with reasons;
- deviations from design;
- reviewer findings and dispositions;
- residual risks;
- next gate.

## State update protocol

After each phase:

1. update this ledger with verified facts only;
2. link the phase evidence and merge commit;
3. update `TRACEABILITY.md` requirement status;
4. record any accepted design amendment in `DECISIONS.md`;
5. identify exactly one next coding phase;
6. never mark a skipped mandatory check as passing.

## Milestone stopping condition

Stop M3 execution after P34 evidence and reviews. Do not publish, tag, or release. Publication requires a separate explicit owner decision against an exact SHA and crate list.
