---
phase: 10
slug: backend-contract-migration-closure
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-18
---

# Phase 10 — Validation Strategy

> Per-phase validation contract for M1R-01 Backend Contract Migration Closure.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust native) |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p roml --all-targets` |
| **Full suite command** | `cargo test -p roml --all-targets && cargo clippy -p roml --all-targets -- -D warnings` |
| **Estimated runtime** | ~60 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p roml`
- **After every plan wave:** Run full suite
- **Before phase close:** Full suite + doc + semver-checks
- **Max feedback latency:** 60s

---

## Verification Commands

| What | Command |
|------|---------|
| Tests | `cargo test -p roml --all-targets` |
| Clippy | `cargo clippy -p roml --all-targets -- -D warnings` |
| Docs | `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` |
| Semver | `cargo semver-checks check-release -p roml` |
| Contract tests | `cargo test -p roml --test backend_contract` |

---

## Validation Sign-Off

- [ ] `SolverAdapter` and `drain_changes()` removed from public API
- [ ] `BackendSession` trait defined and implemented by `ReferenceBackend`
- [ ] All contract tests pass (M1R-C1–C8)
- [ ] No required test remains ignored
- [ ] clippy clean, doc warnings clean
- [ ] semver-checks passes
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
