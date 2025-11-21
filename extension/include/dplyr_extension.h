/**
 * @file dplyr_extension.h
 * @brief C-compatible API for libdplyr DuckDB extension
 * 
 * This header defines the C interface for the libdplyr transpiler
 * to be used in DuckDB extensions. It provides functions for converting
 * dplyr pipeline code to SQL with proper error handling and memory management.
 * 
 * Requirements fulfilled:
 * - R3-AC1: C ABI compatibility with static/dynamic library support
 * - R3-AC2: Structured result format (success/error/message)
 * - R3-AC3: Dedicated memory management with clear ownership rules
 * - R8-AC2: Comprehensive API documentation
 * - R9-AC1: Panic safety across FFI boundaries
 * - R9-AC2: Input validation and DoS prevention
 * - R10-AC1: Debug mode support
 * - R10-AC2: Cache metadata exposure
 */

#ifndef DPLYR_EXTENSION_H
#define DPLYR_EXTENSION_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ========================================================================
 * ERROR CODES AND CONSTANTS
 * ======================================================================== */

/** @brief Success return code */
#define DPLYR_SUCCESS 0

/** @brief FFI-related errors (invalid parameters, encoding issues) */
#define DPLYR_ERROR_NULL_POINTER -1
#define DPLYR_ERROR_INVALID_UTF8 -2

/** @brief Input validation errors (DoS prevention) */
#define DPLYR_ERROR_INPUT_TOO_LARGE -3
#define DPLYR_ERROR_TIMEOUT -4

/** @brief Transpilation errors (syntax, unsupported operations) */
#define DPLYR_ERROR_SYNTAX -5
#define DPLYR_ERROR_UNSUPPORTED -6

/** @brief Internal errors (panic, system issues) */
#define DPLYR_ERROR_INTERNAL -7
#define DPLYR_ERROR_PANIC -8

/* ========================================================================
 * DATA STRUCTURES
 * ======================================================================== */

/**
 * @brief Configuration options for dplyr transpilation
 * 
 * This structure must match the Rust DplyrOptions struct exactly
 * to ensure C ABI compatibility (R3-AC1).
 */
typedef struct DplyrOptions {
    bool strict_mode;               /**< Enable strict parsing mode */
    bool preserve_comments;         /**< Preserve comments in output SQL */
    bool debug_mode;                /**< Enable debug logging (R10-AC1) */
    uint32_t max_input_length;      /**< Maximum input length for DoS prevention (R9-AC2) */
    uint64_t max_processing_time_ms; /**< Maximum processing time in milliseconds (0 = use default) (R9-AC2) */
} DplyrOptions;

/* ========================================================================
 * CORE TRANSPILATION FUNCTIONS
 * ======================================================================== */

/**
 * @brief Convert dplyr pipeline code to SQL
 * 
 * This function transpiles dplyr pipeline syntax to equivalent SQL
 * for the DuckDB dialect. It handles the minimum operation set
 * (select, filter, mutate, arrange, summarise, group_by) as specified
 * in requirement R1-AC2.
 * 
 * @param code The dplyr pipeline code to transpile (must be valid UTF-8)
 * @param options Transpilation options (can be NULL for defaults)
 * @param out_sql Pointer to receive the generated SQL string (allocated by this function)
 * @param out_error Pointer to receive error message if transpilation fails
 * 
 * @return 0 on success, negative error code on failure:
 *         -1: E-FFI (invalid parameters, encoding issues)
 *         -2: E-INTERNAL (input too large, processing timeout)
 *         -3: E-SYNTAX or E-UNSUPPORTED (transpilation errors)
 *         -4: E-INTERNAL (internal panic occurred)
 * 
 * @note Memory management (R3-AC3): 
 *       - out_sql and out_error are allocated by this function
 *       - Caller MUST call dplyr_free_string() to release memory
 *       - Only one of out_sql or out_error will be set (never both)
 * 
 * @note Thread safety (R9-AC3): This function is thread-safe. Multiple threads
 *       can call this function concurrently without external synchronization.
 *       Each thread maintains its own cache (thread_local storage).
 * @note Panic safety (R9-AC1): This function will not propagate Rust panics
 * 
 * @example
 * @code
 * char* sql = NULL;
 * char* error = NULL;
 * DplyrOptions options = {false, false, false, 1024*1024};
 * 
 * int result = dplyr_compile("mtcars %>% select(mpg, cyl)", &options, &sql, &error);
 * if (result == 0) {
 *     printf("Generated SQL: %s\n", sql);
 *     dplyr_free_string(sql);
 * } else {
 *     printf("Error: %s\n", error);
 *     dplyr_free_string(error);
 * }
 * @endcode
 */
