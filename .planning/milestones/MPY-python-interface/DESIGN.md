# Python Interface Design

## 1. Product contract

An operations researcher must be able to read a mathematical model directly in Python, build it once, update named parameters in one call, and solve repeatedly without managing compiler revisions or native pointers. Performance is measured end to end, not inferred from Rust or the presence of a persistent solver object.

The public API has one spelling for each ordinary action: `var`, `vars`, `param`, `params`, `add`, `minimize`, `maximize`, `update`, `solve`, `value`, and `values`. No parallel builder DSL, dictionary DSL, and matrix DSL with competing semantics. A narrow sparse-row entry point serves large inputs through the same core.

```python
import roml as rm

m = rm.Model("production")
x = m.var("x", lb=0.0)
y = m.var("y", lb=0.0)
price = m.param("price", 1.0)
m.add(x + y <= 4.0, name="capacity")
m.add(x <= 3.0)
m.maximize(price * x + y)

with rm.Highs(threads=1, output=False) as solver:
    first = solver.solve(m)
    m.update(price=3.0)
    second = solver.solve(m, time_limit=1.0)
    assert second.is_optimal
    assert second.value(x) == 3.0
    assert second.objective == 10.0
```

`solve` implicitly commits and synchronizes. A successful `update` changes the pending parameter state; the next solve commits the batch. Existing immutable solutions continue to describe their original model revision. No promise that old solutions are current after a mutation.

## 2. Public signatures and behavior

The signatures below are the contract. Type aliases used here: `Shape = int | tuple[int, ...]`, `Numeric = float | int`, `NumericInput` is a real scalar, rectangular real sequence or numeric NumPy array. `ValueInput` also admits model-owned scalar/array parameters and parameter-only expressions. `Affine` admits variables and affine expressions whose coefficients are parameter-dependent. Booleans are not accepted as numeric values.

| Surface | Contract |
|---|---|
| `Model(name: str = "")` | Owns one Rust model, lineage/instance identity and namespace registry |
| `m.var(name, *, lb=0.0, ub=inf, kind="continuous")` | Returns `Var`; `kind` is `continuous`, `integer`, or `binary` |
| `m.vars(name, shape, *, lb=0.0, ub=inf, kind="continuous")` | Returns `VarArray` in C order; scalar or exact-shape numeric bounds |
| `m.param(name, value)` | Returns scalar `Param` |
| `m.params(name, values)` | Returns `ParamArray`; shape inferred once and immutable |
| `m.add(comparison, *, name=None)` | Returns `Constraint` or shape-preserving `ConstraintArray` |
| `m.minimize(expr)` / `m.maximize(expr)` | Creates/selects a scalar canonical objective and returns `Objective` |
| `m.update(**values)` | Keyword names address registered parameters/parameter arrays; validates the entire batch before changing pending state |
| `rm.sum(expr_array)` | Scalar reduction implemented in Rust; empty reduction is numeric zero |
| `rm.dot(coefficients, expressions)` | Scalar dot product of identical-shape arrays, flattening C order; numeric/parameter-only coefficients multiplied by `VarArray` or affine `ExprArray` |
| `m.add_linear_rows(indptr, indices, data, *, variables, lower, upper, name=None)` | CSR bulk rows, numeric coefficients, explicit flattened `VarArray` column map, returns 1-D `ConstraintArray` |
| `Highs(*, threads=1, output=False, time_limit=None, relative_gap=None, absolute_gap=None, random_seed=None)` | Persistent, model-bound native session; explicit keyword-only options |
| `solver.solve(model, *, time_limit=None, relative_gap=None, absolute_gap=None, random_seed=None, start=None)` | Returns `Solution`; provided overrides apply to this call only; omitted values inherit constructor defaults |
| `solver.close()`; context manager | Idempotent native cleanup; subsequent solve raises `ClosedSessionError` |
| `solution.value(var)` | Returns float or raises `NoSolutionError` / `MissingValueError`; never defaults to zero |
| `solution.values(var_array)` | Owned NumPy float64 result, same shape/order; no view into live native buffers |
| `solution.dual(constraint)` / `duals(constraint_array)` | LP evidence only; unsupported/unavailable duals raise `UnavailableDiagnosticError` |
| `solution.reduced_cost(var)` | Same diagnostic policy as duals |
| `solution.is_current(model)` | Compares instance/revision and pending-mutation state; false after any successful mathematical edit until solved again |

Options must be finite and in valid domains. `time_limit` is seconds, positive when supplied; `None` in a call means inherit, not clear an existing constructor limit. A differently configured solver clears defaults. Do not silently accept unknown keywords or ignore unsupported features. Symbolic bounds are expressed through `m.add(x <= capacity)` in MPY; variable bound arguments are numeric.

