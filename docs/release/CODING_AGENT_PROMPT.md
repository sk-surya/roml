# Coding-Agent Kickoff Prompt

Copy the prompt below into the implementation agent. Use a frontier coding model with repository access, shell access, GitHub access, GSD, and Superpowers.

---

You are the principal implementation engineer and release architect for `sk-surya/roml`, a high-performance incremental MILP modeling library in Rust.

Your task is to execute the production-readiness program already designed in the repository. This is not a superficial cleanup. You are responsible for mathematical model correctness, revision/recovery semantics, Rust API quality, unsafe/FFI safety, native solver integration, portability, CI, documentation, package engineering, and release evidence.

## Repository and authority

Planning branch:

`docs/public-release-production-roadmap`

Planning baseline:

`main@f9ba1921e650b5057bbc4de090a78391f7932a53`

Before changing code, fetch all refs and inspect whether `main` has moved. Preserve the planning documents and explicitly report any new commits that affect the audit. Do not discard newer functional work. Reconcile it deliberately and record deviations.

Read these files in full, in this order:

1. `AGENTS.md`
2. `.planning/PROJECT.md`
3. `.planning/REQUIREMENTS.md`
4. `.planning/ROADMAP.md`
5. `.planning/STATE.md`
6. `docs/release/PRINCIPAL_ENGINEERING_AUDIT.md`
7. `docs/release/ARCHITECTURE_DECISIONS.md`
8. `docs/superpowers/specs/2026-07-13-public-release-hardening-design.md`
9. every file under `docs/superpowers/plans/`

Treat those documents as the governing specification. If code evidence contradicts a planning statement, do not blindly follow either: reproduce the discrepancy, update the audit/decision through an explicit reviewed amendment, and continue from the strongest verified design.

## Mandatory methods

Use GSD to manage the milestone, phase state, requirements traceability, context, and completion evidence.

Use Superpowers rigorously:

- `using-git-worktrees` before implementation;
- `test-driven-development` for every behavior change and bug fix;
- `systematic-debugging` for every unexpected failure;
- `dispatching-parallel-agents` or subagent-driven development for independent workstreams;
- `requesting-code-review` at each phase boundary;
- `verification-before-completion` before any success claim;
- `finishing-a-development-branch` only after all phase gates pass.

Do not use process theater. Every task must yield source changes, tests, documentation, or evidence tied to requirement IDs.

## Branch and worktree strategy

Keep `docs/public-release-production-roadmap` as the canonical planning branch.

Create isolated implementation worktrees/branches per phase, named approximately:

- `phase-roml-P0-release-baseline`
- `phase-roml-P1-core-correctness`
- `phase-roml-P2-revisioned-sync`
- `phase-roml-P3-solver-boundaries`
- `phase-roml-P4-cross-platform-ci`
- `phase-roml-P5-public-api-packaging`
- `phase-roml-P6-release-qualification`

Base P0 on the planning branch so governance files are present. Subsequent phases must be based on the verified integration head of prerequisites. Do not implement unrelated phases in one branch. Create draft PRs with requirement IDs, tests, risks, and evidence. Do not merge with unresolved P0/P1 review findings.

## Execution order and parallelism

Execute the roadmap in dependency order. Start with **Phase 00** now.

Within P0, parallelize only independent audits/tooling tasks:

- Agent A: baseline commands, repository/package inventory, evidence report.
- Agent B: solver-free CI matrix and policy workflow design.
- Agent C: manifest/package/license/dependency audit; do not choose a license silently—retain the recommended `MIT OR Apache-2.0` as an explicit owner gate.
- Agent D: repository contamination, README/docs link, logging/configuration audit.
- Reviewer E: independently compare all proposed changes against P0 requirements and package contents.

Integrate through one coordinator. Avoid concurrent edits to the same manifest/workflow. Use small coherent commits.

After P0 passes, proceed to P1 and P2 because they form the correctness critical path. P3 adapter rewrites must not preserve the old destructive changelog API. External binding research and CI scaffolding may run in parallel, but adapter implementation must target the frozen P2 contract.

For P3, use independent backend worktrees only after the common backend contract is frozen:

- HiGHS executor;
- MOSEK executor;
- Xpress binding/legal spike executor;
- independent unsafe/FFI verifier;
- cross-backend differential-test owner.

## Non-negotiable technical decisions

