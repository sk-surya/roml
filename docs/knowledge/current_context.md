# Current Context

**Last updated:** 2026-07-13

## Active Milestone

v0.1 — ROML Public-Release Hardening

## Status

All P0-P5 phases complete. P6 release qualification ready pending owner authorization.
P7 (language ABI) deferred post-v0.1.

## Key Decisions

- Canonical coefficient cells: algebraic combination, not last-write-wins
- Revision protocol: ModelRevision → DeltaBatch → Journal → AdapterCursor
- HiGHS: rust-or/highs-sys when API coverage confirmed
- MOSEK: official mosek Rust API
- Xpress: binding decision blocked on legal verification
- License: MIT OR Apache-2.0 (pending owner confirmation)
