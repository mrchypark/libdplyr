#!/bin/bash

# libdplyr Installation Script
# Enhanced with comprehensive error handling and user experience improvements

set -e

# Colors for output
RED="\033[0;31m"
GREEN="\033[0;32m"
YELLOW="\033[0;33m"
BLUE="\033[0;34m"
CYAN="\033[0;36m"
CLEAR="\033[0m"

# Configuration
REPO="libdplyr/libdplyr"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
FALLBACK_INSTALL_DIR="$HOME/.local/bin"
VERSION="${LIBDPLYR_VERSION:-latest}"
BINARY_NAME="libdplyr"
TEMP_DIR=""

# Progress tracking
TOTAL_STEPS=6
CURRENT_STEP=0

# Logging functions with enhanced formatting
log_info() {
    echo -e "${BLUE}[INFO]${CLEAR} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${CLEAR} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${CLEAR} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${CLEAR} $1" >&2
}

log_debug() {
    if [ "${DEBUG:-false}" = "true" ]; then
        echo -e "${CYAN}[DEBUG]${CLEAR} $1" >&2
    fi
}

# Enhanced progress indicator with visual progress bar
show_progress() {
    CURRENT_STEP=$((CURRENT_STEP + 1))
    local percentage=$((CURRENT_STEP * 100 / TOTAL_STEPS))
    local filled=$((percentage / 5))
    local empty=$((20 - filled))
    
    # Create progress bar
    local bar=""
    for ((i=0; i<filled; i++)); do bar+="█"; done
    for ((i=0; i<empty; i++)); do bar+="░"; done
    
    echo -e "${CYAN}[${CURRENT_STEP}/${TOTAL_STEPS}]${CLEAR} $1"
    echo -e "${BLUE}Progress: [${bar}] ${percentage}%${CLEAR}"
    echo
}

# Enhanced error handling with specific error codes and detailed troubleshooting
handle_error() {
    local error_code=$1
    local error_message="$2"
    local suggestion="$3"
    
    echo
    log_error "❌ $error_message"
    
    if [ -n "$suggestion" ]; then
        echo -e "${YELLOW}💡 Suggestion:${CLEAR} $suggestion"
    fi
    
    # Provide detailed troubleshooting based on error type
    case $error_code in
        1) 
            echo -e "${RED}🌐 Network Error Details:${CLEAR}"
            echo "  • Check your internet connection"
            echo "  • Verify DNS resolution: nslookup github.com"
            echo "  • If behind a corporate firewall, check proxy settings"
            echo "  • Try using a VPN if GitHub is blocked"
            echo "  • Manual download: https://github.com/$REPO/releases"
            ;;
        2) 
            echo -e "${RED}🔒 Permission Error Details:${CLEAR}"
            echo "  • Try running with sudo: sudo $0"
            echo "  • Or install to user directory: INSTALL_DIR=\$HOME/.local/bin $0"
            echo "  • Check directory permissions: ls -la $(dirname "$INSTALL_DIR")"
            echo "  • Ensure you have write access to the target directory"
            ;;
        3) 
            echo -e "${RED}🖥️  Platform Support Details:${CLEAR}"
            echo "  • Supported platforms: Linux (x86_64, ARM64), macOS (Intel, Apple Silicon)"
            echo "  • Current platform: $(uname -s) $(uname -m)"
            echo "  • For Windows: Use PowerShell installer (install.ps1)"
            echo "  • For other platforms: Build from source"
            echo "  • Check releases page: https://github.com/$REPO/releases"
            ;;
        4) 
            echo -e "${RED}⚙️  Installation Error Details:${CLEAR}"
            echo "  • Check available disk space: df -h"
            echo "  • Verify file permissions: ls -la $TEMP_DIR"
            echo "  • Try a different installation directory"
            echo "  • Clear temporary files: rm -rf /tmp/libdplyr-*"
            ;;
        5) 
            echo -e "${RED}✅ Verification Error Details:${CLEAR}"
            echo "  • Binary may be corrupted during download"
            echo "  • Check file integrity: file $FINAL_INSTALL_PATH"
            echo "  • Try re-downloading: rm $FINAL_INSTALL_PATH && $0"
            echo "  • Check system compatibility: ldd $FINAL_INSTALL_PATH"
            ;;
        *) 
            echo -e "${RED}❓ Unknown Error Details:${CLEAR}"
            echo "  • This is an unexpected error"
            echo "  • Please report this issue with full output"
            ;;
    esac
    
    echo
    echo -e "${CYAN}📋 System Information:${CLEAR}"
    echo "  • OS: $(uname -s) $(uname -r)"
    echo "  • Architecture: $(uname -m)"
    echo "  • Shell: $SHELL"
    echo "  • User: $(whoami)"
    echo "  • Install Directory: $INSTALL_DIR"
    echo "  • Version: $VERSION"
    
    echo
    echo -e "${YELLOW}🆘 Need Help?${CLEAR}"
    echo "  • GitHub Issues: https://github.com/$REPO/issues"
    echo "  • Documentation: https://github.com/$REPO#readme"
    echo "  • Include the above system information when reporting issues"
    
    # Cleanup on error
    cleanup_on_error
    
    echo
    echo -e "${RED}💥 Installation failed with error code $error_code${CLEAR}"
    exit $error_code
}

