# Bounded Sync-Journal Design (for review BEFORE implementation)

Owner-authorized core correction for the MPY memory gate. MPY stays
open until this lands and the 10k-cycle soak passes. Implementation
will proceed on a separate branch/PR (`fix/bounded-sync-journal`)
for independent review; no MPY Python code changes are required.

## 1. Problem

`SyncCoordinator::commit_batch` appends every `DeltaBatch` to
`Journal::batches` (`src/journal.rs:55`) with no truncation path.
Each committed change-set is retained forever so lagging adapters can
catch up. Measured: ~20 KB retained per update+solve cycle on the
MPY MPC workload (24 params + 1 bound); 203 MiB over 10k cycles.
Identical on all ROML arms (not a binding leak); direct-highspy
control is flat. The changelog already drains on commit
(`src/model/mod.rs:2356,2782`); only the journal accumulates.

## 2. Requirements (owner)

- Independent session cursors and successful-sync acknowledgements.
- Bounded retention; an idle/abandoned session must not pin history.
- Explicit snapshot rebuild when a session falls behind retained
  history.
- Preserve sync-failure recovery, model identity, immutable snapshots.
- Tests: multiple sessions, failed/partial sync, lagging rebuild,
  disposal, repeated pruning; then the original 10k soak passes.
- Prefer bounded replay + snapshot recovery (existing architecture).
- Design reviewed before implementation; separately reviewable.

## 3. Design: count-bounded replay journal

**Core change (3 lines of behavior + tests):**

1. `Journal` keeps at most the `N` most recent batches
   (`DEFAULT_JOURNAL_CAPACITY: usize = 128`). On `record`, after
   inserting, evict oldest `from`-keys while `len > N`.
2. `deltas_since(since)` returns `RevisionError::Compacted{revision:
   since}` when `since` is older than the oldest retained batch.
   (The variant exists and the docstring already promises it.) The
   current implementation is worse than an empty vec: for a `since`
   below the smallest retained key, `batches.range(since..)` returns
   the retained SUFFIX as `Ok` — prefix-truncated history presented
   as complete. On the facade path, downstream revision checks
   (`AdapterCursor::advance`, backend cursor checks, compiler
   chaining) convert the truncated suffix into a rebuild, so the
   hole is primarily the public `Model::deltas_since` API. Exact
   boundary the implementation must honor (non-empty journal):
   `since < oldest_retained` -> `Compacted`; `since` within
   `[oldest_retained, latest]` -> exact suffix (existing behavior);
   `since == latest` -> `Ok(empty)`. Empty journal + `since == ZERO`
   stays `Ok(empty)` (existing test). The unit test must cover a
   `since` strictly below the window.
3. No other core changes. In particular:
   - No session registry. Cursors stay exactly where they are:
     per-backend `revision()` read fresh each sync, ephemeral
     `AdapterCursor` per call. Independence is structural.
   - No acknowledgement protocol. Sessions advance their own
     cursors only on successful backend application (existing
     behavior, verified by tests). Pruning never consults sessions,
     so an idle/abandoned/dropped session cannot pin anything by
     construction.
   - The existing recovery path handles eviction: `facade.rs:543`
     maps `batches_for_cursor` errors to rebuild-required, and the
     caller (`facade.rs:418-428`) performs one deterministic snapshot
     rebuild. Because eviction makes this path routine (every resume
     past the window), the implementation must match on
     `RevisionError::Compacted{revision}` and include the stale
     revision (plus oldest-retained and current) in the
     `BackendError` message instead of discarding it into the
     generic text. Rebuild behavior is unchanged; this is
     diagnosability for a routine event. A session more than N batches behind a model rebuilds
     instead of replaying — correct, just slower.

**Why count-bound (not ack-pinned, byte-bound, or time-based):**

- Ack-pinned retention (keep until all live sessions acknowledge)
  lets one abandoned session pin the journal forever — explicitly
  rejected by the owner.
- Byte-bounding requires per-batch size accounting and eviction
  policy across heterogeneous batch sizes — more machinery for no
  additional correctness. Count-bounding limits catch-up work
  directly (replay cost scales with batch count) and bounds memory
  for the uniform workloads that matter; pathological single-batch
  sizes are documented as a residual (see §6).
