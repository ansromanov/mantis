# Contributing to tree-viewer (`tv`)

Thanks for your interest in improving `tv`! This guide walks you through everything
you need to build the project, run the tests, and get your first pull request merged.

`tv` is a single-crate Rust project — a fast terminal file tree viewer built with
[ratatui](https://ratatui.rs). If anything here is unclear, please open an issue.

> **Conventions live in [AGENTS.md](AGENTS.md).** That file is the single source of
> truth for architecture, code style, and the dev workflow (it is also read by AI
> coding agents). This guide is the human-friendly onboarding path; AGENTS.md is the
> detailed reference. When the two disagree, AGENTS.md wins.

## Prerequisites

- **Rust toolchain** — stable, installed via [rustup](https://rustup.rs). CI builds on
  `stable`, so anything recent works. The `rustfmt` and `clippy` components are
  required (they ship with the default rustup profile).
- **[`just`](https://github.com/casey/just)** — the task runner that wraps the common
  `cargo` and `git` commands. Install with `cargo install just` (or your package
  manager). Run `just` with no arguments to list every recipe.
- **[`pre-commit`](https://pre-commit.com)** — runs formatting, type-check, and lint
  hooks before each commit. Install with `pipx install pre-commit` (or `pip install`),
  then run `just setup` once after cloning to wire up the git hook.
- **`git`** on your `PATH` — `tv`'s git features (diff, blame, history) shell out to it.
- **Platform notes** — no platform-specific system libraries are needed. On macOS the
  release build re-signs the binary automatically (`codesign`), handled by `just`.

Clone and bootstrap:

```sh
git clone https://github.com/ansromanov/tree-viewer.git
cd tree-viewer
just setup        # install the pre-commit git hook
```

## Building

```sh
just build        # debug build  (cargo build)        -> target/debug/tv
just release      # release build (cargo build --release) -> target/release/tv
```

The release profile strips symbols and enables LTO for a small, fast binary; use the
debug build for day-to-day development since it compiles much faster.

## Running

Launch the TUI against any directory or file:

```sh
just run .                 # view the current directory
just run path/to/dir       # view a specific directory
just run file.md           # open a single file
```

`just run` forwards its arguments to `cargo run -- …`. Press `?` in-app for the full
keybinding help and `q` to quit.

## Tests

```sh
just test                  # run the whole suite (cargo test)
just test <name>           # run tests matching a name
just check                 # type-check only, no test run
```

**Where tests live.** Unit tests are **co-located** with the module they cover in a
sibling `_test.rs` file — never an inline `#[cfg(test)] mod tests { … }` block:

- `src/foo.rs` → tests in `src/foo_test.rs`
- `src/app/mod.rs` → tests in `src/app/mod_test.rs`

Each `_test.rs` starts with `use super::*;` and contains bare `#[test]` functions. The
source file wires it up with one line:

```rust
#[cfg(test)]
#[path = "foo_test.rs"]
mod tests;
```

When you add tests to an existing module, append to its `_test.rs`. When you create a
new module, create its `_test.rs` companion at the same time. Cross-module / black-box
tests live in the integration `tests/` directory. See AGENTS.md for the full testing
guidelines (the `split-tests` skill automates extracting any inline block).

CI runs the suite with [`nextest`](https://nexte.st) under coverage and posts a
coverage summary on your PR; plain `cargo test` locally is sufficient before pushing.

## Code style

- **Formatting** — `cargo fmt --all` (enforced by `cargo fmt --all -- --check` in CI).
- **Linting** — `cargo clippy --all-targets -- -D warnings` must pass with zero warnings.
- **Indentation** 4 spaces, **line length** 100 chars max.
- **Naming** — `snake_case` for functions/vars/modules, `PascalCase` for types/enums.
- **Imports** grouped std → external crates → local modules, separated by blank lines.
- **Doc comments** on all public items; no tautological comments and no emoji/unicode
  in source.
- **No `unwrap`/`expect` in production paths** — file and git errors degrade gracefully
  to UI messages. Tests may use them freely.
- **File size limit** — keep every file (code and tests) under 600 lines; split into
  focused submodules when a file approaches the limit.

See [AGENTS.md → Rust Guidelines](AGENTS.md) for the complete style and
error-handling rules.

## Branch & PR workflow

Always branch from `origin/main` — never from another feature branch:

```sh
just new my-feature        # fetch origin/main, branch from it, install hooks
```

Make your changes, then before opening a PR confirm the checklist below passes
locally (pre-commit enforces the first four on commit):

1. `cargo fmt --all` — formatting clean
2. `cargo clippy --all-targets -- -D warnings` — no warnings
3. `cargo test` — all tests pass
4. `cargo check` — no type errors
5. No debug `println!`, `dbg!`, or commented-out code
6. No hardcoded secrets or credentials

Open the PR:

```sh
just pr            # fetch origin/main, rebase, push --force-with-lease
gh pr create       # the rebase fails loudly on conflicts so you can resolve them first
```

**Commit & PR titles** follow [Conventional Commits](https://www.conventionalcommits.org):
`feat:`, `fix:`, `refactor:`, `docs:`, `chore:`, `perf:`. If your PR resolves an issue,
put `Closes #<n>` in the body so it auto-closes on merge. The repo ships a
[PR template](.github/PULL_REQUEST_TEMPLATE.md) — fill it in.

**What CI checks.** On every PR, GitHub Actions runs (when code changes are present):

- **Linux** — `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and the
  test suite with coverage (posted as a PR comment).
- **Windows** — `cargo build --all-targets` and `cargo test --all`.
- **Bench smoke** — compiles the benchmarks and runs one group to catch crashes.

All jobs must be green before a PR is merged.

## Issue etiquette

- **Search first** — check [open issues](https://github.com/ansromanov/tree-viewer/issues)
  for an existing report before filing a new one.
- **Label appropriately** — use the existing labels: `bug`, `enhancement`,
  `performance`, `ux`, `refactor`, `documentation`.
- **Include a minimal reproduction** for bugs — steps to reproduce, what you expected,
  what happened, and your OS / terminal. A small example beats a long description.
- **Keep scope tight** — one issue per problem; split unrelated requests.

## File descriptions

We are rolling out a convention where every source file under `src/` carries a short
(≈10–15 line) header comment describing its responsibility within the crate (tracked in
issue [#174](https://github.com/ansromanov/tree-viewer/issues/174)). When you add a new
file, write its description; when you change a file's purpose, keep its description
current. This keeps the codebase navigable for newcomers and agents alike.

---

By contributing you agree that your contributions are licensed under the project's
[GPL-3.0-or-later](LICENSE) license.
