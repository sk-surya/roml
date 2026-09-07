# ROML Python Interface Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Independent reviewer agents are permitted at the gates described here. Steps use checkbox syntax for tracking.

**Goal:** deliver the public Python contract in DESIGN.md with measured, reliable MPC re-solves.
**Architecture:** a mixed Python/Rust package exports PyO3 wrappers over ROML's existing canonical model and HiGHS session. Rust owns expression algebra, batch validation, identity and lifecycle; Python exposes typed ergonomic entry points and NumPy bulk inputs/results.
**Tech Stack:** Rust, PyO3, rust-numpy, maturin, NumPy, pytest, mypy, existing ROML/HiGHS.
**Spec:** [DESIGN.md](DESIGN.md).

## Global constraints

- Python 3.13 and 3.14 standard CPython initially; Linux x86_64, macOS arm64, Windows x86_64 wheels. No free-threaded support claim in this milestone.
- Rust core remains Python-free and solver-free. HiGHS is the initial Python backend.
- Preserve core Rust MSRV 1.85.
- No registry publication, release tags, commercial solvers, public ROML C ABI or Marginal domain implementation.
- Names, signatures, shape rules, defaults, error classes and outcome semantics come from DESIGN.md; do not invent alternate spellings per task.
- For each task: add focused failing behavioral tests, verify the failure is meaningful, implement, pass focused tests, inspect the diff, run the phase gate, commit. No unchecked success claims or phase advancement.
- Examples below are real test seeds to copy into the named files. Expand them to the complete requirement matrix; do not treat one seed as full qualification.

## Task MPY-00 — Close prerequisites before bindings

**Files:** inspect/fix current P31 sources and tests named in PR-INTAKE; existing `.planning/phases/34-m3-qualification/34-PLAN.md` and `34-QUALIFICATION-CONTRACT.md`; update `AGENTS.md`, root/milestone state and `evidence/PR-INVENTORY.md`.
**Consumes:** current live PRs and accepted M3 contracts.
**Produces:** reviewed merged P31, P34 closure, explicit budget semantics and verified main SHA; no Python runtime code yet.

- [ ] Execute every step in [PR-INTAKE.md](PR-INTAKE.md). Start with current refs, not the observed planning head.
- [ ] Add the complete-objective regression before fixing the observed omission. Extend the existing HiGHS test by asserting the raw reported canonical objective for obj0 is 3, both at stage 0 and at the final point. Add a controlled backend without reported objective scalar: exact reconstruction or a typed failure is acceptable; omission is not.
- [ ] Run `cargo test -p roml --test objective_policy_faults` and `cargo test -p roml-highs --test objective_policy`; preserve red/green evidence with the actual head. Reconcile the no-incumbent code path with hosted evidence.
- [ ] Implement narrow fixes; run the existing phase matrices and independent review. Merge qualifying prerequisite PRs normally, then fetch main.
- [ ] Execute the complete P34 plan; do not substitute a Python smoke for its corpus/performance/packaging requirements. Merge the qualifying closure PR and update actual root state.
- [ ] Add explicit P31 options/shared-deadline hardening in a focused prerequisite commit/PR if not already integrated. Unit-test deadline exhaustion before stage 1, after stage 1, and after an operational error using a deterministic clock. Preserve last-incumbent semantics and composite cleanup failures.
- [ ] Commit evidence and normal-merge qualified prerequisite work. Record any unrelated PR as outside this milestone rather than blindly merging it.

## Task MPY-01 — Qualify the binding toolchain and package boundary

**Files:** create `roml-python/Cargo.toml`, `roml-python/src/lib.rs`, `pyproject.toml`, `python/roml/__init__.py`, `_native.pyi`, `py.typed`, `python/tests/test_import.py`; update root workspace members and lockfile; create `evidence/DEPENDENCIES.md`.
**Consumes:** qualified main, official version-specific PyO3/maturin documentation.
**Produces:** installed `roml` package with working `_native` module and version metadata, without contaminating core dependencies.

- [ ] Pin an exact compatible dependency/toolchain set in manifest constraints, Cargo lock and a Python development constraints file. Verify PyO3/rust-numpy compatibility, core MSRV and bundled HiGHS link policy.
- [ ] Add this import test and confirm it fails before the module exists:

```python
def test_native_import():
    import roml
    from roml import _native
    assert isinstance(roml.__version__, str)
    assert _native.__name__ == "roml._native"
```

