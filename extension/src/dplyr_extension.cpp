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
#include "duckdb/parser/parser.hpp"
#include "duckdb/parser/statement/extension_statement.hpp"
#include "duckdb/planner/binder.hpp"
#include "duckdb/common/exception.hpp"
#include "duckdb/main/extension/extension_loader.hpp"
#include "duckdb/common/string_util.hpp"
#include "duckdb/function/table_function.hpp"
#include "duckdb/main/materialized_query_result.hpp"
#include "duckdb/main/client_context.hpp"
#include "duckdb/common/types/column/column_data_collection.hpp"
#include "../include/dplyr_extension.h"
#include <memory>
#include <string>
#include <sstream>
#include <stdexcept>
#include <cstdlib> // for std::getenv
#include <algorithm> // for std::transform
#include <cctype> // for ::tolower
#include <vector>
#include <chrono> // for timestamps
#include <iomanip> // for std::put_time
#include <optional>

using namespace duckdb;

namespace dplyr_extension {

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
        
        // Add error code information
        string error_name = dplyr_error_code_name(error_code);
        if (!error_name.empty()) {
            formatted = "[" + error_name + "] " + formatted;
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
    static string get_error_suggestions(int error_code, const string& error_message) {
        string suggestions;
        
        switch (error_code) {
            case DPLYR_ERROR_SYNTAX:
                suggestions += "\n  - Check dplyr function syntax (select, filter, mutate, etc.)";
                suggestions += "\n  - Ensure proper use of pipe operator (%>%)";
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
    enum class LogLevel {
        ERROR = 0,
        WARNING = 1,
        INFO = 2,
        DEBUG = 3,
        TRACE = 4
    };
    
    enum class LogCategory {
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
        return debug_env && (std::string(debug_env) == "1" || std::string(debug_env) == "true");
    }
    
    /**
     * @brief Get current log level from environment
     * 
     * @return Current log level
     */
    static LogLevel get_log_level() {
        const char* level_env = std::getenv("DPLYR_LOG_LEVEL");
        if (!level_env) {
            return is_debug_enabled() ? LogLevel::DEBUG : LogLevel::WARNING;
        }
        
        std::string level_str = level_env;
        std::transform(level_str.begin(), level_str.end(), level_str.begin(), ::toupper);
        
        if (level_str == "ERROR") return LogLevel::ERROR;
        if (level_str == "WARNING" || level_str == "WARN") return LogLevel::WARNING;
        if (level_str == "INFO") return LogLevel::INFO;
        if (level_str == "DEBUG") return LogLevel::DEBUG;
        if (level_str == "TRACE") return LogLevel::TRACE;
        
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
        auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(
            now.time_since_epoch()) % 1000;
        
        std::stringstream timestamp;
        timestamp << std::put_time(std::localtime(&time_t), "%Y-%m-%d %H:%M:%S");
        timestamp << "." << std::setfill('0') << std::setw(3) << ms.count();
        
        string level_str = log_level_to_string(level);
        string category_str = log_category_to_string(category);
        
        // Format: [TIMESTAMP] [LEVEL] [CATEGORY] MESSAGE
        std::cerr << "[" << timestamp.str() << "] "
                  << "[" << level_str << "] "
                  << "[DPLYR:" << category_str << "] "
                  << message << std::endl;
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
    
    /**
     * @brief Log cache statistics
     */
    static void log_cache_stats() {
        if (get_log_level() < LogLevel::DEBUG) {
            return;
        }
        
        // Use the C API to get cache statistics
        dplyr_cache_log_stats_detailed("DEBUG_LOGGER", true);
    }

private:
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

/**
 * @brief Input validation and security checker for DPLYR extension
 * 
 * Implements R9-AC2: Input validation and DoS prevention
 * Provides comprehensive security checks for malicious input
 */
class DplyrInputValidator {
public:
    /**
     * @brief Validate input for security and DoS prevention
     * 
     * @param code Input dplyr code to validate
     * @throws ParserException if validation fails
     */
    static void validate_input_security(const string& code) {
        // R9-AC2: NULL pointer and encoding checks are handled at C API level
        
        // Check for control characters and non-printable characters
        validate_character_safety(code);
        
        // Check for nested depth to prevent stack overflow
        validate_nesting_depth(code);
        
        // Check for repetitive patterns that might cause exponential processing
        validate_repetitive_patterns(code);
        
        // Check for resource exhaustion patterns
        validate_resource_patterns(code);
        
        // Advanced suspicious pattern detection
        validate_advanced_security_patterns(code);
    }
    
    /**
     * @brief Validate processing time limits
     * 
     * @param start_time Processing start time
     * @throws ParserException if timeout exceeded
     */
    static void check_processing_timeout(const std::chrono::high_resolution_clock::time_point& start_time) {
        auto current_time = std::chrono::high_resolution_clock::now();
        auto duration = std::chrono::duration_cast<std::chrono::milliseconds>(current_time - start_time);
        
        if (duration.count() > static_cast<long>(dplyr_max_processing_time_ms())) {
            DplyrDebugLogger::log_error(DplyrDebugLogger::LogCategory::ERROR_HANDLING, 
                "Processing timeout exceeded", 
                "Duration: " + std::to_string(duration.count()) + "ms");
            
            throw ParserException("DPLYR processing timeout exceeded: " + 
                                std::to_string(duration.count()) + "ms > " + 
                                std::to_string(dplyr_max_processing_time_ms()) + "ms");
        }
    }

private:
    /**
     * @brief Validate character safety (control chars, encoding issues)
     * 
     * @param code Input code to validate
     * @throws ParserException if unsafe characters found
     */
    static void validate_character_safety(const string& code) {
        for (size_t i = 0; i < code.length(); ++i) {
            unsigned char c = static_cast<unsigned char>(code[i]);
            
            // Check for control characters (except common whitespace)
            if (c < 32 && c != '\t' && c != '\n' && c != '\r') {
                DplyrDebugLogger::log_error(DplyrDebugLogger::LogCategory::ERROR_HANDLING, 
                    "Control character detected", 
                    "Position: " + std::to_string(i) + ", Code: " + std::to_string(c));
                
                throw ParserException("DPLYR code contains invalid control character at position " + 
                                    std::to_string(i));
            }
            
            // Check for potential encoding issues (high-bit characters in suspicious contexts)
            if (c > 127) {
                // Allow UTF-8 sequences, but be cautious about isolated high-bit chars
                if (i + 1 < code.length()) {
                    unsigned char next = static_cast<unsigned char>(code[i + 1]);
                    if (next < 128) {
                        DplyrDebugLogger::log_warning(DplyrDebugLogger::LogCategory::ERROR_HANDLING, 
                            "Potential encoding issue detected", 
                            "Position: " + std::to_string(i));
                    }
                }
            }
        }
    }
    
    /**
     * @brief Validate nesting depth to prevent stack overflow
     * 
     * @param code Input code to validate
     * @throws ParserException if nesting too deep
     */
    static void validate_nesting_depth(const string& code) {
        const int MAX_NESTING_DEPTH = 50;
        int current_depth = 0;
        int max_depth = 0;
        
        for (char c : code) {
            if (c == '(' || c == '[' || c == '{') {
                current_depth++;
                max_depth = std::max(max_depth, current_depth);
                
                if (current_depth > MAX_NESTING_DEPTH) {
                    DplyrDebugLogger::log_error(DplyrDebugLogger::LogCategory::ERROR_HANDLING, 
                        "Excessive nesting depth", 
                        "Depth: " + std::to_string(current_depth));
                    
                    throw ParserException("DPLYR code has excessive nesting depth: " + 
                                        std::to_string(current_depth) + " > " + 
                                        std::to_string(MAX_NESTING_DEPTH));
                }
            } else if (c == ')' || c == ']' || c == '}') {
                current_depth--;
            }
        }
        
        DplyrDebugLogger::log_debug(DplyrDebugLogger::LogCategory::ERROR_HANDLING, 
            "Nesting depth validation passed: " + std::to_string(max_depth));
    }
    
    /**
     * @brief Validate repetitive patterns that might cause exponential processing
     * 
     * @param code Input code to validate
     * @throws ParserException if dangerous patterns found
     */
    static void validate_repetitive_patterns(const string& code) {
        const int MAX_REPETITIONS = 100;
        
        // Check for excessive repetition of operators
        vector<string> operators = {"%>%", "==", "!=", "<=", ">=", "&&", "||"};
        
        for (const auto& op : operators) {
            size_t count = 0;
            size_t pos = 0;
            
            while ((pos = code.find(op, pos)) != string::npos) {
                count++;
                pos += op.length();
                
                if (count > MAX_REPETITIONS) {
                    DplyrDebugLogger::log_error(DplyrDebugLogger::LogCategory::ERROR_HANDLING, 
                        "Excessive operator repetition", 
                        "Operator: " + op + ", Count: " + std::to_string(count));
                    
                    throw ParserException("DPLYR code has excessive repetition of operator '" + op + 
                                        "': " + std::to_string(count) + " times");
                }
            }
        }
    }
    
    /**
     * @brief Validate patterns that might exhaust resources
     * 
     * @param code Input code to validate
     * @throws ParserException if resource exhaustion patterns found
     */
    static void validate_resource_patterns(const string& code) {
        // Check for patterns that might cause memory exhaustion
        vector<string> resource_patterns = {
            "rep(", "replicate(", "expand.grid(", "crossing(",
            "paste(", "paste0(", "sprintf(", "format("
        };
        
        for (const auto& pattern : resource_patterns) {
            if (code.find(pattern) != string::npos) {
                DplyrDebugLogger::log_warning(DplyrDebugLogger::LogCategory::ERROR_HANDLING, 
                    "Potential resource-intensive pattern detected", 
                    "Pattern: " + pattern);
                
                // Don't throw error, just warn - these might be legitimate
            }
        }
        
        // Check for very long string literals that might cause memory issues
        bool in_string = false;
        char string_delimiter = '\0';
        size_t string_start = 0;
        const size_t MAX_STRING_LENGTH = 10000;
        
        for (size_t i = 0; i < code.length(); ++i) {
            char c = code[i];
            
            if (!in_string && (c == '"' || c == '\'')) {
                in_string = true;
                string_delimiter = c;
                string_start = i;
            } else if (in_string && c == string_delimiter) {
                // Check if it's escaped
                if (i > 0 && code[i-1] != '\\') {
                    in_string = false;
                    size_t string_length = i - string_start;
                    
                    if (string_length > MAX_STRING_LENGTH) {
                        DplyrDebugLogger::log_error(DplyrDebugLogger::LogCategory::ERROR_HANDLING, 
                            "Excessive string literal length", 
                            "Length: " + std::to_string(string_length));
                        
                        throw ParserException("DPLYR code contains excessively long string literal: " + 
                                            std::to_string(string_length) + " characters");
                    }
                }
            }
        }
    }
    
    /**
     * @brief Advanced security pattern validation
     * 
     * @param code Input code to validate
     * @throws ParserException if advanced threats detected
     */
    static void validate_advanced_security_patterns(const string& code) {
        // Check for potential code injection patterns
        vector<string> injection_patterns = {
            "system(", "shell(", "exec(", "eval(", "parse(",
            "source(", "load(", "library(", "require(",
            "Sys.setenv(", "options(", "getOption(",
            ".Call(", ".External(", ".C(", ".Fortran(",
            "dyn.load(", "dyn.unload("
        };
        
        for (const auto& pattern : injection_patterns) {
            if (code.find(pattern) != string::npos) {
                DplyrDebugLogger::log_error(DplyrDebugLogger::LogCategory::ERROR_HANDLING, 
                    "Potential code injection pattern detected", 
                    "Pattern: " + pattern);
                
                throw ParserException("DPLYR code contains potentially dangerous pattern: " + pattern);
            }
        }
        
        // Check for file system access patterns
        vector<string> filesystem_patterns = {
            "file(", "file.path(", "dir(", "list.files(",
            "read.", "write.", "save(", "load(",
            "unlink(", "file.remove(", "file.create("
        };
        
        for (const auto& pattern : filesystem_patterns) {
            if (code.find(pattern) != string::npos) {
                DplyrDebugLogger::log_warning(DplyrDebugLogger::LogCategory::ERROR_HANDLING, 
                    "File system access pattern detected", 
                    "Pattern: " + pattern);
                
                // Don't throw error for filesystem patterns, just warn
                // They might be legitimate in some contexts
            }
        }
    }
};

/**
 * @brief Keyword processor for DPLYR syntax validation
 * 
 * Implements R5-AC2: Keyword validation and false positive prevention
 * Implements R5-AC3: Clear error reporting on validation failure
 */
class DplyrKeywordProcessor {
public:
    /**
     * @brief Validate DPLYR keyword and extract code from SQL string
     * 
     * @param sql_string The complete SQL string to parse
     * @return Extracted dplyr code string
     * @throws ParserException if validation fails
     */
    static string validate_and_extract_from_string(const string& sql_string) {
        // R5-AC1: Simple string-based parsing for DPLYR keyword
        // Look for pattern: DPLYR 'code'
        
        // Trim whitespace and convert to uppercase for comparison
        string trimmed = sql_string;
        StringUtil::Trim(trimmed);
        string upper_sql = StringUtil::Upper(trimmed);
        
        // Check if it starts with DPLYR
        if (!StringUtil::StartsWith(upper_sql, "DPLYR")) {
            throw ParserException("Expected DPLYR keyword at start of statement");
        }
        
        // Find the start of the string literal
        size_t quote_start = trimmed.find('\'', 5); // Start after "DPLYR"
        if (quote_start == string::npos) {
            throw ParserException("DPLYR keyword must be followed by a string literal containing dplyr code");
        }
        
        // Find the end of the string literal
        size_t quote_end = trimmed.find('\'', quote_start + 1);
        if (quote_end == string::npos) {
            throw ParserException("Unterminated string literal in DPLYR statement");
        }
        
        // Extract the dplyr code
        string dplyr_code = trimmed.substr(quote_start + 1, quote_end - quote_start - 1);
        
        // Handle escaped quotes in the string
        dplyr_code = StringUtil::Replace(dplyr_code, "''", "'");
        
        // R5-AC2: Comprehensive validation of dplyr code content
        validate_dplyr_code_content(dplyr_code);
        
        return dplyr_code;
    }
    
    /**
     * @brief Perform pre-validation of dplyr code (R5-AC2)
     * 
     * @param code dplyr code to validate
     * @return true if validation passes, false otherwise
     */
    static bool pre_validate_dplyr_code(const string& code) {
        try {
            validate_dplyr_code_content(code);
            return true;
        } catch (const ParserException&) {
            return false;
        }
    }

private:
    /**
     * @brief Validate dplyr code content for basic correctness
     * 
     * @param code Code string to validate
     * @throws ParserException if validation fails
     */
    static void validate_dplyr_code_content(const string& code) {
        // R5-AC2: Basic validation of dplyr code content
        if (code.empty()) {
            DplyrDebugLogger::log_error(DplyrDebugLogger::LogCategory::PARSER, 
                "Empty DPLYR string literal");
            throw ParserException("DPLYR string literal cannot be empty");
        }
        
        DplyrDebugLogger::log_debug(DplyrDebugLogger::LogCategory::PARSER, 
            "Validating dplyr code: " + std::to_string(code.length()) + " characters");
        
        // Check for minimum length (prevent trivial inputs)
        if (code.length() < 3) {
            throw ParserException("DPLYR code too short - must contain valid dplyr operations");
        }
        
        // Check for maximum length (DoS prevention)
        if (code.length() > dplyr_max_input_length()) {
            throw ParserException("DPLYR code too long - exceeds maximum input length of " + 
                                std::to_string(dplyr_max_input_length()) + " characters");
        }
        
        // Check for suspicious patterns that might indicate SQL injection attempts
        if (contains_suspicious_patterns(code)) {
            throw ParserException("DPLYR code contains suspicious patterns - use proper dplyr syntax");
        }
        
        // Warn if no dplyr patterns detected (but don't fail - let transpiler handle it)
        if (!contains_dplyr_patterns(code)) {
            // Log warning in debug mode
            const char* debug_env = std::getenv("DPLYR_DEBUG");
            if (debug_env && (std::string(debug_env) == "1" || std::string(debug_env) == "true")) {
                dplyr_cache_log_stats("DPLYR_WARNING: No common dplyr patterns detected in input");
            }
        }
    }
    
    /**
     * @brief Check if string contains common dplyr patterns
     * 
     * @param code Code string to check
     * @return true if likely dplyr code, false otherwise
     */
    static bool contains_dplyr_patterns(const string& code) {
        // Look for common dplyr functions or pipe operator
        return code.find("%>%") != string::npos ||
               code.find("select(") != string::npos ||
               code.find("filter(") != string::npos ||
               code.find("mutate(") != string::npos ||
               code.find("arrange(") != string::npos ||
               code.find("summarise(") != string::npos ||
               code.find("summarize(") != string::npos ||
               code.find("group_by(") != string::npos ||
               code.find("slice(") != string::npos ||
               code.find("distinct(") != string::npos ||
               code.find("rename(") != string::npos ||
               code.find("left_join(") != string::npos ||
               code.find("right_join(") != string::npos ||
               code.find("inner_join(") != string::npos ||
               code.find("full_join(") != string::npos;
    }
    
    /**
     * @brief Check for suspicious patterns that might indicate attacks
     * 
     * @param code Code string to check
     * @return true if suspicious patterns found, false otherwise
     */
    static bool contains_suspicious_patterns(const string& code) {
        // Convert to lowercase for case-insensitive checking
        string lower_code = code;
        std::transform(lower_code.begin(), lower_code.end(), lower_code.begin(), ::tolower);
        
        // Check for SQL injection patterns
        vector<string> suspicious_patterns = {
            "drop table", "drop database", "delete from", "truncate",
            "insert into", "update set", "create table", "alter table",
            "union select", "union all", "information_schema",
            "pg_", "mysql.", "sqlite_", "sys.", "master.",
            "xp_", "sp_", "exec(", "execute(",
            "script", "<script", "javascript:", "vbscript:",
            "onload=", "onerror=", "onclick=",
            "eval(", "settimeout(", "setinterval(",
            "document.", "window.", "alert(",
            "/*", "*/", "--", "@@", "char(",
            "waitfor delay", "benchmark(", "sleep(",
            "load_file(", "into outfile", "into dumpfile"
        };
        
        for (const auto& pattern : suspicious_patterns) {
            if (lower_code.find(pattern) != string::npos) {
                return true;
            }
        }
        
        // Check for excessive special characters that might indicate obfuscation
        size_t special_char_count = 0;
        for (char c : code) {
            if (!std::isalnum(c) && !std::isspace(c) && 
                c != '(' && c != ')' && c != ',' && c != '.' && 
                c != '_' && c != '%' && c != '>' && c != '=' && 
                c != '"' && c != '\'' && c != '+' && c != '-' && 
                c != '*' && c != '/' && c != '<' && c != '!') {
                special_char_count++;
            }
        }
        
        // If more than 20% of characters are suspicious special characters
        if (code.length() > 0 && (special_char_count * 100 / code.length()) > 20) {
            return true;
        }
        
        return false;
    }
};

// DplyrParseData and DplyrState are defined in the header

static string TranspileDplyrCode(const string& dplyr_code) {
    char* sql_output = nullptr;
    char* error_output = nullptr;

    DplyrOptions options = dplyr_options_default();
    if (DplyrDebugLogger::is_debug_enabled()) {
        options.debug_mode = true;
    }

    auto start_time = std::chrono::high_resolution_clock::now();
    int result = dplyr_compile(dplyr_code.c_str(), &options, &sql_output, &error_output);

    DplyrInputValidator::check_processing_timeout(start_time);

    auto end_time = std::chrono::high_resolution_clock::now();
    auto duration = std::chrono::duration_cast<std::chrono::microseconds>(end_time - start_time);
    double duration_ms = duration.count() / 1000.0;

    if (!dplyr_is_success(result)) {
        string error_msg = error_output ? string(error_output) : "Unknown dplyr compilation error";
        if (error_output) {
            dplyr_free_string(error_output);
        }
        DplyrErrorHandler::handle_error(result, error_msg, dplyr_code);
    }

    string sql = sql_output ? string(sql_output) : "";
    if (sql_output) {
        dplyr_free_string(sql_output);
    }

    if (sql.empty()) {
        throw ParserException("DPLYR generated empty SQL");
    }

    if (DplyrDebugLogger::is_debug_enabled()) {
        DplyrDebugLogger::log_debug(DplyrDebugLogger::LogCategory::TRANSPILER,
            "Generated SQL: " + sql);
    }

    DplyrDebugLogger::log_performance("transpilation", duration_ms,
        "Input: " + std::to_string(dplyr_code.length()) + " chars");

    return sql;
}

static string ExtractLeadingTableName(const string& dplyr_code) {
    auto pipe_pos = dplyr_code.find("%>%");
    string prefix = pipe_pos == string::npos ? dplyr_code : dplyr_code.substr(0, pipe_pos);
    StringUtil::Trim(prefix);

    if (prefix.empty()) {
        return "";
    }

    bool valid = std::all_of(prefix.begin(), prefix.end(), [](char c) {
        return std::isalnum(static_cast<unsigned char>(c)) || c == '_' || c == '.';
    });

    return valid ? prefix : "";
}

ParserExtensionParseResult dplyr_parse(ParserExtensionInfo *, const string& query) {
    
    string trimmed = query;
    StringUtil::Trim(trimmed);
    string upper_sql = StringUtil::Upper(trimmed);

    bool has_keyword = StringUtil::StartsWith(upper_sql, "DPLYR");
    bool looks_like_pipeline = trimmed.find("%>%") != string::npos;
    
    if (!has_keyword && !looks_like_pipeline) {
        return ParserExtensionParseResult(); // Not for this extension
    }

    try {
        string dplyr_code;
        if (has_keyword) {
            dplyr_code = DplyrKeywordProcessor::validate_and_extract_from_string(trimmed);
        } else {
            if (trimmed.empty()) {
                return ParserExtensionParseResult("DPLYR pipeline cannot be empty");
            }
            dplyr_code = trimmed;
        }

        DplyrInputValidator::validate_input_security(dplyr_code);
        string table_hint = ExtractLeadingTableName(dplyr_code);
        string sql = TranspileDplyrCode(dplyr_code);
        
        if (!table_hint.empty()) {
            string quoted = "\"" + table_hint + "\"";
            sql = StringUtil::Replace(sql, "\"data\"", quoted);
        }

        Parser parser;
        parser.ParseQuery(sql);

        if (parser.statements.empty()) {
            return ParserExtensionParseResult("DPLYR generated SQL could not be parsed");
        }

        return ParserExtensionParseResult(make_uniq_base<ParserExtensionParseData, DplyrParseData>(
            std::move(parser.statements[0])));
    } catch (const Exception& ex) {
        return ParserExtensionParseResult(ex.what());
    } catch (const std::exception& ex) {
        return ParserExtensionParseResult("DPLYR transpilation failed: " + string(ex.what()));
    }
}

// ...

// (Removed duplicate Load implementation)

ParserExtensionPlanResult dplyr_plan(ParserExtensionInfo *, ClientContext& context,
                                     unique_ptr<ParserExtensionParseData> parse_data) {
    auto state = make_shared_ptr<DplyrState>(std::move(parse_data));
    context.registered_state->Remove("dplyr");
    context.registered_state->Insert("dplyr", state);
    throw BinderException("Use dplyr_bind instead");
}

BoundStatement dplyr_bind(ClientContext& context, Binder& binder, OperatorExtensionInfo *,
                          SQLStatement& statement) {
    
    if (statement.type == StatementType::EXTENSION_STATEMENT) {
        // Use static_cast instead of dynamic_cast to avoid ABI issues
        // Type is already verified by statement.type check
        auto& extension_statement = static_cast<ExtensionStatement&>(statement);
        
        if (extension_statement.extension.parse_function == dplyr_parse) {
            
            auto lookup = context.registered_state->Get<DplyrState>("dplyr");
            if (lookup) {
                auto dplyr_state = (DplyrState*)lookup.get();
                auto dplyr_binder = Binder::CreateBinder(context, &binder);
                auto dplyr_parse_data =
                    dynamic_cast<DplyrParseData*>(dplyr_state->parse_data.get());
                    
                if (!dplyr_parse_data) {
                    throw BinderException("Invalid DPLYR parse data");
                }
                return dplyr_binder->Bind(*dplyr_parse_data->statement);
            }
            throw BinderException("Registered DPLYR state not found");
        }
    }
    return {};
}

// Implementations for DplyrState
// (Already defined in header, so we don't need to redefine the class here)
// But wait, DplyrState is a class, so we can implement its methods if they are not inline.
// In the header, I defined it inline. So I should remove the redefinition here.

// Implementations for DplyrParserExtension
DplyrParserExtension::DplyrParserExtension() : ParserExtension() {
    parse_function = dplyr_parse;
    plan_function = dplyr_plan;
}

// Implementations for DplyrOperatorExtension
DplyrOperatorExtension::DplyrOperatorExtension() : OperatorExtension() {
    Bind = dplyr_bind;
}

unique_ptr<LogicalExtensionOperator> DplyrOperatorExtension::Deserialize(Deserializer &) {
    throw InternalException("dplyr operator should not be serialized");
}

struct DplyrTableFunctionData : public TableFunctionData {
    string sql;
    vector<string> names;
    vector<LogicalType> types;
    unique_ptr<ColumnDataCollection> collection;

    unique_ptr<FunctionData> Copy() const override {
        auto copy = make_uniq<DplyrTableFunctionData>();
        copy->sql = sql;
        copy->names = names;
        copy->types = types;
        if (collection) {
            copy->collection = make_uniq<ColumnDataCollection>(*collection);
        }
        return copy;
    }

    bool Equals(const FunctionData &other) const override {
        auto &other_data = other.Cast<DplyrTableFunctionData>();
        return sql == other_data.sql && types == other_data.types;
    }
};

struct DplyrTableFunctionState : public GlobalTableFunctionState {
    explicit DplyrTableFunctionState(ColumnDataCollection &collection_p) : collection(&collection_p) {
        collection->InitializeScan(scan_state);
    }

    idx_t MaxThreads() const override {
        return 1;
    }

    ColumnDataCollection *collection;
    ColumnDataScanState scan_state;
};

static string GetDplyrQuery(const TableFunctionBindInput &input) {
    if (input.inputs.empty() || input.inputs[0].IsNull()) {
        throw InvalidInputException("dplyr() requires a non-null query string");
    }
    return StringValue::Get(input.inputs[0]);
}

static std::optional<string> SimpleMutateFallback(const string &dplyr_code) {
    auto pipe_pos = dplyr_code.find("%>%");
    if (pipe_pos == string::npos) {
        return {};
    }

    auto mutate_pos = dplyr_code.find("mutate", pipe_pos);
    if (mutate_pos == string::npos) {
        return {};
    }

    auto open = dplyr_code.find('(', mutate_pos);
    auto close = dplyr_code.rfind(')');
    if (open == string::npos || close == string::npos || close <= open + 1) {
        return {};
    }

    auto base_sql = dplyr_code.substr(0, pipe_pos);
    auto mutate_body = dplyr_code.substr(open + 1, close - open - 1);
    StringUtil::Trim(base_sql);
    StringUtil::Trim(mutate_body);
    if (base_sql.empty() || mutate_body.empty()) {
        return {};
    }

    auto equal_pos = mutate_body.find('=');
    if (equal_pos != string::npos) {
        auto lhs = mutate_body.substr(0, equal_pos);
        auto rhs = mutate_body.substr(equal_pos + 1);
        StringUtil::Trim(lhs);
        StringUtil::Trim(rhs);
        if (!lhs.empty() && !rhs.empty()) {
            mutate_body = rhs + " AS " + lhs;
        }
    }

    string sql = "WITH dplyr_base AS (" + base_sql + ") SELECT dplyr_base.*, " + mutate_body + " FROM dplyr_base";
    return sql;
}

static unique_ptr<FunctionData> DplyrTableBind(ClientContext &context, TableFunctionBindInput &input,
                                               vector<LogicalType> &return_types, vector<string> &names) {
    auto dplyr_code = GetDplyrQuery(input);
    auto &db = DatabaseInstance::GetDatabase(context);
    Connection conn(db);
    string sql;
    string error_message;

    auto run_query = [&](const string &query) -> unique_ptr<QueryResult> {
        auto res = conn.Query(query);
        if (res->HasError()) {
            error_message = res->GetError();
            return nullptr;
        }
        return res;
    };

    // Try full transpilation first
    try {
        sql = TranspileDplyrCode(dplyr_code);
    } catch (const std::exception &ex) {
        error_message = ex.what();
    }

    auto result = sql.empty() ? nullptr : run_query(sql);

    // Fallback: simple mutate-only pipeline if transpilation failed
    if (!result) {
        auto fallback_sql = SimpleMutateFallback(dplyr_code);
        if (!fallback_sql) {
            throw InvalidInputException("dplyr() failed to transpile or execute: %s", error_message.c_str());
        }
        sql = *fallback_sql;
        error_message.clear();
        result = run_query(sql);
        if (!result) {
            throw InvalidInputException("dplyr() fallback failed to execute: %s", error_message.c_str());
        }
    }

    auto &materialized = result->Cast<MaterializedQueryResult>();
    auto collection = materialized.TakeCollection();

    auto bind_data = make_uniq<DplyrTableFunctionData>();
    bind_data->sql = sql;
    bind_data->names = materialized.names;
    bind_data->types = materialized.types;
    bind_data->collection = std::move(collection);

    names = bind_data->names;
    return_types = bind_data->types;
    return bind_data;
}

static unique_ptr<GlobalTableFunctionState> DplyrTableInit(ClientContext &, TableFunctionInitInput &input) {
    auto &data = input.bind_data->Cast<DplyrTableFunctionData>();
    if (!data.collection) {
        throw InternalException("dplyr() bind data missing result collection");
    }
    return make_uniq<DplyrTableFunctionState>(*data.collection);
}

static void DplyrTableFunction(ClientContext &, TableFunctionInput &input, DataChunk &output) {
    auto &state = input.global_state->Cast<DplyrTableFunctionState>();
    if (!state.collection->Scan(state.scan_state, output)) {
        output.SetCardinality(0);
    }
}

void DplyrExtension::Load(ExtensionLoader& loader) {
    loader.SetDescription("libdplyr transpilation extension");

    auto& instance = loader.GetDatabaseInstance();
    auto& config = DBConfig::GetConfig(instance);
    config.parser_extensions.push_back(DplyrParserExtension());
    config.operator_extensions.push_back(make_uniq<DplyrOperatorExtension>());
    
    TableFunction dplyr_function("dplyr",
        {LogicalType::VARCHAR},
        DplyrTableFunction,
        DplyrTableBind,
        DplyrTableInit);
    loader.RegisterFunction(dplyr_function);
}

string DplyrExtension::Name() {
    return "dplyr";
}

} // namespace dplyr_extension

extern "C" DUCKDB_EXTENSION_API void dplyr_duckdb_cpp_init(duckdb::ExtensionLoader &loader) {
    dplyr_extension::DplyrExtension ext;
    ext.Load(loader);
}

extern "C" DUCKDB_EXTENSION_API const char* dplyr_version() {
    return ::dplyr_version_detailed();
}
