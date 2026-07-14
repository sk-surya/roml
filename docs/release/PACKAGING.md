# Packaging Guide

How ROML crates are built, tested, and packaged for distribution.

## Crate map

| Crate | Publish | Purpose |
|-------|---------|---------|
| `roml` | yes | Solver-free MILP modeling kernel |
| `roml-highs` | yes (after P6) | HiGHS adapter |
| `roml-mosek` | no (experimental) | MOSEK adapter |
| `roml-xpress` | no (experimental) | FICO Xpress adapter |

## Build requirements

### Core (`roml`)

No native dependencies. Builds on stable Rust ≥ 1.85.

```bash
cargo build -p roml
cargo test -p roml --all-targets
cargo doc -p roml --no-deps
```

### HiGHS (`roml-highs`)

Requires HiGHS libraries. Two modes:

**Link existing install:**
```bash
HIGHS_ROOT=/opt/homebrew/opt/highs cargo build -p roml-highs
# or
HIGHS_LIB_DIR=/path/to/lib cargo build -p roml-highs
```

**Build from source:**
```bash
HIGHS_SOURCE_DIR=$HOME/src/HiGHS cargo build -p roml-highs
```

### MOSEK and Xpress

Require separately licensed solver installations. These crates remain
`publish = false` and are not part of the initial release train.

## Package verification

```bash
# List package contents (should be 31 files for roml)
cargo package --list -p roml

# Create package archive
cargo package -p roml --no-verify

# Verify from a fresh project
cargo new /tmp/roml-test && cd /tmp/roml-test
cargo add roml --path /path/to/roml
cargo build
```

## Publication order

1. `roml` (core) — no solver dependencies
2. `roml-highs` — depends on roml

Commercial adapters graduate independently.

## Excluded from packages

- `.claude/` — AI tooling config
- `AGENTS.md` — agent instructions
- `.github/` — CI workflows (repo-level)
- `docs/release/evidence/` — build evidence
- `docs/superpowers/` — planning artifacts
- `.gitignore`, `.planning/` — dev tooling

## MSRV

Minimum Supported Rust Version: **1.85** (stable)

The MSRV is tested in CI. Bumping the MSRV requires a changelog entry
and a minor version increment.

## License

`MIT OR Apache-2.0` (dual-licensed). Each crate's `Cargo.toml` inherits
the workspace license field.

## Release checklist

See `docs/release/RELEASE_CHECKLIST.md` for the full pre-publication
verification sequence.
