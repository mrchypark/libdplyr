#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DUCKDB_VERSION="$(git -C "${ROOT_DIR}/duckdb" describe --tags --exact-match --match 'v[0-9]*' HEAD)" || {
  echo "Error: duckdb submodule HEAD must point to an exact release tag" >&2
  exit 1
}

echo "Building dplyr extension for DuckDB ${DUCKDB_VERSION}..."

# Build C++ extension
cd "${ROOT_DIR}/build/cmake"
make dplyr

# Append metadata
python3 ../../extension-ci-tools/scripts/append_extension_metadata.py \
  -l dplyr.duckdb_extension \
  -o ../../dplyr.duckdb_extension \
  -n dplyr \
  -dv "${DUCKDB_VERSION}" \
  -ev 0.4.0 \
  -p osx_arm64 \
  --abi-type CPP

cd "${ROOT_DIR}"

echo ""
echo "✅ Build complete!"
echo "Extension: dplyr.duckdb_extension"
echo ""
echo "Test with:"
echo "  duckdb -unsigned test.db"
echo "  > LOAD 'dplyr.duckdb_extension';"
echo "  > CREATE TABLE t(x INT);"
echo "  > INSERT INTO t VALUES (1), (2), (3);"
echo "  > t %>% select(x);"
