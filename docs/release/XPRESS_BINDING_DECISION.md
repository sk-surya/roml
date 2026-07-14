# Xpress Binding Decision

**Status:** BLOCKED — pending legal/technical verification  
**Date:** 2026-07-13  
**Decision owner:** sk-surya

## Context

The ROML workspace currently contains handwritten Xpress ABI declarations in
`roml-xpress/src/ffi.rs` (struct layouts, function signatures, constants).
The principal engineering audit flagged these as version-fragile and
potentially violating header-derivative redistribution terms.

Xpress is a commercial solver from FICO. Its C API headers are distributed
under the FICO Xpress license, which typically restricts redistribution of
header-derived code without authorization.

## Options

### Option A: Generated `roml-xpress-sys` with `links = "xprs"`

Generate Rust FFI declarations from official Xpress C headers using `bindgen`
at build time. The sys crate owns the ABI contract and `links` key.

**Pros:**
- ABI declarations derived from installed headers, matching the user's SDK version
- No committed derivative code from proprietary headers
- `links = "xprs"` ownership is clear
- Standard Cargo native-library pattern

**Cons:**
- Requires `bindgen` (LLVM dependency) at build time
- Xpress headers must be installed and discoverable
- Licensing: header-derived bindings may still constitute derivative work
- Build complexity increases for users without Xpress

### Option B: Runtime-loaded adapter with `libloading`

Load `libxprs` dynamically at runtime using `libloading`. No link-time
dependency, no build-time header processing.

**Pros:**
- No build dependency on Xpress SDK
- Clean compilation on hosts without Xpress installed
- Clear diagnostics when library is absent at runtime

**Cons:**
- Function pointers must be loaded and checked at runtime
- Struct layouts must still be defined (copied or generated)
- Callback ABI from C into Rust more complex with dynamic loading
- Licensing: struct layouts still derived from headers

### Option C: Official FICO binding package

Use or contribute to an officially-maintained Rust binding for Xpress (if one
exists or FICO is willing to support one).

**Pros:**
- Licensing resolved by the binding author
- Maintenance burden shared

**Cons:**
- No known official Rust bindings exist as of 2026-07
- ROML has no relationship with FICO

## Recommendation

**Option A (generated bindgen sys crate)** with the following blockers:

1. **Legal:** Verify with FICO that generating and distributing Rust FFI
   declarations from Xpress C headers is permitted under the license.
   Document the response in this file.

2. **Technical:** Determine minimum supported Xpress SDK version and verify
   `bindgen` produces correct declarations for all required API functions,
   structs, callbacks, and constants.

3. **CI:** Design a self-hosted CI runner strategy that can build/test
   against licensed Xpress without exposing credentials or binaries.

## Interim state

Until the legal and technical blockers are resolved:
- `roml-xpress` remains `publish = false`
- Handwritten FFI declarations remain but are marked as `#[allow(dead_code)]`
  and annotated with version/provenance comments
- No Xpress adapter features are documented as "supported"
- ROML release train proceeds with `roml` + `roml-highs` only

## Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-13 | Block on legal verification | Cannot publish derived header code without license clarity |
