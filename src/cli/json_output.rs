//! JSON output formatting module
//!
//! Provides JSON output formatting with metadata for SQL transpilation results.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Result type for JSON output operations
pub type JsonResult<T> = Result<T, JsonError>;

/// Errors that can occur during JSON output processing
#[derive(Debug, thiserror::Error)]
pub enum JsonError {
    #[error("JSON serialization failed: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Metadata collection failed: {0}")]
    MetadataError(String),
    
    #[error("Invalid input data: {0}")]
    InvalidInput(String),
}

/// Transpilation metadata containing processing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranspileMetadata {
    /// Timestamp when transpilation started (Unix timestamp)
    pub timestamp: u64,
    
    /// SQL dialect used for transpilation
    pub dialect: String,
    
    /// Processing statistics
    pub stats: ProcessingStats,
    
    /// Input information
    pub input_info: InputInfo,
    
    /// Version information
    pub version: String,
}

/// Processing statistics for transpilation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingStats {
    /// Time taken for lexical analysis (microseconds)
    pub lex_time_us: u64,
    
    /// Time taken for parsing (microseconds)
    pub parse_time_us: u64,
    
    /// Time taken for SQL generation (microseconds)
    pub generation_time_us: u64,
    
    /// Total processing time (microseconds)
    pub total_time_us: u64,
    
    /// Number of tokens generated
    pub token_count: usize,
    
    /// Number of AST nodes created
    pub ast_node_count: usize,
    
    /// Input size in bytes
    pub input_size_bytes: usize,
    
    /// Output size in bytes
    pub output_size_bytes: usize,
}

/// Input source information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputInfo {
    /// Source type (file, stdin, text)
    pub source_type: String,
    
    /// Source identifier (filename, "stdin", "text")
    pub source_id: String,
    
    /// Input size in bytes
    pub size_bytes: usize,
    
    /// Number of lines in input
    pub line_count: usize,
}

/// JSON output format for transpilation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonOutput {
    /// Success status
    pub success: bool,
    
    /// Generated SQL (if successful)
    pub sql: Option<String>,
    
    /// Error information (if failed)
    pub error: Option<ErrorInfo>,
    
    /// Transpilation metadata
    pub metadata: TranspileMetadata,
}

/// Error information for failed transpilations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    /// Error type (lex, parse, generation)
    pub error_type: String,
    
    /// Error message
    pub message: String,
    
    /// Position information (if available)
    pub position: Option<usize>,
    
    /// Suggestions for fixing the error
    pub suggestions: Vec<String>,
}

/// JSON output formatter
#[derive(Debug)]
pub struct JsonOutputFormatter {
    /// Whether to pretty-print JSON
    pub pretty_print: bool,
    
    /// Whether to include debug information
    pub include_debug: bool,
}

impl JsonOutputFormatter {
    /// Creates a new JSON output formatter
    pub fn new() -> Self {
        Self {
            pretty_print: false,
            include_debug: false,
        }
    }
    
    /// Creates a new JSON output formatter with pretty printing
    pub fn pretty() -> Self {
        Self {
            pretty_print: true,
            include_debug: false,
        }
    }
    
    /// Creates a new JSON output formatter with debug information
    pub fn with_debug() -> Self {
        Self {
            pretty_print: false,
            include_debug: true,
        }
    }
    
    /// Formats a successful transpilation result as JSON
    pub fn format_success(
        &self,
        sql: &str,
        metadata: TranspileMetadata,
    ) -> JsonResult<String> {
        let output = JsonOutput {
            success: true,
            sql: Some(sql.to_string()),
            error: None,
            metadata,
        };
        
        self.serialize_output(&output)
    }
    
    /// Formats a failed transpilation result as JSON
    pub fn format_error(
        &self,
        error_info: ErrorInfo,
        metadata: TranspileMetadata,
    ) -> JsonResult<String> {
        let output = JsonOutput {
            success: false,
            sql: None,
            error: Some(error_info),
            metadata,
        };
        
        self.serialize_output(&output)
    }
    
    /// Formats a successful validation result as JSON
    pub fn format_validation_success(
        &self,
        summary: &crate::cli::validator::ValidationSummary,
        metadata: &TranspileMetadata,
    ) -> String {
        let output = serde_json::json!({
            "success": true,
            "validation": {
                "valid": true,
                "summary": {
                    "operation_count": summary.operation_count,
                    "operations": summary.operations,
                    "column_count": summary.column_count,
                    "columns": summary.columns,
                    "has_aggregation": summary.has_aggregation,
                    "has_grouping": summary.has_grouping,
                    "complexity_score": summary.complexity_score
                }
            },
            "metadata": metadata
        });
        
        if self.pretty_print {
            serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
        } else {
            serde_json::to_string(&output).unwrap_or_else(|_| "{}".to_string())
        }
    }
    
