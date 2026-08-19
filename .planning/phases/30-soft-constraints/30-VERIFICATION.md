---
phase: 30-soft-constraints
status: complete-merged
head: "reviewed code/test head c5c1c33ed3d7fd5e994c5b4f4f974b197c9b0667; this closure commit is documentation-only"
base: 5fe8f5b0f60831b438364a73300b4306dfa6d195
review: 30-CODE-REVIEW-7.md
---

# P30 Verification

The P30 implementation and review remediation are qualified at implementation/
test head `c5200fd84836e3a5d0b435368860538235cce330`, with the reviewed
code/test PR head `c5c1c33ed3d7fd5e994c5b4f4f974b197c9b0667` followed only by
this documentation closure commit. The changes fix two-sided
replacement-bound composition, keep declared variable bounds independent from
persistent fixing, apply feasibility tolerance to candidate-domain checks, and
exercise both acceptance outcomes for an actual `TerminationStatus::Feasible`
result. Exact-head hosted CI and independent review are green. P30 merged via
PR #47 as `28a019e83a40f2c7df637290c48ad23d7d568ec9`; P31 is now eligible for
activation by the milestone routing state.

## Fresh exact-head checks

- `cargo test -p roml --all-targets --locked --quiet` — 293 library tests plus all integration targets passed.
- Focused P30 matrix (`feasibility_relaxation`, `feasibility_relaxation_faults`, `relaxation_provider_policy`, `feasibility_relaxation_p29`, `soft_constraints_qualification`) — 27 passed, including end-to-end P29→P30 execution.
- `cargo test -p roml-highs --all-targets --locked --quiet` — passed with bundled HiGHS 1.15.0.
- `cargo fmt --all -- --check` and `git diff --check` — passed.
- `cargo clippy -p roml --all-targets --locked -- -D warnings` — passed.
- `cargo clippy -p roml-highs --all-targets --locked -- -D warnings` — passed.
- `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps --locked` — passed.
- `cargo package --list -p roml --locked` — 179 files.
- `cargo package --list -p roml-highs --locked` — 51 files.
- `bash scripts/test-quality-policy.sh` — 4 policy tests passed.

## Review and residuals

- `30-CODE-REVIEW-5.md` — prior review; its verdict does not cover this remediation head.
- `30-CODE-REVIEW-6.md` — prior remediation review; it does not cover the two
  new test additions.
- New core regressions cover the P29 fixed-variable bound seam and both sides of
  the feasible-acceptance outcome contract.
- Exact-head hosted CI passed on `c5c1c33ed3d7fd5e994c5b4f4f974b197c9b0667`:
  Core, HiGHS Backend, Coverage, Quality, and Policy.
- `30-CODE-REVIEW-7.md` records the independent final review disposition:
  CLEAR with no P0/P1/P2/P3 findings.
- No publication, tag, or release action was performed. P30 owner merge is
  complete; P31 activation is handled by the post-merge routing update.
- MOSEK/Xpress SDK, license, ABI, and native-relaxation qualification remain outside this P30 HiGHS/local gate.
- P31 priority/lexicographic execution remains a separate phase and is not
  claimed by this evidence.
- No publication, tag, or release action was taken.
