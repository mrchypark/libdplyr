//! Output formatting module
//!
//! Provides various output formatting options for SQL queries including
//! pretty formatting, compact formatting, and basic formatting.

use std::fmt;

/// Result type for output formatting operations
pub type FormatResult<T> = Result<T, FormatError>;

/// Errors that can occur during output formatting
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("Invalid SQL input: {0}")]
    InvalidSql(String),
    
    #[error("Formatting failed: {0}")]
    FormattingFailed(String),
}

/// Output format types
#[derive(Debug, Clone, PartialEq)]
pub enum OutputFormat {
    /// Default format - basic processing
    Default,
    /// Basic format - minimal processing
    Basic,
    /// Pretty format - formatted with proper indentation and line breaks
    Pretty,
    /// Compact format - minimal whitespace
    Compact,
    /// JSON format - structured JSON output
    Json,
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputFormat::Default => write!(f, "default"),
            OutputFormat::Basic => write!(f, "basic"),
            OutputFormat::Pretty => write!(f, "pretty"),
            OutputFormat::Compact => write!(f, "compact"),
            OutputFormat::Json => write!(f, "json"),
        }
    }
}

/// Configuration for output formatting
#[derive(Debug, Clone)]
pub struct FormatConfig {
    /// The output format to use
    pub format: OutputFormat,
    /// Whether to add a trailing newline
    pub add_newline: bool,
    /// Indentation string for pretty formatting
    pub indent: String,
    /// Whether to preserve original case
    pub preserve_case: bool,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            format: OutputFormat::Basic,
            add_newline: true,
            indent: "  ".to_string(),
            preserve_case: true,
        }
    }
}

/// Output formatter for SQL queries
#[derive(Debug)]
pub struct OutputFormatter {
    config: FormatConfig,
}

impl OutputFormatter {
    /// Creates a new OutputFormatter with default configuration
    pub fn new() -> Self {
        Self {
            config: FormatConfig::default(),
        }
    }
    
    /// Creates a new OutputFormatter with the specified format
    pub fn with_format(format: OutputFormat) -> Self {
        Self {
            config: FormatConfig {
                format,
                ..Default::default()
            },
        }
    }
    
    /// Creates a new OutputFormatter with custom configuration
    pub fn with_config(config: FormatConfig) -> Self {
        Self { config }
    }
    
    /// Formats SQL according to the configured format
    pub fn format(&self, sql: &str) -> FormatResult<String> {
        if sql.trim().is_empty() {
            return Err(FormatError::InvalidSql("Empty SQL input".to_string()));
        }
        
        let formatted = match self.config.format {
            OutputFormat::Default | OutputFormat::Basic => self.format_basic(sql),
            OutputFormat::Pretty => self.format_pretty(sql),
            OutputFormat::Compact => self.format_compact(sql),
            OutputFormat::Json => {
                // JSON formatting is handled by JsonOutputFormatter
                // For now, just return basic formatting
                self.format_basic(sql)
            }
        }?;
        
        Ok(self.apply_final_formatting(formatted))
    }
    
    /// Basic formatting - minimal processing
    fn format_basic(&self, sql: &str) -> FormatResult<String> {
        // Clean up extra whitespace while preserving single spaces
        let mut result = String::new();
        let mut prev_was_space = false;
        
        for ch in sql.chars() {
            if ch.is_whitespace() {
                if !prev_was_space {
                    result.push(' ');
                    prev_was_space = true;
                }
            } else {
                result.push(ch);
                prev_was_space = false;
            }
        }
        
        Ok(result.trim().to_string())
    }
    
