#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <ostream>
#include <new>

/// Success - operation completed successfully
constexpr static const int32_t ExitCode_SUCCESS = 0;

/// General error - unspecified error occurred
constexpr static const int32_t ExitCode_GENERAL_ERROR = 1;

/// Invalid arguments - command line arguments are invalid
constexpr static const int32_t ExitCode_INVALID_ARGUMENTS = 2;

/// Input/Output error - file or stdin/stdout operations failed
constexpr static const int32_t ExitCode_IO_ERROR = 3;

/// Validation error - dplyr syntax validation failed
constexpr static const int32_t ExitCode_VALIDATION_ERROR = 4;

/// Transpilation error - SQL generation failed
constexpr static const int32_t ExitCode_TRANSPILATION_ERROR = 5;

/// Configuration error - invalid configuration or settings
constexpr static const int32_t ExitCode_CONFIG_ERROR = 6;

/// Permission error - insufficient permissions
constexpr static const int32_t ExitCode_PERMISSION_ERROR = 7;

/// System error - system-level operations failed (signals, pipes, etc.)
constexpr static const int32_t ExitCode_SYSTEM_ERROR = 8;

/// Network error - network-related operations failed
constexpr static const int32_t ExitCode_NETWORK_ERROR = 9;

/// Timeout error - operation timed out
constexpr static const int32_t ExitCode_TIMEOUT_ERROR = 10;

/// Internal error - unexpected internal error
constexpr static const int32_t ExitCode_INTERNAL_ERROR = 11;

/// Result structure for C API
struct DplyrTranspileResult {
  /// Pointer to the transpiled SQL string on success (NULL on failure)
  char *output_sql;
  /// Pointer to the error message on failure (NULL on success)
  char *error_msg;
};

extern "C" {

/// Transpiles dplyr code to DuckDB SQL via C API
///
/// # Arguments
/// * `dplyr_src` - Null-terminated C string containing dplyr code
/// * `dialect` - Null-terminated C string specifying the SQL dialect (currently "duckdb" only)
///
/// # Returns
/// A DplyrTranspileResult containing either the transpiled SQL or an error message.
/// The caller is responsible for freeing both pointers using `dplyr_free`.
///
/// # Safety
/// This function is unsafe because it deals with raw C pointers.
/// The caller must ensure that both input pointers are valid null-terminated C strings.
/// The returned pointers must be freed with `dplyr_free` to avoid memory leaks.
DplyrTranspileResult dplyr_to_sql(const char *dplyr_src, const char *dialect);

/// Frees memory allocated by the C API
///
/// # Arguments
/// * `ptr` - Pointer to the memory to free (can be NULL)
///
/// # Safety
/// This function is unsafe because it deals with raw C pointers.
/// The pointer must have been allocated by `dplyr_to_sql` or be NULL.
/// After calling this function, the pointer becomes invalid.
void dplyr_free(char *ptr);

}  // extern "C"