Binary variables have effective bounds intersected with [0,1], so omitted bounds
produce the standard binary domain; an empty intersection rejects. Shape objects
expose `.shape` as a tuple. Bounds allow negative infinity only as lower and
positive infinity only as upper; parameter/coefficient values must be finite.
Array numeric inputs are copied into owned Rust buffers before detached work, so
mutating a NumPy input later cannot race a solve or alter a stored parameter.

All registered variable and parameter names are nonempty strings, globally unique at the Python model level. Array elements have deterministic diagnostic names `charge[0]`, `charge[1]`; namespace collisions are rejected before mutation. Parameter names need not be Python identifiers: `m.update(**{"market-price": values})` remains supported. Duplicate names cannot alias different entities. Constraint names are also unique when explicitly supplied. Reject name/type errors with the offending name and expected type.

## 3. Expressions and arrays

Support `+`, `-`, unary minus, scalar multiplication, parameter-dependent coefficients, and `<=`, `>=`, `==`. Variable×variable expressions raise `UnsupportedExpressionError` at construction with an explanation that the interface is LP/MILP. Division is supported only by nonzero numeric constants in this milestone. No implicit quadratic lowering. Evaluate and validate parameter-only nonlinear coefficient expressions through the existing core contract, never a second Python evaluator.

`bool(symbolic_expression)` and `bool(symbolic_comparison)` raise `TypeError`, with examples explaining `m.add(...)`; chained comparisons such as `0 <= x <= 1` fail loudly. Variables are unhashable because symbolic `==` does not define Python key equality; use `solution.value(x)` instead of a handle-keyed dictionary. No implicit solve in `repr`, property access, or NumPy conversion. Repr is bounded and descriptive.

Arrays support positive finite-rank shapes, C-order indexing, integers/slices/ellipsis, and scalar broadcasting only. Array/array operations require equal shapes; do not silently implement NumPy trailing-dimension broadcasting. Slices are lightweight immutable handle collections retaining ownership, not copies of model state. `sum` reduces all dimensions; axis reductions and labeled/pandas/xarray indexing are deferred. Empty arrays are allowed with well-defined empty sums and no native calls for empty batches. Ragged arrays, object/complex/bool dtypes, shape mismatch, and out-of-range indices raise typed errors. Noncontiguous numeric input is accepted by one deliberate contiguous copy. `rm.dot` is the documented fast route; NumPy `@`/ufunc interoperability is not promised.

`rm.dot(price, discharge - charge)` is explicitly supported. Its left input
must be numeric or parameter-only; its right input may contain affine decision
expressions, including parameter-dependent coefficients. The result stays affine
in decision variables. Two decision-dependent inputs reject as nonlinear. Apply
the same rule to elementwise multiplication of arrays.

Expression arrays are Rust-owned structures, not NumPy object arrays of one Python object per coefficient. Python arithmetic may invoke one extension call per vector operation; loops over elements must execute in Rust. Scalar examples stay convenient for small models. Large models use array operations or CSR rows.

CSR validation: integer `indptr`/`indices`, `indptr[0]=0`, monotone pointers, last pointer equals length of data and indices, column indices within flattened `variables`, finite data, one lower/upper value per row (or scalar broadcast), valid infinities on bounds, lower ≤ upper. Sum duplicate entries algebraically; never last-write-wins. Validate before inserting any row. Shape or stale-handle failure must leave all rows unadded.

## 4. Layout and authority boundaries

| Path | Responsibility |
|---|---|
| `roml-python/Cargo.toml`, `src/lib.rs` | New workspace binding crate; `cdylib` named `_native`; registrations only in lib.rs |
| `roml-python/src/model.rs` | Python model owner and atomic mutation boundary |
| `roml-python/src/handles.rs` | Owner-bearing entity handles and identity checks |
| `roml-python/src/expressions.rs`, `arrays.rs` | Rust expression/array wrappers, shape validation, bulk execution |
| `roml-python/src/solver.rs`, `solution.rs` | Session lifecycle, detached solves, immutable result conversion |
| `roml-python/src/errors.rs` | Stable Python error mapping, structured error attributes |
| `python/roml/__init__.py`, `_native.pyi`, `py.typed` | Small exports, complete typing, package marker |
| `python/tests/`, `python/examples/`, `python/benchmarks/` | Public behavior, runnable examples, reproducible benchmarks |
| `pyproject.toml` | maturin manifest path, mixed project layout, dependencies and pytest/type-check configuration |
| `.github/workflows/ci-python.yml` | Build/test standard CPython matrix, wheel/sdist artifact qualification |

Rust core `Cargo.toml` gains no PyO3/NumPy/native dependencies. Workspace membership must not make `cargo test -p roml` need Python or HiGHS. Keep root Rust package include filters intact. Native extension links the existing `roml` and `roml-highs` crates; it neither calls raw HiGHS independently nor recreates synchronization policy.

