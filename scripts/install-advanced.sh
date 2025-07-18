#!/bin/bash

# Advanced libdplyr Installer Script
# This script provides advanced installation features including version management,
# automatic updates, and comprehensive error handling

set -e

# Colors for output
RED="\033[0;31m"
GREEN="\033[0;32m"
YELLOW="\033[0;33m"
BLUE="\033[0;34m"
MAGENTA="\033[0;35m"
CYAN="\033[0;36m"
CLEAR="\033[0m"

# Configuration
DEFAULT_VERSION="latest"
REPO_URL="https://github.com/mrchypark/libdplyr"
API_URL="https://api.github.com/repos/mrchypark/libdplyr"
DEFAULT_INSTALL_DIR="/usr/local/bin"
FALLBACK_INSTALL_DIR="$HOME/.local/bin"
BINARY_NAME="libdplyr"
CONFIG_DIR="$HOME/.config/libdplyr"
CONFIG_FILE="$CONFIG_DIR/install.conf"

# User-configurable variables
INSTALL_DIR="${DEFAULT_INSTALL_DIR}"
REQUESTED_VERSION="${DEFAULT_VERSION}"
FORCE_INSTALL=false
QUIET=false
DRY_RUN=false
AUTO_UPDATE=false
UNINSTALL=false
LIST_VERSIONS=false

# Logging functions
log_info() {
    if [ "$QUIET" != "true" ]; then
        echo -e "${BLUE}[INFO]${CLEAR} $1"
    fi
}

log_success() {
    if [ "$QUIET" != "true" ]; then
        echo -e "${GREEN}[SUCCESS]${CLEAR} $1"
    fi
}

log_warning() {
    if [ "$QUIET" != "true" ]; then
        echo -e "${YELLOW}[WARNING]${CLEAR} $1"
    fi
}

log_error() {
    echo -e "${RED}[ERROR]${CLEAR} $1" >&2
}

log_debug() {
    if [ "${DEBUG:-false}" = "true" ]; then
        echo -e "${CYAN}[DEBUG]${CLEAR} $1" >&2
    fi
}

# Show help message
show_help() {
    cat << EOF
Advanced libdplyr Installer v1.0.0

USAGE:
    install-advanced.sh [OPTIONS]

OPTIONS:
    --dir=PATH          Install libdplyr to PATH (default: ${DEFAULT_INSTALL_DIR})
    --version=VERSION   Install specific version (default: latest)
    --force             Force installation even if already installed
    --quiet             Suppress non-error output
    --dry-run           Show what would be done without actually doing it
    --auto-update       Enable automatic updates
    --uninstall         Uninstall libdplyr
    --list-versions     List available versions
    --help              Show this help message

EXAMPLES:
    # Install latest version
    ./install-advanced.sh

    # Install specific version
    ./install-advanced.sh --version=v0.1.0

    # Install to custom directory with auto-update
    ./install-advanced.sh --dir=\$HOME/bin --auto-update

    # List available versions
    ./install-advanced.sh --list-versions

    # Uninstall
    ./install-advanced.sh --uninstall

ENVIRONMENT VARIABLES:
    INSTALL_DIR         Installation directory (overridden by --dir)
    LIBDPLYR_VERSION    Version to install (overridden by --version)
    DEBUG               Enable debug output (true/false)

EOF
}

# Parse command line arguments
parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            --dir=*)
                INSTALL_DIR="${1#*=}"
                shift
                ;;
            --version=*)
                REQUESTED_VERSION="${1#*=}"
                shift
                ;;
            --force)
                FORCE_INSTALL=true
                shift
                ;;
            --quiet)
                QUIET=true
                shift
                ;;
            --dry-run)
                DRY_RUN=true
                shift
                ;;
            --auto-update)
                AUTO_UPDATE=true
                shift
                ;;
            --uninstall)
                UNINSTALL=true
                shift
                ;;
            --list-versions)
                LIST_VERSIONS=true
                shift
                ;;
            --help)
                show_help
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                show_help
                exit 1
                ;;
        esac
    done
}

# Detect platform
detect_platform() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"
    
    case "${OS}" in
        Linux*) OS="linux" ;;
        Darwin*) OS="macos" ;;
        *)
            log_error "Unsupported operating system: ${OS}"
            exit 1
            ;;
    esac
    
    case "${ARCH}" in
        x86_64) ARCH="x86_64" ;;
        arm64|aarch64) ARCH="aarch64" ;;
        *)
            log_error "Unsupported architecture: ${ARCH}"
            exit 1
            ;;
    esac
    
    log_debug "Detected platform: ${OS}-${ARCH}"
}

