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
 */

#ifndef DPLYR_H
#define DPLYR_H

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
/** @brief Query did not contain a dplyr pipeline and should be ignored by parser hooks */
#define DPLYR_QUERY_NOT_HANDLED 1

/** @brief FFI-related errors (invalid parameters, encoding issues) */
#define DPLYR_ERROR_NULL_POINTER (-1)
#define DPLYR_ERROR_INVALID_UTF8 (-2)

/** @brief Input validation errors (DoS prevention) */
#define DPLYR_ERROR_INPUT_TOO_LARGE (-3)
#define DPLYR_ERROR_TIMEOUT (-4)

/** @brief Transpilation errors (syntax, unsupported operations) */
#define DPLYR_ERROR_SYNTAX (-5)
#define DPLYR_ERROR_UNSUPPORTED (-6)

/** @brief Internal errors (panic, system issues) */
#define DPLYR_ERROR_INTERNAL (-7)
#define DPLYR_ERROR_PANIC (-8)

/**
 * @brief Supported SQL dialects for the generic C API.
 */
typedef enum DplyrDialect {
    DPLYR_DIALECT_DUCKDB = 0,
    DPLYR_DIALECT_POSTGRESQL = 1,
    DPLYR_DIALECT_MYSQL = 2,
    DPLYR_DIALECT_SQLITE = 3
} DplyrDialect;

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
    bool debug_mode;                /**< Enable debug logging (R10-AC1) */
    uint32_t max_input_length;      /**< Maximum input length for DoS prevention (R9-AC2) */
    uint64_t max_processing_time_ms; /**< Maximum processing time in milliseconds (0 = use default) (R9-AC2) */
    uint32_t dialect;               /**< SQL dialect selection as a DPLYR_DIALECT_* value */
} DplyrOptions;

/* ========================================================================
 * CORE TRANSPILATION FUNCTIONS
 * ======================================================================== */

/**
 * @brief Convert dplyr pipeline code to SQL
 * 
 * This function transpiles dplyr pipeline syntax to equivalent SQL
 * for the selected SQL dialect. It handles the minimum operation set
 * (select, filter, mutate, arrange, summarise, group_by) as specified
 * in requirement R1-AC2.
 * 
 * @param code The dplyr pipeline code to transpile (must be valid UTF-8)
 * @param options Transpilation options (can be NULL for defaults)
 * @param out_sql Pointer to receive the generated SQL string (allocated by this function)
 * @param out_error Pointer to receive error message if transpilation fails
 * 
 * @return 0 on success, or a negative error code on failure:
 *         - `DPLYR_ERROR_NULL_POINTER` (-1): A required pointer argument was null
 *         - `DPLYR_ERROR_INVALID_UTF8` (-2): Input string is not valid UTF-8
 *         - `DPLYR_ERROR_INPUT_TOO_LARGE` (-3): Input exceeds the configured size limit
 *         - `DPLYR_ERROR_TIMEOUT` (-4): Processing exceeded the configured time limit
 *         - `DPLYR_ERROR_SYNTAX` (-5): The dplyr pipeline has a syntax error
 *         - `DPLYR_ERROR_UNSUPPORTED` (-6): The pipeline uses an unsupported feature
 *         - `DPLYR_ERROR_INTERNAL` (-7): An unexpected internal error occurred
 *         - `DPLYR_ERROR_PANIC` (-8): A panic occurred, indicating a bug
 * 
 * @note Memory management (R3-AC3): 
 *       - out_sql and out_error are allocated by this function
 *       - On entry, *out_sql and *out_error should be NULL or pointers previously allocated by libdplyr
 *       - Any non-NULL incoming libdplyr-owned pointer is reclaimed by this function before reuse
 *       - Callers must initialize output slots to NULL before the first call; use
 *         dplyr_init_output_string() if you want an explicit helper for this step
 *       - Foreign pointers are not reclaimed; libdplyr only frees pointers it can prove it allocated
 *       - Caller MUST call dplyr_free_string() to release memory
 *       - Only one of out_sql or out_error will be set (never both)
 *       - If the function returns `DPLYR_ERROR_PANIC`, callers must not assume `out_error`
 *         contains a valid message because the panic fallback avoids heap work
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
 * DplyrOptions options = {false, 1024*1024, 0, DPLYR_DIALECT_DUCKDB};
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

/**
 * @brief Compile a full query, including embedded `(| ... |)` dplyr segments.
 *
 * Returns `DPLYR_QUERY_NOT_HANDLED` when the query does not contain a dplyr
 * pipeline. On success, `out_sql` receives either the rewritten SQL query or
 * the compiled pure pipeline SQL.
 *
 * On entry, `*out_sql` and `*out_error` must be `NULL` or pointers previously
 * allocated by libdplyr. Any non-NULL incoming libdplyr-owned pointer is
 * reclaimed by this function before reuse. Callers must initialize output
 * slots to `NULL` before the first call; use dplyr_init_output_string() if you
 * want an explicit helper for that setup. libdplyr cannot validate foreign
 * pointer provenance at runtime before reclaiming a reused output pointer.
 * If the function returns `DPLYR_ERROR_PANIC`, callers must not assume
 * `out_error` contains a valid message because the panic fallback avoids heap work.
 */
int dplyr_compile_query(
    const char* query,
    const DplyrOptions* options,
    char** out_sql,
    char** out_error
);

/* ========================================================================
 * MEMORY MANAGEMENT FUNCTIONS
 * ======================================================================== */

/**
 * @brief Initialize an output slot to NULL before the first FFI call.
 *
 * This helper is intended for C callers that want an explicit initialization
 * step before passing `char**` output slots into dplyr functions.
 *
 * @param out Output slot to initialize
 * @return 0 on success, negative error code if `out` is NULL
 */
int dplyr_init_output_string(char** out);

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

/* ========================================================================
 * VERSION INFORMATION
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
 * @param debug_mode Enable debug information
 * @param max_input_length Maximum input size in bytes
 * @param dialect SQL dialect to target
 * @return DplyrOptions with specified settings
 */
DplyrOptions dplyr_options_create(
    bool debug_mode,
    uint32_t max_input_length,
    uint32_t dialect
);

/**
 * @brief Create DplyrOptions with all settings including timeout
 * 
 * @param debug_mode Enable debug information
 * @param max_input_length Maximum input size in bytes
 * @param max_processing_time_ms Maximum processing time in milliseconds (0 = use default)
 * @param dialect SQL dialect to target
 * @return DplyrOptions with specified settings
 */
DplyrOptions dplyr_options_create_with_timeout(
    bool debug_mode,
    uint32_t max_input_length,
    uint64_t max_processing_time_ms,
    uint32_t dialect
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
 * @return true if the result is non-error (`DPLYR_SUCCESS` or `DPLYR_QUERY_NOT_HANDLED`)
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
 * @example Error Handling
 * @code
 * #include "dplyr_extension.h"
 * #include <stdio.h>
 * 
 * int safe_transpile(const char* code) {
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
#define DPLYR_MIN_DUCKDB_VERSION "1.4.0"

/**
 * @brief Maximum supported DuckDB version
 */
#define DPLYR_MAX_DUCKDB_VERSION "1.5.2"

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

#endif /* DPLYR_EXTENSION_H */
