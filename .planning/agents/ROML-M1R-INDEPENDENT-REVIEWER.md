# ROML-M1R Independent Reviewer Packet

## Mandate
Review evidence and implementation independently. Do not optimize for agreement with the implementation agent. Your job is to find false closure, semantic gaps, unsafe native assumptions, test theater, unsupported claims, and release-evidence mismatches.

## Reviewer independence
- Do not author the implementation under review.
- Start from the phase requirements and governing laws, not the worker summary.
- Inspect source and tests directly.
- Select your own commands, seeds, model traces, and failure sites.
- Treat documentation claims as assertions requiring evidence.

## Universal checks
1. Exact base/head SHA and diff scope are known.
2. Requirement IDs map to executable evidence.
3. Required tests execute; ignored/skipped/unavailable counts are explicit.
4. Focused tests would fail against the pre-fix state or otherwise genuinely discriminate behavior.
5. No hidden compatibility path preserves the old defect.
6. Public docs/examples use the supported API.
7. Error paths and cleanup receive equal scrutiny to success paths.
8. Evidence comes from the same SHA being accepted.

## M1R-00 review
- Independently sample candidate commits/files and requirement dispositions.
- Re-run at least one command from every evidence class.
- Unignore a representative subset and verify the reconciliation method.
- Check external/owner blockers are not mislabeled as technical completion.

## M1R-01 review
- Construct a backend that fails before and after mutation; verify replay and health.
- Attach two sessions and verify independent catch-up.
- Search for `drain_changes`, model-owned options, silent-ignore language, duplicate statuses/errors, and legacy exports.
- Confirm compatibility shim, if any, delegates to safe semantics.
- Review trait boundaries for false uniformity and object-safety/lifetime ergonomics.

## M1R-02 review
- Inventory every unsafe block and native call.
- Search for copied ABI declarations/constants, ignored return values, `assert!`, `unwrap`, unjustified Send/Sync, callback unwind risks, stale pointers/lengths, and partial-construction leaks.
- Validate index-width and version assumptions.
- Compare capability declarations to implemented behavior.

## M1R-03 review
- Add independent deterministic seeds and at least one adversarial trace per operation family.
- Inject failures at sites not chosen by the author.
- Review normalized equality/tolerances for false positives.
- Check incremental and rebuild implementations do not share the same bug-producing helper in a way that makes comparison vacuous.
- Validate statuses and solution attributes against native documentation/known fixtures.

## M1R-04 review
- Confirm workflows ran on the exact SHA and required matrix cells.
- Inspect packed archives and fresh-consumer dependency resolution.
- Verify no workspace path, developer rpath, secret, proprietary artifact, or host-target confusion.
- Check docs.rs and package metadata independently.

## M1R-05 review
- Confirm benchmark decomposition and metadata.
- Check warmup, statistical interpretation, solver-time contamination, and cherry-picked workloads.
- Require equivalence/fault evidence for bulk optimizations.
- Run at least one user journey without repository-local knowledge.

## M1R-08 review
- Compare source SHA, package hashes, SBOM, CI runs, release manifest, changelog, support matrix, and proposed tag.
- Confirm publication order and exact released dependency version.
- Verify all review findings are dispositioned and no P0/P1 issue is waived.

## Finding format
```text
ID:
SEVERITY: P0 | P1 | P2 | P3
REQUIREMENT / LAW:
LOCATION:
CLAIM UNDER REVIEW:
EVIDENCE:
IMPACT:
REQUIRED DISPOSITION:
RETEST:
```

## Verdicts
- **PASS:** all mandatory requirements evidenced; no unresolved blocker.
- **FAIL:** technical blocker or false closure remains.
- **OWNER-BLOCKED:** technical gate passes, explicit owner action remains.
- **EXTERNAL-BLOCKED:** legal/vendor/infrastructure prerequisite remains.

Never convert FAIL to PASS because implementation is large, tests are numerous, or a release deadline exists.