int dplyr_compile(
    const char* code,
    const DplyrOptions* options,
    char** out_sql,
    char** out_error
);

/* ========================================================================
 * MEMORY MANAGEMENT FUNCTIONS
 * ======================================================================== */

/**
 * @brief Free memory allocated by dplyr functions
 * 
 * This function must be used to free all strings allocated by dplyr functions.
 * Using standard free() or delete will result in undefined behavior.
 * 
 * @param s Pointer to string allocated by dplyr functions (can be NULL)
 * @return 0 on success, negative error code on failure
 * 
 * @note Memory ownership (R3-AC3): 
 *       - This function transfers ownership from caller back to the library
 *       - After calling this function, the pointer becomes invalid
 *       - It is safe to call this function with NULL pointers
 *       - Double-free is prevented internally
 */
int dplyr_free_string(char* s);

/**
 * @brief Free multiple strings at once
 * 
 * @param strings Array of string pointers to free
 * @param count Number of strings in the array
 * @return Number of strings successfully freed, or negative error code
 * 
 * @note Each string must have been allocated by dplyr functions
 */
int dplyr_free_strings(char** strings, size_t count);

/**
 * @brief Check if a pointer looks like a valid C string (for debugging)
 * 
 * @param s Pointer to check
 * @return true if pointer appears valid, false otherwise
 * 
 * @note This is a best-effort check and cannot guarantee validity
 */
bool dplyr_is_valid_string_pointer(const char* s);

/* ========================================================================
 * VERSION AND SYSTEM INFORMATION
 * ======================================================================== */

/**
 * @brief Get the version of the libdplyr library
 * 
 * @return Version string (static memory, no need to free)
 * 
 * @note The returned string is in static memory and should not be freed
 */
const char* dplyr_version(void);

/**
 * @brief Get detailed version information including build info
 * 
 * @return Static version string with build details (no need to free)
 */
const char* dplyr_version_detailed(void);

/**
 * @brief Get supported SQL dialects as a comma-separated string
 * 
 * @return Static string listing supported dialects (no need to free)
 */
const char* dplyr_supported_dialects(void);

/**
 * @brief Get build timestamp
 * 
 * @return Static build timestamp string (no need to free)
 */
const char* dplyr_build_timestamp(void);

/**
 * @brief Check if debug mode is available in this build
 * 
 * @return true if debug features are available, false otherwise
 */
bool dplyr_has_debug_support(void);

/**
 * @brief Get maximum supported input length
 * 
 * @return Maximum input length in bytes
 */
uint32_t dplyr_max_input_length(void);

/**
 * @brief Get maximum processing time limit
 * 
 * @return Maximum processing time in milliseconds
 */
uint64_t dplyr_max_processing_time_ms(void);

/**
 * @brief Validate system requirements and configuration
 * 
 * @return 0 if system is ready, negative error code if issues found
 */
int dplyr_check_system(void);

/* ========================================================================
 * OPTIONS MANAGEMENT
 * ======================================================================== */

/**
 * @brief Create default DplyrOptions
 * 
 * @return DplyrOptions with default settings
 */
DplyrOptions dplyr_options_default(void);

/**
 * @brief Create DplyrOptions with custom settings
 * 
 * @param strict_mode Enable strict parsing mode
 * @param preserve_comments Keep comments in output  
 * @param debug_mode Enable debug information
 * @param max_input_length Maximum input size in bytes
 * @return DplyrOptions with specified settings
 */
DplyrOptions dplyr_options_create(
    bool strict_mode,
    bool preserve_comments,
    bool debug_mode,
    uint32_t max_input_length
);

/**
 * @brief Create DplyrOptions with all settings including timeout
 * 
 * @param strict_mode Enable strict parsing mode
 * @param preserve_comments Keep comments in output  
 * @param debug_mode Enable debug information
 * @param max_input_length Maximum input size in bytes
 * @param max_processing_time_ms Maximum processing time in milliseconds (0 = use default)
 * @return DplyrOptions with specified settings
 */
DplyrOptions dplyr_options_create_with_timeout(
    bool strict_mode,
    bool preserve_comments,
    bool debug_mode,
    uint32_t max_input_length,
    uint64_t max_processing_time_ms
);

/**
 * @brief Validate DplyrOptions
 * 
 * @param options Options to validate
 * @return 0 if valid, negative error code if invalid
 */
int dplyr_options_validate(const DplyrOptions* options);

