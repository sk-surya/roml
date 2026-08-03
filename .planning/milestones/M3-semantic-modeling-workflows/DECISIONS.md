# M3 Architecture and API Decisions

## D1 — Preserve semantic constructs canonically

**Decision:** indicators, min/max, absolute value, PWL, Boolean/cardinality, products, and soft constraints remain high-level canonical model entities.

**Reason:** eager expansion loses intent, prevents backend-aware representation, weakens diagnostics, and creates a linear-only dead end.

**Consequence:** canonical snapshots/deltas and invariant checks must include constructs.

## D2 — Separate canonical semantic IR from backend IR

**Decision:** a compiler transforms canonical model state into a capability-targeted backend IR.

**Reason:** canonical model state should not contain native handles or selected formulations, while backends should not parse the mutable semantic model.

**Consequence:** M3 amends the advanced backend synchronization contract.

## D3 — Function-in-set is the extensibility seam

**Decision:** primitive constraints are represented as scalar functions in scalar sets; M3 implements linear scalar functions only.

**Reason:** this keeps the ordinary linear API intact while creating an additive path for quadratic, nonlinear, vector, and conic functions/sets.

**Consequence:** `LinExpr` remains the ergonomic linear expression, not the permanent universal expression type.

## D4 — Lineage governs reusable assignment compatibility

**Decision:** assignments and solutions carry an opaque model lineage in addition to revision/entity information.

**Reason:** independent models can contain coincidentally equal generational IDs, while clones should be able to reuse assignments when descendant entity handles remain valid.

**Consequence:** clones preserve lineage; assignment application validates lineage, entity generation, and value/domain compatibility. Cross-process serialized identity remains out of scope.

## D5 — Generated entities use separate identities

**Decision:** compiler and solve-overlay entities use compiled IDs and mandatory origin mappings.

**Reason:** generated entities must not collide with or masquerade as user entities.

**Invariant:** no compiled entity exists without an `EntityOrigin`.

## D6 — Fixing is first-class and compiles as bounds

**Decision:** persistent fixing is stored separately from declared variable domain and compiles by setting lower and upper bounds equal.

**Reason:** bound fixing is the natural domain restriction, supports incremental bound updates, avoids unnecessary rows, and produces clearer bound provenance.

**Consequence:** equality-row fixing remains an explicit ordinary constraint.

## D7 — Persistent and solve-scoped restrictions are separate

**Decision:** persistent `Model::fix` mutations advance canonical revision; solution locks and temporary fixings live in a reversible solve overlay.

**Reason:** transient experimentation must not pollute model history or leak into later solves.

**Invariant:** rollback uncertainty forces backend rebuild.

## D8 — Assignments, starts, hints, and locks are distinct

**Decision:** `PrimalAssignment` is a neutral partial value map; MIP starts, hints, and locks wrap it with different semantics.

**Reason:** starts seek incumbents, hints guide search, and locks alter feasibility.

**Consequence:** conversions require explicit policy and appear in effective-plan metadata.

## D9 — Objective policy is mathematical intent

**Decision:** single, weighted, and lexicographic objective policies may be stored in the model, with a solve-time override in `SolvePlan`.

**Reason:** objective ordering and degradation tolerances define the optimization problem rather than merely solver tuning.

**Consequence:** backend IR carries an explicit compiled objective policy; sequential fallback uses solve-overlay objective locks.

## D10 — Typed capability registry replaces flat growth

**Decision:** feature support is queried by typed `BackendFeature` with limitations and version-aware declarations.

**Reason:** starts, hints, IIS, indicators, SOS, PWL, and multiobjective support cannot be represented safely as a growing undifferentiated Boolean record.

**Consequence:** native backend support and ROML bridge support remain separate concepts.

## D11 — Bridges are explicit compiler components

**Decision:** every portable formulation is implemented as a named bridge with validation, dependencies, origin mapping, and a formulation report.

