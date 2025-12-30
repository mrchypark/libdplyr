-- DuckDB dplyr Extension Smoke Tests (SQL script)
--
-- This file is executed by `duckdb` CLI (see `tests/run_smoke_tests.sh`).
-- Keep it to valid SQL; do not use sqllogictest directives like `statement ok`.

-- =============================================================================
-- Extension Loading and Basic Verification
-- =============================================================================

-- Note: the extension is loaded by `tests/run_smoke_tests.sh` via `-cmd`.

-- Test 1: Verify extension loaded (side-effect free)
SELECT 'Extension loaded successfully' AS status;

-- Test 2: Verify no interference with standard SQL
SELECT 1 AS test_column;

-- Test 3: Verify standard SQL functions still work
SELECT COUNT(*) AS cnt FROM (VALUES (1), (2), (3)) AS t(x);

-- =============================================================================
-- Test Data
-- =============================================================================

-- Test 4: Create test data for dplyr operations
CREATE OR REPLACE TABLE test_data AS
SELECT
    i AS id,
    'name_' || i AS name,
    (i % 5) + 18 AS age,
    (i % 3) + 1 AS category,
    i * 10.5 AS value,
    CASE WHEN i % 2 = 0 THEN true ELSE false END AS active
FROM range(1, 11) AS t(i);

-- Verify test data created
SELECT COUNT(*) AS row_count FROM test_data;

-- =============================================================================
-- Table Function Entry Point
-- =============================================================================

-- Test 5: Table function basic query
SELECT COUNT(*) AS cnt
FROM dplyr('test_data %>% select(id, name) %>% filter(id <= 5)') AS t;

-- Test 6: Table function with aggregation
SELECT *
FROM dplyr('test_data %>% group_by(category) %>% summarise(count = n(), avg_value = mean(value))') AS t;

-- =============================================================================
-- ParserExtension Entry Points
-- =============================================================================

-- Test 7: Implicit pipeline statement (ParserExtension triggered by `%>%`)
test_data %>% select(id) %>% filter(id <= 3);

-- Test 8: Embedded pipeline inside standard SQL
SELECT COUNT(*) AS cnt
FROM (| test_data %>% select(id) %>% filter(id <= 3) |) AS embedded;
