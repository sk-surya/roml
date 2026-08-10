# P36 Wave 0 Baseline

Recorded before production edits on 2026-08-10.

## Source and toolchain

- Implementation branch: `phase-roml-P36-mps-writeback`
- Exact base: `48fab4db347522cebc786393e5afcbdbcea98f33` (merged PR #45)
- Rust: `rustc 1.97.1 (8bab26f4f 2026-07-14)`
- Cargo: `cargo 1.97.0 (c980f4866 2026-06-30)`
- P35 Netlib gitlink: `56257eea85b433ce6aa67d26156b36385318fd6f`

## Frozen corpus gate

The separately-run pinned Netlib manifest check passed at the exact gitlink:
the manifest and regular-file inventory both contain 94 `.mps` files and
compare exactly. The optional `testdata/corpora/infeasible-lps` setup was not
available for the full core test.

## Baseline verification

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo check -p roml --all-targets` | PASS |
| `cargo clippy -p roml --all-targets -- -D warnings` | PASS |
| `cargo test -p roml --all-targets` | SETUP FAILURE; `pinned_netlib_historical_fixed_files_import_when_initialized` returned `IncompleteSetup { missing: "./testdata/corpora/infeasible-lps" }` |
| Pinned Netlib manifest check at `56257eea85b433ce6aa67d26156b36385318fd6f` | PASS; 94/94 expected `.mps` names present with zero missing or unexpected files |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | PASS |
| `cargo package --list -p roml` | PASS |
| `cargo test -p roml --all-targets -- --list` | 1064 listed tests |
| `cargo test -p roml-highs --all-targets -- --list` | 157 listed tests |

## Public MPS surface inventory

Before P36, `roml::io::mps` exposes the P35 reader surface: format and vector
selection options, resource limits, `MpsReader`, source spans/diagnostics,
error kinds, metadata, source maps, and `MpsImport`. No writer module or
writer public types exist at this base.

## Scope guard

No production Rust, dependency, workflow, solver, or package changes are
included in this baseline record. The next commit is limited to the Wave 0
public seam, types, errors, and contract tests.
