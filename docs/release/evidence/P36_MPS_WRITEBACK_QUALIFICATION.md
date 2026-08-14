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

Observed result on exact implementation candidate `0a4e732c8d884b4ccc6d8248290a7e0bf6555f09`:

- manifest count: 94
- writer output: 94/94 PASS
- deterministic repeated write: 94/94 PASS
- independent ROML mathematical round-trip: 94/94 PASS
- native HiGHS structure comparison: 94/94 PASS
- bounded solve subset: `blend.mps`, `fit2d.mps`, `gfrd-pnc.mps`; all PASS
- process exit: 0
- bundled native solver: HiGHS 1.15.0

The exact-head runner completed with process exit 0, 94/94 PASS rows, and zero FAIL rows.
The three bounded solve-subset models (`blend.mps`, `fit2d.mps`, `gfrd-pnc.mps`) also passed.

The runner writes machine-readable per-file rows under
`target/roml-corpora/p36-mps-writeback`. Those generated artifacts are not
repository sources or package contents.

## Focused oracle evidence

```text
cargo test -p roml --test mps_write_roundtrip
6 passed, 0 failed

cargo test -p roml --test mps_write_report --test mps_write_integration --test mps_write_projection
32 passed, 0 failed

cargo test -p roml-highs --test mps_write_highs_oracle --test mps_write_corpus_contract
5 passed, 0 failed
```

The ROML oracle includes hand-built LP/MILP cases, parameterized evaluated
snapshot coverage, and fixed-seed randomized primitive LP/MILP cases. The
HiGHS oracle covers direct ROML-to-HiGHS versus MPS-to-`Highs_readModel`, full
structure, optimal LP/MILP, infeasible, unbounded, ranged, free-integer, and
no-objective cases.

## Remaining Wave 4 evidence

The following local checks passed on implementation candidate `0a4e732c8d884b4ccc6d8248290a7e0bf6555f09`:

```text
cargo fmt --all -- --check                                  PASS
cargo test -p roml --all-targets --locked                       PASS
cargo test -p roml-highs --all-targets --locked                 PASS
git diff --check                                            PASS
```

Clippy, rustdoc, package-list, and Windows GNU checks from the prior
qualification record were not rerun after `0a4e732`; they remain pending exact-head
hosted CI confirmation.

The following must still be recorded on the final candidate head before P36 closure:

- local/native HiGHS Windows runtime-path qualification in addition to the
  verified Windows GNU compile check;
- independent full implementation review and any remaining exact-head hosted
  checks;
- final P36 verification, review, summary, and owner-authorized merge.

## Hosted Windows Core qualification

The prior hosted Windows Core qualification at `c017d1aa61c0079086d6fdf80bb39cc77b08172c` remains historical evidence.
The current implementation candidate `0a4e732c8d884b4ccc6d8248290a7e0bf6555f09`
requires a fresh exact-head hosted run after push; no current-hosted PASS is claimed here.

Qualification boundary: the Core job does not by itself qualify the P36
`roml-highs` writer/oracle or a native HiGHS Windows runtime path. The local
Linux environment still has no Windows/Wine runtime host, so local/native
HiGHS Windows runtime qualification remains unverified and is not marked
PASS.
