# extension_config.cmake for libdplyr DuckDB Extension
#
# This file defines extension metadata and configuration for the DuckDB extension system.
# It fulfills requirements R8-AC1 (version management) and R4-AC1 (build configuration).

# R8-AC1: Extension metadata and semver policy
set(EXTENSION_NAME "dplyr")
set(EXTENSION_DESCRIPTION "R dplyr syntax support for DuckDB")
set(EXTENSION_VERSION "0.1.0")
set(EXTENSION_VERSION_MAJOR 0)
set(EXTENSION_VERSION_MINOR 1)
set(EXTENSION_VERSION_PATCH 0)

# R8-AC1: Semantic versioning policy
# - MAJOR: Incompatible API changes (requires migration)
# - MINOR: Backward-compatible functionality additions
# - PATCH: Backward-compatible bug fixes
set(EXTENSION_SEMVER_POLICY "Semantic Versioning 2.0.0")
set(EXTENSION_API_VERSION "1")  # API compatibility version

# R8-AC1: DuckDB compatibility policy - Version agnostic approach
# Extension is designed to be compatible with a wide range of DuckDB versions
# by using stable APIs and avoiding version-specific features
set(DUCKDB_EXTENSION_COMPATIBILITY_APPROACH "VERSION_AGNOSTIC")
set(DUCKDB_EXTENSION_MIN_SUPPORTED "2.0.0")  # Minimum for extension metadata support
set(DUCKDB_EXTENSION_TESTED_VERSIONS "2.0.0")

# R8-AC1: Extension compatibility strategy
# - Use only stable DuckDB APIs that are unlikely to change
# - Implement runtime feature detection instead of compile-time version checks
# - Graceful degradation for unsupported features
set(EXTENSION_COMPATIBILITY_STRATEGY "RUNTIME_FEATURE_DETECTION")

# R8-AC1: Deprecation and breaking change policy
# Breaking changes require 1 minor version advance notice
set(EXTENSION_DEPRECATION_POLICY "1_MINOR_VERSION_NOTICE")
set(EXTENSION_BREAKING_CHANGE_NOTICE "CHANGELOG.md and GitHub Release Notes")
set(EXTENSION_SUPPORT_LIFECYCLE "Current + 2 previous minor versions")

# R8-AC1: Extension metadata and contact information
set(EXTENSION_AUTHOR "libdplyr contributors")
set(EXTENSION_MAINTAINER "libdplyr team")
set(EXTENSION_LICENSE "MIT")
set(EXTENSION_HOMEPAGE "https://github.com/your-org/libdplyr")
set(EXTENSION_REPOSITORY "https://github.com/your-org/libdplyr.git")
set(EXTENSION_ISSUES_URL "https://github.com/your-org/libdplyr/issues")
set(EXTENSION_DOCUMENTATION_URL "https://github.com/your-org/libdplyr/blob/main/README.md")

# R8-AC1: Extension categories and tags
set(EXTENSION_CATEGORIES "parser;syntax;r-language;data-manipulation")
set(EXTENSION_TAGS "dplyr;r;sql;transpiler;data-analysis")

# R4-AC1: Build configuration - Source files
set(EXTENSION_SOURCES
    extension/src/dplyr.cpp
)

set(EXTENSION_HEADERS
    extension/include/dplyr.h
)

# R4-AC1: Extension dependencies and requirements
set(EXTENSION_DEPENDENCIES
    # No external DuckDB extension dependencies
    # Rust toolchain required for libdplyr_c compilation
)

# R4-AC1: Build-time dependencies
set(EXTENSION_BUILD_DEPENDENCIES
    "Rust toolchain >= 1.70.0"
    "Corrosion CMake package"
    "libdplyr_c crate"
)

# R4-AC1: Runtime dependencies
set(EXTENSION_RUNTIME_DEPENDENCIES
    # No runtime dependencies beyond DuckDB core
)

# R4-AC1: Required system libraries (platform-specific)
if(WIN32)
    set(EXTENSION_SYSTEM_LIBS
        ws2_32 userenv bcrypt ntdll advapi32 shell32 ole32
    )
elseif(APPLE)
    set(EXTENSION_SYSTEM_LIBS
        "-framework Security"
        "-framework CoreFoundation"
        "-framework SystemConfiguration"
    )
elseif(UNIX)
    set(EXTENSION_SYSTEM_LIBS
        pthread dl m rt
    )
endif()

# R4-AC1: Compiler requirements and standards
set(EXTENSION_CXX_STANDARD 17)
set(EXTENSION_C_STANDARD 11)
set(EXTENSION_CXX_STANDARD_REQUIRED ON)
set(EXTENSION_CXX_EXTENSIONS OFF)

