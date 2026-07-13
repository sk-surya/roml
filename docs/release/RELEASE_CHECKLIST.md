# Release Checklist

This document describes the steps required to publish a new release of ROML and its adapter crates.

> **IMPORTANT**: No crate may be published to crates.io without explicit authorization from the repository owner(s). See [Publication Gate](#publication-gate) below.

## Pre-Release Checks

Run the following checks against the workspace. All must pass before proceeding.

### 1. Formatting

```bash
cargo fmt --all --check
```

All code must be formatted with `rustfmt` (default settings). This check is enforced by CI.

### 2. Clippy Linting

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Clippy must produce zero warnings across all workspace members.

### 3. Testing

```bash
cargo test --workspace
```

All unit tests, integration tests, and doc tests must pass. See the testing section of
[CONTRIBUTING.md](../CONTRIBUTING.md) for test organization details.

### 4. Documentation

```bash
cargo doc --workspace --no-deps
```

Documentation must build without warnings. Broken intra-doc links will cause failures.

### 5. Package Verification

Verify each crate that will be published to ensure it compiles from the `.crate` archive:

```bash
# Verify each crate packages cleanly
cargo package -p roml
cargo package -p roml-highs

# For gated crates
cargo package -p roml-mosek
cargo package -p roml-xpress
```

Each `cargo package` command must produce no warnings. Verify the `.crate` file that
`cargo package` generates (printed in the output):

```bash
# Verify the package in a fresh project
cd /tmp
cargo new verify-roml
cd verify-roml
cargo add --path /path/to/roml/target/package/roml-0.1.0.crate
cargo build
cargo test
```

### 6. Deny / Audit

If `cargo-deny` and `cargo-audit` are installed:

```bash
# Check for license compliance and duplicate dependencies
cargo deny check

# Check for known security vulnerabilities
cargo audit
```

Both must pass. Unlicensed dependencies or vulnerabilities with available fixes block the release.

### 7. Semver Compatibility

If `cargo-semver-checks` is installed:

```bash
cargo semver-checks --workspace
```

This is advisory during pre-1.0 development but required for 1.0+ releases.

### 8. Version Bump

Update version numbers in `Cargo.toml` for all crates being published. Workspace
members share versions at their discretion — each crate can version independently.

Update `CHANGELOG.md`:
- Move items from `[Unreleased]` to a new version section
- Add the release date
- Review categorization: Added, Changed, Deprecated, Removed, Fixed, Security

## Publication Order

Crates must be published in dependency order:

```
1. roml          (core crate — no workspace dependencies)
2. roml-highs    (depends on roml)
```

### Gated Crates

The following crates are gated and must not be published without explicit authorization
from the repository owner(s):

- `roml-mosek` (gated)
- `roml-xpress` (gated)

```bash
# Publish core crate
cargo publish -p roml

# Publish HiGHS adapter
cargo publish -p roml-highs
```

Wait for each `cargo publish` to complete and for the crate to appear on crates.io
before publishing the next.

## Publication Gate

**No crate may be published to crates.io without explicit authorization from the repository owner(s).**

This includes:
- All initial 0.x releases
- All patch, minor, and major version bumps
- Gated crates (roml-mosek, roml-xpress) regardless of owner authorization status for other crates

If you are not a repository owner, coordinate with an owner to authorize and perform
the publication.

## Post-Publication Verification

### 1. Verify docs.rs

Check that documentation builds successfully on docs.rs (typically within minutes of publication):

- `https://docs.rs/roml/<version>/roml/`
- `https://docs.rs/roml-highs/<version>/roml_highs/`
- `https://docs.rs/roml-mosek/<version>/roml_mosek/`
- `https://docs.rs/roml-xpress/<version>/roml_xpress/`

### 2. Verify crates.io

Confirm that each crate page displays correctly:

- `https://crates.io/crates/roml/<version>`
- `https://crates.io/crates/roml-highs/<version>`

### 3. Install Test

Test that the published crate can be used in a fresh project:

```bash
cd /tmp
cargo new test-roml-release
cd test-roml-release
cargo add roml@<version>
cargo build
```

### 4. Tag the Release

Once all publications are verified:

```bash
git tag v<version>
git push origin v<version>
```

### 5. Create GitHub Release (if applicable)

- Navigate to the repository's Releases page
- Draft a new release using the version tag
- Copy the relevant changelog section into the release notes
- Publish the release

## Release Artifacts

After a successful release, the following exist:

- Published crate(s) on crates.io
- Git tag (`v<version>`) pushed to the repository
- GitHub Release with changelog notes

## Rollback

Cargo does not support unpublishing most crate versions once published. If a critical
issue is discovered:

1. **Yank the crate version**: `cargo yank -p roml@<version>`
2. **Patch forward**: apply the fix and publish a new patch version
3. **Update changelog**: document the yanked version and its replacement
