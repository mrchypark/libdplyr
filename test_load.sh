#!/bin/bash
set -e

# 1. Standard build
make release

# 2. Locate built extension
EXT_FILE=$(find build/release/extension -name "dplyr_extension.duckdb_extension" | head -n 1)
if [ -z "$EXT_FILE" ]; then
  echo "Extension binary not found!"
  exit 1
fi

# 3. Load and run a simple query
duckdb -unsigned -c "LOAD '$EXT_FILE'; SELECT * FROM dplyr('SELECT 1 as a %>% filter(a > 0)');"
echo "SUCCESS: Extension loaded and query executed!"
