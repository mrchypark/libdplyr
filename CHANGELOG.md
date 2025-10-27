# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2025-10-27

### Fixed
- Fixed CLI build errors by correcting format string syntax in error messages
- Updated Rust format strings to use positional arguments instead of field access

### Added
- GitHub Actions CI/CD pipeline with comprehensive testing
- Cross-platform installation scripts (Linux, macOS, Windows)
- Security auditing with cargo-audit and cargo-deny
- Performance benchmarking with automatic regression detection
- Code coverage reporting with codecov integration
- Multi-platform release binaries (x86_64, ARM64)
- Installation script testing workflow

### Changed
- All messages and comments converted to English for open source compatibility
- Enhanced error handling with specific exit codes
- Improved installation script with better error messages and fallback options

### Security
- Added cargo-deny configuration for dependency security scanning
- Implemented shellcheck for installation script security validation
- Added security audit workflow in CI pipeline

## [0.1.0] - 2024-01-XX

### Added
- Initial release of libdplyr
- Core dplyr syntax support (select, filter, mutate, arrange, group_by, summarise)
- Multiple SQL dialect support (PostgreSQL, MySQL, SQLite, DuckDB)
- Command-line interface with various output formats
- Rust library API for integration
- Comprehensive test suite
- Performance benchmarks
- Documentation and usage examples

### Features
- Pipeline operations with `%>%` operator
- Aggregate functions (mean, sum, count, min, max)
- Complex expression support
- Error handling with detailed messages
- JSON output format for programmatic use
- Validation-only mode for syntax checking

### Supported Platforms
- Linux (x86_64, ARM64)
- macOS (Intel, Apple Silicon)
- Windows (x86_64, ARM64)

---

## Release Notes Template

When creating a new release, use this template:

```markdown
## [X.Y.Z] - YYYY-MM-DD

### Added
- New features and functionality

### Changed
- Changes to existing functionality

### Deprecated
- Features that will be removed in future versions

### Removed
- Features removed in this version

### Fixed
- Bug fixes

### Security
- Security improvements and fixes

### Performance
- Performance improvements and optimizations

### Breaking Changes
- Changes that break backward compatibility
```

## Versioning Strategy

This project follows [Semantic Versioning](https://semver.org/):

- **MAJOR** version when you make incompatible API changes
- **MINOR** version when you add functionality in a backwards compatible manner
- **PATCH** version when you make backwards compatible bug fixes

### Version Categories

- **0.x.x**: Pre-1.0 development versions
- **1.x.x**: Stable API versions
- **x.0.0**: Major releases with potential breaking changes
- **x.y.0**: Minor releases with new features
- **x.y.z**: Patch releases with bug fixes

## Release Process

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md` with release notes
3. Create and push git tag: `git tag -a v1.0.0 -m "Release v1.0.0"`
4. GitHub Actions will automatically:
   - Build release binaries for all platforms
   - Create GitHub release with assets
   - Publish to crates.io
   - Update installation scripts with new version