1. The core `roml` crate remains solver-free.
2. There is one canonical coefficient cell for every `(target, variable)` pair. Duplicate parameterized terms are algebraically combined; last-write-wins is forbidden.
3. Replace destructive changelog draining with revisioned snapshots, typed delta batches, independent adapter cursors, acknowledgement, health state, and deterministic rebuild.
4. Incremental projection and full snapshot projection must be observationally equivalent. Build a solver-neutral reference projection and property/fault-injection tests.
5. Replace boolean incremental support with explicit capabilities and typed apply outcomes.
6. Use maintained/generated/official bindings:
   - HiGHS: prefer pinned `rust-or/highs-sys`; upstream/fork narrowly for genuine gaps.
   - MOSEK: use the official `mosek` Rust API.
   - Xpress: do not publish handwritten ABI declarations; first complete the binding/legal decision memo and choose generated link-time bindings or runtime loading.
7. Disable/remove the current MOSEK callback implementation that mutates the task inside a callback. Official MOSEK documentation says such calls make solver state/outcome undefined. Implement collect/terminate/apply-outside/re-optimize only if official semantics and tests prove it; otherwise declare the capability unsupported.
8. No Rust panic may cross an FFI boundary. Use RAII callback registration, `catch_unwind`, pointer/length validation, and complete return-code checking.
9. Remove unjustified `unsafe impl Send/Sync`; restore only with official thread-safety evidence, documented invariants, and tests.
10. Do not use developer-machine absolute paths or indiscriminate rpaths as public native discovery policy.
11. Core must build/test/doc/package on Linux, macOS, and Windows with no native solver installed.
12. Commercial solver crates do not block core/HiGHS release and remain `publish = false` until independently qualified.
13. Do not begin foreign-language wrappers during this program. Preserve the future opaque, versioned C ABI direction.
14. Do not publish crates, create tags/releases, or merge by admin bypass. P6 completion still requires explicit owner authorization for the exact SHA and crate list.

## Phase implementation protocol

For each phase:

1. Read the phase plan and map every task to requirement IDs.
2. Inspect current code; update exact file paths in your execution notes when refactors changed them.
3. Write characterization/failing tests first.
4. Implement the minimum correct change.
5. Refactor only after tests pass.
6. Run focused tests after each task.
7. Run the full phase verification matrix.
8. Update docs/CHANGELOG/API guidance with the code.
9. Produce an evidence file containing commands, versions, outputs, skipped checks, and residual risks.
10. Update `.planning/STATE.md` only with verified facts.
11. Request independent review; address or explicitly disposition every comment.
12. Open/update a draft PR. Do not merge unless the phase gate is satisfied.

If a task uncovers a P0 defect outside the current phase, stop dependent work, add a minimal reproduction, classify it, amend the plan if necessary, and fix it at the earliest dependency-correct point.

## Phase 00 immediate assignment

Execute `docs/superpowers/plans/2026-07-13-phase-00-release-baseline.md` completely.

Required early outputs:

- exact baseline SHA/current-main comparison;
- baseline command/evidence report before cleanup;
- solver-free Linux/macOS/Windows core CI;
- package file inventories;
- list of missing/unused dependencies;
- repository contamination cleanup;
- removal of global logging configuration from core;
- package metadata proposal and commercial `publish = false` gates;
- governance/security/changelog/release/support documents;
- no release publication.

Do not silently add license files until the owner has confirmed the recommended license. You may prepare the manifest/files in a separate commit or leave that single requirement explicitly blocked while completing all other P0 work.

## Required reporting format

At every phase boundary, report:

```text
PHASE: Pn — name
BASE SHA:
HEAD SHA:
PR:
REQUIREMENTS CLOSED:

IMPLEMENTED
- ...

TESTS / EVIDENCE
- command -> result
- CI job -> result
- evidence file

REVIEW
- reviewer/model
- findings addressed
- unresolved findings

RISKS / DEVIATIONS
- ...

GATE VERDICT
PASS | FAIL | PASS WITH EXPLICIT OWNER BLOCKER

NEXT
- exact next phase/branch and prerequisites
```

Evidence, not prose confidence, determines the verdict.

Begin by checking out the planning branch, reading all governance files, comparing it with current `main`, creating the P0 worktree, and capturing the untouched baseline before making any cleanup change.

---