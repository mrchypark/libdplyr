# CMakeLists.txt for libdplyr DuckDB Extension
#
# This CMake configuration builds the libdplyr DuckDB extension by integrating
# Rust and C++ components. It follows DuckDB extension development best practices
# and fulfills requirements R4-AC1 and R8-AC1.
#
# Based on duckdb-prql extension structure: https://github.com/ywelsch/duckdb-prql

cmake_minimum_required(VERSION 3.15)

# R8-AC1: Project version and DuckDB compatibility range
project(dplyr VERSION 0.1.0)

# R8-AC1: DuckDB extension configuration
set(DUCKDB_EXTENSION_NAME "dplyr")
# Note: No strict version requirements - extension uses runtime feature detection

# Set C++ standard
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# R4-AC1: Build configuration
if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE "Release" CACHE STRING "Choose the type of build." FORCE)
endif()

# Include DuckDB extension configuration
include(extension_config.cmake)

# R8-AC1: Validate extension compatibility
validate_extension_compatibility()

option(DUCKDB_VERSION_CHECK "Enable DuckDB version compatibility checking" ON)
option(DUCKDB_STRICT_VERSION_CHECK "Enable strict DuckDB version checking" OFF)

# Detect standalone configuration to optionally fetch DuckDB ourselves.
set(STANDALONE_BUILD FALSE)
if("${CMAKE_SOURCE_DIR}" STREQUAL "${CMAKE_CURRENT_SOURCE_DIR}")
    set(STANDALONE_BUILD TRUE)
endif()

if(STANDALONE_BUILD)
    set(DUCKDB_LOCAL_SOURCE "${CMAKE_CURRENT_SOURCE_DIR}/duckdb")
    if(EXISTS "${DUCKDB_LOCAL_SOURCE}/CMakeLists.txt")
        add_subdirectory(${DUCKDB_LOCAL_SOURCE} ${CMAKE_BINARY_DIR}/duckdb)
        set(DUCKDB_INCLUDE_DIR ${DUCKDB_LOCAL_SOURCE}/src/include)
        if(WIN32 AND TARGET duckdb_static)
            set(DUCKDB_LIBRARIES duckdb_static)
        else()
            set(DUCKDB_LIBRARIES duckdb)
        endif()
        if(NOT DEFINED DUCKDB_MODULE_BASE_DIR)
            set(DUCKDB_MODULE_BASE_DIR "${DUCKDB_LOCAL_SOURCE}" CACHE PATH "DuckDB module base directory")
        endif()
    else()
        include(FetchContent)
        FetchContent_Declare(
            duckdb
            GIT_REPOSITORY https://github.com/duckdb/duckdb.git
            GIT_TAG v1.4.2
            GIT_SHALLOW TRUE
        )
        FetchContent_MakeAvailable(duckdb)
        set(DUCKDB_INCLUDE_DIR ${duckdb_SOURCE_DIR}/src/include)
        if(WIN32 AND TARGET duckdb_static)
            set(DUCKDB_LIBRARIES duckdb_static)
        else()
            set(DUCKDB_LIBRARIES duckdb)
        endif()
        if(NOT DEFINED DUCKDB_MODULE_BASE_DIR)
            set(DUCKDB_MODULE_BASE_DIR "${duckdb_SOURCE_DIR}" CACHE PATH "DuckDB module base directory")
        endif()
    endif()
else()
    if(NOT DUCKDB_INCLUDE_DIR)
        if(EXISTS "${CMAKE_SOURCE_DIR}/src/include/duckdb.hpp")
            set(DUCKDB_INCLUDE_DIR "${CMAKE_SOURCE_DIR}/src/include")
        endif()
    endif()

    if(NOT DUCKDB_LIBRARIES)
        set(DUCKDB_LIBRARIES duckdb)
    endif()

    if(NOT DUCKDB_INCLUDE_DIR OR NOT DUCKDB_LIBRARIES)
        find_package(DuckDB QUIET)
        if(DuckDB_FOUND)
            if(NOT DUCKDB_INCLUDE_DIR)
                set(DUCKDB_INCLUDE_DIR ${DuckDB_INCLUDE_DIRS})
            endif()
            if(NOT DUCKDB_LIBRARIES)
                set(DUCKDB_LIBRARIES ${DuckDB_LIBRARIES})
            endif()
        endif()
    endif()
endif()

if(NOT DUCKDB_INCLUDE_DIR OR NOT DUCKDB_LIBRARIES)
    message(FATAL_ERROR "DuckDB headers/libs not provided. Set DUCKDB_INCLUDE_DIR and DUCKDB_LIBRARIES or build standalone.")
endif()

