# ROML Release-Hardening State

## Current milestone

**Milestone:** crates.io production-readiness  
**Status:** planned; implementation not started  
**Current phase:** P0 — baseline, repository hygiene, and release controls  
**Program base:** `f9ba1921e650b5057bbc4de090a78391f7932a53`

## Planning branch

`docs/public-release-production-roadmap`

This branch contains planning/governance only. It is not an implementation branch and is not a release candidate.

## Accepted planning assumptions

- First release train prioritizes `roml` and `roml-highs`.
- `roml-mosek` and `roml-xpress` graduate independently and may remain unpublished/experimental.
- Recommended project license is `MIT OR Apache-2.0`; implementation must obtain owner confirmation before adding license files or changing package metadata.
- Core remains solver-free.
- HiGHS should use `rust-or/highs-sys` if required APIs are available or can be upstreamed.
- MOSEK should use the official `mosek` Rust crate/API.
- Xpress needs a separate binding/licensing investigation before selecting generated link-time bindings versus runtime loading.
- The current changelog API is not a compatibility constraint; correctness and multi-adapter recoverability take precedence.
- Language wrappers are post-v0.1.

## Open owner decisions

These are explicit gates, not requests for immediate clarification:

1. Confirm `MIT OR Apache-2.0` or choose another license before P0 metadata completion.
2. Confirm whether crates.io names `roml`, `roml-highs`, `roml-mosek`, and `roml-xpress` are owned/available before P6.
3. Select which commercial adapters, if any, are included in the first published release.
4. Approve use of protected self-hosted CI runners for licensed solver tests.
5. Approve publication only after P6 evidence review.

## Immediate next actions

1. Execute Phase 00 exactly as specified.
2. Capture baseline command outputs before cleanup.
3. Open separate implementation PR(s) from isolated branches/worktrees.
4. Keep this planning branch immutable except for reviewed roadmap amendments.

## Known P0/P1 findings already established

- Missing license files and incomplete crates.io metadata.
- No CI workflows.
- Root and solver crates mix core/runtime dependencies with logging configuration.
- Placeholder Python scaffold and a tracked solver log are present.
- README links to missing `MODELING_API.md`.
- Handwritten HiGHS, MOSEK, and Xpress ABI declarations.
- macOS-centric and incomplete native discovery/build scripts.
- Invalid MOSEK callback strategy mutates a task inside a callback contrary to official guidance.
- Duplicate parametric terms can map to the same matrix cell with last-write-wins behavior.
- Model changes are destructively drained before backend acknowledgement.
- Multiple adapters cannot independently synchronize from one model.
- Constructors and callback bridges contain panic/unchecked-FFI risks.

## Progress ledger

| Phase | Status | Evidence | Blocking decision |
|---|---|---|---|
| P0 | Not started | none | license confirmation before metadata merge |
| P1 | Not started | none | none |
| P2 | Not started | none | journal retention policy can be decided during design |
| P3 | Not started | none | Xpress binding/licensing decision |
| P4 | Not started | none | commercial CI runner approval |
| P5 | Not started | none | public support labels |
| P6 | Not started | none | explicit publish authorization |
| P7 | Deferred | none | post-v0.1 |

## State update protocol

After each phase:

- set status and completion commit;
- link the evidence report;
- record requirement IDs closed;
- record deviations and ADR amendments;
- identify the next unblocked phase;
- never mark a skipped mandatory check as passing.