/* ========================================================================
 * ERROR HANDLING UTILITIES
 * ======================================================================== */

/**
 * @brief Get human-readable name for error code
 * 
 * @param error_code Error code to look up
 * @return Static string pointer (no need to free)
 */
const char* dplyr_error_code_name(int error_code);

/**
 * @brief Check if error code indicates success
 * 
 * @param error_code Error code to check
 * @return true if success, false if error
 */
bool dplyr_is_success(int error_code);

/**
 * @brief Check if error is recoverable
 * 
 * @param error_code Error code to check
 * @return true if recoverable, false if fatal
 */
bool dplyr_is_recoverable_error(int error_code);

/* ========================================================================
 * CACHE MANAGEMENT FUNCTIONS (R10-AC2)
 * ======================================================================== */

/**
 * @brief Get cache statistics as JSON string
 * 
 * @return JSON string with cache statistics (must be freed with dplyr_free_string)
 */
char* dplyr_cache_get_stats(void);

/**
 * @brief Get cache hit rate as percentage
 * 
 * @return Hit rate as percentage (0.0 to 100.0)
 */
double dplyr_cache_get_hit_rate(void);

/**
 * @brief Check if cache is performing effectively
 * 
 * @return true if hit rate > 50%, false otherwise
 */
bool dplyr_cache_is_effective(void);

/**
 * @brief Clear cache and reset all metrics
 * 
 * @return 0 on success, negative error code on failure
 */
int dplyr_cache_clear(void);

/**
 * @brief Get current cache size
 * 
 * @return Number of entries currently in cache
 */
size_t dplyr_cache_get_size(void);

/**
 * @brief Get maximum cache capacity
 * 
 * @return Maximum number of entries cache can hold
 */
size_t dplyr_cache_get_capacity(void);

/**
 * @brief Get total number of cache hits
 * 
 * @return Total cache hits since last clear
 */
uint64_t dplyr_cache_get_hits(void);

/**
 * @brief Get total number of cache misses
 * 
 * @return Total cache misses since last clear
 */
uint64_t dplyr_cache_get_misses(void);

/**
 * @brief Get total number of cache evictions
 * 
 * @return Total evictions since last clear
 */
uint64_t dplyr_cache_get_evictions(void);

/**
 * @brief Log cache statistics to stderr (for debugging)
 * 
 * @param prefix Optional prefix for log message (can be NULL)
 */
void dplyr_cache_log_stats(const char* prefix);

/**
 * @brief Log cache statistics with timestamp (R10-AC2: Debug mode logging)
 * 
 * @param prefix Optional prefix for log message (can be NULL)
 * @param include_timestamp Whether to include timestamp in log
 */
void dplyr_cache_log_stats_detailed(const char* prefix, bool include_timestamp);

/**
 * @brief Log cache performance warning if performance is poor (R10-AC2)
 * 
 * @return true if warning was logged, false if performance is acceptable
 */
bool dplyr_cache_log_performance_warning(void);

/**
 * @brief Check if cache should be cleared based on performance
 * 
 * @return true if cache performance is poor and should be cleared
 */
bool dplyr_cache_should_clear(void);

/* ========================================================================
 * USAGE EXAMPLES
 * ======================================================================== */

/**
 * @example Basic Usage
 * @code
 * #include "dplyr_extension.h"
 * #include <stdio.h>
 * 
 * int main() {
 *     // Initialize with default options
 *     DplyrOptions options = dplyr_options_default();
 *     options.debug_mode = true;
 *     
 *     // Validate options
 *     if (dplyr_options_validate(&options) != 0) {
 *         fprintf(stderr, "Invalid options\n");
 *         return 1;
 *     }
 *     
 *     // Transpile dplyr code
 *     char* sql = NULL;
 *     char* error = NULL;
 *     const char* dplyr_code = "mtcars %>% select(mpg, cyl) %>% filter(mpg > 20)";
 *     
 *     int result = dplyr_compile(dplyr_code, &options, &sql, &error);
 *     
 *     if (dplyr_is_success(result)) {
 *         printf("Generated SQL: %s\n", sql);
 *         dplyr_free_string(sql);
 *         
 *         // Check cache performance
 *         if (dplyr_cache_is_effective()) {
 *             printf("Cache hit rate: %.1f%%\n", dplyr_cache_get_hit_rate());
 *         }
 *     } else {
 *         fprintf(stderr, "Error (%s): %s\n", 
 *                 dplyr_error_code_name(result), error);
 *         dplyr_free_string(error);
 *         
 *         if (!dplyr_is_recoverable_error(result)) {
 *             return 1;
 *         }
 *     }
 *     
 *     return 0;
 * }
 * @endcode
 */

