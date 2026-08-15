---
phase: 30-soft-constraints
status: passed-exact-head-clear-to-merge
head: ccbd89a4bc0f6da501e5821894e853aa932a2d23
base: 5fe8f5b0f60831b438364a73300b4306dfa6d195
review: 30-CODE-REVIEW-5.md
---

# P30 Verification

The P30 implementation and review remediation are qualified at
`ccbd89a4bc0f6da501e5821894e853aa932a2d23`. This head fixes two-sided
replacement-bound composition, keeps declared variable bounds independent from
persistent fixing, applies feasibility tolerance to candidate-domain checks,
and exercises acceptance of an actual `TerminationStatus::Feasible` result.
The prior review is superseded for this head. Independent final review
`30-CODE-REVIEW-6.md` is CLEAR TO MERGE, and hosted exact-head CI is green.
Owner merge remains pending. P31 remains inactive until the owner-authorized
P30 merge gate is satisfied.

## Fresh exact-head checks

- `cargo test -p roml --all-targets --locked --quiet` — 293 library tests plus all integration targets passed.
- Focused P30 matrix (`feasibility_relaxation`, `feasibility_relaxation_faults`, `relaxation_provider_policy`, `feasibility_relaxation_p29`, `soft_constraints_qualification`) — 25 passed.
- `cargo test -p roml-highs --all-targets --locked --quiet` — passed with bundled HiGHS 1.15.0.
- `cargo fmt --all -- --check` and `git diff --check` — passed.
- `cargo clippy -p roml --all-targets --locked -- -D warnings` — passed.
- `cargo clippy -p roml-highs --all-targets --locked -- -D warnings` — passed.
- `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps --locked` — passed.
- `cargo package --list -p roml --locked` — 179 files.
- `cargo package --list -p roml-highs --locked` — 51 files.
- `bash scripts/test-quality-policy.sh` — 4 policy tests passed.
- Hosted exact-head CI at `ccbd89a4bc0f6da501e5821894e853aa932a2d23` — Core,
  HiGHS, Coverage, Quality, and Policy all passed.

## Review and residuals

- `30-CODE-REVIEW-5.md` — prior review; its verdict does not cover this remediation head.
- `30-CODE-REVIEW-6.md` — independent final remediation review; CLEAR TO MERGE,
  no P0/P1/P2/P3 findings.
- New core Reference and real HiGHS regressions cover both-sided row/variable relaxation and fixing/declared-bound independence; tolerance and actual-Feasible acceptance gaps are covered.
- Owner merge remains pending; no publication, tag, release, or P31 activation
  was performed.
- MOSEK/Xpress SDK, license, ABI, and native-relaxation qualification remain outside this P30 HiGHS/local gate.
- Hosted mandatory CI and owner-authorized merge remain required before P31 activation.
- No publication, tag, or release action was taken.
