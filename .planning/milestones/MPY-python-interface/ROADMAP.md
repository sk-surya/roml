# MPY Execution Roadmap

| Phase | End state | Depends on | Exit gate |
|---|---|---|---|
| MPY-00 | Current prerequisite PRs reviewed/remediated/merged; P34 closed; budget hardening accepted | owner instruction | PR intake ledger and verified main |
| MPY-01 | Exact dependencies and native binding skeleton qualified | MPY-00 | core isolation + import from built wheel |
| MPY-02 | Neat typed scalar modeling API | MPY-01 | golden LP, identity/errors, API ergonomics review |
| MPY-03 | Shaped bulk modeling and atomic updates | MPY-02 | array/CSR correctness, transactional failures and scale evidence |
| MPY-04 | Persistent solve/result/lifecycle contract complete | MPY-03 | outcomes/options/concurrency/fault gates |
| MPY-05 | MPC integration and performance qualified | MPY-04 | equivalence, benchmark and memory thresholds |
| MPY-06 | Portable distribution and final handoff | MPY-05 | installed-wheel matrix, sdist, docs, independent final review |

## Integration and WIP

One active implementation phase and one review gate at a time. Use a dedicated branch/worktree for each prerequisite fix where necessary; MPY itself may use a linear implementation branch with small reviewed commits and a draft PR. Reviewer agents may run independently on completed work. Parallel coding is optional only for disjoint tests/docs after the public contract is frozen; never grow integration WIP merely because workers exist.

After every phase update STATE and the requirement ledger with: objective, exact baseline/head, completed evidence, decisions/deviations, blockers, next gate. Do not repeat all historical work in progress updates. Resume from this ledger after interruption.

Existing M3 contracts still govern P31/P34. This successor routing explicitly prioritizes MPY over M4; it does not authorize quadratic/nonlinear code. Planning work may coexist with P31 review. Runtime Python code starts only after MPY-00 prerequisites pass.

## Acceptance checkpoints

- After MPY-00: merge evidence and only unresolved issues that affect bindings.
- After MPY-02: show the golden LP and a concise explanation of ownership. Perform a reviewer ergonomics check without adding a routine owner approval pause.
- After MPY-04: show the same model solved twice, an old result remaining valid, a rejected bad update, and a limit without an incumbent.
- After MPY-05: show measured wrapper overhead and replay correctness; promote no unmeasured speed claim.
- After MPY-06: return draft implementation PR, exact head, install command, passing/blocked matrix, benchmark summary and residual limits. No PyPI publication.

## Completion predicate

MPY is complete iff PY-01 through PY-35 have passing evidence, required CI and final independent review apply to the delivered code, there are no unresolved P0/P1 findings, wheel/source artifacts are retained in CI, and STATE matches repository reality. A skeleton, editable install, README screenshot, test count, or successful single LP is not completion.
