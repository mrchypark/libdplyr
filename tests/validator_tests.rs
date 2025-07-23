//! Unit tests for DplyrValidator module

use libdplyr::cli::validator::{
    DplyrValidator, ValidateResult, ValidationConfig, ValidationError, ValidationErrorInfo,
    ValidationSummary,
};

#[test]
fn test_validator_creation() {
    let validator = DplyrValidator::new();
    assert!(validator.config().semantic_validation);
    assert!(validator.config().check_common_mistakes);
    assert!(validator.config().detailed_suggestions);
    assert!(validator.config().max_complexity.is_none());

    let config = ValidationConfig {
        semantic_validation: false,
        check_common_mistakes: false,
        detailed_suggestions: false,
        max_complexity: Some(5),
    };
    let custom_validator = DplyrValidator::with_config(config);
    assert!(!custom_validator.config().semantic_validation);
    assert!(!custom_validator.config().check_common_mistakes);
    assert!(!custom_validator.config().detailed_suggestions);
    assert_eq!(custom_validator.config().max_complexity, Some(5));
}

#[test]
fn test_validator_default() {
    let validator1 = DplyrValidator::new();
    let validator2 = DplyrValidator::default();

    assert_eq!(
        validator1.config().semantic_validation,
        validator2.config().semantic_validation
    );
    assert_eq!(
        validator1.config().check_common_mistakes,
        validator2.config().check_common_mistakes
    );
}

#[test]
fn test_validation_config_default() {
    let config = ValidationConfig::default();
    assert!(config.semantic_validation);
    assert!(config.check_common_mistakes);
    assert!(config.detailed_suggestions);
    assert!(config.max_complexity.is_none());
}

#[test]
fn test_validation_config_clone() {
    let config1 = ValidationConfig {
        semantic_validation: false,
        check_common_mistakes: true,
        detailed_suggestions: false,
        max_complexity: Some(8),
    };

    let config2 = config1.clone();
    assert_eq!(config1.semantic_validation, config2.semantic_validation);
    assert_eq!(config1.check_common_mistakes, config2.check_common_mistakes);
    assert_eq!(config1.detailed_suggestions, config2.detailed_suggestions);
    assert_eq!(config1.max_complexity, config2.max_complexity);
}

#[test]
fn test_validation_summary_equality() {
    let summary1 = ValidationSummary {
        operation_count: 2,
        operations: vec!["select".to_string(), "filter".to_string()],
        column_count: 3,
        columns: vec!["name".to_string(), "age".to_string()],
        has_aggregation: false,
        has_grouping: false,
        complexity_score: 3,
    };

    let summary2 = ValidationSummary {
        operation_count: 2,
        operations: vec!["select".to_string(), "filter".to_string()],
        column_count: 3,
        columns: vec!["name".to_string(), "age".to_string()],
        has_aggregation: false,
        has_grouping: false,
        complexity_score: 3,
    };

    assert_eq!(summary1, summary2);
}

#[test]
fn test_validation_error_info_equality() {
    let error1 = ValidationErrorInfo {
        error_type: "parse".to_string(),
        message: "Unexpected token".to_string(),
        position: Some(10),
        context: Some("function call".to_string()),
    };

    let error2 = ValidationErrorInfo {
        error_type: "parse".to_string(),
        message: "Unexpected token".to_string(),
        position: Some(10),
        context: Some("function call".to_string()),
    };

    assert_eq!(error1, error2);
}

#[test]
fn test_validate_result_equality() {
    let summary = ValidationSummary {
        operation_count: 1,
        operations: vec!["select".to_string()],
        column_count: 1,
        columns: vec!["name".to_string()],
        has_aggregation: false,
        has_grouping: false,
        complexity_score: 1,
    };

    let result1 = ValidateResult::Valid {
        summary: summary.clone(),
    };
    let result2 = ValidateResult::Valid { summary };

    assert_eq!(result1, result2);
}

#[test]
fn test_valid_simple_query() {
    let validator = DplyrValidator::new();
    let result = validator.validate("data %>% select(name, age)").unwrap();

    match result {
        ValidateResult::Valid { summary } => {
            assert_eq!(summary.operation_count, 1);
            assert_eq!(summary.operations, vec!["select"]);
            assert_eq!(summary.column_count, 2);
            assert!(summary.columns.contains(&"name".to_string()));
            assert!(summary.columns.contains(&"age".to_string()));
            assert!(!summary.has_aggregation);
            assert!(!summary.has_grouping);
            assert!(summary.complexity_score > 0);
        }
        ValidateResult::Invalid { .. } => panic!("Expected valid result"),
    }
}

