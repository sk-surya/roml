# Support Matrix

This document describes the platform and layer support for ROML and its solver adapters.

**Important distinction:** The v0.1 hardening program verified the **solver-free core**. No native backend has been qualified for publication. HiGHS qualification is the primary goal of M1 (M1.2-M1.4). MOSEK and Xpress are deferred to independent qualification.

## Layer Descriptions

| Layer | Crate | Status |
|-------|-------|--------|
| **Core** | `roml` | Verified — solver-agnostic MILP model layer, expression DSL, revision protocol, change tracking |
| **HiGHS** | `roml-highs` | Experimental — awaiting M1.2 binding migration, M1.3 semantic qualification, M1.4 cross-platform CI |
| **MOSEK** | `roml-mosek` | Experimental (`publish = false`) — requires official binding migration and licensed environment |
| **Xpress** | `roml-xpress` | Experimental (`publish = false`) — requires legal/binding decision and licensed environment |

## Platform Support

| Layer | Linux | macOS | Windows | MSRV |
|-------|-------|-------|---------|------|
| **Core** | ✅ verified | ✅ verified | ✅ verified | ✅ 1.85 |
| **HiGHS** | ⬜ experimental | ⬜ experimental | ⬜ experimental | TBD |
| **MOSEK** | ⬜ compile-only | ⬜ compile-only | ⬜ compile-only | TBD |
| **Xpress** | ⬜ compile-only | ⬜ compile-only | ⬜ compile-only | TBD |

## Support Labels

| Label | Meaning |
|-------|---------|
| **verified** | Tested in CI on all three platforms + MSRV. Builds, tests, documents, and packages without native solver dependencies. |
| **qualified** | Tested in CI with native solver. Commuting-square property holds. Incremental, rebuild, and extracted observables agree. |
| **compile-only** | The crate compiles on this platform. Solving is not guaranteed without a native solver installation and license. Not tested in CI for solve paths. |
| **experimental** | May compile but is not yet qualified. Not recommended for production use. |

## Rust Toolchain

| Setting | Value |
|---------|-------|
| **Minimum Supported Rust Version (MSRV)** | 1.85 (stable) |
| **Target toolchain** | Stable Rust |
| **Edition** | 2021 |
| **Resolver** | v2 |

## Notes

- **Core** requires no external libraries beyond the Rust standard library. Verified on Linux, macOS, Windows, and MSRV 1.85.
- **HiGHS** requires the HiGHS native library. Current adapter uses handwritten FFI (`roml-highs/src/ffi.rs`). M1.2 will migrate to `rust-or/highs-sys` with bundled static build as the default.
- **MOSEK** and **Xpress** adapters are gated with `publish = false`. See [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) for the publication policy.
- All layers use stable Rust features only. No nightly features are used.
- The `roml` core denies `unsafe_code` at the lint level. Backend crates isolate unsafe code behind binding boundaries.
