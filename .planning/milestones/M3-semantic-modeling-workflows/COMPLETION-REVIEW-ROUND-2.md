# M3 Completion Packet — Independent Review Round 2 Disposition

**Reviewed prior head:** `b34213e691e3fc43f24dc78ae5142b169c4ed830`  
**Review method:** five parallel read-only agents, owner-reported synthesis.  
**Verdict:** request changes; no P0, multiple P1 written-spec blockers.  
**Production effect:** none. No P36 production branch/code may start before corrected packet acceptance + merge.

## Finding group 1 — governance reconciliation

### R2-G1 — stale root/milestone routing
**Disposition:** RESOLVED. Root roadmap/state and milestone state now agree on P36 as planned target, no active implementation, and explicit authorization=false until PR #45 merges.

### R2-G2 — P36 requirement numbering
**Disposition:** RESOLVED. One namespace only: MPS-W01–W14 across requirements/roadmap/index/plans/P34.

### R2-G3 — P36/P30 dependency contradiction
**Disposition:** RESOLVED. P36 is an explicit program dependency for P30.

### R2-G4 — planned target vs active implementation
**Disposition:** RESOLVED. A roadmap/current-phase pointer never authorizes code; state must explicitly authorize the phase after its planning/predecessor merge.

## Finding group 2 — shared contract freeze

### R2-S1 — identity lifecycle/equality
**Disposition:** RESOLVED in `SHARED-CONTRACTS.md` §1: lineage family identity; unique model instance; revision scoped to instance; exact CompilationId authority; fingerprint non-authoritative.

### R2-S2 — overlays/dirty/rebuild/error composition
**Disposition:** RESOLVED in shared contracts §2: no canonical overlay commit; rollback only after verification; uncertainty -> `RequiresRebuild`; primary + cleanup + rebuild errors preserved.

### R2-S3 — parameterized export
**Disposition:** RESOLVED: one evaluated exact `(ModelInstanceId, ModelRevision)` snapshot; successful report records consumed parameter environment.

### R2-S4 — deterministic naming
**Disposition:** RESOLVED: preserve valid unique user names or use export-local dense ordinals; raw slot/debug IDs forbidden.

### R2-S5 — objective lock scale
**Disposition:** RESOLVED: `scale=abs(z*)`, `delta=abs_tol+rel_tol*scale`; zero/negative cases frozen; no `max(1,|z*|)`.

### R2-S6 — objective-policy/priority ownership
**Disposition:** RESOLVED: P31 is sole `ObjectivePolicy` owner and sole `ObjectivePriority(u32)` owner; priority 0 highest.

## Finding group 3 — P36 hardening

### R2-W1 — options/report/errors
**Disposition:** RESOLVED in `36-CONTRACT.md`. Defaults/report/error context are frozen. Additional YAGNI correction: decorative `CompiledLinearFormulation` target removed; compiled-formulation export is deferred to a separate design.

### R2-W2 — representability
**Disposition:** RESOLVED with explicit parameter/fixing/domain/construct/objective/name/nonfinite/inactive-state matrix.

### R2-W3 — path transaction
**Disposition:** RESOLVED: `CreateNew`/`AtomicReplace`, same-directory staging, flush/sync, no remove-then-rename, Linux/macOS/Windows atomic semantics or pre-mutation unavailable error, injected path-op seam, cleanup error preservation.

### R2-W4 — independent semantic oracle
**Disposition:** RESOLVED: test-local mathematical extractor cannot call writer projection/naming/report/HiGHS helpers.

### R2-W5 — exact 94-model corpus
**Disposition:** RESOLVED in `36-NETLIB-MANIFEST.md`: exact SHA + exact filenames; missing/drift/parser/writer/mismatch/panic failures; 94 PASS rows required.

### R2-W6 — structure/solve differential
**Disposition:** RESOLVED: frozen structural/objective tolerances, normalized status rules, mismatch dispositions, no automatic HiGHS authority.

### R2-W7 — execution topology
**Disposition:** RESOLVED to requested waves with disjoint production ownership:

