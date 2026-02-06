#!/usr/bin/env bash
# RKnowledge Installer
# Usage: curl -fsSL https://raw.githubusercontent.com/algimantask/rknowledge/main/install.sh | bash
#   or:  curl -fsSL https://raw.githubusercontent.com/algimantask/rknowledge/main/install.sh | bash -s -- --version v0.1.0
#   or:  curl -fsSL https://raw.githubusercontent.com/algimantask/rknowledge/main/install.sh | bash -s -- --install-dir /usr/local/bin
set -euo pipefail

# ── Configuration ─────────────────────────────────────────────────────
REPO="algimantask/rknowledge"
BINARY="rknowledge"
INSTALL_DIR="${RKNOWLEDGE_INSTALL_DIR:-}"
VERSION=""
FORCE=false

# ── Colors ────────────────────────────────────────────────────────────
if [ -t 1 ] && command -v tput >/dev/null 2>&1; then
    BOLD=$(tput bold)
    DIM=$(tput dim)
    GREEN=$(tput setaf 2)
    CYAN=$(tput setaf 6)
    YELLOW=$(tput setaf 3)
    RED=$(tput setaf 1)
    RESET=$(tput sgr0)
else
    BOLD="" DIM="" GREEN="" CYAN="" YELLOW="" RED="" RESET=""
fi

# ── Helpers ───────────────────────────────────────────────────────────
info()  { printf "%s\n" "${GREEN}✓${RESET} $*"; }
warn()  { printf "%s\n" "${YELLOW}⚠${RESET} $*"; }
error() { printf "%s\n" "${RED}✗${RESET} $*" >&2; }
fatal() { error "$@"; exit 1; }

# ── Platform detection ────────────────────────────────────────────────
detect_os() {
    case "$(uname -s)" in
        Linux*)  echo "unknown-linux-gnu" ;;
        Darwin*) echo "apple-darwin" ;;
        MINGW*|MSYS*|CYGWIN*) echo "pc-windows-msvc" ;;
        *) fatal "Unsupported OS: $(uname -s)" ;;
    esac
}

detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64)  echo "x86_64" ;;
        arm64|aarch64) echo "aarch64" ;;
        *) fatal "Unsupported architecture: $(uname -m)" ;;
    esac
}

detect_install_dir() {
    if [ -n "$INSTALL_DIR" ]; then
        echo "$INSTALL_DIR"
        return
    fi
    # Prefer XDG, then ~/.local/bin, then /usr/local/bin
    if [ -n "${XDG_BIN_HOME:-}" ]; then
        echo "$XDG_BIN_HOME"
    elif [ -d "$HOME/.local/bin" ]; then
        echo "$HOME/.local/bin"
    elif [ -w "/usr/local/bin" ]; then
        echo "/usr/local/bin"
    else
        echo "$HOME/.local/bin"
    fi
}

# ── Version resolution ────────────────────────────────────────────────
get_latest_version() {
    local url="https://api.github.com/repos/${REPO}/releases/latest"
    local version

    if command -v curl >/dev/null 2>&1; then
        version=$(curl -fsSL "$url" 2>/dev/null | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
    elif command -v wget >/dev/null 2>&1; then
        version=$(wget -qO- "$url" 2>/dev/null | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
    fi

    echo "${version:-}"
}

# ── Download helpers ──────────────────────────────────────────────────
download() {
    local url="$1" dest="$2"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --progress-bar "$url" -o "$dest"
    elif command -v wget >/dev/null 2>&1; then
        wget -q --show-progress "$url" -O "$dest"
    else
        fatal "Neither curl nor wget found. Install one and retry."
    fi
}

verify_checksum() {
    local file="$1" expected="$2"
    local actual
    if command -v sha256sum >/dev/null 2>&1; then
        actual=$(sha256sum "$file" | cut -d' ' -f1)
    elif command -v shasum >/dev/null 2>&1; then
        actual=$(shasum -a 256 "$file" | cut -d' ' -f1)
    else
        warn "No sha256sum or shasum found, skipping checksum verification"
        return 0
    fi
    if [ "$actual" != "$expected" ]; then
        fatal "Checksum mismatch!\n  Expected: $expected\n  Actual:   $actual"
    fi
}

# ── Parse arguments ───────────────────────────────────────────────────
while [ $# -gt 0 ]; do
    case "$1" in
        --version|-v)    VERSION="$2"; shift 2 ;;
        --install-dir)   INSTALL_DIR="$2"; shift 2 ;;
        --force|-f)      FORCE=true; shift ;;
        --help|-h)
            cat <<EOF
${BOLD}RKnowledge Installer${RESET}

${CYAN}Usage:${RESET}
  curl -fsSL https://raw.githubusercontent.com/${REPO}/main/install.sh | bash
  curl -fsSL ... | bash -s -- [OPTIONS]

${CYAN}Options:${RESET}
  --version, -v <tag>     Install specific version (e.g., v0.1.0)
  --install-dir <path>    Install to specific directory
  --force, -f             Overwrite existing binary
  --help, -h              Show this help

