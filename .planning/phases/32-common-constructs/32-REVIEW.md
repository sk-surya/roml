---
phase: 32-common-constructs
reviewed: 2026-08-03T00:00:00Z
depth: standard
files_reviewed: 18
files_reviewed_list:
  - src/construct/mod.rs
  - src/construct/indicator.rs
  - src/construct/reification.rs
  - src/construct/boolean.rs
  - src/construct/cardinality.rs
  - src/construct/minmax.rs
  - src/construct/absolute.rs
  - src/construct/product.rs
  - src/compiler/bounds.rs
  - src/compiler/bridge/mod.rs
  - src/compiler/session.rs
  - src/compiler/capability.rs
  - src/model/mod.rs
  - src/lib.rs
  - roml-highs/src/session.rs
  - tests/common_constructs.rs
  - tests/compiler_bridges.rs
  - roml-highs/tests/formulation_equivalence.rs
findings:
  critical: 1
  warning: 5
  info: 3
  total: 9
status: issues_found
---

# Phase 32: Code Review Report

**Reviewed:** 2026-08-03
**Depth:** standard
**Files Reviewed:** 18
**Status:** issues_found

## Summary

The P32 common-construct library is, on the whole, carefully and correctly
implemented. The interval-bound analyzer (`BoundAnalyzer`), the one-sided
Big-M helpers (derived-M and explicit-M validation, both fail-closed with the
construct-aware `UnboundedBigM` marker), the `BridgeFinalizer` (dense
deterministic id allocation, `EntityOrigin::Construct` on every generated
entity, bound-evidence report entries), the capability registry
(`SupportLevel::Bridge` with no false native claims on HiGHS), the builder
validation rejections (non-binary activators, duplicate cardinality inputs,
invalid `k`, continuous×continuous products, unbounded expressions, trivially
satisfiable min/max relations), and the algebraic bridges (exact min/max
selector, abs/positive-part/clamp decomposition, binary×bounded-linear
products) all trace out as mathematically exact — I verified every row and
Big-M value against the semantic feasible set, including the D13 exact-vs-
one-sided difference proofs and the zero-binary epigraph/hypograph rows.

One BLOCKER correctness defect exists: reification with the inferred unit gap
is not exact when the set threshold is non-integral (a proven-integer
expression with a fractional rhs silently excludes the integer just above the
threshold from any feasible assignment). The remaining findings are
capability-gating inconsistencies (boolean/cardinality bypass the feature
gate; the Mip gate ignores construct-generated binaries), an incomplete
parameter-dependency derivation for set-threshold parameters, an
overflow-handling inconsistency between the implied and explicit Big-M
paths, and a soft A30 surface leak (the `Fixture` variant is nameable from a
public enum despite being documented crate-private).

## Critical Issues

### CR-01: Reification unit-gap complement is not exact for a non-integral threshold on a proven-integer expression

**File:** `src/model/mod.rs:555-568` (accepts the construct) and `src/compiler/bridge/reification.rs:34,76` (unit-gap `1.0` complement)

**Issue:**
`add_reify` infers the unit gap (`separation_tolerance = None`) whenever
`expression_is_proven_integral` returns true, but that check validates only the
function's coefficients and variable types — it never validates the **set
threshold** integrality. The bridge then emits the complement row at
`rhs + 1.0`. For an integer-valued function the separation `f > rhs ⟺ f ≥ rhs+1`
is exact only when `rhs` is an integer. With a fractional rhs the formulation
silently excludes the integer just above `rhs` from **both** b=0 and b=1,
making the model infeasible where the semantics permit b=0.

Concrete counterexample (`x`, `y` binary, proven integral):

```
model.add_reify((x + y).le(1.5), None, None);   // accepted (proven integral)
```

Semantics: `b = 1 ⟺ x+y <= 1.5` ⟺ `b = 1 ⟺ x+y <= 1` (integer). At `x=y=1`
(`f = 2`) the correct assignment is `b = 0`. The compiled rows are:

- forward:  `x + y + 0.5·b <= 2.0`  (M1 = max(0, 2−1.5) = 0.5)
- complement: `x + y + 2.5·b >= 2.5` (M2 = max(0, 2.5−0) = 2.5; rhs_c = 2.5)

At `f=2`: b=1 violates row 1 (`2+0.5 <= 2`), b=0 violates row 2 (`2 >= 2.5`).
No feasible `b` exists — the model is infeasible at a point the semantic
feasible set contains. The `formulation_equivalence` / `common_constructs`
tests only exercise integral rhs (1.0), so this path is untested.

**Fix:** When `separation` is `None` and `proven_integrality` is true, validate
that every set threshold is integral (reject with a typed error otherwise), or
derive the complement threshold as `ceil(rhs)` for integer-valued functions
instead of `rhs + 1.0`. Add a feasible-set test with a fractional threshold
(e.g. `(x+y).le(1.5)`) asserting `(x=y=1, b=0)` is feasible.

## Warnings

### WR-01: Boolean and cardinality bridges bypass the capability gate entirely

**File:** `src/compiler/bridge/boolean.rs:14-92`, `src/compiler/bridge/cardinality.rs:14-44`

**Issue:**
Every other P32 bridge (`indicator`, `reification`, `minmax`, `absolute`,
`product`) begins with `select_path(capabilities, policy, BackendFeature::…, …)`
and surfaces `CompileError::UnsupportedFeature` when the feature is neither
natively supported nor bridge-declared (and rejects under `NativeRequired`).
The Boolean and cardinality bridges never call `select_path`, so a model with a
Boolean/cardinality construct compiles silently against a capability set that
declares `BackendFeature::Boolean`/`Cardinality` as `Unsupported` (or under a
`NativeRequired` policy where only bridge support exists). This contradicts the
P32 contract's SM-04.4 "unqualified feature is rejected, never silently
ignored" and the evidence note about "intermediate UnsupportedFeature dispatch
arms."

**Fix:** Add the same `select_path` gate at the top of `boolean::compile` and
`cardinality::compile` (gating on `BackendFeature::Boolean` /
`BackendFeature::Cardinality`), and add a test analogous to
`indicator_unqualified_feature_is_unsupported` for both.

### WR-02: Mip capability gate ignores construct-generated binary variables

**File:** `src/compiler/session.rs:230-241`

**Issue:**
`compile_snapshot` requires `BackendFeature::Mip` only when a **user** variable
is integer/binary (`snapshot.variables`). An exact `minmax` or `absolute`
(clamp/abs) construct over all-continuous operands generates selector binaries
at compile time with no Mip gate. A backend declaring only `Lp` (plus the P32
bridge feature) would compile a snapshot containing binary columns without the
Mip gate firing, so the backend could solve a wrong continuous relaxation
silently.

**Fix:** After the construct loop, if any construct output carries generated
binaries (`var_type == Binary`), require `BackendFeature::Mip` (extend the
`has_integer` computation to include generated construct variables).

### WR-03: Set-threshold parameters are missing from construct parameter dependencies

**File:** `src/construct/indicator.rs:43-45`, `src/construct/reification.rs:42-44`

**Issue:**
`IndicatorConstraint::parameter_dependencies` and
`ReificationConstraint::parameter_dependencies` derive dependencies only from
`self.function`. The set's `ValueExpr` threshold can reference a parameter
(e.g. `add_indicator(z, WhenOne, (x).le(p), …)`), which both bridges evaluate
at compile time (`one_sided_implications` / `eval_bound`). Such parameters are
omitted from `ConstructEntry`-level dependency tracking (exposed publicly via
`Model::construct_parameter_dependencies` and invariant-checked in
`validate_invariants`), so a threshold parameter change is not attributed to
the construct.

**Fix:** Include the set thresholds' `ValueExpr::dependencies()` in
`parameter_dependencies()` for both payloads (and update
`derive_parameter_dependencies` consumers accordingly).

