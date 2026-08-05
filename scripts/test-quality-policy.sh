#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
checker="$repo_root/scripts/check-quality-policy.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

new_repo() {
  local dir="$1"
  git init -q "$dir"
  git -C "$dir" config user.email quality@example.invalid
  git -C "$dir" config user.name quality-test
  mkdir -p "$dir/src" "$dir/.github/workflows" "$dir/scripts"
  cp "$checker" "$dir/scripts/check-quality-policy.sh"
  printf 'fn baseline() {}\n' >"$dir/src/lib.rs"
  git -C "$dir" add .
  git -C "$dir" commit -qm baseline
  git -C "$dir" branch -M main
  git -C "$dir" checkout -qb change
}

expect_fail() {
  local dir="$1"
  if (cd "$dir" && bash scripts/check-quality-policy.sh main) >/dev/null 2>&1; then
    echo "expected policy failure in $dir" >&2
    exit 1
  fi
}

expect_pass() {
  local dir="$1"
  (cd "$dir" && bash scripts/check-quality-policy.sh main) >/dev/null
}

case1="$tmp/unmarked-ignore"
new_repo "$case1"
printf '#[test]\n#[ignore]\nfn skipped() {}\n' >>"$case1/src/lib.rs"
git -C "$case1" add . && git -C "$case1" commit -qm change
expect_fail "$case1"

case2="$tmp/marked-ignore"
new_repo "$case2"
printf '#[test]\n// quality-exception: owner=maintainer; reason=external solver; remove=mock available\n#[ignore]\nfn skipped() {}\n' >>"$case2/src/lib.rs"
git -C "$case2" add . && git -C "$case2" commit -qm change
expect_pass "$case2"

case3="$tmp/low-coverage"
new_repo "$case3"
printf 'jobs:\n  coverage:\n    steps:\n      - run: cargo llvm-cov --fail-under-lines 60\n' >"$case3/.github/workflows/quality.yml"
git -C "$case3" add . && git -C "$case3" commit -qm change
expect_fail "$case3"

case4="$tmp/valid-threshold"
new_repo "$case4"
printf 'jobs:\n  coverage:\n    steps:\n      - run: cargo llvm-cov --fail-under-lines 75\n' >"$case4/.github/workflows/quality.yml"
git -C "$case4" add . && git -C "$case4" commit -qm change
expect_pass "$case4"

echo "quality-policy tests: 4 passed"
