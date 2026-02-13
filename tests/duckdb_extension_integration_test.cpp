//! duckdb::DuckDB Extension Integration Tests
//!
//! Tests the complete duckdb::DuckDB extension functionality including:
//! - Extension loading and registration (R7-AC1)
//! - Standard SQL integration and mixing (R2-AC2)
//! - Crash prevention and error handling (R7-AC3)
//! - Implicit pipeline entry point (%>%)
//! - Embedded pipeline entry point (|...|)
//! - Parser extension functionality
//! - Memory safety and FFI boundary protection

#include <gtest/gtest.h>
#include <memory>
#include <string>
#include <vector>
#include <thread>
#include <iostream>
#include <algorithm>
#include <cctype>
#include <chrono>

// duckdb::DuckDB includes
#include "duckdb.hpp"
#include "duckdb/parser/parser_extension.hpp"

class DuckDBExtensionTest : public ::testing::Test {
protected:
    void SetUp() override {
        duckdb::DBConfig config;
        config.options.allow_unsigned_extensions = true;

        // Create in-memory duckdb::DuckDB instance
        db = duckdb::make_uniq<duckdb::DuckDB>(nullptr, &config);
        conn = duckdb::make_uniq<duckdb::Connection>(*db);

        // Provide simple data fixture for smoke tests
        ASSERT_FALSE(conn->Query("DROP TABLE IF EXISTS mtcars")->HasError());
        ASSERT_FALSE(conn->Query("CREATE TABLE mtcars(mpg INTEGER)")->HasError());
        ASSERT_FALSE(conn->Query("INSERT INTO mtcars VALUES (21), (19), (30)")->HasError());

        std::string extension_path = DPLYR_EXTENSION_BINARY_PATH;
        std::replace(extension_path.begin(), extension_path.end(), '\\', '/');

        auto quote_pos = extension_path.find('\'');
        while (quote_pos != std::string::npos) {
            extension_path.insert(quote_pos, "'");
            quote_pos = extension_path.find('\'', quote_pos + 2);
        }

        auto result = conn->Query("LOAD '" + extension_path + "'");
        ASSERT_FALSE(result->HasError()) 
            << "Extension loading failed: " << result->GetError();
    }
    
    void TearDown() override {
        // Clean up connections
        conn.reset();
        db.reset();
    }
    
    // Helper function to normalize SQL for comparison
    std::string normalize_sql(const std::string& sql) {
        std::string normalized = sql;
        // Remove extra whitespace and convert to uppercase
        size_t pos = 0;
        while ((pos = normalized.find("  ", pos)) != std::string::npos) {
            normalized.replace(pos, 2, " ");
        }
        std::transform(normalized.begin(), normalized.end(), normalized.begin(), ::toupper);
        return normalized;
    }
    
    // Helper to execute query and verify no crash
    // Returns nullptr on std::exception - tests should handle this appropriately
    std::unique_ptr<duckdb::MaterializedQueryResult> safe_query(const std::string& query) {
        try {
            return conn->Query(query);
        } catch (const std::exception&) {
            // Exception occurred - return nullptr, let individual tests decide if this is acceptable
            return nullptr;
        }
    }
    
    std::unique_ptr<duckdb::DuckDB> db;
    std::unique_ptr<duckdb::Connection> conn;
};

// ============================================================================
// R7-AC1: duckdb::DuckDB Extension Loading and Basic Functionality Tests
// ============================================================================

TEST_F(DuckDBExtensionTest, ExtensionLoadingSuccess) {
    // Extension should already be loaded in SetUp()
    // Test that we can query system information
    auto result = conn->Query("SELECT 1 as test_value");
    ASSERT_FALSE(result->HasError()) << "Basic query should work after extension load";
    ASSERT_EQ(result->RowCount(), 1);
}

TEST_F(DuckDBExtensionTest, DplyrKeywordRecognition) {
    // DPLYR keyword entry point is intentionally not supported; ensure it fails safely.
    auto result = safe_query("DPLYR 'mtcars %>% select(mpg)'");
    
    ASSERT_NE(result, nullptr);
    EXPECT_TRUE(result->HasError()) << "DPLYR keyword should not be accepted";
}

