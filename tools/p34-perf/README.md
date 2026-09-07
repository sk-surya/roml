# `P34_PRIMITIVE_PARAMETER_UPDATE_V1` harness

Deterministic `set_parameter + solve` benchmark (contract §4). One binary,
no third-party dependencies beyond the measured workspace itself.

## Layout

- `Cargo.toml` — standalone crate (`[workspace]` isolates it; path
  dependencies `../..` and `../../roml-highs` resolve against whatever
  checkout this directory sits in).
- `src/main.rs` — generator (xorshift seed `0x524F4D4C`), driver, and JSON
  reporter. Uses only APIs present in both the current tree and the
  historical baseline, so the mathematical workload is identical.

## Historical baseline required no adapter

The same `src/main.rs` builds unmodified against `4d111cc` and the current
tree and reproduces the identical initial objective (`1284.736622`) and
sync classification. No `adapter_4d111cc.patch` exists because none is
needed; if a future tree breaks the intersection API, add the mechanical
patch here and record it in the perf report.

## Running the two arms (same machine, sequentially, release mode)

```bash
# Baseline arm (example; use an isolated worktree, never the live repo):
git worktree add --detach /tmp/p34-baseline 4d111cceafce17aea44a6e396a838d1cc9ef255d
mkdir -p /tmp/p34-baseline/tools/p34-perf/src
cp tools/p34-perf/Cargo.toml /tmp/p34-baseline/tools/p34-perf/
cp tools/p34-perf/src/main.rs /tmp/p34-baseline/tools/p34-perf/src/
cd /tmp/p34-baseline/tools/p34-perf
cargo build --release --locked --offline
./target/release/p34-perf --warmups 20 --measured 200 > baseline.json

# Candidate arm (exact P34 head worktree):
cd <p34-head>/tools/p34-perf
cargo build --release --locked --offline
./target/release/p34-perf --warmups 20 --measured 200 > candidate.json
```

## Gate

```text
candidate median - baseline median <= max(5% of baseline median, 50 microseconds)
```

Compare the `median_ms` fields. Record both JSON files, machine/CPU/RAM,
Rust toolchain, HiGHS version, and build mode in `M3_PERFORMANCE.md`.
