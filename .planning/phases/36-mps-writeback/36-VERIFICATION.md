# Phase 36 Verification — MPS write-back

## Closure disposition

P36 implementation is complete and merged through PR #46. This artifact
records the code-bearing qualification candidate, the exact PR head reviewed,
and the merge disposition. It does not authorize any release.

- Code-bearing qualification commit: `0a4e732c8d884b4ccc6d8248290a7e0bf6555f09`
- Exact reviewed PR head before merge: `8a8ee7573532c6c9b883249f74afefb477bbb6a1`
- Planning packet merge prerequisite: PR #45 merged as `48fab4db347522cebc786393e5afcbdbcea98f33`
- Pull request: [#46](https://github.com/sk-surya/roml/pull/46), merged as `8838effee84eafdcbc2e502fb417df8d09221248`

No production implementation changed after the code-bearing qualification
commit; the final reviewed PR head was the governance-closed candidate.

## Requirement evidence

| Requirement family | Evidence | Result |
|---|---|---|
| MPS-W01–W08 | writer contract, typed representability/errors, deterministic formatting, objective/row/domain semantics, numeric safety, transactional paths | PASS |
| MPS-W09 | independent ROML mathematical round-trip oracle | PASS |
| MPS-W10–W11 | independent native HiGHS structural and solve differential | PASS |
| MPS-W12 | exact pinned Netlib manifest, 94 regular files, 94/94 writer/determinism/ROML-structure/HiGHS-structure rows | PASS |
| MPS-W13 | source-layout and parameter-graph identity non-goals documented | PASS |
| MPS-W14 | solver-free core, package/policy checks, exact-head hosted CI | PASS on the reviewed merged head |

## Frozen corpus qualification

The corpus is `sk-surya/lp-data-netlib@56257eea85b433ce6aa67d26156b36385318fd6f`
with the exact 94-file manifest in `36-NETLIB-MANIFEST.md`.

Recorded result:

- 94/94 writer output PASS;
- 94/94 deterministic repeated-write PASS;
- 94/94 independent ROML mathematical round-trip PASS;
- 94/94 native HiGHS structural comparison PASS;
- bounded solve subset `blend.mps`, `fit2d.mps`, and `gfrd-pnc.mps` PASS;
- process exit 0, zero FAIL rows, bundled HiGHS 1.15.0.

## Verification commands

The focused and local qualification results recorded on the code-bearing
candidate were:

```text
cargo test -p roml --test mps_write_roundtrip                         PASS (6)
cargo test -p roml --test mps_write_report --test mps_write_integration \
  --test mps_write_projection                                            PASS (32)
cargo test -p roml-highs --test mps_write_highs_oracle \
  --test mps_write_corpus_contract                                      PASS (5)
cargo test -p roml --all-targets --locked                              PASS
cargo test -p roml-highs --all-targets --locked                        PASS
cargo fmt --all -- --check                                              PASS
git diff --check                                                        PASS
```

The frozen runner was invoked as:

```text
cargo run -p roml-highs --example mps_write_corpus_qualification -- /tmp/roml-p36-source
```

## Hosted exact-head checks

The independent review and final hosted checks recorded exact-head Core, HiGHS,
Coverage, Quality, Policy, package, MSRV, documentation, and security/dependency
checks green on the reviewed PR head, including bundled HiGHS on
`windows-latest`.

## Residual gate

There is no remaining P36 implementation or governance gate. P30 is activated by
the authoritative GSD state update after PR #46 merged.
