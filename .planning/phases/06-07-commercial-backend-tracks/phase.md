---
gsd_phase_number: M1R-06-07
name: commercial-backend-independent-tracks
milestone: ROML-M1R
status: non-blocking
parallelism: MOSEK and Xpress are isolated trains; neither edits shared contract after freeze
---

# M1R-06/07 — Commercial Backend Independent Tracks

## Shared rules
- Both crates remain `publish = false` and experimental until independently qualified.
- Neither blocks `roml` + `roml-highs` publication.
- Shared contract changes require coordinator approval and a new decision entry; adapters do not bend generic semantics to preserve legacy behavior.
- Compile, native load, ABI/version, license acquisition, option validation, solve, callback, and extraction failures are distinct.
- Licensed runners are protected, least-privilege, and do not expose proprietary SDKs, binaries, logs, or credentials.

# M1R-06 — MOSEK

## Admission
M1R-01 contract freeze accepted; official MOSEK Rust API/version and supported target matrix verified; owner approves licensed execution resources.

## Tasks
### M06.1 Official API migration
- Add the official `mosek` crate at a reviewed compatible version.
- Remove handwritten FFI declarations/constants and redundant build discovery.
- Use vendor-documented installation variables and lifecycle.
- Preserve only solver-neutral ID/index mapping that remains necessary.

### M06.2 Lifecycle and errors
- Model environment/task ownership and sharing explicitly.
- Distinguish library/configuration/license/version/numerical/internal failures.
- Remove assertions/panics from expected environment failures.
- Record native return code, operation, and health effect.

### M06.3 Callback redesign
- Remove all task/environment mutation from inside callbacks.
- Support documented observation/interruption behavior only.
- Lazy constraints/user cuts remain unsupported unless an officially legal implementation is proven.
- A collect–terminate–apply-outside–reoptimize workflow is a separate opt-in algorithm, not falsely equivalent to in-tree callback mutation.

### M06.4 Contract implementation
Implement snapshot, typed delta, rebuild, request negotiation, status/error mapping, and solution extraction. Run the common conformance/differential/fault suite with MOSEK-specific tolerances and capability declarations.

### M06.5 Protected qualification
Design runner installation, license access, secret masking, artifact policy, cleanup, and trigger restrictions. Separate compile/load/license/solve jobs where useful.

## MOSEK gate
M1R-M1/M2/MX3 pass; official API and callback review have no blocker; protected CI passes exact SHA; owner separately decides publication/support.

# M1R-07 — Xpress

## Admission
M1R-01 contract freeze accepted; legal/redistribution answer recorded; owner approves licensed execution resources.

## Tasks
### X07.1 Binding/legal decision
Obtain a documented answer on whether generated Rust declarations derived from installed Xpress headers may be distributed. Select one:
- generated `roml-xpress-sys` with one `links = "xprs"` owner;
- runtime-loaded boundary with legally maintainable type/layout source;
- vendor-supported binding;
- local-only unpublished adapter if distribution cannot be justified.

Do not infer permission from technical feasibility.

### X07.2 Lifecycle and discovery
Formalize process-wide initialization/free, problem ownership, library/version/license discovery, target filenames, runtime dependencies, and concurrency. No unconditional callback stdout or ignored return codes.

### X07.3 Typed contract migration
Replace adjacency-based legacy `Change` assumptions with typed `DeltaBatch` operations. Implement snapshot, incremental apply, rebuild, request negotiation, status/error mapping, and solution extraction.

### X07.4 Bulk/scalar proof
Characterize current batched row/column/matrix paths. For each admitted bulk path, prove scalar equivalence, incremental/rebuild equivalence, index-map coherence, and failure recovery before retaining it.

### X07.5 Protected qualification
Run common conformance/differential/fault tests on licensed infrastructure. Validate add/remove/reindex and process lifecycle under serial and allowed concurrent usage.

## Xpress gate
M1R-X1/X2/MX3 pass; legal decision is explicit; full native evidence exists; owner separately decides publication/support.

## Exit dispositions
Each commercial track ends in exactly one documented state:
- qualified and publish-authorized;
- qualified but unpublished;
- experimental with known gaps;
- external-blocked;
- retired from workspace.
