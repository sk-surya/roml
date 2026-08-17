# Phase 30 Source Audit

This audit is the coverage ledger for the executable plan set. The existing
`30-PLAN.md` remains the authoritative P30 scope contract; the numbered plans
below provide the executor-facing decomposition.

| Source | ID | Feature / requirement | Plan | Status | Notes |
|---|---|---|---|---|---|
| GOAL | — | Persistent semantic softening plus a distinct portable weighted-L1 solve-scoped repair workflow composed with P29 | 30-01–30-04 | COVERED | Canonical construct, compiler bridge, overlay repair, provenance, and closure evidence are all planned. |
| REQ | SM-10.1 | One-call builder softens an existing constraint | 30-01 | COVERED | Public builder and stable `SoftConstraint` handle. |
| REQ | SM-10.2 | Upper/lower/equality/ranged algebra is exact | 30-02 | COVERED | Independent formulation oracle and differential tests. |
| REQ | SM-10.3 | Nonnegative violation variables have stable handles and origins | 30-01, 30-02 | COVERED | Canonical roles plus generated compiler origins. |
| REQ | SM-10.4 | Supplied maximum violations are finite and validated | 30-02 | COVERED | Atomic builder validation and compiler/report checks. |
| REQ | SM-10.5 | Signed correction is a separate API | 30-02 | COVERED | Separate correction payload and tests; no inference from soft slacks. |
| REQ | SM-10.6 | `None`/`Objective` penalty targets and correct sense handling | 30-01–30-03 | COVERED | P31 priority targeting and objective-policy ownership are excluded. |
| REQ | SM-10.7 | Parameter-dependent penalty weights remain finite | 30-02, 30-03 | COVERED | Resolve and record weights before backend mutation. |
| REQ | SM-10.8 | Solution APIs expose original lower/upper/total violations | 30-02 | COVERED | Accessors retain original-constraint identity. |
| REQ | SM-10.9 | Feasibility relaxation is solve-scoped and separate | 30-03 | COVERED | Existing overlay/apply/rollback/rebuild contract is reused. |
| RESEARCH | — | No `30-RESEARCH.md` exists | 30-01–30-04 | COVERED | Planning uses the accepted context, discussion log, existing P30 contract, and codebase patterns; no new dependency is introduced. |
| CONTEXT | D-01 | Canonical/revisioned semantic soft constraints with stable handles | 30-01 | COVERED | Action and acceptance cite D-01. |
| CONTEXT | D-02 | Exact side algebra and distinct equality/ranged violations | 30-02 | COVERED | Action and independent oracle cite D-02. |
| CONTEXT | D-03 | Finite nonnegative violation caps with atomic rejection | 30-02 | COVERED | Action and tests cite D-03. |
| CONTEXT | D-04 | Signed correction is explicit and separate | 30-02 | COVERED | Action and tests cite D-04. |
| CONTEXT | D-05 | Repair is solve-scoped and uses exact overlay identities | 30-03 | COVERED | Action cites D-05 and SHARED-CONTRACTS §2. |
| CONTEXT | D-06 | Portable weighted-L1 is normative with frozen defaults | 30-03 | COVERED | Action and provider tests cite D-06. |
| CONTEXT | D-07 | Four mathematical outcomes and acceptance classification | 30-03 | COVERED | Action and outcome tests cite D-07. |
| CONTEXT | D-08 | Operational failures and cleanup/rebuild are typed and composite | 30-03, 30-04 | COVERED | Fault matrix and evidence cite D-08. |
| CONTEXT | D-09 | Supported P29 origins map all-or-error with imported provenance | 30-04 | COVERED | Action and source-aware fixture cite D-09. |
| CONTEXT | D-10 | Unsupported/stale P29 members reject explicitly before mutation | 30-04 | COVERED | Negative corpus cites D-10. |
| CONTEXT | D-11 | Finite weights and exact identities are recorded before mutation | 30-02, 30-03 | COVERED | Metadata and preflight tests cite D-11. |
| CONTEXT | D-12 | IIS is diagnostic scope, not a minimum-repair proof | 30-04 | COVERED | Public documentation and qualification assertions cite D-12. |
| CONTEXT | D-13 | Frozen P30 API has no priority/objective-policy/generic-provider escape hatch | 30-01 | COVERED | Compile-target negative API guards cite D-13. |
| CONTEXT | D-14 | Portable fallback is independent of native availability; native selection must be exact | 30-03 | COVERED | Provider policy returns portable fallback or pre-mutation rejection; native relaxation implementation is outside this plan set. |
| CONTEXT | D-15 | Shared M3 identity/overlay/error contracts are binding | 30-01–30-04 | COVERED | Every plan references SHARED-CONTRACTS.md. |

## Explicit scope fence

- P31 owns `PenaltyTarget::Priority`, `ObjectivePriority`, canonical
  `ObjectivePolicy`, lexicographic execution, and objective-stage locks. No P30
  plan creates or modifies those contracts.
- P30 retains the provider-policy surface required by D-14, but this plan set
  implements the portable provider, explicit fallback/rejection behavior, and
  standard compiled-HiGHS differential coverage only. It does not add a native
  feasibility-relaxation execution path or broaden native qualification.
- IIS minimum-cardinality/minimum-weight claims, nonlinear repair, extra solver
  adapters, and publication/release work remain outside the phase.
