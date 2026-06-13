default:
    @just --list

# install git hooks (run once after cloning)
setup:
    pre-commit install

# start a new feature branch from latest origin/main (e.g. just new my-feature)
new branch:
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

# build release and copy tv to ~/.cargo/bin (re-signs at destination on macOS)
install: release
    cp target/release/tv ~/.cargo/bin/tv
    {{ if os() == "macos" { "codesign --force -s - ~/.cargo/bin/tv" } else { "" } }}

# run tests
test *args:
    cargo test {{args}}

# type-check without building
check:
    cargo check

# lint with clippy
clippy:
    cargo clippy

# remove build artifacts
clean:
    cargo clean

# publish a GitHub release for the current version
publish:
    gh workflow run release.yml --ref main
