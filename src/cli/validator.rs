//! Dplyr syntax validation module
//!
//! Provides validation-only functionality for dplyr syntax without SQL generation.

use crate::{Lexer, Parser, TranspileError};
use std::collections::HashSet;

/// Result type for validation operations
pub type ValidationResult<T> = Result<T, ValidationError>;

/// Errors that can occur during validation
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Validation failed: {0}")]
    ValidationFailed(String),
    
    #[error("Internal validation error: {0}")]
    InternalError(String),
}

/// Validation result for dplyr syntax
#[derive(Debug, Clone, PartialEq)]
pub enum ValidateResult {
    /// Syntax is valid
    Valid {
        /// Summary of validated operations
        summary: ValidationSummary,
    },
    /// Syntax is invalid
    Invalid {
        /// Error information
        error: ValidationErrorInfo,
        /// Suggestions for fixing the error
        suggestions: Vec<String>,
    },
}

/// Summary of validation results
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationSummary {
    /// Number of operations in the pipeline
    pub operation_count: usize,
    
    /// Types of operations found
    pub operations: Vec<String>,
    
    /// Number of columns referenced
    pub column_count: usize,
    
    /// Column names referenced (if extractable)
    pub columns: Vec<String>,
    
    /// Whether the query uses aggregation
    pub has_aggregation: bool,
    
    /// Whether the query uses grouping
    pub has_grouping: bool,
    
    /// Complexity score (0-10)
    pub complexity_score: u8,
}

/// Detailed error information for validation failures
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationErrorInfo {
    /// Error type (lex, parse, semantic)
    pub error_type: String,
    
    /// Error message
    pub message: String,
    
    /// Position in input (if available)
    pub position: Option<usize>,
    
    /// Context around the error
    pub context: Option<String>,
}

/// Configuration for validation behavior
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Whether to perform semantic validation (beyond syntax)
    pub semantic_validation: bool,
    
    /// Whether to check for common mistakes
    pub check_common_mistakes: bool,
    
    /// Whether to provide detailed suggestions
    pub detailed_suggestions: bool,
    
    /// Maximum complexity score to allow
    pub max_complexity: Option<u8>,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            semantic_validation: true,
            check_common_mistakes: true,
            detailed_suggestions: true,
            max_complexity: None,
        }
    }
}

/// Dplyr syntax validator
#[derive(Debug)]
pub struct DplyrValidator {
    config: ValidationConfig,
}

impl DplyrValidator {
    /// Creates a new validator with default configuration
    pub fn new() -> Self {
        Self {
            config: ValidationConfig::default(),
        }
    }
    
    /// Creates a new validator with custom configuration
    pub fn with_config(config: ValidationConfig) -> Self {
        Self { config }
    }
    
    /// Validates dplyr syntax
    pub fn validate(&self, dplyr_code: &str) -> ValidationResult<ValidateResult> {
        // Basic input validation
        if dplyr_code.trim().is_empty() {
            return Ok(ValidateResult::Invalid {
                error: ValidationErrorInfo {
                    error_type: "input".to_string(),
                    message: "Empty input provided".to_string(),
                    position: Some(0),
                    context: None,
                },
                suggestions: vec![
                    "Provide valid dplyr code".to_string(),
                    "Example: data %>% select(name, age)".to_string(),
                ],
            });
        }
        
        // Perform lexical and syntactic validation
        match self.parse_syntax(dplyr_code) {
            Ok(ast) => {
                // Generate validation summary
                let summary = self.analyze_ast(&ast, dplyr_code)?;
                
                // Check complexity if configured
                if let Some(max_complexity) = self.config.max_complexity {
                    if summary.complexity_score > max_complexity {
                        return Ok(ValidateResult::Invalid {
                            error: ValidationErrorInfo {
                                error_type: "complexity".to_string(),
                                message: format!(
                                    "Query complexity ({}) exceeds maximum allowed ({})",
                                    summary.complexity_score, max_complexity
                                ),
                                position: None,
                                context: None,
                            },
                            suggestions: vec![
                                "Simplify the query by breaking it into smaller parts".to_string(),
                                "Reduce the number of operations in the pipeline".to_string(),
                            ],
                        });
                    }
                }
                
                // Perform semantic validation if enabled
                if self.config.semantic_validation {
                    if let Some(semantic_error) = self.check_semantic_issues(&summary, dplyr_code) {
                        let suggestions = self.generate_semantic_suggestions(&semantic_error);
                        return Ok(ValidateResult::Invalid {
                            error: semantic_error,
                            suggestions,
                        });
                    }
                }
                
                Ok(ValidateResult::Valid { summary })
            }
            Err(error) => {
                let error_info = self.convert_transpile_error(&error, dplyr_code);
                let suggestions = self.generate_error_suggestions(&error, dplyr_code);
                
                Ok(ValidateResult::Invalid {
                    error: error_info,
                    suggestions,
                })
            }
        }
    }
    