# R4-AC1: Minimum compiler versions
set(EXTENSION_MIN_GCC_VERSION "7.0")
set(EXTENSION_MIN_CLANG_VERSION "6.0")
set(EXTENSION_MIN_MSVC_VERSION "19.14")  # Visual Studio 2017 15.7

# R4-AC1: Build type specific settings
set(EXTENSION_DEBUG_FLAGS "-g -O0 -DDEBUG -DDPLYR_DEBUG_MODE")
set(EXTENSION_RELEASE_FLAGS "-O3 -DNDEBUG -DDPLYR_RELEASE_MODE")
set(EXTENSION_RELWITHDEBINFO_FLAGS "-O2 -g -DNDEBUG")
set(EXTENSION_MINSIZEREL_FLAGS "-Os -DNDEBUG")

# R4-AC1: Warning flags for better code quality
set(EXTENSION_WARNING_FLAGS
    -Wall
    -Wextra
    -Wpedantic
    -Wno-unused-parameter
    -Wno-missing-field-initializers
)

# R4-AC1: Platform-specific compile definitions
if(WIN32)
    set(EXTENSION_COMPILE_DEFINITIONS
        WIN32_LEAN_AND_MEAN
        NOMINMAX
        DUCKDB_EXTENSION_MAIN
        _CRT_SECURE_NO_WARNINGS
        _WIN32_WINNT=0x0601  # Windows 7 minimum
        DPLYR_PLATFORM_WINDOWS
    )
elseif(APPLE)
    set(EXTENSION_COMPILE_DEFINITIONS
        _DARWIN_C_SOURCE
        DPLYR_PLATFORM_MACOS
    )
elseif(UNIX)
    set(EXTENSION_COMPILE_DEFINITIONS
        _GNU_SOURCE
        _POSIX_C_SOURCE=200809L
        DPLYR_PLATFORM_LINUX
    )
endif()

# R4-AC1: Feature detection compile definitions
set(EXTENSION_FEATURE_DEFINITIONS
    DPLYR_VERSION_MAJOR=${EXTENSION_VERSION_MAJOR}
    DPLYR_VERSION_MINOR=${EXTENSION_VERSION_MINOR}
    DPLYR_VERSION_PATCH=${EXTENSION_VERSION_PATCH}
    DPLYR_API_VERSION=${EXTENSION_API_VERSION}
    DPLYR_VERSION_AGNOSTIC=1
    DPLYR_RUNTIME_FEATURE_DETECTION=1
)

# R4-AC1: Security hardening options
if(NOT WIN32)
    set(EXTENSION_SECURITY_FLAGS
        -fstack-protector-strong
        -D_FORTIFY_SOURCE=2
        -fPIE
    )
    if(APPLE)
        set(EXTENSION_SECURITY_LINK_FLAGS "")
    else()
        set(EXTENSION_SECURITY_LINK_FLAGS
            -Wl,-z,relro,-z,now
            -pie
        )
    endif()
endif()

# R8-AC1: Extension capabilities and features
set(EXTENSION_FEATURES
    "parser_extension"      # Provides SQL parser extensions
    "dplyr_syntax"         # Supports R dplyr syntax
    "rust_integration"     # Uses Rust backend
    "thread_safe"          # Thread-safe operations
    "caching"              # Built-in caching support
    "error_recovery"       # Graceful error handling
    "debug_logging"        # Debug logging support
    "performance_metrics"  # Performance monitoring
    "version_agnostic"     # Works across DuckDB versions
    "runtime_detection"    # Runtime feature detection
    "graceful_degradation" # Graceful feature degradation
)

# R8-AC1: Supported dplyr operations (minimum operation set from R1-AC2)
set(EXTENSION_SUPPORTED_OPERATIONS
    "select"               # Column selection
    "filter"               # Row filtering
    "mutate"               # Column transformation
    "arrange"              # Row ordering
    "summarise"            # Aggregation
    "group_by"             # Grouping
)

# R4-AC1: Test configuration
set(EXTENSION_TEST_TIMEOUT 30)
set(EXTENSION_RUST_TEST_TIMEOUT 120)
set(EXTENSION_SMOKE_TEST_FILE "tests/smoke.sql")
set(EXTENSION_INTEGRATION_TEST_DIR "tests")

# R4-AC1: Installation and deployment paths
set(EXTENSION_INSTALL_DIR "extensions")
set(EXTENSION_CONFIG_DIR "config")
set(EXTENSION_DOC_DIR "docs")
set(EXTENSION_BINARY_NAME "dplyr")

