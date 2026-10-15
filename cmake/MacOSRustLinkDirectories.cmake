# R4-AC1: Corrosion v0.5.1 injects the Command Line Tools SDK into imported
# Rust targets, even when CMake uses Xcode's compiler and SDK. The CLT SDK
# can be newer (e.g. SDK 27 with Xcode 26) and contain unreadable TAPI stubs.
# Let the compiler/CMAKE_OSX_SYSROOT select system libraries instead. Keep
# every other search directory, including caller-supplied SDK/library paths.
function(libdplyr_fix_macos_rust_link_directories target)
    if(NOT APPLE)
        return()
    endif()
    get_target_property(link_directories "${target}" INTERFACE_LINK_DIRECTORIES)
    if(link_directories)
        list(REMOVE_ITEM link_directories
            "/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/lib")
        set_property(TARGET "${target}" PROPERTY INTERFACE_LINK_DIRECTORIES "${link_directories}")
    endif()
endfunction()
