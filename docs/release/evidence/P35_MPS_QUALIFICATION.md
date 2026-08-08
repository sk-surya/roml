# Phase 35 MPS import qualification evidence

Date: 2026-08-08

Implementation branch: `phase-roml-P35-mps-import`  
Latest implementation commit at evidence capture: `38552ee`

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
| native differential oracle | bundled HiGHS 1.15.0 `readModel` oracle and synthetic differential test pass |
| corpus security seam | 21 adversarial archive/path/cache tests pass |
| pinned corpus identity | Chinneck and Netlib gitlinks are at the reviewed SHAs and clean |
| Netlib parser differential | 94/94 pinned `.mps` files: ROML parse and native HiGHS structure equivalent in no-solve run |
| Netlib solve smoke | 3/3 bounded smoke files solved to `Optimal` through ROML → HiGHS |
| P29 imported IIS | explicit-row and implicit-bound fixtures produce complete irreducible reports; source-map/provenance checks pass |

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
  --test mps_netlib_qualification                               PASS
cargo run -p roml-highs --example mps_corpus_qualification \
  -- . --max-files 3                                        PASS
cargo run -p roml-highs --example mps_corpus_qualification \
  -- . --no-solve                                            PASS: 94/94 equivalent
```

## Explicit residual gate

The pinned Chinneck checkout contains `7z` archives and no expanded MPS files.
The repository now has the pre-write-safe descriptor-relative materializer
contract and the qualification command refuses to perform blind extraction.
However, this environment has no verified 7z archive reader, so no Chinneck
archive has been materialized and no Chinneck corpus IIS run is claimed as
passing. The phase must remain in progress until a reviewed archive adapter
produces an atomically completed `target/roml-corpora/chinneck` tree and the
selected Chinneck parse/native/IIS qualification is recorded.

No commercial solver checks were attempted. No publication, tag, or release
operation was performed.
