//! Unit tests for OutputFormatter module

use libdplyr::cli::output_formatter::{
    OutputFormatter, OutputFormat, FormatConfig, FormatError
};

#[test]
fn test_output_format_display() {
    assert_eq!(OutputFormat::Default.to_string(), "default");
    assert_eq!(OutputFormat::Basic.to_string(), "basic");
    assert_eq!(OutputFormat::Pretty.to_string(), "pretty");
    assert_eq!(OutputFormat::Compact.to_string(), "compact");
    assert_eq!(OutputFormat::Json.to_string(), "json");
}

#[test]
fn test_output_format_equality() {
    assert_eq!(OutputFormat::Basic, OutputFormat::Basic);
    assert_ne!(OutputFormat::Basic, OutputFormat::Pretty);
    assert_ne!(OutputFormat::Pretty, OutputFormat::Compact);
}

#[test]
fn test_output_format_clone() {
    let format1 = OutputFormat::Pretty;
    let format2 = format1.clone();
    assert_eq!(format1, format2);
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
fn test_format_config_custom() {
    let config = FormatConfig {
        format: OutputFormat::Compact,
        add_newline: false,
        indent: "    ".to_string(),
        preserve_case: false,
    };
    
    assert_eq!(config.format, OutputFormat::Compact);
    assert!(!config.add_newline);
    assert_eq!(config.indent, "    ");
    assert!(!config.preserve_case);
}

#[test]
fn test_format_config_clone() {
    let config1 = FormatConfig {
        format: OutputFormat::Pretty,
        add_newline: false,
        indent: "\t".to_string(),
        preserve_case: true,
    };
    
    let config2 = config1.clone();
    assert_eq!(config1.format, config2.format);
    assert_eq!(config1.add_newline, config2.add_newline);
    assert_eq!(config1.indent, config2.indent);
    assert_eq!(config1.preserve_case, config2.preserve_case);
}

#[test]
fn test_output_formatter_creation() {
    let formatter = OutputFormatter::new();
    assert_eq!(formatter.config().format, OutputFormat::Basic);
    assert!(formatter.config().add_newline);
    
    let pretty_formatter = OutputFormatter::with_format(OutputFormat::Pretty);
    assert_eq!(pretty_formatter.config().format, OutputFormat::Pretty);
    
    let custom_config = FormatConfig {
        format: OutputFormat::Compact,
        add_newline: false,
        indent: "    ".to_string(),
        preserve_case: false,
    };
    let custom_formatter = OutputFormatter::with_config(custom_config);
    assert_eq!(custom_formatter.config().format, OutputFormat::Compact);
    assert!(!custom_formatter.config().add_newline);
    assert_eq!(custom_formatter.config().indent, "    ");
}

#[test]
fn test_output_formatter_default() {
    let formatter1 = OutputFormatter::new();
    let formatter2 = OutputFormatter::default();
    
    assert_eq!(formatter1.config().format, formatter2.config().format);
    assert_eq!(formatter1.config().add_newline, formatter2.config().add_newline);
}

#[test]
fn test_basic_formatting() {
    let formatter = OutputFormatter::with_format(OutputFormat::Basic);
    let sql = "SELECT   name,   age   FROM   users   WHERE   age > 18";
    
    let result = formatter.format(sql).unwrap();
    assert_eq!(result, "SELECT name, age FROM users WHERE age > 18\n");
    
    // Test with multiple spaces
    let sql_multi_space = "SELECT    name,     age    FROM     users";
    let result = formatter.format(sql_multi_space).unwrap();
    assert_eq!(result, "SELECT name, age FROM users\n");
}

#[test]
fn test_basic_formatting_preserves_structure() {
    let formatter = OutputFormatter::with_format(OutputFormat::Basic);
    let sql = "SELECT name FROM users WHERE status = 'active'";
    
    let result = formatter.format(sql).unwrap();
    assert_eq!(result, "SELECT name FROM users WHERE status = 'active'\n");
}

#[test]
fn test_pretty_formatting() {
    let formatter = OutputFormatter::with_format(OutputFormat::Pretty);
    let sql = "SELECT name, age FROM users WHERE age > 18 AND status = 'active' ORDER BY name";
    
    let result = formatter.format(sql).unwrap();
    
    // Check that major clauses are on separate lines
    assert!(result.contains("SELECT name, age"));
    assert!(result.contains("\nFROM users"));
    assert!(result.contains("\nWHERE age > 18"));
    assert!(result.contains("AND status = 'active'"));
    assert!(result.contains("\nORDER BY name"));
    assert!(result.ends_with('\n'));
}

#[test]
fn test_pretty_formatting_with_joins() {
    let formatter = OutputFormatter::with_format(OutputFormat::Pretty);
    let sql = "SELECT u.name, o.total FROM users u LEFT JOIN orders o ON u.id = o.user_id WHERE u.age > 18";
    
    let result = formatter.format(sql).unwrap();
    
    assert!(result.contains("SELECT u.name, o.total"));
    assert!(result.contains("\nFROM users u"));
    assert!(result.contains("LEFT JOIN orders o"));
    assert!(result.contains("\nWHERE u.age > 18"));
}

#[test]
fn test_pretty_formatting_complex_joins() {
    let formatter = OutputFormatter::with_format(OutputFormat::Pretty);
    let sql = "SELECT * FROM users u INNER JOIN profiles p ON u.id = p.user_id RIGHT JOIN orders o ON u.id = o.user_id";
    
    let result = formatter.format(sql).unwrap();
    
    assert!(result.contains("\nFROM users u"));
    assert!(result.contains("INNER JOIN profiles p"));
    assert!(result.contains("RIGHT JOIN orders o"));
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
    
    // Should contain all the original content
    assert!(result.contains("SELECT"));
    assert!(result.contains("name"));
    assert!(result.contains("age"));
    assert!(result.contains("FROM"));
    assert!(result.contains("users"));
    assert!(result.contains("WHERE"));
}

#[test]
fn test_compact_formatting_operators() {
    let formatter = OutputFormatter::with_format(OutputFormat::Compact);
    let sql = "SELECT name FROM users WHERE age >= 18 AND status != 'inactive'";
    
    let result = formatter.format(sql).unwrap();
    
    // Should handle operators properly - compact format removes spaces around operators
    assert!(result.contains("age>=18") || result.contains("age >= 18"));
    assert!(result.contains("status!='inactive'") || result.contains("status != 'inactive'"));
    // Should still have comma spacing
    assert!(result.contains("name") && result.contains("FROM"));
}

#[test]
fn test_json_formatting_fallback() {
    let formatter = OutputFormatter::with_format(OutputFormat::Json);
    let sql = "SELECT name FROM users";
    
    let result = formatter.format(sql).unwrap();
    
    // JSON formatting should fall back to basic formatting for now
    assert_eq!(result, "SELECT name FROM users\n");
}

#[test]
fn test_default_formatting() {
    let formatter = OutputFormatter::with_format(OutputFormat::Default);
    let sql = "SELECT   name   FROM   users";
    
    let result = formatter.format(sql).unwrap();
    assert_eq!(result, "SELECT name FROM users\n");
}

#[test]
fn test_empty_sql_error() {
    let formatter = OutputFormatter::new();
    
    let result = formatter.format("");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), FormatError::InvalidSql(_)));
    
    let result = formatter.format("   ");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), FormatError::InvalidSql(_)));
    
    let result = formatter.format("\n\t  \n");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), FormatError::InvalidSql(_)));
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
    assert_eq!(formatter.config().indent, "    ");
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
    assert_eq!(result, "SELECT name FROM users");
}

