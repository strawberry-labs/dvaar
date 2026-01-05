#!/bin/bash
set -euo pipefail

# Dvaar CLI Installer
# Usage: curl -sSL https://dvaar.io/install | bash

VERSION="${DVAAR_VERSION:-latest}"
GITHUB_REPO="strawberry-labs/dvaar"
INSTALL_DIR="${DVAAR_INSTALL_DIR:-}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info() { echo -e "${BLUE}==>${NC} $1"; }
success() { echo -e "${GREEN}==>${NC} $1"; }
warn() { echo -e "${YELLOW}==>${NC} $1"; }
error() { echo -e "${RED}==>${NC} $1"; exit 1; }

# Detect OS and architecture
detect_platform() {
    OS=$(uname -s | tr '[:upper:]' '[:lower:]')
    ARCH=$(uname -m)

    case "$OS" in
        darwin) OS="darwin" ;;
        linux) OS="linux" ;;
        mingw*|msys*|cygwin*) OS="windows" ;;
        *) error "Unsupported OS: $OS" ;;
    esac

    case "$ARCH" in
        x86_64|amd64) ARCH="x64" ;;
        arm64|aarch64) ARCH="arm64" ;;
        *) error "Unsupported architecture: $ARCH" ;;
    esac

    PLATFORM="${OS}-${ARCH}"
}

# Get latest version from GitHub
get_latest_version() {
    if [ "$VERSION" = "latest" ]; then
        VERSION=$(curl -sSL "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"v([^"]+)".*/\1/')
        if [ -z "$VERSION" ]; then
            error "Failed to get latest version. Try setting DVAAR_VERSION=x.x.x"
        fi
    fi
}

# Determine install directory
determine_install_dir() {
    if [ -n "$INSTALL_DIR" ]; then
        return
    fi

    # Try /usr/local/bin first (requires sudo)
    if [ -w "/usr/local/bin" ]; then
        INSTALL_DIR="/usr/local/bin"
    elif [ -d "$HOME/.local/bin" ]; then
        INSTALL_DIR="$HOME/.local/bin"
    else
        mkdir -p "$HOME/.local/bin"
        INSTALL_DIR="$HOME/.local/bin"
    fi
}

# Download and install
install_dvaar() {
    local TMP_DIR=$(mktemp -d)
    local BINARY_NAME="dvaar"
    local DOWNLOAD_URL="https://github.com/${GITHUB_REPO}/releases/download/v${VERSION}/dvaar-${PLATFORM}.tar.gz"

    info "Downloading dvaar v${VERSION} for ${PLATFORM}..."

    if ! curl -sSL "$DOWNLOAD_URL" -o "${TMP_DIR}/dvaar.tar.gz"; then
        rm -rf "$TMP_DIR"
        error "Download failed. Binary may not exist for ${PLATFORM}."
    fi

    info "Extracting..."
    tar -xzf "${TMP_DIR}/dvaar.tar.gz" -C "$TMP_DIR"

    info "Installing to ${INSTALL_DIR}..."
    if [ -w "$INSTALL_DIR" ]; then
        mv "${TMP_DIR}/dvaar" "${INSTALL_DIR}/${BINARY_NAME}"
        chmod +x "${INSTALL_DIR}/${BINARY_NAME}"
    else
        sudo mv "${TMP_DIR}/dvaar" "${INSTALL_DIR}/${BINARY_NAME}"
        sudo chmod +x "${INSTALL_DIR}/${BINARY_NAME}"
    fi

    rm -rf "$TMP_DIR"
}

# Check if directory is in PATH
check_path() {
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        warn "$INSTALL_DIR is not in your PATH"
        echo ""
        echo "Add this to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
        echo ""
        echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
        echo ""
    fi
}

# Verify installation
verify_install() {
    if command -v dvaar &> /dev/null; then
        success "dvaar v${VERSION} installed successfully!"
        echo ""
        dvaar --version
    else
        check_path
        success "dvaar v${VERSION} installed to ${INSTALL_DIR}/dvaar"
    fi
}

# Print next steps
print_next_steps() {
    echo ""
    echo "Next steps:"
    echo ""
    echo "  1. Login with GitHub:"
    echo "     ${GREEN}dvaar login${NC}"
    echo ""
    echo "  2. Expose a local port:"
    echo "     ${GREEN}dvaar http 3000${NC}"
    echo ""
    echo "Documentation: https://dvaar.io/docs"
    echo ""
}

# Main
main() {
    echo ""
    echo "  ____"
    echo " |  _ \\__   ____ _  __ _ _ __"
    echo " | | | \\ \\ / / _\` |/ _\` | '__|"
    echo " | |_| |\\ V / (_| | (_| | |"
    echo " |____/  \\_/ \\__,_|\\__,_|_|"
    echo ""
    echo " Localhost Tunnel Service"
    echo ""

    detect_platform
    get_latest_version
    determine_install_dir
    install_dvaar
    verify_install
    print_next_steps
}

main "$@"
