# MPY-01 Dependency Set (PY-05, PY-06)

**Recorded:** 2026-09-07. **Base:** `main@f279b03` + MPY-01 skeleton.

## Pinned set (exact; see `Cargo.lock`)

| Component | Pinned version | Source |
|---|---|---|
| Rust toolchain | 1.97.1 (core MSRV 1.85 preserved) | `rustc --version` |
| PyO3 | 0.29.0 (`pyo3 = "0.29"`, MSRV 1.83 ≤ 1.85) | `Cargo.lock` |
| rust-numpy (`numpy` crate) | 0.29.0 (`numpy = "0.29"`) | `Cargo.lock` |
| maturin | 1.15.0 | `pip list` (venv-mpy) |
| NumPy | 2.5.3 (`numpy>=2.0` runtime) | `pip list` (venv-mpy) |
| Python | 3.14.4 (CPython; 3.13 lane via CI) | `python --version` |
| pytest / mypy | 9.1.1 / 2.3.1 (dev) | `pip list` (venv-mpy) |
| highs-sys / HiGHS | 1.15.0 (workspace pin, unchanged) | `Cargo.lock` |

PyO3 0.29 pairs with `numpy` 0.29 (verified via resolver metadata).
`extension-module` feature enabled per maturin project layout.

## Distribution decision (DESIGN §4)

PyPI `roml` is owned by an unrelated 0.0.1 release (checked 2026-09-07 via
`pip index versions`). Distribution name is `roml-python`; import namespace
stays `roml`. No registry claim made; no publication occurred.

## Isolation (verified 2026-09-07)

- `cargo tree -p roml`: only `log` (+ dev-deps). No PyO3/NumPy/native.
- `cargo test -p roml --all-targets --locked`: full core suite green.
- `cargo package --list -p roml`: unchanged by the new member.
- Binding crate `roml-python` links `roml` + `roml-highs`; it never calls
  raw HiGHS independently.
- Core MSRV 1.85 intact (`cargo +1.85` lane in hosted CI); binding MSRV is
  1.85 as well (PyO3 0.29 requires 1.83).

## Build evidence

- `maturin develop` in venv-mpy: editable install works.
- `pytest python/tests/test_import.py`: 1 passed.
- `maturin build --release --locked --out dist`: manylinux x86_64 wheel;
  installs into a fresh venv outside the source tree and imports with
  `roml.__file__` inside site-packages.
