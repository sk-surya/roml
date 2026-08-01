#!/usr/bin/env python3
"""Generate a self-hosted shields-style coverage badge.

Reads the JSON emitted by `cargo llvm-cov report --summary-only --json`
and writes a small static SVG badge (badges/coverage.svg) that the
README references. No external badge service is involved.

Usage:
  cargo llvm-cov report --summary-only --json > coverage.json
  python3 scripts/coverage_badge.py coverage.json badges/coverage.svg
"""

import json
import sys


def pick_color(percent: float) -> str:
    """Shields-style color ramp for a percentage."""
    if percent >= 90.0:
        return "#4c1"
    if percent >= 80.0:
        return "#97ca00"
    if percent >= 70.0:
        return "#dfb317"
    if percent >= 60.0:
        return "#fe7d37"
    return "#e05d44"


def render_badge(label: str, value: str, color: str) -> str:
    """Render a flat shields-style badge as an SVG string."""
    label_w = 6 * len(label) + 10
    value_w = 6 * len(value) + 10
    total_w = label_w + value_w
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="{total_w}" height="20" role="img" aria-label="{label}: {value}">
  <title>{label}: {value}</title>
  <linearGradient id="s" x2="0" y2="100%">
    <stop offset="0" stop-opacity=".1" stop-color="#bbb"/>
    <stop offset="1" stop-opacity=".1"/>
  </linearGradient>
  <clipPath id="r"><rect width="{total_w}" height="20" rx="3" fill="#fff"/></clipPath>
  <g clip-path="url(#r)">
    <rect width="{label_w}" height="20" fill="#555"/>
    <rect x="{label_w}" width="{value_w}" height="20" fill="{color}"/>
    <rect width="{total_w}" height="20" fill="url(#s)"/>
  </g>
  <g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" font-size="11">
    <text x="{label_w // 2}" y="15" fill="#010101" fill-opacity=".3">{label}</text>
    <text x="{label_w // 2}" y="14">{label}</text>
    <text x="{label_w + value_w // 2}" y="15" fill="#010101" fill-opacity=".3">{value}</text>
    <text x="{label_w + value_w // 2}" y="14">{value}</text>
  </g>
</svg>
'''


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 2

    with open(sys.argv[1]) as f:
        report = json.load(f)

    totals = report["data"][0]["totals"]
    percent = totals["lines"]["percent"]

    svg = render_badge("coverage", f"{percent:.0f}%", pick_color(percent))
    with open(sys.argv[2], "w") as f:
        f.write(svg)

    print(f"coverage: {percent:.2f}% ({totals['lines']['covered']}/{totals['lines']['count']} lines)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
