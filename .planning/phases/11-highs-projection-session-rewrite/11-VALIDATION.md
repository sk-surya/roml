---
phase: 11
slug: highs-projection-session-rewrite
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-18
---

# Phase 11 — Validation Strategy

> Per-phase validation contract for M1R-02 HiGHS Projection/Session Rewrite.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust native) |
| **Quick run** | `cargo test -p roml-highs --all-targets` |
| **Full suite** | `cargo test -p roml-highs --all-targets && cargo clippy -p roml-highs --all-targets -- -D warnings` |
| **Estimated runtime** | ~5 min (includes highs-sys compilation) |

---

## Sampling Rate

- After every task commit: `cargo check -p roml-highs`
- After every plan wave: Full suite
- Before phase close: Full suite + doc + package + unsafe grep

---

## Verification Commands

| What | Command |
|------|---------|
| Check | `cargo check -p roml-highs` |
| Tests | `cargo test -p roml-highs --all-targets` |
| Clippy | `cargo clippy -p roml-highs --all-targets -- -D warnings` |
| Docs | `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` |
| Package | `cargo package -p roml-highs --locked` |
| Unsafe audit | `rg -n 'extern "C"|unsafe impl (Send|Sync)|assert!|unwrap\(|expect\(' roml-highs` |

---

## Validation Sign-Off

- [ ] All M1R-H1–H8 tests pass
- [ ] Handwritten ABI eliminated in favor of highs-sys
- [ ] No panic-based normal construction
- [ ] Unsafe review has no unresolved P0/P1 issue
- [ ] BackendSession implemented end-to-end
- [ ] `nyquist_compliant: true`

**Approval:** pending
