#!/usr/bin/env bash
# Install the versioned libdplyr release asset under DuckDB's canonical extension name.

set -euo pipefail

SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

detect_platform() {
    case "$(uname -s)" in
        Linux*)
            printf '%s\n' "linux-x86_64"
            ;;
        Darwin*)
            case "$(uname -m)" in
                arm64) printf '%s\n' "macos-arm64" ;;
                *) printf '%s\n' "macos-x86_64" ;;
            esac
            ;;
        CYGWIN*|MINGW*|MSYS*)
            printf '%s\n' "windows-x86_64"
            ;;
        *)
            echo "Unsupported platform: $(uname -s)" >&2
            return 1
            ;;
    esac
}

PLATFORM="${DPLYR_PLATFORM:-$(detect_platform)}"
case "$PLATFORM" in
    linux-x86_64|macos-x86_64|macos-arm64|windows-x86_64) ;;
    *)
        echo "Unsupported DPLYR_PLATFORM: $PLATFORM" >&2
        echo "Expected one of: linux-x86_64, macos-x86_64, macos-arm64, windows-x86_64" >&2
        exit 1
        ;;
esac

if [[ "$PLATFORM" == "windows-x86_64" ]]; then
    DUCKDB_COMMAND=(duckdb.exe)
elif [[ "$PLATFORM" == "macos-x86_64" && "$(uname -s)" == "Darwin" && "$(uname -m)" == "arm64" ]]; then
    DUCKDB_COMMAND=(arch -x86_64 duckdb)
else
    DUCKDB_COMMAND=(duckdb)
fi

if [[ "$PLATFORM" == "windows-x86_64" ]]; then
    DUCKDB_EXECUTABLE="duckdb.exe"
else
    DUCKDB_EXECUTABLE="duckdb"
fi

if ! command -v "$DUCKDB_EXECUTABLE" >/dev/null 2>&1; then
    echo "$DUCKDB_EXECUTABLE CLI is required and must be available in PATH." >&2
    exit 1
fi

DETECTED_VERSION="$("${DUCKDB_COMMAND[@]}" --version | grep -Eo 'v?[0-9]+[.][0-9]+[.][0-9]+' | head -n 1 || true)"
if [[ -z "$DETECTED_VERSION" ]]; then
    echo "Could not determine the installed DuckDB version." >&2
    exit 1
fi
DETECTED_VERSION="v${DETECTED_VERSION#v}"

DUCKDB_VERSION="${DUCKDB_VERSION:-$DETECTED_VERSION}"
DUCKDB_VERSION="v${DUCKDB_VERSION#v}"

if [[ "$DUCKDB_VERSION" != "$DETECTED_VERSION" ]]; then
    echo "DuckDB version mismatch: requested $DUCKDB_VERSION but found $DETECTED_VERSION." >&2
    exit 1
fi

echo "Detected DuckDB version: $DETECTED_VERSION"

EXTENSION_FILE="$SCRIPT_DIR/dplyr-${DUCKDB_VERSION}-${PLATFORM}.duckdb_extension"
CANONICAL_EXTENSION="$SCRIPT_DIR/dplyr.duckdb_extension"

if [[ ! -f "$EXTENSION_FILE" ]]; then
    echo "Extension file for platform $PLATFORM and DuckDB $DUCKDB_VERSION not found!" >&2
    echo "Available binaries for platform $PLATFORM:" >&2
    find "$SCRIPT_DIR" -maxdepth 1 -name "dplyr-*-${PLATFORM}.duckdb_extension" -print >&2
    exit 1
fi

echo "Installing DuckDB dplyr extension for $PLATFORM..."
echo "Extension file: $EXTENSION_FILE"
cp "$EXTENSION_FILE" "$CANONICAL_EXTENSION"

SQL_EXTENSION_PATH="$CANONICAL_EXTENSION"
if command -v cygpath >/dev/null 2>&1; then
    SQL_EXTENSION_PATH="$(cygpath -m "$CANONICAL_EXTENSION")"
fi
SQL_EXTENSION_PATH="$(printf '%s' "$SQL_EXTENSION_PATH" | sed "s/'/''/g")"

"${DUCKDB_COMMAND[@]}" -unsigned -bail :memory: \
    -cmd "FORCE INSTALL '$SQL_EXTENSION_PATH'; LOAD dplyr;" \
    -c "SELECT extension_name, loaded, installed FROM duckdb_extensions() WHERE extension_name = 'dplyr';"

echo "Installation completed successfully!"
echo "Start DuckDB with: ${DUCKDB_COMMAND[*]} -unsigned"
echo "Then run: LOAD dplyr;"
echo "For implicit pipelines, also run: SET allow_parser_override_extension = 'fallback';"
