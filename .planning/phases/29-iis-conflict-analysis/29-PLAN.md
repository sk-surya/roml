---
phase: 29-iis-conflict-analysis
type: execute
status: planned
owner_packet: 29-DESIGN-PACKET.md from origin/docs/p29-iis-design-packet
depends_on: []
preconditions: routing-gate and exact-highs-api-audit are external gates recorded below
---

# Phase 29 — Solver-Agnostic LP IIS and Conflict Analysis

> **For agentic workers:** use `superpowers:subagent-driven-development` or `superpowers:executing-plans` to execute these slices task-by-task. Every task starts with a failing test, records the expected failure, implements the smallest correct behavior, runs focused and phase checks, and updates evidence. This planning pass contains no production implementation.

**Goal:** deliver a solver-agnostic ROML LP infeasibility engine that reports semantic, origin-aware, explicitly qualified conflicts, with optional audited HiGHS native seeding and no false minimality/native claims.

**Architecture:** compile the requested model scope once into an exact `BackendSnapshot`; derive a deterministic semantic restriction universe with reversible atom toggles; run repeated tri-state feasibility checks in an isolated backend session; reduce a validated seed with adaptive deletion and exact polish; fresh-verify before claiming semantic irreducibility; then render one historical structured report. Native HiGHS data is optional compiled evidence and a seed, never a replacement for semantic verification.

**Tech stack:** Rust 1.85, solver-free `roml` core, existing `CompilationSession`/`BackendSnapshot`/`CompilationId`, `SolverSession` orchestration, `roml-highs` through pinned/generated `highs-sys` 1.15.0, reference backend tests, native qualification tests, mutation/differential tests, and reproducible planted-IIS benchmarks.

## Global constraints

- The primary API is `SolverSession::analyze_infeasibility(&mut self, &Model, &InfeasibilityPlan)`, not a `Model` method.
- `roml` remains solver-free; native IIS code is confined to `roml-highs/src/iis.rs` and authoritative binding boundaries.
- P29 scope is `OriginalLp` plus explicitly requested `LpRelaxation`; LP-feasible/MIP-infeasible analysis is the next phase.
- The feasibility oracle returns only `ProvenFeasible`, `ProvenInfeasible`, or `Unknown`; ambiguous, numerical, interrupted, and limit statuses are `Unknown`.
- A complete portable result may claim only semantic irreducibility relative to its recorded universe/grouping/oracle/numerical policy; it never claims minimum cardinality.
- Every mapping, toggle, cache lookup, and native conversion validates exact `CompilationId`; stale state rejects.
- Analysis uses an isolated session and cannot mutate canonical model state or poison the persistent solve session.
- Normal candidate checks use one build, stable compiled indices, incremental restrictions, and qualified basis reuse; per-candidate rebuilds are prohibited.
- Bound contribution layers are restored top-down: declared bounds → persistent fixing → solve lock/temporary fixing.
- Every native return code is checked; no handwritten HiGHS IIS declarations, layouts, status values, or strategy masks are permitted.
- No feasibility relaxation, all-IIS enumeration, minimum-cardinality search, nonlinear feasibility, MIP-only conflict algorithm, publication, tag, or release is part of this phase.

## Cross-slice contract

The following names are the planned seam. Existing names may be reconciled only when the same semantics remain and the change is recorded in the phase evidence.

### Public analysis plan and mode

```rust
#[non_exhaustive]
pub enum InfeasibilityMode {
    Auto,
    RomlPortable,
    NativeOnly,
    NativeThenRoml,
}

#[non_exhaustive]
pub enum InfeasibilityScope {
    OriginalLp,
    LpRelaxation,
}

pub struct InfeasibilityPlan {
    pub mode: InfeasibilityMode,
    pub scope: InfeasibilityScope,
    pub grouping: ConflictGrouping,
    pub seed_policy: SeedPolicy,
    pub reduction: ReductionPolicy,
    pub budget: AnalysisBudget,
    pub numerical_policy: AnalysisNumericalPolicy,
}

impl<B> SolverSession<B> {
    pub fn analyze_infeasibility(
        &mut self,
        model: &Model,
        plan: &InfeasibilityPlan,
    ) -> Result<InfeasibilityReport, InfeasibilityError>;
}
```

