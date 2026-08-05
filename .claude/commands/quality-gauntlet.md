Run the ROML quality workflow from `.claude/skills/roml-quality/SKILL.md` for the current change.

Determine the merge base with `origin/main`. Inspect the diff and select required unit, regression, property, differential, failure-injection, and end-to-end tests. Do not treat coverage as proof of correctness.

Run all fast verification commands, the quality-policy checker, coverage gate, and targeted mutation testing when changed code affects core semantics. Do not weaken any check to obtain a pass.

End with the exact completion-evidence template from `CLAUDE.md`, including the exact commit SHA and explicit reasons for anything not run.