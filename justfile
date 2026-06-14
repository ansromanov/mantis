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

# build release, copy tv to ~/.cargo/bin, and install default themes
install: release
    cp target/release/tv{{if os() == "windows" { ".exe" } else { "" }}} {{env_var_or_default("CARGO_HOME", home_directory() + "/.cargo")}}/bin/tv{{if os() == "windows" { ".exe" } else { "" }}}
    {{ if os() == "macos" { "codesign --force -s - " + env_var_or_default("CARGO_HOME", home_directory() + "/.cargo") + "/bin/tv" } else { "" } }}
    {{ if os() == "windows" { "mkdir -p \"$APPDATA/tree-viewer/themes\" && cp themes/*.toml \"$APPDATA/tree-viewer/themes/\"" } else { "mkdir -p ~/.config/tree-viewer/themes && cp themes/*.toml ~/.config/tree-viewer/themes/" } }}

# run tests
test *args:
    cargo test {{args}}

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
