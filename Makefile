PROJ_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))

EXT_NAME = dplyr
EXT_CONFIG = ${PROJ_DIR}extension_config.cmake

# Keep Rust artifacts in a deterministic place for repeated builds.
export CARGO_TARGET_DIR ?= ${PROJ_DIR}target
export DUCKDB_EXTENSION_PATH ?= ${PROJ_DIR}build/release/extension/dplyr

include extension-ci-tools/makefiles/duckdb_extension.Makefile
