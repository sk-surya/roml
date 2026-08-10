# ROML M3 Completion and M4 Foundation Design

**Status:** written-spec remediation after independent review; production implementation is not authorized until this packet is accepted and merged.  
**Planning base:** `main@4467797f002c93a1baab638b5e65976fb8492505`.  
**Execution order:** PR #45 merge -> P36 -> P30 -> P31 -> P34 -> M4 design gate.

## 1. Program objective

Close M3 as a coherent LP/MILP research/production system rather than continue horizontal feature accumulation. Remaining work closes four loops:

```text
external model -> ROML -> external model
infeasible model -> semantic IIS -> controlled repair
multi-criteria model -> deterministic staged solve
all M3 capabilities -> independent integrated qualification
```

Only after those loops are qualified does ROML design quadratic/nonlinear production support.

## 2. Routing and authorization

P36 is the **planned routing target**, not an active implementation while this planning PR is open.

```text
PR #45 accepted + merged
  -> authorize P36 production branch/worktree
  -> P36 accepted + merged
  -> authorize P30
  -> P30 accepted + merged
  -> authorize P31
  -> P31 accepted + merged
  -> authorize P34
  -> P34 accepted + merged
  -> M3 complete
  -> M4 design gate only
```

P36 is an explicit program dependency for P30. Although P30's original mathematical prerequisites existed after P28, the owner-selected completion sequence deliberately closes MPS interchange before resuming solve-semantics work.

The root `.planning/STATE.md` explicit authorization flag is authoritative. A phase number/roadmap position alone never authorizes production code.

## 3. Shared semantic contracts

The detailed binding definitions are in `.planning/milestones/M3-semantic-modeling-workflows/SHARED-CONTRACTS.md`. The remaining phases do not independently redefine them.

### 3.1 Identity

- `ModelLineageId`: family compatibility across related clones; clone preserves lineage; exact opaque equality only; not canonical-state or cross-process authority.
- `ModelInstanceId`: one concrete model object; every clone receives a new instance ID.
- `ModelRevision`: monotone semantic version within one instance; exact canonical-state key is `(ModelInstanceId, ModelRevision)`.
- `CompilationId`: exact opaque identity of a compiled backend state; results/origins/overlays/IIS/analysis compose only with exact matching identity.
- fingerprints/hashes are evidence/cache aids, never stale-state authority.

### 3.2 Solve-scoped transactions

Temporary restrictions/objectives/relaxations never commit into canonical state. Apply/rollback uses receipts; rollback success means the expected base compiled state is verified. Uncertainty forces `RequiresRebuild`. Primary operation and cleanup/rollback/rebuild errors are all preserved; a mathematically successful solve followed by failed cleanup is an operational error, not a reusable success result.

### 3.3 Parameterized export

MPS stores numbers, not ROML symbolic parameter graphs. P36 exports one evaluated mathematical snapshot identified by exact model instance/revision and reports every consumed parameter value deterministically. Round-trip equality compares that evaluated mathematics, not parameter identity/dependency mutability.

### 3.4 Deterministic naming

Writer bytes never contain raw slot/generation/debug IDs. Default naming preserves valid unique user names or generates export-local deterministic ordinal names such as `X000001`/`R000001`; collision decisions are deterministic and reported.

### 3.5 Objective degradation locks

For the P31 normalized minimization stage value `z*`, absolute tolerance `a >= 0`, relative tolerance `r >= 0`:

```text
scale = abs(z*)
delta = a + r * scale
g(x) <= z* + delta
```

At zero, relative tolerance contributes zero. Negative optima use positive magnitude `|z*|`. No hidden `max(1, |z*|)` scale is permitted.

### 3.6 Objective ownership

P31 is the sole owner of canonical `ObjectivePolicy` and the one shared `ObjectivePriority(u32)`, with priority 0 highest. P30 does not ship a decorative priority target; P31 adds that variant when the executor exists.

