"""Test path bootstrap: make `benchmarks` helpers importable.

The installed `roml` distribution ships only the `roml` package; test
support modules under `python/benchmarks` stay in-tree. This hook puts
the `python/` source directory on `sys.path` so both editable and
installed-wheel runs resolve them identically.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