Use `module-name = "roml._native"`, `python-source = "python"`, and `manifest-path = "roml-python/Cargo.toml"` in maturin configuration. Distribution name is provisionally `roml`; inspect registry ownership at dependency qualification, but do not publish or claim it is available. If unavailable, use distribution `roml-python` while retaining `import roml`, and record the packaging decision. NumPy is a required runtime dependency for bulk results; scipy is optional and not required for raw CSR arrays.

## 5. Ownership and concurrency

Model owns canonical state behind a safe synchronization primitive; handles carry an owner reference and typed Rust identity. Never export raw integer IDs as usable authority. A handle keeps its model alive. Results own copied values and frozen provenance, including an identity membership map so an old result can still answer for its original handle after later edits. A foreign-model handle always fails, even if slot/generation numbers coincide.

Prefer frozen PyO3 classes with interior mutability and safe `Send` composition. Reuse the existing documented `Send` of `HighsSession`; do not invent `Sync` or add `unsafe impl` to satisfy PyO3. Audit all fields before selecting the pinned PyO3 version. `Mutex<HighsSession>` can protect a Send-only session without claiming the native session itself is Sync.

Before solve: convert Python inputs to owned Rust data while attached, then pass
only Send-safe owned state references/data into the detached closure. Inside
that closure, acquire session then model locks with nonblocking `try_lock`,
revalidate ownership/state under the locks, synchronize and solve, and release
all guards before returning. Never capture a `std::sync::MutexGuard` across the
PyO3 detach boundary; those guards are not Send. Construct Python results only
after reattachment. Contention returns `ModelBusyError` or `SessionBusyError`
rather than waiting while holding the GIL. Mutations and a second solve against
busy state fail deterministically; separate models and sessions may run
concurrently. The acquired locks define the solve's linearization point, so an
edit completed before acquisition is included rather than racing extraction.
No Python callbacks while model/session locks are held. Poisoned state is an
operational error and cannot be silently reused. Guard-based cleanup must cover
every exception/error path.

A solver binds to its first model and rejects another model before mutation (`ModelMismatchError`). This simple Python contract holds even if the Rust facade later supports model switching. `close` while solving raises `SessionBusyError`; native destruction occurs exactly once after ownership is released. Test worker-thread creation/use/drop, overlapping same-model calls, independent sessions, and a progressing Python heartbeat during solve.

MPY supports standard GIL-enabled CPython only. Free-threaded/subinterpreter support and process pickling of live handles are explicitly unsupported; no blanket thread-safety claim. Process workers construct their own sessions after process start.

## 6. Atomic updates and failure semantics

`m.update` validates every name, owner, shape, finite input and evaluated affected coefficient/domain before altering pending state. An invalid later element cannot leave earlier updates installed; previously pending updates survive rejection. Success installs the whole batch without cloning the entire model on the hot path. Where the existing scalar setter cannot guarantee this, add a narrow Rust batch-preflight/apply API and tests; do not fake atomicity in Python or silently relax it. Validate derived overflow, not just finite inputs. `m.add(array)` and CSR additions likewise preflight before mutation; bulk structural staging may use a safe temporary representation before insertion, with its cost measured separately.

Expose a `RomlError` hierarchy: `InvalidModelError`, `InvalidHandleError`, `ModelMismatchError`, `ShapeError`, `UnsupportedExpressionError`, `UnsupportedFeatureError`, `ModelBusyError`, `SessionBusyError`, `ClosedSessionError`, `SolverError`, `NoSolutionError`, `MissingValueError`, `UnavailableDiagnosticError`. Input errors also inherit the appropriate Python `ValueError` or `TypeError` where practical. Attach stable `code` and relevant `name`, `expected_shape`, `actual_shape`, `backend`, `primary`, `cleanup`, `requires_rebuild` attributes when applicable. Do not parse human error strings to classify failures.

Mathematical outcomes return a `Solution`; operational failures raise exceptions retaining primary and cleanup failures. `Solution.status` is a `SolveStatus` enum, with separate `has_primal` and `is_optimal`. Feasible incumbent validation must not infer feasibility from one finite value. Infeasible, unbounded, limits without incumbents, and limits with incumbents have distinct tested behavior. An unknown mathematical outcome must retain its meaning; an uninterpretable backend failure must not be relabeled feasible. Existing core contracts govern any narrowly necessary reconciliation.

Solution exposes `objective`, `best_bound`, `relative_gap` as optional floats, `status`, `has_primal`, `is_optimal`, `metadata`, and accessors above. Metadata includes backend/version, model identity/revision, compilation identity, effective options, synchronization mode, warm-start disposition, and measured timing segments. Missing native diagnostic evidence is `None`, never zero. MILP duals are not advertised as economic marginal values. LP-only dual extraction must verify valid native dual evidence.

