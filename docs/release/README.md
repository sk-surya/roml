# ROML Public-Release Program

This directory indexes the production-readiness program for ROML. The authoritative implementation baseline is `main@82e2ed95545635b628187ba0081fe8c8b03eaafb`.

## Start here

- [`../../.planning/PROJECT.md`](../../.planning/PROJECT.md) — product charter and release scope.
- [`../../.planning/REQUIREMENTS.md`](../../.planning/REQUIREMENTS.md) — stable requirement IDs.
- [`../../.planning/ROADMAP.md`](../../.planning/ROADMAP.md) — mega roadmap and phase gates.
- [`../../.planning/STATE.md`](../../.planning/STATE.md) — current milestone state and owner decisions.
- [`PRINCIPAL_ENGINEERING_AUDIT.md`](PRINCIPAL_ENGINEERING_AUDIT.md) — historical baseline audit at `f9ba192`.
- [`CURRENT_MAIN_DELTA_AUDIT.md`](CURRENT_MAIN_DELTA_AUDIT.md) — authoritative reconciliation of current `main`, including solve options, semi-continuous domains, Xpress batching, and new tracked artifacts.
- [`ARCHITECTURE_DECISIONS.md`](ARCHITECTURE_DECISIONS.md) — accepted implementation decisions.
- [`../superpowers/specs/2026-07-13-public-release-hardening-design.md`](../superpowers/specs/2026-07-13-public-release-hardening-design.md) — target system design.
- [`CODING_AGENT_PROMPT.md`](CODING_AGENT_PROMPT.md) — implementation-agent kickoff prompt.

## Executable plans

1. [`Phase 00 — release baseline`](../superpowers/plans/2026-07-13-phase-00-release-baseline.md)
2. [`Phase 01 — core correctness`](../superpowers/plans/2026-07-13-phase-01-core-correctness.md)
3. [`Phase 02 — revisioned synchronization`](../superpowers/plans/2026-07-13-phase-02-revisioned-sync.md)
4. [`Phase 03 — solver boundaries`](../superpowers/plans/2026-07-13-phase-03-solver-boundaries.md)
5. [`Phase 04 — cross-platform CI`](../superpowers/plans/2026-07-13-phase-04-cross-platform-ci.md)
6. [`Phase 05 — public API and packaging`](../superpowers/plans/2026-07-13-phase-05-public-api-packaging.md)
7. [`Phase 06 — release qualification`](../superpowers/plans/2026-07-13-phase-06-release-qualification.md)

## Governing rule

The repository is not ready for publication at the planning baseline. No plan document authorizes publishing, tagging, or releasing. Those actions require Phase 06 evidence and explicit owner authorization for the exact commit and crate list.