if(NOT DEFINED DUCKDB_VERSION)
    set(_duckdb_header_candidates
        "${DUCKDB_INCLUDE_DIR}/duckdb.hpp"
        "${DUCKDB_INCLUDE_DIR}/duckdb.h"
    )
    foreach(_duckdb_header IN LISTS _duckdb_header_candidates)
        if(EXISTS "${_duckdb_header}")
            file(READ "${_duckdb_header}" DUCKDB_HEADER_CONTENT)
            string(REGEX MATCH "#define DUCKDB_VERSION \"([0-9]+\\.[0-9]+\\.[0-9]+)\""
                   DUCKDB_VERSION_MATCH "${DUCKDB_HEADER_CONTENT}")
            if(DUCKDB_VERSION_MATCH)
                set(DUCKDB_VERSION ${CMAKE_MATCH_1})
                break()
            endif()
        endif()
    endforeach()
endif()

if(NOT DEFINED DUCKDB_VERSION OR DUCKDB_VERSION STREQUAL "unknown")
    if(DEFINED DUCKDB_EXTENSION_TESTED_VERSIONS)
        list(GET DUCKDB_EXTENSION_TESTED_VERSIONS 0 _duckdb_version_guess)
        set(DUCKDB_VERSION "${_duckdb_version_guess}")
    elseif(DEFINED DUCKDB_EXTENSION_MIN_SUPPORTED)
        set(DUCKDB_VERSION "${DUCKDB_EXTENSION_MIN_SUPPORTED}")
    else()
        set(DUCKDB_VERSION "unknown")
    endif()
endif()

if(DUCKDB_VERSION_CHECK AND NOT DUCKDB_VERSION STREQUAL "unknown")
    check_duckdb_version(${DUCKDB_VERSION})
else()
    message(STATUS "DuckDB version checking disabled or version unknown")
    message(STATUS "Extension will use runtime feature detection for compatibility")
endif()

# Ensure a normalized DuckDB version is available for extension metadata
if(NOT DEFINED DUCKDB_NORMALIZED_VERSION OR DUCKDB_NORMALIZED_VERSION STREQUAL "")
    if(DEFINED DUCKDB_VERSION AND NOT DUCKDB_VERSION STREQUAL "unknown")
        set(DUCKDB_NORMALIZED_VERSION "v${DUCKDB_VERSION}")
    endif()
endif()

message(STATUS "DuckDB include directory: ${DUCKDB_INCLUDE_DIR}")
message(STATUS "DuckDB libraries: ${DUCKDB_LIBRARIES}")

# R4-AC1: Find and configure Rust toolchain
message(STATUS "Configuring Rust integration...")

# Check if Rust is available
execute_process(
    COMMAND rustc --version
    OUTPUT_VARIABLE RUSTC_VERSION_OUTPUT
    ERROR_VARIABLE RUSTC_ERROR
    RESULT_VARIABLE RUSTC_RESULT
)

if(NOT RUSTC_RESULT EQUAL 0)
    message(FATAL_ERROR
        "Rust compiler not found. Please install Rust from https://rustup.rs/\n"
        "Error: ${RUSTC_ERROR}")
endif()

# Extract Rust version
string(REGEX MATCH "rustc ([0-9]+\\.[0-9]+\\.[0-9]+)" RUST_VERSION_MATCH "${RUSTC_VERSION_OUTPUT}")
if(RUST_VERSION_MATCH)
    set(RUST_VERSION ${CMAKE_MATCH_1})
    message(STATUS "Found Rust version: ${RUST_VERSION}")
else()
    message(WARNING "Could not determine Rust version")
    set(RUST_VERSION "unknown")
endif()

# Detect Rust host target
execute_process(
    COMMAND rustc --version --verbose
    OUTPUT_VARIABLE RUSTC_VERBOSE_OUTPUT
    ERROR_QUIET
)

if(RUSTC_VERBOSE_OUTPUT MATCHES "host: ([a-zA-Z0-9_-]+)")
    set(RUST_HOST_TARGET ${CMAKE_MATCH_1})
    message(STATUS "Detected Rust host target: ${RUST_HOST_TARGET}")
else()
    # Fallback to platform detection
    if(CMAKE_SYSTEM_NAME STREQUAL "Darwin")
        if(CMAKE_SYSTEM_PROCESSOR MATCHES "arm64|aarch64")
            set(RUST_HOST_TARGET "aarch64-apple-darwin")
        else()
            set(RUST_HOST_TARGET "x86_64-apple-darwin")
        endif()
    elseif(CMAKE_SYSTEM_NAME STREQUAL "Linux")
        if(CMAKE_SYSTEM_PROCESSOR MATCHES "aarch64|arm64")
            set(RUST_HOST_TARGET "aarch64-unknown-linux-gnu")
        else()
            set(RUST_HOST_TARGET "x86_64-unknown-linux-gnu")
        endif()
    elseif(CMAKE_SYSTEM_NAME STREQUAL "Windows")
        if(CMAKE_SYSTEM_PROCESSOR MATCHES "ARM64|aarch64")
            set(RUST_HOST_TARGET "aarch64-pc-windows-msvc")
        else()
            set(RUST_HOST_TARGET "x86_64-pc-windows-msvc")
        endif()
    else()
        message(FATAL_ERROR "Unsupported platform: ${CMAKE_SYSTEM_NAME}")
    endif()
    message(STATUS "Using fallback Rust target: ${RUST_HOST_TARGET}")