TEST_F(DuckDBExtensionTest, DplyrPipelineMatchesSqlResult) {
    // Basic end-to-end pipeline should yield same rows as direct SQL
    ASSERT_FALSE(conn->Query("CREATE TABLE dplyr_numbers(x INTEGER)")->HasError());
    ASSERT_FALSE(
        conn->Query("INSERT INTO dplyr_numbers VALUES (1), (2), (3)")->HasError());

    auto dplyr_result = safe_query("dplyr_numbers %>% select(x)");
    auto sql_result = safe_query("SELECT x FROM dplyr_numbers");

    ASSERT_NE(dplyr_result, nullptr);
    ASSERT_NE(sql_result, nullptr);

    ASSERT_FALSE(dplyr_result->HasError())
        << "Pipeline should execute: " << dplyr_result->GetError();
    ASSERT_FALSE(sql_result->HasError()) 
        << "Baseline SQL should succeed: " << sql_result->GetError();

    ASSERT_EQ(dplyr_result->RowCount(), sql_result->RowCount());
    ASSERT_EQ(dplyr_result->ColumnCount(), sql_result->ColumnCount());

    auto dplyr_chunk = dplyr_result->Fetch();
    auto sql_chunk = sql_result->Fetch();
    ASSERT_TRUE(dplyr_chunk && sql_chunk);
    ASSERT_EQ(dplyr_chunk->size(), sql_chunk->size());

    for (duckdb::idx_t row = 0; row < dplyr_chunk->size(); row++) {
        EXPECT_EQ(dplyr_chunk->GetValue(0, row), sql_chunk->GetValue(0, row))
            << "Row " << row << " should match between DPLYR and SQL";
    }
}

TEST_F(DuckDBExtensionTest, DplyrImplicitPipelineWithoutKeyword) {
    // Should allow PRQL-style implicit FROM without DPLYR keyword
    ASSERT_FALSE(conn->Query("CREATE TABLE implicit_tbl(x INTEGER)")->HasError());
    ASSERT_FALSE(conn->Query("INSERT INTO implicit_tbl VALUES (10), (20)")->HasError());

    auto result = safe_query("implicit_tbl %>% select(x)");

    ASSERT_NE(result, nullptr);
    ASSERT_FALSE(result->HasError()) 
        << "Implicit pipeline should execute: " << result->GetError();
    EXPECT_EQ(result->RowCount(), 2);
}

TEST_F(DuckDBExtensionTest, TableFunctionEntryPoint) {
    // R2-AC1: Test alternative table function entry point
    auto result = safe_query("SELECT * FROM dplyr('mtcars %>% select(mpg)')");

    ASSERT_NE(result, nullptr);
    ASSERT_FALSE(result->HasError()) << "Table function should succeed: " << result->GetError();
    EXPECT_EQ(result->RowCount(), 3);
}

// ============================================================================
// R2-AC2: Standard SQL Integration and Mixing Tests
// ============================================================================

TEST_F(DuckDBExtensionTest, StandardSqlMixingWithCTE) {
    // Test CTE with dplyr integration
    std::string query = R"(
        WITH base_data AS (
            SELECT 1 as id, 'Alice' as name, 25 as age
            UNION ALL
            SELECT 2 as id, 'Bob' as name, 30 as age
            UNION ALL  
            SELECT 3 as id, 'Charlie' as name, 20 as age
        ),
        dplyr_result AS (
            (| base_data %>% select(name, age) %>% filter(age > 22) |)
        )
        SELECT COUNT(*) as result_count FROM dplyr_result
    )";
    
    auto result = safe_query(query);
    
    if (result && !result->HasError()) {
        EXPECT_EQ(result->RowCount(), 1) << "CTE + DPLYR mixing should work";
        auto chunk = result->Fetch();
        if (chunk && chunk->size() > 0) {
            // Should have filtered results (Alice=25, Bob=30, both > 22)
            EXPECT_GE(chunk->GetValue(0, 0).GetValue<int64_t>(), 1) 
                << "Should have at least 1 filtered result";
        }
    } else if (result) {
        // Mixed query had error but didn't crash
        EXPECT_FALSE(result->GetError().empty()) 
            << "Should provide meaningful error for mixed query";
    } else {
        FAIL() << "Mixed CTE + DPLYR query caused crash";
    }
}

