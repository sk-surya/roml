# P33 Review — Pass 1 (Specification and Correctness)

**Review:** gsd-code-reviewer (Pass 1 — spec/correctness)
**Phase:** 33-piecewise-linear-bounds
**Branch:** `phase-roml-P33-piecewise-linear-bounds` (worktree `.git/p33-impl`)
**Base:** `main@40af9f4` (parent of plan commit `c7cacf5`)
**Scope:** commits `1804c90` (Task 1), `ba88f54` (Task 2), `4d01605` (Task 3), `ad707a4` (evidence meta) on `c7cacf5`
**Review date:** 2026-08-04
**Depth:** deep (cross-file: payload → builder → bridge → report → HiGHS)

## Verdict

**CONDITIONAL — P0: 0, P1: 1, P2: 3.** The exact segment-binary formulation, the
one-sided supporting rows, the relation/curvature gating, and the nonconvex
exactness proof are all **mathematically correct** and the focused suite
(`cargo test -p roml --test piecewise_linear`, 27 tests) passes. One P1
semantic-correctness gap blocks merge: **no compiled PWL formulation honors the
declared `ExtrapolationPolicy`, and nothing checks the argument interval against
the breakpoint range** — so when the argument can leave `[x_min, x_max]` the
compiled feasible set silently diverges from the declared semantics (a
relaxation for one-sided rows under `Constant` policy, an implicit domain
tightening for the exact graph under either policy). Three P2 quality/evidence
items are listed for acceptance. P1 must be resolved before merge; P2 items may
merge only when explicitly accepted and scheduled.

## Verification performed

- Read the plan (`33-PLAN.md`), the evidence
  (`M3_P33_PIECEWISE_LINEAR_BOUNDS.md`), and all changed source/test files.
- Re-derived every row in `emit_supporting_rows` (epigraph and hypograph) and
  every row in `emit_exact_graph` (convex-combination + adjacency binaries)
  symbol-by-symbol; the emitted coefficients/RHS match the design math.
- Re-ran the nonconvex-exactness proof (zigzag `(1,0.5)` infeasible, `(1,1)`
  feasible) against the emitted rows — sound.
- Re-ran `cargo test -p roml --test piecewise_linear` on the worktree:
  **27 passed; 0 failed** (exit 0), matching the evidence.
- Traced `ExtrapolationPolicy` usage across the whole compiler: it is stored in
  the payload and used by the direct `evaluate`, but **never read by any
  compilation path**.

## Findings

### P1 (blocking)

#### P1-01 — Compiled PWL formulations ignore the declared `ExtrapolationPolicy`; an argument outside the breakpoint range silently changes the feasible set

**Files:**
- `src/compiler/bridge/piecewise_linear.rs:80-110` (argument interval + breakpoint range recorded, never compared)
- `src/compiler/bridge/piecewise_linear.rs:241-340` (`emit_exact_graph`)
- `src/compiler/bridge/piecewise_linear.rs:408-442` (`emit_supporting_rows`)
- `src/construct/piecewise_linear.rs:175-209` (`evaluate` extrapolates)

**Issue.** `payload.extrapolation` is never read by the bridge. The compiler
records `pwl.argument_interval` (from `BoundAnalyzer`) and `pwl.breakpoint_range`
side by side (bridge lines 80-110) but never compares them, never errors, and
never warns. Consequently, when the argument's feasible interval extends beyond
`[x_min, x_max]`, the compiled model silently implements a different function
than the one the user declared:

1. **One-sided rows under `Constant` policy are a silent relaxation (epigraph)
   or silent restriction (hypograph) outside the range.** `emit_supporting_rows`
   emits `y ≥ v_i + s_i·(argument − x_i)` (epigraph) / `y ≤ v_i + s_i·(argument − x_i)`
   (hypograph) for all breakpoints. These supporting rows characterize the
   epigraph/hypograph of the **linearly-extrapolated** function, not the
   declared `Constant`-clamped function. Concrete failure: `convex_pwl =
   [(0,0),(1,1),(2,4)]` (s₀ = 1), `Epigraph`, `Constant`, argument `x` bounded
   `[-1, 3]`. Declared semantics: `y ≥ f_clamp(x)` with `f_clamp(-1) = 0`. The
   compiled row `y ≥ x` (row 0) admits `y = -0.5` at `x = -1`, which violates
   `y ≥ 0`. The feasible set is strictly larger than the declared epigraph — a
   silent relaxation, exactly what the phase's "no silent relaxation" principle
   and D13 prohibit. On the right edge (`x = 3`, last slope 3) the same rows
   require `y ≥ 7` while `f_clamp(3) = 4`, silently **excluding** valid epigraph
   points `4 ≤ y < 7`. No error, no report entry.

