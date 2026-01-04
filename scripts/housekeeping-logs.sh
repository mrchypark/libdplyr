#!/bin/bash
set -euo pipefail

# Collect noisy log artifacts into a single directory and optionally purge old logs.
#
# Environment:
#   LOG_DIR            Where to store logs (default: logs)
#   LOG_RETENTION_DAYS Delete logs older than this many days (default: 14)
#
# Examples:
#   ./scripts/housekeeping-logs.sh
#   LOG_DIR=logs LOG_RETENTION_DAYS=7 ./scripts/housekeeping-logs.sh
#   ./scripts/housekeeping-logs.sh --dry-run

usage() {
  cat <<'USAGE'
Usage: scripts/housekeeping-logs.sh [--dry-run] [--no-collect] [--no-purge]

Collects known noisy log/artifact files from the repo root into LOG_DIR and
optionally deletes old files inside LOG_DIR.
USAGE
}

DRY_RUN=0
NO_COLLECT=0
NO_PURGE=0

for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=1 ;;
    --no-collect) NO_COLLECT=1 ;;
    --no-purge) NO_PURGE=1 ;;
    -h|--help) usage; exit 0 ;;
    *)
      echo "Unknown argument: $arg" >&2
      usage >&2
      exit 2
      ;;
  esac
done

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

LOG_DIR="${LOG_DIR:-logs}"
LOG_RETENTION_DAYS="${LOG_RETENTION_DAYS:-14}"

mkdir -p "$LOG_DIR"

timestamp="$(date +"%Y%m%d-%H%M%S")"

move_one() {
  local src="$1"
  local dest_dir="$2"

  if [ ! -e "$src" ]; then
    return 0
  fi
  if [ -d "$src" ]; then
    return 0
  fi

  local base
  base="$(basename "$src")"
  local dest="$dest_dir/$base"

  # Avoid overwriting: if the destination exists, append a timestamp.
  if [ -e "$dest" ]; then
    dest="$dest_dir/${base%.log}.${timestamp}.${base##*.}"
    if [ "$dest" = "$dest_dir/$base" ]; then
      dest="$dest_dir/${base}.${timestamp}"
    fi
  fi

  if [ "$DRY_RUN" -eq 1 ]; then
    echo "would move: $src -> $dest"
    return 0
  fi

  mv "$src" "$dest"
}

collect_root_artifacts() {
  # Only collect from repo root to avoid touching build system internals.
  # Add patterns here when new noisy artifacts appear.
  shopt -s nullglob

  local candidates=(
    debug.log
    *.log
    *_failure.log
    *_log.txt
    failed_log_attempt*.txt
    full_failed_log.txt
    all_jobs.json
    jobs_*.json
    *_job_id.txt
  )

  for pattern in "${candidates[@]}"; do
    for path in $pattern; do
      move_one "$path" "$LOG_DIR"
    done
  done
}

purge_old_logs() {
  local days="$1"
  if ! [[ "$days" =~ ^[0-9]+$ ]]; then
    echo "LOG_RETENTION_DAYS must be an integer (got: $days)" >&2
    exit 2
  fi

  if [ "$DRY_RUN" -eq 1 ]; then
    find "$LOG_DIR" -type f -mtime "+$days" -print | sed 's/^/would delete: /'
    return 0
  fi

  find "$LOG_DIR" -type f -mtime "+$days" -print -delete || true
}

if [ "$NO_COLLECT" -eq 0 ]; then
  collect_root_artifacts
fi

if [ "$NO_PURGE" -eq 0 ]; then
  purge_old_logs "$LOG_RETENTION_DAYS"
fi
