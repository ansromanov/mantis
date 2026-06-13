# Installation

## Via cargo install

If you have the Rust toolchain installed, the simplest way to install `tv` is:

```sh
cargo install tree-viewer
```

This compiles and places the `tv` binary in `~/.cargo/bin` (which should already be on your `PATH` after a standard `rustup` install).

To install directly from the git repository without a crates.io release:

```sh
cargo install --git https://github.com/ansromanov/tree-viewer
```

## From source (Rust toolchain required)

```sh
git clone https://github.com/ansromanov/tree-viewer.git
cd tree-viewer
cargo build --release
# binary is at target/release/tv
```

Or, if you have [`just`](https://github.com/casey/just):

```sh
just install   # builds --release and copies tv to ~/.cargo/bin
```

### Prerequisites

- [Rust](https://rustup.rs) (stable toolchain)
- A terminal that supports 24-bit color

## One-liner install

If you have `curl` and `tar` (or `unzip` on Windows):

```sh
curl -fsSL https://github.com/ansromanov/tree-viewer/releases/latest/download/tv-$(uname -s)-$(uname -m).tar.gz \
  | tar xz -C /usr/local/bin
```

## Prebuilt binaries

Prebuilt binaries are available for each [release](https://github.com/ansromanov/tree-viewer/releases):

| Platform        | Architecture    | File                          |
| --------------- | --------------- | ----------------------------- |
| Linux (musl)    | x86_64          | `tv-linux-x86_64`             |
| macOS           | Apple Silicon   | `tv-macos-aarch64`            |
| macOS           | Intel           | `tv-macos-x86_64`             |
| Windows         | x86_64          | `tv-windows-x86_64.exe`       |

Download the appropriate binary for your platform from the latest release,
make it executable (`chmod +x` on Linux/macOS), and place it somewhere on
your `PATH` (e.g. `/usr/local/bin` or `~/.cargo/bin`).

## Via package managers

> Coming soon — Homebrew, Scoop, and more are on the roadmap.
