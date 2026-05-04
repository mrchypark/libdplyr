#define DUCKDB_EXTENSION_MAIN
/**
 * @file dplyr_extension.cpp
 * @brief DuckDB extension implementation for dplyr transpilation
 * 
 * This file implements the DuckDB extension that integrates libdplyr
 * transpilation capabilities into DuckDB. It provides both parser extension
 * and table function entry points as specified in requirements R2-AC1.
 */

#include "duckdb.hpp"
#include "duckdb/parser/parser_extension.hpp"
// Note: Parser.hpp removed - use Connection::Query for parsing validation
#include "duckdb/parser/statement/extension_statement.hpp"
#include "duckdb/planner/binder.hpp"
#include "duckdb/common/exception.hpp"
#include "duckdb/main/extension/extension_loader.hpp"
#if defined(__has_include)
#if __has_include("duckdb/main/extension_callback_manager.hpp")
#include "duckdb/main/extension_callback_manager.hpp"
#endif
#endif
#include "duckdb/common/string_util.hpp"
#include "duckdb/function/table_function.hpp"
#include "duckdb/main/materialized_query_result.hpp"
#include "duckdb/main/client_context.hpp"
#include "duckdb/common/types/column/column_data_collection.hpp"
#include "../include/dplyr_extension.hpp"
#include <cstdint>
#include <memory>
#include <string>
#include <sstream>
#include <stdexcept>
#include <cstdlib> // for std::getenv
#include <algorithm> // for std::transform
#include <cctype> // for ::tolower
#include <vector>
#include <chrono> // for timestamps
#include <ctime> // for localtime_r/localtime_s
#include <iomanip> // for std::put_time
// #include <optional> - removed unused header

using namespace duckdb;

namespace dplyr {

/**
 * @brief Error handler for converting C API errors to DuckDB exceptions
 * 
 * Implements R7-AC3: Crash prevention with safe error handling
 * Implements R1-AC3: Error code, cause token, position, and alternatives
 */
class DplyrErrorHandler {
public:
    /**
     * @brief Convert C API error to appropriate DuckDB exception
     * 
     * @param error_code Error code from dplyr_compile
     * @param error_message Error message from dplyr_compile
     * @param dplyr_code Original dplyr code for context
     * @throws Appropriate DuckDB exception type
     */
    static void handle_error(int error_code, const string& error_message, const string& dplyr_code) {
        // R7-AC3: Prevent crashes by handling all error types safely
        try {
            string formatted_message = format_error_message(error_code, error_message, dplyr_code);
            
            switch (error_code) {
                case DPLYR_ERROR_NULL_POINTER:
                case DPLYR_ERROR_INVALID_UTF8:
                    // FFI-related errors - input validation issues
                    throw InvalidInputException(formatted_message);
                    
                case DPLYR_ERROR_INPUT_TOO_LARGE:
                case DPLYR_ERROR_TIMEOUT:
                    // Resource limit errors - DoS prevention
                    throw InvalidInputException("Resource limit exceeded: " + formatted_message);
                    
                case DPLYR_ERROR_SYNTAX:
                    // Syntax errors in dplyr code
                    throw ParserException("DPLYR syntax error: " + formatted_message);
                    
                case DPLYR_ERROR_UNSUPPORTED:
                    // Unsupported operations
                    throw NotImplementedException("DPLYR unsupported operation: " + formatted_message);
                    
                case DPLYR_ERROR_INTERNAL:
                case DPLYR_ERROR_PANIC:
                    // Internal errors - should not crash DuckDB
                    throw InternalException("DPLYR internal error: " + formatted_message);
                    
                default:
                    // Unknown error codes
                    throw InternalException("DPLYR unknown error (code " + std::to_string(error_code) + "): " + formatted_message);
            }
        } catch (const std::exception& format_error) {
            // R7-AC3: Even error formatting should not crash
            throw InternalException("DPLYR error handling failed: " + string(format_error.what()));
        }
    }
    
