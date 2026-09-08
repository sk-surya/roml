"""Test path bootstrap: make `benchmarks` helpers importable.

The installed `roml` distribution ships only the `roml` package; test
support modules under `python/benchmarks` stay in-tree. This hook
APPENDS the `python/` source directory to `sys.path` (never prepends),
so an installed `roml` in site-packages always wins over the source
tree, while `benchmarks.*` still resolves identically on editable and
installed-wheel runs.
"""

import os
import sys

sys.path.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
