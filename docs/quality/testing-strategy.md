# ROML Testing Strategy

ROML uses layered, partially independent evidence. No single metric proves correctness.

## Test layers

- **Unit tests** isolate validation, lowering, normalization, and state transitions.
- **Regression tests** reproduce every discovered defect and fail when the defect is reintroduced.
- **Property tests** exercise invariants over generated models and edit sequences.
- **Differential tests** compare incremental compilation with clean rebuilds, semantic constructs with direct formulations, and multiple backends where capabilities overlap.
- **Failure-injection tests** prove atomicity and recovery under validation, compilation, and backend failures.
- **End-to-end tests/examples** prove public workflows compile, solve, and report expected metadata.

Gherkin is reserved for workflows that need review by non-Rust stakeholders. It is not a substitute for precise integration tests.

## Coverage

Coverage answers whether code executed, not whether assertions detect faults. The core crate has an initial 75% line-coverage gate. The threshold is a ratchet: raise it from measured exact-head baselines; never lower it merely to pass a change.

A high-risk uncovered branch must be tested even if repository-wide coverage remains above the threshold.

## Mutation testing

Mutation testing changes operators, conditions, constants, and statements and checks whether tests fail. The scheduled/manual mutation workflow requires an 80% score:

`killed / (killed + survived)`

Timeouts and unviable mutants are reported separately. Every survivor in validation, commit atomicity, semantic lowering, dependency tracking, or backend delta generation must be killed, removed as meaningless code, or explicitly justified.

## Complexity and CRAP risk

CRAP combines cyclomatic complexity and coverage to prioritize risky functions. ROML does not gate directly on a CRAP implementation because Rust tool support is inconsistent. Reviewers should treat increased branching plus weak focused coverage as a blocker regardless of the global coverage score.

## QA policy

Agents may not weaken the evidence machinery. Newly ignored tests require a nearby `quality-exception:` comment with owner, reason, and removal condition. Threshold reductions, removed CI lanes, weaker assertions, narrowed mutation scope, or non-blocking required jobs require explicit human approval.

## Human review remains mandatory

Executable evidence does not replace review for public API design, numerical tolerances, solver correctness, unsafe code, concurrency, ownership architecture, security boundaries, performance-critical algorithms, or changes to QA policy and CI.
