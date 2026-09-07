# MPY-00 PR Inventory (PY-01)

**Recorded:** 2026-09-07 (executor refresh; all SHAs re-queried live).
**Planning PR #50:** MERGED as `f6dc68b` (`Merge pull request #50 from sk-surya/docs/python-interface-ultraplan`).
Packet branch `docs/python-interface-ultraplan` commit `32f801a` is on `main`.
**`origin/main`:** `f6dc68b1a01ae707ac451ac5f3ac6a6d5d0fb3ac`.

## Open PRs

| PR | Purpose | Head | Base | Draft | Mergeable | Checks | Review findings | Disposition |
|---|---|---|---|---|---|---|---|---|
| #49 | P31: objective policies and lexicographic solves (`phase-roml-P31-lexicographic`) | `c7d935a5e08497d3fef0d0f9783bd4003bb9ecfa` | `main` | false (was draft; marked ready 2026-09-07) | MERGEABLE | 17 SUCCESS + 2 SKIPPED on exact head | Executor-found delta-path/P1 defects fixed in `4e69fe2`; independent re-review CLEAR (0 P0/P1); P2s fixed/recorded | MERGED 2026-09-07 as `4cbe13caa9a621b43593da76259e9c5b88fc4d18` |

No other open PRs. No P34 PR exists yet (P34 planned/inactive until P31 merges).

## Executor reproduction on PR #49 head `e0a5efa` (2026-09-07)

Baselines (isolated worktree, Rust 1.97.1): `cargo test -p roml --lib` 318 pass;
`--test objective_policy_faults` 12 pass; `cargo test -p roml-highs --test objective_policy` 8 pass.

New finding (P1, blocks merge until fixed): after delta synchronization (the standard
incremental path), `CompilationSession::compiled_objective_terms` returns EMPTY
coefficient vectors for ordinary objectives, because `compile_delta` updates
`compiled_objective_coefficients` for `SetObjectiveCell`/`SetObjectiveConstant` but NOT
for `SetCell`/`RemoveCell` with `CoefficientTarget::Objective` (the ops emitted by
ordinary `minimize`/`maximize` builds). Consequences reproduced with a controlled
backend probe:
- per-stage `objective_values` and final complete vectors silently evaluate to the bare
  constant (0.0) instead of true values;
- missing-primal rejection (added by `e0a5efa`) never fires on the delta path;
- no-candidate outcomes (infeasible/Unknown) get an invented `[0.0, 0.0]` final vector,
  violating the no-zero-point rule.

Also missing per PR-INTAKE: explicit-options/shared-deadline entry point
(`solve_objective_policy` hardcodes `SolveOptions::default()`; no staged budget),
and per-objective/final-vector value assertions on the HiGHS objective-target regression.

## Merge requirements for #49

Normal merge (no admin bypass) after: (1) SetCell/RemoveCell objective tracking fix with
regressions; (2) final-vector candidate gating; (3) options/deadline entry point with
deterministic-clock tests; (4) HiGHS per-objective/final-vector assertions;
(5) independent re-review with zero P0/P1; (6) exact-head mandatory CI green.
Expected-head comparison mandatory at merge time.
