# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial implementation of libdplyr transpiler
- Support for basic dplyr operations (select, filter, mutate, summarise, arrange, group_by)
- Multiple SQL dialect support (PostgreSQL, MySQL, SQLite, DuckDB)
- Command-line interface with stdin/stdout support
- JSON output format with metadata
- Validation-only mode for syntax checking
- Verbose and debug output modes
- Cross-platform signal handling
- Automatic installation scripts for Linux, macOS, and Windows
- Comprehensive test suite with cross-platform compatibility
- Performance benchmarks and optimization
- GitHub Actions CI/CD pipeline

### Changed
- N/A (initial release)

### Deprecated
- N/A (initial release)

### Removed
- N/A (initial release)

### Fixed
- N/A (initial release)

### Security
- N/A (initial release)

## [0.1.0] - 2024-01-XX

### Added
- **Core Transpiler Engine**
  - Lexical analysis for dplyr syntax
  - Abstract Syntax Tree (AST) generation
  - SQL generation with dialect-specific optimizations
  - Error handling with detailed diagnostics

- **Supported dplyr Operations**
  - `select()` - Column selection with renaming support
  - `filter()` - Row filtering with complex conditions
  - `mutate()` - Column creation and transformation
  - `summarise()` - Aggregation operations
  - `arrange()` - Sorting with multiple columns
  - `group_by()` - Grouping for aggregations
  - Pipe operator (`%>%`) chaining

- **SQL Dialect Support**
  - PostgreSQL with advanced features
  - MySQL with dialect-specific functions
  - SQLite with compatibility considerations
  - DuckDB with analytical optimizations

- **Command-Line Interface**
  - File input processing
  - stdin/stdout pipeline integration
  - Multiple output formats (pretty, compact, JSON)
  - Validation-only mode (`--validate-only`)
  - Verbose (`--verbose`) and debug (`--debug`) modes
  - Colored output support

- **Cross-Platform Compatibility**
  - Linux (x86_64, ARM64) support
  - macOS (Intel, Apple Silicon) support
  - Windows (x86_64) support
  - Platform-specific signal handling
  - Automatic pipe detection

- **Installation and Distribution**
  - Automated installation scripts
  - GitHub Releases with binary distributions
  - Cross-compilation for multiple architectures
  - SHA256 checksums for security verification

- **Developer Experience**
  - Comprehensive error messages with suggestions
  - JSON output with processing metadata
  - Performance timing information
  - Memory-efficient processing for large inputs

- **Testing and Quality Assurance**
  - Unit tests for all core components
  - Integration tests for CLI functionality
  - Cross-platform compatibility tests
  - Performance benchmarks
  - Memory safety verification with Miri
  - Security auditing with cargo-audit

### Technical Details

- **Architecture**: Modular design with separate lexer, parser, and generator components
- **Performance**: Optimized for both small queries and large batch processing
- **Memory Usage**: Efficient memory management with streaming support for large inputs
- **Error Handling**: Comprehensive error types with recovery suggestions
- **Extensibility**: Plugin-ready architecture for adding new SQL dialects

### Known Limitations

- Advanced dplyr functions (joins, window functions) not yet implemented
- Limited support for complex nested expressions
- R-specific functions require manual SQL equivalents

### Migration Guide

This is the initial release, so no migration is required.

---

## Release Process

Releases are automatically created when tags are pushed to the repository. The release process includes:

1. **Automated Building**: Cross-platform binaries are built for all supported platforms
2. **Testing**: Comprehensive test suite runs on multiple platforms and Rust versions
3. **Security**: Security audit and dependency vulnerability scanning
4. **Distribution**: Binaries are uploaded to GitHub Releases with checksums
5. **Installation**: Installation scripts are updated with the new version

### Supported Platforms

- **Linux**: x86_64, aarch64 (ARM64)
- **macOS**: x86_64 (Intel), aarch64 (Apple Silicon)
- **Windows**: x86_64

### Installation Methods

- **Automated Script**: One-line installation for Linux/macOS and Windows
- **Manual Download**: Direct binary download from GitHub Releases
- **From Source**: `cargo install libdplyr` (when published to crates.io)

For detailed installation instructions, see [README.md](README.md).