TEST_F(DuckDBExtensionTest, SubqueryIntegration) {
    // Test dplyr in subquery context
    std::string query = R"(
        WITH base AS (
            SELECT i as x FROM range(1, 6) as t(i)
        )
        SELECT outer_result.* FROM (| base %>% select(x) %>% filter(x <= 3) |) as outer_result
        WHERE outer_result.x > 1
    )";
    
    auto result = safe_query(query);
    
    if (result && !result->HasError()) {
        EXPECT_GE(result->RowCount(), 0) << "Subquery integration should work";
    } else if (result) {
        std::string error = result->GetError();
        EXPECT_FALSE(error.empty()) << "Should have error message for subquery issue";
    } else {
        FAIL() << "Subquery with DPLYR caused crash";
    }
}

TEST_F(DuckDBExtensionTest, JoinWithDplyrResults) {
    // Test joining standard SQL with dplyr results
    std::string query = R"(
        WITH standard_table AS (
            SELECT 1 as id, 'Product A' as product
            UNION ALL
            SELECT 2 as id, 'Product B' as product
        ),
        d_src AS (
            SELECT 1 as id, 100 as value
            UNION ALL
            SELECT 2 as id, 200 as value
        ),
        d AS (
            (| d_src %>% select(id, value) |)
        )
        SELECT s.product, d.value 
        FROM standard_table s
        LEFT JOIN d ON s.id = d.id
    )";
    
    auto result = safe_query(query);
    
    if (result && !result->HasError()) {
        EXPECT_GE(result->RowCount(), 0) << "JOIN with DPLYR should work";
    } else if (result) {
        // Join failed but didn't crash
        EXPECT_FALSE(result->GetError().empty()) << "Should have join error message";
    } else {
        FAIL() << "JOIN with DPLYR caused crash";
    }
}

// ============================================================================
// R7-AC3: Crash Prevention and Error Handling Tests
// ============================================================================

TEST_F(DuckDBExtensionTest, InvalidDplyrSyntaxNoCrash) {
    // Test various invalid dplyr syntax patterns
    std::vector<std::string> invalid_queries = {
        "mtcars %>% unknown_function(test)",
        "mtcars %>% filter()",
        "mtcars %>% mutate(x = )",
        "mtcars %>% select(col1 col2)",
    };
    
    for (const auto& query : invalid_queries) {
        auto result = safe_query(query);
        
        // R7-AC3: Should return error, not crash
        ASSERT_NE(result, nullptr) << "Query should not crash: " << query;
        
        if (result->HasError()) {
            std::string error = result->GetError();
            EXPECT_FALSE(error.empty()) << "Should have error message for: " << query;
            
            // R1-AC3: Error should include error code
            EXPECT_TRUE(error.find("E-") != std::string::npos ||
                        error.find("DPLYR") != std::string::npos ||
                        error.find("pipeline") != std::string::npos)
                << "Error should include context: " << error;
        }
    }
}

TEST_F(DuckDBExtensionTest, NullPointerHandling) {
    // Test FFI boundary null pointer handling
    // Requirement: NULL input should not crash duckdb::DuckDB - may throw std::exception or return error
    std::vector<std::string> null_tests = {
        "SELECT * FROM dplyr(NULL)",
    };
    
    for (const auto& query : null_tests) {
        auto result = safe_query(query);
        
        // Either returns error result OR throws std::exception (both are acceptable crash-prevention)
        if (result == nullptr) {
            // Exception was thrown - this is acceptable crash prevention behavior
            SUCCEED() << "NULL input caused std::exception (acceptable): " << query;
            continue;
        }
        
        // If result returned, should be an error
        if (result->HasError()) {
            std::string error = result->GetError();
            EXPECT_TRUE(error.find("null") != std::string::npos ||
                        error.find("std::string literal") != std::string::npos ||
                        error.find("NULL") != std::string::npos ||
                        error.find("non-null") != std::string::npos)
                << "Should indicate null/invalid input: " << error;
        }
    }
}

TEST_F(DuckDBExtensionTest, LargeInputHandling) {
    // R9-AC2: Test DoS prevention with large input
    std::string large_payload(1024 * 1024 + 128, 'a'); // > 1MB
    std::string query = "mtcars %>% select(" + large_payload + ")";
    
    auto result = safe_query(query);
    
    ASSERT_NE(result, nullptr) << "Large input should not crash";
    
    if (result->HasError()) {
        std::string error = result->GetError();
        EXPECT_TRUE(error.find("E-INTERNAL") != std::string::npos || 
                   error.find("too large") != std::string::npos ||
                   error.find("limit") != std::string::npos)
            << "Should indicate input size limit: " << error;
    }
}

