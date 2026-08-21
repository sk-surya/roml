---
phase: 31-lexicographic-objectives
status: implementation-verified-review-pending
base: "9db2ec2997dee17e796b80bc8897399c676e1bd9"
head: "remediation addressing review #4989778702 (P1s+P2) and review #4989844497 (lock-construction fallibility, final-point objective vector, rebuild fault); independent re-review pending"
plan: 31-PLAN.md
---

# P31 Objective Policies — Implementation Evidence

## Scope and status

P31 supplies one canonical `ObjectivePolicy`, one `ObjectivePriority`, a
deterministic portable weighted/lexicographic executor with exact normalized
`|z*|` degradation locks, and the priority-target P30 penalty integration
(SM-11.1–SM-11.8, SM-07.7, P27 objective-lock debt, SM-10.6 priority
targeting).

Base is `main@9db2ec2` (P30 merged, P31 authorized). The implementation/test
head is `377562b` (the evidence-doc commit follows the code/test head
`9839ab4`). **Hosted mandatory CI is green on `377562b`; independent exact-head
code review has not yet been completed on the remediation head.** This document
must not be treated as cleared-to-merge.

## Committed change set (10 commits, base 9db2ec2 -> head 9839ab4)

- `e30df39` canonical `ObjectivePriority` + policy model + atomic validation
- `fcf4aa8` extended `ObjectivePolicy` (None/Single/Weighted/Lexicographic) + P28 override guard
- `27c0c66` exact `|z*|` stage-lock math (`delta = abs + rel*|z*|`)
- `d2e8437` `ObjectiveValue` / `ObjectiveStageResult` / `MultiObjectiveResult` result types
- `b17cfb1` portable weighted/lexicographic executor (`objective_combine` + `objective_executor` + `solve_objective_policy`)
- `0808028` HiGHS weighted + lexicographic integration tests
- `737a49d` `PenaltyTarget::Priority(ObjectivePriority)` API + canonical handling
- `122beb9` fold priority-targeted P30 penalties into lexicographic stages (SM-10.6)
- `c4f9320` objective-executor fault matrix + continuation semantics (Task 31-05)
- `9839ab4` same-objective-different-priority legality + provider-policy behavior (Tasks 31-06/31-08)

## Remediation after review #4989778702 (2 P1 + 1 P2)

Independent review of PR #49 at exact head `8a1fedb` found one P1 production
defect not covered by the original test set, two lifecycle/termination P1s, and
one validation P2. The following remediation is applied and verified:

1. **Priority penalties are part of `z*` and every later degradation lock.**
   The executor now resolves the full `StageScalar` (canonical combination +
   numerically evaluated priority penalties) before backend mutation. Every
   later stage re-materializes each prior stage's penalty rows into its own
   overlay, so a later stage cannot degrade a priority-0 penalty that is
   absent from the canonical objective. Regression: a two-level real-HiGHS
   lexicographic model where priority 0 pins `x = 6` via a priority-targeted
   penalty and priority 1 minimizes `x` (incentive to reintroduce the
   violation) — `x` must remain at 6. A second regression proves `z* = 0`
   relative-only tolerance yields an exact lock (`allowed_degradation = 0`)
   that keeps a satisfiable `x <= 2` penalty from being reintroduced.
2. **`Unknown` is a mathematical stage outcome, not a backend error.** The
   objective executor now maps terminations leniently (only `Error` is an
   operational failure); `Unknown` flows through
   `classify_continuation` → `StopUnknown` (BestFeasible) / `StopNotOptimal`
   (RequireOptimal) and is recorded on the stage result. A controlled-backend
   regression asserts a `TerminationStatus::Unknown` yields a staged result
   with `StopUnknown`, not a `Backend` error.
3. **Post-solve extraction failures roll back.** `objective_values(...)` now
   routes through `rollback_objective` on failure (a controlled foreign/absent
   compilation id verifies rollback + no leaked `MultiObjectiveResult` + a
   clean follow-up solve).
4. **`ObjectivePolicy::Single` stale validation (P2).** `Single` now consults
   the supplied existence checker like `Weighted`/`Lexicographic`, and the
   facade passes a real checker (`model.objective_sense(...).is_some()`) for
   atomic pre-mutation rejection. Regression both at unit level and in the
   objective-policy fault matrix.

## Remediation after review #4989844497 (2 P1 + 1 P2)

Independent re-review at exact head `00da659` confirmed the prior remediation
but found two additional P1s and one P2. The following remediation is applied
and verified:

1. **Lock construction is fully fallible (P1).** Solver-derived candidate
   values are now checked for finiteness before any scalar/lock arithmetic:
   `evaluate_objective`, `evaluate_combined`, `evaluate_constraint_row`, and
   `evaluate_stage_scalar` all return a typed [`Numerical`] error on a `NaN`/
   infinity (never a panic while the stage overlay is applied), routed through
   `rollback_objective`. `ObjectiveLockReport::from_stage` is now fallible too:
   it rejects non-finite stage values, non-finite/negative tolerances, and a
   degradation bound that overflows (`DegradationOverflow`), so lock
   construction can never panic or emit an effectively-unbounded lock.
   Regressions: a real-compiled unit test drives `evaluate_stage_scalar` with
   `NaN`/`±Inf`/finite candidates (`Numerical` on the former, normal value on
   the latter); a unit test asserts `from_stage` returns typed errors for
   `NaN`/`Inf`/bad tolerances/overflow.