## 4. P36 — deterministic semantic MPS write-back

### 4.1 Scope

P36 has one public semantic meaning:

```text
current canonical/evaluated primitive linear LP/MILP
    -> deterministic free MPS
```

There is no P36 `CompiledLinearFormulation` option. If compiled-formulation export is later needed, it receives a separate design because its identity/provenance/backend-IR contract is different.

P36 output dialect is deterministic free MPS with canonical section/order/vector/float/line-ending choices. P35 continues reading fixed and free MPS.

### 4.2 Writer API contract

`MpsWriteOptions` exposes only options that govern actual P36 behavior:

```rust
pub struct MpsWriteOptions {
    pub name_policy: MpsNamePolicy,
    pub destination_policy: MpsDestinationPolicy,
}
```

Defaults:

```text
name_policy = PreserveOrGenerate
destination_policy = AtomicReplace
free MPS / LF / canonical finite f64 formatting
```

`MpsWriteReport` records exact model lineage/instance/revision, evaluated parameter values, dimensions/counts, integer count, objective/vector presence/names, deterministic name map, exact semantic lowerings, and inactive omission counts.

The full error taxonomy and representability matrix are frozen in `.planning/phases/36-mps-writeback/36-CONTRACT.md`.

### 4.3 Representability policy

Supported: active primitive linear rows; continuous/integer/binary domains including free/fixed/custom bounds; active single/no objective; parameterized supported numeric state after evaluation.

Exact semantic lowering: persistent fixing may emit effective equal bounds; report records that fixing provenance itself will not be reconstructed by MPS.

Rejected: active high-level constructs requiring compiler-generated formulations, semi-continuous/semi-integer domains, weighted/lexicographic objective policy, nonfinite/unbound numeric state, and any unsupported feature. Inactive non-mathematical entities may be omitted only under the frozen matrix/report rules.

Source comments/layout/order/rim-vector spellings/parameter graph/fixing provenance are not round-trip claims.

### 4.4 Path transaction

`write_path` stages in the destination directory, writes/flushes/syncs before commit, and uses platform-appropriate atomic replacement on supported Linux/macOS/Windows. It never implements replacement by deleting the old destination first. `CreateNew` handles destination races without modifying an existing destination. An internal path-ops seam injects failures at create/write/flush/sync/replace/cleanup and verifies old destination preservation plus composite errors.

### 4.5 Independent oracles

The ROML round-trip oracle is test-local and cannot reuse writer projection/naming/report helpers. It independently extracts normalized objective, variable-domain, row-bound, integrality, and matrix mathematics before and after `read(write(model))`.

The HiGHS oracle independently compares direct ROML->HiGHS with ROML->MPS->native `readModel` on full structure, normalized status, and paired optimal objective.

Frozen comparison rules/dispositions live in `36-CONTRACT.md`; HiGHS is evidence, not semantic authority.

### 4.6 Exact corpus gate

`36-NETLIB-MANIFEST.md` freezes exactly 94 `.mps` paths at `sk-surya/lp-data-netlib@56257eea85b433ce6aa67d26156b36385318fd6f`.

Every file must exist and pass:

```text
P35 read
 -> P36 write
 -> deterministic second write
 -> P35 re-read + independent ROML structure compare
 -> native HiGHS structure compare
```

Missing corpus/file, manifest drift, parser failure, writer rejection, or unresolved mismatch is failure. P36 does not classify a frozen-manifest writer rejection as a successful exclusion.

### 4.7 Execution waves

```text
Wave 0 serial: contract + manifest + public seam
Wave 1 parallel: projection | formatter | path API (disjoint files)
Wave 2 parallel: bounds/markers | objective/RHS/RANGES | independent ROML oracle | native HiGHS oracle
Wave 3 serial: exact 94-model corpus qualification
Wave 4 serial: docs/package/full review/exact-head evidence/closure
```

P36 closes only after all MPS-W01–W14 and the positive predicate in `36-PLAN.md` pass and owner merge completes.