- Time-based expiry interacts badly with idle-then-resume MPC
  patterns (a paused session would always rebuild).

**Why N = 128:** typical multi-session lag in MPC/multi-start use is
single digits; 128 gives an order of magnitude headroom while bounding
uniform workloads to ~2.5 MB at the measured 20 KB/batch. Lagging
tests use small ad-hoc capacities, not the default.

## 4. API surface

- `pub const DEFAULT_JOURNAL_CAPACITY: usize = 128` (in `journal.rs`).
- `Journal::new()` keeps current behavior with the default bound
  (all existing callers unchanged).
- `Journal::with_capacity(Option<NonZeroUsize>)` for tests/tuning
  (`None` = unbounded, preserving today's semantics explicitly where
  needed — expected to be test-only).
- No `Model`/facade API signature changes. Docstring updates are
  in scope: `Model::deltas_since` ("empty vec means no batches
  recorded") becomes actively misleading post-change — after the
  fix, empty-`Ok` unambiguously means up-to-date and `Compacted` is
  documented. No Python API changes.
- Multi-adapter independence is sequential (`commit` needs `&mut
  Model`; concurrent solves already serialize): one line in the
  implementation notes.
- `SyncCoordinator::journal` and `Journal::batches` stay `pub`
  (in-tree tests depend on it); document the at-most-N invariant on
  the fields so direct external insert is visibly a contract
  violation.

## 5. Test plan (owner bullets mapped)

Unit (`src/journal.rs`):

- record beyond capacity evicts oldest, keeps newest N, order kept.
- `deltas_since` inside window returns exact range (existing tests).
- `deltas_since` below window returns `Compacted` (new; proves the
  empty-vec hole is closed).
- `with_capacity(None)` preserves unbounded behavior.

Integration (new `tests/sync_journal_bounds.rs` or facade suite):

- **Multiple sessions**: two backends at different revisions advance
  independently; both solve correctly.
- **Failed/partial sync**: `apply_deltas` sends each compiled batch
  via a separate backend call, and each success advances the backend
  cursor — so a failure at batch k leaves the cursor at the
  intermediate revision r_k, NOT at the pre-sync revision. (Cursor-
  unmoved holds only for compile-phase failures, which never touch
  the backend, and single-call preflight rejections.) Recovery is
  still correct (next solve rebuilds from snapshot); the test must
  assert the intermediate cursor plus rebuild recovery with correct
  results, and exercise the multi-batch case explicitly as the
  realistic partial-mutation path (including across a prune
  boundary).
- **Lagging-session rebuild**: small capacity, advance model beyond
  window, stale session rebuilds and solves correctly (sync mode
  reports Rebuild honestly).
- **Disposal**: drop a lagging session, run many commits, assert
  journal length stays bounded (no pinning).
- **Repeated pruning**: commit 10x capacity across several rounds,
  assert `len <= N` continuously and deltas still serve fresh
  sessions.
- **Soak**: the original MPY 10k-cycle LP-small soak (in
  `python/benchmarks/mem_soak.py`) passes after the fix.

Existing suites (M3 sync/facade/changelog/transaction incl. the
`journal.len()` assertions at `transaction.rs:373,662,696`) must stay
green — several assert exact lengths that capacity may affect; update
only with justification, never by weakening.

## 6. Residuals (documented, not hidden)

- The bound of 128 is on history LENGTH (batch count), not bytes:
  individual batch sizes remain workload-dependent (a 100k-entity
  bulk build commits one large batch while an MPC delta is
  kilobytes). Uniform MPC workloads are byte-bounded in practice
  (~2.5 MB at the measured 20 KB/batch); byte accounting is future
  work, to be revisited only if a qualification workload exposes
  another retention problem.
- A session idle longer than the window always pays one rebuild on
  resume (correctness unaffected).
- The `Journal::batches` map and `SyncCoordinator::journal` field
  stay `pub` (no encapsulation change in this correction).

## 7. Rollback

Revert the branch. The journal returns to unbounded retention; the
soak fails again loudly (no silent behavior change anywhere).
