# Installation

## One-liner install (recommended)

### Linux / macOS

The fastest way to get `mantis` — no Rust toolchain required — is the install
script. It detects your OS/arch, downloads the matching prebuilt binary,
verifies its SHA-256 checksum, and installs it onto your `PATH`:

```sh
curl -fsSL https://raw.githubusercontent.com/ansromanov/mantis/main/install.sh | sh
```

You can tweak the install with environment variables:

```sh
# install a specific release instead of the latest
MANTIS_VERSION=v0.2.0 curl -fsSL https://raw.githubusercontent.com/ansromanov/mantis/main/install.sh | sh

# install into a directory of your choice
MANTIS_INSTALL_DIR="$HOME/bin" curl -fsSL https://raw.githubusercontent.com/ansromanov/mantis/main/install.sh | sh
```

> Prefer to read before piping to a shell? Download
> [`install.sh`](https://raw.githubusercontent.com/ansromanov/mantis/main/install.sh),
> inspect it, then run `sh install.sh`.

### Windows (PowerShell)

Run this in a PowerShell window — `curl` is not needed, `Invoke-WebRequest`
(`irm`) is built into PowerShell:

```powershell
irm https://raw.githubusercontent.com/ansromanov/mantis/main/install.ps1 | iex
```

From **cmd.exe**:

```cmd
powershell -ExecutionPolicy Bypass -c "irm https://raw.githubusercontent.com/ansromanov/mantis/main/install.ps1 | iex"
```

The script downloads `mantis.exe`, verifies its SHA-256 checksum, installs it to
`%CARGO_HOME%\bin` (if Rust is present) or `%LOCALAPPDATA%\Programs\mantis`,
and adds the directory to your user `PATH`.

You can override the install location:

```powershell
$env:MANTIS_VERSION    = 'v0.2.0'          # install a specific version
$env:MANTIS_INSTALL_DIR = "$HOME\bin"       # install to a custom directory
irm https://raw.githubusercontent.com/ansromanov/mantis/main/install.ps1 | iex
```

## Via cargo install

If you have the Rust toolchain installed, the simplest way to install `mantis` is:

```sh
cargo install mantis
```

This compiles and places the `mantis` binary in `~/.cargo/bin` (which should already be on your `PATH` after a standard `rustup` install).

To install directly from the git repository without a crates.io release:

```sh
cargo install --git https://github.com/ansromanov/mantis
```

## From source (Rust toolchain required)

```sh
git clone https://github.com/ansromanov/mantis.git
cd mantis
cargo build --release
# binary is at target/release/mantis
```

Or, if you have [`just`](https://github.com/casey/just):

```sh
just install   # builds --release and copies mantis to ~/.cargo/bin
```

### Prerequisites

- [Rust](https://rustup.rs) (stable toolchain)
- A terminal that supports 24-bit color

## Prebuilt binaries

If you'd rather not use the install script, prebuilt binaries are attached to
every [release](https://github.com/ansromanov/mantis/releases):

| Platform        | Architecture    | File                          |
| --------------- | --------------- | ----------------------------- |
| Linux (musl)    | x86_64          | `mantis-linux-x86_64`         |
| Linux (musl)    | arm64 / aarch64 | `mantis-linux-aarch64`        |
| macOS           | Apple Silicon   | `mantis-macos-aarch64`        |
| macOS           | Intel           | `mantis-macos-x86_64`         |
| Windows         | x86_64          | `mantis-windows-x86_64.exe`   |

Download the appropriate binary for your platform from the latest release,
make it executable (`chmod +x` on Linux/macOS), and place it somewhere on
your `PATH` (e.g. `/usr/local/bin` or `~/.cargo/bin`).

Each release also ships a `SHA256SUMS` file so you can verify your download:

```sh
sha256sum --check --ignore-missing SHA256SUMS
```

## Via package managers

### Homebrew (macOS / Linux)

The formula lives in this repository, so tap it by URL, then install:

```sh
brew tap ansromanov/mantis https://github.com/ansromanov/mantis
brew install mantis
```

The formula installs the prebuilt binary for your platform (Apple Silicon, Intel,
Linux x86_64, and Linux arm64) — no Rust toolchain required. It is bumped
automatically on every release, so `brew upgrade mantis` always tracks the latest
version.

> The explicit URL is needed because the repository is named `mantis`, not
> `homebrew-mantis`; Homebrew's short `brew tap user/repo` form only resolves the
> `homebrew-`-prefixed name.

### Other package managers

> Coming soon — Scoop, and more are on the roadmap.
