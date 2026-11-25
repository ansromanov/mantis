#!/usr/bin/env python3
"""Fail if any production Rust source file is over the project's size limit.

Production files are every `src/**/*.rs` that is not a `*_test.rs` companion.
Test files have no hard limit. Run from the repository root (CI and
`just file-size` do).
"""

from __future__ import annotations

import sys
from pathlib import Path

LIMIT = 800


def production_files(root: Path) -> list[Path]:
    return sorted(
        path
        for path in root.rglob("*.rs")
        if path.is_file() and not path.name.endswith("_test.rs")
    )


def main() -> int:
    src = Path("src")
    if not src.is_dir():
        print("[file-size] run from the repository root (no src/ here)", file=sys.stderr)
        return 2

    offenders: list[tuple[str, int]] = []
    for path in production_files(src):
        with path.open(encoding="utf-8") as source:
            lines = sum(1 for _ in source)
        if lines > LIMIT:
            offenders.append((path.as_posix(), lines))

    if offenders:
        for name, lines in sorted(offenders, key=lambda item: -item[1]):
            print(f"{name}: {lines} lines (limit {LIMIT}); split the module")
        return 1
    print(f"[file-size] every production Rust file is at most {LIMIT} lines")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
