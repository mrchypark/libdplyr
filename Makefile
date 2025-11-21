PROJ_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))

# Configuration of extension
EXT_NAME=dplyr_extension
EXT_CONFIG=${PROJ_DIR}extension_config.cmake

# Set DuckDB source directory (from submodule)
# DUCKDB_SRCDIR=${PROJ_DIR}duckdb/

# Try to use the extension-ci-tools Makefile if available
# Note: This requires cloning the extension-ci-tools repository as a submodule
ifneq ("$(wildcard extension-ci-tools/makefiles/duckdb_extension.Makefile)","")
include extension-ci-tools/makefiles/duckdb_extension.Makefile
else
$(warning extension-ci-tools not found. Extension metadata generation may not work correctly.)
$(warning Please run: git submodule add https://github.com/duckdb/extension-ci-tools.git)
$(warning Falling back to direct CMake build...)

# Fallback simple build targets
.PHONY: all clean release debug test

all: release

release:
	mkdir -p build/release && \
	cd build/release && \
	cmake -DCMAKE_BUILD_TYPE=Release -DBUILD_DUCKDB=ON ../.. && \
	cmake --build . --config Release

debug:
	mkdir -p build/debug && \
	cd build/debug && \
	cmake -DCMAKE_BUILD_TYPE=Debug -DBUILD_DUCKDB=ON ../.. && \
	cmake --build . --config Debug

test:
	cd build/release && ctest --output-on-failure

clean:
	rm -rf build
endif
