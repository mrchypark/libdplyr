# Plan (mark completed tasks with [x])

Instructions:
- Work through items in order; add one test at a time and only enough code to make it pass.
- Mark completed items with [x].

Tasks/Tests:
- [x] Test: Extend `test/sql/dplyr.test` to cover DPLYR keyword and implicit pipeline syntax (require dplyr).
- [x] Version: Keep DuckDB baseline at v1.4.2 (remove override to v2.0.0; metadata script back to v1.4.2).
- [x] Verify: Run `make release`, `make test`, and `./test_load.sh` after the version adjustment.
- [x] CI: Make CMake rustup toolchain detection work on CI runners that only provide the plain `stable` toolchain alias (Corrosion FindRust failure).
- [x] Verify: Configure the project locally to confirm the Rust toolchain detection no longer errors.
- [x] Metadata: Align extension compatibility metadata (min/tested versions, public header) to DuckDB v1.4.2 baseline.
- [x] Verify: Reconfigure CMake to confirm the compatibility banner reports 1.4.2.
