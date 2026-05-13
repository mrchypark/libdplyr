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

-- Test 4: Pin smoke tests to the magrittr session default
SET dplyr_pipe_syntax = 'magrittr';
SELECT dplyr_pipe_syntax() AS pipe_syntax;

-- =============================================================================
-- Test Data
-- =============================================================================

-- Test 5: Create test data for dplyr operations
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

-- Test 6: Table function basic query
SELECT COUNT(*) AS cnt
FROM dplyr('test_data %>% select(id, name) %>% filter(id <= 5)') AS t;

-- Test 7: Table function with aggregation
SELECT *
FROM dplyr('test_data %>% group_by(category) %>% summarise(count = n(), avg_value = mean(value))') AS t;

-- Test 8: Magrittr lambda RHS keeps the input table bound through `.`
SELECT COUNT(*) AS cnt
FROM dplyr('test_data %>% { . %>% select(id) %>% filter(id <= 3) }') AS t;

-- Test 9: Magrittr lambda RHS supports dot placeholders in function arguments
SELECT COUNT(*) AS cnt
FROM dplyr('test_data %>% { filter(., id <= 4) %>% select(., id) }') AS t;

-- Test 10: Magrittr functional sequence RHS works as a lambda shorthand
SELECT COUNT(*) AS cnt
FROM dplyr('test_data %>% (. %>% select(id) %>% filter(id <= 2))') AS t;

-- Test 11: Magrittr RHS dot placeholder without braces remains supported
SELECT COUNT(*) AS cnt
FROM dplyr('test_data %>% filter(., id <= 5) %>% select(., id)') AS t;

-- =============================================================================
-- ParserExtension Entry Points
-- =============================================================================

-- Test 12: Implicit pipeline statement (ParserExtension triggered by `%>%`)
test_data %>% select(id) %>% filter(id <= 3);

-- Test 13: Embedded pipeline inside standard SQL
SELECT COUNT(*) AS cnt
FROM (| test_data %>% select(id) %>% filter(id <= 3) |) AS embedded;

-- =============================================================================
-- Pipe Syntax Configuration
-- =============================================================================

-- Test 14: Explicit native pipe syntax in the table function
SELECT COUNT(*) AS cnt
FROM dplyr('test_data |> select(id, name) |> filter(id <= 5)', 'native') AS t;

-- Test 15: Explicit native pipe syntax supports lambda RHS
SELECT COUNT(*) AS cnt
FROM dplyr('test_data |> (\(x) x |> select(id) |> filter(id <= 3))()', 'native') AS t;

-- Test 16: Native lambda RHS supports explicit data arguments
SELECT COUNT(*) AS cnt
FROM dplyr('test_data |> (\(x) filter(x, id <= 4) |> select(x, id))()', 'native') AS t;

-- Test 17: Session setting enables native pipe syntax by default
SET dplyr_pipe_syntax = 'native';
SELECT dplyr_pipe_syntax() AS pipe_syntax;

SELECT COUNT(*) AS cnt
FROM dplyr('test_data |> select(id) |> filter(id <= 4)') AS t;

SELECT COUNT(*) AS cnt
FROM dplyr('test_data |> (\(x) x |> select(id) |> filter(id <= 2))()') AS t;

-- Test 18: Switching back to magrittr keeps the original smoke path valid
SET dplyr_pipe_syntax = 'magrittr';
SELECT dplyr_pipe_syntax() AS pipe_syntax;

SELECT COUNT(*) AS cnt
FROM dplyr('test_data %>% select(id) %>% filter(id <= 2)') AS t;
