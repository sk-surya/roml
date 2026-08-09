# Phase 35 MPS import qualification evidence

Date: 2026-08-08

Implementation branch: `phase-roml-P35-mps-import`
Latest implementation commit at evidence capture: `78ad0d6621fbb0eecc7b393d95abeb8ed53263a5`

The design packet was merged to `main` by `8a24bbeb23b7ae1e2e87a47c7df248698374d84c`.
The executable P35 plan was merged to `main` by
`3bd0319518c27127a30bc53878f776e82f1ad822`.

## Implemented and passing

| Area | Evidence |
| --- | --- |
| handwritten fixed/free lexer and section state | 21 lexer unit tests; malformed arbitrary-byte harness |
| transactional staging and semantic resolution | core MPS reader, staging, semantic, and metamorphic tests pass |
| selected-vector validation | duplicate RHS/RANGES, RANGE-on-N, and unselected-vector rules are covered |
| variable domains and provenance | marker FR, implicit continuous/integer defaults, explicit side replacement |
| public stream/path reader | `mps_reader` and module-seam tests pass |
| native differential oracle | bundled HiGHS 1.15.0 `readModel` oracle compares matrix cells, row/column bounds, integrality, objective coefficients, sense, and offset |
| native-vs-ROML solve oracle | termination class and objective value compare under explicit absolute/relative tolerances |
| corpus security seam | 23 archive/path/cache tests pass, including live-payload streaming and uninitialized-gitlink regression |
| reviewed 7z adapter | `sevenz-rust` 0.6.1 enumerates entry metadata and streams each payload into the existing no-follow materializer; no filesystem extractor is used |
| pinned corpus identity | Chinneck and Netlib gitlinks are at the reviewed SHAs and clean |
| Netlib structural differential | 94/94 pinned `.mps` files: ROML parse and native HiGHS full structure equivalent in the no-solve run |
| selected Chinneck qualification | 3 named models: full structure equivalent; native and ROML both `Infeasible`; P29 reports `Irreducible`; all reported original restrictions resolve to exact MPS origins |
| solve differential | 3 selected Chinneck cases: `Infeasible`/`Infeasible`; 3 Netlib cases: `Optimal`/`Optimal`, with equivalent objectives |
| P29 imported IIS | explicit-row and implicit-bound fixtures produce complete irreducible reports; exact reported `(Variable, BoundSide)` resolves to one source origin |

## Commands

```text
cargo fmt --all -- --check                                      PASS
cargo check -p roml --all-targets                              PASS
cargo clippy -p roml --all-targets -- -D warnings              PASS
cargo test -p roml --all-targets                                PASS
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps          PASS
cargo package --list -p roml                                   PASS

cargo check -p roml-highs --all-targets                         PASS
cargo clippy -p roml-highs --all-targets -- -D warnings         PASS
cargo test -p roml-highs --all-targets                          PASS
RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps    PASS
cargo package --list -p roml-highs                              PASS

cargo test --test corpus_archive_security \
  --locked                                                        PASS: 23 tests
cargo run -p roml-highs --example mps_corpus_qualification \
  -- . --max-files 3                                           PASS: 3 selected Chinneck + 3 Netlib solve cases
cargo run -p roml-highs --example mps_corpus_qualification \
  -- . --no-solve                                               PASS: 3 selected Chinneck + 94/94 Netlib structural equivalent
```

## Chinneck materialization and selected model qualification

The pinned Chinneck checkout contains `7z` archives and no expanded MPS files.
The qualification run securely materializes the complete contents of both
reviewed archives before model discovery:

- `INFfromNetlibLPs.7z`
- `INFfromClassificationData.7z`

Model qualification is then restricted to this explicit reviewed allowlist:

- `INFfromClassificationData.7z/IC-balancescale-LB.mps`
- `INFfromClassificationData.7z/IC-balancescale.mps`
- `INFfromClassificationData.7z/IC-breast1-LB.mps`

For each reviewed archive, the adapter hashes the archive and preflights the
complete entry inventory, then reopens the archive and streams every validated
entry payload into a fresh descriptor-relative staging tree. Completion
inventory validation and atomic promotion are required before model discovery;
the three-model allowlist applies only to qualification, not materialization.

The broader archive is intentionally not represented as a blanket pass. During
exploration, an unselected `IC-satimage-LB.mps` reached a HiGHS/P29
`Unknown(Unclassified)` final verification path. It is excluded from this
selected gate and remains a residual qualification target; it is not counted as
supported or equivalent.

No commercial solver checks were attempted. No publication, tag, or release
operation was performed. The MSRV-specific `cargo +1.85.0` toolchain was not
installed in this environment, so the local final matrix used the repository's
available toolchain; CI's pinned MSRV job remains authoritative for that check.

The final hosted matrix for `93985cdda463e833d6d837c8edb1dfc0bcc8ecfa` was
green: Core/MSRV, HiGHS (bundled/system/package/MSRV), Coverage, Quality, and
Policy. PR #44 remains open, mergeable, and draft; merge is intentionally still
an owner gate.
