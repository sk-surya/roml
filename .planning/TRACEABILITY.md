# ROML-M1R Traceability Matrix

Every implementation PR lists the applicable requirement IDs and evidence paths. “Planned” or an evidence-document claim does not close a row.

| Requirement group | Primary phase | Mandatory evidence |
|---|---|---|
| M1R-G1–G5 truth/governance | M1R-00, M1R-08 | state vocabulary, candidate manifest, requirement disposition, exact-SHA authorization |
| M1R-C1 destructive-drain removal | M1R-01 | failure-before-ack test, two-session test, public path source audit |
| M1R-C2 solve policy outside Model | M1R-01 | API/source test, migration documentation |
| M1R-C3 explicit negotiation | M1R-01, M1R-03 | applied/adjusted/rejected tests for each option |
| M1R-C4 health model | M1R-01, M1R-03 | fault matrix and cursor/health assertions |
| M1R-C5 commuting projection | M1R-01, M1R-03 | focused operation matrix + generated traces |
| M1R-C6 status/error/solution fidelity | M1R-01, M1R-03 | status fixtures, native-code classification, solution-view tests |
| M1R-C7 legacy disposition | M1R-01 | removal or safe-deprecation review and semver report |
| M1R-C8 ignored-test closure | M1R-00, M1R-01 | per-test disposition; mandatory suite reports zero ignored |
| M1R-H1 authoritative bindings | M1R-02 | dependency/binding audit; no copied ABI source |
| M1R-H2 fallible construction | M1R-02 | missing/config/version/index-width error tests |
| M1R-H3 native checks | M1R-02 | return-code inventory and unsafe review |
| M1R-H4 thread safety | M1R-02 | Send/Sync decision and compile/runtime tests where valid |
| M1R-H5 complete session implementation | M1R-02 | snapshot/delta/rebuild/request/solve/extraction focused tests |
| M1R-H6 status ambiguity | M1R-02, M1R-03 | infeasible-or-unbounded and incumbent/proof tests |
| M1R-H7 domain partial failure | M1R-02, M1R-03 | mandatory semi-continuous recovery regressions |
| M1R-H8 metadata | M1R-02 | version/build/index/effective-config assertions |
| M1R-Q1 common harness | M1R-03 | identical scenario source instantiated for ReferenceBackend and HiGHS |
| M1R-Q2 generated traces | M1R-03 | seed corpus, shrink artifacts, CI results |
| M1R-Q3 fault injection | M1R-03 | operation/sub-call failure matrix |
| M1R-Q4 independent cursors | M1R-03 | lag/catch-up and failure-isolation tests |
| M1R-Q5 solve observables | M1R-03 | objective/primal/dual/reduced-cost/basis/request tests |
| M1R-P1 matrix | M1R-04 | exact-SHA hosted workflow runs |
| M1R-P2 native build modes | M1R-04 | bundled/system clean-runner tests and diagnostics |
| M1R-P3 packed consumers | M1R-04, M1R-08 | fresh archive consumer logs on required OSes |
| M1R-P4 docs topology | M1R-04 | rustdoc/docs.rs rehearsal without commercial SDKs |
| M1R-P5 policy/provenance | M1R-04, M1R-08 | semver/audit/deny/machete/package/license/SBOM results |
| M1R-P6 scheduled safety | M1R-04 | workflow cadence and first successful run |
| M1R-E1–E4 performance evidence | M1R-05 | decomposed benchmark artifacts and equivalence reports |
| M1R-E5 user journeys | M1R-05 | packed-crate compile/run transcripts |
| M1R-M1/M2 | M1R-06 | official MOSEK API/callback review/protected CI |
| M1R-X1/X2 | M1R-07 | legal decision, binding/lifecycle/bulk evidence |
| M1R-MX3 non-blocking unpublished status | M1R-00, M1R-06/07, M1R-08 | manifests, support matrix, release crate list |
| M1R-R1 scope freeze | M1R-08 | release manifest and changelog/migration/support review |
| M1R-R2 independent reviews | M1R-08 | four signed dispositions |
| M1R-R3 publication order | M1R-08 | released-core consumer before backend publish |
| M1R-R4 artifact identity | M1R-08 | hashes/tag/SBOM/packages/evidence same SHA |
| M1R-R5 operations | M1R-09 | compatibility, patch, security, deprecation procedures |

## Evidence directory convention
```text
docs/release/evidence/M1R/
  M1R-00-ADMISSION.md
  M1R-01-CONTRACT-CLOSURE.md
  M1R-02-HIGHS-REWRITE.md
  M1R-03-NATIVE-QUALIFICATION.md
  M1R-04-PLATFORM-PACKAGE.md
  M1R-05-PERFORMANCE-UX.md
  M1R-06-MOSEK.md
  M1R-07-XPRESS.md
  M1R-08-RELEASE-CANDIDATE.md
  M1R-09-OPERATIONS.md
  artifacts/<phase>/<sha>/...
```

## Phase evidence rule
Each report contains:
- base/head SHA and PR;
- requirement rows closed and still open;
- exact commands, exit codes, test pass/fail/ignored/skipped counts;
- tool/native versions and environment;
- hosted CI links and artifact hashes;
- independent review findings/dispositions;
- deviations/decision references;
- gate state: PASS, FAIL, OWNER-BLOCKED, EXTERNAL-BLOCKED;
- next admitted phase/lanes.
