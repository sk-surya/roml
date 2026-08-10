# M3 Completion Ultra Packet

## Program state

M3 has a mature LP/MILP foundation. P25-P29, P32-P33, and pulled-forward P35 are accepted/merged. Remaining execution is intentionally narrowed to four phases:

```text
ACTIVE: P36 MPS write-back
NEXT:   P30 soft constraints / feasibility relaxation
NEXT:   P31 objective policies / lexicographic solve
FINAL:  P34 integrated qualification / M3 closure
LATER:  M4 quadratic + nonlinear foundation design
```

## Primary objective

Finish a coherent optimization modeling system with closed import/export, diagnosis/repair, and multiobjective workflows before opening nonlinear scope.

## Dependency DAG

```text
P25 Semantic IR ─> P26 Compiler/Backend IR
                         |
              +----------+-----------+
              |                      |
              v                      v
          P27 overlays            P32 constructs
              |                      |
              v                      v
          P28 SolvePlan            P33 PWL
              |
       +------+------+-------------------+
       |             |                   |
       v             v                   |
     P29 IIS        P35 MPS read          |
       |             |                   |
       |             v                   |
       |           P36 MPS write          |
       |             |                   |
       +-----------> P30 relaxation       |
                     |                    |
                     v                    |
                   P31 objectives <-------+
                     |
                     v
                   P34 closure
                     |
                     v
              M4 design gate only
```

P36 does not technically gate P30, but it executes first because it closes the current I/O thread and strengthens differential evidence. P30 then closes the infeasibility repair loop. P31 follows because P30 penalties compose with lexicographic priorities. P34 integrates everything.

## WIP policy

- Exactly one implementation phase is active.
- Planning/research for the following phase is allowed, production implementation is not.
- Every implementation phase gets an isolated branch/worktree, TDD task slices, independent review, fresh exact-head CI, evidence, and owner merge gate.
- No feature is merged because it is architecturally interesting; it must close a listed acceptance criterion.

## Phase gates

### Gate P36

Required before P30 starts:

- deterministic free-MPS writer public API;
- typed representability errors;
- semantic `Model -> MPS -> ROML` equivalence;
- `Model -> MPS -> native HiGHS readModel` structural equivalence;
- bounded native/ROML solve equivalence;
- Netlib transcode qualification for all P35-supported files;
- cross-platform core/HiGHS/MSRV/quality/coverage/policy green;
- writer package/public API docs;
- no P0/P1 review finding.

### Gate P30

Required before P31 starts:

- persistent soft-constraint construct with correct upper/lower/equality/ranged algebra;
- stable violation identities and complete origins;
- parameter-aware finite penalty semantics;
- solution violation accessors in original terms;
- separate solve-scoped portable feasibility relaxation;
- transactional rollback and failure injection;
- P29 IIS-to-relaxation composition without minimum-repair overclaim;
- qualified native path only if exact semantics are established;
- no P0/P1 review finding.

### Gate P31

Required before P34 starts:

- canonical single/weighted/lexicographic policies;
- deterministic portable sequential executor;
- objective locks correct for min/max and positive/zero/negative optima;
- explicit stage continuation policy;
- all stage/all-objective results recorded;
- P30 violation-priority integration;
- native path only when semantically equivalent;
- no leaked stage artifacts under failure injection;
- no P0/P1 review finding.

### Gate P34

Required to close M3:

- every mandatory SM requirement mapped to tests/evidence;
- integrated workflows across import/export, IIS/relaxation, overlays, starts, constructs/PWL, and lexicographic solves;
- cross-platform/version matrix green;
- public API/package/fresh-consumer review passes;
- performance gates pass or owner-approved profiled exception;
- NLP-readiness review explicitly certifies extension seams;
- no P0/P1 findings;
- milestone state set complete;
- no publication implied.

## Program decisions

### C1 — P36 before P30

Close MPS read/write now. Reason: the interchange commuting square becomes an independent correctness oracle for later features and avoids reopening I/O while P30/P31 are active.

### C2 — writer is semantic, not source-preserving

Round-trip target is mathematical equivalence. Original comments, fixed/free layout, duplicate-record spelling, and ordering are not preserved.

### C3 — free MPS only for P36 output

Fixed MPS output is not required. P35 continues reading both.

### C4 — standard linear LP/MILP output only

Unsupported high-level semantics fail explicitly unless an advanced compiled-formulation export is deliberately selected.

### C5 — portable relaxation is ROML-owned

Solver-native feasibility relaxation is an optional qualified provider, analogous to P29 native IIS.

### C6 — softening and relaxation are distinct

Persistent soft constraints mutate canonical model state. Feasibility relaxation is solve-scoped analysis and must roll back.

### C7 — portable lexicographic execution is normative

Native multiobjective is optional and must prove semantic equivalence.

### C8 — M4 extends current seams

Quadratic/nonlinear support must extend `ScalarFunction`, backend IR, capabilities, origin mapping, solve plans, and reports rather than replace them.

## Review bottlenecks

The principal program risk is no longer implementation throughput; it is semantic review and integration. Therefore:

- no parallel P30/P31 implementation;
- every phase has explicit normalized/reference oracles;
- agent-generated APIs are rejected if configuration fields do not govern execution;
- reports cannot claim guarantees stronger than the evidence path;
- test counts are not acceptance evidence by themselves.

## Evidence strategy

Each phase creates `docs/release/evidence/PXX_*.md` containing:

- exact base/head SHA;
- requirement IDs;
- public API changes;
- test commands and results;
- differential corpus outcomes;
- backend/version matrix;
- performance measurements where applicable;
- independent review IDs/dispositions;
- explicit residuals/non-goals.

## Completion state machine

```text
planned
  -> design_reviewed
  -> implementation_in_progress
  -> qualification_in_progress
  -> pending_independent_review
  -> pending_merge
  -> complete
```

No phase skips independent review or exact-head CI.

## Immediate next gate

Execute P36 from its detailed plan. P30/P31/P34 packets are binding scope/architecture but remain inactive until their predecessor merges.