# Cleanup function
cleanup_on_error() {
    if [ -n "$TEMP_DIR" ] && [ -d "$TEMP_DIR" ]; then
        log_debug "Cleaning up temporary directory: $TEMP_DIR"
        rm -rf "$TEMP_DIR" 2>/dev/null || true
    fi
}

# Trap for cleanup
trap 'cleanup_on_error' EXIT

# Enhanced network connectivity check with detailed diagnostics
check_network() {
    show_progress "Checking network connectivity"
    
    local test_urls=(
        "https://api.github.com"
        "https://github.com"
        "https://google.com"
        "https://cloudflare.com"
    )
    
    local connectivity_issues=()
    local working_url=""
    
    for url in "${test_urls[@]}"; do
        log_debug "Testing connectivity to $url"
        if curl -s --connect-timeout 5 --max-time 10 "$url" >/dev/null 2>&1; then
            log_debug "✅ Network connectivity confirmed via $url"
            working_url="$url"
            break
        else
            connectivity_issues+=("❌ Failed to connect to $url")
        fi
    done
    
    if [ -z "$working_url" ]; then
        local error_details=""
        for issue in "${connectivity_issues[@]}"; do
            error_details="$error_details\n  $issue"
        done
        
        # Additional network diagnostics
        echo -e "${YELLOW}🔍 Network Diagnostics:${CLEAR}"
        
        # Check DNS resolution
        if command -v nslookup >/dev/null 2>&1; then
            echo "  • DNS test for github.com:"
            if nslookup github.com >/dev/null 2>&1; then
                echo "    ✅ DNS resolution working"
            else
                echo "    ❌ DNS resolution failed"
                connectivity_issues+=("DNS resolution issues detected")
            fi
        fi
        
        # Check proxy settings
        if [ -n "${HTTP_PROXY:-}" ] || [ -n "${HTTPS_PROXY:-}" ] || [ -n "${http_proxy:-}" ] || [ -n "${https_proxy:-}" ]; then
            echo "  • Proxy detected:"
            echo "    HTTP_PROXY: ${HTTP_PROXY:-${http_proxy:-not set}}"
            echo "    HTTPS_PROXY: ${HTTPS_PROXY:-${https_proxy:-not set}}"
        else
            echo "  • No proxy configuration detected"
        fi
        
        handle_error 1 "No network connectivity detected$error_details" \
            "1. Check your internet connection\n2. Try: ping google.com\n3. If behind a proxy, set HTTP_PROXY and HTTPS_PROXY\n4. Try using a VPN if GitHub is blocked\n5. Download manually from: https://github.com/$REPO/releases"
    fi
    
    log_success "Network connectivity verified"
}

# Enhanced platform detection with detailed error messages
detect_platform() {
    show_progress "Detecting platform"
    
    local os arch
    
    case "$(uname -s)" in
        Linux*)
            os="linux"
            log_debug "Detected Linux operating system"
            ;;
        Darwin*)
            os="macos"
            log_debug "Detected macOS operating system"
            ;;
        MINGW*|MSYS*|CYGWIN*)
            handle_error 3 "Windows is not supported by this installer" \
                "Please download the Windows binary manually from GitHub releases or use PowerShell installer (install.ps1)"
            ;;
        *)
            handle_error 3 "Unsupported operating system: $(uname -s)" \
                "Supported platforms: Linux, macOS. For other platforms, try building from source."
            ;;
    esac
    
    case "$(uname -m)" in
        x86_64|amd64)
            arch="x86_64"
            log_debug "Detected x86_64 architecture"
            ;;
        aarch64|arm64)
            arch="aarch64"
            log_debug "Detected ARM64 architecture"
            ;;
        *)
            handle_error 3 "Unsupported architecture: $(uname -m)" \
                "Supported architectures: x86_64, ARM64. For other architectures, try building from source."
            ;;
    esac
    
    PLATFORM="${os}-${arch}"
    log_info "Platform detected: $PLATFORM"
}

