---
phase: 32-common-constructs
fixed_at: 2026-08-03T00:00:00Z
review_path: .planning/phases/32-common-constructs/32-REVIEW.md
iteration: 1
findings_in_scope: 9
fixed: 9
skipped: 0
status: all_fixed
---

# Phase 32: Code Review Fix Report

**Fixed at:** 2026-08-03
**Source review:** `.planning/phases/32-common-constructs/32-REVIEW.md`
**Iteration:** 1

**Summary:**
- Findings in scope: 9 (1 critical, 5 warnings, 3 info)
- Fixed: 9
- Skipped: 0

**Verification environment:** All fixes were applied and gated directly in the
phase worktree `/Users/skrishnan/repos/roml/.git/p32-impl` on branch
`phase-roml-P32-common-constructs` (the phase's dedicated git worktree; no
nested worktree was created). Every finding was developed test-first: a failing
test was written and its failure recorded before the fix, then a focused test
was run green, then the full gate was run (`cargo test -p roml --all-targets`,
`cargo test -p roml-highs --all-targets`, `cargo clippy -p roml --all-targets
-- -D warnings`, `cargo clippy -p roml-highs --all-targets -- -D warnings`,
`cargo fmt --all -- --check`). Final gate: **roml 773 passed / 0 failed,
roml-highs 121 passed / 0 failed, clippy clean both crates, fmt clean.**

## Fixed Issues

### CR-01: Reification unit-gap complement is not exact for a non-integral threshold on a proven-integer expression

**Files modified:** `src/model/mod.rs`, `tests/common_constructs.rs`
**Commit:** `f3002a9`
**Applied fix:** When `separation` is `None` and `expression_is_proven_integral`
is true, `add_reify` now validates every `le`/`ge` set threshold for
integrality (within a 1e-9 tolerance) at build time. A non-integral threshold
on a proven-integer expression is a typed `ModelError::NonIntegralReificationThreshold`
rejection (the unit gap `f > rhs ⟺ f >= rhs+1` is exact only for an integral
rhs); an explicit separation tolerance remains the exact path for fractional
thresholds. Tests: rejection of `(x+y).le(1.5)` with binary x,y; acceptance of
integral rhs (unit gap) and fractional rhs with explicit separation; and a
feasible-set test asserting the compiled set equals the semantic set for
`(x+y).le(1.5)` with `separation=0.5` (including `(x=y=1, b=0)` feasible).

### WR-01: Boolean and cardinality bridges bypass the capability gate entirely

**Files modified:** `src/compiler/bridge/boolean.rs`,
`src/compiler/bridge/cardinality.rs`, `tests/common_constructs.rs`
**Commit:** `2bb8dd5`
**Applied fix:** `boolean::compile` and `cardinality::compile` now call
`select_path` on `BackendFeature::Boolean` / `BackendFeature::Cardinality`
before emitting rows, so an unqualified feature surfaces a typed
`CompileError::UnsupportedFeature` (SM-04.4) instead of compiling silently, and
`NativeRequired` rejects the bridge-only path. Tests: `boolean_unqualified_feature_is_unsupported`,
`cardinality_unqualified_feature_is_unsupported`, and
`boolean_and_cardinality_reject_under_native_required` (analogous to the
existing `indicator_unqualified_feature_is_unsupported`).

### WR-02: Mip capability gate ignores construct-generated binary variables

**Files modified:** `src/compiler/session.rs`, `tests/common_constructs.rs`
**Commit:** `ea742a6`
**Applied fix:** After the construct loop in `compile_snapshot`, any generated
construct variable with a non-continuous `var_type` now requires
`BackendFeature::Mip` (extending the user-variable `has_integer` gate), so a
backend declaring only `Lp` rejects an exact minmax/abs whose selector binaries
are generated at compile time instead of silently solving a wrong continuous
relaxation. Tests: `construct_generated_binaries_require_mip_gate` (exact
minmax and exact abs over all-continuous operands against an Lp-only capability
set) and `zero_binary_construct_compiles_without_mip` (max epigraph still
compiles against Lp-only).

### WR-03: Set-threshold parameters are missing from construct parameter dependencies

**Files modified:** `src/function/set.rs`, `src/construct/indicator.rs`,
`src/construct/reification.rs`, `tests/common_constructs.rs`
**Commit:** `7d0d68e`
**Applied fix:** Added `ScalarSet::dependencies()` (the union of the parameter
dependencies of every `ValueExpr` threshold). `IndicatorConstraint::parameter_dependencies`
and `ReificationConstraint::parameter_dependencies` now include the set-threshold
dependencies alongside the function's, so a threshold parameter (evaluated by
the bridge at compile time) is attributed to the construct in
`ConstructEntry`-level dependency tracking. Tests: payload-level tests for both
payloads asserting a `LessEqual(ValueExpr::param(p))` /
`GreaterEqual(ValueExpr::param(p))` threshold parameter is a dependency.

