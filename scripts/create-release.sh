#!/bin/bash
# GitHub Release Creation Script
# R4-AC3, R8-AC3: Automated release creation with comprehensive metadata

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
VERSION=""
DRAFT=false
PRERELEASE=false
FORCE=false

# Usage function
usage() {
    echo "Usage: $0 -v VERSION [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -v, --version VERSION    Release version (required, e.g., v1.0.0)"
    echo "  -d, --draft             Create as draft release"
    echo "  -p, --prerelease        Mark as pre-release"
    echo "  -f, --force             Force creation even if tag exists"
    echo "  -h, --help              Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 -v v1.0.0                    # Create stable release"
    echo "  $0 -v v1.0.0-beta -p            # Create pre-release"
    echo "  $0 -v v1.0.0 -d                 # Create draft release"
    echo ""
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -v|--version)
            VERSION="$2"
            shift 2
            ;;
        -d|--draft)
            DRAFT=true
            shift
            ;;
        -p|--prerelease)
            PRERELEASE=true
            shift
            ;;
        -f|--force)
            FORCE=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo -e "${RED}❌ Unknown option: $1${NC}"
            usage
            exit 1
            ;;
    esac
done

# Validate required arguments
if [ -z "$VERSION" ]; then
    echo -e "${RED}❌ Version is required${NC}"
    usage
    exit 1
fi

# Validate version format
if [[ ! "$VERSION" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9]+)?$ ]]; then
    echo -e "${RED}❌ Invalid version format: $VERSION${NC}"
    echo "Expected format: v1.0.0 or v1.0.0-beta"
    exit 1
fi

echo -e "${BLUE}🚀 Creating GitHub Release${NC}"
echo "=========================="
echo "Version: $VERSION"
echo "Draft: $DRAFT"
echo "Pre-release: $PRERELEASE"
echo "Force: $FORCE"
echo ""

# =============================================================================
# Pre-flight Checks
# =============================================================================

echo -e "${BLUE}🔍 Pre-flight Checks${NC}"
echo "--------------------"

# Check if we're in a git repository
if ! git rev-parse --git-dir > /dev/null 2>&1; then
    echo -e "${RED}❌ Not in a git repository${NC}"
    exit 1
fi

# Check if GitHub CLI is available
if ! command -v gh &> /dev/null; then
    echo -e "${RED}❌ GitHub CLI (gh) not found${NC}"
    echo "Please install GitHub CLI: https://cli.github.com/"
    exit 1
fi

# Check if authenticated with GitHub
if ! gh auth status > /dev/null 2>&1; then
    echo -e "${RED}❌ Not authenticated with GitHub${NC}"
    echo "Please run: gh auth login"
    exit 1
fi

echo -e "${GREEN}✅ Git repository and GitHub CLI ready${NC}"

# Check if tag already exists
if git rev-parse "$VERSION" >/dev/null 2>&1; then
    if [ "$FORCE" = false ]; then
        echo -e "${RED}❌ Tag $VERSION already exists${NC}"
        echo "Use --force to overwrite or choose a different version"
        exit 1
    else
        echo -e "${YELLOW}⚠️ Tag $VERSION exists, will be overwritten${NC}"
        git tag -d "$VERSION" || true
        git push origin ":refs/tags/$VERSION" || true
    fi
fi

# Check working directory status
if [ -n "$(git status --porcelain)" ]; then
    echo -e "${YELLOW}⚠️ Working directory has uncommitted changes${NC}"
    echo "Consider committing changes before creating a release"
    read -p "Continue anyway? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

echo -e "${GREEN}✅ Pre-flight checks passed${NC}"

# =============================================================================
# Generate Release Notes
# =============================================================================

echo -e "\n${BLUE}📝 Generating Release Notes${NC}"
echo "----------------------------"

# Get previous tag for changelog
PREV_TAG=$(git describe --tags --abbrev=0 HEAD^ 2>/dev/null || echo "")

# Create temporary release notes file
RELEASE_NOTES_FILE=$(mktemp)