# Check required dependencies with helpful error messages
check_dependencies() {
    show_progress "Checking dependencies"
    
    local missing_deps=()
    local suggestions=()
    
    # Check for download tools
    if command -v curl >/dev/null 2>&1; then
        DOWNLOAD_CMD="curl -fsSL"
        log_debug "Using curl for downloads"
    elif command -v wget >/dev/null 2>&1; then
        DOWNLOAD_CMD="wget -qO-"
        log_debug "Using wget for downloads"
    else
        missing_deps+=("curl or wget")
        suggestions+=("Install curl: apt-get install curl (Ubuntu/Debian) or brew install curl (macOS)")
    fi
    
    # Check other required tools
    local required_tools=("tar" "chmod" "mkdir")
    for tool in "${required_tools[@]}"; do
        if ! command -v "$tool" >/dev/null 2>&1; then
            missing_deps+=("$tool")
            case "$tool" in
                tar) suggestions+=("Install tar: apt-get install tar (Ubuntu/Debian)") ;;
                chmod|mkdir) suggestions+=("$tool should be available by default on Unix systems") ;;
            esac
        fi
    done
    
    if [ ${#missing_deps[@]} -ne 0 ]; then
        local error_msg="Missing required dependencies: ${missing_deps[*]}"
        local suggestion_msg=""
        if [ ${#suggestions[@]} -gt 0 ]; then
            suggestion_msg=$(printf "%s\n" "${suggestions[@]}")
        fi
        handle_error 4 "$error_msg" "$suggestion_msg"
    fi
    
    log_debug "All dependencies satisfied"
}

# Get latest version with retry logic
get_latest_version() {
    show_progress "Fetching latest version"
    
    local api_url="https://api.github.com/repos/${REPO}/releases/latest"
    local max_retries=3
    local retry_count=0
    
    while [ $retry_count -lt $max_retries ]; do
        log_debug "Attempt $((retry_count + 1)) to fetch latest version"
        
        local response
        if response=$($DOWNLOAD_CMD "$api_url" 2>/dev/null); then
            local version
            version=$(echo "$response" | grep '"tag_name":' | sed -E 's/.*"tag_name": "([^"]+)".*/\1/')
            
            if [ -n "$version" ] && [[ "$version" =~ ^v[0-9]+\.[0-9]+\.[0-9]+.*$ ]]; then
                log_info "Latest version: $version"
                echo "$version"
                return 0
            else
                log_debug "Invalid version format received: $version"
            fi
        fi
        
        retry_count=$((retry_count + 1))
        if [ $retry_count -lt $max_retries ]; then
            log_debug "Retrying in 2 seconds..."
            sleep 2
        fi
    done
    
    handle_error 1 "Failed to fetch latest version after $max_retries attempts" \
        "Check your internet connection or try specifying a version manually with LIBDPLYR_VERSION=v1.0.0"
}

# Enhanced download with progress and verification
download_binary() {
    local version="$1"
    local platform="$2"
    
    show_progress "Downloading libdplyr $version"
    
    local binary_name="libdplyr-${platform}"
    local download_url="https://github.com/${REPO}/releases/download/${version}/${binary_name}"
    
    log_info "Downloading from: $download_url"
    
    # Create temporary directory
    TEMP_DIR=$(mktemp -d)
    local temp_file="$TEMP_DIR/libdplyr"
    
    # Download with progress indication
    if command -v curl >/dev/null 2>&1; then
        if ! curl -fL --progress-bar "$download_url" -o "$temp_file"; then
            handle_error 1 "Failed to download binary" \
                "Check if version $version exists at: https://github.com/${REPO}/releases"
        fi
    else
        if ! wget --progress=bar:force "$download_url" -O "$temp_file" 2>&1; then
            handle_error 1 "Failed to download binary" \
                "Check if version $version exists at: https://github.com/${REPO}/releases"
        fi
    fi
    
    # Verify download
    if [ ! -f "$temp_file" ] || [ ! -s "$temp_file" ]; then
        handle_error 1 "Downloaded file is empty or missing" \
            "Try downloading again or check the release page"
    fi
    
    # Make executable
    chmod +x "$temp_file"
    
    log_success "Download completed"
    echo "$temp_file"
}

# Enhanced installation with permission handling
install_binary() {
    local temp_binary="$1"
    local target_dir="$2"
    
    show_progress "Installing binary"
    
    local target_path="$target_dir/$BINARY_NAME"
    
    # Create target directory if it doesn't exist
    if [ ! -d "$target_dir" ]; then
        log_info "Creating directory: $target_dir"
        if ! mkdir -p "$target_dir" 2>/dev/null; then
            if [ "$target_dir" = "$INSTALL_DIR" ] && [ "$INSTALL_DIR" != "$FALLBACK_INSTALL_DIR" ]; then
                log_warning "Cannot create $target_dir, trying fallback location"
                install_binary "$temp_binary" "$FALLBACK_INSTALL_DIR"
                return
            else
                handle_error 2 "Cannot create directory: $target_dir" \
                    "Try running with sudo or choose a different directory with: INSTALL_DIR=\$HOME/bin $0"
            fi
        fi
    fi
    
    # Check write permissions
    if [ ! -w "$target_dir" ]; then
        if [ "$target_dir" = "$INSTALL_DIR" ] && [ "$INSTALL_DIR" != "$FALLBACK_INSTALL_DIR" ]; then
            log_warning "No write permission for $target_dir, trying fallback location"
            install_binary "$temp_binary" "$FALLBACK_INSTALL_DIR"
            return
        else
            handle_error 2 "No write permission for directory: $target_dir" \
                "Try running with sudo or choose a different directory with: INSTALL_DIR=\$HOME/bin $0"
        fi
    fi
    
    # Install binary
    if ! cp "$temp_binary" "$target_path"; then
        handle_error 4 "Failed to copy binary to $target_path" \
            "Check disk space and permissions"
    fi
    
    # Ensure executable permissions
    if ! chmod +x "$target_path"; then
        handle_error 4 "Failed to set executable permissions" \
            "Try: chmod +x $target_path"
    fi
    
    log_success "Binary installed to: $target_path"
    FINAL_INSTALL_PATH="$target_path"
}

# Comprehensive installation verification
verify_installation() {
    show_progress "Verifying installation"
    
    local binary_path="$1"
    
    # Check if binary exists and is executable
    if [ ! -x "$binary_path" ]; then
        handle_error 5 "Binary is not executable: $binary_path" \
            "Try: chmod +x $binary_path"
    fi
    
    # Test binary execution
    local version_output
    if ! version_output=$("$binary_path" --version 2>&1); then
        handle_error 5 "Binary execution failed" \
            "The binary may be corrupted or incompatible with your system"
    fi
    
    log_success "Installation verified: $version_output"
    
    # Check PATH
    check_path_configuration "$binary_path"
}

# Enhanced PATH configuration check and guidance
check_path_configuration() {
    local binary_path="$1"
    local binary_dir
    binary_dir=$(dirname "$binary_path")
    
    echo
    log_info "🔍 Checking PATH configuration..."
    
    if command -v "$BINARY_NAME" >/dev/null 2>&1; then
        local which_path
        which_path=$(command -v "$BINARY_NAME")
        if [ "$which_path" = "$binary_path" ]; then
            log_success "✅ libdplyr is available in PATH and points to the new installation"
            
            # Test the command
            echo "  Testing command execution..."
            if "$BINARY_NAME" --version >/dev/null 2>&1; then
                log_success "  ✅ Command execution test passed"
            else
                log_warning "  ⚠️  Command execution test failed"
            fi
        else
            log_warning "⚠️  Different version of libdplyr found in PATH"
            echo "    Current PATH version: $which_path"
            echo "    Newly installed version: $binary_path"
            echo
            echo "  To use the new version, either:"
            echo "    1. Remove the old version: rm $which_path"
            echo "    2. Update your PATH to prioritize $binary_dir"
            echo "    3. Use the full path: $binary_path"
        fi
    else
        log_warning "⚠️  libdplyr is not in your PATH"
        provide_path_instructions "$binary_dir"
    fi
    
    # Show current PATH for debugging
    if [ "${DEBUG:-false}" = "true" ]; then
        echo
        log_debug "Current PATH directories:"
        echo "$PATH" | tr ':' '\n' | while read -r dir; do
            if [ -n "$dir" ]; then
                echo "  • $dir"
            fi
        done
    fi
}

# Enhanced PATH setup instructions with automatic configuration option
provide_path_instructions() {
    local install_dir="$1"
    
    echo
    log_info "📝 PATH Configuration Required"
    echo "To use libdplyr from anywhere, you need to add it to your PATH."
    echo
    
    # Detect shell and provide appropriate instructions
    local shell_name
    shell_name=$(basename "${SHELL:-/bin/bash}")
    local config_file=""
    local export_cmd="export PATH=\"$install_dir:\$PATH\""
    
    case "$shell_name" in
        bash)
            config_file="$HOME/.bashrc"
            if [ -f "$HOME/.bash_profile" ]; then
                config_file="$HOME/.bash_profile"
            fi
            ;;
        zsh)
            config_file="$HOME/.zshrc"
            ;;
        fish)
            export_cmd="fish_add_path $install_dir"
            ;;
        *)
            config_file="$HOME/.profile"
            ;;
    esac
    
    echo -e "${CYAN}Option 1: Automatic Configuration${CLEAR}"
    if [ "$shell_name" != "fish" ] && [ -n "$config_file" ]; then
        echo "  Run this command to automatically add libdplyr to your PATH:"
        echo -e "  ${GREEN}echo '$export_cmd' >> $config_file && source $config_file${CLEAR}"
        echo
        
        # Offer to do it automatically
        if [ -t 0 ]; then  # Check if running interactively
            echo -n "  Would you like me to do this automatically? [y/N]: "
            read -r response
            case "$response" in
                [yY]|[yY][eE][sS])
                    if echo "$export_cmd" >> "$config_file" 2>/dev/null; then
                        log_success "✅ PATH updated in $config_file"
                        log_info "Please run: source $config_file"
                        log_info "Or restart your terminal to apply changes"
                        return
                    else
                        log_error "❌ Failed to update $config_file"
                        log_info "Please add manually using the instructions below"
                    fi
                    ;;
                *)
                    log_info "Skipping automatic configuration"
                    ;;
            esac
        fi
    fi
    
    echo -e "${CYAN}Option 2: Manual Configuration${CLEAR}"
    case "$shell_name" in
        bash)
            echo "  1. Add to your shell configuration:"
            echo "     echo '$export_cmd' >> $config_file"
            echo "  2. Reload your configuration:"
            echo "     source $config_file"
            ;;
        zsh)
            echo "  1. Add to your shell configuration:"
            echo "     echo '$export_cmd' >> $config_file"
            echo "  2. Reload your configuration:"
            echo "     source $config_file"
            ;;
        fish)
            echo "  1. Add to your PATH:"
            echo "     $export_cmd"
            ;;
        *)
            echo "  1. Add to your shell configuration file:"
            echo "     echo '$export_cmd' >> $config_file"
            echo "  2. Reload your configuration or restart your terminal"
            ;;
    esac
    
    echo
    echo -e "${CYAN}Option 3: Direct Usage${CLEAR}"
    echo "  Use the full path without modifying PATH:"
    echo -e "  ${GREEN}$install_dir/$BINARY_NAME --help${CLEAR}"
    
    echo
    echo -e "${CYAN}Option 4: Temporary PATH (current session only)${CLEAR}"
    echo "  Add to PATH for this session:"
    echo -e "  ${GREEN}export PATH=\"$install_dir:\$PATH\"${CLEAR}"
    
    # Verification instructions
    echo
    log_info "🔍 Verification"
    echo "After updating your PATH, verify the installation:"
    echo "  1. Open a new terminal or run: source $config_file"
    echo "  2. Test the command: libdplyr --version"
    echo "  3. Check PATH: which libdplyr"
}

