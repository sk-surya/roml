# ROML Ultra-Mega Planning Packet Index

**Canonical branch:** `planning/roml-ultra-mega-roadmap-v2`  
**Immediate milestone:** ROML-M1R  
**Strategic horizon:** ROML-M2 through ROML-M5

## Why this packet exists
The previous Phase 0/Phase 1 prompt is obsolete. Solver-free hardening was merged through PR #3, and an inherited candidate branch already contains substantial native-backend implementation. This packet resets state to evidence, closes the remaining architectural gap, and establishes the actual path to a public `roml` + `roml-highs` release.

## Governing documents
1. `docs/superpowers/specs/2026-07-17-roml-ultra-mega-program-design.md`
2. `.planning/ROADMAP.md`
3. `.planning/REQUIREMENTS-V2.md`
4. `.planning/STATE-V2.md`
5. `.planning/DECISIONS-V2.md`
6. `.planning/TRACEABILITY-V2.md`

## M1R phase packets
1. `.planning/phases/00-truth-reset-and-candidate-admission/phase.md`
2. `.planning/phases/01-backend-contract-migration-closure/phase.md`
3. `.planning/phases/02-highs-projection-session-rewrite/phase.md`
4. `.planning/phases/03-native-differential-fault-qualification/phase.md`
5. `.planning/phases/04-cross-platform-package-qualification/phase.md`
6. `.planning/phases/05-performance-ergonomics-acceptance/phase.md`
7. `.planning/phases/06-07-commercial-backend-tracks/phase.md`
8. `.planning/phases/08-09-release-and-operations/phase.md`

## Execution controls
- Master plan: `docs/superpowers/plans/2026-07-17-roml-m1r-execution-plan.md`
- Coordinator packet: `.planning/agents/ROML-M1R-COORDINATOR.md`
- Independent reviewer: `.planning/agents/ROML-M1R-INDEPENDENT-REVIEWER.md`
- Coding-agent launcher: `docs/release/ROML-M1R-CODING-AGENT-PROMPT.md`

## Strategic milestones
### ROML-M2 — Industrial Modeling Completeness
Named entities/metadata, sparse bulk construction, LP/MPS interchange, basis/warm-start abstraction, IIS/diagnostics, explicit SOS/indicator capabilities, model inspection, and versioned serialization identity.

### ROML-M3 — Persistent Incremental Runtime
Long-lived sessions, journal retention/checkpoints, crash-safe replay, shadow verification, structured cancellation/progress, repeated reoptimization, and large sparse performance envelopes.

### ROML-M4 — Language and Ecosystem Boundary
Stable C API, generated headers, Python first, explicit ownership/errors/version negotiation, and wrappers isolated from Rust internals.

### ROML-M5 — 1.0 Stability and Governance
Semver-stable surface, deprecation and compatibility policy, performance regression governance, security and release operations, maintainer ownership, and evidence-backed 1.0 claims.

## Start instruction
Give the coordinator `docs/release/ROML-M1R-CODING-AGENT-PROMPT.md`. The first execution is M1R-00. Do not rerun the old baseline phase and do not accept the inherited candidate wholesale.
