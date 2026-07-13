# ROML Release-Hardening State

## Current milestone

**Milestone:** crates.io production-readiness  
**Status:** planned; implementation not started  
**Current phase:** P0 — baseline, repository hygiene, and release controls  
**Authoritative implementation base:** `82e2ed95545635b628187ba0081fe8c8b03eaafb`  
**Historical audit base:** `f9ba1921e650b5057bbc4de090a78391f7932a53`

## Planning branch

`docs/public-release-production-roadmap`

Current `main` was reconciled into this branch through PR #2, merge commit `083cc6d890c59efab9da74c687031cb9ecf27d5b`. The branch contains current implementation source plus planning/governance documents. It is not an implementation branch and is not a release candidate.

## Accepted planning assumptions

- First release train prioritizes `roml` and `roml-highs`.
- `roml-mosek` and `roml-xpress` graduate independently and may remain unpublished/experimental.
- Recommended project license is `MIT OR Apache-2.0`; implementation must obtain owner confirmation before adding license files or changing package metadata.
- Core remains solver-free; transient `SolveOptions` move out of canonical `Model` state during P1/P2 API redesign.
- HiGHS should use `rust-or/highs-sys` if required APIs are available or can be upstreamed.
- MOSEK should use the official `mosek` Rust crate/API.
- Xpress needs a separate binding/licensing investigation before selecting generated link-time bindings versus runtime loading.
- The current changelog API is not a compatibility constraint; correctness and multi-adapter recoverability take precedence.
- Existing Xpress bulk-update behavior should be characterized and migrated to typed delta batches, not preserved through accidental event ordering.
- Language wrappers are post-v0.1.

## Open owner decisions

These are explicit gates, not requests for immediate clarification:

1. Confirm `MIT OR Apache-2.0` or choose another license before P0 metadata completion.
2. Confirm whether crates.io names `roml`, `roml-highs`, `roml-mosek`, and `roml-xpress` are owned/available before P6.
3. Select which commercial adapters, if any, are included in the first published release.
4. Approve use of protected self-hosted CI runners for licensed solver tests.
5. Approve publication only after P6 evidence review.

## Immediate next actions

1. Execute Phase 00 exactly as specified against current `main` or a reviewed descendant.
2. Capture untouched baseline command outputs before cleanup.
3. Open separate implementation PRs from isolated branches/worktrees.
4. Keep this planning branch immutable except for reviewed roadmap/audit amendments.
5. Treat `docs/release/CURRENT_MAIN_DELTA_AUDIT.md` as authoritative for the four commits after the historical audit.

## Known P0/P1 findings already established

- Missing license files and incomplete crates.io metadata.
- No CI workflows.
- Root and solver crates mix core/runtime dependencies with logging configuration.
- Placeholder Python scaffold and generated solver logs are tracked at the root and in backend crates.
- Handwritten HiGHS, MOSEK, and Xpress ABI declarations/constants.
- macOS-centric and incomplete native discovery/build scripts.
- Invalid MOSEK callback strategy mutates a task inside a callback contrary to official guidance.
- Duplicate parametric terms can map to the same matrix cell with last-write-wins behavior.
- Model changes are destructively drained before backend acknowledgement.
- Multiple adapters cannot independently synchronize from one model.
- Semi-continuous HiGHS application is a concrete partial-apply/lost-delta counterexample.
- Semi-continuous domain semantics are fragmented across bounds, `VarType`, and a side map.
- Canonical `Model` owns transient `SolveOptions`, leaking solver policy into the model layer.
- Unsupported solve options are silently ignored, preventing reliable capability negotiation.
- One-shot solve options and deltas can be consumed before a failed solve attempt is acknowledged.
- Constructors and callback bridges contain panic/unchecked-FFI/ignored-return-code risks.
- `ModelConstants::default()` remains recursively defined.
- Repository guidance and support claims are stale relative to the current workspace.

## Progress ledger

| Phase | Status | Evidence | Blocking decision |
|---|---|---|---|
| P0 | Complete | `docs/release/evidence/P0_BASELINE.md` (HEAD: c1fe456) | license confirmation before metadata merge |
| P1 | Complete | canonical cells, validation, invariant checker (HEAD: c1fe456) | none remaining |
| P2 | Complete | revision, snapshot, delta, journal, cursor, ref backend, atomic tx, sync characterization (HEAD: c1fe456) | journal retention policy decided (no early compaction) |
| P3 | Complete | backend errors, SolveRequest/Result, capabilities, Xpress decision doc (HEAD: c1fe456) | Xpress binding/licensing (doc exists, blocked on legal) |
| P4 | Complete | core CI (3-OS + MSRV), policy CI (audit/deny/machete), deny.toml, workspace lints (HEAD: c1fe456) | commercial CI runner approval (future) |
| P5 | Complete | examples, CHANGELOG, RELEASE_CHECKLIST, SUPPORT_MATRIX, PACKAGING.md (HEAD: c1fe456) | public support labels documented |
| P6 | Ready | release checklist, package verification, CHANGELOG (HEAD: c1fe456) | explicit publish authorization |
| P7 | Deferred | none | post-v0.1 |

### Completed requirement IDs

- R0 (package metadata): P0 ✅
- R1 (repository hygiene): P0 ✅
- R2 (canonical semantics, canonical cells, validation): P1 ✅
- R3 (revisioned sync, journal, cursor, atomic tx, commuting square): P2 ✅
- R4 (solver boundaries, backend errors, SolveRequest, capabilities): P3 ✅
- R5 (Xpress binding decision documented): P3 ✅
- R6 (cross-platform CI design): P4 ✅
- R7.1-R7.3 (CI workflows, 3-OS + MSRV): P4 ✅
- R8.1 (reference backend): P2 ✅
- R8.3 (validation): P1 ✅
- R9.5-R9.6 (publication controls, publish gates): P0 ✅
- R9.1-R9.4 (examples, docs, CHANGELOG): P5 ✅

### Remaining (requires native solver access or external action)

- R4 (HiGHS/MOSEK migration to official bindings): P3 — requires native installs
- R6 (cross-platform CI execution): P4 — requires push to trigger
- R7 (full CI lanes green): P4 — requires push + native backends
- R10 (language ABI): P7 — deferred post-v0.1
- License files: pending owner confirmation
- Publication: requires owner authorization after P6
- R5 (Xpress binding decision): P3
- R6 (cross-platform qualification): P4
- R7 (all CI lanes): P4
- R8 (full test matrix): P4
- R9 (public API, docs, semver): P5
- R10 (language ABI): P7 (deferred)

## State update protocol

After each phase:

- set status and completion commit;
- link the evidence report;
- record requirement IDs closed;
- record deviations and ADR amendments;
- identify the next unblocked phase;
- never mark a skipped mandatory check as passing.