# Agent Quality Gauntlet Design

## Objective

Make agent-produced ROML changes trustworthy without requiring line-by-line implementation review. The repository must generate independent evidence about behavior, regressions, complexity risk, and test strength while preventing agents from weakening the evidence machinery.

## Design

The durable policy lives in `CLAUDE.md`. The executable workflow lives in `.claude/skills/roml-quality/SKILL.md` and is exposed through one operator command, `/quality-gauntlet`. Human rationale and exception policy live in `docs/quality/testing-strategy.md`.

Every pull request retains the existing cross-platform core checks and gains a Linux quality workflow that enforces coverage, policy integrity, and mutation-test readiness. Full mutation testing is scheduled and manually runnable because its runtime is unsuitable for every small PR.

## Required evidence

Changes must select the smallest relevant set from:

- focused unit tests;
- regression tests for every defect;
- property tests for algebraic and state invariants;
- differential tests against clean rebuilds, direct formulations, or another backend;
- failure-injection tests for atomicity and recovery;
- end-to-end examples for public workflows;
- coverage and mutation reports.

## Initial gates

- Core line coverage must remain at or above 75%.
- No newly added ignored Rust tests are allowed without the marker `quality-exception:` on the same or preceding line.
- Changes to protected QA files require explicit review and must not silently lower thresholds.
- Scheduled mutation testing must achieve at least 80% killed mutants after excluding timeouts and unviable mutants.

Thresholds are ratchets, not targets. Raise them after obtaining stable baseline evidence; do not lower them to make a change pass.

## Scope

Gherkin is not introduced now. ROML's Rust integration tests express current contracts more directly. Add Gherkin only when a workflow requires review by non-Rust domain stakeholders.
