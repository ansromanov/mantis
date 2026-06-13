# Installation

## One-liner install (recommended)

On Linux and macOS, the fastest way to get `tv` — no Rust toolchain required —
is the install script. It detects your OS/arch, downloads the matching prebuilt
binary, verifies its SHA-256 checksum, and installs it onto your `PATH`:

```sh
curl -fsSL https://raw.githubusercontent.com/ansromanov/tree-viewer/main/install.sh | sh
```

You can tweak the install with environment variables:

```sh
# install a specific release instead of the latest
TV_VERSION=v0.2.0 curl -fsSL https://raw.githubusercontent.com/ansromanov/tree-viewer/main/install.sh | sh

# install into a directory of your choice
TV_INSTALL_DIR="$HOME/bin" curl -fsSL https://raw.githubusercontent.com/ansromanov/tree-viewer/main/install.sh | sh
```

> Prefer to read before piping to a shell? Download
> [`install.sh`](https://raw.githubusercontent.com/ansromanov/tree-viewer/main/install.sh),
> inspect it, then run `sh install.sh`.

On **Windows**, download `tv-windows-x86_64.exe` from the
[latest release](https://github.com/ansromanov/tree-viewer/releases/latest) and
place it on your `PATH` (or use `cargo install tree-viewer`).

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

## Prebuilt binaries

If you'd rather not use the install script, prebuilt binaries are attached to
every [release](https://github.com/ansromanov/tree-viewer/releases):

| Platform        | Architecture    | File                          |
| --------------- | --------------- | ----------------------------- |
| Linux (musl)    | x86_64          | `tv-linux-x86_64`             |
| Linux (musl)    | arm64 / aarch64 | `tv-linux-aarch64`            |
| macOS           | Apple Silicon   | `tv-macos-aarch64`            |
| macOS           | Intel           | `tv-macos-x86_64`             |
| Windows         | x86_64          | `tv-windows-x86_64.exe`       |

Download the appropriate binary for your platform from the latest release,
make it executable (`chmod +x` on Linux/macOS), and place it somewhere on
your `PATH` (e.g. `/usr/local/bin` or `~/.cargo/bin`).

Each release also ships a `SHA256SUMS` file so you can verify your download:

```sh
sha256sum --check --ignore-missing SHA256SUMS
```

## Via package managers

> Coming soon — Homebrew, Scoop, and more are on the roadmap.