    /// Parses the syntax without generating SQL
    fn parse_syntax(&self, dplyr_code: &str) -> Result<crate::DplyrNode, TranspileError> {
        let lexer = Lexer::new(dplyr_code.to_string());
        let mut parser = Parser::new(lexer)?;
        Ok(parser.parse()?)
    }
    
    /// Analyzes the AST to generate validation summary
    fn analyze_ast(&self, ast: &crate::DplyrNode, _code: &str) -> ValidationResult<ValidationSummary> {
        let mut operations = Vec::new();
        let mut columns = HashSet::new();
        let mut has_aggregation = false;
        let mut has_grouping = false;
        let mut complexity_score = 0u8;
        
        // Analyze the AST structure
        self.analyze_node(ast, &mut operations, &mut columns, &mut has_aggregation, &mut has_grouping, &mut complexity_score);
        
        Ok(ValidationSummary {
            operation_count: operations.len(),
            operations,
            column_count: columns.len(),
            columns: columns.into_iter().collect(),
            has_aggregation,
            has_grouping,
            complexity_score: complexity_score.min(10), // Cap at 10
        })
    }
    
    /// Recursively analyzes AST nodes
    fn analyze_node(
        &self,
        node: &crate::DplyrNode,
        operations: &mut Vec<String>,
        columns: &mut HashSet<String>,
        has_aggregation: &mut bool,
        has_grouping: &mut bool,
        complexity_score: &mut u8,
    ) {
        use crate::DplyrNode;
        
        match node {
            DplyrNode::Pipeline { operations: ops, .. } => {
                for op in ops {
                    self.analyze_operation(op, operations, columns, has_aggregation, has_grouping, complexity_score);
                }
            }
            DplyrNode::DataSource { .. } => {
                // Data source nodes don't contain operations to analyze
            }
        }
    }
    
    /// Analyzes individual operations
    fn analyze_operation(
        &self,
        operation: &crate::DplyrOperation,
        operations: &mut Vec<String>,
        columns: &mut HashSet<String>,
        has_aggregation: &mut bool,
        has_grouping: &mut bool,
        complexity_score: &mut u8,
    ) {
        use crate::DplyrOperation;
        
        match operation {
            DplyrOperation::Select { columns: cols, .. } => {
                operations.push("select".to_string());
                for col in cols {
                    // Extract column name from ColumnExpr
                    if let crate::parser::Expr::Identifier(name) = &col.expr {
                        columns.insert(name.clone());
                    }
                    // If there's an alias, add it too
                    if let Some(alias) = &col.alias {
                        columns.insert(alias.clone());
                    }
                }
                *complexity_score += 1;
            }
            DplyrOperation::Filter { .. } => {
                operations.push("filter".to_string());
                *complexity_score += 2;
            }
            DplyrOperation::Mutate { assignments, .. } => {
                operations.push("mutate".to_string());
                for assignment in assignments {
                    columns.insert(assignment.column.clone());
                }
                *complexity_score += 2;
            }
            DplyrOperation::Arrange { columns: cols, .. } => {
                operations.push("arrange".to_string());
                for col in cols {
                    columns.insert(col.column.clone());
                }
                *complexity_score += 1;
            }
            DplyrOperation::GroupBy { columns: cols, .. } => {
                operations.push("group_by".to_string());
                *has_grouping = true;
                for col in cols {
                    columns.insert(col.clone());
                }
                *complexity_score += 2;
            }
            DplyrOperation::Summarise { aggregations, .. } => {
                operations.push("summarise".to_string());
                *has_aggregation = true;
                for agg in aggregations {
                    // Add the column being aggregated
                    columns.insert(agg.column.clone());
                    // Add the alias if it exists
                    if let Some(alias) = &agg.alias {
                        columns.insert(alias.clone());
                    }
                }
                *complexity_score += 3;
            }
        }
    }
    