/**
 * @example Cache Management
 * @code
 * #include "dplyr_extension.h"
 * #include <stdio.h>
 * 
 * void monitor_cache_performance() {
 *     // Get cache statistics
 *     char* stats = dplyr_cache_get_stats();
 *     printf("Cache stats: %s\n", stats);
 *     dplyr_free_string(stats);
 *     
 *     // Check if cache needs attention
 *     if (dplyr_cache_should_clear()) {
 *         printf("Cache performance is poor, clearing...\n");
 *         dplyr_cache_clear();
 *     }
 *     
 *     // Log detailed statistics in debug mode
 *     dplyr_cache_log_stats_detailed("MONITOR", true);
 *     
 *     // Check for performance warnings
 *     if (dplyr_cache_log_performance_warning()) {
 *         printf("Cache performance warnings logged\n");
 *     }
 * }
 * @endcode
 */

/**
 * @example Error Handling
 * @code
 * #include "dplyr_extension.h"
 * #include <stdio.h>
 * 
 * int safe_transpile(const char* code) {
 *     // Validate system first
 *     if (dplyr_check_system() != 0) {
 *         fprintf(stderr, "System check failed\n");
 *         return -1;
 *     }
 *     
 *     char* sql = NULL;
 *     char* error = NULL;
 *     DplyrOptions options = dplyr_options_default();
 *     
 *     int result = dplyr_compile(code, &options, &sql, &error);
 *     
 *     switch (result) {
 *         case DPLYR_SUCCESS:
 *             printf("Success: %s\n", sql);
 *             dplyr_free_string(sql);
 *             break;
 *             
 *         case DPLYR_ERROR_NULL_POINTER:
 *         case DPLYR_ERROR_INVALID_UTF8:
 *             fprintf(stderr, "Input validation error: %s\n", error);
 *             dplyr_free_string(error);
 *             return -1;
 *             
 *         case DPLYR_ERROR_INPUT_TOO_LARGE:
 *         case DPLYR_ERROR_TIMEOUT:
 *             fprintf(stderr, "Resource limit exceeded: %s\n", error);
 *             dplyr_free_string(error);
 *             return -2;
 *             
 *         case DPLYR_ERROR_SYNTAX:
 *         case DPLYR_ERROR_UNSUPPORTED:
 *             fprintf(stderr, "Transpilation error: %s\n", error);
 *             dplyr_free_string(error);
 *             return -3;
 *             
 *         case DPLYR_ERROR_INTERNAL:
 *         case DPLYR_ERROR_PANIC:
 *             fprintf(stderr, "Internal error: %s\n", error);
 *             dplyr_free_string(error);
 *             return -4;
 *             
 *         default:
 *             fprintf(stderr, "Unknown error code: %d\n", result);
 *             if (error) dplyr_free_string(error);
 *             return -5;
 *     }
 *     
 *     return 0;
 * }
 * @endcode
 */

/* ========================================================================
 * THREAD SAFETY DOCUMENTATION (R9-AC3)
 * ======================================================================== */