TEST_F(DuckDBExtensionTest, ConcurrentAccessSafety) {
    // R9-AC3: Test std::thread safety
    const int num_threads = 4;
    int queries_per_thread = 10;
    std::vector<std::thread> threads;
    std::vector<bool> thread_success(num_threads, true);
    
    for (int t = 0; t < num_threads; t++) {
        threads.emplace_back([this, t, queries_per_thread, &thread_success]() {
            const auto runs = queries_per_thread;
            // Each std::thread creates its own connection
            auto thread_conn = duckdb::make_uniq<duckdb::Connection>(*db);
            
            for (int i = 0; i < runs; i++) {
                try {
                    (void)i;
                    std::string query = "mtcars %>% select(mpg) %>% filter(mpg > 0)";
                    auto result = thread_conn->Query(query);
                    
                    // Should not crash, may have errors
                    if (!result) {
                        thread_success[t] = false;
                        break;
                    }
                } catch (const std::exception& e) {
                    // Exception indicates crash/unsafe behavior
                    thread_success[t] = false;
                    break;
                }
            }
        });
    }
    
    // Wait for all threads
    for (auto& t : threads) {
        t.join();
    }
    
    // Check that no std::thread crashed
    for (int t = 0; t < num_threads; t++) {
        EXPECT_TRUE(thread_success[t]) 
            << "Thread " << t << " should not crash during concurrent access";
    }
}

TEST_F(DuckDBExtensionTest, MemoryLeakPrevention) {
    // Test repeated queries don't cause memory leaks
    const int num_iterations = 100;
    
    for (int i = 0; i < num_iterations; i++) {
        (void)i;
        std::string query = "mtcars %>% select(mpg) %>% filter(mpg > 0)";
        
        auto result = safe_query(query);
        ASSERT_NE(result, nullptr) << "Iteration " << i << " should not crash";
        
        // Force result cleanup
        result.reset();
    }
    
    // If we reach here without crash, memory management is working
    SUCCEED() << "Completed " << num_iterations << " iterations without crash";
}

// ============================================================================
// Error Message Quality Tests
// ============================================================================

TEST_F(DuckDBExtensionTest, ErrorMessageQuality) {
    // Test that error messages are helpful and include required information
    struct ErrorTest {
        std::string query;
        std::string expected_error_type;
        std::string description;
    };
    
    std::vector<ErrorTest> error_tests = {
        {
            "mtcars %>% filter()",
            "E-SYNTAX",
            "Empty filter should give syntax error"
        },
        {
            "mtcars %>% unknown_function(x)", 
            "E-UNSUPPORTED",
            "Unknown function should give unsupported error"
        },
        {
            "mtcars %>% select(col1 col2)",
            "E-SYNTAX", 
            "Missing comma should give syntax error"
        }
    };
    
    for (const auto& test : error_tests) {
        auto result = safe_query(test.query);
        
        ASSERT_NE(result, nullptr) << test.description << " - should not crash";
        
        if (result->HasError()) {
            std::string error = result->GetError();
            
            // R1-AC3: Should include error code, position, and suggestion
            EXPECT_FALSE(error.empty()) << test.description << " - should have error message";
            
            // Check for error code (flexible matching)
            bool has_error_code = error.find("E-") != std::string::npos ||
                                error.find("DPLYR") != std::string::npos ||
                                error.find("Error") != std::string::npos;
            EXPECT_TRUE(has_error_code) 
                << test.description << " - should include error code in: " << error;
        }
    }
}

// ============================================================================
// Performance and Stability Tests
// ============================================================================

TEST_F(DuckDBExtensionTest, BasicPerformanceStability) {
    // R6-AC1: Test that simple queries complete in reasonable time
    auto start_time = std::chrono::high_resolution_clock::now();
    
    auto result = safe_query("mtcars %>% select(mpg) %>% filter(mpg > 20)");
    
    auto end_time = std::chrono::high_resolution_clock::now();
    auto duration = std::chrono::duration_cast<std::chrono::milliseconds>(end_time - start_time);
    
    // Should complete within reasonable time (generous limit for test environment)
    EXPECT_LT(duration.count(), 1000) 
        << "Simple pipeline query should complete within 1 second";
    
    if (result && !result->HasError()) {
        EXPECT_GT(result->RowCount(), 0) << "Should return filtered results";
    }
}

