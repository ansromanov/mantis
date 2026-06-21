default:
    @just --list

# install git hooks and required tools (run once after cloning)
setup:
    cargo install cargo-nextest --locked
    pre-commit install

# start a new feature branch from latest origin/main (e.g. just new my-feature)
new branch:
    cargo install cargo-nextest --locked
    git fetch origin
    git checkout -b {{branch}} origin/main
    pre-commit install

# safe push before opening a PR: fetch latest main, rebase, then push
pr:
    git fetch origin
    git rebase origin/main
    git push -u origin HEAD --force-with-lease

# build debug binary
build:
    cargo build

# build release binary (re-signs on macOS after strip invalidates the linker signature)
release:
    cargo build --release
    {{ if os() == "macos" { "codesign --force -s - target/release/tv" } else { "" } }}

# run with optional args (e.g. just run /some/path)
run *args:
    cargo run -- {{args}}

# build release, copy tv to ~/.cargo/bin, and install default themes
install: release
    #!/usr/bin/env sh
    set -e
    ext=""
    themes_dir="${XDG_CONFIG_HOME:-$HOME/.config}/tree-viewer/themes"
    case "$(uname -s 2>/dev/null)" in
        CYGWIN*|MINGW*|MSYS*)
            ext=".exe"
            themes_dir="${APPDATA}/tree-viewer/themes"
            ;;
    esac
    cp "target/release/tv${ext}" "${CARGO_HOME:-$HOME/.cargo}/bin/tv${ext}"
    [ "$(uname -s 2>/dev/null)" = "Darwin" ] && codesign --force -s - "${CARGO_HOME:-$HOME/.cargo}/bin/tv"
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
    filterset=$(git diff --name-only origin/main...HEAD | bash scripts/related-tests.sh)
    if [[ "$filterset" == "__ALL__" ]]; then
        echo "[test-pr] broad change detected — running full suite"
        cargo nextest run
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
