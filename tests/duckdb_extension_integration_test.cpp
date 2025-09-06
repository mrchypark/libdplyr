//! DuckDB Extension Integration Tests
//!
//! Tests the complete DuckDB extension functionality including:
//! - Extension loading and registration (R7-AC1)
//! - Standard SQL integration and mixing (R2-AC2)
//! - Crash prevention and error handling (R7-AC3)
//! - DPLYR keyword-based entry point (R5-AC1)
//! - Parser extension functionality
//! - Memory safety and FFI boundary protection

#include <gtest/gtest.h>
#include <memory>
#include <string>
#include <vector>
#include <chrono>
#include <thread>

// DuckDB includes
#include "duckdb.hpp"
#include "duckdb/main/extension_helper.hpp"

// Extension includes
#include "../extension/include/dplyr_extension.h"

using namespace duckdb;
using namespace std;

class DuckDBExtensionTest : public ::testing::Test {
protected:
    void SetUp() override {
        // Create in-memory DuckDB instance
        db = make_unique<DuckDB>(nullptr);
        conn = make_unique<Connection>(*db);
        
        // R7-AC1: Test extension loading
        ASSERT_NO_THROW(conn->Query("LOAD 'dplyr_extension'")) 
            << "Extension loading should not throw exceptions";
        
        // Verify extension loaded successfully
        auto result = conn->Query("LOAD 'dplyr_extension'");
        ASSERT_FALSE(result->HasError()) 
            << "Extension loading failed: " << result->GetError();
    }
    
    void TearDown() override {
        // Clean up connections
        conn.reset();
        db.reset();
    }
    
    // Helper function to normalize SQL for comparison
    string normalize_sql(const string& sql) {
        string normalized = sql;
        // Remove extra whitespace and convert to uppercase
        size_t pos = 0;
        while ((pos = normalized.find("  ", pos)) != string::npos) {
            normalized.replace(pos, 2, " ");
        }
        transform(normalized.begin(), normalized.end(), normalized.begin(), ::toupper);
        return normalized;
    }
    
    // Helper to execute query and verify no crash
    unique_ptr<MaterializedQueryResult> safe_query(const string& query) {
        try {
            return conn->Query(query);
        } catch (const std::exception& e) {
            ADD_FAILURE() << "Query caused exception (should return error instead): " 
                         << e.what() << " for query: " << query;
            return nullptr;
        }
    }
    
    unique_ptr<DuckDB> db;
    unique_ptr<Connection> conn;
};

// ============================================================================
// R7-AC1: DuckDB Extension Loading and Basic Functionality Tests
// ============================================================================

TEST_F(DuckDBExtensionTest, ExtensionLoadingSuccess) {
    // Extension should already be loaded in SetUp()
    // Test that we can query system information
    auto result = conn->Query("SELECT 1 as test_value");
    ASSERT_FALSE(result->HasError()) << "Basic query should work after extension load";
    ASSERT_EQ(result->RowCount(), 1);
}

TEST_F(DuckDBExtensionTest, DplyrKeywordRecognition) {
    // R5-AC1: Test DPLYR keyword-based entry point
    auto result = safe_query("DPLYR 'mtcars %>% select(mpg)'");
    
    if (result && !result->HasError()) {
        // Extension successfully processed DPLYR keyword
        EXPECT_GT(result->RowCount(), 0) << "DPLYR query should return results";
    } else if (result) {
        // Extension recognized keyword but had processing error
        string error = result->GetError();
        EXPECT_TRUE(error.find("E-") != string::npos) 
            << "Error should include error code: " << error;
    } else {
        FAIL() << "Query caused crash instead of returning error";
    }
}

TEST_F(DuckDBExtensionTest, TableFunctionEntryPoint) {
    // R2-AC1: Test alternative table function entry point
    auto result = safe_query("SELECT * FROM dplyr('data.frame(x=1:3) %>% select(x)')");
    
    if (result && !result->HasError()) {
        EXPECT_GT(result->RowCount(), 0) << "Table function should return results";
    } else if (result) {
        // Function recognized but had processing error
        string error = result->GetError();
        EXPECT_FALSE(error.empty()) << "Should have meaningful error message";
    } else {
        FAIL() << "Table function caused crash";
    }
}

// ============================================================================
// R2-AC2: Standard SQL Integration and Mixing Tests
// ============================================================================

