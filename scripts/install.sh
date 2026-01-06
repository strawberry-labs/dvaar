#!/bin/bash
set -euo pipefail

# Dvaar CLI Installer
# Usage: curl -sSL https://dvaar.io/install.sh | bash
#
# Works on:
# - macOS (Intel & Apple Silicon)
# - Linux (x64 & arm64)
# - Windows (via Git Bash)

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

    # Set binary name based on OS
    if [ "$OS" = "windows" ]; then
        BINARY_NAME="dvaar.exe"
    else
        BINARY_NAME="dvaar"
    fi
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

    if [ "$OS" = "windows" ]; then
        # Windows: use ~/.dvaar/bin (will be added to PATH)
        INSTALL_DIR="$HOME/.dvaar/bin"
        mkdir -p "$INSTALL_DIR"
    else
        # Unix: try /usr/local/bin first, then ~/.local/bin
        if [ -w "/usr/local/bin" ]; then
            INSTALL_DIR="/usr/local/bin"
        elif [ -d "$HOME/.local/bin" ]; then
            INSTALL_DIR="$HOME/.local/bin"
        else
            mkdir -p "$HOME/.local/bin"
            INSTALL_DIR="$HOME/.local/bin"
        fi
    fi
}

# Download and install
install_dvaar() {
    local TMP_DIR=$(mktemp -d)

    if [ "$OS" = "windows" ]; then
        # Windows: download .zip
        local DOWNLOAD_URL="https://github.com/${GITHUB_REPO}/releases/download/v${VERSION}/dvaar-${PLATFORM}.zip"

        info "Downloading dvaar v${VERSION} for ${PLATFORM}..."

        if ! curl -sSL "$DOWNLOAD_URL" -o "${TMP_DIR}/dvaar.zip"; then
            rm -rf "$TMP_DIR"
            error "Download failed. Binary may not exist for ${PLATFORM}."
        fi

        info "Extracting..."
        unzip -q "${TMP_DIR}/dvaar.zip" -d "$TMP_DIR"
    else
        # Unix: download .tar.gz
        local DOWNLOAD_URL="https://github.com/${GITHUB_REPO}/releases/download/v${VERSION}/dvaar-${PLATFORM}.tar.gz"

        info "Downloading dvaar v${VERSION} for ${PLATFORM}..."

        if ! curl -sSL "$DOWNLOAD_URL" -o "${TMP_DIR}/dvaar.tar.gz"; then
            rm -rf "$TMP_DIR"
            error "Download failed. Binary may not exist for ${PLATFORM}."
        fi

        info "Extracting..."
        tar -xzf "${TMP_DIR}/dvaar.tar.gz" -C "$TMP_DIR"
    fi

    info "Installing to ${INSTALL_DIR}..."

    if [ "$OS" = "windows" ]; then
        # Windows: just move the file
        mv "${TMP_DIR}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
    elif [ -w "$INSTALL_DIR" ]; then
        mv "${TMP_DIR}/dvaar" "${INSTALL_DIR}/${BINARY_NAME}"
        chmod +x "${INSTALL_DIR}/${BINARY_NAME}"
    else
        sudo mv "${TMP_DIR}/dvaar" "${INSTALL_DIR}/${BINARY_NAME}"
        sudo chmod +x "${INSTALL_DIR}/${BINARY_NAME}"
    fi

    rm -rf "$TMP_DIR"
}

# Add to PATH on Windows
add_to_path_windows() {
    info "Adding to Windows PATH..."

    # Check if already in PATH
    if powershell.exe -Command "[Environment]::GetEnvironmentVariable('PATH', 'User')" 2>/dev/null | grep -q "$INSTALL_DIR"; then
        info "Already in PATH"
        return
    fi

    # Add to user PATH via PowerShell
    local WIN_PATH=$(cygpath -w "$INSTALL_DIR" 2>/dev/null || echo "$INSTALL_DIR")
    powershell.exe -Command "[Environment]::SetEnvironmentVariable('PATH', [Environment]::GetEnvironmentVariable('PATH', 'User') + ';${WIN_PATH}', 'User')" 2>/dev/null || true

    warn "Restart your terminal for PATH changes to take effect"
}

# Check if directory is in PATH
check_path() {
    if [ "$OS" = "windows" ]; then
        add_to_path_windows
    elif [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
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
        success "dvaar v${VERSION} installed to ${INSTALL_DIR}/${BINARY_NAME}"
    fi
}

# Print next steps
print_next_steps() {
    echo ""
    echo -e "${BLUE}Dvaar is fully managed through the CLI - no dashboard needed!${NC}"
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""
    echo "Getting started:"
    echo ""
    echo -e "  ${GREEN}dvaar login${NC}            Authenticate with GitHub"
    echo -e "  ${GREEN}dvaar http 3000${NC}        Expose local port 3000"
    echo -e "  ${GREEN}dvaar http ./dist${NC}      Serve a static directory"
    echo ""
    echo "Tunnel management:"
    echo ""
    echo -e "  ${GREEN}dvaar ls${NC}               List active tunnels"
    echo -e "  ${GREEN}dvaar stop <id>${NC}        Stop a tunnel"
    echo -e "  ${GREEN}dvaar logs <id>${NC}        View tunnel logs"
    echo ""
    echo "Account & billing:"
    echo ""
    echo -e "  ${GREEN}dvaar usage${NC}            View bandwidth usage"
    echo -e "  ${GREEN}dvaar upgrade${NC}          Upgrade your plan"
    echo -e "  ${GREEN}dvaar billing${NC}          Manage subscription & invoices"
    echo ""
    echo "Maintenance:"
    echo ""
    echo -e "  ${GREEN}dvaar update${NC}           Update to latest version"
    echo -e "  ${GREEN}dvaar uninstall${NC}        Remove dvaar from your system"
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
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
