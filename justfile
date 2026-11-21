default:
    @just --list

# install git hooks and required tools (run once after cloning)
setup:
    cargo install cargo-nextest --locked
    pre-commit install

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

# Fast deterministic gate for local iteration and the shared project baseline.
fast-test:
    cargo test --lib

test-fast: fast-test

# run automated E2E tests (integration tests + whole-binary TUI smoke test)
test-e2e:
    cargo test --test e2e_tests
    python3 scripts/ci-e2e.py

# Local gate; mirrors CI.
check: fmt-check clippy fast-test

fmt-check:
    cargo fmt --all -- --check

# lint with clippy
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# run performance benchmarks
bench *args:
    cargo bench {{args}}

# run benchmarks and save a dated report with system details + git commit
bench-report *args:
    ./scripts/bench-report.sh {{args}}

# generate shell completions and man page into completions/ and man/ directories
generate-completions: build
    #!/usr/bin/env sh
    set -eu
    mkdir -p completions man
    for shell in bash zsh fish powershell; do
      target/debug/mantis --completions "$shell" > "completions/mantis.${shell}" 2>/dev/null
      echo "generated completions/mantis.${shell}"
    done
    # zsh convention: file is named _mantis
    cp completions/mantis.zsh completions/_mantis
    target/debug/mantis --print-man-page > man/mantis.1 2>/dev/null
    echo "generated man/mantis.1"

# build the documentation site into docs/book
# mdBook only copies files under docs/src, so repository images are staged
# into docs/src/media first (gitignored; the Pages workflow does the same).
docs:
    rm -rf docs/src/media
    cp -R media docs/src/media
    mdbook build docs

# serve the documentation site locally with live reload
docs-serve: docs
    mdbook serve docs --open

# re-render the documentation screenshots from docs/tapes/*.tape
# needs vhs (brew install vhs) and a release build of mantis
docs-screenshots: release
    #!/usr/bin/env bash
    set -euo pipefail
    command -v vhs >/dev/null || { echo "vhs not found: brew install vhs"; exit 1; }
    mkdir -p media/docs
    # vhs does not create its output directory.
    mkdir -p .vhs-out
    for tape in docs/tapes/*.tape; do
        name="$(basename "$tape" .tape)"
        echo "rendering ${name}"
        # Clear stale frames first: vhs writes into the directory without
        # emptying it, so a shorter run would otherwise leave the previous
        # run's higher-numbered frames behind for the copy below to pick up.
        rm -rf .vhs-out/"${name}".png
        PATH="$PWD/target/release:$PATH" vhs "$tape"
        # vhs writes one PNG per frame into a directory; the last text frame is
        # the settled screen the tape was driving toward.
        last="$(ls .vhs-out/"${name}".png/frame-text-*.png | tail -1)"
        cp "$last" "media/docs/${name}.png"
    done
    rm -rf .vhs-out
    echo "wrote media/docs/*.png"

# remove build artifacts
clean:
    cargo clean

# publish a GitHub release for the current version
publish:
    gh workflow run release.yml --ref main
