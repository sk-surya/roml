# M3 Completion Packet — Independent Review Round 2 Disposition

**Reviewed prior head:** `b34213e691e3fc43f24dc78ae5142b169c4ed830`  
**Review method:** five parallel read-only agents, owner-reported synthesis.  
**Verdict:** request changes; no P0, multiple P1 written-spec blockers.  
**Production effect:** none. No P36 production branch/code may start before corrected packet acceptance + merge.

## Finding group 1 — governance reconciliation

### R2-G1 — stale root/milestone routing

**Finding:** `.planning/ROADMAP.md` still described P30 as unplanned and P35 pending; milestone `STATE.md` routed P30.

**Disposition:** RESOLVED.

- root `.planning/ROADMAP.md` is now a concise current routing projection;
- root `.planning/STATE.md` distinguishes `current_phase` routing target from `implementation_authorized: false`;
- milestone `STATE.md` uses the same planned-target/active-implementation language;
- accepted P35 merge is recorded;
- all three authorities require PR #45 merge before P36 code and P36 merge before P30.

### R2-G2 — P36/W requirement numbering mismatch

**Finding:** completion requirements defined MPS-W01–W14 while roadmap summarized W01–W08 as if complete.

**Disposition:** RESOLVED.

`COMPLETION-ROADMAP.md`, `COMPLETION-INDEX.md`, root roadmap, and plans use the single namespace MPS-W01–W14. No W01–W08 alternate numbering remains as an authority.

### R2-G3 — P36 “does not technically gate” contradiction

**Finding:** packet said P36 executed first but did not technically gate P30.

**Disposition:** RESOLVED.

P36 is now explicitly a **program dependency** for P30. Older original architectural independence is historical context only and does not authorize execution out of sequence.

### R2-G4 — planned routing target versus active implementation

**Finding:** P36 was labeled active before planning merge, conflicting with the no-code gate.

**Disposition:** RESOLVED.

While PR #45 is open:

```text
planned routing target = P36
active production implementation = none
implementation_authorized = false
```

Only accepted + merged PR #45 transitions P36 to authorized implementation.

## Finding group 2 — shared contract freeze

### R2-S1 — identity lifecycle/equality under-specified

**Disposition:** RESOLVED in `SHARED-CONTRACTS.md` §1.

Frozen separate roles for lineage, model instance, revision, CompilationId, and non-authoritative recipe fingerprint. Canonical state authority is `(ModelInstanceId, ModelRevision)`; compiled artifact authority requires exact `CompilationId`.

### R2-S2 — overlay/dirty/rebuild/error composition under-specified

**Disposition:** RESOLVED in shared contracts §2.

Apply/rollback verification, `RequiresRebuild`, no canonical overlay commit, primary + cleanup/rebuild error preservation, and “successful solve + failed cleanup = operational error” are binding.

### R2-S3 — parameterized export unclear

**Disposition:** RESOLVED in shared contracts §3 and P36 contract.

P36 writes one evaluated exact canonical snapshot and report metadata records model identity/revision + all consumed parameter values. MPS does not reconstruct symbolic parameter identity.

### R2-S4 — deterministic names could depend on unstable slots

**Disposition:** RESOLVED in shared contracts §4.

Default names use export-local deterministic traversal ordinals; raw IDs/debug rendering are forbidden in output bytes.

### R2-S5 — objective-lock formula ambiguous at zero/negative optima

**Disposition:** RESOLVED in shared contracts §5 and P31.

```text
scale = |z*|
delta = abs_tol + rel_tol * scale
```

Min/max inequalities are frozen; at zero relative degradation is zero. The prior `max(1, |z*|)` proposal is removed.

### R2-S6 — objective policy/priority ownership split

**Disposition:** RESOLVED in shared contracts §6.

P31 is sole canonical `ObjectivePolicy` owner and introduces one `ObjectivePriority(u32)`. P30 ships no priority field early; P31 adds/executes `PenaltyTarget::Priority(ObjectivePriority)`.

## Finding group 3 — P36 hardening

### R2-W1 — options/report/errors incomplete

**Disposition:** RESOLVED in `36-CONTRACT.md`.

Defaults, report fields, structured error taxonomy/context, and no-decorative-option rule are frozen.

Additional YAGNI correction: the public `CompiledLinearFormulation` target was removed entirely from P36. Compiled-formulation export is deferred to a separate design.

### R2-W2 — representability matrix incomplete

**Disposition:** RESOLVED.

Matrix explicitly handles parameters, persistent fixings, continuous/integer/binary/free/fixed/semi domains, active/inactive primitives and constructs, objective policies, names, nonfinite values, overlays, and compiler-generated artifacts.

### R2-W3 — path transaction not cross-platform/testable

**Disposition:** RESOLVED.

`CreateNew`/`AtomicReplace`, no remove-then-rename, Linux/macOS/Windows requirement, `AtomicReplaceUnavailable`, stage-specific errors, and injected path-ops seam are frozen.