```text
Wave 0 serial: contract/manifest/public seam
Wave 1 parallel: projection | formatter | path
Wave 2 parallel: bounds | objective/RHS/RANGES | ROML oracle | HiGHS oracle
Wave 3 serial: exact 94-model qualification
Wave 4 serial: docs/package/review/exact-head closure
```

## Finding group 4 — P30/P31 interface hardening

### R2-O1 — schemas/provider ownership
**Disposition:** RESOLVED. P30 owns `RelaxationProviderPolicy`; P31 owns `ObjectiveProviderPolicy`; provider result enums and stage/report schemas are explicit.

### R2-O2 — IIS mapping
**Disposition:** RESOLVED. Primitive row sides, declared/imported bounds, persistent fixings map; temporary locks, grouped constructs, compiler-only/stale origins reject all-or-error.

### R2-O3 — relaxation outcomes
**Disposition:** RESOLVED. `OptimalRepair`, proven `NoRepairFound`, `Unknown`, operational error are distinct. `RelaxationAcceptance::{RequireOptimal,AcceptFeasible}` freezes when `FeasibleRepair` is legal; default is `RequireOptimal`.

### R2-O4 — parameterized penalties
**Disposition:** RESOLVED. P30 evaluates finite/nonnegative weights before mutation; P31 evaluates P30 weights before priority-stage construction.

### R2-O5 — rollback composition
**Disposition:** RESOLVED. Successful mathematics with failed cleanup never returns reusable success; primary and cleanup/rebuild failures survive.

### R2-O6 — objective weight ambiguity
**Disposition:** RESOLVED. P31 objective weights are finite nonnegative plain `f64` in M3; symbolic objective weights are deferred rather than left to implementation choice.

## Finding group 5 — P34 hardening

### R2-Q1 — leaf ledger
**Disposition:** RESOLVED: one row per `SM-xx.y`, MPS-Wxx, M3-Cxx with evidence/command/backend-OS/review/residual/PASS-BLOCKED.

### R2-Q2 — executable fault matrix
**Disposition:** RESOLVED with boundary-by-boundary expected revision/health/identity/error/cleanup assertions.

### R2-Q3 — native/portable corpus
**Disposition:** RESOLVED: Q01–Q14, ReferenceBackend, bundled HiGHS 1.15.0 Linux/macOS/Windows, system 1.9.0 Linux floor, observables/tolerances/discrepancy rules.

### R2-Q4 — performance
**Disposition:** RESOLVED: `P34_PRIMITIVE_PARAMETER_UPDATE_V1` fixed at 512 vars/rows, 4096 nnz, fixed seed, parameter in 64 cells, 20 warmups/200 measurements, release/one-thread bundled HiGHS; historical baseline `main@4d111cceafce17aea44a6e396a838d1cc9ef255d` and original max(5%,50us) gate.

### R2-Q5 — packed consumers
**Disposition:** RESOLVED: exact package/list protocol, content/path assertions, fresh `/tmp/p34-consumer-{core,highs,mps,iis-relax,lexicographic}` consumers, only exact documented unpublished-`roml` packaging limitation eligible.

### R2-Q6 — NLP readiness
**Disposition:** RESOLVED: N1 convex QP, N2 convex QCQP, N3 nonconvex bilinear, N4 smooth nonlinear parameterized shapes; per-component additive/bounded-amendment/replacement-required verdicts.

### R2-Q7 — positive closure
**Disposition:** RESOLVED: every conjunction in `34-QUALIFICATION-CONTRACT.md` §7 must affirmatively pass; no-P0/P1 alone is insufficient.

## Final contradiction scan

After the main remediation, four residual ambiguities were found and closed:

1. stale P36 “target” field in shared report metadata removed because P36 now has one semantic export meaning;
2. stale `SemanticModel` label in the exact corpus manifest changed to semantic-writer wording;
3. P31 objective-weight representation frozen to finite nonnegative `f64`;
4. P30 `FeasibleRepair` discretion replaced by explicit acceptance policy with `RequireOptimal` default.

## Re-review gate

This disposition does not self-approve PR #45. The corrected exact head must receive independent written-spec re-review. P36 production remains forbidden until that review clears and PR #45 is owner-merged.