# Register the extension with DuckDB (out-of-tree build)
get_filename_component(DPLYR_EXTENSION_ROOT "${CMAKE_CURRENT_LIST_DIR}" ABSOLUTE)
duckdb_extension_load(${EXTENSION_NAME}
    DONT_LINK
    LOAD_TESTS
    SOURCE_DIR "${DPLYR_EXTENSION_ROOT}"
    INCLUDE_DIR "${DPLYR_EXTENSION_ROOT}/extension/include"
    EXTENSION_VERSION ${EXTENSION_VERSION}
)

# R8-AC1: Flexible DuckDB compatibility check function
function(check_duckdb_version DUCKDB_VERSION)
    message(STATUS "Checking DuckDB version compatibility...")
    message(STATUS "  DuckDB version: ${DUCKDB_VERSION}")
    message(STATUS "  Compatibility approach: ${DUCKDB_EXTENSION_COMPATIBILITY_APPROACH}")

    # Only check for very old versions that lack basic extension API
    if(DUCKDB_VERSION VERSION_LESS ${DUCKDB_EXTENSION_MIN_SUPPORTED})
        message(WARNING
            "DuckDB version ${DUCKDB_VERSION} is very old. "
            "Minimum supported version: ${DUCKDB_EXTENSION_MIN_SUPPORTED}. "
            "Extension may not work properly with very old DuckDB versions.")
    endif()

    # Check if version is in tested versions list
    list(FIND DUCKDB_EXTENSION_TESTED_VERSIONS ${DUCKDB_VERSION} VERSION_INDEX)
    if(VERSION_INDEX GREATER -1)
        message(STATUS "  ✓ DuckDB version ${DUCKDB_VERSION} is fully tested and supported")
    else()
        message(STATUS "  ℹ DuckDB version ${DUCKDB_VERSION} is not explicitly tested but should work")
        message(STATUS "    Extension uses runtime feature detection for compatibility")
    endif()

    message(STATUS "  Strategy: ${EXTENSION_COMPATIBILITY_STRATEGY}")
endfunction()

# R8-AC1: Extension compatibility validation
function(validate_extension_compatibility)
    # Check CMake version
    if(CMAKE_VERSION VERSION_LESS "3.15")
        message(FATAL_ERROR "CMake 3.15 or higher is required")
    endif()

    # Check compiler support
    if(CMAKE_CXX_COMPILER_ID STREQUAL "GNU")
        if(CMAKE_CXX_COMPILER_VERSION VERSION_LESS ${EXTENSION_MIN_GCC_VERSION})
            message(FATAL_ERROR "GCC ${EXTENSION_MIN_GCC_VERSION} or higher is required")
        endif()
    elseif(CMAKE_CXX_COMPILER_ID STREQUAL "Clang")
        if(CMAKE_CXX_COMPILER_VERSION VERSION_LESS ${EXTENSION_MIN_CLANG_VERSION})
            message(FATAL_ERROR "Clang ${EXTENSION_MIN_CLANG_VERSION} or higher is required")
        endif()
    elseif(CMAKE_CXX_COMPILER_ID STREQUAL "MSVC")
        if(CMAKE_CXX_COMPILER_VERSION VERSION_LESS ${EXTENSION_MIN_MSVC_VERSION})
            message(FATAL_ERROR "MSVC ${EXTENSION_MIN_MSVC_VERSION} or higher is required")
        endif()
    endif()

    message(STATUS "✓ Extension compatibility validation passed")
endfunction()

# R4-AC1: Rust configuration and integration
set(RUST_CRATE_NAME "libdplyr_c")
set(RUST_CRATE_PATH "libdplyr_c")
set(RUST_TARGET_DIR "${CMAKE_BINARY_DIR}/rust_target")
set(RUST_MIN_VERSION "1.70.0")
set(RUST_EDITION "2021")

# R4-AC1: Rust build configuration
set(RUST_BUILD_TYPE_DEBUG "debug")
set(RUST_BUILD_TYPE_RELEASE "release")
set(RUST_FEATURES "")  # Default features only
set(RUST_TARGET_FEATURES "")  # Platform-specific features

# R4-AC1: Development tools and quality assurance
set(EXTENSION_DEV_TOOLS
    rust-test           # Run Rust unit tests
    rust-clippy         # Rust linter
    rust-fmt            # Rust formatter
    rust-doc            # Generate Rust documentation
    quality-check       # Overall quality checks
    security-audit      # Security vulnerability scan
    benchmark           # Performance benchmarks
)