## 7. Solve budgets and optional advanced workflows

Ordinary solves accept per-attempt time limits and preserve effective configuration. Record preprocessing, synchronization, native solve, extraction and total wall time separately. A native solver time limit is not a hard end-to-end real-time guarantee. Check remaining budget before native solve and report overhead/overruns honestly. Python `KeyboardInterrupt` is delivered after a detached native call returns unless native cancellation is explicitly implemented and qualified; do not promise immediate preemption. Bound long smoke tests with native limits.

P31 readiness must expose explicit solve options and a shared deadline across stages: later stages receive only remaining time, not a reset full allowance. If no budget remains, return the last valid incumbent with an explicit budget-stop reason; never construct a lock without candidate evidence. Add Rust tests with a deterministic clock and a real HiGHS limited solve. These semantics are prerequisite hardening, not a reason to expose every P31 type in the first Python surface.

The initial Python `solve` executes the currently selected single objective. `start=previous_solution` accepts same-model owned primal evidence as an explicit MIP-start request; validate identity/domain, reject unsupported cases, and record whether the backend applied it. Never report warm-start use merely because the same solver object survived. A changed-revision start is permitted only as a checked assignment, not as a current solution.

Weighted/lexicographic Python builders, IIS/repair reports and semantic construct wrappers can follow in separate increments after the MPY core gates; they are not prerequisites for the deterministic MPC baseline. The Rust P31 mathematics still must be repaired and merged first. For MPC, linear constraints and explicit slack variables are sufficient for priced soft penalties; hard constraints retain their meaning.

## 8. BESS acceptance model

Use a standalone synthetic fixture with no TPS or customer data. Horizon 24 intervals, `dt=0.25` hours, energy capacity 4 MWh, initial energy 2 MWh, power 2 MW, charge/discharge efficiency 0.95, and a binary direction variable. Hard direction constraints prevent simultaneous charging/discharging, including under negative prices.

```python
import numpy as np
import roml as rm

n, dt = 24, 0.25
m = rm.Model("rolling-battery")
price = m.params("price", np.full(n, 50.0))
initial_energy = m.param("initial_energy", 2.0)
charge = m.vars("charge", n, ub=2.0)
discharge = m.vars("discharge", n, ub=2.0)
energy = m.vars("energy", n + 1, ub=4.0)
direction = m.vars("direction", n, kind="binary")
m.add(energy[0] == initial_energy, name="initial_soc")
m.add(energy[1:] == energy[:-1] + dt * (0.95 * charge - discharge / 0.95), name="balance")
m.add(charge <= 2.0 * direction, name="charge_mode")
m.add(discharge <= 2.0 * (1.0 - direction), name="discharge_mode")
m.maximize(dt * rm.dot(price, discharge - charge) + 30.0 * energy[-1])

with rm.Highs(threads=1, time_limit=2.0) as solver:
    for k in range(10):
        forecasts = 50.0 + 40.0 * np.sin((np.arange(n) + k) / 4.0)
        m.update(price=forecasts, initial_energy=2.0)
        result = solver.solve(m)
        assert result.has_primal
        action = result.value(discharge[0]) - result.value(charge[0])
```

This API illustration holds initial energy fixed for readability. The qualification replay must advance each policy's own energy using its applied first action, roll the causal forecast window, update derates/commitment rows, and test both positive and negative prices. Deterministic and PWL terminal values must be explicit declared alternatives, not double counted. MPY requires the linear baseline; compiled PWL update behavior is a measured extension fixture, not a hidden implementation prerequisite.

## 9. Sources and dependency gate

Consult and pin compatible releases using the official [PyO3 guide](https://pyo3.rs/), [thread-safety guidance](https://pyo3.rs/main/class/thread-safety), [parallelism guidance](https://pyo3.rs/main/parallelism), and [maturin project layout](https://www.maturin.rs/project_layout.html). These were checked during planning on 2026-09-07; online `main` documentation is not a version pin. MPY-01 records exact PyO3, rust-numpy, maturin, NumPy, Python, Rust, highs-sys and native HiGHS versions before implementation. Preserve core Rust MSRV 1.85. Use a compatible binding dependency release or a separately documented binding-crate MSRV; do not silently raise the core MSRV.

Start with per-CPython wheels. Adopt `abi3` only if the selected PyO3/NumPy features are demonstrably compatible and the installed-wheel matrix passes; no ABI simplification is necessary to finish MPY. Public C ABI, additional solvers, generalized nonlinear modeling, Arrow/pandas/xarray layers, async orchestration, distributed workers and a universal plugin framework are outside this milestone.
