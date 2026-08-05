---
name: roml-quality
description: Use for every ROML behavior change, bug fix, refactor, test addition, or pre-review verification.
---

# ROML Quality Workflow

## 1. Define the contract

Record the public behavior, failure behavior, compatibility surface, and the production change that would make each planned test fail.

## 2. Select independent evidence

Use the smallest sufficient combination:

- unit: local deterministic behavior;
- regression: every concrete defect;
- property: broad invariants over generated inputs;
- differential: incremental vs rebuild, semantic vs direct formulation, or backend vs backend;
- failure injection: atomicity, rollback, cursor/revision health, backend failure;
- end-to-end: public modeling and solve workflows.

A line-coverage-only change is insufficient.

## 3. Red-green-refactor

For behavior changes, run the new focused test before implementation and confirm the expected failure. Implement minimally, rerun focused tests, then affected suites.

## 4. Protect the gauntlet

Reject any unapproved change that weakens assertions, removes tests, adds ignored tests without `quality-exception:`, lowers thresholds, removes platforms/backends, narrows mutation scope, or makes required jobs non-blocking.

## 5. Run verification

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace --all-targets
cargo test --workspace --doc
cargo doc --workspace --no-deps
bash scripts/check-quality-policy.sh "${QUALITY_BASE_REF:-origin/main}"
cargo llvm-cov --workspace --all-features --all-targets --fail-under-lines 75
```

Set `RUSTDOCFLAGS='-D warnings'` for documentation verification.

Run targeted mutation testing for changed core semantics when feasible:

```bash
cargo mutants --package roml --in-diff origin/main --output mutants.out
python3 scripts/check-mutation-score.py mutants.out/outcomes.json 80
```

Full mutation testing is handled by scheduled/manual CI.

## 6. Report evidence

Use the exact evidence template in `CLAUDE.md`. State anything not run and why. Never infer success from stale or partial output.