# Enhanced usage examples and next steps with installation summary
show_usage_examples() {
    echo
    echo "═══════════════════════════════════════════════════════════════"
    log_success "🎉 libdplyr Installation Completed Successfully!"
    echo "═══════════════════════════════════════════════════════════════"
    
    # Installation summary
    echo
    log_info "📋 Installation Summary:"
    echo "  • Version: $([ "$VERSION" = "latest" ] && echo "$target_version" || echo "$VERSION")"
    echo "  • Platform: $PLATFORM"
    echo "  • Location: $FINAL_INSTALL_PATH"
    echo "  • Size: $(du -h "$FINAL_INSTALL_PATH" 2>/dev/null | cut -f1 || echo "Unknown")"
    
    # Quick verification
    echo
    log_info "🔍 Quick Verification:"
    if command -v "$BINARY_NAME" >/dev/null 2>&1; then
        echo "  ✅ libdplyr is available in PATH"
        local version_info
        version_info=$("$BINARY_NAME" --version 2>/dev/null || echo "Version check failed")
        echo "  ✅ Version: $version_info"
    else
        echo "  ⚠️  libdplyr is not in PATH (see instructions above)"
        echo "  ✅ Direct access: $FINAL_INSTALL_PATH --version"
    fi
    
    # Usage examples
    echo
    log_info "🚀 Getting Started - Try These Examples:"
    echo
    
    local cmd_prefix=""
    if ! command -v "$BINARY_NAME" >/dev/null 2>&1; then
        cmd_prefix="$FINAL_INSTALL_PATH"
    else
        cmd_prefix="$BINARY_NAME"
    fi
    
    echo -e "${GREEN}  # Basic dplyr to SQL conversion${CLEAR}"
    echo "  echo \"select(name, age)\" | $cmd_prefix"
    echo
    echo -e "${GREEN}  # Complex query with chaining${CLEAR}"
    echo "  echo \"select(name, age) %>% filter(age > 18) %>% arrange(desc(age))\" | $cmd_prefix --pretty"
    echo
    echo -e "${GREEN}  # JSON output with metadata${CLEAR}"
    echo "  echo \"select(name, age)\" | $cmd_prefix --json"
    echo
    echo -e "${GREEN}  # Syntax validation only${CLEAR}"
    echo "  echo \"select(name, age)\" | $cmd_prefix --validate-only"
    echo
    echo -e "${GREEN}  # Different SQL dialects${CLEAR}"
    echo "  echo \"select(name, age)\" | $cmd_prefix --dialect mysql"
    echo "  echo \"select(name, age)\" | $cmd_prefix --dialect sqlite"
    echo
    echo -e "${GREEN}  # File processing${CLEAR}"
    echo "  $cmd_prefix -i input.R -o output.sql --pretty"
    echo
    echo -e "${GREEN}  # Verbose output for debugging${CLEAR}"
    echo "  echo \"select(name, age)\" | $cmd_prefix --verbose --debug"
    
    # Help and documentation
    echo
    log_info "📚 Help & Documentation:"
    echo "  • Command help: $cmd_prefix --help"
    echo "  • Project repository: https://github.com/$REPO"
    echo "  • Documentation: https://github.com/$REPO#readme"
    echo "  • Report issues: https://github.com/$REPO/issues"
    echo "  • Latest releases: https://github.com/$REPO/releases"
    
    # Performance tips
    echo
    log_info "⚡ Performance Tips:"
    echo "  • Use --compact for minimal output size"
    echo "  • Use --validate-only for syntax checking without conversion"
    echo "  • Process large files with: $cmd_prefix -i large_file.R -o output.sql"
    
    # Troubleshooting
    echo
    log_info "🔧 Troubleshooting:"
    echo "  • If command not found: Check PATH configuration above"
    echo "  • For permission errors: Try sudo or different install directory"
    echo "  • For syntax errors: Use --debug flag for detailed information"
    echo "  • For network issues during updates: Check proxy settings"
    
    echo
    echo "═══════════════════════════════════════════════════════════════"
    log_success "Happy coding with libdplyr! 🦀✨"
    echo "═══════════════════════════════════════════════════════════════"
}