### WR-04: `validated_explicit_big_m` fails open on derived-min overflow while the implied path fails closed

**Files modified:** `src/compiler/bounds.rs`
**Commit:** `14b7d08`
**Applied fix:** When `(upper - rhs)` or `(rhs - lower)` overflows to `+inf`
despite finite endpoints, `validated_explicit_big_m` now returns the
construct-aware `CompileError::UnboundedBigM` marker exactly like
`derived_big_m` / `bound_big_m_implied` (fail closed), instead of accepting an
arbitrary finite M or mislabeling the rejection as `InvalidBigM`. Test:
`validated_explicit_big_m_fails_closed_on_derived_min_overflow` (both Upper and
Lower overflow with `f64::MAX` endpoints).

### WR-05: A30 surface — `ConstructKind::Fixture` and `FixturePayload` are nameable from the public API

**Files modified:** `src/construct/mod.rs`, `src/model/mod.rs`,
`src/compiler/session.rs`, `src/lib.rs`, `src/advanced.rs`, `src/identity.rs`,
`tests/compiler_bridges.rs`, `tests/semantic_ir.rs`
**Commit:** `06b6465`
**Applied fix:** `ConstructKind::Fixture`, `FixturePayload`, its impl, and
`Model::add_construct_fixture` are now `#[cfg(test)]`-gated, and the two
non-test match arms (`derive_parameter_dependencies`,
`CompilationSession::compile_snapshot`) gate the `Fixture` arm the same way.
The scaffolding therefore exists only in test builds and is absent from the
public API surface in non-test builds; module docs updated to state this.
Verified with `cargo public-api -p roml`: `ConstructKind::Fixture` and
`FixturePayload` no longer appear (grep count 0), fully meeting A30.

### IN-01: min/max, absolute, and product bridges make a native selection unobservable

**Files modified:** `src/compiler/bridge/minmax.rs`,
`src/compiler/bridge/absolute.rs`, `src/compiler/bridge/product.rs`,
`tests/common_constructs.rs`
**Commit:** `1f58475`
**Applied fix:** The three bridges now record a `*.path` formulation decision
(`minmax.path` / `absolute.path` / `product.path`) naming the `select_path`
result (native vs exact bridge), matching the indicator bridge precedent, so a
native selection is observable in the formulation decisions instead of being
matched with empty arms. Test: `minmax_absolute_product_record_representation_path`
asserts each `*.path` decision exists with selection `"exact bridge"` under
Auto + bridge caps.

### IN-02: `Model::expression_interval` degrades all `BoundError` causes to one generic error

**Files modified:** `src/model/mod.rs`
**Commit:** `a957660`
**Applied fix:** `expression_interval` now maps each distinct `BoundAnalyzer`
failure to a matching `ModelError` variant via a new `bound_error_to_model_error`
helper (coefficient/constant/parameter-value/interval-arithmetic labels,
`InvalidBounds`, NaN bounds) instead of collapsing every cause into one generic
`NonFiniteValue("expression bounds")`. Covered by an in-crate unit test
(`bound_error_mapping_tests::expression_interval_maps_distinct_bound_error_causes`)
pinning every mapping. (Note: an integration probe showed the distinct causes
are largely preempted by `validate_expression_entities` and the interval
overflow clamping through the public builders, so the mapping is defensive —
the unit test pins it directly.)

### IN-03: Construct builders are not atomic when `add_construct` fails after output-variable creation

**Files modified:** `src/construct/mod.rs`, `src/model/mod.rs`
**Commit:** `0498794`
**Applied fix:** `add_reify`, `add_minmax`, `add_absolute_value`, and
`add_binary_product` now reserve the construct id FIRST (via a new
`ConstructStore::add_with_id` + `Model::add_construct_allocated` path) and only
then create the output/activator variable, so a construct-id failure can never
leave an orphaned variable in the arena/changelog. Covered by an in-crate unit
test (`construct::tests::store_add_with_id_inserts_pre_allocated_construct`).
The failure mode (ConstructId counter exhaustion) is practically unreachable, so
this is a structural fix verified by the pre-allocated-insert unit test and the
full existing builder suite.

## Skipped Issues

None — all 9 in-scope findings were fixed.

---

_Fixed: 2026-08-03_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
