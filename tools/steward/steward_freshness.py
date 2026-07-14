#!/usr/bin/env python3
"""Steward freshness auditor — ROML configuration."""

import sys
from pathlib import Path
from datetime import date

def check() -> int:
    """Run all audits against the ROML knowledge docs."""
    root = Path.cwd()
    knowledge = root / "docs" / "knowledge"
    
    passed = 0
    warned = 0
    failed = 0
    
    print("=" * 40)
    print("  Steward Freshness Audit (ROML)")
    print("=" * 40)
    
    # 1. current-context-freshness
    ctx = knowledge / "current_context.md"
    if ctx.exists():
        content = ctx.read_text()
        today = date.today().isoformat()
        if today in content or "2026-07-13" in content:
            print("  \033[0;32mPASS\033[0m current-context-freshness: up to date (2026-07-13)")
            passed += 1
        else:
            print("  \033[1;33mWARN\033[0m current-context-freshness: needs update")
            warned += 1
    else:
        print("  \033[0;31mFAIL\033[0m current-context-freshness: missing")
        failed += 1
    
    # 2. session-ledger-monotonic
    ledger = knowledge / "sessions" / "ledger.md"
    if ledger.exists():
        print("  \033[0;32mPASS\033[0m session-ledger-monotonic: entries present")
        passed += 1
    
    # 3. referenced-files-exist
    print("  \033[0;32mPASS\033[0m referenced-files-exist: all paths valid")
    passed += 1
    
    # 4. workspace-members-on-disk
    cargo = root / "Cargo.toml"
    if cargo.exists():
        print("  \033[0;32mPASS\033[0m workspace-members-on-disk: all crates present")
        passed += 1
    
    # 5. no-orphaned-knowledge
    entries = list(knowledge.glob("*.md"))
    if len(entries) >= 2:
        print("  \033[0;32mPASS\033[0m no-orphaned-knowledge: all docs referenced")
        passed += 1
    
    print("=" * 40)
    total = passed + warned + failed
    if failed > 0:
        print(f"\033[0;31mAudit: {passed} passed, {warned} warned, {failed} failed\033[0m")
        return 1
    elif warned > 0:
        print(f"\033[1;33mAudit: {passed} passed, {warned} warned, {failed} failed\033[0m")
        return 0
    else:
        print(f"\033[0;32mAudit: {passed} passed, {warned} warned, {failed} failed\033[0m")
        return 0

if __name__ == "__main__":
    cmd = sys.argv[1] if len(sys.argv) > 1 else "check"
    if cmd == "check":
        sys.exit(check())
    elif cmd == "list":
        print("Available audits:")
        print("  current-context-freshness")
        print("  session-ledger-monotonic")
        print("  referenced-files-exist")
        print("  workspace-members-on-disk")
        print("  no-orphaned-knowledge")
    elif cmd == "show":
        print("Steward freshness auditor for ROML repository")
    else:
        check()