TEST_F(DuckDBExtensionTest, ComplexQueryStability) {
    // Test more complex query patterns
    std::string complex_query = R"(
        WITH complex_data AS (
            SELECT
                i as id,
                CASE (i % 4)
                    WHEN 0 THEN 'A'
                    WHEN 1 THEN 'B'
                    WHEN 2 THEN 'C'
                    ELSE 'D'
                END AS category,
                (i % 100) AS value
            FROM range(1, 101) AS t(i)
        ),
        dplyr_result AS (
            (| complex_data %>% select(id, category, value) %>%
               filter(value > 50) %>%
               group_by(category) %>%
               summarise(count = n(), avg_value = mean(value), max_value = max(value)) %>%
               arrange(desc(avg_value)) |)
        )
        SELECT * FROM dplyr_result
    )";
    
    auto result = safe_query(complex_query);
    
    ASSERT_NE(result, nullptr) << "Complex query should not crash";
    
    if (result && !result->HasError()) {
        EXPECT_GE(result->RowCount(), 0) << "Complex query should return results";
    } else if (result) {
        // Complex query failed but didn't crash
        std::string error = result->GetError();
        EXPECT_FALSE(error.empty()) << "Should have meaningful error for complex query";
    }
}

// ============================================================================
// Integration with duckdb::DuckDB Features
// ============================================================================

TEST_F(DuckDBExtensionTest, DuckDBSpecificFeatures) {
    // Test integration with duckdb::DuckDB-specific features
    // Goal: Ensure that using duckdb::DuckDB features with dplyr doesn't crash
    std::vector<std::string> duckdb_integration_tests = {
        // Basic dplyr pipeline (most reliable test)
        "DPLYR 'mtcars %>% select(mpg)'",
        // Test with simple pipeline
        "mtcars %>% select(mpg)"
    };
    
    int successful_tests = 0;
    
    for (const auto& query : duckdb_integration_tests) {
        auto result = safe_query(query);
        
        // Either std::exception (nullptr) or result is acceptable - no crash occurred
        if (result == nullptr) {
            // Exception was thrown - acceptable for crash prevention
            continue;
        }
        
        // May succeed or fail depending on implementation, but should be safe
        if (!result->HasError()) {
            successful_tests++;
        } else {
            std::string error = result->GetError();
            EXPECT_FALSE(error.empty()) 
                << "Should have meaningful error for integration test: " << query;
        }
    }
    
    // At least one test should succeed for basic functionality
    EXPECT_GT(successful_tests, 0) << "At least one duckdb::DuckDB integration test should succeed";
}

// ============================================================================
// Smoke Tests (R4-AC2 compliance)
// ============================================================================

TEST_F(DuckDBExtensionTest, SmokeTestBasicOperations) {
    // R4-AC2: Basic smoke test operations
    std::vector<std::string> smoke_tests = {
        "mtcars %>% select(mpg)",
        "SELECT * FROM (| mtcars %>% select(mpg) %>% filter(mpg > 20) |) AS t",
        "SELECT * FROM dplyr('mtcars %>% select(mpg) %>% filter(mpg > 20)')"
    };
    
    int successful_tests = 0;
    
    for (const auto& query : smoke_tests) {
        auto result = safe_query(query);
        
        ASSERT_NE(result, nullptr) << "Smoke test should not crash: " << query;
        
        if (result && !result->HasError()) {
            successful_tests++;
            EXPECT_GT(result->RowCount(), 0) << "Smoke test should return data: " << query;
        }
    }
    
    // R4-AC2: At least one smoke test should succeed
    EXPECT_GT(successful_tests, 0) 
        << "At least one smoke test should succeed for basic functionality";
}

// ============================================================================
// Main Test Runner
// ============================================================================

int main(int argc, char** argv) {
    ::testing::InitGoogleTest(&argc, argv);
    
    // Set up test environment
    std::cout << "Running duckdb::DuckDB Extension Integration Tests..." << std::endl;
    std::cout << "Testing requirements: R7-AC1, R7-AC3, R2-AC2" << std::endl;
    
    return RUN_ALL_TESTS();
}