2. **Task 31-06 final-point objective vector (P1).** The last executed stage's
   `objective_values` is now recomputed at the FINAL solution point to contain
   the complete distinct policy objective vector (all canonical objectives),
   not just that stage's own objective. Regressions: a two-level fault-matrix
   test asserts the last stage exposes both distinct objectives; the real-HiGHS
   lexicographic test asserts both `obj1`/`obj2` are reported on the last
   stage.
3. **Explicit rebuild-failure fault (P2).** Task 31-05 lifecycle injection now
   includes the REBUILD boundary. The fault harness injects a one-shot rebuild
   failure, asserts the error surfaces, and proves a subsequent forced rebuild
   recovers cleanly.

The `StageScalar` design for priority penalties is retained (no redesign
indicated); these changes are localized to fallibility, final-point reporting,
and fault-matrix completeness.

No MOSEK/Xpress native multiobjective audit change: HiGHS still has no
qualified native normalized-`|z*|` path; the portable path remains normative.

## Remediation after review #4989892030 (1 P1)

Independent re-review at exact head `6536506` confirmed all prior remediation
but found one remaining P1: P30 `PenaltyTarget::Objective` penalties are folded
by the compiler directly into the targeted canonical objective as coefficients
on generated soft-constraint violation variables, which are NOT exposed in
`SolveSolution.variable_values`. Recomputing the stage scalar from primal
values therefore silently dropped those generated penalty terms, so a stage
could report and lock a `z*` that did not match the scalar the backend actually
solved — the same semantic failure class as the earlier priority-penalty defect,
through the ordinary `PenaltyTarget::Objective` path. The following remediation
is applied and verified:

1. **The stage scalar/lock is the exact value the backend solved (P1).** The
   portable executor now prefers `SolveSolution.objective_value` — the value of
   the temporary stage objective the backend actually minimized, which includes
   every canonical term, the constant, and all generated soft-constraint
   violation terms — as the `scalar_stage_value` and the `z*` feeding
   `ObjectiveLockReport::from_stage`. The primal recomputation is retained only
   as a fallback when the backend reports no objective value (e.g. a
   projection-only harness). A non-finite reported value is a typed `Numerical`
   error routed through `rollback_objective` (never a panic under an applied
   overlay). Regression: a two-level real-HiGHS model with a `PenaltyTarget::
   Objective` penalty weight 0.5 on `x >= 6` (targeting `obj0 = min x`). The
   priority-0 minimum lands at `x = 0` with a nonzero violation, so the true
   penalized scalar is `3.0`; the test asserts `z* = 3.0`, the lock reference is
   `3.0`, stage 1 (maximize `x`) remains feasible and both stages prove
   optimality — proving the zero-tolerance lock preserves the penalized scalar.
   Under the previous behavior the scalar was reported as `0` and the
   zero-tolerance lock became infeasible for stage 1.
2. **Missing required user primal values are rejected (P1).** `SolveSolution.
   variable_values` is not guaranteed complete by the backend contract.
   `evaluate_constraint_row`, `evaluate_combined`, and `evaluate_objective` now
   return a typed `Numerical` error when a user variable required by an
   objective/row is absent from the supplied primal values, instead of silently
   treating it as zero. Regression: a real-compiled unit test drives
   `evaluate_stage_scalar` and `evaluate_objective` with an empty primal set and
   asserts a `Numerical` missing-primal error.

### Fresh local checks (round 4, after review #4989892030)

- `cargo test -p roml --all-targets` — all pass; includes the new
  missing-required-primal unit regression.
- `cargo test -p roml --test objective_policy_faults` — 12 passed.
- `cargo test -p roml-highs --test objective_policy` — 8 passed (includes the
  new two-level objective-target-penalty exact-scalar regression).
- `cargo test -p roml-highs --all-targets` — all pass with bundled HiGHS.
- `cargo fmt --all -- --check`, `cargo clippy -p roml -p roml-highs --all-targets -- -D warnings`, and `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` — all clean.

## Fresh local checks at remediation time

- `cargo test -p roml --all-targets` — all pass.
- `cargo test -p roml --test objective_policy_faults` — 10 passed (previous 7
  plus `Unknown`-preservation, post-solve extraction rollback, and stale
  `Single` regressions).
- `cargo test -p roml-highs --test objective_policy` — 7 passed (previous 5
  plus two-level priority-penalty lock and `z*=0` relative-only exact-lock
  real-HiGHS regressions).
- `cargo test -p roml-highs --all-targets` — all pass with bundled HiGHS.
- `cargo fmt --all -- --check`, `cargo clippy -p roml -p roml-highs --all-targets -- -D warnings`, and `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` — clean.

### Fresh local checks (round 3, after review #4989844497)