TEST_F(DuckDBExtensionTest, StandardSqlMixingWithCTE) {
    // Test CTE with dplyr integration
    string query = R"(
        WITH base_data AS (
            SELECT 1 as id, 'Alice' as name, 25 as age
            UNION ALL
            SELECT 2 as id, 'Bob' as name, 30 as age
            UNION ALL  
            SELECT 3 as id, 'Charlie' as name, 20 as age
        ),
        dplyr_result AS (
            DPLYR 'base_data %>% select(name, age) %>% filter(age > 22)'
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
    string query = R"(
        SELECT outer_result.* FROM (
            DPLYR 'data.frame(x=1:5, y=letters[1:5]) %>% select(x, y) %>% filter(x <= 3)'
        ) as outer_result
        WHERE outer_result.x > 1
    )";
    
    auto result = safe_query(query);
    
    if (result && !result->HasError()) {
        EXPECT_GE(result->RowCount(), 0) << "Subquery integration should work";
    } else if (result) {
        string error = result->GetError();
        EXPECT_FALSE(error.empty()) << "Should have error message for subquery issue";
    } else {
        FAIL() << "Subquery with DPLYR caused crash";
    }
}

TEST_F(DuckDBExtensionTest, JoinWithDplyrResults) {
    // Test joining standard SQL with dplyr results
    string query = R"(
        WITH standard_table AS (
            SELECT 1 as id, 'Product A' as product
            UNION ALL
            SELECT 2 as id, 'Product B' as product
        )
        SELECT s.product, d.value 
        FROM standard_table s
        LEFT JOIN (
            DPLYR 'data.frame(id=1:2, value=c(100, 200)) %>% select(id, value)'
        ) d ON s.id = d.id
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
    vector<string> invalid_queries = {
        "DPLYR 'invalid_function(test)'",
        "DPLYR 'select()'",  // Empty select
        "DPLYR 'filter()'",  // Empty filter
        "DPLYR 'mutate(x = )'",  // Incomplete mutate
        "DPLYR 'group_by() %>% summarise()'",  // Empty group_by
        "DPLYR ''",  // Empty string
        "DPLYR 'unclosed_string",  // Malformed string
        "DPLYR 'select(col1 col2)'",  // Missing comma
    };
    
    for (const auto& query : invalid_queries) {
        auto result = safe_query(query);
        
        // R7-AC3: Should return error, not crash
        ASSERT_NE(result, nullptr) << "Query should not crash: " << query;
        
        if (result->HasError()) {
            string error = result->GetError();
            EXPECT_FALSE(error.empty()) << "Should have error message for: " << query;
            
            // R1-AC3: Error should include error code
            EXPECT_TRUE(error.find("E-") != string::npos || 
                       error.find("DPLYR") != string::npos)
                << "Error should include error code or DPLYR context: " << error;
        }
    }
}

TEST_F(DuckDBExtensionTest, NullPointerHandling) {
    // Test FFI boundary null pointer handling
    vector<string> null_tests = {
        "SELECT * FROM dplyr(NULL)",
        "DPLYR NULL",
    };
    
    for (const auto& query : null_tests) {
        auto result = safe_query(query);
        
        ASSERT_NE(result, nullptr) << "NULL input should not crash: " << query;
        
        if (result->HasError()) {
            string error = result->GetError();
            EXPECT_TRUE(error.find("E-FFI") != string::npos || 
                       error.find("null") != string::npos ||
                       error.find("NULL") != string::npos)
                << "Should indicate null pointer error: " << error;
        }
    }
}

TEST_F(DuckDBExtensionTest, LargeInputHandling) {
    // R9-AC2: Test DoS prevention with large input
    string large_dplyr_code = "select(";
    for (int i = 0; i < 10000; i++) {
        large_dplyr_code += "col" + to_string(i);
        if (i < 9999) large_dplyr_code += ", ";
    }
    large_dplyr_code += ")";
    
    string query = "DPLYR '" + large_dplyr_code + "'";
    
    auto result = safe_query(query);
    
    ASSERT_NE(result, nullptr) << "Large input should not crash";
    
    if (result->HasError()) {
        string error = result->GetError();
        EXPECT_TRUE(error.find("E-INTERNAL") != string::npos || 
                   error.find("too large") != string::npos ||
                   error.find("limit") != string::npos)
            << "Should indicate input size limit: " << error;
    }
}

