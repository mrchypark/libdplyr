#!/bin/bash
# Artifact Packaging Script
# R4-AC3: Platform-specific extension binary packaging with metadata and checksums

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
VERSION="${VERSION:-$(git describe --tags --always --dirty 2>/dev/null || echo 'dev')}"
BUILD_DIR="${BUILD_DIR:-build}"
PACKAGE_DIR="${PACKAGE_DIR:-packages}"
PLATFORM="${PLATFORM:-$(uname -s | tr '[:upper:]' '[:lower:]')}"
ARCH="${ARCH:-$(uname -m)}"

# Normalize platform and architecture names
case "$PLATFORM" in
    "linux") PLATFORM="linux" ;;
    "darwin") PLATFORM="macos" ;;
    "mingw"*|"msys"*|"cygwin"*) PLATFORM="windows" ;;
esac

case "$ARCH" in
    "x86_64"|"amd64") ARCH="x86_64" ;;
    "arm64"|"aarch64") ARCH="arm64" ;;
    "i386"|"i686") ARCH="x86" ;;
esac

PLATFORM_ARCH="${PLATFORM}-${ARCH}"
EXTENSION_NAME="dplyr"

echo -e "${BLUE}📦 libdplyr Artifact Packaging${NC}"
echo "================================="
echo "Version: $VERSION"
echo "Platform: $PLATFORM_ARCH"
echo "Build Directory: $BUILD_DIR"
echo "Package Directory: $PACKAGE_DIR"
echo ""

# =============================================================================
# Validation
# =============================================================================

echo -e "${BLUE}🔍 Validating Build Artifacts${NC}"
echo "------------------------------"

# Check if build directory exists
if [ ! -d "$BUILD_DIR" ]; then
    echo -e "${RED}❌ Build directory not found: $BUILD_DIR${NC}"
    echo "Please run the build process first"
    exit 1
fi

# Determine extension file name based on platform
if [ "$PLATFORM" = "windows" ]; then
    EXTENSION_FILE="$BUILD_DIR/Release/$EXTENSION_NAME.duckdb_extension"
    if [ ! -f "$EXTENSION_FILE" ]; then
        EXTENSION_FILE="$BUILD_DIR/$EXTENSION_NAME.duckdb_extension"
    fi
else
    EXTENSION_FILE="$BUILD_DIR/$EXTENSION_NAME.duckdb_extension"
fi

# Check if extension file exists
if [ ! -f "$EXTENSION_FILE" ]; then
    echo -e "${RED}❌ Extension file not found: $EXTENSION_FILE${NC}"
    echo "Please build the extension first"
    exit 1
fi

echo -e "${GREEN}✅ Extension file found: $EXTENSION_FILE${NC}"

# Check extension file size
EXTENSION_SIZE=$(stat -c%s "$EXTENSION_FILE" 2>/dev/null || stat -f%z "$EXTENSION_FILE" 2>/dev/null || echo "0")
echo "Extension size: $(numfmt --to=iec $EXTENSION_SIZE)"

# Basic extension validation
if [ "$EXTENSION_SIZE" -lt 1000 ]; then
    echo -e "${YELLOW}⚠️ Extension file seems unusually small${NC}"
fi

# =============================================================================
# Package Directory Setup
# =============================================================================

echo -e "\n${BLUE}📁 Setting up Package Directory${NC}"
echo "--------------------------------"

# Create package directory structure
PACKAGE_ROOT="$PACKAGE_DIR/$VERSION"
PLATFORM_PACKAGE="$PACKAGE_ROOT/$PLATFORM_ARCH"

mkdir -p "$PLATFORM_PACKAGE"
echo -e "${GREEN}✅ Created package directory: $PLATFORM_PACKAGE${NC}"

# =============================================================================
# Copy and Rename Extension
# =============================================================================

echo -e "\n${BLUE}📋 Copying Extension Binary${NC}"
echo "----------------------------"

# Copy extension with platform-specific name
PACKAGED_EXTENSION="$PLATFORM_PACKAGE/$EXTENSION_NAME-$PLATFORM_ARCH.duckdb_extension"
cp "$EXTENSION_FILE" "$PACKAGED_EXTENSION"

