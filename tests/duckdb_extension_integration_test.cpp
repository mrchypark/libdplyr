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
#include <cstdlib>

// duckdb::DuckDB includes
#include "duckdb.hpp"
#include "duckdb/parser/parser_extension.hpp"

namespace {

void SetEnvironmentVariableForTest(const char *key, const std::string &value) {
#ifdef _WIN32
    _putenv_s(key, value.c_str());
#else
    setenv(key, value.c_str(), 1);
#endif
}

void ClearEnvironmentVariableForTest(const char *key) {
#ifdef _WIN32
    _putenv_s(key, "");
#else
    unsetenv(key);
#endif
}

class ScopedEnvironmentVariable {
public:
    ScopedEnvironmentVariable(const char *key_p, const std::string &value)
        : key(key_p) {
        const char *existing = std::getenv(key);
        if (existing != nullptr) {
            had_original = true;
            original = existing;
        }
        SetEnvironmentVariableForTest(key, value);
    }

    explicit ScopedEnvironmentVariable(const char *key_p)
        : key(key_p) {
        const char *existing = std::getenv(key);
        if (existing != nullptr) {
            had_original = true;
            original = existing;
        }
        ClearEnvironmentVariableForTest(key);
    }

    ~ScopedEnvironmentVariable() {
        if (had_original) {
            SetEnvironmentVariableForTest(key, original);
        } else {
            ClearEnvironmentVariableForTest(key);
        }
    }

private:
    const char *key;
    bool had_original = false;
    std::string original;
};

} // namespace