    /// Pretty formatting with proper indentation and line breaks
    fn format_pretty(&self, sql: &str) -> FormatResult<String> {
        // Start with the original SQL and normalize whitespace manually
        let mut formatted = sql.to_string();
        
        // First, normalize multiple whitespaces to single spaces
        while formatted.contains("  ") {
            formatted = formatted.replace("  ", " ");
        }
        formatted = formatted.trim().to_string();
        
        // Join clauses (process specific joins first before generic JOIN)
        formatted = formatted.replace(" LEFT JOIN ", &format!("\n{}LEFT JOIN ", self.config.indent));
        formatted = formatted.replace(" RIGHT JOIN ", &format!("\n{}RIGHT JOIN ", self.config.indent));
        formatted = formatted.replace(" INNER JOIN ", &format!("\n{}INNER JOIN ", self.config.indent));
        formatted = formatted.replace(" OUTER JOIN ", &format!("\n{}OUTER JOIN ", self.config.indent));
        formatted = formatted.replace(" FULL JOIN ", &format!("\n{}FULL JOIN ", self.config.indent));
        formatted = formatted.replace(" CROSS JOIN ", &format!("\n{}CROSS JOIN ", self.config.indent));
        // Generic JOIN should come last to avoid conflicts
        formatted = formatted.replace(" JOIN ", &format!("\n{}JOIN ", self.config.indent));
        
        // Main SQL clauses
        formatted = formatted.replace(" FROM ", "\nFROM ");
        formatted = formatted.replace(" WHERE ", "\nWHERE ");
        formatted = formatted.replace(" GROUP BY ", "\nGROUP BY ");
        formatted = formatted.replace(" HAVING ", "\nHAVING ");
        formatted = formatted.replace(" ORDER BY ", "\nORDER BY ");
        formatted = formatted.replace(" LIMIT ", "\nLIMIT ");
        formatted = formatted.replace(" OFFSET ", "\nOFFSET ");
        
        // Logical operators with proper indentation
        formatted = formatted.replace(" AND ", &format!("\n{}AND ", &self.config.indent));
        formatted = formatted.replace(" OR ", &format!("\n{}OR ", &self.config.indent));
        
        // Subquery formatting
        formatted = formatted.replace(" UNION ", "\nUNION ");
        formatted = formatted.replace(" UNION ALL ", "\nUNION ALL ");
        formatted = formatted.replace(" INTERSECT ", "\nINTERSECT ");
        formatted = formatted.replace(" EXCEPT ", "\nEXCEPT ");
        
        // Clean up extra whitespace but preserve our formatting
        let lines: Vec<String> = formatted
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();
        
        // Merge lines that were incorrectly split (like "LEFT" and "JOIN")
        let mut merged_lines = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            let current_line = &lines[i];
            
            // Check if this line should be merged with the next
            if i + 1 < lines.len() {
                let next_line = &lines[i + 1];
                
                // Merge JOIN keywords that got split
                if (current_line == "LEFT" || current_line == "RIGHT" || 
                    current_line == "INNER" || current_line == "OUTER" || 
                    current_line == "FULL" || current_line == "CROSS") &&
                   next_line.starts_with("JOIN ") {
                    merged_lines.push(format!("{}{} {}", self.config.indent, current_line, next_line));
                    i += 2; // Skip both lines
                    continue;
                }
            }
            
            merged_lines.push(current_line.clone());
            i += 1;
        }
        
        formatted = merged_lines.join("\n");
        
        Ok(formatted)
    }
    
    /// Compact formatting - minimal whitespace
    fn format_compact(&self, sql: &str) -> FormatResult<String> {
        // Remove all unnecessary whitespace
        let compact = sql
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        
        // Ensure proper spacing around operators and keywords
        let mut result = compact;
        
        // Fix spacing around operators
        result = result.replace(" = ", "=");
        result = result.replace(" > ", ">");
        result = result.replace(" < ", "<");
        result = result.replace(" >= ", ">=");
        result = result.replace(" <= ", "<=");
        result = result.replace(" != ", "!=");
        result = result.replace(" <> ", "<>");
        
        // But ensure space after commas
        result = result.replace(",", ", ");
        result = result.replace(",  ", ", "); // Fix double spaces
        
        // Ensure space around keywords
        let keywords = [
            "SELECT", "FROM", "WHERE", "GROUP BY", "HAVING", 
            "ORDER BY", "LIMIT", "OFFSET", "JOIN", "LEFT JOIN", 
            "RIGHT JOIN", "INNER JOIN", "OUTER JOIN", "UNION", 
            "INTERSECT", "EXCEPT", "AND", "OR"
        ];
        
        for keyword in &keywords {
            // Ensure space before and after keywords
            result = result.replace(&format!(" {}", keyword), &format!(" {} ", keyword));
            result = result.replace(&format!("{}  ", keyword), &format!("{} ", keyword));
        }
        
        // Clean up multiple spaces
        while result.contains("  ") {
            result = result.replace("  ", " ");
        }
        
        Ok(result.trim().to_string())
    }
    
    /// Applies final formatting options like newlines
    fn apply_final_formatting(&self, mut formatted: String) -> String {
        if self.config.add_newline && !formatted.ends_with('\n') {
            formatted.push('\n');
        }
        
        formatted
    }
    
    /// Gets the current format configuration
    pub fn config(&self) -> &FormatConfig {
        &self.config
    }
    
    /// Updates the format configuration
    pub fn set_config(&mut self, config: FormatConfig) {
        self.config = config;
    }
    
    /// Updates just the output format
    pub fn set_format(&mut self, format: OutputFormat) {
        self.config.format = format;
    }
}

