#!/bin/bash
# Multi-platform Artifact Packaging Script
# R4-AC3: Package artifacts for all supported platforms

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
VERSION="${VERSION:-$(git describe --tags --always --dirty 2>/dev/null || echo 'dev')}"
PACKAGE_DIR="${PACKAGE_DIR:-packages}"
EXTENSION_NAME="dplyr"

# Supported platforms
PLATFORMS=(
    "linux-x86_64"
    "macos-x86_64"
    "macos-arm64"
    "windows-x86_64"
)

echo -e "${BLUE}🌍 Multi-platform Artifact Packaging${NC}"
echo "====================================="
echo "Version: $VERSION"
echo "Extension: $EXTENSION_NAME"
echo "Package Directory: $PACKAGE_DIR"
echo ""

# =============================================================================
# Platform Detection and Validation
# =============================================================================

echo -e "${BLUE}🔍 Platform Detection${NC}"
echo "---------------------"

CURRENT_OS=$(uname -s | tr '[:upper:]' '[:lower:]')
CURRENT_ARCH=$(uname -m)

case "$CURRENT_OS" in
    "linux") CURRENT_PLATFORM="linux" ;;
    "darwin") CURRENT_PLATFORM="macos" ;;
    "mingw"*|"msys"*|"cygwin"*) CURRENT_PLATFORM="windows" ;;
    *) CURRENT_PLATFORM="unknown" ;;
esac

case "$CURRENT_ARCH" in
    "x86_64"|"amd64") CURRENT_ARCH="x86_64" ;;
    "arm64"|"aarch64") CURRENT_ARCH="arm64" ;;
    *) CURRENT_ARCH="unknown" ;;
esac

CURRENT_PLATFORM_ARCH="${CURRENT_PLATFORM}-${CURRENT_ARCH}"

echo "Current platform: $CURRENT_PLATFORM_ARCH"
echo ""

# =============================================================================
# Check Available Build Artifacts
# =============================================================================

echo -e "${BLUE}📦 Checking Available Build Artifacts${NC}"
echo "--------------------------------------"

AVAILABLE_PLATFORMS=()
MISSING_PLATFORMS=()

for platform in "${PLATFORMS[@]}"; do
    # Map platform to expected build directory structure
    case "$platform" in
        "linux-x86_64")
            BUILD_DIRS=("build" "build-linux" "build-linux-x86_64")
            ;;
        "macos-x86_64")
            BUILD_DIRS=("build" "build-macos" "build-macos-x86_64")
            ;;
        "macos-arm64")
            BUILD_DIRS=("build" "build-macos" "build-macos-arm64")
            ;;
        "windows-x86_64")
            BUILD_DIRS=("build" "build-windows" "build-windows-x86_64")
            ;;
    esac

    FOUND=false
    for build_dir in "${BUILD_DIRS[@]}"; do
        # Check for extension file
        if [ "$platform" = "windows-x86_64" ]; then
            EXTENSION_PATHS=(
                "$build_dir/Release/$EXTENSION_NAME.duckdb_extension"
                "$build_dir/$EXTENSION_NAME.duckdb_extension"
            )
        else
            EXTENSION_PATHS=(
                "$build_dir/$EXTENSION_NAME.duckdb_extension"
            )
        fi

        for ext_path in "${EXTENSION_PATHS[@]}"; do
            if [ -f "$ext_path" ]; then
                echo -e "  ${GREEN}✅ $platform: $ext_path${NC}"
                AVAILABLE_PLATFORMS+=("$platform:$build_dir")
                FOUND=true
                break 2
            fi
        done
    done

    if [ "$FOUND" = false ]; then
        echo -e "  ${RED}❌ $platform: No build artifacts found${NC}"
        MISSING_PLATFORMS+=("$platform")
    fi
done

echo ""

