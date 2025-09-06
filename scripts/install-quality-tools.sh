#!/bin/bash
# Install Code Quality Tools
# R7-AC4: Setup script for code quality analysis tools

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🔧 Installing Code Quality Tools${NC}"
echo "=================================="

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to install a tool
install_tool() {
    local tool_name="$1"
    local install_command="$2"
    local check_command="${3:-$1}"
    
    echo -e "\n${BLUE}Installing $tool_name...${NC}"
    
    if command_exists "$check_command"; then
        echo -e "${GREEN}✅ $tool_name is already installed${NC}"
        return 0
    fi
    
    echo "Running: $install_command"
    if eval "$install_command"; then
        echo -e "${GREEN}✅ $tool_name installed successfully${NC}"
    else
        echo -e "${RED}❌ Failed to install $tool_name${NC}"
        return 1
    fi
}

# =============================================================================
# System Dependencies
# =============================================================================

echo -e "\n${BLUE}📦 Installing System Dependencies${NC}"
echo "-----------------------------------"

# Detect OS
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    echo "Detected Linux system"
    
    # Update package list
    sudo apt-get update
    
    # Install build tools
    sudo apt-get install -y \
        build-essential \
        cmake \
        pkg-config \
        libssl-dev \
        unzip \
        curl \
        git
    
    # Install analysis tools
    sudo apt-get install -y \
        valgrind \
        cppcheck \
        clang-tidy \
        llvm \
        bc
    
    echo -e "${GREEN}✅ Linux system dependencies installed${NC}"
    
elif [[ "$OSTYPE" == "darwin"* ]]; then
    echo "Detected macOS system"
    
    # Check if Homebrew is installed
    if ! command_exists brew; then
        echo "Installing Homebrew..."
        /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    fi
    
    # Install dependencies
    brew install \
        cmake \
        pkg-config \
        openssl \
        cppcheck \
        llvm \
        bc
    
    echo -e "${GREEN}✅ macOS system dependencies installed${NC}"
    
else
    echo -e "${YELLOW}⚠️ Unsupported OS: $OSTYPE${NC}"
    echo "Please install dependencies manually:"
    echo "  - build-essential/Xcode"
    echo "  - cmake"
    echo "  - pkg-config"
    echo "  - cppcheck"
    echo "  - valgrind (Linux only)"
fi

# =============================================================================
# Rust Tools
# =============================================================================

echo -e "\n${BLUE}🦀 Installing Rust Quality Tools${NC}"
echo "----------------------------------"

# Check if Rust is installed
if ! command_exists rustc; then
    echo -e "${RED}❌ Rust is not installed${NC}"
    echo "Please install Rust from https://rustup.rs/"
    exit 1
fi

echo "Rust version: $(rustc --version)"

# Install Rust components
rustup component add rustfmt clippy llvm-tools-preview

# Install Cargo tools
install_tool "cargo-audit" "cargo install cargo-audit"
install_tool "cargo-deny" "cargo install cargo-deny"
install_tool "cargo-geiger" "cargo install cargo-geiger"
install_tool "cargo-llvm-cov" "cargo install cargo-llvm-cov"
install_tool "cargo-outdated" "cargo install cargo-outdated"

# Install criterion for benchmarking (dev dependency, but useful for local testing)
install_tool "cargo-criterion" "cargo install cargo-criterion" "cargo-criterion"

echo -e "${GREEN}✅ Rust quality tools installed${NC}"

# =============================================================================
# Additional Tools
# =============================================================================

echo -e "\n${BLUE}🔍 Installing Additional Analysis Tools${NC}"
echo "----------------------------------------"

# Install codecov uploader (optional)
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    if ! command_exists codecov; then
        echo "Installing Codecov uploader..."
        curl -Os https://uploader.codecov.io/latest/linux/codecov
        chmod +x codecov
        sudo mv codecov /usr/local/bin/
        echo -e "${GREEN}✅ Codecov uploader installed${NC}"
    else
        echo -e "${GREEN}✅ Codecov uploader already installed${NC}"
    fi
fi

# =============================================================================
# Verification
# =============================================================================

echo -e "\n${BLUE}✅ Verifying Installation${NC}"
echo "----------------------------"

# List of tools to verify
tools=(
    "rustc:Rust compiler"
    "cargo:Cargo package manager"
    "rustfmt:Rust formatter"
    "clippy:Rust linter"
    "cargo-audit:Security auditor"
    "cargo-deny:Dependency checker"
    "cargo-geiger:Unsafe code detector"
    "cargo-llvm-cov:Coverage tool"
    "cmake:Build system"
    "cppcheck:C++ static analyzer"
)

# Add platform-specific tools
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    tools+=("valgrind:Memory analyzer")
    tools+=("clang-tidy:C++ linter")
fi

echo "Checking installed tools:"
echo ""

all_good=true
for tool_info in "${tools[@]}"; do
    IFS=':' read -r tool_cmd tool_desc <<< "$tool_info"
    
    if command_exists "$tool_cmd"; then
        echo -e "  ✅ $tool_desc ($tool_cmd)"
    else
        echo -e "  ❌ $tool_desc ($tool_cmd) - NOT FOUND"
        all_good=false
    fi
done

echo ""

if $all_good; then
    echo -e "${GREEN}🎉 All quality tools installed successfully!${NC}"
    echo ""
    echo "You can now run:"
    echo "  ./scripts/quality-check.sh    # Run all quality checks"
    echo "  cargo fmt                     # Format Rust code"
    echo "  cargo clippy                  # Lint Rust code"
    echo "  cargo test                    # Run tests"
    echo "  cargo bench                   # Run benchmarks"
    echo "  cargo audit                   # Security audit"
    echo "  cargo llvm-cov                # Code coverage"
    echo ""
else
    echo -e "${YELLOW}⚠️ Some tools are missing${NC}"
    echo "Please install the missing tools manually or check the installation logs."
    echo ""
fi

# =============================================================================
# Configuration Files Check
# =============================================================================

echo -e "${BLUE}📋 Checking Configuration Files${NC}"
echo "--------------------------------"

config_files=(
    ".clang-tidy:C++ linter config"
    ".cppcheck:C++ static analyzer config"
    "codecov.yml:Code coverage config"
    "libdplyr_c/deny.toml:Dependency policy config"
)

for config_info in "${config_files[@]}"; do
    IFS=':' read -r config_file config_desc <<< "$config_info"
    
    if [ -f "$config_file" ]; then
        echo -e "  ✅ $config_desc ($config_file)"
    else
        echo -e "  ❌ $config_desc ($config_file) - NOT FOUND"
    fi
done

echo ""
echo -e "${GREEN}✅ Quality tools installation completed!${NC}"