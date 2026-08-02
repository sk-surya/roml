---
schema_version: 1
open_count: 1
waived_count: 0
fixed_count: 0
total_count: 1
last_updated: 2026-08-02T21:48:18.561Z
---

# Broken Windows Ledger

> Cross-phase defect register. `/gsd-ship` blocks while `open_count > 0`.
> Waive with `gsd-tools windows waive <id> "<reason>"` (reason required).
> Mark fixed with `gsd-tools windows fixed <id>`.

| id | phase | kind | file | line | description | status | reason | recorded_at | resolved_at |
|----|-------|------|------|------|-------------|--------|--------|-------------|-------------|
| 1 | 24 | unrun-verify | roml-highs/Cargo.toml |  | cargo package -p roml-highs --locked not runnable pre-publish: roml 0.1.0 not on crates.io (cargo package requires versioned deps to resolve from a registry); validated via package --list + fresh packed consumer | open |  | 2026-08-02T21:48:18.561Z |  |

````json
[
  {
    "id": 1,
    "kind": "unrun-verify",
    "phase": "24",
    "file": "roml-highs/Cargo.toml",
    "line": null,
    "description": "cargo package -p roml-highs --locked not runnable pre-publish: roml 0.1.0 not on crates.io (cargo package requires versioned deps to resolve from a registry); validated via package --list + fresh packed consumer",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-02T21:48:18.561Z",
    "resolved_at": null
  }
]
````