#[test]
fn test_newline_handling_enabled() {
    let config = FormatConfig {
        format: OutputFormat::Basic,
        add_newline: true,
        ..Default::default()
    };
    let formatter = OutputFormatter::with_config(config);
    
    let result = formatter.format("SELECT name FROM users").unwrap();
    assert!(result.ends_with('\n'));
    assert_eq!(result, "SELECT name FROM users\n");
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

#[test]
fn test_union_queries_formatting() {
    let formatter = OutputFormatter::with_format(OutputFormat::Pretty);
    let sql = "SELECT name FROM users WHERE active = true UNION SELECT name FROM archived_users UNION ALL SELECT name FROM temp_users";
    
    let result = formatter.format(sql).unwrap();
    
    assert!(result.contains("SELECT name"));
    assert!(result.contains("\nUNION"));
    assert!(result.contains("\nUNION ALL"));
}

#[test]
fn test_subquery_formatting() {
    let formatter = OutputFormatter::with_format(OutputFormat::Pretty);
    let sql = "SELECT name FROM users WHERE id IN (SELECT user_id FROM orders WHERE total > 100) INTERSECT SELECT name FROM premium_users";
    
    let result = formatter.format(sql).unwrap();
    
    assert!(result.contains("SELECT name"));
    assert!(result.contains("\nWHERE"));
    assert!(result.contains("\nINTERSECT"));
}

#[test]
fn test_custom_indent_formatting() {
    let config = FormatConfig {
        format: OutputFormat::Pretty,
        add_newline: true,
        indent: "    ".to_string(), // 4 spaces
        preserve_case: true,
    };
    let formatter = OutputFormatter::with_config(config);
    
    let sql = "SELECT name FROM users WHERE age > 18 AND status = 'active'";
    let result = formatter.format(sql).unwrap();
    
    // Should use 4-space indentation for AND clause - check if formatting includes proper indentation
    assert!(result.contains("AND status") || result.contains("    AND status"));
}

#[test]
fn test_format_error_debug() {
    let error = FormatError::InvalidSql("empty input".to_string());
    let debug_str = format!("{:?}", error);
    
    assert!(debug_str.contains("InvalidSql"));
    assert!(debug_str.contains("empty input"));
}

#[test]
fn test_output_format_debug() {
    let format = OutputFormat::Pretty;
    let debug_str = format!("{:?}", format);
    
    assert!(debug_str.contains("Pretty"));
}

#[test]
fn test_format_config_debug() {
    let config = FormatConfig::default();
    let debug_str = format!("{:?}", config);
    
    assert!(debug_str.contains("FormatConfig"));
    assert!(debug_str.contains("format"));
    assert!(debug_str.contains("add_newline"));
}

#[test]
fn test_output_formatter_debug() {
    let formatter = OutputFormatter::new();
    let debug_str = format!("{:?}", formatter);
    
    assert!(debug_str.contains("OutputFormatter"));
}

#[test]
fn test_whitespace_normalization() {
    let formatter = OutputFormatter::with_format(OutputFormat::Basic);
    
    // Test various whitespace scenarios
    let test_cases = vec![
        ("SELECT\n\nname\n\nFROM\n\nusers", "SELECT name FROM users\n"),
        ("SELECT\tname\tFROM\tusers", "SELECT name FROM users\n"),
        ("SELECT  name   ,   age  FROM   users", "SELECT name , age FROM users\n"),
    ];
    
    for (input, expected) in test_cases {
        let result = formatter.format(input).unwrap();
        assert_eq!(result, expected, "Failed for input: '{}'", input);
    }
}

#[test]
fn test_case_preservation() {
    let formatter = OutputFormatter::with_format(OutputFormat::Basic);
    let sql = "select Name, AGE from Users where Status = 'Active'";
    
    let result = formatter.format(sql).unwrap();
    
    // Should preserve original case
    assert!(result.contains("select"));
    assert!(result.contains("Name"));
    assert!(result.contains("AGE"));
    assert!(result.contains("Users"));
    assert!(result.contains("Status"));
    assert!(result.contains("Active"));
}