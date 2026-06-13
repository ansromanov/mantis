default:
    @just --list

# install git hooks (run once after cloning)
setup:
    pre-commit install

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

# build release and copy tv to ~/.cargo/bin
install: release
    cp target/release/tv ~/.cargo/bin/tv

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