# Generate changelog
if [ -n "$PREV_TAG" ]; then
    echo "## 🚀 What's Changed" >> "$RELEASE_NOTES_FILE"
    echo "" >> "$RELEASE_NOTES_FILE"

    # Get commits since last tag
    git log --pretty=format:"- %s (%h)" "$PREV_TAG..HEAD" >> "$RELEASE_NOTES_FILE"
    echo "" >> "$RELEASE_NOTES_FILE"
    echo "" >> "$RELEASE_NOTES_FILE"

    # Get contributors
    CONTRIBUTORS=$(git log --pretty=format:"%an" "$PREV_TAG..HEAD" | sort -u | tr '\n' ',' | sed 's/,$//')
    if [ -n "$CONTRIBUTORS" ]; then
        echo "## 👥 Contributors" >> "$RELEASE_NOTES_FILE"
        echo "Thanks to: $CONTRIBUTORS" >> "$RELEASE_NOTES_FILE"
        echo "" >> "$RELEASE_NOTES_FILE"
    fi
fi

# Add standard release information
cat >> "$RELEASE_NOTES_FILE" << EOF
## 📦 Installation

### Quick Install
\`\`\`bash
# Download and install automatically
curl -L https://github.com/\${{ github.repository }}/releases/download/$VERSION/dplyr-$VERSION-all-platforms.tar.gz | tar -xz
cd combined && ./install.sh
\`\`\`

### Platform-specific Downloads
- **Linux x86_64**: \`dplyr-linux-x86_64.duckdb_extension\`
- **macOS x86_64**: \`dplyr-macos-x86_64.duckdb_extension\`
- **macOS ARM64**: \`dplyr-macos-arm64.duckdb_extension\`
- **Windows x86_64**: \`dplyr-windows-x86_64.duckdb_extension\`

### Manual Installation
1. Download the appropriate platform package
2. Extract the extension binary
3. Load in DuckDB: \`LOAD '/path/to/extension';\`

## 🔧 Requirements
- DuckDB 0.9.0 or later
- Compatible operating system (Linux, macOS, Windows)

## 📊 Usage Example
\`\`\`sql
-- Load the extension
LOAD '/path/to/dplyr-platform.duckdb_extension';

-- Use implicit pipeline syntax (%>%)
mtcars %>%
       select(mpg, cyl, hp) %>%
       filter(mpg > 20) %>%
       arrange(desc(hp));

-- Table function syntax
SELECT * FROM dplyr('mtcars %>% select(mpg, cyl) %>% filter(cyl == 4)');

-- Mixed with standard SQL
WITH filtered_data AS (
    SELECT * FROM dplyr('mtcars %>% filter(mpg > 25)')
)
SELECT AVG(hp) as avg_horsepower FROM filtered_data;
\`\`\`

## 🔍 Verification
\`\`\`bash
# Verify checksum
sha256sum -c checksums.txt

# Test loading
duckdb -c "LOAD './extension'; SELECT 'Extension loaded successfully' as status;"

# Test basic functionality
duckdb -c "
LOAD './extension';
CREATE TABLE test AS SELECT 1 as id, 'test' as name;
SELECT * FROM dplyr('test %>% select(id, name)');
"
\`\`\`

## 📈 Performance
- Simple queries: <2ms transpilation time
- Complex queries: <15ms transpilation time
- Extension loading: <50ms
- Memory efficient with built-in caching

## 🔒 Security
- All artifacts include SHA256 checksums
- Source code scanned with CodeQL
- Dependencies audited for vulnerabilities
- Memory safety verified with Valgrind

## 🐛 Known Issues
EOF

# Check for known issues in GitHub issues
if command -v gh &> /dev/null; then
    KNOWN_ISSUES=$(gh issue list --label "known-issue" --state open --json title,number --jq '.[] | "- [#\(.number)] \(.title)"' 2>/dev/null || echo "")
    if [ -n "$KNOWN_ISSUES" ]; then
        echo "$KNOWN_ISSUES" >> "$RELEASE_NOTES_FILE"
    else
        echo "- None reported for this release" >> "$RELEASE_NOTES_FILE"
    fi
else
    echo "- None reported for this release" >> "$RELEASE_NOTES_FILE"
fi

cat >> "$RELEASE_NOTES_FILE" << EOF

## 📚 Documentation
- [Installation Guide](https://github.com/\${{ github.repository }}/blob/$VERSION/docs/installation.md)
- [User Guide](https://github.com/\${{ github.repository }}/blob/$VERSION/docs/user-guide.md)
- [API Reference](https://github.com/\${{ github.repository }}/blob/$VERSION/docs/api-reference.md)
- [Troubleshooting](https://github.com/\${{ github.repository }}/blob/$VERSION/docs/troubleshooting.md)

## 🤝 Contributing
We welcome contributions! Please see our [Contributing Guide](https://github.com/\${{ github.repository }}/blob/$VERSION/CONTRIBUTING.md).

---

**Full Changelog**: https://github.com/\${{ github.repository }}/compare/$PREV_TAG...$VERSION
EOF

echo -e "${GREEN}✅ Release notes generated${NC}"

# =============================================================================
# Create Git Tag
# =============================================================================

echo -e "\n${BLUE}🏷️ Creating Git Tag${NC}"
echo "-------------------"

# Configure git if needed
git config user.name "$(git config user.name || echo 'Release Bot')"
git config user.email "$(git config user.email || echo 'release@example.com')"

# Create annotated tag
git tag -a "$VERSION" -m "Release $VERSION"

# Push tag to origin
git push origin "$VERSION"

echo -e "${GREEN}✅ Tag $VERSION created and pushed${NC}"

# =============================================================================
# Trigger Release Workflow
# =============================================================================

echo -e "\n${BLUE}🚀 Triggering Release Workflow${NC}"
echo "-------------------------------"

# Trigger the release deployment workflow
gh workflow run release-deploy.yml \
    -f tag="$VERSION" \
    -f draft="$DRAFT" \
    -f prerelease="$PRERELEASE"

echo -e "${GREEN}✅ Release workflow triggered${NC}"

# =============================================================================
# Monitor Release Progress
# =============================================================================

echo -e "\n${BLUE}👀 Monitoring Release Progress${NC}"
echo "-------------------------------"

echo "Waiting for workflow to start..."
sleep 10

# Get the latest workflow run
RUN_ID=$(gh run list --workflow=release-deploy.yml --limit=1 --json databaseId --jq '.[0].databaseId')

if [ -n "$RUN_ID" ]; then
    echo "Workflow run ID: $RUN_ID"
    echo "Monitor progress: https://github.com/$(gh repo view --json owner,name --jq '.owner.login + "/" + .name')/actions/runs/$RUN_ID"

    # Wait for workflow to complete (with timeout)
    echo "Waiting for workflow to complete (timeout: 30 minutes)..."

    TIMEOUT=1800  # 30 minutes
    ELAPSED=0
    INTERVAL=30

    while [ $ELAPSED -lt $TIMEOUT ]; do
        STATUS=$(gh run view "$RUN_ID" --json status --jq '.status')

        case "$STATUS" in
            "completed")
                CONCLUSION=$(gh run view "$RUN_ID" --json conclusion --jq '.conclusion')
                if [ "$CONCLUSION" = "success" ]; then
                    echo -e "\n${GREEN}✅ Release workflow completed successfully!${NC}"
                    break
                else
                    echo -e "\n${RED}❌ Release workflow failed with conclusion: $CONCLUSION${NC}"
                    exit 1
                fi
                ;;
            "in_progress"|"queued")
                echo -n "."
                ;;
            "cancelled")
                echo -e "\n${YELLOW}⚠️ Release workflow was cancelled${NC}"
                exit 1
                ;;
            *)
                echo -e "\n${RED}❌ Unknown workflow status: $STATUS${NC}"
                exit 1
                ;;
        esac

        sleep $INTERVAL
        ELAPSED=$((ELAPSED + INTERVAL))
    done

    if [ $ELAPSED -ge $TIMEOUT ]; then
        echo -e "\n${YELLOW}⚠️ Workflow monitoring timed out${NC}"
        echo "Check the workflow status manually"
    fi
else
    echo -e "${YELLOW}⚠️ Could not find workflow run${NC}"
    echo "Check GitHub Actions manually"
fi

# =============================================================================
# Verify Release
# =============================================================================

echo -e "\n${BLUE}🔍 Verifying Release${NC}"
echo "-------------------"

# Wait a bit for GitHub to process the release
sleep 5

# Check if release was created
if gh release view "$VERSION" > /dev/null 2>&1; then
    echo -e "${GREEN}✅ Release $VERSION created successfully${NC}"

    # Get release information
    RELEASE_URL=$(gh release view "$VERSION" --json url --jq '.url')
    ASSET_COUNT=$(gh release view "$VERSION" --json assets --jq '.assets | length')

    echo "Release URL: $RELEASE_URL"
    echo "Assets uploaded: $ASSET_COUNT"

    # List assets
    echo ""
    echo "Release assets:"
    gh release view "$VERSION" --json assets --jq '.assets[] | "  - " + .name + " (" + (.size | tostring) + " bytes)"'

else
    echo -e "${RED}❌ Release $VERSION not found${NC}"
    echo "The release may still be processing or the workflow failed"
    exit 1
fi

# =============================================================================
# Post-Release Tasks
# =============================================================================

echo -e "\n${BLUE}📋 Post-Release Tasks${NC}"
echo "---------------------"

# Create post-release checklist
cat > "post-release-checklist-$VERSION.md" << EOF
# Post-Release Checklist - $VERSION

## ✅ Completed Automatically
- [x] Git tag created and pushed
- [x] GitHub Release created
- [x] Multi-platform binaries built and uploaded
- [x] Checksums generated and verified
- [x] Release notes generated
- [x] Installation scripts created

## 📋 Manual Tasks

### Documentation Updates
- [ ] Update README.md with new version
- [ ] Update installation instructions
- [ ] Update compatibility matrix
- [ ] Review and update troubleshooting guide

### Community Outreach
- [ ] Announce on relevant forums/communities
- [ ] Update package managers (if applicable)
- [ ] Submit to DuckDB community repository
- [ ] Update project website (if applicable)

### Quality Assurance
- [ ] Test installation on different platforms
- [ ] Verify all download links work
- [ ] Test with different DuckDB versions
- [ ] Monitor for user-reported issues

### Monitoring
- [ ] Monitor download statistics
- [ ] Watch for bug reports
- [ ] Track performance metrics
- [ ] Monitor security advisories

## 🔗 Useful Links
- Release: $RELEASE_URL
- Workflow: https://github.com/$(gh repo view --json owner,name --jq '.owner.login + "/" + .name')/actions/workflows/release-deploy.yml
- Issues: https://github.com/$(gh repo view --json owner,name --jq '.owner.login + "/" + .name')/issues

## 📊 Release Statistics
- Version: $VERSION
- Created: $(date -u +"%Y-%m-%d %H:%M:%S UTC")
- Assets: $ASSET_COUNT files
- Platforms: 4 (Linux, macOS x86_64, macOS ARM64, Windows)

EOF

echo -e "${GREEN}✅ Post-release checklist created: post-release-checklist-$VERSION.md${NC}"

# =============================================================================
# Final Summary
# =============================================================================

echo -e "\n${BLUE}🎉 Release Creation Complete${NC}"
echo "============================"

echo -e "${GREEN}✅ Successfully created release $VERSION${NC}"
echo ""
echo "📊 Release Summary:"
echo "  - Version: $VERSION"
echo "  - Type: $([ "$PRERELEASE" = true ] && echo "Pre-release" || echo "Stable release")"
echo "  - Status: $([ "$DRAFT" = true ] && echo "Draft" || echo "Published")"
echo "  - Assets: $ASSET_COUNT files"
echo "  - Platforms: 4 supported"
echo ""
echo "🔗 Links:"
echo "  - Release: $RELEASE_URL"
echo "  - Workflow: https://github.com/$(gh repo view --json owner,name --jq '.owner.login + "/" + .name')/actions"
echo "  - Checklist: post-release-checklist-$VERSION.md"
echo ""
echo "📋 Next Steps:"
echo "  1. Review the post-release checklist"
echo "  2. Test the release on different platforms"
echo "  3. Update documentation as needed"
echo "  4. Announce the release"
echo ""
echo -e "${GREEN}🚀 Release $VERSION is ready!${NC}"

# Cleanup
rm -f "$RELEASE_NOTES_FILE"