echo -e "${GREEN}✅ Extension copied to: $PACKAGED_EXTENSION${NC}"

# =============================================================================
# Generate Metadata
# =============================================================================

echo -e "\n${BLUE}📊 Generating Metadata${NC}"
echo "-----------------------"

# Extract build information
BUILD_TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
GIT_COMMIT=$(git rev-parse HEAD 2>/dev/null || echo "unknown")
GIT_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "unknown")
GIT_TAG=$(git describe --tags --exact-match 2>/dev/null || echo "")

# Get Rust version
RUST_VERSION=$(rustc --version 2>/dev/null || echo "unknown")

# Get CMake version
CMAKE_VERSION=$(cmake --version 2>/dev/null | head -n1 || echo "unknown")

# Get DuckDB version (if available)
DUCKDB_VERSION="unknown"
if command -v duckdb &> /dev/null; then
    DUCKDB_VERSION=$(duckdb --version 2>/dev/null || echo "unknown")
fi

# Get libdplyr version from Cargo.toml
LIBDPLYR_VERSION="unknown"
if [ -f "libdplyr_c/Cargo.toml" ]; then
    LIBDPLYR_VERSION=$(grep '^version = ' libdplyr_c/Cargo.toml | head -n1 | cut -d'"' -f2 || echo "unknown")
fi

# Create comprehensive metadata
cat > "$PLATFORM_PACKAGE/metadata.json" << EOF
{
  "extension": {
    "name": "$EXTENSION_NAME",
    "version": "$VERSION",
    "platform": "$PLATFORM",
    "architecture": "$ARCH",
    "platform_arch": "$PLATFORM_ARCH",
    "filename": "$(basename "$PACKAGED_EXTENSION")",
    "size_bytes": $EXTENSION_SIZE,
    "size_human": "$(numfmt --to=iec $EXTENSION_SIZE)"
  },
  "build": {
    "timestamp": "$BUILD_TIMESTAMP",
    "git_commit": "$GIT_COMMIT",
    "git_branch": "$GIT_BRANCH",
    "git_tag": "$GIT_TAG",
    "build_type": "Release"
  },
  "versions": {
    "libdplyr": "$LIBDPLYR_VERSION",
    "rust": "$RUST_VERSION",
    "cmake": "$CMAKE_VERSION",
    "duckdb_tested": "$DUCKDB_VERSION"
  },
  "compatibility": {
    "duckdb_min_version": "0.9.0",
    "duckdb_max_version": "1.0.0",
    "abi_version": "1",
    "api_version": "1"
  },
  "features": {
    "dplyr_keywords": true,
    "table_functions": true,
    "error_handling": true,
    "caching": true,
    "debug_logging": true
  },
  "requirements": {
    "minimum_memory_mb": 64,
    "recommended_memory_mb": 256,
    "disk_space_mb": 10
  }
}
EOF

echo -e "${GREEN}✅ Metadata generated: $PLATFORM_PACKAGE/metadata.json${NC}"

# =============================================================================
# Generate Installation Instructions
# =============================================================================

echo -e "\n${BLUE}📖 Generating Installation Instructions${NC}"
echo "----------------------------------------"

cat > "$PLATFORM_PACKAGE/INSTALL.md" << EOF
# DuckDB dplyr Extension Installation

## Platform: $PLATFORM_ARCH
**Version**: $VERSION  
**Build Date**: $BUILD_TIMESTAMP

## Prerequisites

- **DuckDB**: Version 0.9.0 or later
- **Operating System**: $PLATFORM ($ARCH architecture)
- **Memory**: At least 64MB available RAM
- **Disk Space**: At least 10MB free space

## Installation Steps

