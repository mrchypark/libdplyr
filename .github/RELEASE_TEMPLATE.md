# Release Template for libdplyr

## Pre-release Checklist

- [ ] Update version in `Cargo.toml`
- [ ] Update version in `install.sh`
- [ ] Update version in `install.ps1`
- [ ] Update `CHANGELOG.md` with new features and fixes
- [ ] Run all tests locally: `cargo test --all-features`
- [ ] Run cross-platform tests: `cargo test --test cross_platform_tests`
- [ ] Run benchmarks: `cargo bench`
- [ ] Test installation scripts locally
- [ ] Update documentation if needed

## Release Process

1. **Create and push tag:**
   ```bash
   git tag -a v0.1.0 -m "Release v0.1.0"
   git push origin v0.1.0
   ```

2. **Monitor GitHub Actions:**
   - Check that all build jobs complete successfully
   - Verify that binaries are uploaded to the release
   - Confirm checksums are generated

3. **Test installation:**
   ```bash
   # Test Linux/macOS installation
   curl -sSL https://raw.githubusercontent.com/mrchyaprk/libdplyr/main/install.sh | bash
   
   # Test Windows installation (PowerShell)
   Irm https://raw.githubusercontent.com/mrchyaprk/libdplyr/main/install.ps1 | iex
   ```

4. **Verify release:**
   - Download and test binaries manually
   - Check that all platforms are included
   - Verify checksums match

## Post-release Tasks

- [ ] Announce release on relevant channels
- [ ] Update documentation website (if applicable)
- [ ] Create GitHub discussion for release feedback
- [ ] Monitor for any immediate issues

## Release Notes Template

```markdown
## What's New in v0.1.0

### 🚀 New Features
- Feature 1 description
- Feature 2 description

### 🐛 Bug Fixes
- Fix 1 description
- Fix 2 description

### 🔧 Improvements
- Improvement 1 description
- Improvement 2 description

### 📚 Documentation
- Documentation update 1
- Documentation update 2

### 🏗️ Internal Changes
- Internal change 1
- Internal change 2

## Installation

### Quick Install

**Linux/macOS:**
```bash
curl -sSL https://raw.githubusercontent.com/mrchyaprk/libdplyr/main/install.sh | bash
```

**Windows (PowerShell):**
```powershell
Irm https://raw.githubusercontent.com/mrchyaprk/libdplyr/main/install.ps1 | iex
```

### Manual Download

Download the appropriate binary for your platform from the assets below.

## Supported Platforms

- Linux x86_64
- Linux ARM64 (aarch64)
- macOS Intel (x86_64)
- macOS Apple Silicon (ARM64)
- Windows x86_64

## Checksums

All binaries include SHA256 checksums for verification. Download `checksums.txt` and verify with:

```bash
sha256sum -c checksums.txt
```

## Full Changelog

**Full Changelog**: https://github.com/mrchyaprk/libdplyr/compare/v0.0.1...v0.1.0
```

## Troubleshooting Release Issues

### Build Failures
- Check that all dependencies are properly specified
- Verify cross-compilation tools are installed
- Ensure all tests pass on the target platform

### Upload Failures
- Check GitHub token permissions
- Verify release was created successfully
- Ensure asset names don't conflict

### Installation Script Issues
- Test scripts on clean systems
- Verify download URLs are correct
- Check that binaries are executable after download