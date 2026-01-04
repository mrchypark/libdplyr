#!/bin/bash
# libdplyr installation script for Linux and macOS
# Usage: curl -sSL https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.sh | bash

set -e

VERSION="0.2.0"
REPO="mrchypark/libdplyr"
BINARY_NAME="libdplyr"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Detect OS and architecture
detect_platform() {
    local os
    local arch

    case "$(uname -s)" in
        Linux*)
            os="linux"
            ;;
        Darwin*)
            os="macos"
            ;;
        *)
            log_error "Unsupported operating system: $(uname -s)"
            exit 1
            ;;
    esac

    case "$(uname -m)" in
        x86_64|amd64)
            arch="x86_64"
            ;;
        aarch64|arm64)
            if [ "$os" = "macos" ]; then
                arch="aarch64"
            else
                log_error "Unsupported architecture for $os: $(uname -m)"
                exit 1
            fi
            ;;
        *)
            log_error "Unsupported architecture: $(uname -m)"
            exit 1
            ;;
    esac

    echo "${os}-${arch}"
}

# Get the latest version from GitHub API
get_latest_version() {
    if command -v curl >/dev/null 2>&1; then
        curl -s "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' | sed 's/^v//'
    elif command -v wget >/dev/null 2>&1; then
        wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' | sed 's/^v//'
    else
        log_error "Neither curl nor wget is available. Please install one of them."
        exit 1
    fi
}

# Download and install binary
install_binary() {
    local platform="$1"
    local version="$2"
    local install_dir="${3:-/usr/local/bin}"

    # Use musl version for Linux for better compatibility
    if [[ "$platform" == "linux-x86_64" ]]; then
        platform="linux-x86_64-musl"
    fi

    local filename="${BINARY_NAME}-v${version}-${platform}"
    local archive_name="${filename}.tar.gz"
    local download_url="https://github.com/${REPO}/releases/download/v${version}/${archive_name}"

    log_info "Downloading ${BINARY_NAME} v${version} for ${platform}..."
    log_info "URL: ${download_url}"

    # Create temporary directory
    local temp_dir
    temp_dir=$(mktemp -d)
    cd "$temp_dir"

    # Download the archive
    if command -v curl >/dev/null 2>&1; then
        if ! curl -L -o "$archive_name" "$download_url"; then
            log_error "Failed to download $archive_name"
            exit 1
        fi
    elif command -v wget >/dev/null 2>&1; then
        if ! wget -O "$archive_name" "$download_url"; then
            log_error "Failed to download $archive_name"
            exit 1
        fi
    fi

    # Extract the archive
    log_info "Extracting archive..."
    if ! tar -xzf "$archive_name"; then
        log_error "Failed to extract archive"
        exit 1
    fi

    # Find the binary
    local binary_path
    if [ -f "$BINARY_NAME" ]; then
        binary_path="$BINARY_NAME"
    elif [ -f "./$BINARY_NAME" ]; then
        binary_path="./$BINARY_NAME"
    else
        log_error "Binary not found in archive"
        exit 1
    fi

    # Make binary executable
    chmod +x "$binary_path"

    # Install binary
    log_info "Installing to ${install_dir}..."

    # Check if we need sudo
    if [ -w "$install_dir" ]; then
        cp "$binary_path" "$install_dir/"
    else
        if command -v sudo >/dev/null 2>&1; then
            sudo cp "$binary_path" "$install_dir/"
        else
            log_error "Cannot write to $install_dir and sudo is not available"
            log_info "Please run this script as root or choose a different installation directory"
            exit 1
        fi
    fi

    # Cleanup
    cd /
    rm -rf "$temp_dir"

    log_success "${BINARY_NAME} v${version} installed successfully!"
}

# Verify installation
verify_installation() {
    local install_dir="${1:-/usr/local/bin}"
    local binary_path="${install_dir}/${BINARY_NAME}"

    if [ -x "$binary_path" ]; then
        log_info "Verifying installation..."
        if "$binary_path" --version >/dev/null 2>&1; then
            log_success "Installation verified!"
            log_info "Run '${BINARY_NAME} --help' to get started"
        else
            log_warning "Binary installed but may not be working correctly"
        fi
    else
        log_error "Installation verification failed"
        exit 1
    fi
}

# Main installation function
main() {
    local install_dir="/usr/local/bin"
    local use_latest=true
    local specified_version=""

    # Parse command line arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --version)
                specified_version="$2"
                use_latest=false
                shift 2
                ;;
            --install-dir)
                install_dir="$2"
                shift 2
                ;;
            --help)
                echo "libdplyr installation script"
                echo ""
                echo "Usage: $0 [OPTIONS]"
                echo ""
                echo "Options:"
                echo "  --version VERSION    Install specific version (default: latest)"
                echo "  --install-dir DIR    Installation directory (default: /usr/local/bin)"
                echo "  --help              Show this help message"
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                exit 1
                ;;
        esac
    done

    log_info "Starting libdplyr installation..."

    # Detect platform
    local platform
    platform=$(detect_platform)
    log_info "Detected platform: $platform"

    # Get version to install
    local version_to_install
    if [ "$use_latest" = true ]; then
        log_info "Fetching latest version..."
        version_to_install=$(get_latest_version)
        if [ -z "$version_to_install" ]; then
            log_warning "Could not fetch latest version, using default: $VERSION"
            version_to_install="$VERSION"
        fi
    else
        version_to_install="$specified_version"
    fi

    log_info "Installing version: $version_to_install"

    # Check if install directory exists and is writable
    if [ ! -d "$install_dir" ]; then
        log_error "Installation directory does not exist: $install_dir"
        exit 1
    fi

    # Install binary
    install_binary "$platform" "$version_to_install" "$install_dir"

    # Verify installation
    verify_installation "$install_dir"

    log_success "Installation complete!"
    echo ""
    echo "Next steps:"
    echo "  1. Make sure $install_dir is in your PATH"
    echo "  2. Run '${BINARY_NAME} --help' to see usage information"
    echo "  3. Try: ${BINARY_NAME} -t \"data %>% select(name, age)\""
}

# Run main function
main "$@"