    /// Formats a validation error as JSON
    pub fn format_validation_error(
        &self,
        error: &crate::cli::validator::ValidationErrorInfo,
        suggestions: &[String],
    ) -> String {
        let output = serde_json::json!({
            "success": false,
            "validation": {
                "valid": false,
                "error": {
                    "type": error.error_type,
                    "message": error.message,
                    "position": error.position,
                    "context": error.context
                },
                "suggestions": suggestions
            }
        });
        
        if self.pretty_print {
            serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
        } else {
            serde_json::to_string(&output).unwrap_or_else(|_| "{}".to_string())
        }
    }
    
    /// Formats a transpilation result as JSON
    pub fn format_transpile_result(&self, sql: &str, metadata: &TranspileMetadata) -> String {
        let output = serde_json::json!({
            "success": true,
            "sql": sql,
            "metadata": metadata
        });
        
        if self.pretty_print {
            serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
        } else {
            serde_json::to_string(&output).unwrap_or_else(|_| "{}".to_string())
        }
    }
    
    /// Serializes the JSON output
    fn serialize_output(&self, output: &JsonOutput) -> JsonResult<String> {
        if self.pretty_print {
            Ok(serde_json::to_string_pretty(output)?)
        } else {
            Ok(serde_json::to_string(output)?)
        }
    }
}

impl Default for JsonOutputFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating TranspileMetadata
#[derive(Debug)]
pub struct MetadataBuilder {
    dialect: String,
    stats: ProcessingStats,
    input_info: InputInfo,
    version: String,
}