    /// Checks for semantic issues
    fn check_semantic_issues(&self, summary: &ValidationSummary, _code: &str) -> Option<ValidationErrorInfo> {
        // Check for aggregation without grouping in complex cases
        if summary.has_aggregation && !summary.has_grouping && summary.operation_count > 2 {
            return Some(ValidationErrorInfo {
                error_type: "semantic".to_string(),
                message: "Using aggregation functions without GROUP BY in complex query may produce unexpected results".to_string(),
                position: None,
                context: Some("Consider adding group_by() before summarise()".to_string()),
            });
        }
        
        // Check for very high complexity
        if summary.complexity_score > 8 {
            return Some(ValidationErrorInfo {
                error_type: "complexity".to_string(),
                message: format!("Query complexity is very high ({})", summary.complexity_score),
                position: None,
                context: Some("Consider breaking the query into smaller parts".to_string()),
            });
        }
        
        None
    }
    
    /// Converts TranspileError to ValidationErrorInfo
    fn convert_transpile_error(&self, error: &TranspileError, code: &str) -> ValidationErrorInfo {
        match error {
            TranspileError::LexError(e) => ValidationErrorInfo {
                error_type: "lex".to_string(),
                message: e.to_string(),
                position: None, // TODO: Extract position from error if available
                context: self.extract_error_context(code, None),
            },
            TranspileError::ParseError(e) => ValidationErrorInfo {
                error_type: "parse".to_string(),
                message: e.to_string(),
                position: None, // TODO: Extract position from error if available
                context: self.extract_error_context(code, None),
            },
            TranspileError::GenerationError(e) => ValidationErrorInfo {
                error_type: "generation".to_string(),
                message: e.to_string(),
                position: None,
                context: None,
            },
            TranspileError::IoError(e) => ValidationErrorInfo {
                error_type: "io".to_string(),
                message: e.to_string(),
                position: None,
                context: None,
            },
            TranspileError::ValidationError(e) => ValidationErrorInfo {
                error_type: "validation".to_string(),
                message: e.to_string(),
                position: None,
                context: None,
            },
            TranspileError::ConfigurationError(e) => ValidationErrorInfo {
                error_type: "configuration".to_string(),
                message: e.to_string(),
                position: None,
                context: None,
            },
            TranspileError::SystemError(e) => ValidationErrorInfo {
                error_type: "system".to_string(),
                message: e.to_string(),
                position: None,
                context: None,
            },
        }
    }
    
    /// Extracts context around an error position
    fn extract_error_context(&self, code: &str, position: Option<usize>) -> Option<String> {
        if let Some(pos) = position {
            let start = pos.saturating_sub(20);
            let end = (pos + 20).min(code.len());
            Some(code[start..end].to_string())
        } else {
            None
        }
    }
    
    /// Generates suggestions for fixing errors
    fn generate_error_suggestions(&self, error: &TranspileError, _code: &str) -> Vec<String> {
        match error {
            TranspileError::LexError(_) => vec![
                "Check for invalid characters or malformed strings".to_string(),
                "Ensure proper quoting of string literals".to_string(),
                "Remove any non-ASCII characters".to_string(),
            ],
            TranspileError::ParseError(_) => vec![
                "Check dplyr function syntax and arguments".to_string(),
                "Ensure proper use of pipe operator (%>%)".to_string(),
                "Verify function names are spelled correctly".to_string(),
                "Check parentheses and comma placement".to_string(),
            ],
            TranspileError::GenerationError(_) => vec![
                "This error shouldn't occur during validation-only mode".to_string(),
            ],
            TranspileError::IoError(_) => vec![
                "Check file permissions and paths".to_string(),
                "Ensure input/output resources are available".to_string(),
            ],
            TranspileError::ValidationError(_) => vec![
                "Check dplyr syntax and function usage".to_string(),
                "Verify all required arguments are provided".to_string(),
            ],
            TranspileError::ConfigurationError(_) => vec![
                "Check configuration settings".to_string(),
                "Verify all required options are provided".to_string(),
            ],
            TranspileError::SystemError(_) => vec![
                "Check system permissions and resources".to_string(),
                "Verify signal handling or pipeline configuration".to_string(),
            ],
        }
    }
    
