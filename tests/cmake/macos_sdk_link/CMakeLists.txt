cmake_minimum_required(VERSION 3.15)
project(macos_sdk_link_regression LANGUAGES CXX)
if(NOT APPLE)
    message(FATAL_ERROR "This regression requires Apple's native SDK and linker")
endif()
if(NOT IS_ABSOLUTE "${CMAKE_OSX_SYSROOT}")
    message(FATAL_ERROR "Pass the absolute xcrun-selected CMAKE_OSX_SYSROOT")
endif()
include("${CMAKE_CURRENT_LIST_DIR}/../../../cmake/MacOSRustLinkDirectories.cmake")

# Model the static library interface Corrosion creates, without downloading Rust
# or DuckDB. Both imported targets use the same real, locally built archive.
add_library(mock STATIC mock.cpp)
set(clt_sdk "/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/lib")
set(custom_dir "${CMAKE_CURRENT_BINARY_DIR}/custom libraries")
file(MAKE_DIRECTORY "${custom_dir}")
foreach(variant control fixed)
    add_library(rust_${variant} STATIC IMPORTED)
    set_target_properties(rust_${variant} PROPERTIES
        IMPORTED_LOCATION "${CMAKE_CURRENT_BINARY_DIR}/libmock.a"
        INTERFACE_LINK_DIRECTORIES "${clt_sdk};${custom_dir}"
        INTERFACE_LINK_LIBRARIES "System;c;m;c++")
    add_dependencies(rust_${variant} mock)
    # The control is generated for inspection only: never link against a
    # potentially incompatible CLT SDK just to test the fixed configuration.
    add_executable(${variant} EXCLUDE_FROM_ALL main.cpp)
    target_link_libraries(${variant} PRIVATE rust_${variant})
    target_link_options(${variant} PRIVATE -Wl,-v)
endforeach()
libdplyr_fix_macos_rust_link_directories(rust_fixed)
