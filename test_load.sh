#!/bin/bash
set -e

# Keep noisy local logs out of the repo root.
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
"$ROOT_DIR/scripts/housekeeping-logs.sh" || true

# 1. Standard build
make release

# 2. Locate built extension
EXT_FILE=$(find build/release/extension -name "dplyr.duckdb_extension" | head -n 1)
if [ -z "$EXT_FILE" ]; then
  echo "Extension binary not found!"
  exit 1
fi

# 3. Load and run a simple query
duckdb -unsigned -c "LOAD '$EXT_FILE'; SELECT a, b FROM dplyr('SELECT 1 as a %>% mutate(b = a + 1)');"
echo "SUCCESS: Extension loaded and real dplyr pipeline executed!"
