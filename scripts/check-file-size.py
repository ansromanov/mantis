#!/usr/bin/env python3
"""Reject changed production Rust source files over the project's size limit."""

from __future__ import annotations

import sys
from pathlib import Path

LIMIT = 700


def main() -> int:
    offenders: list[tuple[str, int]] = []
    for raw_path in sys.stdin:
        name = raw_path.strip()
        if not name.startswith("src/") or not name.endswith(".rs"):
            continue
        path = Path(name)
        if name.endswith("_test.rs") or not path.is_file():
            continue
        with path.open(encoding="utf-8") as source:
            lines = sum(1 for _ in source)
        if lines > LIMIT:
            offenders.append((name, lines))

    if offenders:
        for name, lines in offenders:
            print(f"{name}: {lines} lines (limit {LIMIT}); split the module")
        return 1
    print(f"[file-size] changed production Rust files are at most {LIMIT} lines")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