endif()

# Check if rustup is available and find toolchains
set(Rust_TOOLCHAIN_DISPLAY "rustc-${RUST_VERSION}")
execute_process(
    COMMAND rustup --version
    OUTPUT_VARIABLE RUSTUP_VERSION_OUTPUT
    ERROR_QUIET
    RESULT_VARIABLE RUSTUP_RESULT
)

if(RUSTUP_RESULT EQUAL 0)
    message(STATUS "Found rustup: ${RUSTUP_VERSION_OUTPUT}")

    # Get available toolchains
    execute_process(
        COMMAND rustup toolchain list
        OUTPUT_VARIABLE RUSTUP_TOOLCHAINS
        ERROR_QUIET
    )

    string(STRIP "${RUSTUP_TOOLCHAINS}" RUSTUP_TOOLCHAINS_STRIPPED)

    if(RUSTUP_TOOLCHAINS_STRIPPED STREQUAL "")
        message(WARNING "rustup toolchain list is empty; falling back to system rustc (no rustup toolchain hint)")
        unset(Rust_TOOLCHAIN)
        set(Rust_TOOLCHAIN_DISPLAY "system-rustc")
    else()
        # Parse available toolchains and select the best one (prefer plain "stable")
        if(RUSTUP_TOOLCHAINS MATCHES "1\\.85[^\\n]*-${RUST_HOST_TARGET}")
            string(REGEX MATCH "1\\.85[^\\n]*-${RUST_HOST_TARGET}" Rust_TOOLCHAIN "${RUSTUP_TOOLCHAINS}")
        elseif(RUSTUP_TOOLCHAINS MATCHES "stable")
            set(Rust_TOOLCHAIN "stable")
        elseif(RUSTUP_TOOLCHAINS MATCHES "nightly-${RUST_HOST_TARGET}")
            set(Rust_TOOLCHAIN "nightly-${RUST_HOST_TARGET}")
        else()
            # Use the first available toolchain as a fallback
            string(REGEX MATCH "([^\\n]+)" FIRST_TOOLCHAIN "${RUSTUP_TOOLCHAINS}")
            if(FIRST_TOOLCHAIN)
                string(STRIP "${FIRST_TOOLCHAIN}" Rust_TOOLCHAIN)
                string(REGEX REPLACE " \\(default\\)" "" Rust_TOOLCHAIN "${Rust_TOOLCHAIN}")
            else()
                unset(Rust_TOOLCHAIN)
            endif()
        endif()

        if(DEFINED Rust_TOOLCHAIN AND NOT Rust_TOOLCHAIN STREQUAL "")
            set(Rust_TOOLCHAIN_DISPLAY "${Rust_TOOLCHAIN}")
            message(STATUS "Using Rust toolchain: ${Rust_TOOLCHAIN_DISPLAY}")
        else()
            message(WARNING "No rustup toolchain selected; using system rustc")
            unset(Rust_TOOLCHAIN)
            set(Rust_TOOLCHAIN_DISPLAY "system-rustc")
        endif()
    endif()
else()
    message(STATUS "rustup not found, using system Rust")
    unset(Rust_TOOLCHAIN)
    set(Rust_TOOLCHAIN_DISPLAY "system-rustc")
endif()

# Verify Cargo is available
execute_process(
    COMMAND cargo --version
    OUTPUT_VARIABLE CARGO_VERSION_OUTPUT
    ERROR_VARIABLE CARGO_ERROR
    RESULT_VARIABLE CARGO_RESULT
)

if(NOT CARGO_RESULT EQUAL 0)
    message(FATAL_ERROR
        "Cargo not found. Please ensure Rust is properly installed.\n"
        "Error: ${CARGO_ERROR}")
endif()

message(STATUS "Found Cargo: ${CARGO_VERSION_OUTPUT}")