- [ ] Create a minimal cdylib registration module exposing version metadata with the pinned PyO3 syntax. Use this maturin layout:

```toml
[tool.maturin]
manifest-path = "roml-python/Cargo.toml"
python-source = "python"
module-name = "roml._native"
```

- [ ] Configure `pytest` test discovery to `python/tests`; declare NumPy runtime and pytest/mypy development dependencies. Compile with `python -m maturin develop --manifest-path roml-python/Cargo.toml` in a venv, then run `python -m pytest python/tests/test_import.py -q`.
- [ ] Build a wheel with `python -m maturin build --release --locked --manifest-path roml-python/Cargo.toml --out dist` and import it from a fresh environment outside the source tree.
- [ ] Run `cargo test -p roml --all-targets --locked`, inspect `cargo tree -p roml`, and inspect `cargo package --list -p roml`. Core must still build without Python/native solvers. Review and commit the qualified skeleton.

## Task MPY-02 — Deliver the scalar API and errors

**Files:** create `roml-python/src/{model,handles,expressions,errors}.rs`; modify registrations/exports/stubs; create `python/tests/test_scalar.py`, `test_identity.py`, `test_errors.py`, `python/examples/production.py`.
**Consumes:** DESIGN scalar signatures and the core Model/Variable/Parameter/Expression APIs.
**Produces:** scalar Model, Var, Param, Constraint, Objective and affine operations; an initial synchronous solve bridge sufficient for the golden LP. MPY-04 finishes solver options/concurrency/results.

- [ ] Add this golden test and establish red behavior:

```python
import pytest
import roml as rm

def test_production_reprice():
    m = rm.Model("production")
    x, y = m.var("x"), m.var("y")
    price = m.param("price", 1.0)
    m.add(x + y <= 4.0, name="capacity")
    m.add(x <= 3.0)
    m.maximize(price * x + y)
    with rm.Highs() as solver:
        first = solver.solve(m)
        assert first.objective == pytest.approx(4.0)
        m.update(price=3.0)
        second = solver.solve(m)
        assert second.value(x) == pytest.approx(3.0)
        assert second.objective == pytest.approx(10.0)
        assert first.objective == pytest.approx(4.0)

def test_symbolic_misuse_is_loud():
    m = rm.Model()
    x = m.var("x")
    with pytest.raises(TypeError):
        bool(x <= 2.0)
    with pytest.raises(rm.UnsupportedExpressionError):
        x * x

def test_foreign_handle_rejected():
    a, b = rm.Model("a"), rm.Model("b")
    x = a.var("x")
    with pytest.raises(rm.ModelMismatchError):
        b.add(x <= 1.0)
```

- [ ] Implement owner-bearing handles and validate owner before converting to core IDs. Implement core-backed affine operators and comparison descriptors; forbid symbolic truthiness. Native resource ownership must already be safe even though full concurrency qualification comes later.
- [ ] Map documented errors and add tests for duplicate names, invalid bounds, bool/NaN/Inf coefficients, stale handles, negative indices where supported, unbounded variables via explicit bounds, binary bounds, and no implicit native work in repr.
- [ ] Complete `_native.pyi` for every public member added, including enum/error attributes. Run `python -m pytest python/tests/test_scalar.py python/tests/test_identity.py python/tests/test_errors.py -q` and type-check production.py.
- [ ] Perform an independent ergonomics read: can the golden LP be understood without compiler/FFI vocabulary? Fix confusing names before array work. Commit one coherent scalar API.

## Task MPY-03 — Add bulk arrays and real atomic updates

**Files:** create `roml-python/src/arrays.rs`; extend model/expressions; add narrow core batch helpers under `src/model/` when required; create `python/tests/test_arrays.py`, `test_updates.py`, `test_csr.py` and focused Rust batch tests.
**Consumes:** scalar ownership/algebra, existing parameter transaction semantics.
**Produces:** shaped VarArray/ParamArray/ExprArray/ConstraintArray, dot/sum, slice operations, atomic named updates and CSR insertion.

- [ ] Add the following update and shape tests before implementing batch mutation:

