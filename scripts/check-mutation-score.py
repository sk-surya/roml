#!/usr/bin/env python3
import json
import sys
from pathlib import Path


def strings(value):
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for item in value.values():
            yield from strings(item)
    elif isinstance(value, list):
        for item in value:
            yield from strings(item)


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: check-mutation-score.py OUTCOMES_JSON MIN_PERCENT", file=sys.stderr)
        return 2
    path = Path(sys.argv[1])
    minimum = float(sys.argv[2])
    data = json.loads(path.read_text())
    values = [s.lower() for s in strings(data)]
    killed = sum("caught" in s or "killed" in s for s in values)
    survived = sum("missed" in s or "survived" in s for s in values)
    timeout = sum("timeout" in s for s in values)
    unviable = sum("unviable" in s for s in values)
    denominator = killed + survived
    if denominator == 0:
        print("mutation-score: no viable mutant outcomes found", file=sys.stderr)
        return 2
    score = 100.0 * killed / denominator
    print(f"mutation-score: {score:.2f}% killed={killed} survived={survived} timeout={timeout} unviable={unviable}")
    return 0 if score >= minimum else 1


if __name__ == "__main__":
    raise SystemExit(main())