if [ ${#MISSING_PLATFORMS[@]} -gt 0 ]; then
    echo -e "${YELLOW}⚠️ Missing build artifacts for:${NC}"
    for platform in "${MISSING_PLATFORMS[@]}"; do
        echo "  - $platform"
    done
    echo ""
    echo "To build missing platforms:"
    echo "  1. Set up cross-compilation environment"
    echo "  2. Run platform-specific builds"
    echo "  3. Or use CI/CD to build all platforms"
    echo ""
fi

if [ ${#AVAILABLE_PLATFORMS[@]} -eq 0 ]; then
    echo -e "${RED}❌ No build artifacts found for any platform${NC}"
    echo "Please build the extension first"
    exit 1
fi

# =============================================================================
# Package Available Platforms
# =============================================================================

echo -e "${BLUE}📦 Packaging Available Platforms${NC}"
echo "---------------------------------"

PACKAGED_PLATFORMS=()
FAILED_PLATFORMS=()

for platform_info in "${AVAILABLE_PLATFORMS[@]}"; do
    IFS=':' read -r platform build_dir <<< "$platform_info"

    echo -e "\n${BLUE}Packaging $platform...${NC}"

    # Set environment variables for the packaging script
    export PLATFORM_OVERRIDE="$platform"
    export BUILD_DIR="$build_dir"
    export VERSION="$VERSION"
    export PACKAGE_DIR="$PACKAGE_DIR"

    # Run platform-specific packaging
    if ./scripts/package-artifacts.sh; then
        echo -e "${GREEN}✅ Successfully packaged $platform${NC}"
        PACKAGED_PLATFORMS+=("$platform")
    else
        echo -e "${RED}❌ Failed to package $platform${NC}"
        FAILED_PLATFORMS+=("$platform")
    fi
done

# =============================================================================
# Create Combined Release Package
# =============================================================================

echo -e "\n${BLUE}🎁 Creating Combined Release Package${NC}"
echo "------------------------------------"

RELEASE_DIR="$PACKAGE_DIR/$VERSION"
COMBINED_PACKAGE="$RELEASE_DIR/combined"

mkdir -p "$COMBINED_PACKAGE"

# Copy all platform packages to combined directory
for platform in "${PACKAGED_PLATFORMS[@]}"; do
    PLATFORM_DIR="$RELEASE_DIR/$platform"
    if [ -d "$PLATFORM_DIR" ]; then
        cp -r "$PLATFORM_DIR" "$COMBINED_PACKAGE/"
        echo -e "${GREEN}✅ Added $platform to combined package${NC}"
    fi
done

# =============================================================================
# Generate Combined Metadata
# =============================================================================

echo -e "\n${BLUE}📊 Generating Combined Metadata${NC}"
echo "--------------------------------"

BUILD_TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
GIT_COMMIT=$(git rev-parse HEAD 2>/dev/null || echo "unknown")
GIT_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "unknown")

# Create combined release metadata
cat > "$COMBINED_PACKAGE/release-metadata.json" << EOF
{
  "release": {
    "version": "$VERSION",
    "extension_name": "$EXTENSION_NAME",
    "build_timestamp": "$BUILD_TIMESTAMP",
    "git_commit": "$GIT_COMMIT",
    "git_branch": "$GIT_BRANCH"
  },
  "platforms": {
EOF

# Add platform information
FIRST=true
for platform in "${PACKAGED_PLATFORMS[@]}"; do
    if [ "$FIRST" = false ]; then
        echo "," >> "$COMBINED_PACKAGE/release-metadata.json"
    fi
    FIRST=false

    # Extract platform and arch
    IFS='-' read -r plat arch <<< "$platform"

    cat >> "$COMBINED_PACKAGE/release-metadata.json" << EOF
    "$platform": {
      "platform": "$plat",
      "architecture": "$arch",
      "extension_file": "$EXTENSION_NAME-$platform.duckdb_extension",
      "available": true
    }EOF
done

# Add missing platforms
for platform in "${MISSING_PLATFORMS[@]}"; do
    if [ "$FIRST" = false ]; then
        echo "," >> "$COMBINED_PACKAGE/release-metadata.json"
    fi
    FIRST=false

    IFS='-' read -r plat arch <<< "$platform"

    cat >> "$COMBINED_PACKAGE/release-metadata.json" << EOF
    "$platform": {
      "platform": "$plat",
      "architecture": "$arch",
      "extension_file": "$EXTENSION_NAME-$platform.duckdb_extension",
      "available": false,
      "reason": "Build artifacts not found"
    }EOF
done

cat >> "$COMBINED_PACKAGE/release-metadata.json" << EOF

  },
  "compatibility": {
    "duckdb_min_version": "0.9.0",
    "duckdb_max_version": "1.0.0",
    "abi_version": "1",
    "api_version": "1"
  },
  "statistics": {
    "total_platforms": ${#PLATFORMS[@]},
    "packaged_platforms": ${#PACKAGED_PLATFORMS[@]},
    "missing_platforms": ${#MISSING_PLATFORMS[@]},
    "success_rate": "$(( ${#PACKAGED_PLATFORMS[@]} * 100 / ${#PLATFORMS[@]} ))%"
  }
}
EOF

echo -e "${GREEN}✅ Combined metadata: $COMBINED_PACKAGE/release-metadata.json${NC}"

# =============================================================================
# Generate Installation Script
# =============================================================================

echo -e "\n${BLUE}📜 Generating Installation Script${NC}"
echo "----------------------------------"

cat > "$COMBINED_PACKAGE/install.sh" << 'EOF'
#!/bin/bash
# DuckDB dplyr Extension Auto-installer
# Automatically detects platform and installs the appropriate extension

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}🚀 DuckDB dplyr Extension Installer${NC}"
echo "===================================="

# Detect platform
PLATFORM=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$PLATFORM" in
    "linux") PLATFORM="linux" ;;
    "darwin") PLATFORM="macos" ;;
    *) echo -e "${RED}❌ Unsupported platform: $PLATFORM${NC}"; exit 1 ;;
esac

case "$ARCH" in
    "x86_64"|"amd64") ARCH="x86_64" ;;
    "arm64"|"aarch64") ARCH="arm64" ;;
    *) echo -e "${RED}❌ Unsupported architecture: $ARCH${NC}"; exit 1 ;;
