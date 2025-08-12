#!/bin/bash

# libdplyr installation script
# Supports Linux and macOS

set -e

# Configuration
REPO="example/libdplyr"  # Change this to your actual GitHub repository path
VERSION="0.1.0"  # This will be automatically updated during releases
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Color definitions
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

# Detect system information
detect_system() {
    local os
    local arch
    
    os=$(uname -s | tr '[:upper:]' '[:lower:]')
    arch=$(uname -m)
    
    case "$os" in
        linux*)
            OS="linux"
            ;;
        darwin*)
            OS="macos"
            ;;
        *)
            log_error "Unsupported operating system: $os"
            exit 1
            ;;
    esac
    
    case "$arch" in
        x86_64|amd64)
            ARCH="x86_64"
            ;;
        aarch64|arm64)
            ARCH="aarch64"
            ;;
        *)
            log_error "Unsupported architecture: $arch"
            exit 1
            ;;
    esac
    
    # On macOS, aarch64 is displayed as arm64
    if [[ "$OS" == "macos" && "$arch" == "arm64" ]]; then
        ARCH="aarch64"
    fi
    
    log_info "Detected system: $OS-$ARCH"
}

# Get latest version
get_latest_version() {
    if command -v curl >/dev/null 2>&1; then
        VERSION=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' | sed 's/^v//')
    elif command -v wget >/dev/null 2>&1; then
        VERSION=$(wget -qO- "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' | sed 's/^v//')
    else
        log_warning "curl or wget is required. Using default version $VERSION."
        return
    fi
    
    if [[ -z "$VERSION" ]]; then
        log_warning "Could not fetch latest version. Using default version $VERSION."
    else
        log_info "Latest version: $VERSION"
    fi
}

# Generate download URL
get_download_url() {
    local filename
    
    if [[ "$OS" == "linux" ]]; then
        # Prefer musl version for better compatibility
        if [[ "$ARCH" == "x86_64" ]]; then
            filename="libdplyr-linux-x86_64-musl.tar.gz"
        else
            filename="libdplyr-linux-$ARCH.tar.gz"
        fi
    elif [[ "$OS" == "macos" ]]; then
        filename="libdplyr-macos-$ARCH.tar.gz"
    fi
    
    DOWNLOAD_URL="https://github.com/$REPO/releases/download/v$VERSION/$filename"
    log_info "Download URL: $DOWNLOAD_URL"
}

# Download and install binary
install_binary() {
    local temp_dir
    temp_dir=$(mktemp -d)
    
    log_info "Temporary directory: $temp_dir"
    
    # Download
    log_info "Downloading libdplyr v$VERSION..."
    if command -v curl >/dev/null 2>&1; then
        curl -L "$DOWNLOAD_URL" -o "$temp_dir/libdplyr.tar.gz"
    elif command -v wget >/dev/null 2>&1; then
        wget "$DOWNLOAD_URL" -O "$temp_dir/libdplyr.tar.gz"
    else
        log_error "curl or wget is required."
        exit 1
    fi
    
    # Extract
    log_info "Extracting archive..."
    tar -xzf "$temp_dir/libdplyr.tar.gz" -C "$temp_dir"
    
    # Create install directory
    mkdir -p "$INSTALL_DIR"
    
    # Copy binary
    log_info "Installing to $INSTALL_DIR..."
    cp "$temp_dir/libdplyr-$OS-$ARCH" "$INSTALL_DIR/libdplyr"
    chmod +x "$INSTALL_DIR/libdplyr"
    
    # Clean up temporary files
    rm -rf "$temp_dir"
    
    log_success "libdplyr v$VERSION has been successfully installed!"
}

# Check PATH and provide guidance
check_path() {
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        log_warning "$INSTALL_DIR is not in your PATH."
        echo
        echo "Run the following command to add it to your PATH:"
        echo
        if [[ "$SHELL" == *"zsh"* ]]; then
            echo "  echo 'export PATH=\"$INSTALL_DIR:\$PATH\"' >> ~/.zshrc"
            echo "  source ~/.zshrc"
        elif [[ "$SHELL" == *"fish"* ]]; then
            echo "  fish_add_path $INSTALL_DIR"
        else
            echo "  echo 'export PATH=\"$INSTALL_DIR:\$PATH\"' >> ~/.bashrc"
            echo "  source ~/.bashrc"
        fi
        echo
        echo "Or for the current session only:"
        echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
    fi
}

# Verify installation
verify_installation() {
    if [[ -x "$INSTALL_DIR/libdplyr" ]]; then
        log_success "Installation verified: $INSTALL_DIR/libdplyr"
        
        # Check version
        if "$INSTALL_DIR/libdplyr" --version >/dev/null 2>&1; then
            local installed_version
            installed_version=$("$INSTALL_DIR/libdplyr" --version | grep -o '[0-9]\+\.[0-9]\+\.[0-9]\+' | head -1)
            log_success "Installed version: $installed_version"
        fi
        
        echo
        echo "Usage:"
        echo "  libdplyr --help"
        echo "  echo 'data %>% select(name, age)' | libdplyr --dialect postgresql"
    else
        log_error "Installation verification failed"
        exit 1
    fi
}

# Show help
show_help() {
    cat << EOF
libdplyr installation script

Usage:
  $0 [options]

Options:
  -h, --help              Show this help message
  -v, --version VERSION   Install specific version
  -d, --dir DIRECTORY     Specify installation directory (default: $HOME/.local/bin)
  --latest                Force check for latest version

Examples:
  $0                      # Install latest version
  $0 -v 0.2.0            # Install specific version
  $0 -d /usr/local/bin   # System-wide installation

Environment variables:
  INSTALL_DIR             Installation directory (default: $HOME/.local/bin)

EOF
}

# Main function
main() {
    local force_latest=false
    
    # Parse command line arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            -h|--help)
                show_help
                exit 0
                ;;
            -v|--version)
                VERSION="$2"
                shift 2
                ;;
            -d|--dir)
                INSTALL_DIR="$2"
                shift 2
                ;;
            --latest)
                force_latest=true
                shift
                ;;
            *)
                log_error "Unknown option: $1"
                show_help
                exit 1
                ;;
        esac
    done
    
    log_info "Starting libdplyr installation script"
    
    # Detect system
    detect_system
    
    # Get latest version (if version not explicitly specified)
    if [[ "$force_latest" == true ]] || [[ "$VERSION" == "0.1.0" ]]; then
        get_latest_version
    fi
    
    # Generate download URL
    get_download_url
    
    # Execute installation
    install_binary
    
    # Check PATH
    check_path
    
    # Verify installation
    verify_installation
    
    log_success "Installation completed!"
}

# Execute script
main "$@"