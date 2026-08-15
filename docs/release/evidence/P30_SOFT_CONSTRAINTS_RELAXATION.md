# P30 Soft Constraints and Portable Relaxation Evidence

Evidence was collected on implementation/test head `c5200fd84836e3a5d0b435368860538235cce330` in the isolated `agent-p30-execute` worktree. The current PR branch is a documentation/formatting-only continuation of that tested head; its hosted exact-head status is tracked on PR #47. The implementation/test head includes the owner-approved P30 decisions D-01 through D-15, the audit fixes, inactive-variable evidence handling, two-sided replacement-bound composition, independent declared-bound/persistent-fixing semantics, feasibility-tolerance candidate validation, actual-Feasible acceptance coverage, the end-to-end P29 fixed-variable bound seam regression with preserved provenance, and the paired feasible-acceptance outcome regression; P31 priority/lexicographic execution and unqualified native relaxation remain out of scope.

## Qualification ledger

| P30 surface | Evidence | Result |
|---|---|---|
| SM-10.1 persistent handle/lifecycle | `soft_constraints_contract`, `soft_constraints_lifecycle` | pass |
| SM-10.2 exact lower/upper/equality/ranged bridge | `soft_constraints_algebra`, `soft_constraints_qualification` | pass |
| SM-10.3 finite caps and atomic numeric validation | `soft_constraints_algebra`, `soft_constraints_qualification` | pass |
| SM-10.4 parameterized weights and objective-sense normalization | `soft_constraints_algebra`, `soft_constraints_qualification` | pass |
| SM-10.5 original-constraint solution violation/correction accessors | `soft_constraint_solution` | pass |
| SM-10.6 portable weighted-L1 repair contract | `feasibility_relaxation`, `relaxation_provider_policy` | pass |
| SM-10.7 outcome/acceptance/provider semantics | `feasibility_relaxation`, provider policy implementation | pass |
| SM-10.8 overlay cleanup and rebuild boundary | existing overlay fault corpus plus P30 cleanup tests; extraction paths preserve cleanup | pass |
| SM-10.9 P29 composition and qualification evidence | `feasibility_relaxation_p29`, `feasibility_relaxation`, real HiGHS differential tests, this ledger | pass |

The portable repair report records model lineage/instance/revision, distinct
base and relaxation compilation identities, provider, termination, numeric
objective evidence, evaluated weights, restriction identity, and the isolated
candidate `Solution`. Successful reports are emitted only after rollback and
clean verification.

## Commands and outcomes

All commands ran at the candidate head above with Rust `1.97.1` and Cargo
`1.97.1`:

```text
cargo fmt --all -- --check                         PASS
cargo test -p roml --test feasibility_relaxation \
  --test feasibility_relaxation_faults \
  --test relaxation_provider_policy \
  --test feasibility_relaxation_p29 \
  --test soft_constraints_qualification --quiet       PASS (27 tests)
cargo test -p roml --all-targets --locked --quiet       PASS (293 library tests; all targets)
cargo test -p roml-highs --all-targets --locked --quiet PASS (native HiGHS 1.15.0)
cargo clippy -p roml --all-targets --locked -- -D warnings PASS
cargo clippy -p roml-highs --all-targets --locked -- -D warnings PASS
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps --locked PASS
cargo package --list -p roml                         PASS (179 files)
cargo package --list -p roml-highs                   PASS (51 files)
bash scripts/test-quality-policy.sh                  PASS (4 policy tests)
git diff --check                                     PASS
```

The final full core run completed 293 library tests and all integration targets,
including the new end-to-end P29 and outcome-contract tests. The
HiGHS run completed its native adapter and integration matrix, including real
lower-side violations with both row sides and both variable bounds selected.
Native license,
MOSEK, and Xpress qualification were not required by P30 and were not claimed
as passes.

## Semantic and safety fences

- Persistent softening is canonical/revisioned; repair is a temporary overlay.
- Portable repair is weighted L1 only, with `PortableOnly` as the default.
- `PreferNative` records an explicit portable fallback; `NativeRequired`
  rejects before synchronization when no qualified provider exists.
- `RequireOptimal` does not promote an unproven feasible incumbent; `NoRepairFound`
  is reserved for a proven infeasible permitted relaxation.
- P29 mapping accepts only primitive/imported constraint sides, variable-bound
  sides, and persistent fixings. Unsupported or stale members fail all-or-error.
- IIS membership is diagnostic scope only; no minimum-cardinality or
  minimum-weight repair guarantee is made.
- P31 owns objective-priority and lexicographic execution. No native relaxation
  implementation is claimed by this phase.

## Review and residual risk

The implementation was reviewed against AGENTS.md, SHARED-CONTRACTS.md, and
locked P30 decisions. Core and HiGHS automated gates pass locally. The prior
reviews are superseded for this test-remediation head; independent final review
and hosted exact-head CI remain required. Residual risks are
limited to deferred backend qualification (MOSEK/Xpress/native relaxation and
license-specific runs) and the planned P31 priority/lexicographic layer. No
crate was published, no tag was created, and no release action was taken.

## Closure disposition

P30 is locally implementation- and evidence-complete at the test-remediation
head, pending independent final review and hosted exact-head CI. P31 must not
be activated by this phase's state update; its work may start only after the
owner accepts this evidence and the P31 plan is executed.
