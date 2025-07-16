//! Error type definitions
//!
//! Defines all error types used in libdplyr.

use thiserror::Error;

/// Errors that occur during lexing (tokenization)
#[derive(Debug, Error, Clone, PartialEq)]
pub enum LexError {
    #[error("Unexpected character: '{0}' (position: {1})")]
    UnexpectedCharacter(char, usize),
    
    #[error("Unterminated string literal (start position: {0})")]
    UnterminatedString(usize),
    
    #[error("Invalid number format: '{0}' (position: {1})")]
    InvalidNumber(String, usize),
    
    #[error("Invalid identifier: '{0}' (position: {1})")]
    InvalidIdentifier(String, usize),
    
    #[error("Invalid pipe operator: '{0}' (position: {1})")]
    InvalidPipeOperator(String, usize),
    
    #[error("Unsupported escape sequence: '\\{0}' (position: {1})")]
    InvalidEscapeSequence(char, usize),
    
    #[error("Input is empty")]
    EmptyInput,
}

/// Errors that occur during parsing
#[derive(Debug, Error, Clone, PartialEq)]
pub enum ParseError {
    #[error("Unexpected token: expected '{expected}' but found '{found}' (position: {position})")]
    UnexpectedToken { 
        expected: String, 
        found: String, 
        position: usize 
    },
    
    #[error("Invalid dplyr operation: '{operation}' (position: {position})")]
    InvalidOperation { 
        operation: String, 
        position: usize 
    },
    
    #[error("Missing required argument for function '{function}' (position: {position})")]
    MissingArgument { 
        function: String, 
        position: usize 
    },
    
    #[error("Too many arguments provided for function '{function}' (position: {position})")]
    TooManyArguments { 
        function: String, 
        position: usize 
    },
    
    #[error("Invalid expression syntax: '{expr}' (position: {position})")]
    InvalidExpression { 
        expr: String, 
        position: usize 
    },
    
    #[error("Unsupported function: '{function}' (position: {position})")]
    UnsupportedFunction { 
        function: String, 
        position: usize 
    },
    
    #[error("Invalid column alias: '{alias}' (position: {position})")]
    InvalidAlias { 
        alias: String, 
        position: usize 
    },
    
    #[error("Empty pipeline: at least one operation is required")]
    EmptyPipeline,
    
    #[error("Lexing error: {0}")]
    LexError(#[from] LexError),
    
    #[error("Unexpected end of file (position: {0})")]
    UnexpectedEof(usize),
}

/// Errors that occur during SQL generation
#[derive(Debug, Error, Clone, PartialEq)]
pub enum GenerationError {
    #[error("Unsupported operation in '{dialect}' dialect: '{operation}'")]
    UnsupportedOperation { 
        operation: String, 
        dialect: String 
    },
    
    #[error("Invalid column reference: '{column}'{}", match table.as_ref() { Some(t) => format!(" (table: {})", t), None => String::new() })]
    InvalidColumnReference { 
        column: String, 
        table: Option<String> 
    },
    
    #[error("Unsupported complex expression: '{expr}' (type: {expr_type})")]
    ComplexExpression { 
        expr: String, 
        expr_type: String 
    },
    
    #[error("Invalid AST structure: {reason}")]
    InvalidAst { reason: String },
    
    #[error("Unsupported aggregate function: '{function}' (dialect: {dialect})")]
    UnsupportedAggregateFunction { 
        function: String, 
        dialect: String 
    },
    
    #[error("Invalid data type conversion: from '{from_type}' to '{to_type}'")]
    InvalidTypeConversion { 
        from_type: String, 
        to_type: String 
    },
    
    #[error("Circular reference detected: '{reference}'")]
    CircularReference { reference: String },
    
    #[error("Maximum nesting depth exceeded: {depth} (max: {max_depth})")]
    MaxNestingDepthExceeded { 
        depth: usize, 
        max_depth: usize 
    },
    
    #[error("Empty query: no SQL to generate")]
    EmptyQuery,
}

/// Unified error that can occur during the entire conversion process
#[derive(Debug, Error)]
pub enum TranspileError {
    #[error("Lexing error: {0}")]
    LexError(#[from] LexError),
    
    #[error("Parsing error: {0}")]
    ParseError(#[from] ParseError),
    
    #[error("SQL generation error: {0}")]
    GenerationError(#[from] GenerationError),
}

/// Result type aliases
pub type LexResult<T> = Result<T, LexError>;
pub type ParseResult<T> = Result<T, ParseError>;
pub type GenerationResult<T> = Result<T, GenerationError>;
pub type TranspileResult<T> = Result<T, TranspileError>;