    /**
     * @brief Check if error is recoverable
     * 
     * @param error_code Error code to check
     * @return true if error is recoverable, false if fatal
     */
    static bool is_recoverable_error(int error_code) {
        return dplyr_is_recoverable_error(error_code);
    }
    
    /**
     * @brief Get error category for logging
     * 
     * @param error_code Error code
     * @return Error category string
     */
    static string get_error_category(int error_code) {
        switch (error_code) {
            case DPLYR_ERROR_NULL_POINTER:
            case DPLYR_ERROR_INVALID_UTF8:
                return "INPUT_VALIDATION";
                
            case DPLYR_ERROR_INPUT_TOO_LARGE:
            case DPLYR_ERROR_TIMEOUT:
                return "RESOURCE_LIMIT";
                
            case DPLYR_ERROR_SYNTAX:
                return "SYNTAX_ERROR";
                
            case DPLYR_ERROR_UNSUPPORTED:
                return "UNSUPPORTED_OPERATION";
                
            case DPLYR_ERROR_INTERNAL:
            case DPLYR_ERROR_PANIC:
                return "INTERNAL_ERROR";
                
            default:
                return "UNKNOWN_ERROR";
        }
    }

private:
    /**
     * @brief Format error message with context information
     * 
     * @param error_code Error code from C API
     * @param error_message Raw error message
     * @param dplyr_code Original dplyr code for context
     * @return Formatted error message with context
     */
    static string format_error_message(int error_code, const string& error_message, const string& dplyr_code) {
        // R1-AC3: Include error code, cause, position, and alternatives
        string formatted = error_message;
        
        string error_name = dplyr_error_code_name(error_code);
        if (!error_name.empty()) {
            formatted = "[";
            formatted.append(error_name);
            formatted.append("] ");
            formatted.append(error_message);
        }
        
        // Add context information for syntax errors
        if (error_code == DPLYR_ERROR_SYNTAX || error_code == DPLYR_ERROR_UNSUPPORTED) {
            formatted += "\n\nInput code: " + truncate_code_for_display(dplyr_code);
            formatted += "\n\nSuggestions:";
            formatted += get_error_suggestions(error_code, error_message);
        }
        
        // Add recovery information
        if (is_recoverable_error(error_code)) {
            formatted += "\n\nThis error is recoverable. You can try again with corrected input.";
        } else {
            formatted += "\n\nThis is a fatal error. Please check your system configuration.";
        }
        
        return formatted;
    }
    
    /**
     * @brief Truncate code for display in error messages
     * 
     * @param code Original dplyr code
     * @return Truncated code suitable for error display
     */
    static string truncate_code_for_display(const string& code) {
        const size_t MAX_DISPLAY_LENGTH = 200;
        
        if (code.length() <= MAX_DISPLAY_LENGTH) {
            return "'" + code + "'";
        }
        
        return "'" + code.substr(0, MAX_DISPLAY_LENGTH - 3) + "...'";
    }
    