impl Default for OutputFormatter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_output_format_display() {
        assert_eq!(OutputFormat::Basic.to_string(), "basic");
        assert_eq!(OutputFormat::Pretty.to_string(), "pretty");
        assert_eq!(OutputFormat::Compact.to_string(), "compact");
    }
    
    #[test]
    fn test_format_config_default() {
        let config = FormatConfig::default();
        assert_eq!(config.format, OutputFormat::Basic);
        assert!(config.add_newline);
        assert_eq!(config.indent, "  ");
        assert!(config.preserve_case);
    }
    
    #[test]
    fn test_output_formatter_creation() {
        let formatter = OutputFormatter::new();
        assert_eq!(formatter.config.format, OutputFormat::Basic);
        
        let pretty_formatter = OutputFormatter::with_format(OutputFormat::Pretty);
        assert_eq!(pretty_formatter.config.format, OutputFormat::Pretty);
        
        let custom_config = FormatConfig {
            format: OutputFormat::Compact,
            add_newline: false,
            indent: "    ".to_string(),
            preserve_case: false,
        };
        let custom_formatter = OutputFormatter::with_config(custom_config);
        assert_eq!(custom_formatter.config.format, OutputFormat::Compact);
        assert!(!custom_formatter.config.add_newline);
    }
    
    #[test]
    fn test_basic_formatting() {
        let formatter = OutputFormatter::with_format(OutputFormat::Basic);
        let sql = "SELECT   name,   age   FROM   users   WHERE   age > 18";
        
        let result = formatter.format(sql).unwrap();
        assert_eq!(result, "SELECT name, age FROM users WHERE age > 18\n");
    }
    
    #[test]
    fn test_pretty_formatting() {
        let formatter = OutputFormatter::with_format(OutputFormat::Pretty);
        let sql = "SELECT name, age FROM users WHERE age > 18 AND status = 'active' ORDER BY name";
        
        let result = formatter.format(sql).unwrap();
        assert!(result.contains("\nFROM"));
        assert!(result.contains("\nWHERE"));
        assert!(result.contains("AND"));
        assert!(result.contains("\nORDER BY"));
        assert!(result.ends_with('\n'));
    }
    
    #[test]
    fn test_compact_formatting() {
        let formatter = OutputFormatter::with_format(OutputFormat::Compact);
        let sql = "SELECT   name,   age   FROM   users   WHERE   age > 18";
        
        let result = formatter.format(sql).unwrap();
        // Should be compact but still readable
        assert!(!result.contains("  ")); // No double spaces
        assert!(result.contains(", ")); // Proper comma spacing
        assert!(result.ends_with('\n'));
    }
    
    #[test]
    fn test_pretty_formatting_with_joins() {
        let formatter = OutputFormatter::with_format(OutputFormat::Pretty);
        let sql = "SELECT u.name, o.total FROM users u LEFT JOIN orders o ON u.id = o.user_id WHERE u.age > 18";
        
        let result = formatter.format(sql).unwrap();
        assert!(result.contains("\nFROM"));
        assert!(result.contains("LEFT JOIN"));
        assert!(result.contains("\nWHERE"));
    }
    
    #[test]
    fn test_empty_sql_error() {
        let formatter = OutputFormatter::new();
        let result = formatter.format("");
        assert!(result.is_err());
        
        let result = formatter.format("   ");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_format_error_display() {
        let error = FormatError::InvalidSql("test error".to_string());
        assert_eq!(error.to_string(), "Invalid SQL input: test error");
        
        let error = FormatError::FormattingFailed("format error".to_string());
        assert_eq!(error.to_string(), "Formatting failed: format error");
    }
    
    #[test]
    fn test_config_updates() {
        let mut formatter = OutputFormatter::new();
        assert_eq!(formatter.config().format, OutputFormat::Basic);
        
        formatter.set_format(OutputFormat::Pretty);
        assert_eq!(formatter.config().format, OutputFormat::Pretty);
        
        let new_config = FormatConfig {
            format: OutputFormat::Compact,
            add_newline: false,
            indent: "    ".to_string(),
            preserve_case: false,
        };
        formatter.set_config(new_config);
        assert_eq!(formatter.config().format, OutputFormat::Compact);
        assert!(!formatter.config().add_newline);
    }
    
    #[test]
    fn test_newline_handling() {
        let config = FormatConfig {
            format: OutputFormat::Basic,
            add_newline: false,
            ..Default::default()
        };
        let formatter = OutputFormatter::with_config(config);
        
        let result = formatter.format("SELECT name FROM users").unwrap();
        assert!(!result.ends_with('\n'));
    }
    
    #[test]
    fn test_complex_query_formatting() {
        let formatter = OutputFormatter::with_format(OutputFormat::Pretty);
        let sql = "SELECT u.name, u.email, COUNT(o.id) as order_count FROM users u LEFT JOIN orders o ON u.id = o.user_id WHERE u.status = 'active' AND u.created_at > '2023-01-01' GROUP BY u.id, u.name, u.email HAVING COUNT(o.id) > 0 ORDER BY order_count DESC LIMIT 10";
        
        let result = formatter.format(sql).unwrap();
        
        // Check that all major clauses are on separate lines
        assert!(result.contains("SELECT u.name"));
        assert!(result.contains("\nFROM users u"));
        assert!(result.contains("LEFT JOIN orders o"));
        assert!(result.contains("\nWHERE u.status"));
        assert!(result.contains("AND u.created_at"));
        assert!(result.contains("\nGROUP BY"));
        assert!(result.contains("\nHAVING"));
        assert!(result.contains("\nORDER BY"));
        assert!(result.contains("\nLIMIT"));
    }
}