# P36 MPS write-back qualification

This record covers the verified P36 implementation work on the isolated
`phase-roml-P36-mps-writeback` branch. It is evidence for review, not a claim
that the phase is merge-complete until the remaining Wave 4 gates pass.

## Frozen corpus

- Source: `sk-surya/lp-data-netlib`
- Required commit: `56257eea85b433ce6aa67d26156b36385318fd6f`
- Manifest: `.planning/phases/36-mps-writeback/36-NETLIB-MANIFEST.md`
- Inventory: exactly 94 regular `.mps` files
- Observed inventory contract: PASS

## Qualification command

```text
cargo run -p roml-highs --example mps_write_corpus_qualification -- /tmp/roml-p36-source
```

Observed result on candidate `e02b4fb` (the final tolerance-fix commit before
this evidence refresh):

- manifest count: 94
- writer output: 94/94 PASS
- deterministic repeated write: 94/94 PASS
- independent ROML mathematical round-trip: 94/94 PASS
- native HiGHS structure comparison: 94/94 PASS
- bounded solve subset: `blend.mps`, `fit2d.mps`, `gfrd-pnc.mps`; all PASS
- process exit: 0
- bundled native solver: HiGHS 1.15.0

The complete runner was rerun after the P36 tolerance correction; the final
process exit remained 0 with 94/94 PASS rows.

The runner writes machine-readable per-file rows under
`target/roml-corpora/p36-mps-writeback`. Those generated artifacts are not
repository sources or package contents.

## Focused oracle evidence

```text
cargo test -p roml --test mps_write_roundtrip
5 passed, 0 failed

cargo test -p roml-highs --test mps_write_highs_oracle --test mps_write_corpus_contract
5 passed, 0 failed
```

The ROML oracle includes hand-built LP/MILP cases, parameterized evaluated
snapshot coverage, and fixed-seed randomized primitive LP/MILP cases. The
HiGHS oracle covers direct ROML-to-HiGHS versus MPS-to-`Highs_readModel`, full
structure, optimal LP/MILP, infeasible, unbounded, ranged, free-integer, and
no-objective cases.

## Remaining Wave 4 evidence

The following local checks passed after the qualification runner:

```text
cargo fmt --all -- --check                                  PASS
cargo test -p roml --all-targets                            PASS
cargo test -p roml-highs --all-targets                      PASS
cargo clippy -p roml --all-targets -- -D warnings           PASS
cargo clippy -p roml-highs --all-targets -- -D warnings     PASS
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps      PASS
RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps PASS
cargo package --list -p roml                                PASS
cargo package --list -p roml-highs                          PASS
cargo check -p roml --target x86_64-pc-windows-gnu --tests PASS
cargo run -p roml --example mps_write                       PASS
git diff --check                                            PASS
```

The following must still be recorded on the final candidate head before P36 closure:

- local/native HiGHS Windows runtime-path qualification in addition to the
  verified Windows GNU compile check;
- independent full implementation review and any remaining exact-head hosted
  checks;
- final P36 verification, review, summary, and owner-authorized merge.

## Hosted Windows Core qualification

At the exact PR #46 head `c017d1aa61c0079086d6fdf80bb39cc77b08172c`, the hosted
GitHub Actions `CI - Core / Test (windows-latest)` job passed
([run/job](https://github.com/sk-surya/roml/actions/runs/31359712077/job/93365995014)).
That solver-free Core job runs ROML all-target compilation and tests, in
addition to formatting, Clippy, doctests, and rustdoc. This is a PASS for
hosted Windows Core target/test qualification.

Qualification boundary: the Core job does not by itself qualify the P36
`roml-highs` writer/oracle or a native HiGHS Windows runtime path. The local
Linux environment still has no Windows/Wine runtime host, so local/native
HiGHS Windows runtime qualification remains unverified and is not marked
PASS.
