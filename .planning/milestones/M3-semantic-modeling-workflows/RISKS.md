# M3 Risk Register

## Risk classification

- **P0:** correctness, data/model corruption, unsafe backend state, or materially false guarantee.
- **P1:** major API/architecture flaw, unsupported silent behavior, or unusable production workflow.
- **P2:** performance, ergonomics, documentation, or limited-support issue with a safe workaround.

## R1 — Semantic IR becomes a universal abstraction project

**Severity:** P1  
**Failure mode:** M3 attempts to model every optimization paradigm before proving MILP constructs.  
**Mitigation:** implement only linear scalar functions and the enumerated M3 construct set; add extension seams, not unused nonlinear machinery.  
**Stop condition:** a phase proposes AD, Hessians, cones, or general nonlinear AST implementation.

## R2 — Canonical and backend IR responsibilities blur

**Severity:** P0  
**Failure mode:** selected Big-M values/native handles leak into `Model`, or backends inspect/mutate canonical stores.  
**Mitigation:** separate modules/types; compiler output is immutable; architecture tests enforce dependency direction.  
**Reversal trigger:** none. This is a non-negotiable boundary.

## R3 — Backend contract migration destabilizes M2

**Severity:** P1  
**Failure mode:** ordinary solve/recovery workflows regress while introducing backend IR.  
**Mitigation:** P26 starts with an identity compiler; preserve all M2 characterization tests; migrate ReferenceBackend first, then HiGHS; keep one bounded retry invariant.  
**Gate:** no downstream feature work until M2 tests pass through backend IR.

## R4 — Origin mapping is incomplete or lossy

**Severity:** P0  
**Failure mode:** IIS, logs, or reports expose generated IDs without a valid user origin.  
**Mitigation:** origin completeness validator is mandatory; backend snapshots cannot be constructed with unmapped generated entities; property tests cover every bridge.  
**Stop condition:** any bridge creates an anonymous row/variable.

## R5 — Arbitrary Big-M causes invalid or weak models

**Severity:** P0 for invalidity, P1 for numerical weakness  
**Failure mode:** a convenience picks `1e6` or similar without proof.  
**Mitigation:** finite interval proof or explicit user M; report derivation; reject otherwise.  
**Stop condition:** code contains a default Big-M constant.

## R6 — Exact semantics silently become relaxations

**Severity:** P0  
**Failure mode:** exact max/PWL/product/reification compiles only one side or to a convex envelope.  
**Mitigation:** exactness is an explicit type/enum; bridge tests compare feasible sets on bounded small instances; errors use `RelaxationWouldChangeSemantics`.  
**Stop condition:** formulation correctness depends on an objective becoming tight unless the user selected an epigraph/hypograph relation.

## R7 — Temporary overlays leak into later solves

**Severity:** P0  
**Failure mode:** lock/objective row remains after solve or partial failure.  
**Mitigation:** explicit overlay receipt, reverse operations, health check, rebuild fallback, failure injection at every lifecycle step.  
**Gate:** later solve on the same session must match clean rebuild after every injected failure.

## R8 — Assignment identity is insufficient

**Severity:** P0  
**Failure mode:** an assignment from an unrelated model is applied to coincidentally equal entity IDs, or provenance is mistaken for exact state compatibility.  
**Mitigation:** `ModelLineageId` plus generation-safe handles govern assignment compatibility; source instance/revision are retained as provenance; values/domains are validated before backend mutation.  
**Residual:** cross-process serialized reuse is unsupported and documented.

## R9 — Fixing destroys declared-domain information

**Severity:** P1  
**Failure mode:** `unfix` cannot restore bounds or bound changes while fixed behave inconsistently.  
**Mitigation:** declared domain and fixing stored separately; effective domain derived; atomic validation.  
**Gate:** randomized fix/set-bounds/unfix sequences match a reference state machine.

## R10 — Starts, hints, and locks are conflated

**Severity:** P1  
**Failure mode:** unsupported hint becomes fixing or MIP start silently.  
**Mitigation:** separate types; default reject; explicit conversion policy; effective-plan report.  
**Gate:** feasible region is unchanged by starts/hints and changed by locks.

## R11 — Solver capability names hide semantic differences

**Severity:** P1  
**Failure mode:** native multiobjective/PWL/IIS behavior is assumed equivalent by name.  
**Mitigation:** official API audit; limitations and version in capabilities; native path only after semantic conformance tests.  
**Fallback:** portable exact path or typed unsupported.

## R12 — IIS claims overstate backend guarantees

**Severity:** P0  
**Failure mode:** LP-relaxation conflict is called original-MIP irreducible IIS.  
**Mitigation:** kind/scope/minimality/completion fields are required; report renderer prints them.  
**Stop condition:** adapter cannot determine scope or guarantee from official API.

## R13 — HiGHS version skew breaks system mode

**Severity:** P1  
**Failure mode:** bundled headers expose APIs/fields absent from minimum system version.  
**Mitigation:** compile/test both supported modes; version-gated modules/capabilities; no struct-field assumptions without header evidence.  
**Gate:** system mode either passes or rejects the feature actionably without failing unrelated solves.