#[test]
fn test_valid_complex_query() {
    let validator = DplyrValidator::new();
    let result = validator.validate(
        "data %>% select(name, age, salary) %>% filter(age > 18) %>% group_by(department) %>% summarise(avg_salary = mean(salary))"
    ).unwrap();

    match result {
        ValidateResult::Valid { summary } => {
            assert_eq!(summary.operation_count, 4);
            assert!(summary.operations.contains(&"select".to_string()));
            assert!(summary.operations.contains(&"filter".to_string()));
            assert!(summary.operations.contains(&"group_by".to_string()));
            assert!(summary.operations.contains(&"summarise".to_string()));
            assert!(summary.has_aggregation);
            assert!(summary.has_grouping);
            assert!(summary.complexity_score > 5);
        }
        ValidateResult::Invalid { .. } => panic!("Expected valid result"),
    }
}

#[test]
fn test_valid_mutate_query() {
    let validator = DplyrValidator::new();
    let result = validator
        .validate("data %>% mutate(adult = age >= 18, bonus = salary * 0.1)")
        .unwrap();

    match result {
        ValidateResult::Valid { summary } => {
            assert_eq!(summary.operation_count, 1);
            assert_eq!(summary.operations, vec!["mutate"]);
            assert!(summary.columns.contains(&"adult".to_string()));
            assert!(summary.columns.contains(&"bonus".to_string()));
            assert!(!summary.has_aggregation);
            assert!(!summary.has_grouping);
        }
        ValidateResult::Invalid { .. } => panic!("Expected valid result"),
    }
}

#[test]
fn test_valid_arrange_query() {
    let validator = DplyrValidator::new();
    let result = validator
        .validate("data %>% arrange(name, desc(age))")
        .unwrap();

    match result {
        ValidateResult::Valid { summary } => {
            assert_eq!(summary.operation_count, 1);
            assert_eq!(summary.operations, vec!["arrange"]);
            assert!(summary.columns.contains(&"name".to_string()));
            assert!(summary.columns.contains(&"age".to_string()));
            assert!(!summary.has_aggregation);
            assert!(!summary.has_grouping);
        }
        ValidateResult::Invalid { .. } => panic!("Expected valid result"),
    }
}

#[test]
fn test_invalid_syntax() {
    let validator = DplyrValidator::new();
    let result = validator.validate("invalid_function(test)").unwrap();

    match result {
        ValidateResult::Invalid { error, suggestions } => {
            assert_eq!(error.error_type, "parse");
            assert!(!error.message.is_empty());
            assert!(!suggestions.is_empty());
            assert!(suggestions.iter().any(|s| s.contains("function syntax")));
        }
        ValidateResult::Valid { .. } => panic!("Expected invalid result"),
    }
}

#[test]
fn test_empty_input() {
    let validator = DplyrValidator::new();
    let result = validator.validate("").unwrap();

    match result {
        ValidateResult::Invalid { error, suggestions } => {
            assert_eq!(error.error_type, "input");
            assert_eq!(error.message, "Empty input provided");
            assert_eq!(error.position, Some(0));
            assert!(!suggestions.is_empty());
            assert!(suggestions.iter().any(|s| s.contains("Example:")));
        }
        ValidateResult::Valid { .. } => panic!("Expected invalid result"),
    }
}

#[test]
fn test_whitespace_only_input() {
    let validator = DplyrValidator::new();
    let result = validator.validate("   \n\t  ").unwrap();

    match result {
        ValidateResult::Invalid { error, .. } => {
            assert_eq!(error.error_type, "input");
            assert_eq!(error.message, "Empty input provided");
        }
        ValidateResult::Valid { .. } => panic!("Expected invalid result for whitespace-only input"),
    }
}

#[test]
fn test_complexity_limit() {
    let config = ValidationConfig {
        max_complexity: Some(2),
        ..Default::default()
    };
    let validator = DplyrValidator::with_config(config);

    // This should exceed complexity limit
    let result = validator
        .validate("data %>% select(a, b, c) %>% filter(a > 1) %>% mutate(d = a + b) %>% arrange(d)")
        .unwrap();

    match result {
        ValidateResult::Invalid { error, suggestions } => {
            assert_eq!(error.error_type, "complexity");
            assert!(error.message.contains("exceeds maximum"));
            assert!(!suggestions.is_empty());
            assert!(suggestions.iter().any(|s| s.contains("Simplify")));
        }
        ValidateResult::Valid { .. } => panic!("Expected complexity error"),
    }
}

#[test]
fn test_complexity_limit_pass() {
    let config = ValidationConfig {
        max_complexity: Some(5),
        ..Default::default()
    };
    let validator = DplyrValidator::with_config(config);

    // Simple query should pass
    let result = validator.validate("data %>% select(name)").unwrap();
    match result {
        ValidateResult::Valid { summary } => {
            assert!(summary.complexity_score <= 5);
        }
        ValidateResult::Invalid { .. } => panic!("Simple query should pass complexity check"),
    }
}

#[test]
fn test_semantic_validation_disabled() {
    let config = ValidationConfig {
        semantic_validation: false,
        ..Default::default()
    };
    let validator = DplyrValidator::with_config(config);

    // This would normally trigger a semantic warning but shouldn't with disabled validation
    let result = validator
        .validate("data %>% select(name) %>% summarise(count = n())")
        .unwrap();

    match result {
        ValidateResult::Valid { .. } => {} // Should be valid with semantic validation disabled
        ValidateResult::Invalid { .. } => {
            panic!("Expected valid result with semantic validation disabled")
        }
    }
}

