# MPY Performance Notes (living; finalized in MPY-05)

## MPY-03 bulk construction (2026-09-07, Linux x86_64, 100k-coefficient fixture)

Decomposition (seconds, 100k vars + 10k rows x 10 coefs):

| Arm | vars | rows | total |
|---|---|---|---|
| Bulk (`vars` + CSR, NumPy inputs) | 0.052 | 0.371 | ~0.43 |
| Scalar loop (`m.var` + expr `m.add`) | 0.162 | ~0.38 | ~0.55 |

Findings:

- Bulk `vars()` is 3.1x faster than the per-element `m.var` loop.
- Bulk CSR matches or beats scalar adds per row (5us vs 6us at 1 coef/row).
- Per-coefficient rate: ~3.7us bulk vs ~8.5us scalar (2.3x).
- End-to-end fixture ratio is ~1.3x because ~85% of wall time is identical
  core entity-insertion work (changelog, validation, coefficient index) in
  both arms — not interpreter overhead.
- No Python element loop exists on the bulk path (2 extension calls build
  100k variables; 1 call inserts 100k CSR coefficients).

## MPY-05 gate risk

QUALIFICATION requires >=3x end-to-end on this fixed fixture shape. With
core per-entity costs identical in both arms, 3x is unachievable by wrapper
work alone; the gap is a fixture/threshold property, not a wrapper defect.
Carried as an at-risk item into MPY-05 with this mechanism documented. No
speculative core batch API is added to chase it.

## MPY-05 memory soak (2026-09-07): FAILING — unbounded journal retention

Memory-soak gate (10k update/solve LP-small cycles, retain ≤ max(32 MiB,
10% post-warmup)): **FAIL. Growth ≈ 20 KB/cycle (≈ 203 MiB over 10k).**

Isolation experiments (all post-gc current RSS):

- update-only (2k changing updates, no solves): 0.0 MB.
- solve-only (2k solves, no updates): 0.0 MB.
- const-value updates + solves (2.2k cycles): 0.0 MB.
- changing updates + solves: linear ≈ 20 KB/cycle (Rust arm too:
  ~254 MB mid-flight at ~10k gates; highspy persistent: 0.0 MB/2.5k).

Mechanism: only *committed changes* grow. `Model::commit` drains the
changelog (`model.rs:2356,2782`) but `SyncCoordinator::commit_batch`
appends every `DeltaBatch` to `Journal::batches` (`src/journal.rs:55`)
with no truncation path anywhere in-tree. Each gate's parameter/bound
delta is retained forever so lagging adapters can catch up
(`batches_for_cursor`). No prune-on-acknowledgement protocol exists:
sessions never report cursors back to the model.

This is M3-core behavior (identical on all ROML arms; not wrapper
overhead and not a binding leak), but it breaks the MPY-05 memory gate
as specified. Deliberately NOT worked around (e.g. periodic model
rebuilds would fake the persistent-model leak test) and NOT fixed by a
hasty core change (cursor-acknowledged pruning is an architectural
protocol touching the multi-adapter invariant). Escalated for owner
disposition: amend the threshold, authorize a journal-pruning design,
or accept periodic session/model recycling with explicit semantics.

## MPY-05 canonical numbers (release wheel, quiet host, 2026-09-07)

MPC matched MILP (1000 gates x 30 reps, threads=1, limit 2.0s):

| Arm | p50 ms/gate |
|---|---|
| Python ROML persistent | 4.45 |
| Python ROML fresh rebuild | 5.99 (+33%: state-reuse benefit) |
| Direct highspy persistent | 3.97 |
| Direct Rust ROML persistent | 3.98 |

Wrapper overhead on this workload: ~0.55 ms/gate (~14% over direct
highspy). Gate-by-gate objectives agree exactly across arms.

LP-small (relaxation): 0.64 ms/gate (Python persistent).

LP-scale (100x96, 9,600 cells/gate, release):

| Arm | update | extract | non-solve | native solve |
|---|---|---|---|---|
| Python persistent | 2.82 | 0.13 | ~2.95 | ~475 |
| Direct Rust | 0.23 | 0.12 | ~0.35 | ~482 |

PY-27 gate: 2.95 <= 1.5 * 0.35 + 5 = 5.53 ms. PASS.

IMPORTANT METHODOLOGY NOTE: all pre-release Python timings in this
file's MPY-03 section were measured on debug (`maturin develop`)
builds, which run ~10x slower than release on validation-heavy paths
(26 ms vs 2.8 ms for the LP-scale update). Canonical qualification
numbers above are release-wheel only.