# R4-AC1: Setup Corrosion for Rust-CMake integration
find_package(corrosion QUIET)
if(NOT corrosion_FOUND)
    message(STATUS "Corrosion not found, downloading...")
    include(FetchContent)
    FetchContent_Declare(
        Corrosion
        GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
        GIT_TAG v0.5.1
        GIT_SHALLOW TRUE
    )

    # Check if fetch was successful
    FetchContent_GetProperties(Corrosion)
    if(NOT corrosion_POPULATED)
        message(STATUS "Downloading Corrosion...")
        FetchContent_Populate(Corrosion)

        # Verify Corrosion was downloaded
        if(NOT EXISTS "${corrosion_SOURCE_DIR}/CMakeLists.txt")
            message(FATAL_ERROR "Failed to download Corrosion")
        endif()

        add_subdirectory(${corrosion_SOURCE_DIR} ${corrosion_BINARY_DIR})
    endif()

    message(STATUS "Successfully configured Corrosion for Rust integration")
else()
    message(STATUS "Found existing Corrosion installation")
endif()
# R4-AC1: Configure Rust build environment
set(RUST_TARGET_DIR "${CMAKE_BINARY_DIR}/rust_target")
set(ENV{CARGO_TARGET_DIR} ${RUST_TARGET_DIR})

# Verify Rust crate exists
set(RUST_MANIFEST_PATH "${CMAKE_CURRENT_SOURCE_DIR}/libdplyr_c/Cargo.toml")
if(NOT EXISTS ${RUST_MANIFEST_PATH})
    message(FATAL_ERROR
        "Rust crate manifest not found at ${RUST_MANIFEST_PATH}. "
        "Please ensure the libdplyr_c crate is properly set up.")
endif()

# R4-AC1: Import libdplyr_c Rust crate
message(STATUS "Importing Rust crate: libdplyr_c")
corrosion_import_crate(
    MANIFEST_PATH ${RUST_MANIFEST_PATH}
    CRATES libdplyr_c
)

# Verify the crate was imported successfully
if(NOT TARGET libdplyr_c)
    message(FATAL_ERROR "Failed to import Rust crate libdplyr_c")
endif()

message(STATUS "Successfully imported Rust crate: libdplyr_c")

# R4-AC1: Set Rust compilation flags based on build type
if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    set(ENV{RUST_BACKTRACE} "1")
else()
    # Release build optimizations are handled by Cargo.toml profile
endif()

# R4-AC1: Platform-specific Rust compilation settings
if(WIN32)
    # Windows-specific Rust settings
    set(ENV{RUSTFLAGS} "$ENV{RUSTFLAGS} -C target-feature=+crt-static")
endif()

# Extension source files
set(EXTENSION_SOURCES
    extension/src/dplyr.cpp
)

# R4-AC1: Build loadable extension with DuckDB helper macro
build_loadable_extension(${DUCKDB_EXTENSION_NAME} " " ${EXTENSION_SOURCES})
build_static_extension(${DUCKDB_EXTENSION_NAME} ${EXTENSION_SOURCES})
set(DUCKDB_EXTENSION_TARGET ${DUCKDB_EXTENSION_NAME}_loadable_extension)
set(DUCKDB_EXTENSION_STATIC_TARGET ${DUCKDB_EXTENSION_NAME}_extension)
set(EXTENSION_TARGETS ${DUCKDB_EXTENSION_TARGET} ${DUCKDB_EXTENSION_STATIC_TARGET})