/**
 * @section thread_safety Thread Safety
 * 
 * All functions in this API are designed to be thread-safe with the following characteristics:
 * 
 * **Thread-Safe Functions (no external synchronization required):**
 * - dplyr_compile() - Uses thread-local caching, safe for concurrent access
 * - dplyr_free_string() - Safe memory deallocation across threads
 * - dplyr_free_strings() - Safe batch memory deallocation
 * - All utility functions (dplyr_version, dplyr_max_*, etc.) - Read-only static data
 * - All error handling functions - Stateless operations
 * - All options functions - Value-based operations
 * - All cache functions - Use thread-local storage
 * 
 * **Cache Isolation:**
 * Each thread maintains its own cache (thread_local storage). This means:
 * - Cache entries created in one thread are not visible to other threads
 * - Cache statistics are per-thread
 * - No cache contention between threads
 * - Each thread can have different cache performance characteristics
 * 
 * **Memory Management:**
 * - Strings allocated by one thread can be safely freed by another thread
 * - Memory allocation/deallocation is handled by the Rust runtime
 * - No shared mutable state in memory management functions
 * 
 * **Panic Safety:**
 * All FFI functions use panic::catch_unwind to prevent Rust panics from
 * crossing the FFI boundary, ensuring thread stability.
 * 
 * **Reentrancy:**
 * Functions are reentrant - the same function can be called recursively
 * or from signal handlers without issues.
 * 
 * @example Thread Safety Example
 * @code
 * #include <pthread.h>
 * #include "dplyr_extension.h"
 * 
 * void* worker_thread(void* arg) {
 *     int thread_id = *(int*)arg;
 *     
 *     // Each thread can safely call dplyr_compile concurrently
 *     char* sql = NULL;
 *     char* error = NULL;
 *     
 *     char code[100];
 *     snprintf(code, sizeof(code), "select(col_%d)", thread_id);
 *     
 *     int result = dplyr_compile(code, NULL, &sql, &error);
 *     
 *     if (result == 0) {
 *         printf("Thread %d: %s\n", thread_id, sql);
 *         dplyr_free_string(sql);
 *     } else {
 *         printf("Thread %d error: %s\n", thread_id, error);
 *         dplyr_free_string(error);
 *     }
 *     
 *     return NULL;
 * }
 * 
 * int main() {
 *     pthread_t threads[4];
 *     int thread_ids[4] = {0, 1, 2, 3};
 *     
 *     // Create multiple threads
 *     for (int i = 0; i < 4; i++) {
 *         pthread_create(&threads[i], NULL, worker_thread, &thread_ids[i]);
 *     }
 *     
 *     // Wait for all threads
 *     for (int i = 0; i < 4; i++) {
 *         pthread_join(threads[i], NULL);
 *     }
 *     
 *     return 0;
 * }
 * @endcode
 */

/* ========================================================================
 * COMPATIBILITY AND VERSIONING
 * ======================================================================== */

/**
 * @brief Minimum supported DuckDB version
 */
#define DPLYR_MIN_DUCKDB_VERSION "0.9.0"

/**
 * @brief Maximum supported DuckDB version
 */
#define DPLYR_MAX_DUCKDB_VERSION "1.0.0"

/**
 * @brief API version for compatibility checking
 */
#define DPLYR_API_VERSION 1

/**
 * @brief Check API compatibility
 * 
 * @param required_version Required API version
 * @return true if compatible, false otherwise
 */
static inline bool dplyr_is_api_compatible(int required_version) {
    return required_version <= DPLYR_API_VERSION;
}

#ifdef __cplusplus
}
#endif

#ifdef __cplusplus
#include "duckdb.hpp"

namespace dplyr_extension {

class DplyrExtension : public duckdb::Extension {
public:
    void Load(duckdb::ExtensionLoader &loader) override;
    std::string Name() override;
    std::string Version() const override { return dplyr_version(); }
};

// Parser Extension
struct DplyrParserExtension : public duckdb::ParserExtension {
    DplyrParserExtension();
};

// Operator Extension
struct DplyrOperatorExtension : public duckdb::OperatorExtension {
    DplyrOperatorExtension();
    std::string GetName() override { return "dplyr"; }
    duckdb::unique_ptr<duckdb::LogicalExtensionOperator> Deserialize(duckdb::Deserializer &deserializer) override;
};

// Parse Data
struct DplyrParseData : duckdb::ParserExtensionParseData {
    duckdb::unique_ptr<duckdb::SQLStatement> statement;

    DplyrParseData(duckdb::unique_ptr<duckdb::SQLStatement> statement)
        : statement(std::move(statement)) {}

    duckdb::unique_ptr<duckdb::ParserExtensionParseData> Copy() const override {
        return duckdb::make_uniq_base<duckdb::ParserExtensionParseData, DplyrParseData>(statement->Copy());
    }

    std::string ToString() const override { return "DplyrParseData"; }
};

// Client Context State
class DplyrState : public duckdb::ClientContextState {
public:
    explicit DplyrState(duckdb::unique_ptr<duckdb::ParserExtensionParseData> parse_data)
        : parse_data(std::move(parse_data)) {}

    void QueryEnd() override { parse_data.reset(); }

    duckdb::unique_ptr<duckdb::ParserExtensionParseData> parse_data;
};

// Function declarations
duckdb::ParserExtensionParseResult dplyr_parse(duckdb::ParserExtensionInfo *info, const std::string &query);
duckdb::ParserExtensionPlanResult dplyr_plan(duckdb::ParserExtensionInfo *info, duckdb::ClientContext &context, duckdb::unique_ptr<duckdb::ParserExtensionParseData> parse_data);
duckdb::BoundStatement dplyr_bind(duckdb::ClientContext &context, duckdb::Binder &binder, duckdb::OperatorExtensionInfo *info, duckdb::SQLStatement &statement);

} // namespace dplyr_extension
#endif

#endif /* DPLYR_EXTENSION_H */
