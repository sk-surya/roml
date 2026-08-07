# HiGHS IIS API audit for Phase 29

This is the binding gate for the Phase 29 native provider. No IIS ABI detail
is copied from the existing handwritten `roml-highs/src/ffi.rs` module. The
portable feasibility oracle is intentionally separate from the native
provider, so system HiGHS can run ROML semantic analysis without claiming
native IIS support.

## Audited artifacts

| Artifact | Exact identity | Result |
| --- | --- | --- |
| `highs-sys` crate | `v1.15.0`, commit `bb9af7f05b11826c5b487ba8625743a16b7ce3b4` | Generated with bindgen 0.72.1 |
| vendored HiGHS | `v1.15.0`, commit `83960019015b0d5152df73110ff142f328edcfd2` | Matching submodule |
| generated bindings | `/tmp/highs-sys-1.15.0/target/debug/build/highs-sys-38d40704aecccef2/out/c_bindings.rs` | Build passed on the audit host |
| C declaration | `HiGHS/highs/interfaces/highs_c_api.h` at the HiGHS commit above | `Highs_getIis` present |
| C implementation | `HiGHS/highs/interfaces/highs_c_api.cpp` at the HiGHS commit above | Return-code and array-copy behavior inspected |

The exact source checkouts and generated output are audit inputs, not package
or repository assets. The reproducible commands were:

```text
git clone --depth 1 --branch v1.15.0 https://github.com/ERGO-Code/HiGHS.git /tmp/HiGHS-1.15.0
git clone --depth 1 --branch v1.15.0 https://github.com/rust-or/highs-sys.git /tmp/highs-sys-1.15.0
git -C /tmp/highs-sys-1.15.0 submodule update --init --depth 1
cargo build --manifest-path /tmp/highs-sys-1.15.0/Cargo.toml
```

## Generated surface used by the provider

The generated file contains these exact declarations and types:

```text
pub type HighsInt = ::std::os::raw::c_int;
pub fn Highs_getIis(
    highs: *mut ::std::os::raw::c_void,
    iis_num_col: *mut HighsInt,
    iis_num_row: *mut HighsInt,
    col_index: *mut HighsInt,
    row_index: *mut HighsInt,
    col_bound: *mut HighsInt,
    row_bound: *mut HighsInt,
    col_status: *mut HighsInt,
    row_status: *mut HighsInt,
) -> HighsInt;
pub fn Highs_run(highs: *mut ::std::os::raw::c_void) -> HighsInt;
pub fn Highs_getModelStatus(highs: *const ::std::os::raw::c_void) -> HighsInt;
pub fn Highs_changeColBounds(
    highs: *mut ::std::os::raw::c_void, col: HighsInt,
    lower: f64, upper: f64,
) -> HighsInt;
pub fn Highs_changeRowBounds(
    highs: *mut ::std::os::raw::c_void, row: HighsInt,
    lower: f64, upper: f64,
) -> HighsInt;
pub fn Highs_version() -> *const ::std::os::raw::c_char;
pub fn Highs_versionMajor() -> HighsInt;
pub fn Highs_versionMinor() -> HighsInt;
pub fn Highs_versionPatch() -> HighsInt;
```

The generated constants used for interpretation are:

```text
kHighsStatusError = -1, kHighsStatusOk = 0, kHighsStatusWarning = 1
kHighsModelStatusOptimal = 7
kHighsModelStatusInfeasible = 8
kHighsModelStatusUnboundedOrInfeasible = 9
kHighsModelStatusUnbounded = 10
kHighsModelStatusTimeLimit = 13
kHighsModelStatusIterationLimit = 14
kHighsModelStatusUnknown = 15
kHighsIisStrategyLight = 0
kHighsIisStrategyFromLpRowPriority = 6
kHighsIisStrategyFromLpColPriority = 14
kHighsIisBoundFree = 1, kHighsIisBoundLower = 2
kHighsIisBoundUpper = 3, kHighsIisBoundBoxed = 4
kHighsIisStatusNotInConflict = -1
kHighsIisStatusMaybeInConflict = 0
kHighsIisStatusInConflict = 1
```

The native provider does not reproduce strategy masks or status numbers. It
uses the generated constants directly.

## Ownership and call protocol

`Highs_getIis` calls `Highs::getIis`, which writes the two count outputs and
then conditionally copies the variable/row indices, bound classifications,
and full per-variable/per-row status arrays. The provider therefore performs
two checked calls: first with null data arrays to obtain counts, then with
vectors sized from those counts. Count conversion and every return code are
checked before any indexing. The arrays are owned by the Rust provider after
the call; HiGHS retains no pointer to them.

The C++ implementation creates the IIS result from the incumbent HiGHS
instance. It is not a pure query: native IIS calculation can update solver
internal IIS state and may run additional LPs. Phase 29 consequently invokes
it only on a fresh analysis session and never on the persistent solve session.

The C header documents IIS for LP, QP, and the relaxation of a MIP. Phase 29
qualifies only continuous LP snapshots and explicitly labeled LP relaxations;
it does not turn native MIP or integrality-only behavior into an original-MIP
IIS claim.

## Version and feature gate

The bundled feature is compiled against the pinned `highs-sys 1.15.0`
submodule, where the generated `Highs_getIis` declaration is present. The
native provider in `roml-highs/src/native_iis.rs` is therefore compiled only
for the bundled, pinned provider and also checks the runtime version is
exactly `1.15.0` before use. The portable oracle in `roml-highs/src/iis.rs`
is compiled for both bundled and system features.

The `system` feature uses `pkg-config` discovery and currently accepts a broad
`highs >= 1.5.0` build-time range; the repository's CI documentation calls
out a 1.9.0 system floor but does not provide a version-by-version header and
library qualification matrix. System builds can run the portable oracle;
native IIS remains typed `Unsupported` until each supported header/library
pair is separately compile-, link-, load-, and IIS-qualified. A runtime symbol
probe is not used as a substitute for this compile gate.

## Stop condition

If a future `highs-sys` or system header does not generate `Highs_getIis` with
this exact shape, the native module must not add a handwritten declaration.
That build is unqualified and the portable ROML provider remains available.
