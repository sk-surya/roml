# ROML Program Decisions v2

Decisions are frozen unless superseded by a new dated entry with evidence.

## D-001 — M1R truth reset precedes further implementation
The inherited M1 branch is treated as a candidate, not an accepted milestone. Completion labels are revalidated against code, executable tests, CI, and external gates.

## D-002 — Supported backend execution is revisioned
The supported public path uses canonical snapshots, typed delta batches, independent adapter cursors, explicit acknowledgement, adapter health, immutable solve requests, and effective configuration. Destructive `Change` draining is not a stable contract.

## D-003 — Compatibility cannot preserve unsafe semantics
A legacy adapter shim may exist temporarily only if it delegates to the safe protocol, cannot lose replayability, rejects unsupported policy explicitly, and is loudly deprecated. Otherwise it is removed before v0.1.

## D-004 — HiGHS is the sole mandatory v0.1 backend
`roml` + `roml-highs` define the release critical path. MOSEK and Xpress remain independent, unpublished tracks until their own gates pass.

## D-005 — Authoritative native bindings
HiGHS uses maintained generated bindings. MOSEK uses the official Rust API. Xpress receives a generated/runtime boundary only after legal and redistribution review. Adapter source does not hand-copy ABI declarations where authoritative bindings exist.

## D-006 — No false callback uniformity
Capabilities distinguish progress, interruption, incumbent observation/injection, lazy constraints, and user cuts. Each backend exposes only documented legal behavior.

## D-007 — Fallible backend construction
Native library discovery, ABI mismatch, initialization, and license failures return typed errors. Constructors do not panic for expected environmental failures.

## D-008 — Evidence is layered
Core tests, reference-backend tests, local native tests, hosted CI, packed-consumer tests, and release validation are separate evidence levels. A lower level cannot substitute for a higher required level.

## D-009 — Planning and implementation lane separation
The v2 branch is canonical planning authority. Implementation occurs in isolated worktrees and draft PRs. Shared contract files have one integration owner.

## D-010 — Performance follows correctness
Bulk/native optimizations require scalar equivalence, failure-recovery equivalence, and measured benefit. No optimization may weaken canonical semantics or replayability.

## D-011 — Publication is an owner capability
Agents may prepare evidence and commands but may not publish, tag, reserve names through functional releases, or alter release authorization without explicit owner instruction.

## D-012 — Strategic milestones are admission-gated
M2–M5 are roadmap envelopes, not permission to expand M1R. Their admission criteria must pass before detailed implementation planning begins.
