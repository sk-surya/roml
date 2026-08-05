#!/usr/bin/env bash
set -euo pipefail

base_ref="${1:-origin/main}"

if ! git rev-parse --verify "$base_ref" >/dev/null 2>&1; then
  echo "quality-policy: base ref '$base_ref' is unavailable" >&2
  exit 2
fi

merge_base="$(git merge-base HEAD "$base_ref")"
diff="$(git diff --unified=1 "$merge_base"...HEAD -- '*.rs' '.github/workflows/*.yml' '.github/workflows/*.yaml' 'mutants.toml')"

violations=0

# Added ignored tests need an explicit adjacent exception marker.
while IFS= read -r file; do
  [[ -z "$file" || ! -f "$file" ]] && continue
  while IFS=: read -r line_no _; do
    [[ -z "$line_no" ]] && continue
    start=$(( line_no > 1 ? line_no - 1 : 1 ))
    context="$(sed -n "${start},${line_no}p" "$file")"
    if ! grep -q 'quality-exception:' <<<"$context"; then
      echo "quality-policy: $file:$line_no adds #[ignore] without adjacent quality-exception:" >&2
      violations=$((violations + 1))
    fi
  done < <(grep -n '^[[:space:]]*#\[ignore' "$file" || true)
done < <(git diff --name-only --diff-filter=AM "$merge_base"...HEAD -- '*.rs')

# Required thresholds may be raised, but not reduced below the repository floor.
while IFS= read -r value; do
  if (( value < 75 )); then
    echo "quality-policy: coverage threshold reduced below 75%" >&2
    violations=$((violations + 1))
  fi
done < <(grep -oE '^\+.*--fail-under-lines[ =]+[0-9]+' <<<"$diff" | grep -oE '[0-9]+$' || true)

while IFS= read -r value; do
  if (( value < 80 )); then
    echo "quality-policy: mutation threshold reduced below 80%" >&2
    violations=$((violations + 1))
  fi
done < <(grep -oE '^\+.*check-mutation-score.py[^0-9]+[0-9]+' <<<"$diff" | grep -oE '[0-9]+$' || true)

if grep -qE '^\+.*continue-on-error:[[:space:]]*true' <<<"$diff"; then
  echo "quality-policy: required quality changes may not add continue-on-error: true" >&2
  violations=$((violations + 1))
fi

if (( violations > 0 )); then
  echo "quality-policy: $violations violation(s)" >&2
  exit 1
fi

echo "quality-policy: pass"
