# Phase 15 CONTEXT: Commercial backend tracks

**Date:** 2026-07-19
**Phase:** 15 (M1R-06/07) — MOSEK + Xpress independent qualification
**Goal:** qualify MOSEK and Xpress without contaminating or blocking the main release — non-blocking side track
**Requirements:** M1R-M1–M2, M1R-X1–X2, M1R-MX3

## Domain

Commercial adapter maintenance. Both crates have E0432 errors from Phase 10 contract changes (SolverAdapter/SolverStatus/SolveOptions removed). Fix compilation, maintain `publish = false`, document external-blocked gates.

## Decisions

### D1: Fix E0432 structurally
Replace `SolverAdapter` trait implementation with `BackendSession` implementation in both crates. Keep native call logic intact. This is a structural migration, not a rewrite.

### D2: publish = false maintained
Both crates already have `publish = false`. Do not change.

### D3: External gates documented
MOSEK needs official `mosek` Rust crate investigation. Xpress needs header redistribution legal decision. ADR-002 documents both gates.

### D4: Tests ignored with reason
Full solve tests require solver licenses. Mark as `#[ignore = "requires MOSEK/Xpress license"]`.

## Canonical refs
- `.planning/ROADMAP.md` — M1R-06/07 sections
- `.planning/REQUIREMENTS.md` — M1R-M1–M2, M1R-X1–X2, M1R-MX3
- `.planning/phases/15-commercial-backend-tracks-mosek-xpress/phase.md`

## Locked requirements
| ID | Description |
|---|---|
| M1R-M1 | MOSEK uses official Rust API, never mutates task illegally in callbacks |
| M1R-M2 | MOSEK compile/load/license/solve failures distinct, protected CI before support claims |
| M1R-X1 | Xpress binding redistribution/legal decision recorded |
| M1R-X2 | Xpress lifecycle and bulk/scalar equivalence pass common contract |
| M1R-MX3 | Commercial crates remain publish = false and non-blocking |

## Deferred ideas
- Full contract/differential/fault suite for commercial backends — post-v0.1
- Protected CI (license-restricted runners) — post-v0.1
- MOSEK official crate migration — post-v0.1
- Xpress binding decision — post-v0.1