### 1. Download the Extension
Download the extension file: \`$(basename "$PACKAGED_EXTENSION")\`

### 2. Verify the Download (Recommended)
\`\`\`bash
# Check file size
ls -lh $(basename "$PACKAGED_EXTENSION")

# Verify checksum (see checksums.txt)
sha256sum -c checksums.txt
\`\`\`

### 3. Load the Extension in DuckDB

#### Option A: Load from File Path
\`\`\`sql
-- Load the extension
LOAD '/path/to/$(basename "$PACKAGED_EXTENSION")';

-- Verify it loaded successfully
SELECT 'Extension loaded successfully' as status;
\`\`\`

#### Option B: Install to DuckDB Extensions Directory
\`\`\`bash
# Copy to DuckDB extensions directory (platform-specific)
EOF

# Add platform-specific installation paths
case "$PLATFORM" in
    "linux")
        cat >> "$PLATFORM_PACKAGE/INSTALL.md" << EOF
# Linux
cp $(basename "$PACKAGED_EXTENSION") ~/.duckdb/extensions/
# or system-wide
sudo cp $(basename "$PACKAGED_EXTENSION") /usr/local/lib/duckdb/extensions/
EOF
        ;;
    "macos")
        cat >> "$PLATFORM_PACKAGE/INSTALL.md" << EOF
# macOS
cp $(basename "$PACKAGED_EXTENSION") ~/Library/Application\ Support/duckdb/extensions/
# or system-wide
sudo cp $(basename "$PACKAGED_EXTENSION") /usr/local/lib/duckdb/extensions/
EOF
        ;;
    "windows")
        cat >> "$PLATFORM_PACKAGE/INSTALL.md" << EOF
# Windows
copy $(basename "$PACKAGED_EXTENSION") %APPDATA%\\duckdb\\extensions\\
# or system-wide
copy $(basename "$PACKAGED_EXTENSION") "C:\\Program Files\\duckdb\\extensions\\"
EOF
        ;;
esac

cat >> "$PLATFORM_PACKAGE/INSTALL.md" << EOF
\`\`\`

Then load with:
\`\`\`sql
LOAD '$EXTENSION_NAME';
\`\`\`

## Usage Examples

### Basic dplyr Operations
\`\`\`sql
-- Load the extension
LOAD '/path/to/$(basename "$PACKAGED_EXTENSION")';

-- Create sample data
CREATE TABLE mtcars AS 
SELECT * FROM 'https://raw.githubusercontent.com/tidyverse/dplyr/main/data-raw/mtcars.csv';

-- Use dplyr syntax
DPLYR 'mtcars %>% 
       select(mpg, cyl, hp) %>% 
       filter(mpg > 20) %>% 
       arrange(desc(hp))';
\`\`\`

### Table Function Syntax
\`\`\`sql
-- Alternative syntax using table function
SELECT * FROM dplyr('mtcars %>% 
                     select(mpg, cyl) %>% 
                     filter(cyl == 4)');
\`\`\`

### Mixed with Standard SQL
\`\`\`sql
-- Combine with standard SQL
WITH high_efficiency AS (
    SELECT * FROM dplyr('mtcars %>% filter(mpg > 25)')
)
SELECT AVG(hp) as avg_horsepower 
FROM high_efficiency;
\`\`\`

## Verification

### Test Basic Functionality
\`\`\`sql
-- Test extension loading
LOAD '/path/to/$(basename "$PACKAGED_EXTENSION")';
SELECT 'Extension loaded' as test_result;

-- Test basic dplyr operation
CREATE TABLE test_data AS SELECT 1 as id, 'test' as name;
DPLYR 'test_data %>% select(id, name)';
\`\`\`

### Performance Test
\`\`\`sql
-- Test performance with larger dataset
CREATE TABLE perf_test AS 
SELECT i as id, 'name_' || i as name, random() as value 
FROM range(1, 10000) as t(i);

-- Time a dplyr operation
.timer on
DPLYR 'perf_test %>% 
       select(id, name, value) %>% 
       filter(value > 0.5) %>% 
       arrange(desc(value)) %>% 
       limit(100)';
.timer off
\`\`\`

## Troubleshooting

### Common Issues

1. **Extension fails to load**
   - Check DuckDB version compatibility (>= 0.9.0)
   - Verify file permissions
   - Ensure correct platform/architecture

2. **"Function not found" errors**
   - Confirm extension is loaded: \`LOAD '/path/to/extension';\`
   - Check for typos in dplyr syntax

3. **Performance issues**
   - Enable caching for repeated queries
   - Check available memory
   - Consider query complexity

### Debug Mode
\`\`\`bash
# Enable debug logging
export DPLYR_DEBUG=1
duckdb your_database.db
\`\`\`

### Getting Help
- Check the error message for specific error codes (E-*)
- Review the documentation at [project repository]
- Report issues with system information and error details

## Uninstallation

To remove the extension:
1. Remove the extension file from your system
2. Restart DuckDB (extensions are loaded per session)

## Version Information

- **Extension Version**: $VERSION
- **Build Commit**: $GIT_COMMIT
- **Compatible DuckDB**: 0.9.0 - 1.0.0
- **Platform**: $PLATFORM_ARCH
- **Build Date**: $BUILD_TIMESTAMP

For more information, visit the project repository.
EOF

echo -e "${GREEN}✅ Installation guide generated: $PLATFORM_PACKAGE/INSTALL.md${NC}"

# =============================================================================
# Generate Checksums
# =============================================================================

echo -e "\n${BLUE}🔐 Generating Checksums${NC}"
echo "-----------------------"

cd "$PLATFORM_PACKAGE"

# Generate multiple hash types
echo "# Checksums for $(basename "$PACKAGED_EXTENSION")" > checksums.txt
echo "# Generated on $BUILD_TIMESTAMP" >> checksums.txt
echo "" >> checksums.txt

# SHA256
if command -v sha256sum &> /dev/null; then
    sha256sum "$(basename "$PACKAGED_EXTENSION")" >> checksums.txt
elif command -v shasum &> /dev/null; then
    shasum -a 256 "$(basename "$PACKAGED_EXTENSION")" >> checksums.txt
else
    echo "# SHA256: Not available on this system" >> checksums.txt
fi

# MD5 (for compatibility)
if command -v md5sum &> /dev/null; then
    echo "" >> checksums.txt
    echo "# MD5 (for compatibility):" >> checksums.txt
    md5sum "$(basename "$PACKAGED_EXTENSION")" >> checksums.txt
elif command -v md5 &> /dev/null; then
    echo "" >> checksums.txt
    echo "# MD5 (for compatibility):" >> checksums.txt
    md5 "$(basename "$PACKAGED_EXTENSION")" >> checksums.txt
fi

cd - > /dev/null

echo -e "${GREEN}✅ Checksums generated: $PLATFORM_PACKAGE/checksums.txt${NC}"

# =============================================================================
# Create Archive
# =============================================================================

echo -e "\n${BLUE}📦 Creating Archive${NC}"
echo "-------------------"

ARCHIVE_NAME="$EXTENSION_NAME-$VERSION-$PLATFORM_ARCH"

cd "$PACKAGE_ROOT"

# Create tar.gz archive
if command -v tar &> /dev/null; then
    tar -czf "$ARCHIVE_NAME.tar.gz" "$PLATFORM_ARCH/"
    echo -e "${GREEN}✅ Archive created: $PACKAGE_ROOT/$ARCHIVE_NAME.tar.gz${NC}"
    
    # Generate archive checksum
    if command -v sha256sum &> /dev/null; then
        sha256sum "$ARCHIVE_NAME.tar.gz" > "$ARCHIVE_NAME.tar.gz.sha256"
    elif command -v shasum &> /dev/null; then
        shasum -a 256 "$ARCHIVE_NAME.tar.gz" > "$ARCHIVE_NAME.tar.gz.sha256"
    fi
    
    echo -e "${GREEN}✅ Archive checksum: $PACKAGE_ROOT/$ARCHIVE_NAME.tar.gz.sha256${NC}"
fi

# Create zip archive (for Windows compatibility)
if command -v zip &> /dev/null; then
    zip -r "$ARCHIVE_NAME.zip" "$PLATFORM_ARCH/" > /dev/null
    echo -e "${GREEN}✅ ZIP archive created: $PACKAGE_ROOT/$ARCHIVE_NAME.zip${NC}"
    
    # Generate zip checksum
    if command -v sha256sum &> /dev/null; then
        sha256sum "$ARCHIVE_NAME.zip" > "$ARCHIVE_NAME.zip.sha256"
    elif command -v shasum &> /dev/null; then
        shasum -a 256 "$ARCHIVE_NAME.zip" > "$ARCHIVE_NAME.zip.sha256"
    fi
fi

cd - > /dev/null

# =============================================================================
# Generate Release Summary
# =============================================================================

echo -e "\n${BLUE}📋 Generating Release Summary${NC}"
echo "------------------------------"

cat > "$PACKAGE_ROOT/release-summary-$PLATFORM_ARCH.md" << EOF
# Release Summary: $EXTENSION_NAME $VERSION ($PLATFORM_ARCH)

## Build Information
- **Version**: $VERSION
- **Platform**: $PLATFORM_ARCH
- **Build Date**: $BUILD_TIMESTAMP
- **Git Commit**: $GIT_COMMIT
- **Git Branch**: $GIT_BRANCH

## Package Contents
- \`$(basename "$PACKAGED_EXTENSION")\` - Main extension binary ($EXTENSION_SIZE bytes)
- \`metadata.json\` - Detailed build and compatibility information
- \`INSTALL.md\` - Installation and usage instructions
- \`checksums.txt\` - File integrity verification

## Archives
- \`$ARCHIVE_NAME.tar.gz\` - Compressed archive (Unix/Linux)
- \`$ARCHIVE_NAME.zip\` - Compressed archive (Windows)

## Compatibility
- **DuckDB**: 0.9.0 - 1.0.0
- **Platform**: $PLATFORM ($ARCH)
- **ABI Version**: 1
- **API Version**: 1

## Features
- ✅ DPLYR keyword syntax
- ✅ Table function interface
- ✅ Error handling with codes
- ✅ Query result caching
- ✅ Debug logging support

## Installation
1. Download the appropriate archive for your platform
2. Extract the extension binary
3. Load in DuckDB: \`LOAD '/path/to/extension';\`
4. Use dplyr syntax: \`DPLYR 'data %>% select(col)';\`

## Verification
\`\`\`bash
# Verify checksum
sha256sum -c checksums.txt

# Test loading
duckdb -c "LOAD './$(basename "$PACKAGED_EXTENSION")'; SELECT 'OK' as status;"
\`\`\`

## Support
- Documentation: See INSTALL.md
- Issues: Report with platform and DuckDB version
- Debug: Set DPLYR_DEBUG=1 for verbose logging

---
Generated by libdplyr packaging system
EOF

echo -e "${GREEN}✅ Release summary: $PACKAGE_ROOT/release-summary-$PLATFORM_ARCH.md${NC}"

# =============================================================================
# Final Summary
# =============================================================================

echo -e "\n${BLUE}🎉 Packaging Complete${NC}"
echo "====================="

echo -e "${GREEN}✅ Successfully packaged $EXTENSION_NAME $VERSION for $PLATFORM_ARCH${NC}"
echo ""
echo "Package location: $PLATFORM_PACKAGE"
echo "Archive location: $PACKAGE_ROOT"
echo ""
echo "Files created:"
echo "  📦 $(basename "$PACKAGED_EXTENSION") ($(numfmt --to=iec $EXTENSION_SIZE))"
echo "  📊 metadata.json"
echo "  📖 INSTALL.md"
echo "  🔐 checksums.txt"
echo "  📋 release-summary-$PLATFORM_ARCH.md"

if [ -f "$PACKAGE_ROOT/$ARCHIVE_NAME.tar.gz" ]; then
    ARCHIVE_SIZE=$(stat -c%s "$PACKAGE_ROOT/$ARCHIVE_NAME.tar.gz" 2>/dev/null || stat -f%z "$PACKAGE_ROOT/$ARCHIVE_NAME.tar.gz" 2>/dev/null || echo "0")
    echo "  📦 $ARCHIVE_NAME.tar.gz ($(numfmt --to=iec $ARCHIVE_SIZE))"
fi

if [ -f "$PACKAGE_ROOT/$ARCHIVE_NAME.zip" ]; then
    ZIP_SIZE=$(stat -c%s "$PACKAGE_ROOT/$ARCHIVE_NAME.zip" 2>/dev/null || stat -f%z "$PACKAGE_ROOT/$ARCHIVE_NAME.zip" 2>/dev/null || echo "0")
    echo "  📦 $ARCHIVE_NAME.zip ($(numfmt --to=iec $ZIP_SIZE))"
fi

echo ""
echo "Next steps:"
echo "  1. Test the packaged extension"
echo "  2. Upload to release repository"
echo "  3. Update documentation"
echo ""
echo -e "${GREEN}🚀 Ready for distribution!${NC}"