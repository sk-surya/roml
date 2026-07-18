---
gsd_phase_number: M1R-02
name: highs-projection-session-rewrite
milestone: ROML-M1R
goal: Implement the frozen backend session contract safely against authoritative HiGHS bindings.
dependencies: [M1R-01]
parallelism: binding/lifecycle, projection, solve/extraction, and documentation lanes may run in parallel after interface freeze
---

# M1R-02 — HiGHS Projection and Session Rewrite

## Scope
This phase converts `roml-highs` from a legacy `Change`-driven adapter into the mandatory v0.1 implementation of the frozen snapshot/delta/session contract. Existing candidate code is evidence and may be reused only after M1R-00 disposition.

## Target file boundaries
The implementation plan should prefer focused modules such as:
- `roml-highs/src/bindings.rs` or `ffi.rs`: narrow re-export/wrapper over `highs-sys`; no copied ABI.
- `roml-highs/src/error.rs`: native status/error classification.
- `roml-highs/src/lifecycle.rs`: construction, ownership, destruction, version/index-width checks.
- `roml-highs/src/projection.rs`: snapshot rebuild and model operation projection.
- `roml-highs/src/session.rs`: cursor, health, synchronize, request negotiation, solve.
- `roml-highs/src/solution.rs`: status and solution extraction.
- `roml-highs/src/callback.rs`: only officially supported callbacks for the pinned version.
- `roml-highs/src/index_map.rs`: dense index bookkeeping with invariant tests.

Do not force this exact split if current code supports a clearer bounded structure, but avoid a monolithic adapter owning ABI, model projection, solve policy, callbacks, and extraction.

## Tasks
### 02.1 Binding authority and feature topology
- Pin an exact compatible `highs-sys` range/version and record upstream commit/header compatibility.
- Verify every required symbol is exposed by the maintained binding.
- Remove handwritten function declarations, structs, callback layouts, enum/control/status values, and duplicate `links` ownership.
- Define bundled/static default and optional system-discovery feature behavior.
- Ensure docs/core builds do not require a system HiGHS installation.

### 02.2 Fallible lifecycle
Implement typed construction errors for:
- native create failure;
- unsupported index width;
- ABI/version mismatch;
- option validation/configuration failure;
- unavailable system library in discovery mode.

`new()` may return `Result`; a convenience panic constructor, if retained, must be explicitly named and documented as such. Check destruction, reset/rebuild, double-free prevention, null handles, and partial-construction cleanup.

### 02.3 Thread-safety and unsafe audit
- Remove unconditional `unsafe impl Send` unless HiGHS documentation and wrapper ownership justify it.
- Never implement `Sync` without explicit proof.
- Document safety invariants adjacent to every unsafe block.
- Check pointer/length/index conversions, CString creation, callback userdata, and returned array capacities.
- Check every HiGHS return code; no ignored option-set or extraction failures.

### 02.4 Snapshot projection
Implement deterministic full rebuild for:
- variables and coherent domains;
- bounds and activity;
- constraints, ranged rows, and activity;
- sparse matrix coefficients;
- active objective, sense, costs, and offset;
- empty/objectiveless models;
- parameter-resolved canonical cells.

Projection either completes and acknowledges the snapshot revision or leaves the session classified for deterministic retry/rebuild.

### 02.5 Delta application
Map every admitted `ModelOp` to a typed native operation. For multi-call operations:
- prevalidate where possible;
- identify the first mutation boundary;
- classify failures as retryable vs rebuild-required;
- acknowledge only after complete success;
- preserve the source delta for replay.

Handle dense index changes after add/remove/deactivate/reactivate and objective switching. Batch only typed operations with explicit ordering guarantees.

### 02.6 Solve request negotiation
Map generic request fields to HiGHS controls with explicit outcomes:
- algorithm;
- time/iteration/node limits;
- threads;
- presolve;
- output/logging;
- MIP gap/tolerances where admitted;
- basis/hot-start requests;
- required solution attributes.

Return requested and effective configuration. Invalid keys/values and unsupported policy must reject; no best-effort silence.

### 02.7 Solve and extraction
- Preserve HiGHS native model/run status distinctions.
- Represent optimal, feasible incumbent, infeasible, unbounded, infeasible-or-unbounded, interrupted, limits, numerical, and backend failures accurately.
- Extract objective with ROML offset semantics exactly once.
- Expose primal values and, when valid, duals/reduced costs/basis through non-cloning views or bounded owned snapshots defined by M1R-01.
- Invalidate stale solution state after model mutation or failed solve.

### 02.8 Callback disposition
Inventory official callback support for the pinned HiGHS version. Implement only legal progress/interruption/incumbent behavior. Reject lazy constraints/user cuts/incumbent injection unless officially supported and tested. Catch Rust panics before crossing C and perform deterministic cleanup.

### 02.9 Compatibility and migration
Update public examples and migration docs. If a legacy `HighsAdapter` name is retained, make it the safe session implementation or a thin safe wrapper—not a second semantic path.

## Focused verification
```bash
cargo test -p roml-highs --all-targets
cargo clippy -p roml-highs --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps
cargo package -p roml-highs --locked
rg -n 'extern "C"|unsafe impl (Send|Sync)|assert!|unwrap\(|expect\(' roml-highs
```
Every match receives disposition in the evidence report.

## Gate
- M1R-H1–H8 pass.
- No handwritten ABI survives.
- Safe contract is implemented end-to-end.
- Unsafe/native review has no unresolved P0/P1 issue.
- M1R-03 receives a native backend fixture implementing the common conformance harness.