    /**
     * @brief Get suggestions based on error type and message
     * 
     * @param error_code Error code
     * @param error_message Error message
     * @return Suggestion string
     */
    static string get_error_suggestions(int error_code, [[maybe_unused]] const string& error_message) {
        string suggestions;
        
        switch (error_code) {
            case DPLYR_ERROR_SYNTAX:
                suggestions += "\n  - Check dplyr function syntax (select, filter, mutate, etc.)";
                suggestions += "\n  - Ensure proper use of pipe operator (%>%)";
                if (error_message.find("Native pipe is not enabled") != string::npos) {
                    suggestions += "\n  - Set DPLYR_PIPE_SYNTAX=native before starting DuckDB";
                    suggestions += "\n  - For the table function, pass 'native' as the second argument";
                } else if (error_message.find("Magrittr pipe is not enabled") != string::npos) {
                    suggestions += "\n  - Set DPLYR_PIPE_SYNTAX=magrittr before starting DuckDB";
                    suggestions += "\n  - For the table function, pass 'magrittr' as the second argument";
                }
                suggestions += "\n  - Verify column names and function arguments";
                suggestions += "\n  - Check for balanced parentheses and quotes";
                break;
                
            case DPLYR_ERROR_UNSUPPORTED:
                suggestions += "\n  - Use supported dplyr functions: select, filter, mutate, arrange, summarise, group_by";
                suggestions += "\n  - Check if the operation is supported in DuckDB dialect";
                suggestions += "\n  - Consider breaking complex operations into simpler steps";
                break;
                
            case DPLYR_ERROR_INPUT_TOO_LARGE:
                suggestions += "\n  - Reduce the length of your dplyr code";
                suggestions += "\n  - Break complex pipelines into multiple steps";
                suggestions += "\n  - Current limit: " + std::to_string(dplyr_max_input_length()) + " characters";
                break;
                
            case DPLYR_ERROR_TIMEOUT:
                suggestions += "\n  - Simplify your dplyr pipeline";
                suggestions += "\n  - Avoid deeply nested operations";
                suggestions += "\n  - Current timeout: " + std::to_string(dplyr_max_processing_time_ms()) + "ms";
                break;
                
            default:
                suggestions += "\n  - Check the dplyr documentation for correct syntax";
                suggestions += "\n  - Try a simpler version of your pipeline first";
                break;
        }
        
        // Add common suggestions for all error types
        suggestions += "\n  - Enable debug mode with DPLYR_DEBUG=1 for more details";
        
        return suggestions;
    }
};

/**
 * @brief Debug logging system for DPLYR extension
 * 
 * Implements R10-AC1: Environment variable and session option debug mode toggle
 * Integrates with DuckDB logging system with timestamps and categories
 */
class DplyrDebugLogger {
public:
    enum class LogLevel : uint8_t {
        ERROR = 0,
        WARNING = 1,
        INFO = 2,
        DEBUG = 3,
        TRACE = 4
    };
    
    enum class LogCategory : uint8_t {
        GENERAL,
        PARSER,
        TRANSPILER,
        CACHE,
        ERROR_HANDLING,
        PERFORMANCE
    };
    
    /**
     * @brief Check if debug mode is enabled
     * 
     * @return true if debug mode is enabled via environment variable
     */
    static bool is_debug_enabled() {
        const char* debug_env = std::getenv("DPLYR_DEBUG");
        return debug_env != nullptr && (std::string(debug_env) == "1" || std::string(debug_env) == "true");
    }
    
    /**
     * @brief Get current log level from environment
     * 
     * @return Current log level
     */
    static LogLevel get_log_level() {
        const char* level_env = std::getenv("DPLYR_LOG_LEVEL");
        if (level_env == nullptr) {
            return is_debug_enabled() ? LogLevel::DEBUG : LogLevel::WARNING;
        }
        
        std::string level_str = level_env;
        std::transform(level_str.begin(), level_str.end(), level_str.begin(), ::toupper);
        
        if (level_str == "ERROR") {
            return LogLevel::ERROR;
        }
        if (level_str == "WARNING" || level_str == "WARN") {
            return LogLevel::WARNING;
        }
        if (level_str == "INFO") {
            return LogLevel::INFO;
        }
        if (level_str == "DEBUG") {
            return LogLevel::DEBUG;
        }
        if (level_str == "TRACE") {
            return LogLevel::TRACE;
        }
        
        return LogLevel::WARNING; // Default
    }
    
