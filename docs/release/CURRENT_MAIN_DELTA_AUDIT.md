# Current-Main Delta Audit

**Authoritative implementation baseline:** `main@82e2ed95545635b628187ba0081fe8c8b03eaafb`  
**Historical audit baseline:** `main@f9ba1921e650b5057bbc4de090a78391f7932a53`  
**Delta commits reviewed:** `dacf1c1`, `2153570`, `d561e73`, `82e2ed9`  
**Reconciliation date:** 2026-07-13

This document supplements `PRINCIPAL_ENGINEERING_AUDIT.md`. The historical audit remains useful evidence for the original state; this delta is authoritative wherever the four newer commits changed facts or introduced new risks.

## Delta summary

The new commits add variable-type mutation, Xpress bulk synchronization, semi-continuous variables, and per-solve LP algorithm selection. They do not invalidate the release-hardening roadmap. They strengthen the need to redesign the model/backend contract before stabilizing APIs because new solver semantics are now encoded through the same destructive changelog and handwritten native constants.

## Superseded historical fact

`MODELING_API.md` now exists. The prior finding that README links to a missing modeling guide is closed as a missing-file defect. The guide still requires correctness and public-API review because it documents prototype synchronization semantics that P1/P2 will change.

## New and strengthened findings

### D1 — Solver concepts now leak into `Model` — P1

`Model` stores `Option<SolveOptions>`, and `set_solver_options` mutates model state for a one-shot backend request. This contradicts the stated solver-independent model boundary and couples canonical mathematical state to solve-session policy.

**Required correction:** move solve configuration into `SolveRequest`/adapter-session calls. The canonical model should not own transient optimizer algorithm choices, logging controls, time limits, callback registrations, or backend policy.

### D2 — Best-effort silent option handling is unsuitable for production — P1

`SolveOptions` documents that unsupported options are silently ignored. Silent degradation makes experiments irreproducible and prevents users from knowing whether requested algorithms were honored.

**Required correction:** capability-aware validation with an explicit result such as `Applied`, `Adjusted`, or `Unsupported`; include the effective backend configuration in solve metadata.

### D3 — Option consumption is non-transactional — P0/P1

`solve_model` drains model changes, then removes one-shot options from the model, then calls `apply_options`. If change application or option application fails, changes and/or options may be lost while the backend can be partially mutated.

**Required correction:** P2 revision acknowledgement plus a solve request that is immutable for the attempt. Consume neither model deltas nor solve policy until the relevant operation is acknowledged.

### D4 — Semi-continuous HiGHS path concretely demonstrates partial-apply data loss — P0

`set_semicontinuous` can first raise the variable's ordinary lower bound and then emit a semi-continuous change. HiGHS can apply the bound update and subsequently return `NotSupported` for the semi-continuous operation. Because `sync_model` destructively drains the batch before application, the backend is partially changed and the model no longer has a replayable delta.

This is no longer a theoretical failure mode. It is an executable counterexample to the current synchronization protocol.

**Required tests in P2/P3:**

1. create a HiGHS-supported variable;
2. request semi-continuous behavior;
3. inject/apply the mixed batch;
4. prove the adapter is classified dirty or rebuild-required;
5. prove no model operation is lost;
6. rebuild from the canonical snapshot and obtain the documented unsupported-capability result without partial state ambiguity.

### D5 — Semi-continuous state is not modeled as a coherent variable domain — P0/P1

Semi-continuity is stored in a side `HashMap<VarId, f64>` while integrality remains in `VarType` and ordinary bounds are mutated separately. Removal, repeated updates, conversion back to ordinary variables, cloning, validation, serialization, and combined semi-integer semantics require one coherent domain model.

**Required correction:** represent variable domain as a validated algebraic data type or a normalized domain specification, for example continuous/integer/binary/semi-continuous/semi-integer with explicit zero-or-interval semantics. Emit one canonical variable-domain transition rather than order-sensitive independent events.

### D6 — New handwritten native constants expand ABI risk — P0/P1

The newer MOSEK and Xpress work adds copied variable-type, algorithm, parameter, and control constants. This increases the surface on which an SDK upgrade can compile while invoking the wrong native semantics.

**Required correction:** reinforces D-006 through D-008: maintained generated HiGHS bindings, official MOSEK Rust API, and a generated/legal Xpress binding boundary.

### D7 — Xpress bulk synchronization is an optimization over an unstable event protocol — P1

The bulk path recognizes an ad hoc subset of event sequences, folds selected mutations into local maps, and falls back for others. This optimization must be preserved only after characterization and then re-expressed over typed P2 `DeltaBatch` operations. It must not become a compatibility constraint on the current adjacency/order conventions.

**Required tests:** bulk-vs-scalar projection equivalence, duplicate cells, mixed add/update batches, semi-continuous transitions, objective switching, failure after native sub-step, and rebuild equivalence.

### D8 — Native construction still panics on environmental failures — P0/P1

New Xpress control helpers use assertions for native return codes. Missing/incompatible installs, invalid controls, and license/configuration failures remain process panics rather than typed constructor errors.

### D9 — Repository contamination increased — P1

Current `main` tracks solver logs at the root and under HiGHS, MOSEK, and Xpress. P0 cleanup must remove all generated logs and prevent recurrence. `.claude/settings.json` should be reviewed for whether repository-wide agent permissions belong in a public crate package/repository.

### D10 — Repository guidance is stale and overclaims readiness — P1

Current `AGENTS.md` calls ROML production-grade, describes only the older workspace shape, and treats destructive changelog draining as an accepted architecture. On the planning branch it is replaced with current repository context plus release-governance rules. Public documentation must use precise pre-release support labels until P6.

## Impact on roadmap

The phase ordering remains unchanged:

1. P0 baseline/hygiene and package controls.
2. P1 canonical model/domain correctness.
3. P2 revisioned synchronization and recovery.
4. P3 authoritative bindings and backend rewrites.
5. P4 platform/backend qualification.
6. P5 API/docs/package curation.
7. P6 release qualification.

The delta adds explicit P1 work for coherent variable domains and explicit P2/P3 tests for solve-request and semi-continuous partial failure. It also confirms that Xpress batching should be characterized and migrated, not discarded blindly.

## Reconciliation record

Current `main` was merged into `docs/public-release-production-roadmap` through PR #2, producing planning-branch merge commit `083cc6d890c59efab9da74c687031cb9ecf27d5b`. No production implementation was changed by the planning work.