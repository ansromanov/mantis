#!/bin/sh
# tree-viewer (tv) installer.
#
# Downloads the prebuilt `tv` binary for your OS/arch from GitHub Releases,
# verifies its SHA-256 checksum, and installs it onto your PATH.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/ansromanov/tree-viewer/main/install.sh | sh
#
# Environment overrides:
#   TV_VERSION       release tag to install (default: latest), e.g. v0.2.0
#   TV_INSTALL_DIR   directory to install into (default: auto-detected PATH dir)

set -eu

REPO="ansromanov/tree-viewer"
BIN="tv"
VERSION="${TV_VERSION:-latest}"
INSTALL_DIR="${TV_INSTALL_DIR:-}"

# --- pretty output --------------------------------------------------------
if [ -t 2 ]; then
  BOLD="$(printf '\033[1m')"; RED="$(printf '\033[31m')"
  GREEN="$(printf '\033[32m')"; YELLOW="$(printf '\033[33m')"
  RESET="$(printf '\033[0m')"
else
  BOLD=""; RED=""; GREEN=""; YELLOW=""; RESET=""
fi

info()  { printf '%s\n' "${BOLD}==>${RESET} $*"; }
warn()  { printf '%s\n' "${YELLOW}warning:${RESET} $*" >&2; }
error() { printf '%s\n' "${RED}error:${RESET} $*" >&2; exit 1; }

need() { command -v "$1" >/dev/null 2>&1 || error "required tool not found: $1"; }

# --- detect platform ------------------------------------------------------
os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Linux)  os_part="linux" ;;
  Darwin) os_part="macos" ;;
  *) error "unsupported OS: $os (use the Windows binary or 'cargo install tree-viewer')" ;;
esac

case "$arch" in
  x86_64 | amd64)   arch_part="x86_64" ;;
  aarch64 | arm64)  arch_part="aarch64" ;;
  *) error "unsupported architecture: $arch" ;;
esac

asset="${BIN}-${os_part}-${arch_part}"

# --- resolve download tool + URLs -----------------------------------------
if command -v curl >/dev/null 2>&1; then
  download() { curl -fsSL "$1" -o "$2"; }
elif command -v wget >/dev/null 2>&1; then
  download() { wget -qO "$2" "$1"; }
else
  error "need either curl or wget to download"
fi

if [ "$VERSION" = "latest" ]; then
  base="https://github.com/${REPO}/releases/latest/download"
else
  base="https://github.com/${REPO}/releases/download/${VERSION}"
fi

# --- choose install dir ---------------------------------------------------
if [ -z "$INSTALL_DIR" ]; then
  if [ -w "/usr/local/bin" ]; then
    INSTALL_DIR="/usr/local/bin"
  else
    INSTALL_DIR="${HOME}/.local/bin"
  fi
fi

# --- download + verify ----------------------------------------------------
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

info "Downloading ${BOLD}${asset}${RESET} (${VERSION})"
download "${base}/${asset}" "${tmp}/${asset}" \
  || error "failed to download ${base}/${asset} (no release asset for ${os_part}-${arch_part}?)"

info "Verifying checksum"
download "${base}/SHA256SUMS" "${tmp}/SHA256SUMS" \
  || error "failed to download checksums from ${base}/SHA256SUMS"

if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "${tmp}/${asset}" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
  actual="$(shasum -a 256 "${tmp}/${asset}" | awk '{print $1}')"
else
  error "need sha256sum or shasum to verify the download"
fi

expected="$(awk -v a="$asset" '$2 == a || $2 == "*"a {print $1}' "${tmp}/SHA256SUMS")"
[ -n "$expected" ] || error "no checksum for ${asset} in SHA256SUMS"
[ "$expected" = "$actual" ] || error "checksum mismatch for ${asset}
  expected: ${expected}
  actual:   ${actual}"

# --- install --------------------------------------------------------------
mkdir -p "$INSTALL_DIR" || error "could not create ${INSTALL_DIR}"
chmod +x "${tmp}/${asset}"

if [ -w "$INSTALL_DIR" ]; then
  mv "${tmp}/${asset}" "${INSTALL_DIR}/${BIN}"
elif command -v sudo >/dev/null 2>&1; then
  warn "${INSTALL_DIR} is not writable; using sudo"
  sudo mv "${tmp}/${asset}" "${INSTALL_DIR}/${BIN}"
else
  error "${INSTALL_DIR} is not writable and sudo is unavailable; set TV_INSTALL_DIR to a writable directory"
fi

info "${GREEN}Installed${RESET} ${BIN} to ${BOLD}${INSTALL_DIR}/${BIN}${RESET}"

# --- PATH hint ------------------------------------------------------------
case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *) warn "${INSTALL_DIR} is not on your PATH. Add it, e.g.:
    export PATH=\"${INSTALL_DIR}:\$PATH\"" ;;
esac

info "Run ${BOLD}${BIN}${RESET} to view the current directory (press ${BOLD}?${RESET} for help)."
