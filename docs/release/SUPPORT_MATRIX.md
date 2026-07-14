# Support Matrix

This document describes the platform and layer support for ROML and its solver adapters.

## Layer Descriptions

| Layer | Crate | Description |
|-------|-------|-------------|
| **Core** | `roml` | Solver-agnostic MILP model layer, expression DSL, change tracking, solution store |
| **HiGHS** | `roml-highs` | HiGHS solver adapter — builds, links, and solves with the HiGHS library |
| **MOSEK** | `roml-mosek` | MOSEK solver adapter — builds and compiles; solving requires a MOSEK license |
| **Xpress** | `roml-xpress` | FICO Xpress solver adapter — builds and compiles; solving requires an Xpress license |

## Platform Support

| Layer       | Linux | macOS | Windows |
|-------------|-------|-------|---------|
| **Core**    | supported | supported | supported |
| **HiGHS**   | supported | supported | supported |
| **MOSEK**   | compile-only | compile-only | compile-only |
| **Xpress**  | compile-only | compile-only | compile-only |

## Support Labels

| Label | Meaning |
|-------|---------|
| **supported** | Tested in CI. Builds, links, and runs on the target platform. |
| **tested** | Tested in CI on this platform but not officially supported (may have limitations). |
| **compile-only** | The crate compiles on this platform without a solver library. Solving is not guaranteed without a license or runtime; not tested in CI for solve paths. |
| **experimental** | May compile but is not tested. Not recommended for production use. |

## Rust Toolchain

| Setting | Value |
|---------|-------|
| **Minimum Supported Rust Version (MSRV)** | TBD (to be determined — currently targets Rust 1.75+) |
| **Target toolchain** | Stable Rust |
| **Edition** | 2021 |
| **Resolver** | v2 |

The MSRV is not yet pinned. It will be determined once the 1.0 release is prepared. During pre-1.0 development, the project tracks the latest stable Rust toolchain.

## Notes

- **Core** requires no external libraries beyond the Rust standard library.
- **HiGHS** requires the HiGHS shared library to be pre-installed on the build system. The `roml-highs/build.rs` script locates it automatically.
- **MOSEK** and **Xpress** adapters are gated for publication. See [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) for the publication policy.
- All layers use stable Rust features only. No nightly features are used.