2. **The exact graph silently tightens the argument domain to the breakpoint
   range and diverges from `evaluate`.** `emit_exact_graph` adds
   `argument = Σ x_i·λ_i` with `λ ≥ 0, Σλ = 1`, which forces `argument ∈
   [x_min, x_max]` by construction. If the user's argument variable is bounded
   wider (e.g. `x ∈ [0, 5]` with breakpoints `[0, 2]`), the compiled MIP is
   infeasible for every `x ∈ (2, 5]`, whereas the direct evaluator
   (`src/construct/piecewise_linear.rs:175-209`) extrapolates per the declared
   policy (`evaluate(3.0) = 7` linear / `4` constant). The compiled model and the
   declared semantics silently diverge; the user's declared variable bounds are
   silently tightened. This affects **both** policies (even `Linear`), because
   the graph is never extended beyond the breakpoints.

The evidence and tests only ever sample arguments **inside** the breakpoint
range (variable bounds equal `[x0, xn]` in every test), so the gap is entirely
unexercised — but the API allows and the plan's SM-14.1 explicitly promises an
"explicit extrapolation policy" that "defines the behavior outside the breakpoint
range". The compiled model does not implement that promise.

**Fix.** After computing the argument interval in `compile`
(`src/compiler/bridge/piecewise_linear.rs:80-110`), compare
`[trace.result.lower, trace.result.upper]` against `[x_min, x_max]`:

- **ExactGraph:** if the argument interval extends beyond the breakpoint range,
  either (a) reject with a typed `CompileError` naming the construct, the
  argument interval, and the breakpoint range (the graph over the declared range
  is the only representable semantics; extrapolation is not compiled — surface
  it, don't tighten silently), or (b) compile the extrapolated graph explicitly
  by adding endpoint segments out to the interval bounds per the policy. Record
  a `pwl.extrapolation` decision entry stating the policy and its handling.
- **Epigraph/Hypograph:** under `Constant` policy, if the argument interval
  extends beyond the range, reject with a typed error (the supporting rows would
  be a relaxation/restriction). Under `Linear` policy the supporting rows are
  globally exact, so only a report note is needed.
- Add a test that builds the PWL with an argument variable bounded wider than
  the breakpoint range and asserts the typed rejection (or the correctly
  extrapolated rows), for both one-sided and exact-graph relations.

---

### P2 (accepted / advisory)

#### P2-01 — Public `evaluate` / `classify_curvature` / `segment_slopes` panic on parameter-dependent point values

**File:** `src/construct/piecewise_linear.rs:142-147` (`point_value`),
`153-160` (`segment_slopes`), `166-168` (`classify_curvature`), `175-209`
(`evaluate`).

**Issue.** `point_value` calls `as_constant().expect("direct PWL
evaluation/classification requires constant point values")`. Any user code that
builds a PWL with a parameter-dependent point value (which the builder
explicitly supports, and which the compiler bridge evaluates correctly) and then
calls `payload.evaluate(x)` / `payload.classify_curvature()` /
`payload.segment_slopes()` gets a **panic** from a public API. The evidence
documents this as a residual risk, but a panic on a supported payload class is a
quality defect. The compiler path is correct; only the direct-evaluation surface
is affected.

**Fix.** Return `Result<f64, _>` (or take an explicit `&HashMap<ParamId, f64>`
value map) from `evaluate`/`segment_slopes`/`classify_curvature` instead of
panicking, or gate the direct path behind a `#[cfg(test)]`/documented
constant-only contract enforced with a typed error rather than `expect`.

#### P2-02 — Report keys inconsistent between the one-sided and exact paths

**File:** `src/compiler/bridge/piecewise_linear.rs:344-357`
(`record_zero_binary_decisions`) vs `174-194` (exact graph).

**Issue.** The exact path records `pwl.generated_auxiliary_variables` (one per
point) and `pwl.scaling`; the one-sided path records neither. A consumer reading
`pwl.*` decisions for an epigraph construct finds `pwl.generated_binaries` and
`pwl.binary_avoidance_reason` but no `pwl.generated_auxiliary_variables` and no
`pwl.scaling`, while the same keys exist for an exact-graph construct. SM-14.6
asks the report to state generated counts consistently.

**Fix.** Emit `pwl.generated_auxiliary_variables = "0"` (and the
`pwl.scaling` diagnostic) in `record_zero_binary_decisions` so the `pwl.*`
decision schema is uniform across relations.

#### P2-03 — Evidence overstates the P33 public-API delta as "all additive P33 additions"

**File:** `docs/release/evidence/M3_P33_PIECEWISE_LINEAR_BOUNDS.md:331-343`.

**Issue.** The evidence reports `cargo public-api` grew 18792 → 22178
(+3386) and attributes the growth to P33 additions "all additive". The P32
baseline capture (`M3_P32_public_api_roml.txt`, 18792 items) was taken mid-P32
on base `192cd005…` (per `M3_P32_COMMON_CONSTRUCTS.md`), so the +3386 delta
includes later P32 Task 16/17 additions plus P33 — not P33 alone. The delta
overstates the P33 surface. Pass 2 re-checks the public API diff; flagging here
for evidence accuracy.

**Fix.** Re-capture the public API on `main@40af9f4` (the P32-complete base) and
report the delta against that capture, or relabel the +3386 as "since the
mid-P32 capture".

---

### Info

#### IN-01 — Weak assertion on the argument-interval report entry

**File:** `tests/piecewise_linear.rs:821-831`.

`assert!(arg_interval.selection.contains("[0, 2]") || contains("0"))` — the first
disjunct already matches for this fixture and the `contains("0")` fallback is
vacuous for any interval rendered in this format. Tighten to assert the exact
`"[0, 2]"` (or the computed interval from `BoundAnalyzer`).

#### IN-02 — `ExtrapolationPolicy` field is dead in the compiler

**File:** `src/construct/piecewise_linear.rs:50-56` (declaration),
`src/compiler/bridge/piecewise_linear.rs` (no read).

The field is stored and used only by the direct evaluator; the compiler never
reads it (verified by grep across `src/compiler/`). Once P1-01 is fixed this
becomes a live semantic input; until then it is effectively documentation. No
action beyond P1-01.

## Findings count

| Severity | Count |
|----------|-------|
| P0 | 0 |
| P1 | 1 |
| P2 | 3 |
| Info | 2 |

## Dimension-by-dimension disposition

- **Requirement coverage (SM-13 full closure, SM-14.1–14.7, SM-12.8):** MET —
  clause-level coverage matches the plan; SM-12.8 stable-handle builder verified.
- **Semantic correctness (one-sided rows, relation/curvature matching, exact
  segment binaries, nonconvex proof):** CORRECT within the breakpoint range; the
  supporting rows, the SOS2-style convex-combination with adjacency binaries, the
  `λ₀ ≤ z₀`, `λᵢ ≤ zᵢ₋₁ + zᵢ`, `λₘ ≤ zₘ₋₁` gating, and the zigzag proof all
  re-derive correctly. **Gap at domain edges (P1-01).**
- **Invariant preservation (D12 no default Big-M, D13 no objective-context
  relaxation, D24 relation-determines-formulation):** MET — no Big-M constant
  anywhere in the bridge; curvature is classified from slopes, not objective;
  relation dispatch drives the formulation. The extrapolation-policy relaxation
  (P1-01) is adjacent to D13's spirit but is a distinct gap.
- **Origin completeness (SM-02.5):** MET — every generated PWL variable/row is
  `EntityOrigin::Construct { construct, role }`, asserted by `missing_origins`.
- **Unsupported/error behavior:** MET — `NativeRequired` rejects via
  `select_path`; relation/curvature mismatches are typed `UnsupportedFeature`
  errors; the four invalid point classes are typed `ModelError`s.
- **API coherence (A29 preference, SM-12.8):** MET — preference lives on
  `ConstructEntry` (not the payload), the builder returns `(Construct, VarId)`.
- **Test quality:** GOOD overall (validation ×4, curvature ×4, evaluator ×3,
  zero-binary row shapes, feasible-set-equals-graph, nonconvex proof, 256-case
  randomized agreement, origin completeness, HiGHS reference-vs-portable). The
  corpus never exercises arguments outside the breakpoint range (P1-01), and the
  algebraic feasibility oracle in `ExactGraphCtx::feasible` assumes the canonical
  SOS2 assignment (independently confirmed by the HiGHS solve-based section).

---

_Reviewed: 2026-08-04_
_Reviewer: gsd-code-reviewer (Pass 1 — specification and correctness)_
_Depth: deep_

## Dispositions (orchestrator, 2026-08-04)

### P1-01 — compiled PWL formulations ignore the declared ExtrapolationPolicy — **FIXED**

Verified against the code: `payload.extrapolation` was read only by the direct
`evaluate`; no compilation path read it. The math checks out: the supporting
rows are exact for the LINEARLY-extrapolated function (a convex PL function is
the max of its supporting lines), so under `Constant` the clamped function
diverges from the rows outside the range (relaxation when `s_0 > 0`,
restriction when `s_0 < 0`), and the exact segment-binary formulation pins the
argument to `[x_min, x_max]` under either policy.

Fix (TDD, 5 new tests first → RED, then implementation → GREEN):

- New typed error `CompileError::ExtrapolationConflict { construct,
  expression, interval, range, policy }` (`src/compiler/mod.rs`) with a
  `Display` arm that names the construct, interval, range, and policy, and
  points to the two remedies (tighten argument bounds or use linear
  extrapolation).
- `src/compiler/bridge/piecewise_linear.rs`: after the bound-derived argument
  interval is computed, `leaves_range` checks the interval against the
  breakpoint range (non-finite interval always leaves the range); the
  epigraph/hypograph arms reject under `ExtrapolationPolicy::Constant` when
  `leaves_range`; the exact-graph arm rejects when `leaves_range` under either
  policy; Linear one-sided compiles exactly (proven: at `x = -1` and `x = 3`
  the rows imply exactly `f_lin(-1) = -1` and `f_lin(3) = 7`).
- A `pwl.extrapolation` report decision records the policy and disposition in
  every compile (SM-14.6).
- New tests: `pwl_constant_extrapolation_rejects_when_argument_leaves_
  breakpoint_range`, `pwl_constant_hypograph_rejects_when_argument_leaves_
  breakpoint_range`, `pwl_linear_extrapolation_one_sided_compiles_exactly_
  outside_range`, `pwl_exact_graph_rejects_when_argument_leaves_breakpoint_
  range` (both policies), `pwl_report_records_extrapolation_decision_and_full_
  schema`.

Verification: `cargo test -p roml --test piecewise_linear` 32/32; roml
all-targets green (35 suites); roml-highs all-targets green (18 suites);
clippy both crates `-D warnings`; rustdoc both crates `-D warnings`; fmt clean.

### P2-01 — public evaluate/classify_curvature/segment_slopes panic on parameter-dependent values — **ACCEPTED, scheduled**

The doc comment and the evidence already document that direct evaluation
requires constant point values ("a panic here is a programming error"). The
parameter-dependent evaluation path is the compiler bridge
(`eval_point_value`, typed `MissingConstructParameter`). Changing the public
`evaluate` signature to `Result` is a public-API change not appropriate at the
review gate; recorded in the evidence residual risks as a scheduled follow-up.

### P2-02 — one-sided path omits `pwl.generated_auxiliary_variables` and `pwl.scaling` — **FIXED**

`record_zero_binary_decisions` now records `pwl.generated_auxiliary_variables`
= "0" and the one-sided arms call `record_scaling_diagnostic`, matching the
exact path's schema. Covered by
`pwl_report_records_extrapolation_decision_and_full_schema`.

### P2-03 — evidence attributes the full +3386 public-api delta to P33 — **FIXED in evidence**

The evidence's public API section now records the caveat: the P32 baseline
capture was mid-P32 (`192cd005…`), so the delta overstates the P33 surface; the
additive-vs-unintended analysis is unaffected.

### IN-01 — vacuous argument-interval assertion — **FIXED**

`pwl.argument_interval` assertion tightened from
`contains("[0, 2]") || contains("0")` to the exact `contains("[0, 2]")`.

### IN-02 — ExtrapolationPolicy dead in the compiler — **RESOLVED**

`ExtrapolationPolicy` is now read at compile time by the P1-01 gate and
recorded in the report.

**Residual (accepted):** none blocking. P2-01 remains scheduled.

## Re-review (Pass 1 fix round, 2026-08-04) — gsd-code-reviewer

Re-reviewed the fix commit `ffb72ca` against the P1-01 specification and the
P2/IN dispositions. **Verdict: PASS — P1-01 verified FIXED as specified; all
P2/IN dispositions confirmed; no regressions introduced.** Re-ran on the fix
commit: `cargo test -p roml --test piecewise_linear` **32/32 green**;
`cargo test -p roml-highs --test formulation_equivalence pwl` **1/1 green**.

### P1-01 — FIXED (verified)

Gate logic re-derived for every relation × policy × range combination:

| relation | policy | argument interval | disposition |
|---|---|---|---|
| Epigraph / Hypograph | Constant | in range | compile |
| Epigraph / Hypograph | Constant | leaves range | `ExtrapolationConflict` |
| Epigraph / Hypograph | Linear | in range | compile |
| Epigraph / Hypograph | Linear | leaves range | compile (exact) |
| ExactGraph | either | in range | compile |
| ExactGraph | either | leaves range | `ExtrapolationConflict` |

`leaves_range = !lo.is_finite() || !hi.is_finite() || lo < x_min || hi > x_max`
is a correct conservative test. The Linear one-sided exactness claim is sound:
a convex PL function is the pointwise max of its breakpoint supporting lines over
all real arguments (verified numerically: at `x = -1` the rows imply
`max(0+1·(−1), 1+3·(−2), 4+3·(−3)) = −1 = f_lin(−1)`; at `x = 3`,
`max(3, 7, 7) = 7 = f_lin(3)`). The row-implication computation in
`pwl_linear_extrapolation_one_sided_compiles_exactly_outside_range`
(`implied = (lower − x_coeff·x) / y_coeff` with `y_coeff == 1.0`) is correct and
non-vacuous — it asserts against the compiled rows, not a re-derivation from the
payload. The exact-graph rejection under either policy closes the silent
domain-narrowing gap; the non-finite (unbounded-argument) handling is correct
(Linear one-sided rows are globally exact even for unbounded arguments; Constant
one-sided and the exact graph reject).

### P2-02 — FIXED (verified)

`record_zero_binary_decisions` records `pwl.generated_auxiliary_variables = "0"`
and both one-sided arms call `record_scaling_diagnostic`, so the one-sided and
exact paths expose the same `pwl.*` schema keys.

### P2-03 — FIXED in evidence (verified)

The evidence's public-API section now carries the mid-P32-baseline caveat.

### P2-01 — ACCEPTED, scheduled (confirmed)

Non-blocking; the deferred `Result`-signature change is recorded in the evidence
residual risks — consistent with the P2 merge policy.

### IN-01 — FIXED (verified)

The argument-interval assertion is tightened from the vacuous
`contains("[0, 2]") || contains("0")` to the exact `contains("[0, 2]")`.

### IN-02 — RESOLVED (verified)

`ExtrapolationPolicy` is now read by the compile gate and recorded in the report.

### Regression check

- The 27 pre-fix tests are unaffected: every one bounds the argument to the
  breakpoint range, so `leaves_range` is false and the gate never fires.
- IN-01's tightening is strictly stronger, not weaker.
- `CompileError` gains a new variant + `Display` arm; no exhaustive match on
  `CompileError` is broken (the only `match err` in `src/model/mod.rs:494` is on
  `BoundError`, not `CompileError`).
- The HiGHS exact-graph equivalence section still passes because its argument is
  bounded to `[x0, xn]` (in-range).

**Re-review verdict: PASS — P1-01 fixed as specified; all P2/IN dispositions
confirmed; no regressions found; merge gate cleared for the P33 Pass-1 findings.**

## Second review round (owner disposition, PR #31)

### P1 — public segment_slopes/classify_curvature/evaluate panic on valid parameterized payloads — **FIXED**

Verified: `point_value` resolved via `as_constant().expect(...)`; `segment_slopes`, `classify_curvature`, and `evaluate` all route through it, so a VALID parameterized payload (parameter-dependent `PwlPoint::value`) panicked on these public semantic operations.

Fix (TDD, 3 tests first → RED, then implementation → GREEN):

- The `expect` path is removed. The constant-only operations now return a typed **`PwlEvalError`** — `ParameterizedPointValue { index, parameter }` for parameter-dependent points (the valid-payload case), `MissingParameter { parameter }` for an unresolved parameter (F5) — never a panic, never a silent default.
- Parameter-resolver variants added: **`segment_slopes_with`**, **`classify_curvature_with`**, **`evaluate_with`** (shared `slopes_impl`/`evaluate_impl` cores; constant-only versions delegate with the `as_constant` accessor).
- Tests: `pwl_parameterized_points_return_typed_errors_from_constant_only_ops` (evaluate/classify_curvature/segment_slopes → typed `ParameterizedPointValue`); `pwl_resolver_variants_evaluate_parameterized_points` (slopes, curvature, interpolation, Constant extrapolation clamps, Linear extrapolation continuation); `pwl_missing_parameter_is_typed_error` (`MissingParameter`).
- Public API: `PwlEvalError` added to `roml::construct` re-exports; the three methods' signatures now return `Result`.

Verification: `cargo test -p roml --test piecewise_linear` **35/35**; roml all-targets green (35 suites); roml-highs all-targets green (18 suites); clippy both crates `-D warnings`; rustdoc `-D warnings`; fmt clean.
