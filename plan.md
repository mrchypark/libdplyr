# Plan (mark completed tasks with [x])

Instructions:
- Work through items in order; add one test at a time and only enough code to make it pass.
- Mark completed items with [x].

Tasks/Tests:
- [ ] Test: Add sqllogictest `test/sql/dplyr.test` that requires `dplyr` and runs a basic query.
- [ ] Rename: Use extension name `dplyr` (was `dplyr_extension`) across `extension_config.cmake`, `CMakeLists.txt`, and scripts (`test_load.sh` expects `dplyr.duckdb_extension`).
- [ ] Assets: Add `vcpkg.json` placeholder if missing for CI.
- [ ] Verify: Run `make clean && make release`, `make test`, and `./test_load.sh` with the new name.