foreach(EXT_TARGET ${EXTENSION_TARGETS})
    set_target_properties(${EXT_TARGET} PROPERTIES
        CXX_STANDARD 17
        CXX_STANDARD_REQUIRED ON
        POSITION_INDEPENDENT_CODE ON
    )

    target_include_directories(${EXT_TARGET} PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/extension/include
        ${DUCKDB_INCLUDE_DIR}
        ${CMAKE_CURRENT_BINARY_DIR}  # For generated headers
    )

    target_link_libraries(${EXT_TARGET} libdplyr_c)

    if(WIN32)
        target_link_libraries(${EXT_TARGET}
            ws2_32 userenv bcrypt ntdll advapi32 shell32 ole32
        )

        target_compile_definitions(${EXT_TARGET}
            PRIVATE
            WIN32_LEAN_AND_MEAN NOMINMAX
            DUCKDB_EXTENSION_MAIN
            DUCKDB_STATIC_BUILD
            DUCKDB_VERSION="${DUCKDB_VERSION}"
        )

    elseif(APPLE)
        target_link_libraries(${EXT_TARGET}
            "-framework Security"
            "-framework CoreFoundation"
            "-framework SystemConfiguration"
        )

        target_compile_options(${EXT_TARGET}
            PRIVATE
            -mmacosx-version-min=10.14
        )

    elseif(UNIX)
        target_link_libraries(${EXT_TARGET}
            pthread dl m rt
        )

        target_compile_options(${EXT_TARGET}
            PRIVATE
            -fPIC
        )
    endif()

    # R4-AC1: Compiler-specific optimizations and warnings
    if(CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang")
        target_compile_options(${EXT_TARGET} PRIVATE
            -Wall -Wextra -Wpedantic -Wno-unused-parameter
            $<$<CONFIG:Release>:-O3 -DNDEBUG>
            $<$<CONFIG:Debug>:-g -O0>
        )

        # Link-time optimization for release builds
        if(CMAKE_BUILD_TYPE STREQUAL "Release")
            target_compile_options(${EXT_TARGET} PRIVATE -flto)
            target_link_options(${EXT_TARGET} PRIVATE -flto)
        endif()

    elseif(CMAKE_CXX_COMPILER_ID MATCHES "MSVC")
        target_compile_options(${EXT_TARGET} PRIVATE
            /W4 /wd4100 /wd4996  # Disable unused parameter and deprecated warnings
            $<$<CONFIG:Release>:/O2 /DNDEBUG /GL>
            $<$<CONFIG:Debug>:/Od /Zi /RTC1>
        )

        # Whole program optimization for release builds
        if(CMAKE_BUILD_TYPE STREQUAL "Release")
            target_link_options(${EXT_TARGET} PRIVATE /LTCG)
        endif()
    endif()

    # DuckDB ships with RTTI disabled; extensions must match or they can reference
    # missing base class typeinfo symbols at load time (e.g. GlobalTableFunctionState).
    if(CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang")
        # Also disable DuckDB's RTTI-based debug assertions to keep headers
        # compatible with -fno-rtti (e.g. D_ASSERT(typeid(...))).
        target_compile_options(${EXT_TARGET} PRIVATE -fno-rtti -UDEBUG)
        target_compile_definitions(${EXT_TARGET} PRIVATE NDEBUG)
    elseif(CMAKE_CXX_COMPILER_ID MATCHES "MSVC")
        target_compile_options(${EXT_TARGET} PRIVATE /GR- /UDEBUG)
        target_compile_definitions(${EXT_TARGET} PRIVATE NDEBUG)
    endif()

    # R4-AC1: Security hardening flags
    if(NOT WIN32)
        target_compile_options(${EXT_TARGET} PRIVATE
            -fstack-protector-strong
            -D_FORTIFY_SOURCE=2
        )
        if(APPLE)
            target_link_options(${EXT_TARGET} PRIVATE
                "-Wl,-dead_strip_dylibs"
            )
        else()
            target_link_options(${EXT_TARGET} PRIVATE
                -Wl,-z,relro,-z,now
            )
        endif()
    endif()
endforeach()

if(COMMAND install_duckdb_extension)
    install_duckdb_extension(${DUCKDB_EXTENSION_NAME})
endif()

# R8-AC1: Version information with build timestamp
string(TIMESTAMP BUILD_TIMESTAMP "%Y-%m-%d %H:%M:%S UTC" UTC)

# Set feature flags based on build type
if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    set(DPLYR_EXTENSION_DEBUG_MODE 1)
    set(DPLYR_EXTENSION_RELEASE_MODE 0)
else()
    set(DPLYR_EXTENSION_DEBUG_MODE 0)
    set(DPLYR_EXTENSION_RELEASE_MODE 1)
endif()

configure_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/extension_version.h.in"
    "${CMAKE_CURRENT_BINARY_DIR}/extension_version.h"
    @ONLY
)

# R4-AC2: Enable testing
enable_testing()

# R4-AC2: Test configuration
set(EXTENSION_BINARY_PATH "${CMAKE_BINARY_DIR}/${DUCKDB_EXTENSION_NAME}.duckdb_extension")
set(TEST_TIMEOUT 30) # 30 seconds timeout for tests

# R4-AC2: Smoke test - basic extension loading
add_test(
    NAME smoke_test_load
    COMMAND ${CMAKE_COMMAND} -E env
        "DUCKDB_EXTENSION_PATH=${CMAKE_BINARY_DIR}"
        duckdb -unsigned -c "LOAD '${EXTENSION_BINARY_PATH}'; SELECT 'Extension loaded successfully' as result;"
)
set_tests_properties(smoke_test_load PROPERTIES TIMEOUT ${TEST_TIMEOUT})

# R4-AC2: Comprehensive smoke tests
if(EXISTS "${CMAKE_SOURCE_DIR}/tests/smoke.sql")
    # Create temporary database for smoke tests
    add_test(
        NAME smoke_test_comprehensive
        COMMAND ${CMAKE_COMMAND} -E env
            "DUCKDB_EXTENSION_PATH=${CMAKE_BINARY_DIR}"
            duckdb -unsigned ":memory:" -c "
                LOAD '${EXTENSION_BINARY_PATH}';
                CREATE TABLE test_data AS SELECT i as id, (i % 5) + 18 as age FROM range(1, 6) as t(i);
                -- Table function entry point (works in CLI scripts reliably)
                SELECT * FROM dplyr('test_data %>% select(id) %>% arrange(id)');
            "
    )
    set_tests_properties(smoke_test_comprehensive PROPERTIES
        TIMEOUT 120
        LABELS "smoke;comprehensive"
    )

    # R1-AC2: Test minimum operation set specifically
    add_test(
        NAME smoke_test_minimum_operations
        COMMAND ${CMAKE_COMMAND} -E env
            "DUCKDB_EXTENSION_PATH=${CMAKE_BINARY_DIR}"
            duckdb -unsigned ":memory:" -c "
                LOAD '${EXTENSION_BINARY_PATH}';
                CREATE TABLE test_ops AS SELECT i as id, 'name_' || i as name, (i % 5) + 18 as age FROM range(1, 6) as t(i);
                -- Test each minimum operation via table function entry point
                SELECT * FROM dplyr('test_ops %>% select(id)');
                SELECT * FROM dplyr('test_ops %>% filter(age > 20)');
                SELECT * FROM dplyr('test_ops %>% mutate(age_plus_one = age + 1)');
                SELECT * FROM dplyr('test_ops %>% arrange(desc(age))');
                SELECT * FROM dplyr('test_ops %>% group_by(age) %>% summarise(count = n())');
            "
    )
    set_tests_properties(smoke_test_minimum_operations PROPERTIES
        TIMEOUT 60
        LABELS "smoke;minimum_ops"
    )

    # Embedded pipeline entry point test
    add_test(
        NAME smoke_test_dplyr_keyword
        COMMAND ${CMAKE_COMMAND} -E env
            "DUCKDB_EXTENSION_PATH=${CMAKE_BINARY_DIR}"
            duckdb -unsigned ":memory:" -c "
                LOAD '${EXTENSION_BINARY_PATH}';
                CREATE TABLE test_keyword AS SELECT 1 as id, 'test' as name;
                SELECT COUNT(*) FROM dplyr('test_keyword %>% select(id)') AS t;
            "
    )
    set_tests_properties(smoke_test_dplyr_keyword PROPERTIES
        TIMEOUT 30
        LABELS "smoke;keyword"
    )

    message(STATUS "Comprehensive smoke tests configured")
else()
    message(WARNING "Smoke test file not found at ${CMAKE_SOURCE_DIR}/tests/smoke.sql")
endif()

# R4-AC2: Rust unit tests
add_test(
    NAME rust_unit_tests
    COMMAND ${CMAKE_COMMAND} -E env
        "CARGO_TARGET_DIR=${RUST_TARGET_DIR}"
        cargo test --manifest-path ${CMAKE_CURRENT_SOURCE_DIR}/libdplyr_c/Cargo.toml
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
)
set_tests_properties(rust_unit_tests PROPERTIES TIMEOUT 120)

# R7-AC1, R7-AC3: C++ Integration Tests
option(BUILD_CPP_TESTS "Build C++ integration tests" ON)

if(BUILD_CPP_TESTS)
    # Find Google Test
    find_package(GTest QUIET)

    if(NOT GTest_FOUND)
        message(STATUS "Google Test not found, downloading...")
        include(FetchContent)
        FetchContent_Declare(
            googletest
            GIT_REPOSITORY https://github.com/google/googletest.git
            GIT_TAG v1.14.0
            GIT_SHALLOW TRUE
        )

        # For Windows: Prevent overriding the parent project's compiler/linker settings
        set(gtest_force_shared_crt ON CACHE BOOL "" FORCE)

        FetchContent_MakeAvailable(googletest)
        message(STATUS "Successfully configured Google Test")
    else()
        message(STATUS "Found existing Google Test installation")
    endif()

    # Create C++ integration test executable
    add_executable(duckdb_extension_integration_test
        tests/duckdb_extension_integration_test.cpp
    )

    # Configure test executable
    target_include_directories(duckdb_extension_integration_test PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/extension/include
        ${DUCKDB_INCLUDE_DIR}
        ${CMAKE_CURRENT_BINARY_DIR}
    )

    # Link test dependencies
    target_link_libraries(duckdb_extension_integration_test
        PRIVATE
        gtest_main
        gtest
        ${DUCKDB_LIBRARIES}
        libdplyr_c
        ${DUCKDB_EXTENSION_STATIC_TARGET}
    )

    # Platform-specific test linking
    if(WIN32)
        target_link_libraries(duckdb_extension_integration_test
            PRIVATE
            ws2_32 userenv bcrypt ntdll advapi32 shell32 ole32
        )
        target_compile_definitions(duckdb_extension_integration_test PRIVATE DUCKDB_STATIC_BUILD)
    elseif(APPLE)
        target_link_libraries(duckdb_extension_integration_test
            PRIVATE
            "-framework Security"
            "-framework CoreFoundation"
            "-framework SystemConfiguration"
        )
    elseif(UNIX)
        target_link_libraries(duckdb_extension_integration_test
            PRIVATE
            pthread dl m rt
        )
    endif()

    # Set test properties
    set_target_properties(duckdb_extension_integration_test PROPERTIES
        CXX_STANDARD 17
        CXX_STANDARD_REQUIRED ON
    )

    # Compiler-specific test settings
    if(CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang")
        target_compile_options(duckdb_extension_integration_test PRIVATE
            -Wall -Wextra -Wpedantic -Wno-unused-parameter
            $<$<CONFIG:Debug>:-g -O0>
        )
    elseif(CMAKE_CXX_COMPILER_ID MATCHES "MSVC")
        target_compile_options(duckdb_extension_integration_test PRIVATE
            /W4 /wd4100 /wd4996
            $<$<CONFIG:Debug>:/Od /Zi /RTC1>
        )
    endif()

    if(CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang")
        target_compile_options(duckdb_extension_integration_test PRIVATE -fno-rtti -UDEBUG)
        target_compile_definitions(duckdb_extension_integration_test PRIVATE NDEBUG)
    elseif(CMAKE_CXX_COMPILER_ID MATCHES "MSVC")
        target_compile_options(duckdb_extension_integration_test PRIVATE /GR- /UDEBUG)
        target_compile_definitions(duckdb_extension_integration_test PRIVATE NDEBUG)
    endif()

    # R7-AC1: DuckDB extension loading and functionality tests
    add_test(
        NAME cpp_integration_extension_loading
        COMMAND duckdb_extension_integration_test --gtest_filter="DuckDBExtensionTest.ExtensionLoadingSuccess:DuckDBExtensionTest.DplyrKeywordRecognition:DuckDBExtensionTest.TableFunctionEntryPoint"
        WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
    )
    set_tests_properties(cpp_integration_extension_loading PROPERTIES
        TIMEOUT 60
        ENVIRONMENT "DUCKDB_EXTENSION_PATH=${CMAKE_BINARY_DIR}"
    )

    # R2-AC2: Standard SQL integration and mixing tests
    add_test(
        NAME cpp_integration_sql_mixing
        COMMAND duckdb_extension_integration_test --gtest_filter="DuckDBExtensionTest.StandardSqlMixingWithCTE:DuckDBExtensionTest.SubqueryIntegration:DuckDBExtensionTest.JoinWithDplyrResults"
        WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
    )
    set_tests_properties(cpp_integration_sql_mixing PROPERTIES
        TIMEOUT 60
        ENVIRONMENT "DUCKDB_EXTENSION_PATH=${CMAKE_BINARY_DIR}"
    )

    # R7-AC3: Crash prevention and error handling tests
    add_test(
        NAME cpp_integration_crash_prevention
        COMMAND duckdb_extension_integration_test --gtest_filter="DuckDBExtensionTest.InvalidDplyrSyntaxNoCrash:DuckDBExtensionTest.NullPointerHandling:DuckDBExtensionTest.LargeInputHandling:DuckDBExtensionTest.ConcurrentAccessSafety:DuckDBExtensionTest.MemoryLeakPrevention"
        WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
    )
    set_tests_properties(cpp_integration_crash_prevention PROPERTIES
        TIMEOUT 120
        ENVIRONMENT "DUCKDB_EXTENSION_PATH=${CMAKE_BINARY_DIR}"
    )

    # R4-AC2: Smoke tests for basic functionality
    add_test(
        NAME cpp_integration_smoke_tests
        COMMAND duckdb_extension_integration_test --gtest_filter="DuckDBExtensionTest.SmokeTestBasicOperations"
        WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
    )
    set_tests_properties(cpp_integration_smoke_tests PROPERTIES
        TIMEOUT 30
        ENVIRONMENT "DUCKDB_EXTENSION_PATH=${CMAKE_BINARY_DIR}"
    )

    # Additional comprehensive test
    add_test(
        NAME cpp_integration_comprehensive
        COMMAND duckdb_extension_integration_test
        WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
    )
    set_tests_properties(cpp_integration_comprehensive PROPERTIES
        TIMEOUT 180
        ENVIRONMENT "DUCKDB_EXTENSION_PATH=${CMAKE_BINARY_DIR}"
    )

    # Make tests depend on extension build
    add_dependencies(duckdb_extension_integration_test ${DUCKDB_EXTENSION_STATIC_TARGET})

    message(STATUS "C++ integration tests configured successfully")
else()
    message(STATUS "C++ integration tests disabled (BUILD_CPP_TESTS=OFF)")
endif()

# R4-AC1: Development and utility targets
add_custom_target(dev-build
    COMMAND ${CMAKE_COMMAND} --build ${CMAKE_BINARY_DIR} --parallel
    COMMENT "Building extension for development"
)

add_custom_target(dev-test
    COMMAND ${CMAKE_COMMAND} --build ${CMAKE_BINARY_DIR} --target test
    COMMENT "Running all tests"
    DEPENDS ${EXTENSION_TARGETS}
)

add_custom_target(dev-clean
    COMMAND ${CMAKE_COMMAND} --build ${CMAKE_BINARY_DIR} --target clean
    COMMAND cargo clean --manifest-path ${CMAKE_CURRENT_SOURCE_DIR}/libdplyr_c/Cargo.toml
    COMMENT "Cleaning all build artifacts"
)

add_custom_target(rust-test
    COMMAND cargo test --manifest-path ${CMAKE_CURRENT_SOURCE_DIR}/libdplyr_c/Cargo.toml
    COMMENT "Running Rust unit tests"
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
)

add_custom_target(rust-clippy
    COMMAND cargo clippy --manifest-path ${CMAKE_CURRENT_SOURCE_DIR}/libdplyr_c/Cargo.toml -- -D warnings
    COMMENT "Running Rust clippy linter"
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
)

add_custom_target(rust-fmt
    COMMAND cargo fmt --manifest-path ${CMAKE_CURRENT_SOURCE_DIR}/libdplyr_c/Cargo.toml --check
    COMMENT "Checking Rust code formatting"
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
)

# R4-AC2: Combined quality check target
add_custom_target(quality-check
    DEPENDS rust-test rust-clippy rust-fmt
    COMMENT "Running all quality checks"
)

# C++ test targets
if(BUILD_CPP_TESTS)
    add_custom_target(cpp-test
        COMMAND ${CMAKE_COMMAND} -E env
            "DUCKDB_EXTENSION_PATH=${CMAKE_BINARY_DIR}"
            ${CMAKE_BINARY_DIR}/duckdb_extension_integration_test
        COMMENT "Running C++ integration tests"
        DEPENDS duckdb_extension_integration_test
    )

    add_custom_target(cpp-test-verbose
        COMMAND ${CMAKE_COMMAND} -E env
            "DUCKDB_EXTENSION_PATH=${CMAKE_BINARY_DIR}"
            ${CMAKE_BINARY_DIR}/duckdb_extension_integration_test --gtest_color=yes --gtest_print_time=1
        COMMENT "Running C++ integration tests with verbose output"
        DEPENDS duckdb_extension_integration_test
    )

    # Update quality-check to include C++ tests
    add_dependencies(quality-check duckdb_extension_integration_test)
endif()

# R4-AC2: Smoke test targets
add_custom_target(smoke-test
    COMMAND ${CMAKE_COMMAND} -E env
        "DUCKDB_EXTENSION_PATH=${CMAKE_BINARY_DIR}"
        ${CMAKE_CURRENT_SOURCE_DIR}/tests/run_smoke_tests.sh
    COMMENT "Running comprehensive smoke tests"
    DEPENDS ${DUCKDB_EXTENSION_TARGET}
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
)

add_custom_target(smoke-test-quick
    COMMAND ${CMAKE_COMMAND} -E env
        "DUCKDB_EXTENSION_PATH=${CMAKE_BINARY_DIR}"
        duckdb ":memory:" -c "LOAD '${EXTENSION_BINARY_PATH}'; SELECT 'Extension loaded successfully' as status;"
    COMMENT "Running quick smoke test (extension loading only)"
    DEPENDS ${DUCKDB_EXTENSION_TARGET}
)

# Update quality-check to include smoke tests
add_dependencies(quality-check smoke-test-quick)

# Print configuration summary
message(STATUS "DuckDB dplyr Extension Configuration:")
message(STATUS "  Extension Name: ${DUCKDB_EXTENSION_NAME}")
message(STATUS "  Version: ${PROJECT_VERSION}")
message(STATUS "  DuckDB Version: ${DUCKDB_VERSION} (version-agnostic design)")
message(STATUS "  Build Type: ${CMAKE_BUILD_TYPE}")
message(STATUS "  Platform: ${CMAKE_SYSTEM_NAME} ${CMAKE_SYSTEM_PROCESSOR}")
message(STATUS "  C++ Standard: ${CMAKE_CXX_STANDARD}")
message(STATUS "  Rust Toolchain: ${Rust_TOOLCHAIN}")
message(STATUS "  Rust Target: ${RUST_HOST_TARGET}")
message(STATUS "  Extension Sources: ${EXTENSION_SOURCES}")
message(STATUS "  DuckDB Include: ${DUCKDB_INCLUDE_DIR}")
message(STATUS "  DuckDB Libraries: ${DUCKDB_LIBRARIES}")