${CYAN}Environment:${RESET}
  RKNOWLEDGE_INSTALL_DIR  Override default install directory
EOF
            exit 0 ;;
        *) fatal "Unknown option: $1. Use --help for usage." ;;
    esac
done

# ── Main install ──────────────────────────────────────────────────────
main() {
    printf "\n"
    printf "  ${BOLD}RKnowledge Installer${RESET}\n"
    printf "  ${DIM}https://github.com/${REPO}${RESET}\n"
    printf "\n"

    # Detect platform
    local os arch target
    os=$(detect_os)
    arch=$(detect_arch)
    target="${arch}-${os}"

    info "Platform: ${CYAN}${target}${RESET}"

    # Resolve version
    if [ -z "$VERSION" ]; then
        printf "  ${DIM}Fetching latest release...${RESET}\r"
        VERSION=$(get_latest_version)
        if [ -z "$VERSION" ]; then
            fatal "Could not determine latest version. Use --version to specify."
        fi
    fi
    info "Version: ${CYAN}${VERSION}${RESET}"

    # Determine install location
    local install_dir
    install_dir=$(detect_install_dir)
    info "Install directory: ${CYAN}${install_dir}${RESET}"

    # Check existing installation
    local dest="${install_dir}/${BINARY}"
    if [ -f "$dest" ] && [ "$FORCE" = false ]; then
        local existing_version
        existing_version=$("$dest" --version 2>/dev/null | head -1 || echo "unknown")
        warn "Existing installation found: ${existing_version}"
        warn "Use --force to overwrite, or remove ${dest} first"
        exit 0
    fi

    # Construct download URL
    local ext=""
    case "$os" in
        *windows*) ext=".exe" ;;
    esac

    local archive_name="${BINARY}-${VERSION}-${target}.tar.gz"
    local download_url="https://github.com/${REPO}/releases/download/${VERSION}/${archive_name}"
    local checksum_url="https://github.com/${REPO}/releases/download/${VERSION}/checksums.sha256"

    # Download to temp directory
    local tmp_dir
    tmp_dir=$(mktemp -d)
    trap 'rm -rf "$tmp_dir"' EXIT

    printf "\n"
    info "Downloading ${CYAN}${archive_name}${RESET}..."
    download "$download_url" "${tmp_dir}/${archive_name}"

    # Try checksum verification
    if download "$checksum_url" "${tmp_dir}/checksums.sha256" 2>/dev/null; then
        local expected_checksum
        expected_checksum=$(grep "${archive_name}" "${tmp_dir}/checksums.sha256" | cut -d' ' -f1)
        if [ -n "$expected_checksum" ]; then
            verify_checksum "${tmp_dir}/${archive_name}" "$expected_checksum"
            info "Checksum verified"
        fi
    fi

    # Extract
    info "Extracting..."
    tar -xzf "${tmp_dir}/${archive_name}" -C "${tmp_dir}"

    # Install
    mkdir -p "$install_dir"
    local binary_path="${tmp_dir}/${BINARY}${ext}"

    # Handle case where binary is inside a directory in the archive
    if [ ! -f "$binary_path" ]; then
        binary_path=$(find "$tmp_dir" -name "${BINARY}${ext}" -type f | head -1)
        if [ -z "$binary_path" ]; then
            fatal "Binary not found in archive"
        fi
    fi

    chmod +x "$binary_path"
    mv "$binary_path" "$dest"

    info "Installed to ${CYAN}${dest}${RESET}"

    # Verify installation
    if "$dest" --version >/dev/null 2>&1; then
        local installed_version
        installed_version=$("$dest" --version 2>/dev/null | head -1)
        info "Verified: ${GREEN}${installed_version}${RESET}"
    fi

    # Check PATH
    printf "\n"
    case ":$PATH:" in
        *":${install_dir}:"*) ;;
        *)
            warn "${install_dir} is not in your PATH"
            printf "\n"
            printf "  Add to your shell config:\n"
            printf "  ${DIM}# bash${RESET}\n"
            printf "  echo 'export PATH=\"\$PATH:${install_dir}\"' >> ~/.bashrc\n"
            printf "\n"
            printf "  ${DIM}# zsh${RESET}\n"
            printf "  echo 'export PATH=\"\$PATH:${install_dir}\"' >> ~/.zshrc\n"
            printf "\n"
            printf "  ${DIM}# fish${RESET}\n"
            printf "  fish_add_path ${install_dir}\n"
            printf "\n"
            ;;
    esac

    printf "  ${BOLD}${GREEN}Installation complete!${RESET}\n"
    printf "\n"
    printf "  Get started:\n"
    printf "    ${DIM}\$${RESET} rknowledge init    ${DIM}# Initialize & start Neo4j${RESET}\n"
    printf "    ${DIM}\$${RESET} rknowledge auth    ${DIM}# Configure LLM provider${RESET}\n"
    printf "    ${DIM}\$${RESET} rknowledge build . ${DIM}# Build knowledge graph${RESET}\n"
    printf "\n"
}

main
