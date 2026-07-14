# ROML-M1 State

**Milestone:** Native Backend Qualification and Public Release
**Status:** Planned; execution not started
**Base:** `main@ef37c88a6d80775ea69d2ccb986655edeb5789ec`
**Planning branch:** `planning/roml-M1-native-backends-release`
**Current phase:** M1.0 Admission and external gates

## Predecessor evidence

PR #3 merged the solver-free hardening program. Reported verification: format, clippy, 263 passing tests, rustdoc, package, Linux/macOS/Windows/MSRV, audit, deny and machete. Eleven ignored tests are explicitly classified as known P1/P2 defects. M1 does not reinterpret ignored tests as passing requirements.

## Owner gates

1. Confirm `MIT OR Apache-2.0` and authorize license-file commit.
2. Confirm/control crates.io names `roml` and `roml-highs`.
3. Authorize publication only after M1.8 evidence.
4. Approve protected runners/licenses before MOSEK/Xpress qualification.

## Phase ledger

| Phase | Status | Blocking gate |
|---|---|---|
| M1.0 Admission | Complete | licenses committed, support labels corrected, ignored tests reconciled, SDK inventory done; name ownership pending crates.io access |
| M1.1 Contract freeze | Complete | protocol types frozen (10 sections), backend conformance harness defined, contract document at docs/release/evidence/M1/M1.1_CONTRACT_FREEZE.md |
| M1.2 HiGHS bindings | Complete | highs-sys 1.15.0, no handwritten ABI remains, 252 lines deleted, ROML constant aliases preserved |
| M1.3 HiGHS correctness | Complete | commuting square proven (16 single-op + random sequences), fault injection (3 scenarios), multi-cursor independence, rebuild determinism, semi-continuous recovery (3 tests), status lattice (7), error classification (10), solve-request negotiation (9); 333 tests pass |
| M1.4 Platform qualification | Not started | M1.2/M1.3 |
| M1.5 Performance/UX | Not started | stable semantic path |
| M1.6 MOSEK | Deferred parallel | licensed environment |
| M1.7 Xpress | Deferred parallel | legal/binding decision |
| M1.8 Release | Blocked | all mandatory HiGHS gates + owner auth |
| M1.9 Operations | Not started | publication |

## Immediate execution

- Preserve this branch as milestone authority.
- Create `phase-roml-M1.0-admission` from this head.
- Capture current package/CI evidence and close license/name gates.
- Create an explicit backend contract inventory before any adapter rewrite.
- Run independent review at every phase boundary.

## Non-negotiables

- Core/HiGHS release is not blocked by commercial backends.
- No handwritten ABI survives where maintained generated/official bindings exist.
- No native partial failure may destroy replayability.
- No unsupported feature silently degrades.
- No publication from a workspace path; test packed consumer artifacts.
