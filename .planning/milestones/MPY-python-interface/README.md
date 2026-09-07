# MPY — Python Interface Ultra-Planning Packet

**Owner instruction, 2026-09-07:** prepare the implementation packet; the coding agent must first check and merge pending PRs, then implement a neat, intuitive, performant PyO3 + maturin interface for ROML, with Marginal MPC as the first consumer.

**Observed baseline:** `main@659c30c93fb5b2da056d6ecce245c977dbd7fd3e`. The only open PR observed before creating this packet was draft #49, `phase-roml-P31-lexicographic@e0a5efa736aa6dc46408204ae6fd2194e355dfab`. Refresh all repository facts at execution time. This packet will itself introduce a planning PR.

**Deliverable:** installable, typed Python wheels exposing the existing Rust model and persistent HiGHS session, with an ergonomic scalar/array API, atomic bulk updates, reliable outcomes, and measured repeated-solve behavior. No exported ROML C ABI is required.

## Read order

1. [STATE.md](STATE.md) — actual progress and next gate.
2. [PR-INTAKE.md](PR-INTAKE.md) — prerequisite review, correction, merge authority.
3. [DESIGN.md](DESIGN.md) — normative API, ownership, failure and concurrency contracts.
4. [REQUIREMENTS.md](REQUIREMENTS.md) — acceptance ledger.
5. [ROADMAP.md](ROADMAP.md) — dependencies, work limits, stop conditions.
6. [IMPLEMENTATION-PLAN.md](IMPLEMENTATION-PLAN.md) — tasks, files, executable test seeds.
7. [QUALIFICATION.md](QUALIFICATION.md) — correctness, performance and wheel gates.
8. [AGENT-PROMPT.md](AGENT-PROMPT.md) — self-contained kickoff instructions.
9. [PLANNING-REVIEW.md](PLANNING-REVIEW.md) — document checks and independent review disposition.

## Authority and scope

This is the owner-requested successor to M3 qualification, ahead of the deferred M4 quadratic/nonlinear preview. The old public-release project's wrapper non-goal applied to that release train; it does not prohibit this new milestone. Its suggestion of a C ABI for future bindings is superseded for Python by direct PyO3 bindings. Core model semantics, identities, solver independence, native safety, and ordinary branch protections remain binding.

The owner has authorized the executor to inspect, remediate and normally merge relevant pending PRs after review and checks. This includes the planning PR containing this packet. It does not authorize blind merge, admin bypass, weakened CI, unrelated PR work, commercial-backend expansion, live market actions, or registry publication. MPY implementation PRs are to be left reviewable unless the owner separately authorizes their merge; prerequisite P31 and P34 closure merges are authorized by this packet's completion sequence.

No runtime implementation is delivered by this planning PR. The API examples are acceptance targets, not claims of existing functions. Keep evidence files under this milestone's `evidence/` directory as execution produces them; do not manufacture completed artifacts.

## Decisions fixed by this packet

- Direct PyO3 extension `roml._native`, packaged by maturin; import namespace `roml`.
- Python 3.13 and 3.14 standard CPython initially; Linux x86_64, macOS arm64, Windows x86_64 wheels. No free-threaded support claim in this milestone.
- Rust core remains Python-free and solver-free. HiGHS is the initial Python backend.
- Scalar and dense-shaped handle arrays share Rust expression/model semantics; bulk data crosses the boundary in batches.
- One implementation phase active at a time; independent review capacity is reserved before starting the next phase.
- Prerequisite order: current PR intake and P31 closure → existing P34 closure → MPY implementation → installed-wheel MPC qualification.
- Marginal domain models, settlement, forecasting and market authority remain outside ROML.