# Installation confirmation and final verification
perform_final_verification() {
    echo
    log_info "🔍 Performing final installation verification..."
    
    local verification_passed=true
    local issues=()
    
    # Check binary exists and is executable
    if [ ! -x "$FINAL_INSTALL_PATH" ]; then
        verification_passed=false
        issues+=("Binary is not executable: $FINAL_INSTALL_PATH")
    fi
    
    # Check binary can run
    if ! "$FINAL_INSTALL_PATH" --version >/dev/null 2>&1; then
        verification_passed=false
        issues+=("Binary execution failed")
    fi
    
    # Check file size (should be reasonable)
    local file_size
    file_size=$(stat -f%z "$FINAL_INSTALL_PATH" 2>/dev/null || stat -c%s "$FINAL_INSTALL_PATH" 2>/dev/null || echo "0")
    if [ "$file_size" -lt 1000000 ]; then  # Less than 1MB seems suspicious
        verification_passed=false
        issues+=("Binary file size seems too small: $file_size bytes")
    fi
    
    # Check PATH availability
    local path_available=false
    if command -v "$BINARY_NAME" >/dev/null 2>&1; then
        local which_path
        which_path=$(command -v "$BINARY_NAME")
        if [ "$which_path" = "$FINAL_INSTALL_PATH" ]; then
            path_available=true
        fi
    fi
    
    if [ "$verification_passed" = true ]; then
        log_success "✅ All verification checks passed"
        if [ "$path_available" = true ]; then
            log_success "✅ libdplyr is available in PATH"
        else
            log_warning "⚠️  libdplyr is not in PATH (manual configuration required)"
        fi
    else
        log_error "❌ Verification failed:"
        for issue in "${issues[@]}"; do
            echo "  • $issue"
        done
        
        # Offer to retry or continue
        if [ -t 0 ]; then  # Interactive mode
            echo
            echo -n "Would you like to retry the installation? [y/N]: "
            read -r response
            case "$response" in
                [yY]|[yY][eE][sS])
                    log_info "Retrying installation..."
                    cleanup_on_error
                    exec "$0" "$@"  # Restart the script
                    ;;
                *)
                    log_warning "Continuing despite verification issues..."
                    ;;
            esac
        fi
    fi
}

