---
phase: 31-lexicographic-objectives
status: implementation-verified-review-pending
base: "9db2ec2997dee17e796b80bc8897399c676e1bd9"
head: "377562b (code/test/evidence head); hosted CI green on this head; independent review pending"
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
code review has not yet been completed on this head.** This document must not
be treated as cleared-to-merge.

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

## Fresh local checks at head 377562b (code/test commits through 9839ab4)

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

- Independent exact-head code review on `377562b` has not been completed and
  is required before any CLEAR status.
- MOSEK/Xpress native multiobjective audit: HiGHS provides no qualified native
  normalized-`|z*|` multiobjective path, so the portable path is normative and
  `PreferNative`/`NativeRequired` behave accordingly; a full documented native
  audit across all adapters remains for later qualification.
- No publication, tag, release, or merge action is taken or implied.

## Required next steps

1. Complete exact-head independent code review with no unresolved P0/P1 (PR #49 open, draft).
2. Owner-authorized merge of PR #49.
3. Update the milestone routing state to authorize P34 after the P31 merge.
