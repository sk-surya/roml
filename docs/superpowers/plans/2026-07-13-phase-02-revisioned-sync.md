# Phase 02 Plan — Revisioned, Recoverable Synchronization

> This is the architectural critical path. Freeze the P1 canonical model contract before implementation. Use a solver-neutral reference backend and fault injection before migrating native adapters.

**Goal:** replace destructive single-consumer change draining with revisioned snapshots, typed delta batches, independent adapter cursors, transactions, and deterministic recovery.

**Requirements:** R3.*, R4.1, R8.1.

## Task 2.1 — Characterize current synchronization failures

**Files:** `tests/sync_characterization.rs`.

Create failing tests proving:

- drained changes disappear when adapter apply returns an error;
- a second adapter cannot observe changes consumed by the first;
- partial application leaves no deterministic recovery path;
- adjacency-dependent add-row batching can be broken by legal event interleaving;
- reset/rebuild behavior is not tied to a model revision.

Use a fake adapter that fails after operation `k` and records applied operations.

## Task 2.2 — Define revision/snapshot/delta types

**Files:** create `src/revision.rs`, `src/snapshot.rs`, `src/delta.rs`; revise exports.

Implement opaque `ModelRevision`, `ModelSnapshot`, `DeltaBatch`, and typed aggregate `ModelOp` values. Requirements:

- `from < to` for non-empty committed batches;
- operations are deterministic and self-contained enough for adapters;
- add-row/add-objective operations carry canonical cells rather than relying on adjacent events;
- parameter updates compile into evaluated cell changes;
- snapshots include all active state and enough inactive state if reactivation semantics require it;
- revision overflow has a defined error rather than wrapping silently.

Add rustdoc invariants and unit tests for construction/ordering.

## Task 2.3 — Make model mutations transactional

**Files:** create/refactor `src/transaction.rs`, model mutators, tests.

1. Implement an internal staging transaction.
2. Validate all staged changes against final state.
3. Commit canonical state and one delta batch atomically.
4. On error/panic-safe boundary, leave state, revision, journal, caches, and indices unchanged.
5. Decide single-operation convenience methods: each is one transaction/revision or uses an explicit batch API.
6. Add bulk model construction/update API to avoid one revision per scalar when callers need atomic batches.

Tests: rollback at every validation point, deletion cascades, duplicate cell normalization, empty transaction, nested transaction policy.

## Task 2.4 — Implement journal and cursor semantics

**Files:** create `src/journal.rs`, `src/sync.rs`.

1. Store committed `DeltaBatch` values by revision.
2. Provide `deltas_since(revision)` with typed errors for future/compacted revisions.
3. Implement `AdapterCursor { applied_revision, health }`.
4. Define `AdapterHealth::{Ready, RequiresRebuild, Terminal}`.
5. Do not compact initially; document memory tradeoff.
6. Provide model-owned or session-owned synchronization coordinator without granting adapters mutable access to model internals.

Tests: two cursors advancing independently, lagging adapter, no-op catch-up, invalid cursor, retained replay.

## Task 2.5 — Define apply outcomes and recovery

**Files:** solver-neutral traits/errors under `src/solver/` or a new backend module.

Define outcomes:

- fully applied and acknowledged;
- operation unsupported incrementally, requires rebuild without corruption;
- recoverable failure with backend unchanged;
- dirty partial failure requiring rebuild;
- terminal session failure.

Synchronization algorithm:

1. read cursor revision;
2. obtain batches;
3. apply in order;
4. advance cursor only after full batch acknowledgement;
5. on `RequiresRebuild`/dirty failure, build current snapshot and call rebuild;
6. set cursor to snapshot revision only after rebuild succeeds;
7. preserve journal regardless of adapter result.

Test every transition with fault injection.

## Task 2.6 — Build reference projection backend

**Files:** create `src/solver/reference.rs` or a dev/test support crate if it should not be public.

The backend stores variables, rows, cells, objectives, activity, and revision but does not optimize. It must support all canonical operations and expose a normalized state view.

Use it to test the commuting square:

```text
project(snapshot r1) == project(snapshot r0); apply(deltas r0->r1)
```

Generate arbitrary P1-valid mutation sequences, randomly split them into synchronization intervals, and compare state after each interval.

## Task 2.7 — Add snapshot rebuild and compaction hooks

1. Implement deterministic snapshot projection order independent of hash iteration.
2. Add explicit adapter rebuild API.
3. Add journal metrics and a future compaction API contract, but do not optimize retention prematurely.
4. Define what happens when a cursor references compacted history: rebuild from current snapshot.
5. Benchmark snapshot generation and delta retrieval separately.

## Task 2.8 — Migrate public extension API

Replace `sync_model` destructive drain semantics with a session/coordinator API. Provide a temporary deprecation shim only if local downstream users need migration time; it must delegate to safe revision semantics.

Update docs/examples/CHANGELOG. Remove old `ChangeLog::drain` from public use and eventually delete the old primitive event representation after native migrations.

## Verification

```bash
cargo fmt --all -- --check
cargo clippy -p roml --all-targets -- -D warnings
cargo test -p roml --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
```

Run generated tests with:

- at least two independent cursors;
- random fault point injection;
- random sync interval partitioning;
- transaction rollback;
- snapshot rebuild after dirty failure.

**Phase gate:** no model operation is lost on adapter failure; multiple cursors synchronize independently; all generated incremental projections equal snapshot projections; native adapters can now target a frozen revision contract.