# Main installation function with enhanced error recovery
main() {
    echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${CLEAR}"
    echo -e "${BLUE}║                    libdplyr Installer                        ║${CLEAR}"
    echo -e "${BLUE}║              dplyr to SQL Transpiler for Rust                ║${CLEAR}"
    echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${CLEAR}"
    echo
    
    # Show installation info
    log_info "🔧 Installation Configuration:"
    echo "  • Target directory: $INSTALL_DIR"
    echo "  • Fallback directory: $FALLBACK_INSTALL_DIR"
    echo "  • Version: $VERSION"
    echo "  • Repository: https://github.com/$REPO"
    
    # Environment info for debugging
    if [ "${DEBUG:-false}" = "true" ]; then
        echo
        log_debug "🐛 Debug Information:"
        log_debug "  INSTALL_DIR: ${INSTALL_DIR}"
        log_debug "  LIBDPLYR_VERSION: ${VERSION}"
        log_debug "  HOME: ${HOME}"
        log_debug "  SHELL: ${SHELL}"
        log_debug "  USER: $(whoami)"
        log_debug "  PWD: $(pwd)"
        log_debug "  PATH: ${PATH}"
    fi
    
    echo
    
    # Pre-installation checks
    local start_time
    start_time=$(date +%s)
    
    # Installation steps with error recovery
    check_network
    detect_platform
    check_dependencies
    
    # Determine version to install
    local target_version
    if [ "$VERSION" = "latest" ]; then
        target_version=$(get_latest_version)
    else
        target_version="$VERSION"
        log_info "Installing specified version: $target_version"
    fi
    
    # Download and install with retry logic
    local temp_binary
    local max_retries=2
    local retry_count=0
    
    while [ $retry_count -le $max_retries ]; do
        if [ $retry_count -gt 0 ]; then
            log_info "Retry attempt $retry_count of $max_retries"
            sleep 2
        fi
        
        if temp_binary=$(download_binary "$target_version" "$PLATFORM"); then
            break
        else
            retry_count=$((retry_count + 1))
            if [ $retry_count -gt $max_retries ]; then
                handle_error 1 "Failed to download after $max_retries attempts" \
                    "Try again later or download manually from GitHub releases"
            fi
        fi
    done
    
    install_binary "$temp_binary" "$INSTALL_DIR"
    verify_installation "$FINAL_INSTALL_PATH"
    perform_final_verification
    
    # Calculate installation time
    local end_time
    end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    # Success message and usage examples
    show_usage_examples
    
    echo
    log_success "⏱️  Installation completed in ${duration} seconds"
    
    # Cleanup
    trap - EXIT
    cleanup_on_error
}