#[test]
fn test_semantic_validation_enabled() {
    let config = ValidationConfig {
        semantic_validation: true,
        ..Default::default()
    };
    let validator = DplyrValidator::with_config(config);

    // Complex query with aggregation without grouping might trigger semantic warning
    let result = validator.validate(
        "data %>% select(name, age, salary) %>% filter(age > 18) %>% summarise(total = sum(salary))"
    ).unwrap();

    // This might be valid or invalid depending on semantic rules
    match result {
        ValidateResult::Valid { summary } => {
            assert!(summary.has_aggregation);
            assert!(!summary.has_grouping);
        }
        ValidateResult::Invalid { error, .. } => {
            assert_eq!(error.error_type, "semantic");
        }
    }
}

#[test]
fn test_validation_summary_debug() {
    let summary = ValidationSummary {
        operation_count: 2,
        operations: vec!["select".to_string(), "filter".to_string()],
        column_count: 1,
        columns: vec!["name".to_string()],
        has_aggregation: false,
        has_grouping: false,
        complexity_score: 3,
    };

    let debug_str = format!("{:?}", summary);
    assert!(debug_str.contains("ValidationSummary"));
    assert!(debug_str.contains("operation_count"));
    assert!(debug_str.contains("complexity_score"));
}

#[test]
fn test_validation_error_info_debug() {
    let error_info = ValidationErrorInfo {
        error_type: "parse".to_string(),
        message: "Unexpected token".to_string(),
        position: Some(10),
        context: Some("around position 10".to_string()),
    };

    let debug_str = format!("{:?}", error_info);
    assert!(debug_str.contains("ValidationErrorInfo"));
    assert!(debug_str.contains("parse"));
    assert!(debug_str.contains("Unexpected token"));
}

#[test]
fn test_validate_result_debug() {
    let summary = ValidationSummary {
        operation_count: 1,
        operations: vec!["select".to_string()],
        column_count: 1,
        columns: vec!["name".to_string()],
        has_aggregation: false,
        has_grouping: false,
        complexity_score: 1,
    };

    let result = ValidateResult::Valid { summary };
    let debug_str = format!("{:?}", result);

    assert!(debug_str.contains("Valid"));
    assert!(debug_str.contains("ValidationSummary"));
}

#[test]
fn test_validation_error_display() {
    let error = ValidationError::ValidationFailed("test error".to_string());
    assert_eq!(error.to_string(), "Validation failed: test error");

    let error = ValidationError::InternalError("internal error".to_string());
    assert_eq!(
        error.to_string(),
        "Internal validation error: internal error"
    );
}

#[test]
fn test_config_updates() {
    let mut validator = DplyrValidator::new();
    assert!(validator.config().semantic_validation);

    let new_config = ValidationConfig {
        semantic_validation: false,
        check_common_mistakes: false,
        detailed_suggestions: true,
        max_complexity: Some(3),
    };

    validator.set_config(new_config);
    assert!(!validator.config().semantic_validation);
    assert!(!validator.config().check_common_mistakes);
    assert!(validator.config().detailed_suggestions);
    assert_eq!(validator.config().max_complexity, Some(3));
}

#[test]
fn test_validation_error_info_fields() {
    let error_info = ValidationErrorInfo {
        error_type: "lex".to_string(),
        message: "Invalid character".to_string(),
        position: Some(15),
        context: Some("at function start".to_string()),
    };

    assert_eq!(error_info.error_type, "lex");
    assert_eq!(error_info.message, "Invalid character");
    assert_eq!(error_info.position, Some(15));
    assert_eq!(error_info.context, Some("at function start".to_string()));
}

#[test]
fn test_validation_summary_fields() {
    let summary = ValidationSummary {
        operation_count: 3,
        operations: vec![
            "select".to_string(),
            "filter".to_string(),
            "arrange".to_string(),
        ],
        column_count: 2,
        columns: vec!["name".to_string(), "age".to_string()],
        has_aggregation: true,
        has_grouping: true,
        complexity_score: 7,
    };

    assert_eq!(summary.operation_count, 3);
    assert_eq!(summary.operations.len(), 3);
    assert_eq!(summary.column_count, 2);
    assert_eq!(summary.columns.len(), 2);
    assert!(summary.has_aggregation);
    assert!(summary.has_grouping);
    assert_eq!(summary.complexity_score, 7);
}

#[test]
fn test_validator_debug() {
    let validator = DplyrValidator::new();
    let debug_str = format!("{:?}", validator);

    assert!(debug_str.contains("DplyrValidator"));
}

#[test]
fn test_validation_config_debug() {
    let config = ValidationConfig::default();
    let debug_str = format!("{:?}", config);

    assert!(debug_str.contains("ValidationConfig"));
    assert!(debug_str.contains("semantic_validation"));
    assert!(debug_str.contains("max_complexity"));
}
