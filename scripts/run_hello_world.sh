#!/bin/bash
set -euo pipefail

# Run the hello_world examples from `community-pr/description.yml` against a locally built extension.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
"$SCRIPT_DIR/housekeeping-logs.sh" || true

usage() {
  cat <<'USAGE'
Usage: scripts/run_hello_world.sh [--no-build]

Environment:
  DUCKDB_BIN        Path to duckdb binary (default: duckdb)
  CARGO_TARGET_DIR  Override Rust build output directory (helps when disk is full)
  SKIP_PARQUET      Set to 0 to build DuckDB's parquet extension anyway (default: 1)
  EXTENSION_STATIC_BUILD Set to 1 to statically link DuckDB into the extension (default: 0)

Notes:
  - This script runs the SQL examples (not the ASCII result tables) from description.yml.
USAGE
}

NO_BUILD=0
SKIP_PARQUET="${SKIP_PARQUET:-1}"
EXTENSION_STATIC_BUILD="${EXTENSION_STATIC_BUILD:-0}"
for arg in "$@"; do
  case "$arg" in
    --no-build) NO_BUILD=1 ;;
    --with-parquet) SKIP_PARQUET=0 ;;
    --static) EXTENSION_STATIC_BUILD=1 ;;
    -h|--help) usage; exit 0 ;;
    *)
      echo "Unknown argument: $arg" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [ ! -f "CMakeLists.txt" ] || [ ! -d "extension" ]; then
  echo "Error: run this script from the project root" >&2
  exit 1
fi

DUCKDB_BIN_ENV_SET=0
if [ "${DUCKDB_BIN+x}" = "x" ]; then
  DUCKDB_BIN_ENV_SET=1
fi
DUCKDB_BIN="${DUCKDB_BIN:-duckdb}"

AVAILABLE_MB="$(df -Pm . | tail -n 1 | awk '{print $4}')"
if [ -n "${AVAILABLE_MB:-}" ] && [ "${AVAILABLE_MB:-0}" -lt 2048 ]; then
  echo "Warning: low disk space (${AVAILABLE_MB}MB available). Builds may fail with 'No space left on device'." >&2
  echo "Tip: free disk space, or set CARGO_TARGET_DIR to a drive with space (e.g., export CARGO_TARGET_DIR=/path/target)." >&2
  echo >&2
fi

if [ "$NO_BUILD" -eq 0 ]; then
  echo "Building extension (make release)..."
  MAKE_VARS=()
  MAKE_VARS+=(EXTENSION_STATIC_BUILD="$EXTENSION_STATIC_BUILD")
  if [ "$SKIP_PARQUET" -eq 1 ]; then
    MAKE_VARS+=(EXT_RELEASE_FLAGS="-DSKIP_EXTENSIONS=parquet")
  fi

  if ! make release "${MAKE_VARS[@]}"; then
    echo >&2
    echo "Build failed. If you saw 'No space left on device':" >&2
    echo "  - Free disk space (this repo can be cleaned with: rm -rf build target)" >&2
    echo "  - Or move Rust artifacts: export CARGO_TARGET_DIR=/path/with/space/target" >&2
    echo "  - Or symlink ./build to a drive with space" >&2
    echo "  - Or skip building DuckDB's parquet extension (default): ./scripts/run_hello_world.sh" >&2
    exit 1
  fi
fi

if [ "$DUCKDB_BIN_ENV_SET" -eq 0 ] && [ -x "build/release/duckdb" ]; then
  # Prefer the DuckDB CLI built alongside the extension to avoid version mismatch.
  DUCKDB_BIN="build/release/duckdb"
fi

if ! command -v "$DUCKDB_BIN" >/dev/null 2>&1; then
  echo "Error: duckdb not found (set DUCKDB_BIN=/path/to/duckdb)" >&2
  exit 1
fi

EXT_FILE="$(find build/release/extension -name "dplyr.duckdb_extension" | head -n 1)"
if [ -z "${EXT_FILE}" ]; then
  echo "Error: extension binary not found under build/release/extension" >&2
  exit 1
fi

EXT_FILE_DIR="$(cd "$(dirname "$EXT_FILE")" && pwd)"
EXT_FILE="${EXT_FILE_DIR}/$(basename "$EXT_FILE")"

SQL_EXT_FILE="${EXT_FILE//\'/\'\'}"

echo "Using extension: ${EXT_FILE}"
echo

run_step() {
  local step_name="$1"
  local step_sql="$2"

  echo "== ${step_name} =="

  set +e
  "$DUCKDB_BIN" -unsigned <<SQL
LOAD '$SQL_EXT_FILE';

DROP TABLE IF EXISTS iris;
CREATE TABLE iris AS SELECT * FROM (VALUES
  (5.1, 3.5, 1.4, 0.2, 'setosa'),
  (4.9, 3.0, 1.4, 0.2, 'setosa'),
  (7.0, 3.2, 4.7, 1.4, 'versicolor'),
  (6.4, 3.2, 4.5, 1.5, 'versicolor'),
  (6.3, 3.3, 6.0, 2.5, 'virginica'),
  (5.8, 2.7, 5.1, 1.9, 'virginica')
) AS t(sepal_length, sepal_width, petal_length, petal_width, species);

${step_sql}
SQL
  local rc=$?
  set -e

  if [ "$rc" -ne 0 ]; then
    echo "FAILED: ${step_name} (exit ${rc})" >&2
    exit "$rc"
  fi

  echo
}

run_step "Pipeline (filter/select/arrange)" $'iris %>%\n  filter(sepal_length > 5) %>%\n  select(species, sepal_length, petal_length) %>%\n  arrange(desc(sepal_length));'

run_step "Pipeline (group_by/summarise)" $'iris %>%\n  group_by(species) %>%\n  summarise(avg_sepal = mean(sepal_length), count = n());'

run_step "Pipeline (mutate/select/arrange)" $'iris %>%\n  mutate(sepal_ratio = sepal_length / sepal_width) %>%\n  select(species, sepal_ratio) %>%\n  arrange(sepal_ratio);'

run_step "Embedded pipeline (| ... |)" $'SELECT species, COUNT(*) AS n\nFROM (| iris %>% filter(sepal_length > 5) %>% select(species) |)\nGROUP BY species\nORDER BY n DESC;'

echo
echo "OK"
