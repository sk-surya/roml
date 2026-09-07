# Pending PR Intake and Prerequisite Closure

## Authority

The 2026-09-07 owner instruction authorizes the coding agent to check and merge pending PRs before Python implementation. Apply that authority to reviewed, relevant ROML prerequisite work, including the planning PR containing this packet. Do not ask again for routine prerequisite merges after all gates pass. Do not bypass branch protection, fabricate approvals, dismiss unresolved reviews without remediation, or merge an unrelated newly discovered feature merely because it is open. Record unrelated PRs with an explicit disposition and continue when they do not affect this path.

## Intake procedure

- [ ] Preserve all existing uncommitted work. Fetch refs; list open PRs, their exact head/base SHAs, mergeability, drafts, review threads, checks and dependencies. Record the repository's actual merge/ruleset requirements.
- [ ] Create `evidence/PR-INVENTORY.md` with columns: PR, purpose, head, dependencies, review findings, required checks, disposition, merge SHA. Re-query before each merge; expected-head comparison is mandatory.
- [ ] Review the planning PR containing this packet and merge it through normal checks, if not already merged, to establish canonical routing. Documentation tests may run before P31 closes; do not mistake this for Python implementation.
- [ ] Fetch each relevant PR's actual patch, comments, tests and evidence. A draft or green badge is neither rejection nor acceptance. Fix defects in that PR or a clearly dependent remediation branch. Re-run affected tests and current mandatory CI.
- [ ] Obtain independent review of the final code. A separate reviewer agent is permitted if available; preserve its actual findings. Tool-generated approvals must not impersonate a human reviewer or bypass a required independent GitHub approval.
- [ ] Merge P31 only after its math/lifecycle issues are addressed. Use normal merge mechanisms with expected head. If the head changes, refresh review and affected checks. Never use admin merge or force-push another person's work.
- [ ] Record merge SHA, fetch main, verify the commit is present, and run the existing P34 qualification contract on this integrated baseline. Implement narrowly required fixes, qualify/review the P34 closure PR and normally merge it. Root state must truthfully say M3 complete before MPY runtime implementation.
- [ ] Reconcile stale `AGENTS.md` historical architecture/defect assertions against code, preserving useful regression targets as history. Amend the old wrapper/C-ABI direction explicitly using this packet, rather than silently treating history as current authority.

## Known P31 review targets, not assumed current facts

Observed PR #49 head: `e0a5efa736aa6dc46408204ae6fd2194e355dfab`. All five associated workflows reported success at planning time. Linux Core logs showed 1,224 passed and two skipped on a synthetic merge. The latest written review preceded the latest remediation. Neither CI nor old review comments replace inspecting the current code.

### Objective completeness

Inspect `src/solver/objective_executor.rs`, `src/solver/facade.rs`, `src/solver/objective_combine.rs`, `roml-highs/tests/objective_policy.rs` and `tests/objective_policy_faults.rs` on the PR head.

The observed latest code trusts a finite backend scalar for a lock, but `evaluate_objective` and the portable scalar fallback still skip compiled terms that have no canonical user-variable mapping. P30 objective-targeted penalty variables have this shape. Add a regression using `g(x)=x+0.5*max(0,6-x)` on `0<=x<=10`: the optimum is x=0 and g=3. Assert backend scalar, stage lock, per-objective vector and final vector all retain the full penalty; the existing fixture only checks some of these.

Also cover normalized maximization, non-unit weights, parameterized penalties, nonzero degradation tolerance, missing reported scalar, and missing required primal evidence. Required evidence that cannot be reconstructed exactly must return a typed error, never be treated as zero. A raw objective and a penalized objective can coexist only with explicit separately named semantics and an accepted contract change; do not call an incomplete value the canonical objective.

### No-incumbent and cleanup interaction

The source review found unconditional final-vector extraction after staged termination worth reconciling with the newly strict missing-primal evaluator. Existing hosted tests reported successful infeasible/Unknown cases. Reproduce on the actual integrated SHA before asserting a bug. Exercise infeasible/Unknown/time-limit-without-incumbent and ensure final-vector evaluation is gated on actual candidate availability. An absent solution cannot trigger evaluation at an invented zero point. Preserve rollback, dirty-state recovery and composite errors.

### MPC solve budget

The observed P31 `solve_objective_policy` calls `synchronize_base(model, SolveOptions::default())`; it does not expose options or a total deadline. Add a backwards-compatible explicit-options/deadline entry point in a focused prerequisite commit/PR after the math fix. Keep existing default behavior available. Use the remaining budget across stages, preserve the last valid incumbent on exhaustion, and distinguish incumbent availability from termination status. Test with a deterministic clock; do not rely on flaky sleep-based assertions. Keep this hardening out of unrelated M3 qualification claims if it lands as a separate prerequisite PR.

## Stop conditions

Stop only for a concrete unresolved authority/security issue, missing required credential or human approval, incompatible scope change, reproducible correctness defect that cannot be safely resolved, or an inaccessible mandatory execution target. Record exact action, evidence and one next step. Routine remediation, toolchain setup, CI fixes and review iterations are authorized work; do not stop after discovering them or produce a planning-only handoff.