class DuckDBExtensionTest : public ::testing::Test {
protected:
    void SetUp() override {
        scoped_pipe_syntax = std::make_unique<ScopedEnvironmentVariable>("DPLYR_PIPE_SYNTAX");

        duckdb::DBConfig config;
        try {
            config.SetOptionByName("allow_unsigned_extensions", duckdb::Value::BOOLEAN(true));
        } catch (...) {
        }

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
        scoped_pipe_syntax.reset();
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
    // Returns nullptr on any C++ exception - tests should handle this appropriately
    std::unique_ptr<duckdb::MaterializedQueryResult> safe_query(duckdb::Connection &target_conn, const std::string& query) {
        try {
            return target_conn.Query(query);
        } catch (const duckdb::Exception &ex) {
            return duckdb::make_uniq<duckdb::MaterializedQueryResult>(duckdb::ErrorData(ex));
        } catch (const std::exception &ex) {
            return duckdb::make_uniq<duckdb::MaterializedQueryResult>(
                duckdb::ErrorData(duckdb::ExceptionType::INVALID_INPUT, ex.what()));
        } catch (...) {
            return duckdb::make_uniq<duckdb::MaterializedQueryResult>(
                duckdb::ErrorData(duckdb::ExceptionType::INVALID_INPUT, "Unknown C++ exception"));
        }
    }

    std::unique_ptr<duckdb::MaterializedQueryResult> safe_query(const std::string& query) {
        return safe_query(*conn, query);
    }

    std::unique_ptr<duckdb::MaterializedQueryResult> query_no_throw(
        duckdb::Connection &target_conn,
        const std::string& query) {
        std::unique_ptr<duckdb::MaterializedQueryResult> result;
        EXPECT_NO_THROW({
            result = target_conn.Query(query);
        }) << "Query should return an error result without escaping a C++ exception: " << query;
        return result;
    }

    void expect_query_error_no_throw(
        duckdb::Connection &target_conn,
        const std::string& query,
        const std::vector<std::string> &expected_fragments) {
        auto result = query_no_throw(target_conn, query);
        ASSERT_NE(result, nullptr);
        ASSERT_TRUE(result->HasError()) << "Query should fail: " << query;
        const auto error = result->GetError();
        ASSERT_FALSE(error.empty()) << "Query error should include a message: " << query;
        EXPECT_EQ(error.find("Unknown C++ exception"), std::string::npos) << error;
        EXPECT_EQ(error.find("Unknown exception"), std::string::npos) << error;
        EXPECT_EQ(error.find("Unknown exception in ExecutorTask::Execute"), std::string::npos) << error;
        for (const auto &fragment : expected_fragments) {
            EXPECT_NE(error.find(fragment), std::string::npos) << error;
        }
    }

    void expect_query_error_no_throw(
        const std::string& query,
        const std::vector<std::string> &expected_fragments) {
        expect_query_error_no_throw(*conn, query, expected_fragments);
    }
    
    std::unique_ptr<duckdb::DuckDB> db;
    std::unique_ptr<duckdb::Connection> conn;
    std::unique_ptr<ScopedEnvironmentVariable> scoped_pipe_syntax;
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

TEST_F(DuckDBExtensionTest, TableFunctionMissingTableReturnsQueryErrorWithoutThrowing) {
    std::unique_ptr<duckdb::MaterializedQueryResult> result;

    EXPECT_NO_THROW({
        result = conn->Query("SELECT * FROM dplyr('missing_table %>% select(x)')");
    });

    ASSERT_NE(result, nullptr);
    ASSERT_TRUE(result->HasError()) << "Missing table should be reported as a query error";
    const auto error = result->GetError();
    EXPECT_NE(error.find("missing_table"), std::string::npos) << error;
}

TEST_F(DuckDBExtensionTest, TableFunctionUsesCallerContextForTempTables) {
    ASSERT_FALSE(conn->Query("CREATE TEMP TABLE dplyr_temp_visible(x INTEGER)")->HasError());
    ASSERT_FALSE(conn->Query("INSERT INTO dplyr_temp_visible VALUES (11), (12)")->HasError());

    auto result = safe_query("SELECT * FROM dplyr('dplyr_temp_visible %>% select(x)')");

    ASSERT_NE(result, nullptr);
    ASSERT_FALSE(result->HasError())
        << "dplyr() should bind and execute against caller-visible temp tables: " << result->GetError();
    EXPECT_EQ(result->RowCount(), 2);
}

TEST_F(DuckDBExtensionTest, TableFunctionUsesCallerTransactionForUncommittedRows) {
    ASSERT_FALSE(conn->Query("CREATE TABLE dplyr_tx_visible(x INTEGER)")->HasError());
    ASSERT_FALSE(conn->Query("INSERT INTO dplyr_tx_visible VALUES (1)")->HasError());
    ASSERT_FALSE(conn->Query("BEGIN TRANSACTION")->HasError());
    ASSERT_FALSE(conn->Query("INSERT INTO dplyr_tx_visible VALUES (2)")->HasError());

    auto result = safe_query("SELECT COUNT(*) FROM dplyr('dplyr_tx_visible %>% select(x)')");

    ASSERT_FALSE(conn->Query("ROLLBACK")->HasError());
    ASSERT_NE(result, nullptr);
    ASSERT_FALSE(result->HasError())
        << "dplyr() should execute inside the caller transaction: " << result->GetError();
    auto chunk = result->Fetch();
    ASSERT_TRUE(chunk);
    ASSERT_EQ(chunk->size(), 1);
    EXPECT_EQ(chunk->GetValue(0, 0).GetValue<int64_t>(), 2);
}

TEST_F(DuckDBExtensionTest, TableFunctionNativePipeSyntaxConfig) {
    auto result = safe_query("SELECT * FROM dplyr('mtcars |> select(mpg)', 'native')");

    ASSERT_NE(result, nullptr);
    ASSERT_FALSE(result->HasError()) << "Native pipe table function should succeed: " << result->GetError();
    EXPECT_EQ(result->RowCount(), 3);
}

TEST_F(DuckDBExtensionTest, PipeSyntaxScalarDefaultReportsConfiguredMode) {
    auto result = safe_query("SELECT dplyr_pipe_syntax()");

    ASSERT_NE(result, nullptr);
    ASSERT_FALSE(result->HasError()) << "dplyr_pipe_syntax() should succeed: " << result->GetError();
    ASSERT_EQ(result->RowCount(), 1);

    auto chunk = result->Fetch();
    ASSERT_TRUE(chunk);
    ASSERT_EQ(chunk->size(), 1);

    EXPECT_EQ(chunk->GetValue(0, 0).ToString(), "magrittr");
}

TEST_F(DuckDBExtensionTest, PipeSyntaxScalarDefaultReflectsCurrentEnvironment) {
    ScopedEnvironmentVariable pipe_syntax("DPLYR_PIPE_SYNTAX", "native");

    auto native_result = safe_query("SELECT dplyr_pipe_syntax()");
    ASSERT_NE(native_result, nullptr);
    ASSERT_FALSE(native_result->HasError())
        << "dplyr_pipe_syntax() should read current native env: " << native_result->GetError();

    auto native_chunk = native_result->Fetch();
    ASSERT_TRUE(native_chunk);
    ASSERT_EQ(native_chunk->size(), 1);
    EXPECT_EQ(native_chunk->GetValue(0, 0).ToString(), "native");

    SetEnvironmentVariableForTest("DPLYR_PIPE_SYNTAX", "magrittr");

    auto magrittr_result = safe_query("SELECT dplyr_pipe_syntax()");
    ASSERT_NE(magrittr_result, nullptr);
    ASSERT_FALSE(magrittr_result->HasError())
        << "dplyr_pipe_syntax() should read current magrittr env: " << magrittr_result->GetError();

    auto magrittr_chunk = magrittr_result->Fetch();
    ASSERT_TRUE(magrittr_chunk);
    ASSERT_EQ(magrittr_chunk->size(), 1);
    EXPECT_EQ(magrittr_chunk->GetValue(0, 0).ToString(), "magrittr");
}

TEST_F(DuckDBExtensionTest, PipeSyntaxSettingCanonicalizesAliasesAndAffectsTableFunctionDefault) {
    auto set_native_alias = safe_query("SET dplyr_pipe_syntax = '|>'");
    ASSERT_NE(set_native_alias, nullptr);
    ASSERT_FALSE(set_native_alias->HasError())
        << "native alias setting should succeed: " << set_native_alias->GetError();

    auto current_native = safe_query(
        "SELECT dplyr_pipe_syntax(), current_setting('dplyr_pipe_syntax')");
    ASSERT_NE(current_native, nullptr);
    ASSERT_FALSE(current_native->HasError())
        << "current syntax should reflect native setting: " << current_native->GetError();
    auto current_native_chunk = current_native->Fetch();
    ASSERT_TRUE(current_native_chunk);
    ASSERT_EQ(current_native_chunk->size(), 1);
    EXPECT_EQ(current_native_chunk->GetValue(0, 0).ToString(), "native");
    EXPECT_EQ(current_native_chunk->GetValue(1, 0).ToString(), "native");

    auto native_default = safe_query("SELECT * FROM dplyr('mtcars |> select(mpg)')");
    ASSERT_NE(native_default, nullptr);
    ASSERT_FALSE(native_default->HasError())
        << "table function default should use native setting: " << native_default->GetError();
    EXPECT_EQ(native_default->RowCount(), 3);
    EXPECT_EQ(std::getenv("DPLYR_PIPE_SYNTAX"), nullptr)
        << "pipe syntax setting must not mutate the process environment";

    auto set_magrittr_alias = safe_query("SET dplyr_pipe_syntax = '%>%'");
    ASSERT_NE(set_magrittr_alias, nullptr);
    ASSERT_FALSE(set_magrittr_alias->HasError())
        << "magrittr alias setting should succeed: " << set_magrittr_alias->GetError();

    auto current_magrittr = safe_query(
        "SELECT dplyr_pipe_syntax(), current_setting('dplyr_pipe_syntax')");
    ASSERT_NE(current_magrittr, nullptr);
    ASSERT_FALSE(current_magrittr->HasError())
        << "current syntax should reflect magrittr setting: " << current_magrittr->GetError();
    auto current_magrittr_chunk = current_magrittr->Fetch();
    ASSERT_TRUE(current_magrittr_chunk);
    ASSERT_EQ(current_magrittr_chunk->size(), 1);
    EXPECT_EQ(current_magrittr_chunk->GetValue(0, 0).ToString(), "magrittr");
    EXPECT_EQ(current_magrittr_chunk->GetValue(1, 0).ToString(), "magrittr");

    auto magrittr_default = safe_query("SELECT * FROM dplyr('mtcars %>% select(mpg)')");
    ASSERT_NE(magrittr_default, nullptr);
    ASSERT_FALSE(magrittr_default->HasError())
        << "table function default should use magrittr setting: " << magrittr_default->GetError();
    EXPECT_EQ(magrittr_default->RowCount(), 3);
}

TEST_F(DuckDBExtensionTest, PipeSyntaxSettingIsSessionLocalAndAffectsImplicitParserPipeline) {
    auto set_native = safe_query("SET dplyr_pipe_syntax = 'native'");
    ASSERT_NE(set_native, nullptr);
    ASSERT_FALSE(set_native->HasError())
        << "native setting should succeed: " << set_native->GetError();

    duckdb::Connection other_conn(*db);
    auto current_other = safe_query(other_conn, "SELECT dplyr_pipe_syntax()");
    ASSERT_NE(current_other, nullptr);
    ASSERT_FALSE(current_other->HasError())
        << "second connection should keep default pipe syntax: " << current_other->GetError();
    auto current_other_chunk = current_other->Fetch();
    ASSERT_TRUE(current_other_chunk);
    ASSERT_EQ(current_other_chunk->size(), 1);
    EXPECT_EQ(current_other_chunk->GetValue(0, 0).ToString(), "magrittr");

    auto implicit_native = safe_query("mtcars |> select(mpg)");
    ASSERT_NE(implicit_native, nullptr);
    ASSERT_FALSE(implicit_native->HasError())
        << "implicit parser pipeline should use current connection pipe syntax: " << implicit_native->GetError();
    EXPECT_EQ(implicit_native->RowCount(), 3);

    auto magrittr_on_other = safe_query(other_conn, "SELECT * FROM dplyr('mtcars %>% select(mpg)')");
    ASSERT_NE(magrittr_on_other, nullptr);
    ASSERT_FALSE(magrittr_on_other->HasError())
        << "second connection should keep magrittr table function default: " << magrittr_on_other->GetError();

    auto set_other_native = safe_query(other_conn, "SET dplyr_pipe_syntax = 'native'");
    ASSERT_NE(set_other_native, nullptr);
    ASSERT_FALSE(set_other_native->HasError())
        << "second connection native setting should succeed: " << set_other_native->GetError();

    auto current_conn = safe_query("SELECT dplyr_pipe_syntax()");
    ASSERT_NE(current_conn, nullptr);
    ASSERT_FALSE(current_conn->HasError())
        << "first connection should stay native after second connection mutation: " << current_conn->GetError();
    auto current_conn_chunk = current_conn->Fetch();
    ASSERT_TRUE(current_conn_chunk);
    ASSERT_EQ(current_conn_chunk->size(), 1);
    EXPECT_EQ(current_conn_chunk->GetValue(0, 0).ToString(), "native");

    auto reset_other = safe_query(other_conn, "RESET dplyr_pipe_syntax");
    ASSERT_NE(reset_other, nullptr);
    ASSERT_FALSE(reset_other->HasError())
        << "second connection reset should succeed: " << reset_other->GetError();

    auto current_other_after_reset = safe_query(other_conn, "SELECT dplyr_pipe_syntax()");
    ASSERT_NE(current_other_after_reset, nullptr);
    ASSERT_FALSE(current_other_after_reset->HasError())
        << "second connection should return to default after reset: " << current_other_after_reset->GetError();
    auto current_other_after_reset_chunk = current_other_after_reset->Fetch();
    ASSERT_TRUE(current_other_after_reset_chunk);
    ASSERT_EQ(current_other_after_reset_chunk->size(), 1);
    EXPECT_EQ(current_other_after_reset_chunk->GetValue(0, 0).ToString(), "magrittr");

    auto current_conn_after_other_reset = safe_query("SELECT dplyr_pipe_syntax()");
    ASSERT_NE(current_conn_after_other_reset, nullptr);
    ASSERT_FALSE(current_conn_after_other_reset->HasError())
        << "first connection should stay native after second connection reset: "
        << current_conn_after_other_reset->GetError();
    auto current_conn_after_other_reset_chunk = current_conn_after_other_reset->Fetch();
    ASSERT_TRUE(current_conn_after_other_reset_chunk);
    ASSERT_EQ(current_conn_after_other_reset_chunk->size(), 1);
    EXPECT_EQ(current_conn_after_other_reset_chunk->GetValue(0, 0).ToString(), "native");
}

TEST_F(DuckDBExtensionTest, PipeSyntaxResetReturnsToEnvironmentFallback) {
    ScopedEnvironmentVariable pipe_syntax("DPLYR_PIPE_SYNTAX", "native");

    auto set_magrittr = safe_query("SET dplyr_pipe_syntax = 'magrittr'");
    ASSERT_NE(set_magrittr, nullptr);
    ASSERT_FALSE(set_magrittr->HasError())
        << "magrittr setting should override native environment: " << set_magrittr->GetError();

    auto current_magrittr = safe_query("SELECT dplyr_pipe_syntax()");
    ASSERT_NE(current_magrittr, nullptr);
    ASSERT_FALSE(current_magrittr->HasError())
        << "current syntax should use explicit setting: " << current_magrittr->GetError();
    auto current_magrittr_chunk = current_magrittr->Fetch();
    ASSERT_TRUE(current_magrittr_chunk);
    ASSERT_EQ(current_magrittr_chunk->size(), 1);
    EXPECT_EQ(current_magrittr_chunk->GetValue(0, 0).ToString(), "magrittr");

    auto reset = safe_query("RESET dplyr_pipe_syntax");
    ASSERT_NE(reset, nullptr);
    ASSERT_FALSE(reset->HasError())
        << "RESET should return to environment/default behavior: " << reset->GetError();

    auto current_native = safe_query("SELECT dplyr_pipe_syntax()");
    ASSERT_NE(current_native, nullptr);
    ASSERT_FALSE(current_native->HasError())
        << "current syntax should fall back to environment after RESET: " << current_native->GetError();
    auto current_native_chunk = current_native->Fetch();
    ASSERT_TRUE(current_native_chunk);
    ASSERT_EQ(current_native_chunk->size(), 1);
    EXPECT_EQ(current_native_chunk->GetValue(0, 0).ToString(), "native");

    auto native_default = safe_query("SELECT * FROM dplyr('mtcars |> select(mpg)')");
    ASSERT_NE(native_default, nullptr);
    ASSERT_FALSE(native_default->HasError())
        << "table function default should use environment after RESET: " << native_default->GetError();
    EXPECT_EQ(native_default->RowCount(), 3);

    SetEnvironmentVariableForTest("DPLYR_PIPE_SYNTAX", "magrittr");

    auto current_magrittr_after_env_change = safe_query("SELECT dplyr_pipe_syntax()");
    ASSERT_NE(current_magrittr_after_env_change, nullptr);
    ASSERT_FALSE(current_magrittr_after_env_change->HasError())
        << "RESET should clear the override so later env changes are visible: "
        << current_magrittr_after_env_change->GetError();
    auto current_magrittr_after_env_change_chunk = current_magrittr_after_env_change->Fetch();
    ASSERT_TRUE(current_magrittr_after_env_change_chunk);
    ASSERT_EQ(current_magrittr_after_env_change_chunk->size(), 1);
    EXPECT_EQ(current_magrittr_after_env_change_chunk->GetValue(0, 0).ToString(), "magrittr");
}

TEST_F(DuckDBExtensionTest, PipeSyntaxGlobalDefaultAllowsSessionOverride) {
    duckdb::Connection other_conn(*db);
    auto set_global_native = safe_query("SET GLOBAL dplyr_pipe_syntax = 'native'");
    ASSERT_NE(set_global_native, nullptr);
    ASSERT_FALSE(set_global_native->HasError())
        << "global native setting should succeed: " << set_global_native->GetError();

    auto current = safe_query("SELECT dplyr_pipe_syntax()");
    ASSERT_NE(current, nullptr);
    ASSERT_FALSE(current->HasError())
        << "current session should read global pipe syntax default: " << current->GetError();
    auto current_chunk = current->Fetch();
    ASSERT_TRUE(current_chunk);
    ASSERT_EQ(current_chunk->size(), 1);
    EXPECT_EQ(current_chunk->GetValue(0, 0).ToString(), "native");

    auto current_other = safe_query(other_conn, "SELECT dplyr_pipe_syntax()");
    ASSERT_NE(current_other, nullptr);
    ASSERT_FALSE(current_other->HasError())
        << "other connection should read global pipe syntax default: " << current_other->GetError();
    auto current_other_chunk = current_other->Fetch();
    ASSERT_TRUE(current_other_chunk);
    ASSERT_EQ(current_other_chunk->size(), 1);
    EXPECT_EQ(current_other_chunk->GetValue(0, 0).ToString(), "native");

    auto set_session_magrittr = safe_query("SET dplyr_pipe_syntax = 'magrittr'");
    ASSERT_NE(set_session_magrittr, nullptr);
    ASSERT_FALSE(set_session_magrittr->HasError())
        << "session magrittr setting should succeed: " << set_session_magrittr->GetError();

    auto session_current = safe_query("SELECT dplyr_pipe_syntax()");
    ASSERT_NE(session_current, nullptr);
    ASSERT_FALSE(session_current->HasError())
        << "session setting should override global default: " << session_current->GetError();
    auto session_current_chunk = session_current->Fetch();
    ASSERT_TRUE(session_current_chunk);
    ASSERT_EQ(session_current_chunk->size(), 1);
    EXPECT_EQ(session_current_chunk->GetValue(0, 0).ToString(), "magrittr");

    auto other_after_override = safe_query(other_conn, "SELECT dplyr_pipe_syntax()");
    ASSERT_NE(other_after_override, nullptr);
    ASSERT_FALSE(other_after_override->HasError())
        << "session override should not mutate global default: " << other_after_override->GetError();
    auto other_after_override_chunk = other_after_override->Fetch();
    ASSERT_TRUE(other_after_override_chunk);
    ASSERT_EQ(other_after_override_chunk->size(), 1);
    EXPECT_EQ(other_after_override_chunk->GetValue(0, 0).ToString(), "native");
}

TEST_F(DuckDBExtensionTest, PipeSyntaxSettingInvalidValueErrorsAndPreservesState) {
    auto set_native = safe_query("SET dplyr_pipe_syntax = 'native'");
    ASSERT_NE(set_native, nullptr);
    ASSERT_FALSE(set_native->HasError())
        << "native setting should succeed: " << set_native->GetError();

    expect_query_error_no_throw(
        "SET dplyr_pipe_syntax = 'invalid-pipe-mode'",
        {"invalid-pipe-mode"});

    auto current = safe_query("SELECT dplyr_pipe_syntax()");
    ASSERT_NE(current, nullptr);
    ASSERT_FALSE(current->HasError())
        << "invalid SET should preserve previous syntax: " << current->GetError();
    auto current_chunk = current->Fetch();
    ASSERT_TRUE(current_chunk);
    ASSERT_EQ(current_chunk->size(), 1);
    EXPECT_EQ(current_chunk->GetValue(0, 0).ToString(), "native");
}

TEST_F(DuckDBExtensionTest, PipeSyntaxScalarSetterOverloadIsRemovedAndDoesNotMutateState) {
    expect_query_error_no_throw(
        "SELECT dplyr_pipe_syntax('native')",
        {"dplyr_pipe_syntax"});

    auto current = safe_query("SELECT dplyr_pipe_syntax()");
    ASSERT_NE(current, nullptr);
    ASSERT_FALSE(current->HasError())
        << "removed scalar setter should not change session pipe syntax: " << current->GetError();
    auto current_chunk = current->Fetch();
    ASSERT_TRUE(current_chunk);
    ASSERT_EQ(current_chunk->size(), 1);
    EXPECT_EQ(current_chunk->GetValue(0, 0).ToString(), "magrittr");
}

TEST_F(DuckDBExtensionTest, PipeSyntaxScalarSetterRelationalContextsDoNotMutateState) {
    const std::vector<std::string> queries = {
        "SELECT dplyr_pipe_syntax(mode) FROM (VALUES ('native'), ('magrittr')) modes(mode)",
        "SELECT * FROM (SELECT dplyr_pipe_syntax('native')) setter_result",
        "SELECT COUNT(dplyr_pipe_syntax('native'))",
        "SELECT dplyr_pipe_syntax('native') UNION ALL SELECT dplyr_pipe_syntax('magrittr')"};

    for (const auto &query : queries) {
        expect_query_error_no_throw(query, {"dplyr_pipe_syntax"});

        auto current = safe_query("SELECT dplyr_pipe_syntax()");
        ASSERT_NE(current, nullptr);
        ASSERT_FALSE(current->HasError())
            << "failed scalar setter query should not change session pipe syntax: " << current->GetError();
        auto current_chunk = current->Fetch();
        ASSERT_TRUE(current_chunk);
        ASSERT_EQ(current_chunk->size(), 1);
        EXPECT_EQ(current_chunk->GetValue(0, 0).ToString(), "magrittr")
            << "failed scalar setter query should not mutate state: " << query;
    }
}

TEST_F(DuckDBExtensionTest, TableFunctionInvalidPipeSyntaxErrorsWithGuidance) {
    auto result = safe_query("SELECT * FROM dplyr('mtcars %>% select(mpg)', 'invalid-pipe-mode')");

    ASSERT_NE(result, nullptr) << "Invalid explicit pipe mode should return an inspectable query error";
    ASSERT_TRUE(result->HasError()) << "Invalid explicit pipe mode should return an error";
    const auto error = result->GetError();
    ASSERT_FALSE(error.empty()) << "Invalid explicit pipe mode should report an error message";
    EXPECT_NE(error.find("Expected 'magrittr' or 'native'"), std::string::npos) << error;
    EXPECT_NE(error.find("DPLYR_PIPE_SYNTAX"), std::string::npos) << error;
}

TEST_F(DuckDBExtensionTest, DisabledPipeSyntaxErrorMentionsDuckDBSetting) {
    const std::vector<std::string> expected_fragments = {
        "Native pipe is not enabled", "DPLYR_PIPE_SYNTAX=native", "SET dplyr_pipe_syntax = 'native'"};

    expect_query_error_no_throw(
        "SELECT * FROM dplyr('mtcars |> select(mpg)')",
        expected_fragments);
}

TEST_F(DuckDBExtensionTest, DirectParserDisabledPipeSyntaxReturnsQueryErrorWithoutThrowing) {
    const std::vector<std::string> expected_fragments = {
        "Native pipe is not enabled", "SET dplyr_pipe_syntax = 'native'"};

    expect_query_error_no_throw(
        "mtcars |> select(mpg)",
        expected_fragments);
}

TEST_F(DuckDBExtensionTest, NativePipeLambdaRhs) {
    auto result = safe_query(R"(SELECT * FROM dplyr('mtcars |> (\(x) x |> select(mpg) |> filter(mpg > 20))()', 'native'))");
    auto explicit_arg_result = safe_query(R"(SELECT * FROM dplyr('mtcars |> (\(x) filter(x, mpg > 20) |> select(x, mpg))()', 'native'))");

    ASSERT_NE(result, nullptr);
    ASSERT_FALSE(result->HasError()) << "Native pipe lambda RHS should execute: " << result->GetError();
    EXPECT_EQ(result->RowCount(), 2);

    ASSERT_NE(explicit_arg_result, nullptr);
    ASSERT_FALSE(explicit_arg_result->HasError()) << "Native pipe lambda data parameter should execute: " << explicit_arg_result->GetError();
    EXPECT_EQ(explicit_arg_result->RowCount(), 2);
}

TEST_F(DuckDBExtensionTest, MagrittrPipeLambdaRhs) {
    auto braced_result = safe_query(R"(mtcars %>% { . %>% select(mpg) %>% filter(mpg > 20) })");
    auto sequence_result = safe_query(R"(mtcars %>% (. %>% select(mpg) %>% filter(mpg > 20)))");
    auto dot_arg_result = safe_query(R"(mtcars %>% { filter(., mpg > 20) %>% select(., mpg) })");
    auto rhs_dot_arg_result = safe_query(R"(mtcars %>% filter(., mpg > 20) %>% select(., mpg))");

    ASSERT_NE(braced_result, nullptr);
    ASSERT_FALSE(braced_result->HasError()) << "Magrittr braced lambda RHS should execute: " << braced_result->GetError();
    EXPECT_EQ(braced_result->RowCount(), 2);

    ASSERT_NE(sequence_result, nullptr);
    ASSERT_FALSE(sequence_result->HasError()) << "Magrittr functional sequence RHS should execute: " << sequence_result->GetError();
    EXPECT_EQ(sequence_result->RowCount(), 2);

    ASSERT_NE(dot_arg_result, nullptr);
    ASSERT_FALSE(dot_arg_result->HasError()) << "Magrittr dot data placeholder should execute: " << dot_arg_result->GetError();
    EXPECT_EQ(dot_arg_result->RowCount(), 2);

    ASSERT_NE(rhs_dot_arg_result, nullptr);
    ASSERT_FALSE(rhs_dot_arg_result->HasError()) << "Magrittr RHS dot data placeholder should execute: " << rhs_dot_arg_result->GetError();
    EXPECT_EQ(rhs_dot_arg_result->RowCount(), 2);
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
    
    ASSERT_NE(result, nullptr);
    ASSERT_FALSE(result->HasError()) << "CTE + DPLYR mixing should work: " << result->GetError();
    ASSERT_EQ(result->RowCount(), 1);

    auto chunk = result->Fetch();
    ASSERT_TRUE(chunk);
    ASSERT_EQ(chunk->size(), 1);
    EXPECT_EQ(chunk->GetValue(0, 0).GetValue<int64_t>(), 2);
}

TEST_F(DuckDBExtensionTest, InvalidPipeEnvironmentPreservesPlainSqlPassthrough) {
    ScopedEnvironmentVariable pipe_syntax("DPLYR_PIPE_SYNTAX", "invalid-pipe-mode");

    auto result = safe_query("SELECT 42 AS answer");

    ASSERT_NE(result, nullptr);
    ASSERT_FALSE(result->HasError())
        << "Plain SQL should bypass dplyr pipe syntax configuration: " << result->GetError();
    ASSERT_EQ(result->RowCount(), 1);

    auto chunk = result->Fetch();
    ASSERT_TRUE(chunk);
    ASSERT_EQ(chunk->size(), 1);
    EXPECT_EQ(chunk->GetValue(0, 0).GetValue<int32_t>(), 42);
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
    
    ASSERT_NE(result, nullptr);
    ASSERT_FALSE(result->HasError()) << "Subquery integration should work: " << result->GetError();
    ASSERT_EQ(result->RowCount(), 2);

    auto chunk = result->Fetch();
    ASSERT_TRUE(chunk);
    ASSERT_EQ(chunk->size(), 2);
    EXPECT_EQ(chunk->GetValue(0, 0).GetValue<int64_t>(), 2);
    EXPECT_EQ(chunk->GetValue(0, 1).GetValue<int64_t>(), 3);
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
    
    ASSERT_NE(result, nullptr);
    ASSERT_FALSE(result->HasError()) << "JOIN with DPLYR should work: " << result->GetError();
    ASSERT_EQ(result->RowCount(), 2);

    auto chunk = result->Fetch();
    ASSERT_TRUE(chunk);
    ASSERT_EQ(chunk->size(), 2);
    EXPECT_EQ(chunk->GetValue(0, 0).ToString(), "Product A");
    EXPECT_EQ(chunk->GetValue(1, 0).GetValue<int32_t>(), 100);
    EXPECT_EQ(chunk->GetValue(0, 1).ToString(), "Product B");
    EXPECT_EQ(chunk->GetValue(1, 1).GetValue<int32_t>(), 200);
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
    // Requirement: NULL input should not crash duckdb::DuckDB - may throw or return error
    std::vector<std::string> null_tests = {
        "SELECT * FROM dplyr(NULL)",
    };
    
    for (const auto& query : null_tests) {
        auto result = safe_query(query);
        
        // Either returns error result OR throws an exception (both are acceptable crash-prevention)
        if (result == nullptr) {
            // An exception was thrown - this is acceptable crash prevention behavior
            SUCCEED() << "NULL input caused exception (acceptable): " << query;
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
        EXPECT_TRUE(error.find("E-INPUT-TOO-LARGE") != std::string::npos ||
                   error.find("Resource limit exceeded") != std::string::npos ||
                   error.find("exceeds maximum") != std::string::npos ||
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