- `cargo test -p roml --all-targets` — all pass; includes a new real-compiled
  `evaluate_stage_scalar` finiteness unit test (`NaN`/`±Inf` → `Numerical`,
  finite → normal value) and `ObjectiveLockReport::from_stage` fallibility unit
  tests (`NonFiniteStageValue`, `NonFiniteTolerance`, `DegradationOverflow`).
- `cargo test -p roml --test objective_policy_faults` — 12 passed (round 2's 10
  plus explicit rebuild-failure and last-stage-complete-objective-vector
  regressions).
- `cargo test -p roml-highs --test objective_policy` — 7 passed, with the
  lexicographic test now also asserting the final stage reports both distinct
  canonical objectives at the final point.
- `cargo test -p roml-highs --all-targets`, `cargo fmt --all -- --check`,
  `cargo clippy -p roml -p roml-highs --all-targets -- -D warnings`, and
  `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` — all clean.

## Fresh local checks at pre-remediation head 377562b (kept for audit trail)

- `cargo test -p roml --all-targets` — all pass (313 library tests; all integration targets).
- `cargo test -p roml --lib objective_policy` — 13 passed (priority ordering,
  duplicate-within-level rejected, duplicate-priority rejected, empty rejected,
  invalid weight/tolerance rejected, stale-objective rejected, exact `|z*|` lock
  math across positive/zero/negative optima, same objective at different
  priorities legal, result/schema shape).
- `cargo test -p roml --test objective_policy_faults` — 7 passed (apply,
  partial-apply+rollback, solve, rollback, verify fault injection; composite
  `Cleanup` error with rebuild requirement; no leaked state; `NativeRequired`
  pre-mutation rejection; `BestFeasible` descend/lock; `RequireOptimal`
  infeasible stop).
- `cargo test -p roml-highs --all-targets` — all pass with bundled HiGHS.
- `cargo test -p roml-highs --test objective_policy` — 5 passed (weighted,
  lexicographic, priority-penalty folding to x=6.0, `PreferNative` fallback to
  portable, `NativeRequired` rejection without mutation).
- `cargo fmt --all -- --check` and `cargo clippy -p roml -p roml-highs --all-targets -- -D warnings` — clean.
- `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` — clean.

## What the evidence covers

- One canonical `ObjectivePriority` and `ObjectivePolicy`; numeric nonnegative
  `f64` weights; `+w*f` MIN / `-w*f` MAX normalization into a single minimized
  scalar per stage.
- Exact normalized lock: `delta = abs_tol + rel_tol*|z*|`, `g(x) <= z* + delta`;
  `z*=0` gives zero relative degradation; negative/positive optima use positive
  magnitude.
- Portable sequential execution applies all prior-stage degradation locks
  every stage and always rolls back before a result escapes.
- Continuation: `RequireOptimal` stops on any non-optimal stage;
  `BestFeasible` descends from a feasible incumbent, locks against the
  incumbent scalar, and records `ContinueBestFeasible` without claiming
  optimality; infeasible/unknown are distinct.
- Fault matrix at apply / partial-apply+rollback / solve / rollback / verify
  boundaries: primary and cleanup/rebuild errors are both preserved; successful
  math + failed cleanup yields a composite error and NO `MultiObjectiveResult`;
  an ordinary solve after a forced rebuild proves no leaked stage state.
- Provider policy: `PortableOnly`/`PreferNative` yield the portable provider
  (observable `PortableSequential`); `NativeRequired` rejects before mutation
  when no qualified native provider exists.
- Priority-targeted P30 penalties resolve the parameterized weight against the
  current snapshot, fold into the correct stage, and drive the optimum (HiGHS:
  weight-5 penalty on `x >= 6` pins `x = 6`).

## Hosted CI (exact head 377562b)

PR #49 (`sk-surya/roml`) at head `377562bc78819e46fc736f1fad3fea756f15aab`:
all mandatory hosted workflows green on that exact head — Code coverage,
Coverage & policy, Feature exclusivity, Lint + doc (HiGHS), MSRV (HiGHS,
1.85), MSRV (Linux, 1.85), Package verification, Security audit, License and
dependency check, Unused dependency, Test (bundled + system) across
ubuntu/macos/windows. (Update coverage badge and Mutation score are skips, not
failures.)

## Residuals / not yet claimed

- Independent exact-head code re-review on the remediation head has not been
  completed and is required before any CLEAR status.
- Hosted exact-head CI must be re-run on the remediation head (the prior green
  CI was on `377562b`/`8a1fedb`, before the remediation commits).
- MOSEK/Xpress native multiobjective audit: HiGHS provides no qualified native
  normalized-`|z*|` multiobjective path, so the portable path is normative and
  `PreferNative`/`NativeRequired` behave accordingly; a full documented native
  audit across all adapters remains for later qualification.
- No publication, tag, release, or merge action is taken or implied.

## Required next steps

1. Push the remediation commits to PR #49 and confirm exact-head hosted CI.
2. Complete exact-head independent re-review with no unresolved P0/P1 (PR #49 open, draft).
3. Owner-authorized merge of PR #49.
4. Update the milestone routing state to authorize P34 after the P31 merge.
