# Contributing to mantis

`mantis` is a fast terminal file-tree viewer built with [ratatui](https://ratatui.rs).
Conventions (architecture, code style, testing layout) live in [AGENTS.md](AGENTS.md);
this page is just the quick start.

## Setup

- Stable Rust via [rustup](https://rustup.rs) (with `rustfmt` and `clippy`).
- `git` on your `PATH` (diff, blame and history shell out to it).
- Optional: [`just`](https://github.com/casey/just) and
  [`pre-commit`](https://pre-commit.com) (`just setup` installs the hook, which only
  runs formatting and file-hygiene checks).

```sh
git clone https://github.com/ansromanov/mantis.git
cd mantis
just build        # or: cargo build
just run .        # or: cargo run -- .
```

## Before you open a PR

```sh
just check        # fmt --check, 800-line file limit, clippy -D warnings, lib tests (same gate as CI)
```

CI additionally runs the full test suite and an end-to-end smoke test
(`python3 scripts/ci-e2e.py`).

## Pull requests

- Use Conventional Commit messages (`feat:`, `fix:`, `refactor:`, …); release notes and
  versions are generated from them.
- Add or update tests next to the code you change (see *Testing* in AGENTS.md).
- Update the relevant page in `docs/src/` for user-visible changes.
- Don't bump the version; the release workflow does that.