esac

PLATFORM_ARCH="${PLATFORM}-${ARCH}"
EXTENSION_FILE="dplyr-${PLATFORM_ARCH}.duckdb_extension"

echo "Detected platform: $PLATFORM_ARCH"
echo "Extension file: $EXTENSION_FILE"
echo ""

# Check if extension file exists
if [ ! -f "$PLATFORM_ARCH/$EXTENSION_FILE" ]; then
    echo -e "${RED}❌ Extension not found for platform $PLATFORM_ARCH${NC}"
    echo "Available platforms:"
    for dir in */; do
        if [ -d "$dir" ] && [[ "$dir" != "combined/" ]]; then
            echo "  - ${dir%/}"
        fi
    done
    exit 1
fi

# Install extension
INSTALL_DIR="$HOME/.duckdb/extensions"
mkdir -p "$INSTALL_DIR"

echo "Installing extension to $INSTALL_DIR..."
cp "$PLATFORM_ARCH/$EXTENSION_FILE" "$INSTALL_DIR/"

echo -e "${GREEN}✅ Extension installed successfully!${NC}"
echo ""
echo "To use the extension in DuckDB:"
echo "  1. Start DuckDB: duckdb"
echo "  2. Load extension: LOAD 'dplyr';"
echo "  3. Example: SELECT * FROM dplyr('data %>% select(col)');"
echo ""
echo "For more information, see the INSTALL.md file in the platform directory."
EOF

chmod +x "$COMBINED_PACKAGE/install.sh"

# Windows installer
cat > "$COMBINED_PACKAGE/install.bat" << 'EOF'
@echo off
REM DuckDB dplyr Extension Auto-installer (Windows)

echo 🚀 DuckDB dplyr Extension Installer
echo ====================================

set PLATFORM=windows
set ARCH=x86_64
set PLATFORM_ARCH=%PLATFORM%-%ARCH%
set EXTENSION_FILE=dplyr-%PLATFORM_ARCH%.duckdb_extension

