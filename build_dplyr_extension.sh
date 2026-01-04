#!/bin/bash
set -e

echo "Building dplyr extension..."

# Build C++ extension
cd build/cmake
make dplyr

# Append metadata
python3 ../../extension-ci-tools/scripts/append_extension_metadata.py \
  -l dplyr.duckdb_extension \
  -o ../../dplyr.duckdb_extension \
  -n dplyr \
  -dv v1.4.2 \
  -ev 0.2.0 \
  -p osx_arm64 \
  --abi-type CPP

cd ../..

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