impl MetadataBuilder {
    /// Creates a new metadata builder
    pub fn new(dialect: &str) -> Self {
        Self {
            dialect: dialect.to_string(),
            stats: ProcessingStats {
                lex_time_us: 0,
                parse_time_us: 0,
                generation_time_us: 0,
                total_time_us: 0,
                token_count: 0,
                ast_node_count: 0,
                input_size_bytes: 0,
                output_size_bytes: 0,
            },
            input_info: InputInfo {
                source_type: "unknown".to_string(),
                source_id: "unknown".to_string(),
                size_bytes: 0,
                line_count: 0,
            },
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
    
    /// Sets processing statistics
    pub fn with_stats(mut self, stats: ProcessingStats) -> Self {
        self.stats = stats;
        self
    }
    
    /// Sets input information
    pub fn with_input_info(mut self, input_info: InputInfo) -> Self {
        self.input_info = input_info;
        self
    }
    
    /// Sets version information
    pub fn with_version(mut self, version: &str) -> Self {
        self.version = version.to_string();
        self
    }
    
    /// Builds the metadata
    pub fn build(self) -> TranspileMetadata {
        TranspileMetadata {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            dialect: self.dialect,
            stats: self.stats,
            input_info: self.input_info,
            version: self.version,
        }
    }
}

/// Helper functions for creating common metadata components
impl ProcessingStats {
    /// Creates empty processing stats
    pub fn empty() -> Self {
        Self {
            lex_time_us: 0,
            parse_time_us: 0,
            generation_time_us: 0,
            total_time_us: 0,
            token_count: 0,
            ast_node_count: 0,
            input_size_bytes: 0,
            output_size_bytes: 0,
        }
    }
    
    /// Creates processing stats with timing information
    pub fn with_timing(
        lex_time_us: u64,
        parse_time_us: u64,
        generation_time_us: u64,
    ) -> Self {
        Self {
            lex_time_us,
            parse_time_us,
            generation_time_us,
            total_time_us: lex_time_us + parse_time_us + generation_time_us,
            token_count: 0,
            ast_node_count: 0,
            input_size_bytes: 0,
            output_size_bytes: 0,
        }
    }
}

impl InputInfo {
    /// Creates input info for file source
    pub fn from_file(filename: &str, content: &str) -> Self {
        Self {
            source_type: "file".to_string(),
            source_id: filename.to_string(),
            size_bytes: content.len(),
            line_count: content.lines().count(),
        }
    }
    
    /// Creates input info for stdin source
    pub fn from_stdin(content: &str) -> Self {
        Self {
            source_type: "stdin".to_string(),
            source_id: "stdin".to_string(),
            size_bytes: content.len(),
            line_count: content.lines().count(),
        }
    }
    
    /// Creates input info for text source
    pub fn from_text(content: &str) -> Self {
        Self {
            source_type: "text".to_string(),
            source_id: "command_line".to_string(),
            size_bytes: content.len(),
            line_count: content.lines().count(),
        }
    }
}

impl ErrorInfo {
    /// Creates error info from a transpile error
    pub fn from_transpile_error(error: &crate::TranspileError) -> Self {
        match error {
            crate::TranspileError::LexError(e) => Self {
                error_type: "lex".to_string(),
                message: e.to_string(),
                position: None,
                suggestions: vec![
                    "Check for invalid characters or malformed strings".to_string(),
                    "Ensure proper quoting of string literals".to_string(),
                ],
            },
            crate::TranspileError::ParseError(e) => Self {
                error_type: "parse".to_string(),
                message: e.to_string(),
                position: None,
                suggestions: vec![
                    "Check dplyr function syntax and arguments".to_string(),
                    "Ensure proper use of pipe operator (%>%)".to_string(),
                    "Verify function names are spelled correctly".to_string(),
                ],
            },
            crate::TranspileError::GenerationError(e) => Self {
                error_type: "generation".to_string(),
                message: e.to_string(),
                position: None,
                suggestions: vec![
                    "Try a different SQL dialect".to_string(),
                    "Simplify complex expressions".to_string(),
                    "Check if the operation is supported in the selected dialect".to_string(),
                ],
            },
            crate::TranspileError::IoError(e) => Self {
                error_type: "io".to_string(),
                message: e.to_string(),
                position: None,
                suggestions: vec![
                    "Check file permissions and paths".to_string(),
                    "Ensure input/output resources are available".to_string(),
                ],
            },
            crate::TranspileError::ValidationError(e) => Self {
                error_type: "validation".to_string(),
                message: e.to_string(),
                position: None,
                suggestions: vec![
                    "Check dplyr syntax and function usage".to_string(),
                    "Verify all required arguments are provided".to_string(),
                ],
            },
            crate::TranspileError::ConfigurationError(e) => Self {
                error_type: "configuration".to_string(),
                message: e.to_string(),
                position: None,
                suggestions: vec![
                    "Check configuration settings".to_string(),
                    "Verify all required options are provided".to_string(),
                ],
            },
            crate::TranspileError::SystemError(e) => Self {
                error_type: "system".to_string(),
                message: e.to_string(),
                position: None,
                suggestions: vec![
                    "Check system permissions".to_string(),
                    "Verify signal handling or pipeline configuration".to_string(),
                ],
            },
        }
    }
}

impl TranspileMetadata {
    /// Creates metadata from a validation summary
    pub fn from_validation_summary(_summary: &crate::cli::validator::ValidationSummary) -> Self {
        Self {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            dialect: "validation".to_string(),
            stats: ProcessingStats {
                lex_time_us: 0,
                parse_time_us: 0,
                generation_time_us: 0,
                total_time_us: 0,
                token_count: 0,
                ast_node_count: 0,
                input_size_bytes: 0,
                output_size_bytes: 0,
            },
            input_info: InputInfo {
                source_type: "validation".to_string(),
                source_id: "validation".to_string(),
                size_bytes: 0,
                line_count: 0,
            },
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
    
    /// Creates metadata for successful transpilation
    pub fn transpilation_success(
        dialect: &crate::cli::SqlDialectType,
        processing_time: std::time::Duration,
        input: &str,
        output: &str,
    ) -> Self {
        Self {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            dialect: format!("{:?}", dialect).to_lowercase(),
            stats: ProcessingStats {
                lex_time_us: 0,
                parse_time_us: 0,
                generation_time_us: 0,
                total_time_us: processing_time.as_micros() as u64,
                token_count: 0,
                ast_node_count: 0,
                input_size_bytes: input.len(),
                output_size_bytes: output.len(),
            },
            input_info: InputInfo {
                source_type: "transpilation".to_string(),
                source_id: "transpilation".to_string(),
                size_bytes: input.len(),
                line_count: input.lines().count(),
            },
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_json_output_formatter_creation() {
        let formatter = JsonOutputFormatter::new();
        assert!(!formatter.pretty_print);
        assert!(!formatter.include_debug);
        
        let pretty_formatter = JsonOutputFormatter::pretty();
        assert!(pretty_formatter.pretty_print);
        assert!(!pretty_formatter.include_debug);
        
        let debug_formatter = JsonOutputFormatter::with_debug();
        assert!(!debug_formatter.pretty_print);
        assert!(debug_formatter.include_debug);
    }
    
    #[test]
    fn test_metadata_builder() {
        let metadata = MetadataBuilder::new("postgresql")
            .with_version("1.0.0")
            .build();
        
        assert_eq!(metadata.dialect, "postgresql");
        assert_eq!(metadata.version, "1.0.0");
        assert!(metadata.timestamp > 0);
    }
    
    #[test]
    fn test_processing_stats() {
        let stats = ProcessingStats::empty();
        assert_eq!(stats.total_time_us, 0);
        
        let stats = ProcessingStats::with_timing(100, 200, 300);
        assert_eq!(stats.lex_time_us, 100);
        assert_eq!(stats.parse_time_us, 200);
        assert_eq!(stats.generation_time_us, 300);
        assert_eq!(stats.total_time_us, 600);
    }
    
    #[test]
    fn test_input_info() {
        let info = InputInfo::from_file("test.R", "data %>% select(name)");
        assert_eq!(info.source_type, "file");
        assert_eq!(info.source_id, "test.R");
        assert!(info.size_bytes > 0);
        assert_eq!(info.line_count, 1);
        
        let info = InputInfo::from_stdin("data %>% select(name)");
        assert_eq!(info.source_type, "stdin");
        assert_eq!(info.source_id, "stdin");
        
        let info = InputInfo::from_text("data %>% select(name)");
        assert_eq!(info.source_type, "text");
        assert_eq!(info.source_id, "command_line");
    }
    
    #[test]
    fn test_json_output_success() {
        let formatter = JsonOutputFormatter::new();
        let metadata = MetadataBuilder::new("postgresql").build();
        
        let result = formatter.format_success("SELECT * FROM data", metadata);
        assert!(result.is_ok());
        
        let json = result.unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("SELECT * FROM data"));
        assert!(json.contains("postgresql"));
    }
    
    #[test]
    fn test_json_output_error() {
        let formatter = JsonOutputFormatter::new();
        let metadata = MetadataBuilder::new("postgresql").build();
        let error_info = ErrorInfo {
            error_type: "parse".to_string(),
            message: "Invalid syntax".to_string(),
            position: Some(10),
            suggestions: vec!["Check syntax".to_string()],
        };
        
        let result = formatter.format_error(error_info, metadata);
        assert!(result.is_ok());
        
        let json = result.unwrap();
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("Invalid syntax"));
        assert!(json.contains("parse"));
    }
    
    #[test]
    fn test_pretty_print() {
        let formatter = JsonOutputFormatter::pretty();
        let metadata = MetadataBuilder::new("postgresql").build();
        
        let result = formatter.format_success("SELECT * FROM data", metadata);
        assert!(result.is_ok());
        
        let json = result.unwrap();
        // Pretty printed JSON should contain newlines and indentation
        assert!(json.contains('\n'));
        assert!(json.contains("  "));
    }
    
    #[test]
    fn test_error_info_from_transpile_error() {
        // This test would require actual TranspileError instances
        // For now, we'll test the structure
        let error_info = ErrorInfo {
            error_type: "lex".to_string(),
            message: "Unexpected character".to_string(),
            position: Some(5),
            suggestions: vec!["Check for invalid characters".to_string()],
        };
        
        assert_eq!(error_info.error_type, "lex");
        assert_eq!(error_info.message, "Unexpected character");
        assert_eq!(error_info.position, Some(5));
        assert_eq!(error_info.suggestions.len(), 1);
    }
    
    #[test]
    fn test_json_serialization() {
        let output = JsonOutput {
            success: true,
            sql: Some("SELECT * FROM data".to_string()),
            error: None,
            metadata: MetadataBuilder::new("postgresql").build(),
        };
        
        let json = serde_json::to_string(&output);
        assert!(json.is_ok());
        
        let deserialized: Result<JsonOutput, _> = serde_json::from_str(&json.unwrap());
        assert!(deserialized.is_ok());
        
        let deserialized = deserialized.unwrap();
        assert!(deserialized.success);
        assert_eq!(deserialized.sql, Some("SELECT * FROM data".to_string()));
    }
}