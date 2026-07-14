# ROML Public-Release Hardening Design

**Status:** accepted planning design  
**Baseline:** `f9ba1921e650b5057bbc4de090a78391f7932a53`

## 1. Problem

ROML currently combines four concerns:

1. a symbolic MILP model with parameter-dependent coefficients;
2. a mutable event log;
3. solver-specific projections;
4. native ABI/build policy.

That composition works on the maintainer's tested macOS environment, but the boundaries are not strong enough for a public production library. Correctness currently depends on event adjacency, single-consumer draining, handwritten ABI constants/layouts, and backend callback assumptions.

## 2. Target decomposition

```text
User modeling API
       |
       v
Canonical Model --------------------> Snapshot(revision)
       |                                      |
       v                                      v
Revisioned Journal --> Delta Compiler --> Adapter Session
                                             |
                           +-----------------+-----------------+
                           v                 v                 v
                       HiGHS safe         MOSEK safe        Xpress safe
                       adapter            adapter           adapter
                           |                 |                 |
                       highs-sys        official mosek      generated/dynamic
```

The core's universal property is: every legal user mutation factors through one canonical model transition and one revision. Backend-specific projections are morphisms from canonical revisions to backend states. Two projection paths—incremental deltas and full snapshots—must commute observationally:

```text
Model(r0) --mutations--> Model(r1)
   |                         |
 snapshot                  snapshot
   v                         v
Backend(r0) --deltas------> Backend(r1)
```

For each supported backend observation `O`:

`O(apply(snapshot(r0), deltas(r0,r1))) == O(apply(snapshot(r1)))`

This commuting-square property is the central correctness oracle.

## 3. Canonical model design

### 3.1 Identity

Typed IDs remain opaque capabilities inside one model instance. Public APIs must not encourage persistence of raw slot indices. Future stable external IDs are separate.

### 3.2 Coefficients

A matrix/objective coefficient is identified by:

```rust
struct CellKey {
    target: CoefficientTarget,
    variable: VarId,
}
```

The canonical store maps each key to one normalized `ValueExpr` plus cached evaluated value and dependency set. User expressions can contain duplicate terms, but compilation combines them with exact symbolic addition before insertion.

A zero expression may either remove the cell or remain as a zero cell according to a documented normalization policy; the initial implementation should remove structural zeros after safe simplification.

### 3.3 Validation

All mutations pass through validation:

- no NaN;
- finite coefficient/parameter values unless a specific type permits otherwise;
- bounds permit signed infinity but require `lower <= upper` and no NaN;
- binary variable semantics are explicit when user-supplied bounds differ from `[0,1]`;
- division by zero/non-finite expression evaluation returns an error;
- stale/foreign IDs return typed identity errors.

### 3.4 Transactions

A transaction stages canonical changes, validates the final staged state, and commits exactly one revision with one typed `DeltaBatch`. Failure leaves model state/revision unchanged.

## 4. Revision and journal design

```rust
#[repr(transparent)]
pub struct ModelRevision(u64);

pub struct DeltaBatch {
    pub from: ModelRevision,
    pub to: ModelRevision,
    pub operations: Vec<ModelOp>,
}

pub struct ModelSnapshot {
    pub revision: ModelRevision,
    // canonical active entities/cells/objective state
}
```

The journal retains ordered batches. An adapter session stores:

```rust
pub struct AdapterCursor {
    applied: ModelRevision,
    health: AdapterHealth,
}

enum AdapterHealth {
    Ready,
    RequiresRebuild { reason: String },
    Terminal,
}
```

Application outcomes distinguish successful acknowledgement, unsupported incremental operation requiring rebuild, recoverable failure with unchanged backend state, and dirty partial failure.

The first implementation may retain all batches for model lifetime. Compaction is a later optimization after cursor semantics are proven.

## 5. Delta operations

Avoid compiler conventions represented as neighboring primitive events. Prefer backend-relevant aggregate operations:

```rust
enum ModelOp {
    AddVariable { id, bounds, kind, name },
    RemoveVariable { id },
    UpdateVariable { id, bounds, kind, active },
    AddConstraint { id, bounds, active, cells: Vec<(VarId, f64)> },
    RemoveConstraint { id },
    UpdateConstraint { id, bounds, active },
    SetCell { target, variable, value },
    RemoveCell { target, variable },
    AddObjective { id, sense, constant, active, cells: Vec<(VarId, f64)> },
    RemoveObjective { id },
    SetActiveObjective { id: Option<ObjId> },
    UpdateObjective { id, sense, constant },
}
```

Parameter changes compile into evaluated `SetCell` operations for affected canonical cells. The model journal may separately preserve semantic parameter operations for auditing, but adapters consume deterministic projected operations.

## 6. Adapter contract

A backend implements:

```rust
trait SolverBackend {
    type Session: SolverSession;
    fn metadata(&self) -> BackendMetadata;
    fn capabilities(&self) -> BackendCapabilities;
    fn create_session(&self, snapshot: &ModelSnapshot) -> Result<Self::Session, SolverError>;
}

trait SolverSession {
    fn revision(&self) -> ModelRevision;
    fn apply(&mut self, batch: &DeltaBatch) -> Result<ApplyOutcome, SolverError>;
    fn rebuild(&mut self, snapshot: &ModelSnapshot) -> Result<(), SolverError>;
    fn solve(&mut self, request: &SolveRequest) -> Result<SolveResult, SolverError>;
}
```

The exact names may change during implementation, but the semantics may not regress to destructive model-owned synchronization.

## 7. Binding strategy

### HiGHS

Use `rust-or/highs-sys`, generated from the official header and capable of bundled static builds/system discovery. Pin a known version. Audit callback symbols and official callback semantics. Keep unsafe code inside a minimal native module; the public adapter is safe.

### MOSEK

Use official `mosek` Rust API. The official callback contract forbids arbitrary task mutation from callbacks. Implement only supported observation/termination behavior, or collect/terminate/apply outside if formally supported.

### Xpress

Perform legal/technical binding investigation. A dedicated boundary owns generated declarations and either link-time discovery or runtime loading. Do not ship copied numeric constants without header/version generation.

## 8. Callback model

Callbacks are capability-specific, not one universal action enum.

Potential traits/events:

- progress observer;
- cancellation predicate;
- incumbent observer;
- lazy constraint provider, only for backends with a legal API;
- user cut provider, separately;
- incumbent injection, separately.

Every callback boundary:

- documents calling thread and reentrancy;
- catches Rust panics;
- validates pointers/slices;
- stores panic/error state for return after native solve;
- owns callback state through an RAII registration guard;
- never performs backend operations not explicitly permitted by vendor documentation.

## 9. Error/status model

Errors contain:

- backend name/version;
- operation;
- category (`Configuration`, `LibraryNotFound`, `AbiMismatch`, `License`, `InvalidModel`, `Numerical`, `Interrupted`, `Native`, `InternalInvariant`);
- native code/message when available;
- adapter health effect.

Solve termination separates solution availability from proof status. A time-limited feasible incumbent is not `Error`, and ambiguous infeasible/unbounded is not coerced.

## 10. Packaging and feature design

- `roml`: default features minimal; no native solver.
- `roml-highs`: depends on `roml` with `path + version` in workspace and `highs-sys` pinned.
- commercial adapters: optional independent packages, `publish = false` until qualified.
- workspace metadata centralizes version, edition, rust-version, authors, repository, license, and lints.
- crates use explicit package include lists and verify packed archives.
- docs.rs builds do not require commercial native libraries.

## 11. Verification architecture

### Core

- unit tests for every invariant;
- property tests over model mutation sequences;
- transaction rollback tests;
- snapshot/journal replay tests;
- two independent adapter cursors;
- fault-injection backend;
- public API compile tests.

### Native adapters

- initial snapshot projection;
- every operation class;
- delete/reindex sequences;
- active toggles/objective switches;
- repeated parameter updates;
- failure recovery/rebuild;
- incremental-vs-rebuild objective/solution equivalence;
- callback panic/error behavior;
- clean-host discovery diagnostics;
- platform matrix.

### Release

- package archive consumer tests;
- docs and examples;
- semver baseline;
- license/dependency/provenance checks;
- benchmark evidence.

## 12. Migration strategy

This is pre-release, so prioritize the correct contract over preserving accidental API. Still:

1. build new canonical/revision internals behind current modeling ergonomics;
2. add deprecations only where useful to downstream local code;
3. migrate HiGHS first;
4. keep commercial adapters compile-gated until migrated;
5. remove old changelog/FFI only after equivalence tests pass;
6. document all user-visible changes in CHANGELOG.

## 13. Rejected alternatives

- **Publish current core and fix later:** rejected because coefficient and journal defects can yield incorrect results/data loss.
- **Create three new sys crates immediately:** rejected; HiGHS and MOSEK already have stronger binding owners.
- **Keep a single generic callback API:** rejected because vendor callback semantics are not isomorphic.
- **Put all adapters behind root features:** rejected because native dependency resolution and commercial licensing would contaminate core package usability.
- **Add language wrappers now:** rejected because it would freeze unstable identities/errors/lifecycle.
- **Use rpath as the primary native deployment solution:** rejected as platform- and executable-context-specific; package discovery and application deployment must be distinct.

## 14. Completion criterion

The design is realized when canonical model semantics are unique and validated, synchronization is revisioned and recoverable, native bindings have authoritative ownership, the support matrix is executable, packages are clean, and release evidence demonstrates that incremental and rebuild paths commute.