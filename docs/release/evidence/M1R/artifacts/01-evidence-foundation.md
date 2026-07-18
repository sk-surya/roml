# M1R Evidence Artifact 01: Foundation

**Generated:** 2026-07-18
**Phase:** 09-truth-reset-and-candidate-admission (M1R-00)
**Plan:** 09-01
**Executor:** Claude Code (architect)
**Method:** Raw git commands and HTTP requests — no interpolated claims from RESEARCH.md beyond cross-reference.

---

## Commit Inventory

### Source Commands

```bash
# Candidate HEAD SHA
git rev-parse planning/roml-M1-native-backends-release
# → 649c635974cae1d6716bbc19429833a0135df22f

# Planning branch HEAD SHA
git rev-parse docs/public-release-production-roadmap
# → b38a5b256476266bc42bf0e1e978be8bb02f61a9

# Merge base between main and candidate branch
git merge-base main planning/roml-M1-native-backends-release
# → 82e2ed95545635b628187ba0081fe8c8b03eaafb

# Enumerate all 20 candidate commits
git log --oneline --decorate main..planning/roml-M1-native-backends-release
```

### Canonical Commit Inventory

The candidate branch `planning/roml-M1-native-backends-release` is 20 commits ahead of `main@82e2ed95`. Table ordered chronologically (oldest first).

| # | SHA (7) | Type | Description | Classification (per RESEARCH.md) | Touches .planning/? |
|---|---------|------|-------------|----------------------------------|---------------------|
| 1 | `073106f` | docs | docs(planning): add ROML M1 native backend mega roadmap | documentation-only | Yes — `.planning/milestones/ROML-M1-NATIVE-BACKENDS-RELEASE.md` |
| 2 | `bc8e3e0` | docs | docs(planning): add ROML M1 requirement contract | documentation-only | Yes — `.planning/milestones/ROML-M1-REQUIREMENTS.md` |
| 3 | `dba71c0` | docs | docs(planning): bootstrap ROML M1 state | documentation-only | Yes — `.planning/milestones/ROML-M1-STATE.md` |
| 4 | `302b098` | docs | docs(planning): add ROML M1 coding agent prompt | documentation-only | No |
| 5 | `c4db3e0` | feat | feat(M1.0): admission — licenses, support labels, ignored test reconciliation | contaminated | Yes — `.planning/milestones/ROML-M1-STATE.md` |
| 6 | `22074c0` | feat | feat(M1.1): backend contract freeze — protocol types, status lattice, taxonomy | contaminated | Yes — `.planning/milestones/ROML-M1-STATE.md` |
| 7 | `c1d5e90` | feat | feat(M1.2): migrate HiGHS FFI to maintained highs-sys 1.15.0 | implementation-only | No |
| 8 | `a94b75e` | docs | docs(M1): mark M1.2 complete — HiGHS highs-sys migration | documentation-only | Yes — `.planning/milestones/ROML-M1-STATE.md` |
| 9 | `c48f8c3` | feat | feat(M1.3): status lattice, error classification, and solve-request negotiation tests | implementation-only | No |
| 10 | `00a586c` | feat | feat(M1.3): semi-continuous recovery protocol tests | implementation-only | No |
| 11 | `084e58e` | feat | feat(M1.3): differential harness — commuting square, fault injection, multi-cursor | implementation-only | No |
| 12 | `5d117cf` | docs | docs(M1.3): semantic equivalence evidence tracker | documentation-only | Yes — `.planning/milestones/ROML-M1-STATE.md` |
| 13 | `537a035` | docs | docs(M1): mark M1.3 complete — commuting square proven | documentation-only | Yes — `.planning/milestones/ROML-M1-STATE.md` |
| 14 | `fb2cb88` | feat | feat(M1.4-M1.8): CI workflow, platform qual, performance plan, release prep | contaminated | Yes — `.planning/milestones/ROML-M1-STATE.md` |
| 15 | `8a1212f` | docs | docs(M1.6): MOSEK qualification plan | documentation-only | No |
| 16 | `cf8dc8b` | fix | fix: allow clippy::approx_constant in differential harness tests | implementation-only | No |
| 17 | `0fe295b` | fix | fix(M1.4): HiGHS adapter clippy clean, 11/11 tests pass locally | implementation-only | No |
| 18 | `85a6396` | docs | docs(M1): mark M1.4 complete — HiGHS 11/11 tests pass locally | documentation-only | Yes — `.planning/milestones/ROML-M1-STATE.md` |
| 19 | `97f8792` | feat | feat(M1.5): criterion benchmark harness — 4 benches, 100k iterations | contaminated | Yes — `.planning/milestones/ROML-M1-STATE.md` |
| 20 | `649c635` | fix | fix(M1.6,M1.7): clippy-clean workspace — mosek/xpress warnings resolved | implementation-only | No |

### Classification Summary

| Category | Count | Commits |
|----------|-------|---------|
| Implementation-only | 7 | c1d5e90, c48f8c3, 00a586c, 084e58e, cf8dc8b, 0fe295b, 649c635 |
| Contaminated (impl + planning) | 4 | c4db3e0, 22074c0, fb2cb88, 97f8792 |
| Documentation-only | 9 | 073106f, bc8e3e0, dba71c0, 302b098, a94b75e, 5d117cf, 537a035, 8a1212f, 85a6396 |
| **Total** | **20** | |

### Cross-Check Against RESEARCH.md

The 20 commits enumerated above exactly match the 20 commits listed in RESEARCH.md Candidate Commit Inventory (11 implementation + 9 documentation). All SHAs are identical. No missing or extra commits detected.

**Note on file paths:** RESEARCH.md references `.planning/ROADMAP.md` and `.planning/PROJECT.md` for contaminated planning touches. The actual file path on the candidate branch is `.planning/milestones/ROML-M1-STATE.md` — the candidate branch uses a nested milestones subdirectory rather than flat `.planning/` files. This is a minor path discrepancy but does not affect contamination classification.

### Stale Completion Claims

Three commits make milestone-completion claims that are not fully supported by current evidence:

| Commit | Claim | Actual State |
|--------|-------|-------------|
| `a94b75e` (~8) | "mark M1.2 complete" | M1.2 (highs-sys migration) is accurate — verified by dependency in `roml-highs/Cargo.toml`. Claim is factual but **stale** (no re-verification performed). |
| `537a035` (~13) | "mark M1.3 complete" | Partially true — ReferenceBackend commuting square proven via contract tests, but HiGHS differential harness has never executed on CI. |
| `85a6396` (~18) | "mark M1.4 complete" | Locally verified (commit 0fe295b message on local Mac). CI has never executed on GitHub runners. |
