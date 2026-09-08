#!/usr/bin/env python3
"""Generate a corner-case fixture tree for the capability-sweep skill.

Builds a directory tree that stresses the parts of mantis that only break at
scale: thousands of files, deep nesting, a huge single file, very long lines,
a large minified JSON, a wide CSV, plus the small format-detection samples
(binary, CRLF, BOM, tiny PNG). No third-party deps — stdlib only.

    python3 gen_fixtures.py <out-dir> [--quick]

--quick divides every count by ~10 for a fast smoke run. Prints a one-line
summary per fixture and the absolute out-dir path on the last line.
"""
import base64
import json
import os
import random
import subprocess
import sys
from pathlib import Path

# 1x1 transparent PNG.
PNG_1x1 = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="
)


def main() -> None:
    args = [a for a in sys.argv[1:] if not a.startswith("-")]
    quick = "--quick" in sys.argv
    if not args:
        print("usage: gen_fixtures.py <out-dir> [--quick]", file=sys.stderr)
        sys.exit(2)

    out = Path(args[0]).resolve()
    out.mkdir(parents=True, exist_ok=True)

    n_files = 400 if quick else 5000
    n_dirs = 20 if quick else 60
    deep_levels = 12 if quick else 30
    huge_lines = 6000 if quick else 60000
    wide_lines = 60 if quick else 500
    wide_cols = 4000 if quick else 10000
    json_keys = 2000 if quick else 20000
    csv_rows = 500 if quick else 5000

    rnd = random.Random(1234)
    exts = ["rs", "py", "js", "ts", "go", "json", "md", "yaml", "toml", "txt", "sh"]

    # --- many files across many subdirs ---------------------------------------
    many = out / "many_files"
    for d in range(n_dirs):
        sub = many / f"pkg_{d:03d}"
        sub.mkdir(parents=True, exist_ok=True)
    made = 0
    for i in range(n_files):
        d = i % n_dirs
        ext = exts[i % len(exts)]
        p = many / f"pkg_{d:03d}" / f"file_{i:05d}.{ext}"
        p.write_text(f"// generated fixture {i}\nname = {i}\nvalue = {rnd.randint(0, 1_000_000)}\n")
        made += 1
    print(f"many_files/        {made} files in {n_dirs} dirs")

    # --- deep nesting --------------------------------------------------------
    deep = out / "deep"
    cur = deep
    for lvl in range(deep_levels):
        cur = cur / f"level_{lvl:02d}"
        cur.mkdir(parents=True, exist_ok=True)
        (cur / f"note_{lvl}.md").write_text(f"# depth {lvl}\n\nnested marker line {lvl}\n")
    print(f"deep/              {deep_levels} nested levels")

    # --- huge single source file ------------------------------------------------
    huge = out / "huge.rs"
    with huge.open("w") as fh:
        fh.write("// huge generated file\n")
        for i in range(huge_lines):
            if i % 40 == 0:
                fh.write(f"\npub mod block_{i} {{\n")
            elif i % 40 == 39:
                fh.write("}\n")
            else:
                fh.write(f"    pub const K_{i}: u64 = {i};\n")
    print(f"huge.rs            {huge_lines} lines ({huge.stat().st_size // 1024} KiB)")

    # --- very long lines ---------------------------------------------------------
    wide = out / "wide.txt"
    with wide.open("w") as fh:
        for i in range(wide_lines):
            fh.write(f"L{i:04d} " + ("x" * wide_cols) + "\n")
    print(f"wide.txt           {wide_lines} lines x {wide_cols} cols")

    # --- large minified JSON (prettify stress) --------------------------------
    obj = {f"key_{i:06d}": {"i": i, "sq": i * i, "tag": f"t{i % 97}"} for i in range(json_keys)}
    (out / "big.json").write_text(json.dumps(obj, separators=(",", ":")))
    print(f"big.json           {json_keys} keys, minified")

    # --- wide CSV -------------------------------------------------------------
    csv = out / "data.csv"
    with csv.open("w") as fh:
        fh.write(",".join(f"col_{c}" for c in range(12)) + "\n")
        for r in range(csv_rows):
            fh.write(",".join(str(r * 12 + c) for c in range(12)) + "\n")
    print(f"data.csv           {csv_rows} rows x 12 cols")

    # --- small format-detection samples ------------------------------------------
    (out / "sample.png").write_bytes(PNG_1x1)
    (out / "binary.bin").write_bytes(bytes(rnd.randrange(256) for _ in range(2048)))
    (out / "crlf.txt").write_bytes(b"line one\r\nline two\r\nline three\r\n")
    (out / "bom.txt").write_bytes(b"\xef\xbb\xbffile with a UTF-8 BOM\nsecond line\n")
    md = "# Markdown sample\n\n- one\n- two\n\n```rust\nfn main() {}\n```\n\n> quote\n"
    (out / "readme.md").write_text(md)
    yaml = "root:\n  anchor: &a\n    x: 1\n  alias: *a\nlist:\n  - one\n  - two\n"
    (out / "config.yaml").write_text(yaml)
    print("samples            sample.png binary.bin crlf.txt bom.txt readme.md config.yaml")

    # --- git repo with history + working-tree changes (for git-mode at scale) ----
    try:
        env = {**os.environ, "GIT_AUTHOR_NAME": "fixture", "GIT_AUTHOR_EMAIL": "f@x.y",
               "GIT_COMMITTER_NAME": "fixture", "GIT_COMMITTER_EMAIL": "f@x.y"}
        run = lambda *c: subprocess.run(["git", "-C", str(out), *c], check=True,
                                        capture_output=True, env=env)
        run("init", "-q")
        run("add", "-A")
        run("commit", "-qm", "fixture base")
        (out / "huge.rs").write_text((out / "huge.rs").read_text() + "\n// working-tree edit\n")
        (out / "new_untracked.rs").write_text("fn added_after_commit() {}\n")
        print("git                initialised, 1 commit, 1 modified + 1 untracked")
    except Exception as e:  # noqa: BLE001
        print(f"git                skipped ({e})")

    print(out)


if __name__ == "__main__":
    main()