`Auto` may fall back from an unavailable native provider. `NativeOnly` returns typed `Unsupported` for an unqualified provider and never invokes the portable reducer. `NativeThenRoml` requires a qualified native seed, then performs semantic reduction and verification.

### Semantic universe and exact identity

```rust
pub struct SemanticConflictUniverse {
    pub compilation_id: CompilationId,
    pub atoms: Vec<SemanticRestrictionAtom>,
    pub compiled_restrictions: Vec<CompiledRestrictionRef>,
    pub grouping: ConflictGrouping,
}

pub struct SemanticRestrictionAtom {
    pub id: ConflictAtomId,
    pub origin: ConflictOrigin,
    pub kind: ConflictAtomKind,
    pub compiled_restrictions: Vec<CompiledRestrictionRef>,
    pub disable: RestrictionTogglePlan,
    pub restore: RestrictionTogglePlan,
    pub snapshot: ConflictMemberSnapshot,
}

pub struct RestrictionSelection { /* exact universe identity + selected atom ids */ }

pub trait RestrictionOriginMap {
    fn compilation_id(&self) -> CompilationId;
    fn atom(&self, id: ConflictAtomId) -> Result<&SemanticRestrictionAtom, InfeasibilityError>;
    fn map_compiled(
        &self,
        compilation_id: CompilationId,
        member: CompiledRestrictionRef,
    ) -> Result<ConflictAtomId, InfeasibilityError>;
}
```

`ConflictAtomKind` must represent constraint sides, variable bound sides, persistent fixings, solve locks, temporary fixings, grouped constructs, and reserved future function-in-set kinds without exposing matrix rows as the public abstraction. `CompilationId` is mandatory; `Option<CompilationId>` is not an accepted final report field.

### Tri-state oracle and backend factory

```rust
pub trait FeasibilityOracle {
    fn compilation_id(&self) -> CompilationId;
    fn check(
        &mut self,
        selection: &RestrictionSelection,
        budget: &OracleBudget,
    ) -> Result<FeasibilityOutcome, BackendError>;
}

#[non_exhaustive]
pub enum FeasibilityOutcome {
    ProvenFeasible(FeasibilityEvidence),
    ProvenInfeasible(InfeasibilityEvidence),
    Unknown(UnknownReason),
}

pub trait InfeasibilityOracleFactory {
    type Oracle: FeasibilityOracle;

    fn spawn_infeasibility_oracle(
        &self,
        snapshot: &BackendSnapshot,
        universe: &SemanticConflictUniverse,
    ) -> Result<Self::Oracle, BackendError>;

    fn native_conflict(
        &self,
        request: &NativeConflictRequest,
    ) -> Result<NativeConflict, BackendError>;
}
```

An unsupported default factory is allowed. A native failure in `Auto` may fall back only after the isolated session is proven unchanged or rebuilt. `NativeOnly` never falls back.

### Canonical report

```rust
pub struct InfeasibilityReport {
    pub model_lineage: ModelLineageId,
    pub model_instance: ModelInstanceId,
    pub model_revision: ModelRevision,
    pub compilation_id: CompilationId,
    pub backend: BackendIdentity,
    pub provider_chain: Vec<AnalysisProviderRecord>,
    pub scope: InfeasibilityScope,
    pub candidate_universe: CandidateUniverseSummary,
    pub outcome: InfeasibilityOutcome,
    pub completion: AnalysisCompletion,
    pub guarantee: ConflictGuarantee,
    pub oracle_strength: FeasibilityProofStrength,
    pub numerical_policy: AnalysisNumericalPolicy,
    pub members: Vec<ConflictMember>,
    pub native_evidence: Option<NativeConflictEvidence>,
    pub statistics: InfeasibilityStatistics,
    pub warnings: Vec<AnalysisWarning>,
}

#[non_exhaustive]
pub enum InfeasibilityOutcome {
    NoConflict,
    Conflict,
    NoConflictProof,
}

pub struct TextInfeasibilityReport<'a>(pub &'a InfeasibilityReport);
pub struct MarkdownInfeasibilityReport<'a>(pub &'a InfeasibilityReport);
```

