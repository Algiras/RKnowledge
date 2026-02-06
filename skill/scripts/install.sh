#!/bin/bash
# RKnowledge Installation Script
# Installs the rknowledge CLI tool

set -e

# Configuration
REPO="algimantask/rknowledge"
BINARY_NAME="rknowledge"
INSTALL_DIR="${RKNOWLEDGE_INSTALL_DIR:-${XDG_BIN_DIR:-$HOME/.local/bin}}"

# Detect OS and architecture
detect_platform() {
    local os arch

    case "$(uname -s)" in
        Linux*)  os="linux";;
        Darwin*) os="darwin";;
        MINGW*|MSYS*|CYGWIN*) os="windows";;
        *)       echo "Unsupported OS: $(uname -s)"; exit 1;;
    esac

    case "$(uname -m)" in
        x86_64|amd64) arch="x86_64";;
        arm64|aarch64) arch="aarch64";;
        *)            echo "Unsupported architecture: $(uname -m)"; exit 1;;
    esac

    echo "${os}-${arch}"
}

# Get latest release version
get_latest_version() {
    curl -s "https://api.github.com/repos/${REPO}/releases/latest" | \
        grep '"tag_name":' | \
        sed -E 's/.*"([^"]+)".*/\1/'
}

# Download and install binary
install_binary() {
    local platform="$1"
    local version="$2"
    local download_url
    local tmp_dir

    tmp_dir=$(mktemp -d)
    trap "rm -rf $tmp_dir" EXIT

    # Construct download URL
    if [[ "$platform" == *"windows"* ]]; then
        download_url="https://github.com/${REPO}/releases/download/${version}/${BINARY_NAME}-${platform}.exe"
        BINARY_NAME="${BINARY_NAME}.exe"
    else
        download_url="https://github.com/${REPO}/releases/download/${version}/${BINARY_NAME}-${platform}"
    fi

    echo "Downloading ${BINARY_NAME} ${version} for ${platform}..."
    
    if command -v curl &> /dev/null; then
        curl -fsSL "$download_url" -o "${tmp_dir}/${BINARY_NAME}"
    elif command -v wget &> /dev/null; then
        wget -q "$download_url" -O "${tmp_dir}/${BINARY_NAME}"
    else
        echo "Error: curl or wget is required"
        exit 1
    fi

    # Create install directory if needed
    mkdir -p "$INSTALL_DIR"

    # Install binary
    chmod +x "${tmp_dir}/${BINARY_NAME}"
    mv "${tmp_dir}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"

    echo "Installed ${BINARY_NAME} to ${INSTALL_DIR}/${BINARY_NAME}"
}

# Check if directory is in PATH
check_path() {
    case ":$PATH:" in
        *":$INSTALL_DIR:"*) return 0;;
        *) return 1;;
    esac
}

# Main installation
main() {
    echo "Installing RKnowledge..."
    echo

    local platform version

    platform=$(detect_platform)
    echo "Detected platform: ${platform}"

    version=$(get_latest_version)
    if [[ -z "$version" ]]; then
        echo "Warning: Could not determine latest version, using 'latest'"
        version="latest"
    else
        echo "Latest version: ${version}"
    fi

    install_binary "$platform" "$version"

    echo
    echo "Installation complete!"

    if ! check_path; then
        echo
        echo "NOTE: ${INSTALL_DIR} is not in your PATH."
        echo "Add it with:"
        echo "  export PATH=\"\$PATH:${INSTALL_DIR}\""
        echo
        echo "Or add to your shell config (~/.bashrc, ~/.zshrc, etc.)"
    fi

    echo
    echo "Get started:"
    echo "  ${BINARY_NAME} init    # Initialize and start Neo4j"
    echo "  ${BINARY_NAME} --help  # Show all commands"
}

main "$@"
