# Plan (mark completed tasks with [x])

Instructions:
- Work through items in order; add one test at a time and only enough code to make it pass.
- Mark completed items with [x].

Tasks/Tests:
- [x] Test: `test_load.sh` builds via `make release`, loads the unsigned extension, and runs a simple DPLYR query.
- [x] Build: Simplify `Makefile` to include `extension-ci-tools/makefiles/duckdb_extension.Makefile`, exporting needed Rust env vars.
- [x] Build: Refactor `CMakeLists.txt` to rely on parent-provided DuckDB (isolate FetchContent to standalone) and link the static `libdplyr_c`.
- [x] Verify: Run `test_load.sh` to see the success message and update checkboxes.