echo Detected platform: %PLATFORM_ARCH%
echo Extension file: %EXTENSION_FILE%
echo.

REM Check if extension file exists
if not exist "%PLATFORM_ARCH%\%EXTENSION_FILE%" (
    echo ❌ Extension not found for platform %PLATFORM_ARCH%
    echo Available platforms:
    for /d %%d in (*) do (
        if not "%%d"=="combined" echo   - %%d
    )
    exit /b 1
)

REM Install extension
set INSTALL_DIR=%APPDATA%\duckdb\extensions
if not exist "%INSTALL_DIR%" mkdir "%INSTALL_DIR%"

echo Installing extension to %INSTALL_DIR%...
copy "%PLATFORM_ARCH%\%EXTENSION_FILE%" "%INSTALL_DIR%\" >nul

echo ✅ Extension installed successfully!
echo.
echo To use the extension in DuckDB:
echo   1. Start DuckDB: duckdb
echo   2. Load extension: LOAD 'dplyr';
echo   3. Example: SELECT * FROM dplyr('data %%^>%% select^(col^)');
echo.
echo For more information, see the INSTALL.md file in the platform directory.
EOF

echo -e "${GREEN}✅ Installation scripts: install.sh, install.bat${NC}"

# =============================================================================
# Create Combined Archives
# =============================================================================

echo -e "\n${BLUE}📦 Creating Combined Archives${NC}"
echo "------------------------------"

cd "$RELEASE_DIR"

# Create comprehensive archive with all platforms
ARCHIVE_NAME="$EXTENSION_NAME-$VERSION-all-platforms"

if command -v tar &> /dev/null; then
    tar -czf "$ARCHIVE_NAME.tar.gz" combined/
    echo -e "${GREEN}✅ Combined archive: $ARCHIVE_NAME.tar.gz${NC}"

    # Generate checksum
    if command -v sha256sum &> /dev/null; then
        sha256sum "$ARCHIVE_NAME.tar.gz" > "$ARCHIVE_NAME.tar.gz.sha256"
    elif command -v shasum &> /dev/null; then
        shasum -a 256 "$ARCHIVE_NAME.tar.gz" > "$ARCHIVE_NAME.tar.gz.sha256"
    fi
fi

if command -v zip &> /dev/null; then
    zip -r "$ARCHIVE_NAME.zip" combined/ > /dev/null
    echo -e "${GREEN}✅ Combined ZIP: $ARCHIVE_NAME.zip${NC}"

    # Generate checksum
    if command -v sha256sum &> /dev/null; then
        sha256sum "$ARCHIVE_NAME.zip" > "$ARCHIVE_NAME.zip.sha256"
    elif command -v shasum &> /dev/null; then
        shasum -a 256 "$ARCHIVE_NAME.zip" > "$ARCHIVE_NAME.zip.sha256"
    fi
fi

cd - > /dev/null

# =============================================================================
# Generate Release Notes
# =============================================================================

echo -e "\n${BLUE}📝 Generating Release Notes${NC}"
echo "----------------------------"

cat > "$RELEASE_DIR/RELEASE_NOTES.md" << EOF
# DuckDB dplyr Extension Release $VERSION

## 🚀 Release Information

- **Version**: $VERSION
- **Release Date**: $BUILD_TIMESTAMP
- **Git Commit**: $GIT_COMMIT
- **Git Branch**: $GIT_BRANCH

## 📦 Available Platforms

EOF

for platform in "${PACKAGED_PLATFORMS[@]}"; do
    echo "- ✅ **$platform**: Ready for download" >> "$RELEASE_DIR/RELEASE_NOTES.md"
done