**Reason:** hidden expansion is unreviewable and makes solver comparisons unreliable.

**Consequence:** bridge recipes have deterministic fingerprints, but those fingerprints are not stale-state authority.

## D12 — Big-M requires proof

**Decision:** ROML derives Big-M only from finite bounds or accepts an explicit user value; otherwise compilation fails.

**Reason:** arbitrary constants create incorrect or numerically weak models.

**Consequence:** M values and derivations are inspectable.

## D13 — Do not infer exactness from objective context

**Decision:** exact min/max/abs/PWL relations are distinct from epigraph/hypograph relations.

**Reason:** relying on a current objective to tighten inequalities is fragile under objective changes, feasibility solves, or lexicographic stages.

**Consequence:** users select exact or one-sided semantics explicitly.

## D14 — Continuous reification needs separation semantics

**Decision:** exact threshold detection for continuous expressions requires an explicit separation/tolerance.

**Reason:** strict inequalities are not directly representable in MILP.

**Consequence:** integral expressions may infer a unit gap only when integrality is proven.

## D15 — Soft constraints preserve both intent and usable slacks

**Decision:** softening remains a semantic construct but creates stable canonical auxiliary violation variables with generated provenance.

**Reason:** users need a one-call workflow and may also need to inspect or reuse violation variables.

**Consequence:** the compiler owns row modification; auxiliary variables are visible through returned handles but grouped under the construct.

## D16 — Signed correction is not ordinary violation slack

**Decision:** signed correction has a separate API and explicit penalty semantics.

**Reason:** linearly penalizing a free signed variable is generally not an L1 violation penalty and may be unbounded.

## D17 — IIS and feasibility relaxation are separate

**Decision:** infeasibility analysis reports conflicting members; feasibility relaxation proposes or executes weighted changes. They use separate APIs.

**Reason:** they answer different questions and have different backend guarantees.

## D18 — IIS reports state guarantees precisely

**Decision:** reports include analysis kind, scope, minimality claim, completion status, backend/version, model instance/revision, and exact compilation identity.

**Reason:** solver APIs differ; ROML must not label an LP-relaxation conflict as an original-MIP IIS or map conflict IDs through a stale compilation.

## D19 — Native IIS means official backend support

**Decision:** HiGHS IIS implementation is derived from pinned official headers/APIs. Unsupported versions return typed `Unsupported`.

**Reason:** a portable deletion filter is not equivalent to native IIS capability.

## D20 — Native multiobjective is used only when semantically equivalent

**Decision:** backend-native execution is selected only when its priority/tolerance semantics match ROML's policy; otherwise ROML runs portable sequential stages.

**Reason:** nominal feature names do not guarantee identical behavior.

## D21 — Portable policy exists for research

**Decision:** `CompilationPolicy::Portable` forces deterministic ROML formulations where available.

**Reason:** researchers need matched formulations across solvers and reproducible comparisons.

**Consequence:** the effective compilation report is part of benchmark evidence.

## D22 — Semantic changes may rebuild first

**Decision:** M3 v1 permits semantic construct changes to force deterministic backend rebuild while preserving existing primitive incremental paths.

**Reason:** correctness and compiler-rebuild equivalence precede incremental relowering complexity.

**Reversal trigger:** a measured production workload shows construct mutation rebuilds dominate and a stable recipe delta can be proven.

## D23 — Limit exact product scope

**Decision:** M3 exact products cover binary-binary and binary times bounded linear scalar functions.

**Reason:** continuous bilinear equality is nonconvex and cannot be represented exactly by a generic MILP bridge.

**Consequence:** future relaxations must be named as relaxations.

## D24 — PWL relation determines formulation

**Decision:** convex epigraphs and concave hypographs use linear inequalities; exact/nonconvex graphs use native PWL, SOS2, or exact disjunctions.

**Reason:** relation semantics, not merely point data, determine whether binaries are necessary.

## D25 — No general nonlinear implementation in M3

