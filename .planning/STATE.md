---
gsd_state_version: 1.0
milestone: MIR
milestone_name: Shared Modeling IR and Block-Native Core
status: in_progress
stopped_at: MIR-00 complete (IR-01 evidenced); MIR-01 in progress (IR-02, IR-04 evidenced)
last_updated: "2026-09-11T00:00:00Z"
current_phase: MIR-01
current_phase_name: trusted block allocation
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

Finish MIR-01: the Model variable-block API with one packed
`Change::VariableBlockAdded` and one self-contained `ModelOp::AddVariableBlock`
(IR-03/IR-05/IR-06/IR-07 evidence), then MIR-02.

MIR-00 is complete: `evidence/BASELINE.md` records the measured current path
(28,800 price parameters driving 57,600 objective cells; `parametric_bulk=1`,
`general_affine=0`, `param_positions_cells=57,600`, 57,600 per-cell
reverse-index lookups and 86,400 delta ops per reprice). MIR-01 so far adds
opaque trusted spans (`src/bulk.rs`), arena/store block allocation, and
`Model::add_parameter_block` with unchanged scalar creation semantics.

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