if [ ${#MISSING_PLATFORMS[@]} -gt 0 ]; then
    echo "" >> "$RELEASE_DIR/RELEASE_NOTES.md"
    echo "### Missing Platforms" >> "$RELEASE_DIR/RELEASE_NOTES.md"
    for platform in "${MISSING_PLATFORMS[@]}"; do
        echo "- ❌ **$platform**: Build artifacts not available" >> "$RELEASE_DIR/RELEASE_NOTES.md"
    done
fi

cat >> "$RELEASE_DIR/RELEASE_NOTES.md" << EOF

## 📥 Download Options

### Individual Platforms
Each platform has its own directory with:
- Extension binary (\`.duckdb_extension\`)
- Installation guide (\`INSTALL.md\`)
- Metadata (\`metadata.json\`)
- Checksums (\`checksums.txt\`)

### Combined Package
- \`$ARCHIVE_NAME.tar.gz\` - All platforms (Unix/Linux)
- \`$ARCHIVE_NAME.zip\` - All platforms (Windows)

## 🔧 Quick Installation

### Automatic Installation
\`\`\`bash
# Download and extract the combined package
# Run the installer
./combined/install.sh    # Linux/macOS
# or
combined\\install.bat    # Windows
\`\`\`

### Manual Installation
1. Download the appropriate platform package
2. Extract the extension binary
3. Load in DuckDB: \`LOAD '/path/to/extension';\`

## 📊 Package Statistics

- **Total Platforms**: ${#PLATFORMS[@]}
- **Successfully Packaged**: ${#PACKAGED_PLATFORMS[@]}
- **Missing**: ${#MISSING_PLATFORMS[@]}
- **Success Rate**: $(( ${#PACKAGED_PLATFORMS[@]} * 100 / ${#PLATFORMS[@]} ))%

## 🔍 Verification

Each package includes checksums for integrity verification:
\`\`\`bash
# Verify individual extension
sha256sum -c platform/checksums.txt

# Verify combined archive
sha256sum -c $ARCHIVE_NAME.tar.gz.sha256
\`\`\`

## 📚 Documentation

- See individual \`INSTALL.md\` files for platform-specific instructions
- Check \`metadata.json\` for detailed build information
- Visit the project repository for complete documentation

## 🐛 Issues and Support

If you encounter issues:
1. Check the platform-specific \`INSTALL.md\`
2. Verify DuckDB version compatibility (>= 0.9.0)
3. Enable debug logging: \`export DPLYR_DEBUG=1\`
4. Report issues with platform and version information

---

Generated by libdplyr multi-platform packaging system
EOF

echo -e "${GREEN}✅ Release notes: $RELEASE_DIR/RELEASE_NOTES.md${NC}"

# =============================================================================
# Final Summary
# =============================================================================

echo -e "\n${BLUE}🎉 Multi-platform Packaging Complete${NC}"
echo "====================================="

echo -e "${GREEN}✅ Successfully packaged $EXTENSION_NAME $VERSION${NC}"
echo ""
echo "📊 Summary:"
echo "  Total platforms: ${#PLATFORMS[@]}"
echo "  Successfully packaged: ${#PACKAGED_PLATFORMS[@]}"
echo "  Missing: ${#MISSING_PLATFORMS[@]}"
echo "  Success rate: $(( ${#PACKAGED_PLATFORMS[@]} * 100 / ${#PLATFORMS[@]} ))%"
echo ""

if [ ${#PACKAGED_PLATFORMS[@]} -gt 0 ]; then
    echo "✅ Packaged platforms:"
    for platform in "${PACKAGED_PLATFORMS[@]}"; do
        echo "  - $platform"
    done
    echo ""
fi

if [ ${#FAILED_PLATFORMS[@]} -gt 0 ]; then
    echo "❌ Failed platforms:"
    for platform in "${FAILED_PLATFORMS[@]}"; do
        echo "  - $platform"
    done
    echo ""
fi

echo "📦 Release artifacts:"
echo "  - Individual platform packages: $RELEASE_DIR/[platform]/"
echo "  - Combined package: $RELEASE_DIR/combined/"
echo "  - Archives: $RELEASE_DIR/$ARCHIVE_NAME.*"
echo "  - Release notes: $RELEASE_DIR/RELEASE_NOTES.md"
echo ""

echo "🚀 Ready for distribution!"
echo ""
echo "Next steps:"
echo "  1. Test the packaged extensions"
echo "  2. Upload to GitHub Releases"
echo "  3. Update documentation"
echo "  4. Announce the release"
