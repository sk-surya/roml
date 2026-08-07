# Phase 29 HiGHS IIS qualification evidence

## Bundled pinned cell

| Gate | Result |
| --- | --- |
| `highs-sys` version | `1.15.0` |
| HiGHS source | `v1.15.0`, commit `83960019015b0d5152df73110ff142f328edcfd2` |
| generated crate commit | `highs-sys v1.15.0`, `bb9af7f05b11826c5b487ba8625743a16b7ce3b4` |
| generated `Highs_getIis` declaration | present; compile-gated by the pinned dependency |
| bundled compile/link/load | PASS |
| LP IIS extraction | PASS |
| semantic reduction and fresh verification | PASS |
| persistent solve-session preservation | PASS |

Commands run on the audit host:

```text
cargo check -p roml-highs --all-targets                         PASS
cargo test -p roml-highs --test iis -- --nocapture              PASS (3 tests)
```

The test corpus covers contradictory rows, Auto native seed plus ROML
reduction, persistent fixing provenance, exact semantic members, and a solve
after analysis. Native extraction performs checked count and data calls and
retains row/column membership plus bound-side evidence.

## System-discovery cells

`roml-highs` currently permits `highs >= 1.5.0` through `pkg-config`; project
CI documentation identifies 1.9.0 as its system floor, but no complete
version-by-version header/library matrix is present. The audit host has no
`highs.pc` (`pkg-config --modversion highs` fails). All system native IIS
requests therefore remain typed `Unsupported`; the module is not enabled for
system builds. This is an explicit skipped qualification cell, not a pass.

No handwritten IIS declaration was added to `roml-highs/src/ffi.rs`. The
authoritative generated binding and matching header/source audit is recorded
in [`highs_iis_api.md`](../../knowledge/highs_iis_api.md).

## Residual risks

- System HiGHS versions need independent compile/link/load/run qualification
  before native support can be enabled for that feature.
- Native evidence is only a seed; ROML semantic reduction and fresh
  verification are mandatory for a semantic irreducibility claim.
- This qualification does not cover MIP-only infeasibility, nonlinear models,
  feasibility relaxation, all-IIS enumeration, or minimum-cardinality IIS.