    /**
     * @brief Log message with timestamp and category
     * 
     * @param level Log level
     * @param category Log category
     * @param message Log message
     */
    static void log(LogLevel level, LogCategory category, const string& message) {
        if (level > get_log_level()) {
            return; // Skip if log level is too low
        }
        
        // R10-AC1: Timestamp and category-based logging
        auto now = std::chrono::system_clock::now();
        auto time_t = std::chrono::system_clock::to_time_t(now);
        const int MS_PER_SECOND = 1000;
        auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(
            now.time_since_epoch()) % MS_PER_SECOND;
        
        std::tm tm_snapshot{};
        if (!try_get_local_time(time_t, tm_snapshot)) {
            tm_snapshot = {};
        }

        std::stringstream timestamp;
        timestamp << std::put_time(&tm_snapshot, "%Y-%m-%d %H:%M:%S");
        timestamp << "." << std::setfill('0') << std::setw(3) << ms.count();
        
        const string level_str = log_level_to_string(level);
        const string category_str = log_category_to_string(category);
        
        // Format: [TIMESTAMP] [LEVEL] [CATEGORY] MESSAGE
        std::cerr << "[" << timestamp.str() << "] "
                  << "[" << level_str << "] "
                  << "[DPLYR:" << category_str << "] "
                  << message << '\n';
    }
    
    /**
     * @brief Log error with context
     * 
     * @param category Log category
     * @param message Error message
     * @param context Additional context information
     */
    static void log_error(LogCategory category, const string& message, const string& context = "") {
        string full_message = message;
        if (!context.empty()) {
            full_message += " | Context: " + context;
        }
        log(LogLevel::ERROR, category, full_message);
    }
    
    /**
     * @brief Log warning with context
     * 
     * @param category Log category
     * @param message Warning message
     * @param context Additional context information
     */
    static void log_warning(LogCategory category, const string& message, const string& context = "") {
        string full_message = message;
        if (!context.empty()) {
            full_message += " | Context: " + context;
        }
        log(LogLevel::WARNING, category, full_message);
    }
    
    /**
     * @brief Log info message
     * 
     * @param category Log category
     * @param message Info message
     */
    static void log_info(LogCategory category, const string& message) {
        log(LogLevel::INFO, category, message);
    }
    
    /**
     * @brief Log debug message
     * 
     * @param category Log category
     * @param message Debug message
     */
    static void log_debug(LogCategory category, const string& message) {
        log(LogLevel::DEBUG, category, message);
    }
    
    /**
     * @brief Log performance metrics
     * 
     * @param operation Operation name
     * @param duration_ms Duration in milliseconds
     * @param additional_info Additional performance info
     */
    static void log_performance(const string& operation, double duration_ms, const string& additional_info = "") {
        if (get_log_level() < LogLevel::DEBUG) {
            return;
        }
        
        string message = "Performance: " + operation + " took " + std::to_string(duration_ms) + "ms";
        if (!additional_info.empty()) {
            message += " | " + additional_info;
        }
        
        log(LogLevel::DEBUG, LogCategory::PERFORMANCE, message);
    }
    
private:
    static bool try_get_local_time(std::time_t t, std::tm &out) {
#ifdef _WIN32
        return localtime_s(&out, &t) == 0;
#else
        return localtime_r(&t, &out) != nullptr;
#endif
    }

    /**
     * @brief Convert log level to string
     * 
     * @param level Log level
     * @return String representation
     */
    static string log_level_to_string(LogLevel level) {
        switch (level) {
            case LogLevel::ERROR: return "ERROR";
            case LogLevel::WARNING: return "WARN";
            case LogLevel::INFO: return "INFO";
            case LogLevel::DEBUG: return "DEBUG";
            case LogLevel::TRACE: return "TRACE";
            default: return "UNKNOWN";
        }
    }
    