## R14 — Soft-constraint penalties have wrong objective sign

**Severity:** P0  
**Failure mode:** violation is rewarded in maximize or minimize models.  
**Mitigation:** canonical penalty semantics are always minimization of violation; compiler translates sign into target objective; direct algebra tests for both senses.  
**Gate:** increasing violation never improves the penalized priority under the declared semantics.

## R15 — Signed slack creates unbounded objectives

**Severity:** P0  
**Failure mode:** free variable receives a one-sided linear penalty.  
**Mitigation:** separate signed-correction API; L1 uses positive/negative parts; validation rejects ambiguous penalty.  
**Stop condition:** `soft()` accepts a free signed variable under ordinary linear penalty.

## R16 — Lexicographic fallback locks the wrong bound

**Severity:** P0  
**Failure mode:** degradation formula is wrong for minimize/maximize or relative tolerance near zero/negative objectives.  
**Mitigation:** specify formulas explicitly; property tests over objective senses/signs; compare native and portable paths.  
**Gate:** stage locks are inspectable in effective-plan metadata and origin map.

## R17 — Native multiobjective and portable paths diverge

**Severity:** P1  
**Failure mode:** backend priorities/tolerances use different semantics.  
**Mitigation:** native conformance corpus; use portable path unless exact match proven.  
**Reversal trigger:** version-specific support may be disabled without removing public objective policy.

## R18 — Auxiliary variables pollute ordinary user output

**Severity:** P2  
**Failure mode:** values/iteration become dominated by generated entities.  
**Mitigation:** generated provenance and visibility classification; ordinary solution iteration defaults to user variables; returned construct handles expose relevant auxiliaries.  
**Advanced path:** diagnostic API enumerates all compiled/generated entities.

## R19 — Construct store becomes a collection of side maps

**Severity:** P1  
**Failure mode:** every feature adds new `Model` fields and special-case snapshot logic.  
**Mitigation:** one construct arena/store with typed payloads and common lifecycle/dependency interfaces.  
**Gate:** adding a construct type does not modify unrelated model stores.

## R20 — Compilation cache or analysis mapping becomes stale

**Severity:** P0  
**Failure mode:** a divergent clone or updated model reuses a result/origin map from another compiled state, or a parameter/domain/construct change fails to invalidate generated coefficients.  
**Mitigation:** unique `ModelInstanceId`, exact `CompilationId`, explicit recipe dependencies, rebuild on uncertainty, and compiled-delta versus rebuild tests. Recipe fingerprints are evidence/cache hints only.  
**Default:** any exact-ID mismatch rejects use; semantic changes conservatively rebuild in M3 v1.

## R21 — Portable policy is not actually portable

**Severity:** P1  
**Failure mode:** it still relies on a backend-native construct.  
**Mitigation:** portable report rejects native primitives except universally supported linear/integer rows; CI compares ReferenceBackend/HiGHS compilation artifacts where meaningful.  
**Gate:** portable snapshot inventory contains only declared portable primitives.

## R22 — Performance regression on ordinary models

**Severity:** P2, elevated to P1 if production workloads are materially affected  
**Failure mode:** every primitive parameter solve recompiles the whole model.  
**Mitigation:** identity compiler fast path; exact compiled-state cache keyed by instance/revision and recipe dependencies; P34 baseline and threshold.  
**Threshold:** less than 5% or 50 microseconds median overhead, whichever is larger, on the defined primitive fixture.

## R23 — Public API expands faster than review capacity

**Severity:** P1  
**Failure mode:** inconsistent builders/types become accidental pre-1.0 commitments.  
**Mitigation:** phase-scoped public API inventories; default-private compiler internals; replacement-before-deprecation; one active coding phase.  
**Gate:** each phase receives independent API review.

## R24 — NLP readiness becomes vague marketing

**Severity:** P1  
**Failure mode:** architecture still assumes rows/matrices despite claims.  
**Mitigation:** P34 review uses a concrete extension exercise for `ScalarFunction::Quadratic` and `Nonlinear`, backend IR/capability additions, and required file changes.  
**Pass criterion:** identity, metadata, constructs, objective policy, solve plans, origin mapping, and reports require extension only, not replacement.

## R25 — Planning packet and implementation diverge

**Severity:** P1  
**Failure mode:** agents bypass phase gates or silently rename interfaces.  
**Mitigation:** requirement IDs in every PR; evidence/state updates; design amendments recorded in `DECISIONS.md`; task interface contract in implementation plan.  
**Stop condition:** code introduces a contradictory public semantic without an approved amendment.

## R26 — Finite fingerprints are treated as exact identity

**Severity:** P0  
**Failure mode:** a hash collision or incorrectly reused digest authorizes stale overlay rollback, result mapping, or IIS projection.  
**Mitigation:** correctness uses checked opaque `CompilationId`; deterministic recipe fingerprints are never accepted as authority.  
**Gate:** APIs requiring exact compiled state accept/compare `CompilationId`, not only digest/fingerprint.