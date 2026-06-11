default:
    @just --list

# build debug binary
build:
    cargo build

# build release binary
release:
    cargo build --release

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