`ConflictGuarantee::Irreducible` includes the candidate-universe/grouping identity. `ConflictGuarantee::NativeReported` preserves the backend claim without promoting it. `AnalysisCompletion` includes complete, time limit, oracle-call limit, iteration limit, interrupted, numerical failure, and backend failure. `InfeasibilityOutcome::NoConflict` is returned only after a proven-feasible initial check; `NoConflictProof` records an Unknown initial check. Neither is an IIS.

## Requirement and decision traceability

| Requirement/decision | Closure artifacts |
|---|---|
| Packet D1–D4, D17–D19, D28 | `29-CONTEXT.md`, `29-01` through `29-06`, contract/evidence tests |
| R2.6, R2.9 | semantic-universe invariant tests; no solver state in canonical model |
| R3.5, R3.6 | oracle rollback/rebuild and snapshot-equivalence tests |
| R4.1–R4.3, R4.6 | typed provider capabilities, tri-state oracle, typed errors, unsupported-mode tests |
| R5.1, R5.2, R5.9–R5.11 | HiGHS audit, generated binding compile gate, return-code tests |
| R6.1 | native status-to-tristate tests and qualification matrix |
| R7.4 | bundled/system HiGHS qualification evidence |
| R8.1–R8.3 | benchmark corpus, differential results, mutation suite, verifier gate |
| R9.1–R9.3, R9.6 | curated exports, rustdoc, renderers, examples, support labels |

## Slice order and gates

1. `29-01-PLAN.md` — contract and characterization. Gate: APIs and statuses are characterized; no guessed backend behavior.
2. `29-02-PLAN.md` — semantic restriction universe. Gate: every atom has complete exact mapping and lossless disable/restore.
3. `29-03-PLAN.md` — isolated LP oracle. Gate: transactional checks, status triage, recovery, and persistent-session preservation pass.
4. `29-04-PLAN.md` — reducer and verifier. Gate: complete reports cannot claim irreducibility without fresh single-atom verification.
5. `29-05-PLAN.md` — structured report/renderers. Gate: report and renderers are deterministic and preserve historical evidence.
6. `29-06-PLAN.md` — native HiGHS. Gate: authoritative header/source/version audit and compile-gated generated API pass; otherwise native remains typed `Unsupported`.
7. `29-07-PLAN.md` — qualification/performance evidence. Gate: correctness, mutation, differential, bundled/system, and planted-IIS evidence is attached and independently reviewed.

Slices are serial because each later slice consumes interfaces from the prior slice. The header/source research subtask of Slice 6 may be prepared earlier but cannot land production references before the audit gate.

## Final phase verification matrix

Run the exact applicable per-crate commands and record versions, outputs, skips, and residual risks in `docs/release/evidence/P29_IIS_QUALIFICATION.md`:

```text
cargo fmt --all -- --check
cargo check -p roml --all-targets
cargo clippy -p roml --all-targets -- -D warnings
cargo test -p roml --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
cargo package --list -p roml
cargo check -p roml-highs --all-targets                 # bundled lane
cargo test -p roml-highs --all-targets                 # bundled lane
cargo clippy -p roml-highs --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps
cargo package --list -p roml-highs
cargo test -p roml-highs --no-default-features --features system --all-targets  # each installed system version
```

The system command is run once per supported installed version, never represented by one generic pass. Missing native installations are recorded as skipped/unsupported with diagnostics, not as passing qualification.

## Stop conditions

Stop and request review when a plan would mutate canonical model state, drain a model journal destructively, rebuild per candidate, use a stale `CompilationId`, restore a bound to infinity while a lower layer exists, map Unknown to infeasible, promote native membership to semantic irreducibility without re-reduction, claim minimum cardinality, silently fall back from `NativeOnly`, use a guessed HiGHS API, or broaden scope to MIP/nonlinear/feasibility relaxation.

Stop the native slice if the official generated API, exact header/source, or a supported system version cannot be compile-gated and qualified. The portable provider remains a valid P29 deliverable under that outcome.
