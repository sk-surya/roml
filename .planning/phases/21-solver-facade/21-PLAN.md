# Phase 21 — Solver Façade and Unified Result

> **For agentic workers:** implement task-by-task with focused tests. Preserve the backend contract and revision/recovery invariants.

**Goal:** provide a complete `Highs::solve(&mut Model)` path backed by generic core orchestration and one user-facing result model.

**Requirements:** API-01, API-02, API-03.

## Planned file structure

Create:

- `src/solver/facade.rs` — generic `SolverSession<B>` orchestration.
- `src/solver/error.rs` — user-facing `SolveError` if not already appropriately located.
- `src/solution/metadata.rs` — `SolveMetadata` and synchronization metadata.
- `roml-highs/src/facade.rs` — `Highs` wrapper.
- `tests/solver_facade.rs` — core orchestration with reference/fault backends.
- `roml-highs/tests/facade_tests.rs` — real HiGHS end-to-end behavior.

Modify:

- `src/solver/mod.rs`
- `src/solution/mod.rs`
- `src/lib.rs`
- `src/model/mod.rs` only for core-private orchestration accessors.
- `roml-highs/src/lib.rs`

## Interfaces

```rust
pub struct SolverSession<B> {
    backend: B,
}

impl<B> SolverSession<B>
where
    B: BackendSession + SessionHealth + BackendMetadata,
{
    pub fn new(backend: B) -> Self;
    pub fn solve(&mut self, model: &mut Model) -> Result<Solution, SolveError>;
    pub fn solve_with(
        &mut self,
        model: &mut Model,
        options: SolveOptions,
    ) -> Result<Solution, SolveError>;
}
```

```rust
pub struct Highs {
    inner: SolverSession<HighsSession>,
}

impl Highs {
    pub fn new() -> Result<Self, HighsError>;
    pub fn solve(&mut self, model: &mut Model) -> Result<Solution, SolveError>;
    pub fn solve_with(
        &mut self,
        model: &mut Model,
        options: SolveOptions,
    ) -> Result<Solution, SolveError>;
}
```

## Task 1 — Define unified status and metadata

- [ ] Add exhaustive tests mapping every current `TerminationStatus` variant into `SolveStatus` or `SolveError`.
- [ ] Add tests for optimal, feasible-limit, infeasible, unbounded, interrupted, numerical, license, and backend error semantics.
- [ ] Add `SolveMetadata` fields for backend name, model revision, effective configuration, and synchronization mode (`Delta`, `Rebuild`, `NoChange`).
- [ ] Implement conversion without wildcard matches.
- [ ] Commit as `feat: normalize solve status and metadata`.

## Task 2 — Normalize backend solution data

- [ ] Add tests converting `SolveResult` and active objective metadata into `Solution`.
- [ ] Cover missing primal values, duals, reduced costs, objective identity, and objective constants of `+5`, `-5`, and `0`.
- [ ] Prove objective constant appears exactly once by comparing façade value, direct backend value, and model expression evaluation.
- [ ] Implement conversion in focused functions, not inside HiGHS-specific code.
- [ ] Commit as `feat: normalize backend results into solution`.

## Task 3 — Implement synchronization decision logic

The solve algorithm is:

```text
1. model.commit(); fail before backend mutation on error.
2. inspect backend health/revision.
3. terminal -> return SolveError without retry.
4. requires rebuild or missing delta chain -> snapshot rebuild.
5. ready and behind -> apply sequential delta batches.
6. ready and current -> no synchronization.
7. on recoverable/dirty synchronization failure -> one snapshot rebuild attempt.
8. solve exactly once after successful synchronization.
9. normalize result and attach metadata.
```

- [ ] Write tests using reference and fault-injecting backends for every branch.
- [ ] Assert at most one rebuild retry.
- [ ] Assert backend revision equals committed model revision before solve.
- [ ] Assert any prior solution is invalidated after mutation.
- [ ] Implement `SolverSession<B>` inside core so it can access model-private coordinator state.
- [ ] Commit as `feat: orchestrate model synchronization in core`.

## Task 4 — Add solve options façade

- [ ] Introduce or alias `SolveOptions` over the existing immutable request contract.
- [ ] Provide builders `time_limit(Duration)`, `relative_gap(f64)`, `absolute_gap(f64)`, `threads(i32)`, `output(bool)`, `random_seed(i32)`, and `backend_option(key, value)`.
- [ ] Validate non-negative duration/gaps and positive threads before synchronization.
- [ ] Preserve effective configuration and adjustments/rejections in metadata.
- [ ] Commit as `feat: add ergonomic solve options`.

## Task 5 — Add HiGHS façade

- [ ] Write a failing test for `Highs::new`, `solve`, and `solve_with`.
- [ ] Implement `Highs` as a thin wrapper; do not duplicate synchronization logic.
- [ ] Keep `HighsSession` exported under the advanced/backend surface.
- [ ] Add rustdoc showing the complete quickstart.
- [ ] Commit as `feat(highs): add user-facing solver facade`.

## Task 6 — End-to-end repeated solve and recovery

- [ ] Test first solve from a new model.
- [ ] Test no-change second solve.
- [ ] Test bound delta second solve.
- [ ] Test parameter-driven coefficient delta second solve.
- [ ] Test objective switch.
- [ ] Test rebuild-required recovery.
- [ ] Test failed option validation leaves model/backend state unchanged.
- [ ] Test terminal backend state returns error without loop.
- [ ] Commit as `test: qualify facade repeated solve semantics`.

## Verification

```bash
cargo fmt --all -- --check
cargo clippy -p roml -p roml-highs --all-targets -- -D warnings
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc -p roml -p roml-highs --no-deps
```

## Gate

P21 passes when the target solve and incremental fixtures compile and execute, all backend/protocol suites remain green, objective offsets are proven correct, and independent protocol review approves recovery semantics.