# R4-AC1: Quality gates and thresholds
set(EXTENSION_QUALITY_GATES
    "test_coverage_min=85"      # Minimum test coverage (from testing-standards.md)
    "clippy_warnings_max=0"     # No clippy warnings allowed
    "build_warnings_max=0"      # No build warnings allowed
    "benchmark_regression_max=5" # Max 5% performance regression
)

# R8-AC1: Extension registration and loading information
set(EXTENSION_LOAD_TYPE "EXPLICIT")  # Requires explicit LOAD command
set(EXTENSION_SCOPE "DATABASE")      # Database-scoped extension
set(EXTENSION_AUTOLOAD "false")      # Not loaded automatically
set(EXTENSION_PRIORITY "normal")     # Loading priority

# R8-AC1: Extension entry points (from R2-AC1)
set(EXTENSION_ENTRY_POINTS
    "DPLYR keyword"             # Primary parser extension entry point
    "dplyr() table function"    # Optional table function entry point
)

# R4-AC1: Performance and resource limits
set(EXTENSION_MAX_INPUT_SIZE "1048576")     # 1MB max input (from R9-AC2)
set(EXTENSION_MAX_PROCESSING_TIME "30000")  # 30 seconds max processing
set(EXTENSION_CACHE_SIZE "100")             # Cache up to 100 entries
set(EXTENSION_THREAD_SAFETY "true")         # Thread-safe operations

# R8-AC1: Runtime feature detection configuration
set(EXTENSION_RUNTIME_FEATURES
    "parser_extension_api"  # Check for parser extension API availability
    "table_function_api"    # Check for table function API
    "error_handling_api"    # Check for enhanced error handling
    "memory_management"     # Check for memory management features
)

# R8-AC1: Fallback behavior configuration
set(EXTENSION_FALLBACK_BEHAVIOR
    "disable_unsupported_features"  # Disable features not available
    "log_compatibility_warnings"    # Log compatibility issues
    "graceful_error_messages"       # Provide helpful error messages
)

# R8-AC1: Comprehensive configuration summary
message(STATUS "=== DuckDB dplyr Extension Configuration ===")
message(STATUS "Extension Information:")
message(STATUS "  Name: ${EXTENSION_NAME}")
message(STATUS "  Version: ${EXTENSION_VERSION} (API v${EXTENSION_API_VERSION})")
message(STATUS "  Description: ${EXTENSION_DESCRIPTION}")
message(STATUS "  License: ${EXTENSION_LICENSE}")
message(STATUS "  Homepage: ${EXTENSION_HOMEPAGE}")

message(STATUS "Compatibility:")
message(STATUS "  Approach: ${DUCKDB_EXTENSION_COMPATIBILITY_APPROACH}")
message(STATUS "  Strategy: ${EXTENSION_COMPATIBILITY_STRATEGY}")
message(STATUS "  Min Supported: ${DUCKDB_EXTENSION_MIN_SUPPORTED}+")
message(STATUS "  Tested Versions: ${DUCKDB_EXTENSION_TESTED_VERSIONS}")
message(STATUS "  Semver Policy: ${EXTENSION_SEMVER_POLICY}")
message(STATUS "  Deprecation Policy: ${EXTENSION_DEPRECATION_POLICY}")

message(STATUS "Features and Capabilities:")
message(STATUS "  Features: ${EXTENSION_FEATURES}")
message(STATUS "  Supported Operations: ${EXTENSION_SUPPORTED_OPERATIONS}")
message(STATUS "  Entry Points: ${EXTENSION_ENTRY_POINTS}")
message(STATUS "  Load Type: ${EXTENSION_LOAD_TYPE}")
message(STATUS "  Scope: ${EXTENSION_SCOPE}")

message(STATUS "Build Configuration:")
message(STATUS "  C++ Standard: ${EXTENSION_CXX_STANDARD}")
message(STATUS "  Rust Version: >= ${RUST_MIN_VERSION}")
message(STATUS "  Rust Edition: ${RUST_EDITION}")
message(STATUS "  Thread Safety: ${EXTENSION_THREAD_SAFETY}")

message(STATUS "Quality Assurance:")
message(STATUS "  Quality Gates: ${EXTENSION_QUALITY_GATES}")
message(STATUS "  Dev Tools: ${EXTENSION_DEV_TOOLS}")

message(STATUS "Resource Limits:")
message(STATUS "  Max Input Size: ${EXTENSION_MAX_INPUT_SIZE} bytes")
message(STATUS "  Max Processing Time: ${EXTENSION_MAX_PROCESSING_TIME} ms")
message(STATUS "  Cache Size: ${EXTENSION_CACHE_SIZE} entries")
message(STATUS "==============================================")