### R2-W4 — semantic oracle not independent

**Disposition:** RESOLVED.

P36 Wave 2 has a test-local mathematical extractor prohibited from calling writer projection/naming/report/HiGHS helpers.

### R2-W5 — 94-model inventory/count-only qualification

**Disposition:** RESOLVED.

`36-NETLIB-MANIFEST.md` freezes the exact 94 filenames and corpus SHA. Missing corpus/file, drift, parser rejection, writer rejection, or unresolved mismatch is failure. Required result is 94 explicit PASS rows.

### R2-W6 — solve/structure discrepancy contract incomplete

**Disposition:** RESOLVED.

P36 freezes structure/objective tolerances, normalized termination comparison, and mismatch dispositions. HiGHS never becomes semantic authority.

### R2-W7 — P36 execution topology

**Disposition:** RESOLVED to reviewer-prescribed waves with disjoint production file ownership:

```text
Wave 0 serial: contract/manifest/public seam
Wave 1 parallel: projection | formatter | path
Wave 2 parallel: bounds | objective/RHS/RANGES | ROML oracle | HiGHS oracle
Wave 3 serial: exact 94-model qualification
Wave 4 serial: docs/package/review/exact-head closure
```

## Finding group 4 — P30/P31 interface hardening

### R2-O1 — schemas/provider policies ambiguous

**Disposition:** RESOLVED.

P30 now owns a dedicated `RelaxationProviderPolicy`; P31 owns `ObjectiveProviderPolicy`. P31 schemas for weighted levels, priorities, stage results, locks, and aggregate result are explicit.

### R2-O2 — IIS-to-relaxation origin behavior incomplete

**Disposition:** RESOLVED.

P30 mapping table supports primitive row sides, declared/imported variable bounds, and persistent fixings. Temporary locks, grouped semantic constructs, compiled-only origins, or stale reports reject all-or-error; no silent dropping.

### R2-O3 — Unknown/NoRepairFound/operational failures conflated

**Disposition:** RESOLVED.

P30 defines `OptimalRepair`, `FeasibleRepair`, proven `NoRepairFound`, `Unknown(reason)`, and operational `Err` separately. Numerical metadata is explicit.

### R2-O4 — parameterized penalties before priorities

**Disposition:** RESOLVED.

P30 evaluates active parameterized weights before mutation; P31 revalidates/evaluates all P30 penalty weights before constructing priority stages.

### R2-O5 — rollback could overwrite primary failure

**Disposition:** RESOLVED through shared contracts and P30/P31 fault plans. Primary + cleanup/rebuild evidence is preserved; failed cleanup prevents a success result.

## Finding group 5 — P34 qualification hardening

### R2-Q1 — requirement ledger too coarse

**Disposition:** RESOLVED.

`34-QUALIFICATION-CONTRACT.md` requires one row for every leaf `SM-xx.y`, MPS-Wxx, and M3-Cxx with evidence, command, backend/OS, review disposition, residual risk, PASS/BLOCKED.

### R2-Q2 — fault matrix prose-only

**Disposition:** RESOLVED.

The contract freezes an executable boundary-by-boundary fault matrix with expected revision, health, exact-state trust, cleanup, and result/error behavior.

### R2-Q3 — native/portable matrix underspecified

**Disposition:** RESOLVED.

Q01–Q14 fixtures, ReferenceBackend, bundled HiGHS 1.15.0 Linux/macOS/Windows, system HiGHS 1.9.0 Linux floor, observables, tolerances, and mismatch dispositions are explicit.

### R2-Q4 — performance fixture/baseline unnamed

**Disposition:** RESOLVED.

`P34_PRIMITIVE_PARAMETER_UPDATE_V1` is frozen with exact dimensions/seed/warmups/measured count/configuration and historical baseline `main@4d111cceafce17aea44a6e396a838d1cc9ef255d`.

### R2-Q5 — packed consumers underspecified

**Disposition:** RESOLVED.

The contract requires exact package commands, archive/source assertions, five fresh `/tmp` consumers, no live-workspace path dependencies, and the one historically documented unpublished-`roml` packaging limitation only.

### R2-Q6 — NLP readiness abstract

**Disposition:** RESOLVED.

N1 convex QP, N2 convex QCQP, N3 nonconvex bilinear, N4 smooth nonlinear parameterized shapes are frozen. Each major M3 component receives `READY_ADDITIVE`, `READY_WITH_BOUNDED_M4_AMENDMENT`, or `BLOCKED_REPLACEMENT_REQUIRED`.

### R2-Q7 — no positive closure predicate

**Disposition:** RESOLVED.

P34 now requires the entire affirmative conjunction in `34-QUALIFICATION-CONTRACT.md` §7. Absence of P0/P1 alone is insufficient.

## Re-review gate

This disposition does not self-approve PR #45. The corrected exact head must receive independent written-spec re-review. Production P36 remains forbidden until that review clears and PR #45 is owner-merged.