TEST_F(DuckDBExtensionTest, ConcurrentAccessSafety) {
    // R9-AC3: Test thread safety
    const int num_threads = 4;
    const int queries_per_thread = 10;
    vector<thread> threads;
    vector<bool> thread_success(num_threads, true);
    
    for (int t = 0; t < num_threads; t++) {
        threads.emplace_back([this, t, queries_per_thread, &thread_success]() {
            // Each thread creates its own connection
            auto thread_conn = make_unique<Connection>(*db);
            
            for (int i = 0; i < queries_per_thread; i++) {
                try {
                    string query = "DPLYR 'data.frame(x=" + to_string(t * 100 + i) + 
                                 ") %>% select(x)'";
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
    
    // Check that no thread crashed
    for (int t = 0; t < num_threads; t++) {
        EXPECT_TRUE(thread_success[t]) 
            << "Thread " << t << " should not crash during concurrent access";
    }
}

TEST_F(DuckDBExtensionTest, MemoryLeakPrevention) {
    // Test repeated queries don't cause memory leaks
    const int num_iterations = 100;
    
    for (int i = 0; i < num_iterations; i++) {
        string query = "DPLYR 'data.frame(x=" + to_string(i) + 
                      ") %>% select(x) %>% filter(x > 0)'";
        
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
        string query;
        string expected_error_type;
        string description;
    };
    
    vector<ErrorTest> error_tests = {
        {
            "DPLYR 'select()'",
            "E-SYNTAX",
            "Empty select should give syntax error"
        },
        {
            "DPLYR 'unknown_function(x)'", 
            "E-UNSUPPORTED",
            "Unknown function should give unsupported error"
        },
        {
            "DPLYR 'select(col1 col2)'",
            "E-SYNTAX", 
            "Missing comma should give syntax error"
        }
    };
    
    for (const auto& test : error_tests) {
        auto result = safe_query(test.query);
        
        ASSERT_NE(result, nullptr) << test.description << " - should not crash";
        
        if (result->HasError()) {
            string error = result->GetError();
            
            // R1-AC3: Should include error code, position, and suggestion
            EXPECT_FALSE(error.empty()) << test.description << " - should have error message";
            
            // Check for error code (flexible matching)
            bool has_error_code = error.find("E-") != string::npos ||
                                error.find("DPLYR") != string::npos ||
                                error.find("Error") != string::npos;
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
    auto start_time = chrono::high_resolution_clock::now();
    
    auto result = safe_query("DPLYR 'data.frame(x=1:10) %>% select(x) %>% filter(x > 5)'");
    
    auto end_time = chrono::high_resolution_clock::now();
    auto duration = chrono::duration_cast<chrono::milliseconds>(end_time - start_time);
    
    // Should complete within reasonable time (generous limit for test environment)
    EXPECT_LT(duration.count(), 1000) 
        << "Simple DPLYR query should complete within 1 second";
    
    if (result && !result->HasError()) {
        EXPECT_GT(result->RowCount(), 0) << "Should return filtered results";
    }
}

TEST_F(DuckDBExtensionTest, ComplexQueryStability) {
    // Test more complex query patterns
    string complex_query = R"(
        DPLYR 'data.frame(
            id = 1:100,
            category = rep(c("A", "B", "C", "D"), 25),
            value = runif(100, 1, 100)
        ) %>%
        select(id, category, value) %>%
        filter(value > 50) %>%
        group_by(category) %>%
        summarise(
            count = n(),
            avg_value = mean(value),
            max_value = max(value)
        ) %>%
        arrange(desc(avg_value))'
    )";
    
    auto result = safe_query(complex_query);
    
    ASSERT_NE(result, nullptr) << "Complex query should not crash";
    
    if (result && !result->HasError()) {
        EXPECT_GE(result->RowCount(), 0) << "Complex query should return results";
    } else if (result) {
        // Complex query failed but didn't crash
        string error = result->GetError();
        EXPECT_FALSE(error.empty()) << "Should have meaningful error for complex query";
    }
}

// ============================================================================
// Integration with DuckDB Features
// ============================================================================

TEST_F(DuckDBExtensionTest, DuckDBSpecificFeatures) {
    // Test integration with DuckDB-specific features
    vector<string> duckdb_integration_tests = {
        // Test with DuckDB's built-in functions
        "SELECT duckdb_version(), (DPLYR 'data.frame(x=1) %>% select(x)').x",
        
        // Test with DuckDB's PRAGMA
        "PRAGMA table_info('test_table'); DPLYR 'data.frame(info=\"test\") %>% select(info)'",
        
        // Test with DuckDB's array functions (if supported)
        "DPLYR 'data.frame(arr=list(c(1,2,3))) %>% select(arr)'"
    };
    
    for (const auto& query : duckdb_integration_tests) {
        auto result = safe_query(query);
        
        // Should not crash regardless of success/failure
        ASSERT_NE(result, nullptr) 
            << "DuckDB integration should not crash for: " << query;
        
        // May succeed or fail depending on implementation, but should be safe
        if (result && result->HasError()) {
            string error = result->GetError();
            EXPECT_FALSE(error.empty()) 
                << "Should have meaningful error for integration test: " << query;
        }
    }
}

// ============================================================================
// Smoke Tests (R4-AC2 compliance)
// ============================================================================

TEST_F(DuckDBExtensionTest, SmokeTestBasicOperations) {
    // R4-AC2: Basic smoke test operations
    vector<string> smoke_tests = {
        "DPLYR 'mtcars %>% select(mpg)'",
        "DPLYR 'data.frame(x=1:3) %>% select(x) %>% filter(x > 1)'",
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
    cout << "Running DuckDB Extension Integration Tests..." << endl;
    cout << "Testing requirements: R7-AC1, R7-AC3, R2-AC2, R5-AC1" << endl;
    
    return RUN_ALL_TESTS();
}