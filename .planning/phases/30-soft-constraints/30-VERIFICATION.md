---
phase: 30-soft-constraints
status: passed-local-remediation-review-pending
head: 4c6f796
base: 5fe8f5b0f60831b438364a73300b4306dfa6d195
review: 30-CODE-REVIEW-7.md
---

# P30 Verification

The P30 implementation and review remediation are locally qualified at
`4c6f796`. This head fixes two-sided
replacement-bound composition, keeps declared variable bounds independent from
persistent fixing, applies feasibility tolerance to candidate-domain checks,
and exercises acceptance of an actual `TerminationStatus::Feasible` result.
The prior review is superseded for this head. Independent final re-review and
hosted exact-head CI for this test-bearing head remain pending. P31 remains
inactive until the owner-authorized P30 merge gate is satisfied.

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
- Final independent re-review and hosted exact-head CI remain pending.
- No publication, tag, release, owner merge, or P31 activation was performed.
- MOSEK/Xpress SDK, license, ABI, and native-relaxation qualification remain outside this P30 HiGHS/local gate.
- Hosted mandatory CI and owner-authorized merge remain required before P31 activation.
- No publication, tag, or release action was taken.