# Show help information
show_help() {
    echo -e "${BLUE}libdplyr Installation Script${CLEAR}"
    echo
    echo "USAGE:"
    echo "  $0 [OPTIONS]"
    echo
    echo "OPTIONS:"
    echo "  -h, --help              Show this help message"
    echo "  -v, --version VERSION   Install specific version (e.g., v1.0.0)"
    echo "  -d, --dir DIRECTORY     Install to specific directory"
    echo "  --debug                 Enable debug output"
    echo "  --dry-run              Show what would be installed without installing"
    echo
    echo "ENVIRONMENT VARIABLES:"
    echo "  LIBDPLYR_VERSION       Version to install (default: latest)"
    echo "  INSTALL_DIR            Installation directory (default: /usr/local/bin)"
    echo "  DEBUG                  Enable debug mode (true/false)"
    echo
    echo "EXAMPLES:"
    echo "  # Install latest version"
    echo "  $0"
    echo
    echo "  # Install specific version"
    echo "  $0 --version v1.0.0"
    echo
    echo "  # Install to custom directory"
    echo "  $0 --dir \$HOME/.local/bin"
    echo
    echo "  # Install with debug output"
    echo "  $0 --debug"
    echo
    echo "  # Dry run (show what would be installed)"
    echo "  $0 --dry-run"
    echo
    echo "For more information, visit: https://github.com/$REPO"
}

