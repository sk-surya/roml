# M3 Completion Ultra Packet — Index

## Current gate

```text
PR #45 written-spec review
  -> independent acceptance + owner merge
  -> P36 production implementation becomes active
  -> P36 merge
  -> P30 merge
  -> P31 merge
  -> P34 merge / M3 closure
  -> M4 design gate only
```

**Planned routing target:** P36.  
**Active production implementation:** none while PR #45 is unmerged.

## Authority order

1. `.planning/STATE.md` — root routing + explicit implementation authorization.
2. `STATE.md` in this milestone — detailed milestone routing/ledger.
3. `SHARED-CONTRACTS.md` — binding cross-phase identity, transaction, parameter, naming, objective-lock, and ownership semantics.
4. `COMPLETION-REQUIREMENTS.md` — MPS-W01–W14 + M3-C01–C05.
5. `COMPLETION-ROADMAP.md` — current execution sequence and gates.
6. Original `REQUIREMENTS.md` / `DECISIONS.md` — stable M3 semantic requirement/decision history.
7. Phase-specific frozen contracts/plans.

If a phase plan contradicts a higher authority, execution stops for written-spec amendment.

## Packet files

### Program design and governance

- `docs/superpowers/specs/2026-08-09-m3-completion-and-m4-foundation-design.md`
- `.planning/ROADMAP.md`
- `.planning/STATE.md`
- `COMPLETION-ROADMAP.md`
- `COMPLETION-PACKET.md`
- `COMPLETION-REQUIREMENTS.md`
- `SHARED-CONTRACTS.md`
- `COMPLETION-SELF-REVIEW.md`
- `COMPLETION-REVIEW-ROUND-2.md`

### P36 — deterministic MPS write-back

- `.planning/phases/36-mps-writeback/36-CONTRACT.md`
  - semantic-only public writer;
  - frozen defaults/report/error taxonomy;
  - complete representability matrix;
  - evaluated-parameter snapshot semantics;
  - deterministic export-local naming;
  - cross-platform path transaction + fault seam;
  - independent oracle/mismatch/status/tolerance rules.
- `.planning/phases/36-mps-writeback/36-NETLIB-MANIFEST.md`
  - exact pinned 94-file qualification inventory;
  - missing file/corpus drift/writer rejection are failures.
- `.planning/phases/36-mps-writeback/36-PLAN.md`
  - Wave 0 serial contract freeze;
  - Wave 1 parallel projection/formatter/path with disjoint ownership;
  - Wave 2 parallel bounds/objective/ROML oracle/HiGHS oracle;
  - Wave 3 serial 94-model qualification;
  - Wave 4 docs/package/review/exact-head closure.

### P30 — soft constraints / feasibility relaxation

- `.planning/phases/30-soft-constraints/30-PLAN.md`
  - persistent softening versus solve-scoped repair;
  - weighted-L1 normative portable provider;
  - P30-specific native-provider policy;
  - explicit OptimalRepair/FeasibleRepair/NoRepairFound/Unknown/operational-error distinction;
  - exact P29 supported-origin map with all-or-error rejection;
  - parameter weight evaluation before mutation;
  - primary + cleanup/rebuild error preservation.

### P31 — objective policies / lexicographic solve

- `.planning/phases/31-lexicographic-objectives/31-PLAN.md`
  - sole canonical `ObjectivePolicy` owner;
  - one shared `ObjectivePriority`;
  - final weighted-level/stage/lock/result schemas;
  - frozen `scale = |z*|` degradation formula;
  - portable sequential reference executor;
  - P30 parameterized penalty resolution before priority execution;
  - native provider only on exact semantic qualification.

### P34 — M3 final qualification

- `.planning/phases/34-m3-qualification/34-QUALIFICATION-CONTRACT.md`
  - every leaf SM-xx.y/MPS-Wxx/M3-Cxx ledger row;
  - executable fault-injection matrix;
  - Q01–Q14 native/portable fixture corpus;
  - Reference/HiGHS version + OS matrix;
  - frozen numeric/discrepancy rules;
  - `P34_PRIMITIVE_PARAMETER_UPDATE_V1` fixture and historical baseline;
  - exact packed-consumer protocol;
  - N1–N4 quadratic/NLP readiness shapes + per-component verdicts;
  - positive M3 closure predicate.
- `.planning/phases/34-m3-qualification/34-PLAN.md` — execution of that contract.

### M4 preview

- `.planning/milestones/M4-quadratic-nonlinear-foundation/PROJECT.md`
  - preview/design questions only;
  - no production authorization until P34 closes M3.

## Stable requirement numbering

P36 uses **MPS-W01 through MPS-W14** everywhere. No abbreviated W01–W08 roadmap subset is an alternate requirement scheme.

P30 retains SM-10 ownership with one explicit split: P30 closes `None`/`Objective` penalty targets, P31 adds and qualifies the shared priority target. P31 owns SM-11 and canonical objective policy. P34 owns SM-15 and complete leaf-level closure evidence.

## Acceptance rule for this planning PR

PR #45 is ready to merge only after independent re-review finds zero unresolved P0/P1 written-spec blockers. Merging the packet changes routing authority and permits **only P36** production execution; it does not start code automatically and does not authorize P30/P31/P34/M4 production work.