## 5. P30 — soft constraints and feasibility relaxation

### 5.1 Persistent softening

Persistent soft constraints are canonical semantic constructs and advance model revision. Upper/lower violation roles are distinct with complete origins.

```text
upper f(x) <= u -> f(x) - v_up <= u, v_up >= 0
lower f(x) >= l -> f(x) + v_lo >= l, v_lo >= 0
equality/range  -> distinct lower + upper violation roles
```

Finite nonnegative caps are explicit. Signed correction remains a separate API.

### 5.2 Penalties

P30 implements only `PenaltyTarget::{None,Objective}`. The weight uses the existing parameter-aware numeric expression contract and is evaluated to a finite nonnegative number before compilation/backend mutation. P31 later adds `Priority(ObjectivePriority)` and closes that subpart of SM-10.6.

### 5.3 Solve-scoped repair

P30 defines its own provider policy:

```text
PortableOnly | PreferNative | NativeRequired
```

and explicit acceptance policy:

```text
RequireOptimal | AcceptFeasible
```

Defaults are weighted-L1, portable provider, and `RequireOptimal`. Mathematical outcomes remain distinct:

```text
OptimalRepair
FeasibleRepair        // only AcceptFeasible + valid feasible incumbent
NoRepairFound         // proven no permitted repair under scope/caps
Unknown(reason)       // no accepted repair and no proof of no repair
```

Under `RequireOptimal`, a feasible incumbent without optimality proof remains `Unknown` according to termination evidence rather than becoming `FeasibleRepair`. Preflight/compile/apply/solve/extract/rollback/rebuild failures are operational errors, not mathematical outcomes. Numerical metadata records objective/bound/gaps/tolerances where honestly available.

### 5.4 P29 composition

IIS-to-relaxation mapping is all-or-error:
- original/imported row side -> `ConstraintSide`;
- declared/imported explicit/synthetic variable bound -> `VariableBound`;
- persistent fixing -> `PersistentFixing`;
- temporary locks/overlay-only restrictions/grouped semantic constructs/compiler-only members -> explicit unsupported-origin error;
- stale instance/revision/CompilationId -> stale-analysis rejection before mutation.

No unsupported IIS member is silently dropped, and IIS scoping does not imply a globally minimum repair.

## 6. P31 — canonical objective policy and lexicographic execution

P31 introduces one `ObjectivePriority(u32)` and one canonical:

```rust
pub enum ObjectivePolicy {
    None,
    Single(Objective),
    Weighted(WeightedObjectives),
    Lexicographic(LexicographicObjectives),
}
```

**P31 objective weights are finite nonnegative plain `f64` in M3. Parameterized/symbolic P31 objective weights are deferred.** P30 penalty weights remain parameter-aware, and P31 resolves those penalty weights numerically before constructing priority stages.

Weighted levels, execution provider, stage result, lock report, continuation decision, exact CompilationId, and aggregate result schemas are frozen by `31-PLAN.md`.

Portable execution is the semantic reference:

```text
validate exact policy/base
for priority ascending:
  apply normalized temporary objective
  solve stage
  extract all objective values + scalar stage value
  classify continuation
  create exact |z*|-scaled normalized degradation lock if another stage runs
finally rollback and verify base
return result only after cleanup verification
```

Default continuation requires optimal. `BestFeasible` is explicit and may descend only from a valid feasible incumbent; reports never imply optimality.

Native multiobjective is selected only if priorities, numeric weights, abs/rel tolerances, zero/negative optimum scaling, continuation/status, and objective constants match the ROML contract.

## 7. P34 — final M3 qualification

P34 is governed by `34-QUALIFICATION-CONTRACT.md`; it does not invent features to make M3 appear broader.

### 7.1 Leaf traceability

