# ROML Agent Instructions

## Objective

Produce research- and production-grade optimization software through executable evidence. Do not optimize for code volume, novelty, or superficial coverage.

## Required workflow

1. Read repository-local planning and API documents relevant to the change.
2. State the behavioral contract, failure semantics, and compatibility constraints.
3. Write the smallest failing test before implementation for behavior changes.
4. Implement only enough to satisfy the contract.
5. Run the narrow test, then the affected crate suite, then the repository quality gauntlet.
6. Report exact commands, counts, failures, exclusions, and unresolved risks.

Use `.claude/skills/roml-quality/SKILL.md` for test selection and evidence requirements. Use `/quality-gauntlet` before requesting review.

## Test selection

Choose tests by risk, not by habit:

- Unit tests for local logic and validation.
- Regression tests for every discovered defect.
- Property tests for algebraic, normalization, dependency, and state invariants.
- Differential tests for incremental versus clean rebuild behavior, semantic constructs versus direct formulations, and backend equivalence where supported.
- Failure-injection tests for atomicity, rollback, revision advancement, and backend error handling.
- End-to-end examples for public user workflows.

Critical ROML invariants include:

- failed commit leaves frontend and backend committed state unchanged;
- incremental compilation is semantically equivalent to clean recompilation;
- add then remove is observationally equivalent to no change;
- an edit-free commit emits no backend delta;
- dependency tracking is derived from actual expressions;
- all numeric ingress rejects NaN and infinities;
- semantic constructs preserve documented mathematical meaning.

## Protected evidence machinery

Do not weaken, delete, skip, ignore, or bypass tests, assertions, CI jobs, quality thresholds, supported platform lanes, mutation scope, or warning policies to make a change pass.

Changes to these paths require explicit justification in the PR:

- `.github/workflows/**`
- `scripts/check-quality-policy.sh`
- `mutants.toml`
- `CLAUDE.md`
- `.claude/skills/**`
- `.claude/commands/**`

A test may be ignored only with a nearby `quality-exception:` comment that states the owner, reason, and removal condition.

Never replace a precise assertion with a weaker assertion. Never accept `continue-on-error` for a required quality gate. Never lower a threshold without explicit human approval and baseline evidence.

## Human-review boundaries

Require direct design or code review for public API changes, unsafe code, concurrency, ownership/lifetime architecture, solver correctness or numerical tolerances, security boundaries, performance-critical algorithms, and changes to the evidence machinery itself.

## Completion evidence

Report:

```text
Contract criteria: <mapped>/<total>
Focused tests: <command and result>
Affected suites: <command and result>
Property/differential cases: <count and result or not applicable with reason>
Coverage: <line percentage; threshold>
Mutation: <score; killed/survived/timeout/unviable, or not run with reason>
Platforms/backends: <matrix actually exercised>
Quality configuration changed: <yes/no; justification>
Unresolved risks: <none or explicit list>
Exact head: <commit SHA>
```

Do not claim completion without fresh exact-head evidence.