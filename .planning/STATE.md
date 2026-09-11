---
gsd_state_version: 1.0
milestone: MIR
milestone_name: Shared Modeling IR and Block-Native Core
status: in_progress
stopped_at: MIR-01 complete (IR-02..IR-07 evidenced); MIR-02 next
last_updated: "2026-09-11T00:00:00Z"
current_phase: MIR-02
current_phase_name: parametric packed construction and block-native propagation
implementation_authorized: true
---

# ROML Active State — MIR

## Current routing

**Current target:** MIR — shared modeling IR and block-native core.

Predecessor state:
- M3 semantic modeling/solve workflows: complete.
- MPY Python interface: implementation merged via PR #53.
- Post-MPY performance/certification stack: merged via PR #55, with post-merge closeout PR #56 on current `main@c590692ace5446cc20c7eb91cb8fa0d594a054b0`.

The owner has authorized MIR ahead of further Python modeling ergonomics and the deferred M4 preview. Detailed MIR state lives at `.planning/milestones/MIR-modeling-ir/STATE.md`.

## Authorization

Tranche 1 — MIR-00, MIR-01, MIR-02 — is authorized. One implementation phase at a time. Runtime implementation PRs remain reviewable unless separately authorized to merge. Publication, tags and releases remain separate owner gates.

## Next gate

MIR-02: packed parametric rows/objectives, validated L2 `ParamDepLayout`
witnesses, block-native transactional propagation, and self-contained packed
coefficient-patch deltas (IR-08…IR-17).

MIR-00 is complete (`evidence/BASELINE.md`: 28,800 price parameters driving
57,600 objective cells; `parametric_bulk=1`, `general_affine=0`, per-cell
propagation and 86,400 delta ops per reprice). MIR-01 is complete
(`evidence/MIR-01-REPORT.md`: opaque trusted spans, block allocation, and the
packed variable-block Change/ModelOp; IR-02…IR-07 evidenced).

## Binding authorities

- Root routing: `.planning/STATE.md`, `.planning/ROADMAP.md`
- MIR packet: `.planning/milestones/MIR-modeling-ir/README.md`
- MIR architecture: `docs/release/ARCHITECTURE_DECISIONS.md` D-019
- MIR invariants skill: `.planning/milestones/MIR-modeling-ir/skills/roml-ir-invariants/SKILL.md`
- Historical MPY packet: `.planning/milestones/MPY-python-interface/`

## WIP rules

- One MIR implementation phase at a time.
- Baseline/characterization before storage redesign.
- Correctness, stale-ID, ownership, transaction, snapshot and revision semantics outrank performance.
- No admin merge bypass.
- A skipped mandatory check never counts as pass.
- M4 production and publication remain unauthorized by this routing update.