### WR-04: `validated_explicit_big_m` fails open on derived-min overflow while the implied path fails closed

**File:** `src/compiler/bounds.rs:577-600`

**Issue:**
`bound_big_m_implied` treats `(upper - rhs)` overflowing to `+inf` as
`UnboundedBigM` (fail closed). `validated_explicit_big_m` computes the same
expression without an infinity check: `Some(min)` becomes `Some(inf)`, and since
`proposed < inf` is false for any finite `proposed`, an arbitrary finite
explicit M is accepted even though the true minimum is effectively unbounded.
The two one-sided helpers disagree on the same input.

**Fix:** When the derived minimum is infinite after the subtraction (finite
endpoints, overflow), treat it like `derived_big_m` does — return the
`UnboundedBigM` marker (or require the explicit value against the unbounded
derivation), rather than accepting any finite M.

### WR-05: A30 surface — `ConstructKind::Fixture` and `FixturePayload` are nameable from the public API

**File:** `src/construct/mod.rs:80-99`, `src/lib.rs:23-26`

**Issue:**
The module doc and lib.rs comment state the fixture scaffolding "stays
crate-private," but `ConstructKind` is a public, re-exported enum whose
`Fixture(FixturePayload)` variant is public, and `FixturePayload` is a `pub
struct` inside the now-public `pub mod construct` (only `#[doc(hidden)]`).
External code can match `ConstructKind::Fixture(_)` and name
`roml::construct::FixturePayload`; it simply cannot construct it (fields and
`new` are `pub(crate)`). The A30 "fixture scaffolding absent from public API"
contract is only partially met.

**Fix:** Either move the fixture scaffolding out of the public enum into a
crate-private test-only module, or document the variant as a deliberate
observability leak with a `#[doc(hidden)]` note. At minimum, correct the module
docs so they match the actual surface.

## Info

### IN-01: min/max, absolute, and product bridges make a native selection unobservable

**File:** `src/compiler/bridge/minmax.rs:66-72`, `src/compiler/bridge/absolute.rs:53-59`, `src/compiler/bridge/product.rs:45-51`

**Issue:** Unlike the indicator bridge (which emits the distinct
`IndicatorNative` role + a `indicator.representation` decision when a native
primitive is selected), the `select_path` result in these three bridges is
matched with empty arms, and the emitted rows/roles are identical for the
`Native` and `Bridge` paths. A native selection cannot be distinguished in the
origin map or the formulation decisions.

**Fix:** Record a representation decision naming the selected path (native vs
bridge) as the indicator bridge does.

### IN-02: `Model::expression_interval` degrades all `BoundError` causes to one generic error

**File:** `src/model/mod.rs:900-920`

**Issue:** `add_minmax` / `add_absolute_value` / `add_binary_product` map every
`BoundAnalyzer` failure (`NonFiniteCoefficient`, `InvalidBounds`,
`ArithmeticNan`, …) to `ModelError::NonFiniteValue("expression bounds")`,
losing the specific cause that would help a user debug an unbounded construct.

**Fix:** Map the distinct `BoundError` variants to matching `ModelError`
variants (or carry the analyzer reason through).

### IN-03: Construct builders are not atomic when `add_construct` fails after output-variable creation

**File:** `src/model/mod.rs:571-579` (`add_reify`), `752-760` (`add_minmax`), `801-808` (`add_absolute_value`), `840-847` (`add_binary_product`)

**Issue:** Each builder creates its output/activator variable via
`add_variable_internal` before calling `add_construct`. If `add_construct`
fails (`ConstructId` counter exhaustion), the variable is left in the arena and
changelog with no construct referencing it. The risk is vanishingly small (id
counter exhaustion only), but the sequence is non-atomic.

**Fix:** Allocate the construct id first, or roll back the variable insertion on
the `add_construct` error path.

---

_Reviewed: 2026-08-03_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
