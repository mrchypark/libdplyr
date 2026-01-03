PROJ_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))

EXT_NAME = dplyr
EXT_CONFIG = ${PROJ_DIR}extension_config.cmake

# Keep Rust artifacts in a deterministic place for repeated builds.
export CARGO_TARGET_DIR ?= ${PROJ_DIR}target
export DUCKDB_EXTENSION_PATH ?= ${PROJ_DIR}build/release/extension/dplyr

include extension-ci-tools/makefiles/duckdb_extension.Makefile

# WASM builds require the Rust wasm32-unknown-emscripten target.
# extension-ci-tools' WASM targets depend on `wasm_pre_build_step`, so we
# override it here (without patching the submodule) to ensure the target exists.
wasm_pre_build_step:
	@if command -v rustup >/dev/null 2>&1; then \
		toolchain="$${RUSTUP_TOOLCHAIN:-stable}"; \
		echo "Ensuring Rust target wasm32-unknown-emscripten is installed for toolchain $${toolchain}"; \
		rustup target add --toolchain "$${toolchain}" wasm32-unknown-emscripten; \
	else \
		echo "rustup not found; cannot install wasm32-unknown-emscripten target"; \
		exit 1; \
	fi