    /**
     * @brief Convert log category to string
     * 
     * @param category Log category
     * @return String representation
     */
    static string log_category_to_string(LogCategory category) {
        switch (category) {
            case LogCategory::GENERAL: return "GENERAL";
            case LogCategory::PARSER: return "PARSER";
            case LogCategory::TRANSPILER: return "TRANSPILER";
            case LogCategory::CACHE: return "CACHE";
            case LogCategory::ERROR_HANDLING: return "ERROR";
            case LogCategory::PERFORMANCE: return "PERF";
            default: return "UNKNOWN";
        }
    }
};

// DplyrParseData is defined in the header

enum class QueryCompileStatus {
    Success,
    NotHandled,
    Error
};

static DplyrOptions DefaultDplyrOptions() {
    DplyrOptions options = dplyr_options_default();
    options.dialect = DPLYR_DIALECT_DUCKDB;
    if (DplyrDebugLogger::is_debug_enabled()) {
        options.debug_mode = true;
    }
    return options;
}

static bool ParsePipeSyntaxOption(const string& value, uint32_t &pipe_syntax, string &error_out) {
    auto normalized = StringUtil::Lower(value);
    StringUtil::Trim(normalized);

    if (normalized == "magrittr" || normalized == "%>%") {
        pipe_syntax = DPLYR_PIPE_SYNTAX_MAGRITTR;
        return true;
    }
    if (normalized == "native" || normalized == "|>") {
        pipe_syntax = DPLYR_PIPE_SYNTAX_NATIVE;
        return true;
    }

    error_out = "Invalid dplyr pipe syntax '" + value + "'. Expected 'magrittr' or 'native'.";
    return false;
}

struct DefaultPipeSyntaxResult {
    QueryCompileStatus status;
    uint32_t pipe_syntax;
    string error;
};

static const DefaultPipeSyntaxResult& CachedDefaultPipeSyntax() {
    static const DefaultPipeSyntaxResult result = [] {
        DefaultPipeSyntaxResult cached;
        cached.pipe_syntax = DPLYR_PIPE_SYNTAX_MAGRITTR;

        const char* env_value = std::getenv("DPLYR_PIPE_SYNTAX");
        if (env_value == nullptr || env_value[0] == '\0') {
            cached.status = QueryCompileStatus::Success;
            return cached;
        }

        cached.status = ParsePipeSyntaxOption(env_value, cached.pipe_syntax, cached.error)
            ? QueryCompileStatus::Success
            : QueryCompileStatus::Error;
        return cached;
    }();

    return result;
}

static void InitializeDefaultPipeSyntaxCache() {
    (void)CachedDefaultPipeSyntax();
}

static QueryCompileStatus DefaultPipeSyntax(uint32_t &pipe_syntax, string &error_out) {
    const auto &cached = CachedDefaultPipeSyntax();
    pipe_syntax = cached.pipe_syntax;
    error_out = cached.error;
    return cached.status;
}

template <class CompileFn>
static QueryCompileStatus CompileDplyrQueryWithCompiler(const string& query, string &sql_out, string &error_out,
                                                        CompileFn compile_fn) {
    char* sql_output = nullptr;
    char* error_output = nullptr;
    DplyrOptions options = DefaultDplyrOptions();

    auto start_time = std::chrono::high_resolution_clock::now();
    int result = compile_fn(&options, &sql_output, &error_output);

    auto end_time = std::chrono::high_resolution_clock::now();
    auto duration = std::chrono::duration_cast<std::chrono::microseconds>(end_time - start_time);
    const double MS_TO_SEC = 1000.0;
    double duration_ms = static_cast<double>(duration.count()) / MS_TO_SEC; // NOLINT(bugprone-narrowing-conversions)

    if (result == DPLYR_QUERY_NOT_HANDLED) {
        return QueryCompileStatus::NotHandled;
    }

    if (!dplyr_is_success(result)) {
        error_out = (error_output != nullptr) ? string(error_output) : "Unknown dplyr compilation error";
        if (error_output != nullptr) {
            dplyr_free_string(error_output);
        }
        return QueryCompileStatus::Error;
    }

    sql_out = (sql_output != nullptr) ? string(sql_output) : "";
    if (sql_output != nullptr) {
        dplyr_free_string(sql_output);
    }

    if (!dplyr_result_has_output(result) || sql_out.empty()) {
        error_out = "DPLYR generated empty SQL";
        return QueryCompileStatus::Error;
    }

    if (DplyrDebugLogger::is_debug_enabled()) {
        DplyrDebugLogger::log_debug(DplyrDebugLogger::LogCategory::TRANSPILER,
            "Generated SQL: " + sql_out);
    }

    DplyrDebugLogger::log_performance("transpilation", duration_ms,
        "Input: " + std::to_string(query.length()) + " chars");

    return QueryCompileStatus::Success;
}

static QueryCompileStatus CompileDplyrQuery(const string& query, string &sql_out, string &error_out) {
    return CompileDplyrQueryWithCompiler(query, sql_out, error_out,
        [&](DplyrOptions* options, char** sql_output, char** error_output) {
            return dplyr_compile_query(query.c_str(), options, sql_output, error_output);
        });
}

static QueryCompileStatus CompileDplyrQueryWithPipeSyntax(const string& query, uint32_t pipe_syntax,
                                                          string &sql_out, string &error_out) {
    return CompileDplyrQueryWithCompiler(query, sql_out, error_out,
        [&](DplyrOptions* options, char** sql_output, char** error_output) {
            return dplyr_compile_query_with_pipe_syntax(
                query.c_str(),
                options,
                pipe_syntax,
                sql_output,
                error_output);
        });
}

static string StripTrailingSemicolon(string input) {
    StringUtil::Trim(input);
    while (!input.empty() && input.back() == ';') {
        input.pop_back();
        StringUtil::Trim(input);
    }
    return input;
}

ParserExtensionParseResult dplyr_parse(ParserExtensionInfo * /*info*/, const string& query) {
    try {
        string sql;
        string error;
        auto status = CompileDplyrQuery(query, sql, error);
        if (status == QueryCompileStatus::NotHandled) {
            return ParserExtensionParseResult();
        }
        if (status == QueryCompileStatus::Error || !error.empty()) {
            return ParserExtensionParseResult(error);
        }
        return ParserExtensionParseResult(make_uniq_base<ParserExtensionParseData, DplyrParseData>(std::move(sql)));
    } catch (const Exception& ex) {
        return ParserExtensionParseResult(ex.what());
    } catch (const std::exception& ex) {
        return ParserExtensionParseResult("DPLYR transpilation failed: " + string(ex.what()));
    }
}

// ...

// (Removed duplicate Load implementation)

static void DplyrTableFunction(ClientContext &, TableFunctionInput &input, DataChunk &output);
static unique_ptr<FunctionData> DplyrSqlTableBind(ClientContext &context, TableFunctionBindInput &input,
                                                  vector<LogicalType> &return_types, vector<string> &names);
static unique_ptr<GlobalTableFunctionState> DplyrTableInit(ClientContext &context, TableFunctionInitInput &input);

ParserExtensionPlanResult dplyr_plan(ParserExtensionInfo * /*info*/, ClientContext& context,
                                     unique_ptr<ParserExtensionParseData> parse_data) {
    if (!parse_data) {
        throw InternalException("DPLYR plan called without parse data");
    }

    auto *dplyr_data = static_cast<DplyrParseData *>(parse_data.get());

    ParserExtensionPlanResult result;
    result.function = TableFunction("dplyr_query", {LogicalType::VARCHAR}, DplyrTableFunction, DplyrSqlTableBind,
                                    DplyrTableInit);
    result.parameters.emplace_back(dplyr_data->sql);
    result.requires_valid_transaction = true;
    result.return_type = StatementReturnType::QUERY_RESULT;
    return result;
}

// Implementations for DplyrParserExtension
DplyrParserExtension::DplyrParserExtension() : ParserExtension() { // NOLINT(modernize-use-equals-default)
    parse_function = dplyr_parse;
    plan_function = dplyr_plan;
}

struct DplyrTableFunctionData : public TableFunctionData {
    string sql;
    vector<string> names;
    vector<LogicalType> types;

