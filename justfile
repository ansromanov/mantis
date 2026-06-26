default:
    @just --list

# install git hooks and required tools (run once after cloning)
setup:
    cargo install cargo-nextest --locked
    pre-commit install

# start a new feature branch from latest origin/main (e.g. just new my-feature)
# refuses to branch when the current branch already has an open PR — fixing
# review comments must push to that PR, not spawn a new branch. Override for
# genuinely unrelated work with: ALLOW_NESTED=1 just new my-feature
new branch:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ "${ALLOW_NESTED:-0}" != "1" ]] && pr=$(gh pr view --json number -q .number 2>/dev/null); then
        echo "[new] BLOCKED: current branch has open PR #$pr." >&2
        echo "      Fixing review comments? Don't branch — push to this branch then 'just resolve-threads'." >&2
        echo "      Genuinely starting unrelated work? Re-run: ALLOW_NESTED=1 just new {{branch}}" >&2
        exit 1
    fi
    cargo install cargo-nextest --locked
    git fetch origin
    git checkout -b {{branch}} origin/main
    pre-commit install

# check out an existing PR's branch to push fixes (never branch for fixes)
# usage: just fix 240
fix pr:
    gh pr checkout {{pr}}
    pre-commit install

# safe push before opening a PR: fetch latest main, rebase, then push
pr:
    git fetch origin
    git rebase origin/main
    git push -u origin HEAD --force-with-lease

# resolve every unresolved review thread on the current branch's PR
resolve-threads:
    ./scripts/resolve-threads.sh

# end-to-end ship of the current branch: fmt, related tests, push, then open a
# PR that closes the given issue (or update the existing PR + resolve threads).
# usage: just ship 239
ship issue:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo fmt --all
    just test-pr
    git push -u origin HEAD --force-with-lease
    if gh pr view --json number -q .number >/dev/null 2>&1; then
        echo "[ship] existing PR updated; resolving addressed review threads"
        ./scripts/resolve-threads.sh || true
    else
        gh pr create --title "$(git log -1 --format=%s)" --body "Closes #{{issue}}"
    fi

# build debug binary
build:
    cargo build

# build release binary (re-signs on macOS after strip invalidates the linker signature)
release:
    cargo build --release
    {{ if os() == "macos" { "codesign --force -s - target/release/mantis" } else { "" } }}

# run with optional args (e.g. just run /some/path)
run *args:
    cargo run -- {{args}}

# build release, copy mantis to ~/.cargo/bin, and install default themes
install: release
    #!/usr/bin/env sh
    set -e
    ext=""
    themes_dir="${XDG_CONFIG_HOME:-$HOME/.config}/mantis/themes"
    case "$(uname -s 2>/dev/null)" in
        CYGWIN*|MINGW*|MSYS*)
            ext=".exe"
            themes_dir="${APPDATA}/mantis/themes"
            ;;
    esac
    cp "target/release/mantis${ext}" "${CARGO_HOME:-$HOME/.cargo}/bin/mantis${ext}"
    [ "$(uname -s 2>/dev/null)" = "Darwin" ] && codesign --force -s - "${CARGO_HOME:-$HOME/.cargo}/bin/mantis"
    mkdir -p "${themes_dir}"
    cp themes/*.toml "${themes_dir}/"

# run tests
test *args:
    cargo test {{args}}

# run only tests related to files changed vs origin/main (fast path for PRs)
# falls back to the full suite when broad files (Cargo.toml, lib.rs, …) change
test-pr:
    #!/usr/bin/env bash
    set -euo pipefail
    changed=$( { git diff --name-only origin/main...HEAD; git diff --name-only; git diff --name-only --cached; } | sort -u )
    filterset=$(echo "$changed" | bash scripts/related-tests.sh)
    if [[ "$filterset" == "__ALL__" ]]; then
        echo "[test-pr] broad change detected — skipping (run 'cargo nextest run' manually if needed)"
        exit 0
    else
        echo "[test-pr] running related tests: $filterset"
        cargo nextest run -E "$filterset"
    fi

# type-check without building
check:
    cargo check

# lint with clippy
clippy:
    cargo clippy

# run performance benchmarks
bench *args:
    cargo bench {{args}}

# run benchmarks and save a dated report with system details + git commit
bench-report *args:
    ./scripts/bench-report.sh {{args}}

# remove build artifacts
clean:
    cargo clean

# publish a GitHub release for the current version
publish:
    gh workflow run release.yml --ref main