# Dry run mode - show what would be installed
dry_run() {
    echo -e "${CYAN}🔍 Dry Run Mode - Showing what would be installed${CLEAR}"
    echo
    
    detect_platform
    
    local target_version
    if [ "$VERSION" = "latest" ]; then
        echo "Fetching latest version..."
        # Try to get latest version, but don't fail in dry-run mode
        if target_version=$(get_latest_version 2>/dev/null); then
            log_success "Latest version: $target_version"
        else
            target_version="latest"
            log_warning "Could not fetch latest version (network issue), using 'latest' placeholder"
        fi
    else
        target_version="$VERSION"
    fi
    
    echo
    echo -e "${GREEN}📋 Installation Plan:${CLEAR}"
    echo "  • Version: $target_version"
    echo "  • Platform: $PLATFORM"
    echo "  • Binary URL: https://github.com/$REPO/releases/download/$target_version/libdplyr-$PLATFORM"
    echo "  • Install Directory: $INSTALL_DIR"
    echo "  • Fallback Directory: $FALLBACK_INSTALL_DIR"
    echo "  • Final Path: $INSTALL_DIR/$BINARY_NAME"
    echo
    
    # Check current installation
    if command -v "$BINARY_NAME" >/dev/null 2>&1; then
        local current_version current_path
        current_path=$(command -v "$BINARY_NAME")
        current_version=$("$BINARY_NAME" --version 2>/dev/null || echo "Unknown")
        echo -e "${YELLOW}📦 Current Installation:${CLEAR}"
        echo "  • Location: $current_path"
        echo "  • Version: $current_version"
        
        if [ "$current_path" = "$INSTALL_DIR/$BINARY_NAME" ]; then
            echo "  • Status: Will be updated in place"
        else
            echo "  • Status: New installation will be separate"
        fi
        echo
    else
        echo -e "${YELLOW}📦 Current Installation:${CLEAR}"
        echo "  • Status: No existing installation found"
        echo
    fi
    
    # Check permissions
    echo -e "${BLUE}🔒 Permission Check:${CLEAR}"
    if [ -w "$INSTALL_DIR" ] 2>/dev/null || [ ! -d "$INSTALL_DIR" ] && [ -w "$(dirname "$INSTALL_DIR")" ] 2>/dev/null; then
        echo "  • Primary directory ($INSTALL_DIR): ✅ Writable"
    else
        echo "  • Primary directory ($INSTALL_DIR): ❌ Not writable"
        if [ -w "$FALLBACK_INSTALL_DIR" ] 2>/dev/null || [ ! -d "$FALLBACK_INSTALL_DIR" ] && [ -w "$(dirname "$FALLBACK_INSTALL_DIR")" ] 2>/dev/null; then
            echo "  • Fallback directory ($FALLBACK_INSTALL_DIR): ✅ Writable"
        else
            echo "  • Fallback directory ($FALLBACK_INSTALL_DIR): ❌ Not writable"
            echo "  • Note: May require sudo or different directory"
        fi
    fi
    
    echo
    echo -e "${BLUE}🚀 To proceed with installation:${CLEAR}"
    echo "  ./install.sh"
    echo
    echo -e "${BLUE}🔧 To install with custom options:${CLEAR}"
    echo "  ./install.sh --version $target_version --dir \$HOME/.local/bin"
    echo
    echo -e "${CYAN}💡 Tips:${CLEAR}"
    echo "  • Use --debug for verbose output during installation"
    echo "  • Set INSTALL_DIR environment variable for custom directory"
    echo "  • Check https://github.com/$REPO/releases for available versions"
}

# Parse command line arguments
parse_arguments() {
    while [ $# -gt 0 ]; do
        case $1 in
            -h|--help)
                show_help
                exit 0
                ;;
            -v|--version)
                if [ -n "$2" ] && [ "${2#-}" = "$2" ]; then
                    VERSION="$2"
                    shift
                else
                    log_error "Version argument required"
                    exit 2
                fi
                ;;
            -d|--dir)
                if [ -n "$2" ] && [ "${2#-}" = "$2" ]; then
                    INSTALL_DIR="$2"
                    shift
                else
                    log_error "Directory argument required"
                    exit 2
                fi
                ;;
            --debug)
                DEBUG=true
                ;;
            --dry-run)
                DRY_RUN=true
                ;;
            *)
                log_error "Unknown option: $1"
                echo "Use --help for usage information"
                exit 2
                ;;
        esac
        shift
    done
}

# Handle script interruption with graceful cleanup
handle_interruption() {
    echo
    log_warning "⚠️  Installation interrupted by user"
    
    if [ -n "$TEMP_DIR" ] && [ -d "$TEMP_DIR" ]; then
        log_info "Cleaning up temporary files..."
        cleanup_on_error
    fi
    
    echo
    log_info "To resume installation, run the script again:"
    log_info "  $0"
    
    exit 130
}

# Set up signal handlers
trap 'handle_interruption' INT TERM

# Parse command line arguments
parse_arguments "$@"

# Run in dry-run mode if requested
if [ "${DRY_RUN:-false}" = "true" ]; then
    dry_run
    exit 0
fi

# Run main installation function
main "$@"