# Check dependencies
check_dependencies() {
    local missing_deps=()
    
    if command -v curl &>/dev/null; then
        DOWNLOAD_CMD="curl -sSL"
    elif command -v wget &>/dev/null; then
        DOWNLOAD_CMD="wget -qO-"
    else
        missing_deps+=("curl or wget")
    fi
    
    for cmd in tar mkdir chmod; do
        if ! command -v "$cmd" &>/dev/null; then
            missing_deps+=("$cmd")
        fi
    done
    
    if [ ${#missing_deps[@]} -ne 0 ]; then
        log_error "Missing dependencies: ${missing_deps[*]}"
        exit 1
    fi
}

# Get available versions from GitHub API
get_available_versions() {
    log_debug "Fetching available versions..."
    
    local response
    if ! response=$($DOWNLOAD_CMD "$API_URL/releases" 2>/dev/null); then
        log_error "Could not fetch version information"
        return 1
    fi
    
    # Extract version tags from JSON
    echo "$response" | grep '"tag_name":' | sed -E 's/.*"tag_name": "([^"]+)".*/\1/' | sort -V -r
}

# Get latest version
get_latest_version() {
    local response
    if ! response=$($DOWNLOAD_CMD "$API_URL/releases/latest" 2>/dev/null); then
        log_warning "Could not fetch latest version, using default"
        echo "v0.1.0"
        return
    fi
    
    echo "$response" | grep '"tag_name":' | sed -E 's/.*"tag_name": "([^"]+)".*/\1/'
}

# List available versions
list_versions() {
    log_info "Available libdplyr versions:"
    echo
    
    local versions
    if versions=$(get_available_versions); then
        echo "$versions" | while read -r version; do
            if [ -n "$version" ]; then
                echo "  $version"
            fi
        done
    else
        log_error "Could not retrieve version list"
        exit 1
    fi
}

# Check current installation
check_current_installation() {
    if [ -x "${INSTALL_DIR}/${BINARY_NAME}" ]; then
        local current_version
        current_version=$("${INSTALL_DIR}/${BINARY_NAME}" --version 2>/dev/null | head -1 || echo "unknown")
        echo "$current_version"
    else
        echo ""
    fi
}

# Save installation configuration
save_config() {
    local version="$1"
    local install_dir="$2"
    
    mkdir -p "$CONFIG_DIR"
    
    cat > "$CONFIG_FILE" << EOF
# libdplyr installation configuration
INSTALL_DIR="$install_dir"
INSTALLED_VERSION="$version"
AUTO_UPDATE=$AUTO_UPDATE
INSTALL_DATE="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
EOF
    
    log_debug "Configuration saved to $CONFIG_FILE"
}

# Load installation configuration
load_config() {
    if [ -f "$CONFIG_FILE" ]; then
        # shellcheck source=/dev/null
        source "$CONFIG_FILE"
        log_debug "Configuration loaded from $CONFIG_FILE"
    fi
}

# Download and install libdplyr
download_and_install() {
    local version="$1"
    local install_dir="$2"
    
    log_info "Installing libdplyr $version for ${OS}-${ARCH}..."
    
    local temp_dir
    temp_dir=$(mktemp -d)
    trap "rm -rf '$temp_dir'" EXIT
    
    local archive_name="libdplyr-${OS}-${ARCH}.tar.gz"
    local download_url="$REPO_URL/releases/download/$version/$archive_name"
    
    if [ "$DRY_RUN" = "true" ]; then
        log_info "[DRY RUN] Would download: $download_url"
        log_info "[DRY RUN] Would install to: $install_dir"
        return 0
    fi
    
    # Download
    log_info "Downloading $archive_name..."
    if ! $DOWNLOAD_CMD "$download_url" > "$temp_dir/$archive_name"; then
        log_error "Failed to download $version"
        return 1
    fi
    
    # Extract
    log_info "Extracting..."
    if ! tar -xzf "$temp_dir/$archive_name" -C "$temp_dir"; then
        log_error "Failed to extract archive"
        return 1
    fi
    
    # Find binary
    local binary_path
    if [ -f "$temp_dir/$BINARY_NAME" ]; then
        binary_path="$temp_dir/$BINARY_NAME"
    else
        binary_path=$(find "$temp_dir" -name "$BINARY_NAME" -type f -executable | head -1)
        if [ -z "$binary_path" ]; then
            log_error "Binary not found in archive"
            return 1
        fi
    fi
    
    # Create install directory
    if [ ! -d "$install_dir" ]; then
        if ! mkdir -p "$install_dir"; then
            log_error "Failed to create installation directory"
            return 1
        fi
    fi
    
    # Install binary
    if ! cp "$binary_path" "$install_dir/$BINARY_NAME"; then
        log_error "Failed to install binary"
        return 1
    fi
    
    chmod +x "$install_dir/$BINARY_NAME"
    
    # Save configuration
    save_config "$version" "$install_dir"
    
    log_success "Installation completed"
    return 0
}

# Uninstall libdplyr
uninstall_libdplyr() {
    load_config
    
    local install_dir="${INSTALLED_INSTALL_DIR:-$INSTALL_DIR}"
    local binary_path="$install_dir/$BINARY_NAME"
    
    if [ "$DRY_RUN" = "true" ]; then
        log_info "[DRY RUN] Would remove: $binary_path"
        log_info "[DRY RUN] Would remove: $CONFIG_FILE"
        return 0
    fi
    
    if [ -f "$binary_path" ]; then
        log_info "Removing libdplyr binary..."
        rm -f "$binary_path"
        log_success "Binary removed"
    else
        log_warning "Binary not found at $binary_path"
    fi
    
    if [ -f "$CONFIG_FILE" ]; then
        log_info "Removing configuration..."
        rm -f "$CONFIG_FILE"
        log_success "Configuration removed"
    fi
    
    log_success "libdplyr has been uninstalled"
}

# Check for updates
check_for_updates() {
    load_config
    
    if [ -z "${INSTALLED_VERSION:-}" ]; then
        log_info "No previous installation found"
        return 1
    fi
    
    local latest_version
    latest_version=$(get_latest_version)
    
    if [ "$INSTALLED_VERSION" != "$latest_version" ]; then
        log_info "Update available: $INSTALLED_VERSION → $latest_version"
        return 0
    else
        log_info "Already up to date ($INSTALLED_VERSION)"
        return 1
    fi
}

# Auto-update if enabled
auto_update_check() {
    load_config
    
    if [ "${AUTO_UPDATE:-false}" = "true" ]; then
        if check_for_updates; then
            log_info "Auto-update is enabled, installing latest version..."
            local latest_version
            latest_version=$(get_latest_version)
            download_and_install "$latest_version" "${INSTALLED_INSTALL_DIR:-$INSTALL_DIR}"
        fi
    fi
}

# Verify installation
verify_installation() {
    local install_dir="$1"
    local binary_path="$install_dir/$BINARY_NAME"
    
    if [ "$DRY_RUN" = "true" ]; then
        log_info "[DRY RUN] Would verify: $binary_path"
        return 0
    fi
    
    if [ ! -x "$binary_path" ]; then
        log_error "Binary not found or not executable"
        return 1
    fi
    
    local version_output
    if version_output=$("$binary_path" --version 2>&1); then
        log_success "Installation verified: $version_output"
    else
        log_error "Installation verification failed"
        return 1
    fi
    
    # Check PATH
    if command -v "$BINARY_NAME" &>/dev/null; then
        log_success "libdplyr is available in PATH"
    else
        log_warning "libdplyr is not in PATH"
        log_info "Add this to your shell profile:"
        log_info "  export PATH=\"$install_dir:\$PATH\""
    fi
}

# Main function
main() {
    parse_args "$@"
    
    if [ "$QUIET" != "true" ]; then
        echo -e "${MAGENTA}Advanced libdplyr Installer${CLEAR}"
        echo
    fi
    
    # Handle special modes
    if [ "$LIST_VERSIONS" = "true" ]; then
        list_versions
        exit 0
    fi
    
    if [ "$UNINSTALL" = "true" ]; then
        uninstall_libdplyr
        exit 0
    fi
    
    # Check for auto-updates first
    auto_update_check
    
    # Normal installation flow
    detect_platform
    check_dependencies
    
    # Determine version to install
    local target_version
    if [ "$REQUESTED_VERSION" = "latest" ]; then
        target_version=$(get_latest_version)
        log_info "Installing latest version: $target_version"
    else
        target_version="$REQUESTED_VERSION"
        log_info "Installing requested version: $target_version"
    fi
    
    # Check current installation
    local current_version
    current_version=$(check_current_installation)
    if [ -n "$current_version" ] && [ "$FORCE_INSTALL" != "true" ]; then
        log_info "Current installation: $current_version"
        if [ "$current_version" = "$target_version" ]; then
            log_info "Already installed, use --force to reinstall"
            exit 0
        fi
    fi
    
    # Try installation directory, fallback if needed
    if [ ! -w "$INSTALL_DIR" ] && [ "$INSTALL_DIR" = "$DEFAULT_INSTALL_DIR" ]; then
        log_warning "Cannot write to $INSTALL_DIR, using fallback"
        INSTALL_DIR="$FALLBACK_INSTALL_DIR"
    fi
    
    # Install
    if download_and_install "$target_version" "$INSTALL_DIR"; then
        verify_installation "$INSTALL_DIR"
        
        if [ "$QUIET" != "true" ]; then
            echo
            log_success "libdplyr installation completed!"
            echo
            log_info "Try it out:"
            echo "  echo \"select(name, age) %>% filter(age > 18)\" | libdplyr --pretty"
        fi
    else
        log_error "Installation failed"
        exit 1
    fi
}

# Run main function
main "$@"