```python
import numpy as np
import pytest
import roml as rm

def test_rejected_batch_preserves_prior_pending_update():
    m = rm.Model()
    x = m.vars("x", 2, ub=1.0)
    p = m.params("price", [1.0, 2.0])
    m.param("unused", 0.0)
    m.maximize(rm.dot(p, x))
    with rm.Highs() as solver:
        solver.solve(m)
        m.update(price=[3.0, 4.0])
        with pytest.raises(ValueError):
            m.update(price=[8.0, 9.0], unused=float("nan"))
        assert solver.solve(m).objective == pytest.approx(7.0)

def test_array_shape_and_empty_reduction():
    m = rm.Model()
    x = m.vars("x", (2, 3), ub=1.0)
    assert x.shape == (2, 3)
    assert x[0, :].shape == (3,)
    with pytest.raises(rm.ShapeError):
        rm.dot(np.ones((3, 2)), x)
    assert rm.sum(m.vars("empty", 0)) == 0.0
```

- [ ] Before mutation, convert/copy numeric inputs to an owned validated representation. Resolve names, shape and all IDs, evaluate affected parameter-dependent coefficients for finiteness and domain validity. Only then install the batch. A full-model clone per update fails the hot-path design gate.
- [ ] Implement arrays as shape metadata plus Rust-owned typed handles/expressions, sharing scalar core semantics. Slice without copying model state; loops over coefficients remain Rust loops. Implement only scalar broadcast/equal-shape operations.
- [ ] Add runtime and typing tests for `rm.dot(price, discharge - charge)` from the flagship BESS example, including parameter-only constants inside affine expressions; reject decision-dependent coefficients multiplying decision expressions.
- [ ] Add malformed CSR tests for each specified invariant, duplicate-column accumulation, valid explicit infinities in bounds, partial-input rejection and row-count preservation. Add a round-trip CSR LP whose objective is independently known.
- [ ] Add derived-overflow tests with finite parameters producing a nonfinite coefficient; verify both prior pending values and model revision/currentness remain unchanged after rejection. If the core requires a helper, test it directly in Rust before binding it.
- [ ] Run `python -m pytest python/tests/test_arrays.py python/tests/test_updates.py python/tests/test_csr.py -q` and relevant Rust tests. Benchmark construction at 1k, 10k and 100k coefficients to detect accidental Python element loops. Commit after interface and atomicity review.

## Task MPY-04 — Complete persistent solve, outcomes and lifecycle

**Files:** create/finish `roml-python/src/solver.rs`, `solution.rs`; extend errors/stubs; add `python/tests/test_sessions.py`, `test_outcomes.py`, `test_concurrency.py`, `test_diagnostics.py`, `test_warm_start.py`; use test-only controlled Rust backends for deterministic failures.
**Consumes:** owned model state, batch APIs, qualified Rust SolverSession, native options.
**Produces:** full DESIGN solve/result contract, safe detached solves and reproducible operational errors.

- [ ] Add these outcome tests:

```python
import pytest
import roml as rm

def test_infeasible_is_a_result_not_a_fake_point():
    m = rm.Model()
    x = m.var("x", ub=1.0)
    m.add(x >= 2.0)
    m.minimize(x)
    with rm.Highs() as solver:
        result = solver.solve(m)
    assert result.status == rm.SolveStatus.INFEASIBLE
    assert not result.has_primal
    assert result.objective is None
    with pytest.raises(rm.NoSolutionError):
        result.value(x)

def test_close_is_idempotent():
    solver = rm.Highs()
    solver.close()
    solver.close()
    with pytest.raises(rm.ClosedSessionError):
        solver.solve(rm.Model())
```

- [ ] Implement try-lock ownership ordering, detached native execution and RAII cleanup. Do not call Python while locks are held. Confirm each PyO3 class's actual Send/Sync requirements from pinned documentation; no blanket unsafe trait implementation.
- [ ] Add deterministic barrier-controlled test backend coverage for a progressing Python heartbeat, mutation during solve, two solves on one model/session, independent sessions, close during solve, worker-thread drop and poisoned state. A timing-only tiny LP test does not prove GIL release.
- [ ] Add installed native LP/MIP tests for option validation, per-call override nonleak, same-solver re-solves, initial model binding, old result access, `is_current` after pending update, unknown/no-incumbent/partial-primal handling and dual validity. Do not expose test backends in release wheels.
- [ ] Implement `start=Solution` as an explicit checked request with applied/rejected disposition; cover changed revision, removed variable/domain mismatch and unsupported start capability. Return fresh immutable snapshots with owned NumPy buffers.
- [ ] Retain primary and cleanup errors in Python attributes and prevent stale result reuse after operational failure. Add deterministic fault tests at conversion, sync, solve, extraction and cleanup boundaries, with a successful ordinary solve after supported recovery.
- [ ] Run `python -m pytest python/tests/test_sessions.py python/tests/test_outcomes.py python/tests/test_concurrency.py python/tests/test_diagnostics.py python/tests/test_warm_start.py -q`, complete native/core gates, and independently review lifecycle. Commit only after concurrency and ownership contracts are evidenced.