    /// Generates suggestions for semantic errors
    fn generate_semantic_suggestions(&self, error: &ValidationErrorInfo) -> Vec<String> {
        match error.error_type.as_str() {
            "semantic" => vec![
                "Consider adding group_by() before summarise()".to_string(),
                "Review the logical flow of your operations".to_string(),
            ],
            "complexity" => vec![
                "Break the query into smaller, simpler parts".to_string(),
                "Use intermediate variables to store partial results".to_string(),
                "Consider if all operations are necessary".to_string(),
            ],
            _ => vec!["Review the query structure".to_string()],
        }
    }
    
    /// Gets the current validation configuration
    pub fn config(&self) -> &ValidationConfig {
        &self.config
    }
    
    /// Updates the validation configuration
    pub fn set_config(&mut self, config: ValidationConfig) {
        self.config = config;
    }
}

impl Default for DplyrValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validator_creation() {
        let validator = DplyrValidator::new();
        assert!(validator.config.semantic_validation);
        assert!(validator.config.check_common_mistakes);
        assert!(validator.config.detailed_suggestions);
        assert!(validator.config.max_complexity.is_none());
        
        let config = ValidationConfig {
            semantic_validation: false,
            check_common_mistakes: false,
            detailed_suggestions: false,
            max_complexity: Some(5),
        };
        let custom_validator = DplyrValidator::with_config(config);
        assert!(!custom_validator.config.semantic_validation);
        assert_eq!(custom_validator.config.max_complexity, Some(5));
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
    fn test_invalid_syntax() {
        let validator = DplyrValidator::new();
        let result = validator.validate("invalid_function(test)").unwrap();
        
        match result {
            ValidateResult::Invalid { error, suggestions } => {
                assert_eq!(error.error_type, "parse");
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
                assert!(!suggestions.is_empty());
            }
            ValidateResult::Valid { .. } => panic!("Expected invalid result"),
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
        let result = validator.validate(
            "data %>% select(a, b, c) %>% filter(a > 1) %>% mutate(d = a + b) %>% arrange(d)"
        ).unwrap();
        
        match result {
            ValidateResult::Invalid { error, .. } => {
                assert_eq!(error.error_type, "complexity");
                assert!(error.message.contains("exceeds maximum"));
            }
            ValidateResult::Valid { .. } => panic!("Expected complexity error"),
        }
    }
    
    #[test]
    fn test_validation_summary() {
        let validator = DplyrValidator::new();
        let result = validator.validate("data %>% select(name) %>% filter(age > 18)").unwrap();
        
        match result {
            ValidateResult::Valid { summary } => {
                assert_eq!(summary.operation_count, 2);
                assert_eq!(summary.operations, vec!["select", "filter"]);
                assert!(summary.complexity_score >= 3); // select(1) + filter(2)
            }
            ValidateResult::Invalid { .. } => panic!("Expected valid result"),
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
        let result = validator.validate("data %>% select(name) %>% summarise(count = n())").unwrap();
        
        match result {
            ValidateResult::Valid { .. } => {}, // Should be valid with semantic validation disabled
            ValidateResult::Invalid { .. } => panic!("Expected valid result with semantic validation disabled"),
        }
    }
    
    #[test]
    fn test_validation_error_info() {
        let error_info = ValidationErrorInfo {
            error_type: "parse".to_string(),
            message: "Unexpected token".to_string(),
            position: Some(10),
            context: Some("around position 10".to_string()),
        };
        
        assert_eq!(error_info.error_type, "parse");
        assert_eq!(error_info.message, "Unexpected token");
        assert_eq!(error_info.position, Some(10));
        assert_eq!(error_info.context, Some("around position 10".to_string()));
    }
}