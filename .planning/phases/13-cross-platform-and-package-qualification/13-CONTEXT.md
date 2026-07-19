# Phase 13 CONTEXT: Cross-platform and package qualification

**Date:** 2026-07-19
**Phase:** 13 (M1R-04) — Cross-platform and package qualification
**Goal:** prove that supported users can build, load, package, and consume ROML + HiGHS without maintainer-machine assumptions
**Requirements:** M1R-P1–P6

## Domain

Infrastructure/DevOps phase. Create CI workflows, verify cross-platform builds, set up package hygiene checks, and run packed-consumer verification. Runs locally first; CI is setup for automated future runs.

## Decisions

### D1: Minimal CI matrix
Linux (ubuntu-latest), macOS (macos-latest arm64), Windows (windows-latest). MSRV smoke on ubuntu-latest. This mirrors the ROADMAP matrix.

### D2: HiGHS bundled build
Default `bundled` feature builds from source via highs-sys CMake. System discovery via `system` feature (Linux-only for CI, apt-based).

### D3: Core and HiGHS lanes
Separate CI workflows: solver-free core (fast, no deps) and HiGHS backend (requires C++ compiler, cmake). This keeps the core CI fast.

### D4: Package hygiene
cargo deny, cargo audit, cargo machete, cargo semver-checks. deny.toml from existing worktree.

### D5: No macOS x86_64
macos-13 runner not required for M1R. arm64 (macos-latest) suffices.

## Canonical refs
- `.planning/ROADMAP.md` — M1R-04 section
- `.planning/REQUIREMENTS.md` — M1R-P1–P6
- `.planning/phases/13-cross-platform-and-package-qualification/phase.md`

## Locked requirements
| ID | Description |
|---|---|
| M1R-P1 | Core and HiGHS matrices cover Linux, macOS, Windows, stable, MSRV |
| M1R-P2 | Bundled/static default and explicit system-discovery mode |
| M1R-P3 | Packed .crate archives build in fresh consumers |
| M1R-P4 | docs.rs topology does not require commercial SDKs |
| M1R-P5 | fmt, clippy, tests, rustdoc, semver, audit, deny, machete pass |
| M1R-P6 | Sanitizer/fuzz checks have defined cadence |

## Deferred ideas
- macOS x86_64 runner — post-M1R
- Windows HiGHS system discovery — post-M1R (bundled default works)
- Miri/fuzz scheduled runs — post-M1R, Phase 13 provides the workflow skeleton
