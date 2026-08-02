#!/usr/bin/env bash
# P20 Task 2 — Reproduce and assert the README/MODELING_API documentation
# drift from the COMMITTED fixtures (API-08 evidence).
#
# Two failures are frozen in tests/ui/:
#   1. tests/ui/current_readme_drift.rs      -> E0432 (HighsAdapter does not exist)
#   2. tests/ui/current_solve_model_method.rs -> E0599 (solve_model not on HighsSession)
#
# Neither fixture is auto-discovered by Cargo (files under tests/ui/), so the
# default test suites stay green (API-10.1). This script temporarily copies
# each fixture into the roml-highs integration-test directory (where
# roml_highs is available as a dependency), compiles it, asserts the expected
# error code appears, prints the capture, and removes the temporary file.
#
# Usage: scripts/p20-capture-drift.sh
# Exit 0 when both expected errors are reproduced; non-zero otherwise.
#
# The captured output is recorded in docs/release/evidence/M2_P20_BASELINE.md.

set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

CAPTURE_DIR=roml-highs/tests
TMP_DRIFT="$CAPTURE_DIR/zz_p20_drift_capture.rs"
TMP_METHOD="$CAPTURE_DIR/zz_p20_solve_model_capture.rs"

cleanup() {
  rm -f "$TMP_DRIFT" "$TMP_METHOD"
}
trap cleanup EXIT

fail() {
  echo "FAIL: $1" >&2
  exit 1
}

echo "== 1/2: drift fixture (README HighsAdapter) -> expect E0432 =="
cp tests/ui/current_readme_drift.rs "$TMP_DRIFT"
DRIFT_OUTPUT=$(cargo check -p roml-highs --test zz_p20_drift_capture 2>&1 || true)
printf '%s\n' "$DRIFT_OUTPUT"
grep -q "error\[E0432\]" <<<"$DRIFT_OUTPUT" \
  || fail "E0432 (unresolved import HighsAdapter) not reproduced"
grep -q "no \`HighsAdapter\` in the root" <<<"$DRIFT_OUTPUT" \
  || fail "unexpected E0432 message"
rm -f "$TMP_DRIFT"

echo
echo "== 2/2: solve_model method fixture (HighsSession) -> expect E0599 =="
cp tests/ui/current_solve_model_method.rs "$TMP_METHOD"
METHOD_OUTPUT=$(cargo check -p roml-highs --test zz_p20_solve_model_capture 2>&1 || true)
printf '%s\n' "$METHOD_OUTPUT"
grep -q "error\[E0599\]" <<<"$METHOD_OUTPUT" \
  || fail "E0599 (no method solve_model) not reproduced"
grep -q "no method named \`solve_model\`" <<<"$METHOD_OUTPUT" \
  || fail "unexpected E0599 message"
rm -f "$TMP_METHOD"

echo
echo "OK: both documented drift failures reproduced from committed fixtures."
