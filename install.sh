#!/bin/sh
# mantis installer.
#
# Downloads the prebuilt `mantis` binary for your OS/arch from GitHub Releases,
# verifies its SHA-256 checksum, and installs it onto your PATH.
# Supports Linux, macOS, and Windows (Git Bash / MSYS2 / Cygwin).
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/ansromanov/mantis/main/install.sh | sh
#
# Environment overrides:
#   MANTIS_VERSION       release tag to install (default: latest), e.g. v0.2.0
#   MANTIS_INSTALL_DIR   directory to install into (default: auto-detected PATH dir)

set -eu

REPO="ansromanov/mantis"
BIN="mantis"
VERSION="${MANTIS_VERSION:-latest}"
INSTALL_DIR="${MANTIS_INSTALL_DIR:-}"

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
  Linux)                         os_part="linux";   exe="" ;;
  Darwin)                        os_part="macos";   exe="" ;;
  CYGWIN* | MINGW* | MSYS*)     os_part="windows"; exe=".exe" ;;
  *) error "unsupported OS: $os (use 'cargo install mantis')" ;;
esac

case "$arch" in
  x86_64 | amd64)   arch_part="x86_64" ;;
  aarch64 | arm64)  arch_part="aarch64" ;;
  *) error "unsupported architecture: $arch" ;;
esac

asset="${BIN}-${os_part}-${arch_part}${exe}"

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
  : "${HOME:?HOME is unset; set MANTIS_INSTALL_DIR to specify install location}"
  # Pick a writable directory that is on PATH
  for _dir in "/usr/local/bin" "${HOME}/.local/bin"; do
    case ":${PATH}:" in
      *":${_dir}:"*)
        if [ -w "$_dir" ] || [ ! -e "$_dir" ]; then
          INSTALL_DIR="$_dir"
          break
        fi
        ;;
    esac
  done
  # Fallback: any writable PATH entry
  if [ -z "$INSTALL_DIR" ]; then
    IFS=:
    for _dir in $PATH; do
      [ -w "$_dir" ] && { INSTALL_DIR="$_dir"; break; }
    done
    unset IFS
  fi
fi

# Last resort: write to ~/.local/bin (may not be on PATH)
if [ -z "$INSTALL_DIR" ]; then
  INSTALL_DIR="${HOME}/.local/bin"
  mkdir -p "$INSTALL_DIR" 2>/dev/null || true
  warn "installing to ${INSTALL_DIR} (not on PATH; set MANTIS_INSTALL_DIR to override)"
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
if ! mkdir -p "$INSTALL_DIR" 2>/dev/null; then
  if command -v sudo >/dev/null 2>&1; then
    warn "could not create ${INSTALL_DIR}; using sudo"
    sudo mkdir -p "$INSTALL_DIR"
  else
    error "could not create ${INSTALL_DIR} and sudo is unavailable; set MANTIS_INSTALL_DIR to a writable directory"
  fi
fi
chmod +x "${tmp}/${asset}"

if [ -w "$INSTALL_DIR" ]; then
  mv "${tmp}/${asset}" "${INSTALL_DIR}/${BIN}${exe}"
elif command -v sudo >/dev/null 2>&1; then
  warn "${INSTALL_DIR} is not writable; using sudo"
  sudo mv "${tmp}/${asset}" "${INSTALL_DIR}/${BIN}${exe}"
else
  error "${INSTALL_DIR} is not writable and sudo is unavailable; set MANTIS_INSTALL_DIR to a writable directory"
fi

info "${GREEN}Installed${RESET} ${BIN} to ${BOLD}${INSTALL_DIR}/${BIN}${exe}${RESET}"

# --- generate + install shell completions (best-effort) ------------------
installed_bin="${INSTALL_DIR}/${BIN}${exe}"
case "$os_part" in
  linux|macos)
    # Derive the share directory from the install prefix. If the binary went
    # into a system path (e.g. /usr/local/bin), use /usr/local/share; otherwise
    # use XDG_DATA_HOME or ~/.local/share.
    prefix="$(dirname "$INSTALL_DIR")"
    if [ "$prefix" = "/usr/local" ] || [ "$prefix" = "/usr" ]; then
      data_root="${prefix}/share"
    else
      data_root="${XDG_DATA_HOME:-$HOME/.local/share}"
    fi

    # Bash completions
    bash_comp_dirs="${data_root}/bash-completion/completions"
    if mkdir -p "$bash_comp_dirs" 2>/dev/null; then
      "$installed_bin" --completions bash > "$bash_comp_dirs/mantis" 2>/dev/null && \
        info "Installed bash completions to ${bash_comp_dirs}"
    fi

    # Zsh completions
    zsh_comp_dirs="${data_root}/zsh/site-functions"
    if mkdir -p "$zsh_comp_dirs" 2>/dev/null; then
      "$installed_bin" --completions zsh > "$zsh_comp_dirs/_mantis" 2>/dev/null && \
        info "Installed zsh completions to ${zsh_comp_dirs}"
    fi

    # Fish completions
    fish_comp_dirs="${data_root}/fish/vendor_completions.d"
    if mkdir -p "$fish_comp_dirs" 2>/dev/null; then
      "$installed_bin" --completions fish > "$fish_comp_dirs/mantis.fish" 2>/dev/null && \
        info "Installed fish completions to ${fish_comp_dirs}"
    fi

    # Man page
    man_dir="${data_root}/man/man1"
    if mkdir -p "$man_dir" 2>/dev/null; then
      "$installed_bin" --print-man-page > "$man_dir/mantis.1" 2>/dev/null && \
        info "Installed man page to ${man_dir}"
    fi
    ;;
esac

# --- PATH hint ------------------------------------------------------------
case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *) warn "${INSTALL_DIR} is not on your PATH. Add it, e.g.:
    export PATH=\"${INSTALL_DIR}:\$PATH\"" ;;
esac

info "Run ${BOLD}${BIN}${RESET} to view the current directory (press ${BOLD}?${RESET} for help)."
