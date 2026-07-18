---
gsd_phase_number: M1R-03
name: native-differential-fault-qualification
milestone: ROML-M1R
goal: Prove HiGHS incremental correctness, failure recovery, and solve semantics against the common contract.
dependencies: [M1R-02]
parallelism: semantic families and fault campaigns may run independently; verifier owns final reconciliation
---

# M1R-03 — Native Differential and Fault Qualification

## Testing principle
ReferenceBackend is the executable model of the solver-neutral projection contract. HiGHS is compared to canonical snapshots, expected normalized state, and solve observables. Matching a previous native implementation is not sufficient.

## Harness architecture
Create a backend fixture/factory interface allowing the same scenario definitions to run against:
- ReferenceBackend;
- HiGHS rebuilt from snapshot;
- HiGHS incrementally synchronized;
- fault-injected HiGHS session.

Each failure artifact records backend version/build, OS/architecture, seed, initial snapshot hash, operations, revisions, request, expected/actual normalized observations, and adapter health.

## Tasks
### 03.1 Operation coverage matrix
Define one mandatory scenario family for every admitted operation and transition:
- add/remove/reactivate/deactivate variable;
- bounds and domain transitions;
- add/remove/reactivate/deactivate constraint;
- ranged-row changes;
- canonical coefficient add/change/remove;
- parameter-propagated cell change;
- objective add/remove/switch/sense/cost/offset;
- empty and objectiveless states;
- transaction batches and revision gaps.

No operation is “covered” only because it appears in a random generator.

### 03.2 Generated mutation traces
- Generate legal traces from a model-state-aware generator.
- Include deterministic seed corpus and randomized scheduled runs.
- Compare incremental session at every revision to a fresh snapshot rebuild.
- Shrink failures to minimal traces.
- Persist regression traces in a text/JSON format stable enough for tests, without freezing internal arena IDs as public serialization.

### 03.3 Fault-injection matrix
For each native projection composed of multiple calls, inject failure:
- before mutation;
- after each native sub-call;
- before acknowledgement;
- during rebuild;
- during option application;
- during solve;
- during solution extraction.

Assert expected adapter health, cursor revision, replay availability, stale-solution invalidation, and recovery route. Dirty native state must never be reported Ready.

### 03.4 Multi-session independence
Attach at least two sessions to one journal. Verify:
- independent lag and catch-up;
- one session failure does not advance another cursor;
- one session rebuild does not compact history needed by another;
- request/configuration state is session-local;
- divergent capabilities produce explicit rejection without corrupting model history.

### 03.5 Semi-continuous and domain regressions
Promote the historical partial-apply sequence to mandatory native tests. Cover unsupported domains, transition ordering, lower-bound semantics, deactivate/reactivate, and rebuild. Reject unsupported operations before mutation whenever feasible.

### 03.6 Status and error conformance
Construct or use fixtures for:
- optimal LP/MIP;
- feasible incumbent at time/node/iteration limit;
- infeasible;
- unbounded;
- ambiguous infeasible-or-unbounded;
- user interruption;
- numerical failure;
- invalid request;
- native internal/load/version failure.

Verify native code/category, operation, recoverability, incumbent availability, and proof state.

### 03.7 Solution and objective conformance
Check objective offset exactly once, active objective switching, primal indexing, dual sign conventions, reduced costs, unavailable MIP duals, stale-view behavior, and basis/hot-start validity after structural versus parametric changes.

### 03.8 Request negotiation
For every admitted generic option, test applied, adjusted, and rejected outcomes. Validate reproducibility metadata and ensure failed negotiation precedes native mutation/solve.

### 03.9 Independent verification
The verifier:
- did not author HiGHS implementation;
- selects additional seeds and fault sites;
- manually reviews at least one trace per operation family;
- reviews tolerance choices and false-equivalence risk;
- confirms ignored/skipped counts are zero for mandatory tests.

## Commands
```bash
cargo test -p roml --test backend_contract -- --nocapture
cargo test -p roml --test differential_harness -- --nocapture
cargo test -p roml --test semicontinuous_recovery -- --nocapture
cargo test -p roml --test status_negotiation_tests -- --nocapture
cargo test -p roml-highs --all-targets -- --nocapture
```
Add exact backend-enabled commands defined by the implementation; do not claim common-harness coverage if tests only instantiate ReferenceBackend.

## Gate
- M1R-Q1–Q5 pass.
- Every admitted operation has focused and generated evidence.
- Fault injection loses no replayable history.
- HiGHS incremental and rebuild paths commute observationally.
- Independent verifier signs the evidence report with no unresolved blocker.
