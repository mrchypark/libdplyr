//! Integration tests for JSON output functionality

use libdplyr::cli::json_output::{
    JsonOutputFormatter, MetadataBuilder, ProcessingStats, InputInfo, ErrorInfo
};

#[test]
fn test_json_output_integration() {
    let formatter = JsonOutputFormatter::new();
    
    // Create sample metadata
    let stats = ProcessingStats::with_timing(100, 200, 300);
    let input_info = InputInfo::from_text("data %>% select(name)");
    let metadata = MetadataBuilder::new("postgresql")
        .with_stats(stats)
        .with_input_info(input_info)
        .build();
    
    // Test successful output
    let result = formatter.format_success("SELECT \"name\" FROM \"data\"", metadata.clone());
    assert!(result.is_ok());
    
    let json = result.unwrap();
    println!("Success JSON output:\n{}", json);
    
    // Verify JSON structure
    assert!(json.contains("\"success\":true"));
    assert!(json.contains("SELECT \\\"name\\\" FROM \\\"data\\\""));
    assert!(json.contains("\"dialect\":\"postgresql\""));
    assert!(json.contains("\"lex_time_us\":100"));
    assert!(json.contains("\"parse_time_us\":200"));
    assert!(json.contains("\"generation_time_us\":300"));
    assert!(json.contains("\"total_time_us\":600"));
}

#[test]
fn test_json_error_output_integration() {
    let formatter = JsonOutputFormatter::pretty();
    
    // Create sample metadata
    let stats = ProcessingStats::empty();
    let input_info = InputInfo::from_stdin("invalid syntax");
    let metadata = MetadataBuilder::new("mysql")
        .with_stats(stats)
        .with_input_info(input_info)
        .build();
    
    // Create error info
    let error_info = ErrorInfo {
        error_type: "parse".to_string(),
        message: "Unexpected token at position 5".to_string(),
        position: Some(5),
        suggestions: vec![
            "Check function syntax".to_string(),
            "Verify pipe operator usage".to_string(),
        ],
    };
    
    // Test error output
    let result = formatter.format_error(error_info, metadata);
    assert!(result.is_ok());
    
    let json = result.unwrap();
    println!("Error JSON output:\n{}", json);
    
    // Verify JSON structure
    assert!(json.contains("\"success\": false"));
    assert!(json.contains("\"error_type\": \"parse\""));
    assert!(json.contains("\"message\": \"Unexpected token at position 5\""));
    assert!(json.contains("\"position\": 5"));
    assert!(json.contains("\"suggestions\""));
    assert!(json.contains("Check function syntax"));
    assert!(json.contains("\"dialect\": \"mysql\""));
    assert!(json.contains("\"source_type\": \"stdin\""));
}

#[test]
fn test_json_deserialization() {
    let formatter = JsonOutputFormatter::new();
    let metadata = MetadataBuilder::new("sqlite").build();
    
    let json = formatter.format_success("SELECT * FROM users", metadata).unwrap();
    
    // Test that we can deserialize the JSON back
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    
    assert_eq!(parsed["success"], true);
    assert_eq!(parsed["sql"], "SELECT * FROM users");
    assert_eq!(parsed["metadata"]["dialect"], "sqlite");
    assert!(parsed["metadata"]["timestamp"].is_number());
}

#[test]
fn test_metadata_completeness() {
    let stats = ProcessingStats {
        lex_time_us: 150,
        parse_time_us: 250,
        generation_time_us: 100,
        total_time_us: 500,
        token_count: 10,
        ast_node_count: 5,
        input_size_bytes: 25,
        output_size_bytes: 45,
    };
    
    let input_info = InputInfo {
        source_type: "file".to_string(),
        source_id: "query.R".to_string(),
        size_bytes: 25,
        line_count: 1,
    };
    
    let metadata = MetadataBuilder::new("duckdb")
        .with_stats(stats)
        .with_input_info(input_info)
        .with_version("1.0.0")
        .build();
    
    let formatter = JsonOutputFormatter::pretty();
    let json = formatter.format_success("SELECT COUNT(*) FROM data", metadata).unwrap();
    
    println!("Complete metadata JSON:\n{}", json);
    
    // Verify all metadata fields are present
    assert!(json.contains("\"dialect\": \"duckdb\""));
    assert!(json.contains("\"version\": \"1.0.0\""));
    assert!(json.contains("\"lex_time_us\": 150"));
    assert!(json.contains("\"token_count\": 10"));
    assert!(json.contains("\"ast_node_count\": 5"));
    assert!(json.contains("\"source_type\": \"file\""));
    assert!(json.contains("\"source_id\": \"query.R\""));
    assert!(json.contains("\"line_count\": 1"));
}