**Decision:** M3 adds only the abstraction boundary and readiness tests for NLP.

**Reason:** the immediate product objective is a solid, useful MILP framework. Implementing expression tracing, derivatives, and NLP backends now would expand scope before the semantic compiler is proven.

## D26 — One active implementation phase by default

**Decision:** WIP is bounded to one coding phase and one review/fix branch.

**Reason:** the bottleneck is review and integration quality, not raw parallel code generation.

## D27 — M2 ordinary API remains stable

**Decision:** existing `Model`, `LinExpr`, `Highs::solve`, `solve_with`, and `Solution` golden-path usage remains source-compatible unless an executable contradiction is documented and approved.

**Reason:** M3 should add power without reopening settled ordinary ergonomics.

## D28 — Exact state uses instance and compilation IDs, not hashes

**Decision:** every live model clone receives a distinct `ModelInstanceId`; every compiled backend state receives a distinct opaque `CompilationId`. Deterministic recipe fingerprints/digests may support evidence and caching but never authorize result, overlay, or analysis mapping.

**Reason:** two clones can preserve lineage and have equal revision numbers while containing different canonical states, and finite hashes are not exact identity.

**Consequence:** canonical state is identified by `(ModelInstanceId, ModelRevision)`. Backend results, overlay receipts, conflict data, and origin maps must agree on exact `CompilationId` before use. The four identity values and their roles are: `ModelLineageId` (assignment compatibility across related clones), `ModelInstanceId` (identity of one concrete model clone), `CompilationId` (exact backend state used by solutions, overlays, and IIS results), and `RecipeFingerprint` (deterministic evidence/cache aid only — never a correctness authority).
## A29 — Formulation preference is part of the canonical construct entry

**Amendment to D-plan interface contract (accepted during P25 re-verification, blocking review F4).** `ConstructEntry` gains `preference: FormulationPreference`; the preference threads through `Change::ConstructAdded`, `ModelOp::AddConstruct`, and snapshot/delta reconstruction. `ConstructData.preference` is removed — the entry is the single authority.

**Reason:** P26 must honor `Auto`/`Portable`/`NativeRequired` while compiling only from canonical snapshots/deltas; without the field the preference existed only in the live arena.

**Consequence:** `ConstructEntry { id, kind, active, preference }` is the canonical per-construct record from P25 onward.

## A30 — P25 construct fixture payload is crate-private

**Amendment (blocking review F3).** The P25 fixture scaffolding (`FixturePayload`, `ConstructData`, `Model::add_construct_fixture`, and the `ConstructKind::Fixture` variant) is crate-private; the construct module is `pub(crate)` and only `Construct` and `FormulationPreference` are exported. `ConstructKind`/`ConstructEntry` become public exports when the real per-construct variants land (P32+); the `#[non_exhaustive]` extension boundary stays.

**Reason:** the plan requires a private fixture payload; a public fixture variant would ship test scaffolding in the API and invite misuse before real constructs exist.

**Consequence:** P25 construct-lifecycle tests live in-crate (`#[cfg(test)]`); `ModelSnapshot.constructs` remains `pub #[doc(hidden)]` so external crates can build snapshot literals.

## A31 — DeltaBatch semantic entries carry a narrowed contract

**Amendment (blocking review F2).** `DeltaBatch.functions`/`constructs` are the view of entities **added** by the batch with final folded bounds, minus entities removed by the same batch. Updates to pre-existing functions ride the underlying ops (`SetCell`/`SetConstraintBounds`/`RemoveConstraint`); full before/after semantic entries for pre-existing functions are deferred until recipe-level incremental equivalence is proven (design §8).

**Reason:** a self-contained full semantic delta for updates requires incremental equivalence evidence that M3 v1 deliberately defers; the narrowed contract must be explicit so P26 does not assume coverage it will not get.

**Consequence:** P26 consumes the ops for updates and the semantic entries for added entities; any consumer must not treat `functions` as exhaustive for pre-existing constraints.
