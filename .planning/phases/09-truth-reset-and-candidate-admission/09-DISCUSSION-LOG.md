# Phase 9: Truth reset and candidate admission — Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-17
**Phase:** 9 (M1R-00) — Truth reset and candidate admission
**Areas discussed:** Admission criteria & evidence format, Ignored test disposition, Branch contamination & replay strategy, Crates.io names & license verification

---

## Admission criteria & evidence format

| Option | Description | Selected |
|--------|-------------|----------|
| Answer from context | STATE.md vocabulary + TRACEABILITY.md format already defined this. Admission = "accepted" + requirement disposition. Single M1R-00-ADMISSION.md with source citations and CI output links. | ✓ |

**User's choice:** "you should be able to answer that from repo context"
**Notes:** Resolved by reading STATE.md (state vocabulary) and TRACEABILITY.md (evidence directory convention). No discussion needed — the program design docs already define this.

---

## Ignored test disposition

| Option | Description | Selected |
|--------|-------------|----------|
| Fix them | Promote all 11 ignored tests to mandatory in this phase | ✓ |
| Delete them | Remove tests that characterize broken behavior | |
| Defer to Phase 01 | Move P1/P2 test remediation to M1R-01 contract closure | |

**User's choice:** "fix the tests"
**Notes:** All 11 tests fixed in this phase. P2 characterization tests updated to document expected post-M1R-01 behavior with appropriate ignore annotations explaining the dependency.

---

## Branch contamination & replay strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Merge after repairs | Admit candidate as-is, fix in place | |
| Split and replay | Extract implementation commits onto clean PR branches | ✓ |
| Replace | Discard candidate entirely, reimplement from scratch | |

**User's choice:** "split and replay"
**Notes:** Implementation-only commits extracted from the candidate. Planning commits stay on the planning branch. Clean PRs opened from `main@ef37c88`.

---

## Crates.io names & license verification

| Option | Description | Selected |
|--------|-------------|----------|
| Resolve from context | These are external gates — can't be resolved by code audit. Need direction. | |
| Ask Opus for recommendation | Feed full planning context to Opus and get structured recommendation | ✓ |

**User's choice:** "if its messy, give opus planning context, roadmap, and ask it for direction"
**Notes:** Opus recommended:
- **License**: Record committed license files as evidence of intent. Disposition: OWNER-BLOCKED. Defer explicit confirmation to M1R-08. (Nothing in M1R-01 through M1R-07 depends on it.)
- **Crates.io names**: Run `cargo owner --list`, record results. No placeholder publication (violates D-011). PASS if owner-owned, OWNER-BLOCKED if available, EXTERNAL-BLOCKED if stranger-owned (program stop).

---

## Claude's Discretion

None — all areas resolved with user direction or repo-context analysis.

## Deferred Ideas

None.
