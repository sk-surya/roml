---
gsd_state_version: 1.0
milestone: MIR
milestone_name: Shared Modeling IR and Block-Native Core
status: in_progress
stopped_at: MIR-02 remediated after owner review; tranche 1 awaiting independent re-review
last_updated: "2026-09-11T00:00:00Z"
current_phase: MIR-02
current_phase_name: remediation pending independent re-review
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

Tranche 1 (MIR-00/01/02) is implemented, MIR-02 was remediated after owner
review on 2026-09-11, and the direct persistent-HiGHS solve gate passes. The
tranche is **not accepted**: PR #58 must be independently re-reviewed before any
MIR-03 work. MIR-03 (shared `roml::modeling` strided array IR, sink-aware
`try_param_block_layout`, mixed CSR builder) is not started.

Remediation evidence is in
`.planning/milestones/MIR-modeling-ir/evidence/MIR-02-REPORT.md` (remediation
section), with tests `tests/mir02_remediation.rs`,
`tests/mir02_backend_batching.rs`, and
`roml-highs/tests/mir02_bess_batching.rs`.

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