    unique_ptr<FunctionData> Copy() const override {
        auto copy = make_uniq<DplyrTableFunctionData>();
        copy->sql = sql;
        copy->names = names;
        copy->types = types;
        return copy;
    }

    bool Equals(const FunctionData &other) const override {
        auto &other_data = other.Cast<DplyrTableFunctionData>();
        return sql == other_data.sql && types == other_data.types;
    }

    // Disable statement caching for this table function since results depend on current catalog state.
    bool SupportStatementCache() const override {
        return false;
    }
};

struct DplyrTableFunctionState : public GlobalTableFunctionState {
    explicit DplyrTableFunctionState(unique_ptr<ColumnDataCollection> collection_p)
        : collection(std::move(collection_p)) {
        collection->InitializeScan(scan_state);
    }

    idx_t MaxThreads() const override {
        return 1;
    }

    unique_ptr<ColumnDataCollection> collection;
    ColumnDataScanState scan_state;
};

static string GetDplyrQuery(const TableFunctionBindInput &input) {
    if (input.inputs.empty() || input.inputs[0].IsNull()) {
        throw InvalidInputException("dplyr() requires a non-null query string");
    }
    return StringValue::Get(input.inputs[0]);
}

static uint32_t GetDplyrPipeSyntax(const TableFunctionBindInput &input) {
    if (input.inputs.size() < 2 || input.inputs[1].IsNull()) {
        string error;
        uint32_t pipe_syntax = DPLYR_PIPE_SYNTAX_MAGRITTR;
        auto status = DefaultPipeSyntax(pipe_syntax, error);
        if (status != QueryCompileStatus::Success) {
            throw InvalidInputException("%s", error.c_str());
        }
        return pipe_syntax;
    }

    string error;
    uint32_t pipe_syntax = DPLYR_PIPE_SYNTAX_MAGRITTR;
    if (!ParsePipeSyntaxOption(StringValue::Get(input.inputs[1]), pipe_syntax, error)) {
        throw InvalidInputException("%s", error.c_str());
    }
    return pipe_syntax;
}

static unique_ptr<FunctionData> DplyrTableBind(ClientContext &context, TableFunctionBindInput &input,
                                               vector<LogicalType> &return_types, vector<string> &names) {
    auto dplyr_code = StripTrailingSemicolon(GetDplyrQuery(input));

    auto &db = DatabaseInstance::GetDatabase(context);
    Connection conn(db);

    string sql;
    string error;
    auto status = CompileDplyrQueryWithPipeSyntax(dplyr_code, GetDplyrPipeSyntax(input), sql, error);
    if (status == QueryCompileStatus::NotHandled) {
        throw InvalidInputException("dplyr() requires a configured pipeline expression");
    }
    if (status == QueryCompileStatus::Error || !error.empty()) {
        throw InvalidInputException("dplyr() transpilation failed: %s", error.c_str());
    }

    // Bind should be lightweight: infer schema without materializing rows.
    string schema_query = "SELECT * FROM (" + sql + ") AS dplyr_subquery LIMIT 0";
    auto schema_result = conn.Query(schema_query);
    if (schema_result->HasError()) {
        throw InvalidInputException(
            "dplyr() schema inference failed: %s",
            schema_result->GetError().c_str());
    }

    auto &materialized = schema_result->Cast<MaterializedQueryResult>();

    auto bind_data = make_uniq<DplyrTableFunctionData>();
    bind_data->sql = sql;
    bind_data->names = materialized.names;
    bind_data->types = materialized.types;

    names = bind_data->names;
    return_types = bind_data->types;
    return bind_data;
}

static unique_ptr<FunctionData> DplyrSqlTableBind(ClientContext &context, TableFunctionBindInput &input,
                                                  vector<LogicalType> &return_types, vector<string> &names) {
    if (input.inputs.empty() || input.inputs[0].IsNull()) {
        throw InvalidInputException("dplyr_query() requires a non-null SQL string");
    }

    string sql = StringValue::Get(input.inputs[0]);
    sql = StripTrailingSemicolon(std::move(sql));
    if (sql.empty()) {
        throw InvalidInputException("dplyr_query() requires a non-empty SQL string");
    }

    auto &db = DatabaseInstance::GetDatabase(context);
    Connection conn(db);

    string schema_query = "SELECT * FROM (" + sql + ") AS dplyr_subquery LIMIT 0";
    auto schema_result = conn.Query(schema_query);
    if (schema_result->HasError()) {
        throw InvalidInputException("dplyr_query() schema inference failed: %s", schema_result->GetError().c_str());
    }

    auto &materialized = schema_result->Cast<MaterializedQueryResult>();

    auto bind_data = make_uniq<DplyrTableFunctionData>();
    bind_data->sql = std::move(sql);
    bind_data->names = materialized.names;
    bind_data->types = materialized.types;

    names = bind_data->names;
    return_types = bind_data->types;
    return bind_data;
}

static unique_ptr<GlobalTableFunctionState> DplyrTableInit(ClientContext &context, TableFunctionInitInput &input) {
    auto &data = input.bind_data->Cast<DplyrTableFunctionData>();
    auto &db = DatabaseInstance::GetDatabase(context);
    Connection conn(db);

    auto result = conn.Query(data.sql);
    if (result->HasError()) {
        throw InvalidInputException("dplyr() failed to execute: %s", result->GetError().c_str());
    }

    // Fetch all chunks from the result and build a new collection
    // This avoids using TakeCollection which is not exported on Windows
    auto collection = make_uniq<ColumnDataCollection>(Allocator::DefaultAllocator(), data.types);
    
    while (true) {
        auto chunk = result->Fetch();
        if (!chunk || chunk->size() == 0) {
            break;
        }
        collection->Append(*chunk);
    }

    return make_uniq<DplyrTableFunctionState>(std::move(collection));
}

static void DplyrTableFunction(ClientContext & /*context*/, TableFunctionInput &input, DataChunk &output) {
    auto &state = input.global_state->Cast<DplyrTableFunctionState>();
    if (!state.collection->Scan(state.scan_state, output)) {
        output.SetCardinality(0);
    }
}

void DplyrExtension::Load(ExtensionLoader& loader) {
    loader.SetDescription("libdplyr transpilation extension");
    InitializeDefaultPipeSyntaxCache();

    auto& instance = loader.GetDatabaseInstance();
    auto& config = DBConfig::GetConfig(instance);
#if defined(__has_include) && __has_include("duckdb/main/extension_callback_manager.hpp")
    ParserExtension::Register(config, DplyrParserExtension());
#else
    config.parser_extensions.push_back(DplyrParserExtension());
#endif
    
    TableFunction dplyr_function("dplyr",
        {LogicalType::VARCHAR},
        DplyrTableFunction,
        DplyrTableBind,
        DplyrTableInit);
    loader.RegisterFunction(dplyr_function);

    TableFunction dplyr_function_with_config("dplyr",
        {LogicalType::VARCHAR, LogicalType::VARCHAR},
        DplyrTableFunction,
        DplyrTableBind,
        DplyrTableInit);
    loader.RegisterFunction(dplyr_function_with_config);

}

string DplyrExtension::Name() {
    return "dplyr";
}

} // namespace dplyr

// On Windows with DUCKDB_STATIC_BUILD, DUCKDB_EXTENSION_API is empty.
// We need explicit dllexport to ensure the entrypoint is visible in the DLL.
#if defined(_WIN32) || defined(_MSC_VER)
#define DPLYR_ENTRYPOINT_EXPORT __declspec(dllexport)
#else
#define DPLYR_ENTRYPOINT_EXPORT
#endif

extern "C" DPLYR_ENTRYPOINT_EXPORT DUCKDB_EXTENSION_API void dplyr_duckdb_cpp_init(duckdb::ExtensionLoader &loader) {
    dplyr::DplyrExtension ext;
    ext.Load(loader);
}