## Task MPY-05 — Qualify the MPC application and performance

**Files:** create `python/examples/bess_mpc.py`, `python/tests/test_mpc.py`, `python/benchmarks/bench_mpc.py`, `python/benchmarks/fixtures.py`, `python/benchmarks/highspy_reference.py`; a matching Rust benchmark under `roml-highs/examples/`; evidence and raw measurements under this milestone.
**Consumes:** complete installed Python API and [QUALIFICATION.md](QUALIFICATION.md).
**Produces:** causal synthetic rolling BESS benchmark, oracle equivalence and measured performance/memory evidence.

- [ ] Turn DESIGN's battery snippet into a runnable example, then implement a causal replay that advances energy from each applied first action. Add availability/commitment updates using explicit affine rows. Keep all fixtures synthetic and deterministic.
- [ ] Independently implement the same mathematical LP/MILP using direct highspy CSR data. Use identical native versions/options and matched MPS/matrix fingerprints where representable. Validate dimensions, coefficients, bounds, signs, time units and objective offsets before comparing speed. Freeze an identical exogenous state/input stream for cross-arm timing and equivalence; separately validate closed-loop trajectories against a fresh oracle at each arm's own state, as specified in QUALIFICATION.
- [ ] Add physical checks for every replay: energy recurrence, bounds, charge/discharge exclusivity, delivery constraints, objective recomputation and declared terminal value. Compare objective/feasibility, not arbitrary alternate optimal schedules.
- [ ] Run all correctness and performance workloads in QUALIFICATION; collect cold construction, updates, synchronization, solve, extraction, rebuild and memory measurements separately. Fix bottlenecks supported by profiles; no speculative core rewrite.
- [ ] Run `python -m pytest python/tests/test_mpc.py -q` and `python python/benchmarks/bench_mpc.py --seed 20260907 --repeats 30 --steps 1000 --output benchmark-results.json` from the qualified environment. The benchmark CLI must implement these exact flags and write configuration, versions and per-sample data.
- [ ] Review thresholds and causal/accounting tests independently, commit benchmark code and evidence. A missed threshold is a blocker to completion; report its mechanism rather than silently replacing the target.

## Task MPY-06 — Ship qualified artifacts and finish

**Files:** create `.github/workflows/ci-python.yml`, `python/tests/test_installed.py`, `python/tests/typing/public_api.py`, `docs/python/README.md`; update top-level README, CHANGELOG and CONTRIBUTING; finalize stubs, packaging manifests and requirement evidence.
**Consumes:** complete public API, performance evidence, dependency pins.
**Produces:** six installed-wheel matrix cells, clean sdist rebuild, verified examples/docs and reviewable implementation PR.

- [ ] Build CPython 3.13 and 3.14 wheels for Linux x86_64, macOS arm64 and Windows x86_64. Use supported portable build images/tools; audit linked libraries and native licenses. Upload wheels and sdist as CI artifacts without registry publication.
- [ ] In each clean runner, install the wheel outside the repository; verify `roml.__file__` points inside that environment. Run installed LP/MIP, parameter update, array, exception, typing and lifecycle smokes. Editable installs cannot satisfy this gate.
- [ ] Extract the sdist into an isolated temporary directory without access to the repository. Build a wheel there, proving path-dependent ROML crates/native sources are present and correctly referenced. Audit packaged files to exclude local paths, solver credentials, commercial binaries, tests' private data and agent state.
- [ ] Run all README/doc examples directly from the installed artifact. Use `python -m mypy --strict python/tests/typing/public_api.py`; add a stub/runtime member consistency check for every public class, property and keyword-only signature.
- [ ] Run current mandatory Rust checks plus Python lint/type/tests and the complete wheel matrix at the delivered head. Use the phase's required package commands, not broad all-feature commercial backend tests that cannot be qualified without licenses.
- [ ] Request independent final review against PY-01 through PY-35. Resolve P0/P1 findings, repeat affected evidence, update STATE and create `evidence/FINAL-REPORT.md` containing exact heads, commands, artifact links, performance results, supported matrix and limitations.
- [ ] Push the implementation branch and leave its PR reviewable. Return the pinned install instructions and a small successful MPC example. Do not publish, tag, or claim production qualification beyond the evidence.