One ledger row for every `SM-xx.y`, MPS-W01–W14, M3-C01–C05 records owner phase, implementation evidence, exact qualification command/test, backend/OS scope, review disposition, residual risk, and PASS/BLOCKED. Aggregate SM rows do not substitute for leaf requirements.

### 7.2 Executable fault matrix

The frozen matrix covers canonical validation, compile/delta/rebuild, overlay apply/solve/extract/rollback/verification, P29 unknown/final verify, P30 relaxation, P31 stage/lock/final cleanup, and P36 path failure boundaries. Every row asserts canonical revision, backend health, exact-state trust, cleanup, and error composition.

### 7.3 Native/portable corpus

Q01–Q14 cover primitive parameter deltas, constructs, PWL, overlays, MIP starts, IIS, weighted-L1 relaxation, mixed-sense/zero-optimum lexicographic solves, and parameterized MPS snapshots.

Required primary matrix includes ReferenceBackend, bundled HiGHS 1.15.0 on Linux/macOS/Windows, and system HiGHS 1.9.0 Linux compatibility floor. Structure and optimal-objective tolerances/discrepancy dispositions are frozen.

### 7.4 Performance

Fixture `P34_PRIMITIVE_PARAMETER_UPDATE_V1` is deterministic: 512 variables, 512 rows, 4096 nnz, fixed seed, one parameter affecting 64 cells, 20 warmups and 200 measured release attempts, bundled HiGHS, one thread.

Historical baseline: `main@4d111cceafce17aea44a6e396a838d1cc9ef255d` (pre-P25 M3 implementation). Candidate median may exceed baseline by at most `max(5% baseline median, 50us)` unless profiling evidence receives explicit owner-approved exception.

### 7.5 Packed consumers

A committed script packages/lists from a clean worktree, verifies no planning/worktree/corpus/git/target/log/machine leakage, then creates fresh `/tmp/p34-consumer-{core,highs,mps,iis-relax,lexicographic}` projects using only packed/extracted dependency sources. The known pre-publication `roml-highs` inability to resolve unpublished `roml` is the only eligible packaging limitation and must match its exact diagnostic; all other packaging failures block closure.

### 7.6 NLP readiness

Review concrete shapes:
- N1 convex QP objective;
- N2 convex quadratic constraint;
- N3 nonconvex bilinear objective/function;
- N4 smooth nonlinear parameterized objective/constraint.

For every shape and every major M3 component, verdict is `READY_ADDITIVE`, `READY_WITH_BOUNDED_M4_AMENDMENT`, or `BLOCKED_REPLACEMENT_REQUIRED`. Any replacement-required verdict blocks closure unless the owner explicitly revises the architectural goal through reviewed decision.

### 7.7 Positive closure

M3 closes only if every conjunction in `34-QUALIFICATION-CONTRACT.md` §7 has affirmative evidence, independent reviews have no unresolved P0/P1, exact-head mandatory CI passes, and P34 is owner-merged.

## 8. M4 direction after closure

M4 begins with design, not automatic code. It must extend rather than replace the M3 function-in-set, identities, provenance, compiler/backend IR, capabilities, SolvePlan/overlays, objective policies, and reporting boundaries.

Proposed design questions include quadratic canonical terms, convexity proof/metadata, QP/QCQP backend IR, nonconvex semantics without fake MILP exactness, smooth nonlinear evaluation/derivative interfaces, and local-vs-global diagnostic guarantees.

## 9. Non-goals before P34 closure

Do not start:
- fixed MPS writer or compiled-formulation MPS export;
- LP/JSON/SMPS breadth;
- generalized new solver adapters solely for parity;
- minimum-cardinality IIS research;
- quadratic/nonlinear production implementation;
- publication/tag/release without a separate explicit owner gate.

## 10. Planning packet acceptance

This written design is accepted only after independent re-review finds zero unresolved P0/P1 contradictions/gaps across routing, shared contracts, P36/P30/P31 interfaces, P34 closure protocol, and requirement numbering. Only its merge authorizes the P36 production phase.
