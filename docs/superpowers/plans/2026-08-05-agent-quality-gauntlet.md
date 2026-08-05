# Agent Quality Gauntlet Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add durable agent testing instructions and enforce coverage, QA-policy integrity, and mutation testing for ROML.

**Architecture:** Keep the existing cross-platform CI unchanged. Add one Linux quality workflow for coverage and policy checks, plus scheduled/manual mutation testing. Store agent behavior in `CLAUDE.md`, an executable skill, and one slash command.

**Tech Stack:** Rust 1.85+, GitHub Actions, cargo-nextest, cargo-llvm-cov, cargo-mutants, Bash.

## Global Constraints

- Do not lower existing test or lint strictness.
- Do not add runtime dependencies to ROML.
- Keep expensive mutation testing off the ordinary PR critical path.
- Coverage threshold starts at 75%; mutation threshold starts at 80%.
- Any exception must be explicit, narrow, documented, and reviewable.

---

### Task 1: Durable agent policy

**Files:**
- Create: `CLAUDE.md`
- Create: `.claude/skills/roml-quality/SKILL.md`
- Create: `.claude/commands/quality-gauntlet.md`

- [ ] Define authority boundaries and mandatory test selection.
- [ ] Prohibit silent weakening of tests, workflows, thresholds, and assertions.
- [ ] Define the exact evidence report agents must produce.
- [ ] Expose one `/quality-gauntlet` command.
- [ ] Review all instructions for contradictions and unsupported commands.

### Task 2: Human testing strategy

**Files:**
- Create: `docs/quality/testing-strategy.md`

- [ ] Document test layers and when each is required.
- [ ] Document coverage, mutation, and complexity-risk interpretation.
- [ ] Document exception and ratchet policy.
- [ ] Document the boundary where human code/design review remains mandatory.

### Task 3: Policy regression check

**Files:**
- Create: `scripts/check-quality-policy.sh`
- Create: `tests/quality_policy.bats`

- [ ] Write tests for newly added ignored tests, allowed marked exceptions, and threshold reductions.
- [ ] Run the tests and confirm they fail before the checker exists.
- [ ] Implement the smallest checker that passes them.
- [ ] Run the policy tests and confirm all pass.

### Task 4: Coverage and mutation CI

**Files:**
- Create: `.github/workflows/ci-quality.yml`
- Create: `mutants.toml`

- [ ] Add a PR/push coverage job using `cargo llvm-cov --fail-under-lines 75`.
- [ ] Run the policy checker against the PR base diff.
- [ ] Add scheduled/manual mutation testing with an 80% score gate.
- [ ] Upload coverage and mutation reports as artifacts.
- [ ] Verify workflow YAML and shell syntax in GitHub Actions.

### Task 5: Integration evidence

- [ ] Open a draft PR.
- [ ] Read exact-head workflow results.
- [ ] Correct any failures without lowering thresholds.
- [ ] Record actual coverage and mutation evidence in the PR.
- [ ] Leave the PR draft if scheduled mutation evidence is not yet available.
