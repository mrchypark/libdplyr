//! Lexer module
//!
//! Provides functionality to tokenize dplyr code.

use crate::error::{LexError, LexResult};
use std::collections::HashMap;

lazy_static::lazy_static! {
    /// Keyword mapping for efficient lookup
    static ref KEYWORDS: HashMap<&'static str, Token> = {
        let mut m = HashMap::new();
        m.insert("select", Token::Select);
        m.insert("filter", Token::Filter);
        m.insert("mutate", Token::Mutate);
        m.insert("rename", Token::Rename);
        m.insert("arrange", Token::Arrange);
        m.insert("group_by", Token::GroupBy);
        m.insert("summarise", Token::Summarise);
        m.insert("summarize", Token::Summarise);
        m.insert("inner_join", Token::InnerJoin);
        m.insert("left_join", Token::LeftJoin);
        m.insert("right_join", Token::RightJoin);
        m.insert("full_join", Token::FullJoin);
        m.insert("semi_join", Token::SemiJoin);
        m.insert("anti_join", Token::AntiJoin);
        m.insert("intersect", Token::Intersect);
        m.insert("union", Token::Union);
        m.insert("setdiff", Token::SetDiff);
        // R functions with dots (treated as identifiers)
        m.insert("is.na", Token::Identifier("is.na".to_string()));
        m.insert("as.numeric", Token::Identifier("as.numeric".to_string()));
        m.insert("as.integer", Token::Identifier("as.integer".to_string()));
        m.insert("as.character", Token::Identifier("as.character".to_string()));
        m.insert("as.logical", Token::Identifier("as.logical".to_string()));
        m.insert("as.double", Token::Identifier("as.double".to_string()));
        m.insert("na.fill", Token::Identifier("na.fill".to_string()));
        m.insert("na.replace", Token::Identifier("na.replace".to_string()));
        m.insert("desc", Token::Desc);
        m.insert("asc", Token::Asc);
        m.insert("TRUE", Token::Boolean(true));
        m.insert("true", Token::Boolean(true));
        m.insert("FALSE", Token::Boolean(false));
        m.insert("false", Token::Boolean(false));
        m.insert("NULL", Token::Null);
        m.insert("null", Token::Null);
        m.insert("NA", Token::Null);
        m
    };
}

/// Token types used in dplyr code
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // dplyr functions
    Select,
    Filter,
    Mutate,
    Rename,
    Arrange,
    GroupBy,
    Summarise,
    InnerJoin,
    LeftJoin,
    RightJoin,
    FullJoin,
    SemiJoin,
    AntiJoin,
    Intersect,
    Union,
    SetDiff,

    // dplyr helper functions
    Desc, // desc()
    Asc,  // asc()

    // Operators
    Pipe,               // %>%
    ArrowRight,         // ->
    ArrowLeft,          // <-
    Assignment,         // =
    Equal,              // ==
    NotEqual,           // !=
    LessThan,           // <
    LessThanOrEqual,    // <=
    GreaterThan,        // >
    GreaterThanOrEqual, // >=
    And,                // &
    Or,                 // |
    Plus,               // +
    Minus,              // -
    Multiply,           // *
    Divide,             // /

    // Literals
    Identifier(String),
    String(String),
    Number(f64),
    Boolean(bool),
    Null, // NULL, NA

    // Structural tokens
    LeftParen,  // (
    RightParen, // )
    Comma,      // ,
    Dot,        // .

    // Special tokens
    EOF,        // End of file
    Newline,    // Line break
    Whitespace, // Whitespace (usually ignored)
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Select => write!(f, "select"),
            Self::Filter => write!(f, "filter"),
            Self::Mutate => write!(f, "mutate"),
            Self::Rename => write!(f, "rename"),
            Self::Arrange => write!(f, "arrange"),
            Self::GroupBy => write!(f, "group_by"),
            Self::Summarise => write!(f, "summarise"),
            Self::InnerJoin => write!(f, "inner_join"),
            Self::LeftJoin => write!(f, "left_join"),
            Self::RightJoin => write!(f, "right_join"),
            Self::FullJoin => write!(f, "full_join"),
            Self::SemiJoin => write!(f, "semi_join"),
            Self::AntiJoin => write!(f, "anti_join"),
            Self::Intersect => write!(f, "intersect"),
            Self::Union => write!(f, "union"),
            Self::SetDiff => write!(f, "setdiff"),
            Self::Desc => write!(f, "desc"),
            Self::Asc => write!(f, "asc"),
            Self::Pipe => write!(f, "%>%"),
            Self::ArrowRight => write!(f, "->"),
            Self::ArrowLeft => write!(f, "<-"),
            Self::Assignment => write!(f, "="),
            Self::Equal => write!(f, "=="),
            Self::NotEqual => write!(f, "!="),
            Self::LessThan => write!(f, "<"),
            Self::LessThanOrEqual => write!(f, "<="),
            Self::GreaterThan => write!(f, ">"),
            Self::GreaterThanOrEqual => write!(f, ">="),
            Self::And => write!(f, "&"),
            Self::Or => write!(f, "|"),
            Self::Plus => write!(f, "+"),
            Self::Minus => write!(f, "-"),
            Self::Multiply => write!(f, "*"),
            Self::Divide => write!(f, "/"),
            Self::Identifier(name) => write!(f, "{name}"),
            Self::String(s) => write!(f, "\"{s}\""),
            Self::Number(n) => write!(f, "{n}"),
            Self::Boolean(b) => write!(f, "{b}"),
            Self::Null => write!(f, "NULL"),
            Self::LeftParen => write!(f, "("),
            Self::RightParen => write!(f, ")"),
            Self::Comma => write!(f, ","),
            Self::Dot => write!(f, "."),
            Self::EOF => write!(f, "EOF"),
            Self::Newline => write!(f, "\\n"),
            Self::Whitespace => write!(f, " "),
        }
    }
}

/// Lexer struct
///
/// Provides functionality to tokenize input strings.
pub struct Lexer {
    input: Vec<char>,
    position: usize,
    current_char: Option<char>,
}

impl Lexer {
    /// Creates a new lexer instance.
    ///
    /// # Arguments
    ///
    /// * `input` - The input string to tokenize
    pub fn new(input: String) -> Self {
        let chars: Vec<char> = input.chars().collect();
        let current_char = chars.first().copied();

        Self {
            input: chars,
            position: 0,
            current_char,
        }
    }

    /// Returns the next token.
    ///
    /// # Returns
    ///
    /// Returns the next token on success, LexError on failure.
    pub fn next_token(&mut self) -> LexResult<Token> {
        // Skip whitespace
        self.skip_whitespace();

        match self.current_char {
            None => Ok(Token::EOF),
            Some(ch) => {
                match ch {
                    '(' => {
                        self.advance();
                        Ok(Token::LeftParen)
                    }
                    ')' => {
                        self.advance();
                        Ok(Token::RightParen)
                    }
                    ',' => {
                        self.advance();
                        Ok(Token::Comma)
                    }
                    '.' => {
                        // Check if this is a decimal number starting with a dot
                        if let Some(next_char) = self.input.get(self.position + 1) {
                            if next_char.is_ascii_digit() {
                                self.read_number()
                            } else {
                                self.advance();
                                Ok(Token::Dot)
                            }
                        } else {
                            self.advance();
                            Ok(Token::Dot)
                        }
                    }
                    '+' => {
                        self.advance();
                        Ok(Token::Plus)
                    }
                    '-' => {
                        self.advance();
                        if self.current_char == Some('>') {
                            self.advance();
                            Ok(Token::ArrowRight)
                        } else {
                            Ok(Token::Minus)
                        }
                    }
                    '*' => {
                        self.advance();
                        Ok(Token::Multiply)
                    }
                    '/' => {
                        self.advance();
                        Ok(Token::Divide)
                    }
                    '=' => {
                        self.advance();
                        if self.current_char == Some('=') {
                            self.advance();
                            Ok(Token::Equal)
                        } else {
                            Ok(Token::Assignment)
                        }
                    }
                    '!' => {
                        self.advance();
                        if self.current_char == Some('=') {
                            self.advance();
                            Ok(Token::NotEqual)
                        } else {
                            Err(LexError::UnexpectedCharacter(ch, self.position))
                        }
                    }
                    '<' => {
                        self.advance();
                        if self.current_char == Some('-') {
                            self.advance();
                            Ok(Token::ArrowLeft)
                        } else if self.current_char == Some('=') {
                            self.advance();
                            Ok(Token::LessThanOrEqual)
                        } else {
                            Ok(Token::LessThan)
                        }
                    }
                    '>' => {
                        self.advance();
                        if self.current_char == Some('=') {
                            self.advance();
                            Ok(Token::GreaterThanOrEqual)
                        } else {
                            Ok(Token::GreaterThan)
                        }
                    }
                    '&' => {
                        self.advance();
                        Ok(Token::And)
                    }
                    '|' => {
                        self.advance();
                        Ok(Token::Or)
                    }
                    '%' => {
                        // Handle pipe operator %>%
                        self.read_pipe_operator()
                    }
                    '"' | '\'' => self.read_string(),
                    '\n' => {
                        self.advance();
                        Ok(Token::Newline)
                    }
                    _ if ch.is_ascii_digit() => self.read_number(),
                    _ if ch.is_ascii_alphabetic() || ch == '_' => self.read_identifier_or_keyword(),
                    _ => Err(LexError::UnexpectedCharacter(ch, self.position)),
                }
            }
        }
    }

    /// Advances the current position to the next character.
    fn advance(&mut self) {
        self.position += 1;
        self.current_char = self.input.get(self.position).copied();
    }

    /// Skips whitespace characters.
    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current_char {
            if ch.is_whitespace() && ch != '\n' {
                self.advance();
            } else {
                break;
            }
        }
    }

    /// Reads the pipe operator %>%.
    fn read_pipe_operator(&mut self) -> LexResult<Token> {
        let start_position = self.position;
        let mut pipe_str = String::new();

        // Read the first %
        pipe_str.push('%');
        self.advance();

        if self.current_char == Some('>') {
            pipe_str.push('>');
            self.advance();
            if self.current_char == Some('%') {
                pipe_str.push('%');
                self.advance();
                Ok(Token::Pipe)
            } else {
                Err(LexError::InvalidPipeOperator(pipe_str, start_position))
            }
        } else {
            // Include the current character in the error string if it exists
            if let Some(ch) = self.current_char {
                pipe_str.push(ch);
            }
            Err(LexError::InvalidPipeOperator(pipe_str, start_position))
        }
    }

    /// Reads a string literal.
    fn read_string(&mut self) -> LexResult<Token> {
        let quote_char = self.current_char.unwrap();
        self.advance(); // Skip opening quote

        let mut value = String::new();

        while let Some(ch) = self.current_char {
            if ch == quote_char {
                self.advance(); // Skip closing quote
                return Ok(Token::String(value));
            } else if ch == '\\' {
                // Handle escape characters
                self.advance();
                match self.current_char {
                    Some('n') => value.push('\n'),
                    Some('t') => value.push('\t'),
                    Some('r') => value.push('\r'),
                    Some('\\') => value.push('\\'),
                    Some('"') => value.push('"'),
                    Some('\'') => value.push('\''),
                    Some(c) => value.push(c),
                    None => return Err(LexError::UnterminatedString(self.position)),
                }
                self.advance();
            } else {
                value.push(ch);
                self.advance();
            }
        }

        Err(LexError::UnterminatedString(self.position))
    }

    /// Reads a number.
    fn read_number(&mut self) -> LexResult<Token> {
        let mut number_str = String::new();

        while let Some(ch) = self.current_char {
            if ch.is_ascii_digit() || ch == '.' {
                number_str.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        number_str
            .parse::<f64>()
            .map(Token::Number)
            .map_err(|_| LexError::InvalidNumber(number_str, self.position))
    }

    /// Reads an identifier or keyword.
    fn read_identifier_or_keyword(&mut self) -> LexResult<Token> {
        let mut identifier = String::new();

        while let Some(ch) = self.current_char {
            // Allow alphanumeric, underscore, and dot (for R compatibility like is.na, as.numeric)
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' {
                identifier.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        // Check for keywords using the static hashmap
        let token = KEYWORDS
            .get(identifier.as_str())
            .cloned()
            .unwrap_or(Token::Identifier(identifier));

        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to create a lexer and collect all tokens
    fn tokenize_all(input: &str) -> LexResult<Vec<Token>> {
        let mut lexer = Lexer::new(input.to_string());
        let mut tokens = Vec::new();

        loop {
            let token = lexer.next_token()?;
            if token == Token::EOF {
                tokens.push(token);
                break;
            }
            tokens.push(token);
        }

        Ok(tokens)
    }

    // Helper function to assert token sequence
    fn assert_tokens(input: &str, expected: Vec<Token>) {
        let tokens = tokenize_all(input).expect("Tokenization should succeed");
        assert_eq!(
            tokens, expected,
            "Token sequence mismatch for input: '{input}'"
        );
    }

    // ===== 기본 토큰 파싱 테스트 =====

    mod basic_token_parsing {
        use super::*;

        #[test]
        fn test_structural_tokens() {
            assert_tokens("()", vec![Token::LeftParen, Token::RightParen, Token::EOF]);
            assert_tokens(",", vec![Token::Comma, Token::EOF]);
            assert_tokens(".", vec![Token::Dot, Token::EOF]);
            assert_tokens(
                "(),.)",
                vec![
                    Token::LeftParen,
                    Token::RightParen,
                    Token::Comma,
                    Token::Dot,
                    Token::RightParen,
                    Token::EOF,
                ],
            );
        }

        #[test]
        fn test_arithmetic_operators() {
            assert_tokens(
                "+ - * /",
                vec![
                    Token::Plus,
                    Token::Minus,
                    Token::Multiply,
                    Token::Divide,
                    Token::EOF,
                ],
            );
        }

        #[test]
        fn test_comparison_operators() {
            assert_tokens(
                "= == != < <= > >=",
                vec![
                    Token::Assignment,
                    Token::Equal,
                    Token::NotEqual,
                    Token::LessThan,
                    Token::LessThanOrEqual,
                    Token::GreaterThan,
                    Token::GreaterThanOrEqual,
                    Token::EOF,
                ],
            );
        }

        #[test]
        fn test_logical_operators() {
            assert_tokens("& |", vec![Token::And, Token::Or, Token::EOF]);
        }

        #[test]
        fn test_identifiers_basic() {
            assert_tokens(
                "name",
                vec![Token::Identifier("name".to_string()), Token::EOF],
            );
            assert_tokens(
                "column_name",
                vec![Token::Identifier("column_name".to_string()), Token::EOF],
            );
            assert_tokens(
                "_private",
                vec![Token::Identifier("_private".to_string()), Token::EOF],
            );
            assert_tokens(
                "var123",
                vec![Token::Identifier("var123".to_string()), Token::EOF],
            );
        }

        #[test]
        fn test_identifiers_edge_cases() {
            // Single character identifiers
            assert_tokens("a", vec![Token::Identifier("a".to_string()), Token::EOF]);
            assert_tokens("_", vec![Token::Identifier("_".to_string()), Token::EOF]);

            // Mixed case identifiers
            assert_tokens(
                "MyColumn",
                vec![Token::Identifier("MyColumn".to_string()), Token::EOF],
            );
            assert_tokens(
                "camelCase",
                vec![Token::Identifier("camelCase".to_string()), Token::EOF],
            );

            // Long identifiers
            let long_name = "very_long_column_name_with_many_underscores_123";
            assert_tokens(
                long_name,
                vec![Token::Identifier(long_name.to_string()), Token::EOF],
            );
        }

        #[test]
        fn test_string_literals_double_quotes() {
            assert_tokens(
                "\"hello\"",
                vec![Token::String("hello".to_string()), Token::EOF],
            );
            assert_tokens(
                "\"hello world\"",
                vec![Token::String("hello world".to_string()), Token::EOF],
            );
            assert_tokens("\"\"", vec![Token::String("".to_string()), Token::EOF]);
        }

        #[test]
        fn test_string_literals_single_quotes() {
            assert_tokens(
                "'hello'",
                vec![Token::String("hello".to_string()), Token::EOF],
            );
            assert_tokens(
                "'hello world'",
                vec![Token::String("hello world".to_string()), Token::EOF],
            );
            assert_tokens("''", vec![Token::String("".to_string()), Token::EOF]);
        }

        #[test]
        fn test_string_literals_with_escapes() {
            assert_tokens(
                "\"hello\\nworld\"",
                vec![Token::String("hello\nworld".to_string()), Token::EOF],
            );
            assert_tokens(
                "\"tab\\there\"",
                vec![Token::String("tab\there".to_string()), Token::EOF],
            );
            assert_tokens(
                "\"quote\\\"here\"",
                vec![Token::String("quote\"here".to_string()), Token::EOF],
            );
            assert_tokens(
                "'single\\'quote'",
                vec![Token::String("single'quote".to_string()), Token::EOF],
            );
            assert_tokens(
                "\"backslash\\\\\"",
                vec![Token::String("backslash\\".to_string()), Token::EOF],
            );
            assert_tokens(
                "\"carriage\\rreturn\"",
                vec![Token::String("carriage\rreturn".to_string()), Token::EOF],
            );
        }

        #[test]
        fn test_string_literals_mixed_quotes() {
            assert_tokens(
                "\"hello\" 'world'",
                vec![
                    Token::String("hello".to_string()),
                    Token::String("world".to_string()),
                    Token::EOF,
                ],
            );
        }

        #[test]
        fn test_numbers_integers() {
            assert_tokens("0", vec![Token::Number(0.0), Token::EOF]);
            assert_tokens("123", vec![Token::Number(123.0), Token::EOF]);
            assert_tokens("999", vec![Token::Number(999.0), Token::EOF]);
        }

        #[test]
        fn test_numbers_decimals() {
            assert_tokens("0.5", vec![Token::Number(0.5), Token::EOF]);
            assert_tokens("123.456", vec![Token::Number(123.456), Token::EOF]);
            assert_tokens("0.0", vec![Token::Number(0.0), Token::EOF]);
            assert_tokens(".5", vec![Token::Number(0.5), Token::EOF]);
        }

        #[test]
        fn test_numbers_edge_cases() {
            assert_tokens("0.0000001", vec![Token::Number(0.0000001), Token::EOF]);
            assert_tokens(
                "999999.999999",
                vec![Token::Number(999999.999999), Token::EOF],
            );
        }

        #[test]
        fn test_boolean_literals() {
            assert_tokens("TRUE", vec![Token::Boolean(true), Token::EOF]);
            assert_tokens("FALSE", vec![Token::Boolean(false), Token::EOF]);
            assert_tokens("true", vec![Token::Boolean(true), Token::EOF]);
            assert_tokens("false", vec![Token::Boolean(false), Token::EOF]);
        }

        #[test]
        fn test_null_literals() {
            assert_tokens("NULL", vec![Token::Null, Token::EOF]);
            assert_tokens("null", vec![Token::Null, Token::EOF]);
            assert_tokens("NA", vec![Token::Null, Token::EOF]);
        }
    }

    // ===== dplyr 함수 토큰 인식 테스트 =====

    mod dplyr_function_recognition {
        use super::*;

        #[test]
        fn test_core_dplyr_functions() {
            assert_tokens("select", vec![Token::Select, Token::EOF]);
            assert_tokens("filter", vec![Token::Filter, Token::EOF]);
            assert_tokens("mutate", vec![Token::Mutate, Token::EOF]);
            assert_tokens("arrange", vec![Token::Arrange, Token::EOF]);
            assert_tokens("group_by", vec![Token::GroupBy, Token::EOF]);
            assert_tokens("summarise", vec![Token::Summarise, Token::EOF]);
        }

        #[test]
        fn test_summarise_alias() {
            // Both spellings should work
            assert_tokens("summarise", vec![Token::Summarise, Token::EOF]);
            assert_tokens("summarize", vec![Token::Summarise, Token::EOF]);
        }

        #[test]
        fn test_helper_functions() {
            assert_tokens("desc", vec![Token::Desc, Token::EOF]);
            assert_tokens("asc", vec![Token::Asc, Token::EOF]);
        }

        #[test]
        fn test_dplyr_functions_case_sensitivity() {
            // These should be treated as identifiers, not keywords
            assert_tokens(
                "SELECT",
                vec![Token::Identifier("SELECT".to_string()), Token::EOF],
            );
            assert_tokens(
                "Filter",
                vec![Token::Identifier("Filter".to_string()), Token::EOF],
            );
            assert_tokens(
                "MUTATE",
                vec![Token::Identifier("MUTATE".to_string()), Token::EOF],
            );
        }

        #[test]
        fn test_dplyr_functions_in_sequence() {
            assert_tokens(
                "select filter mutate",
                vec![Token::Select, Token::Filter, Token::Mutate, Token::EOF],
            );
        }

        #[test]
        fn test_dplyr_functions_with_parentheses() {
            assert_tokens(
                "select()",
                vec![
                    Token::Select,
                    Token::LeftParen,
                    Token::RightParen,
                    Token::EOF,
                ],
            );
            assert_tokens(
                "filter(age)",
                vec![
                    Token::Filter,
                    Token::LeftParen,
                    Token::Identifier("age".to_string()),
                    Token::RightParen,
                    Token::EOF,
                ],
            );
        }
    }

    // ===== 파이프 연산자 및 특수 문자 처리 테스트 =====

    mod pipe_operator_and_special_chars {
        use super::*;

        #[test]
        fn test_pipe_operator_basic() {
            assert_tokens("%>%", vec![Token::Pipe, Token::EOF]);
        }

        #[test]
        fn test_pipe_operator_in_expression() {
            assert_tokens(
                "data %>% select",
                vec![
                    Token::Identifier("data".to_string()),
                    Token::Pipe,
                    Token::Select,
                    Token::EOF,
                ],
            );
        }

        #[test]
        fn test_multiple_pipe_operators() {
            assert_tokens(
                "data %>% select %>% filter",
                vec![
                    Token::Identifier("data".to_string()),
                    Token::Pipe,
                    Token::Select,
                    Token::Pipe,
                    Token::Filter,
                    Token::EOF,
                ],
            );
        }

        #[test]
        fn test_pipe_operator_with_whitespace() {
            assert_tokens(
                "data  %>%  select",
                vec![
                    Token::Identifier("data".to_string()),
                    Token::Pipe,
                    Token::Select,
                    Token::EOF,
                ],
            );
        }

        #[test]
        fn test_newline_handling() {
            assert_tokens(
                "select\nfilter",
                vec![Token::Select, Token::Newline, Token::Filter, Token::EOF],
            );

            assert_tokens(
                "data %>%\nselect(name)",
                vec![
                    Token::Identifier("data".to_string()),
                    Token::Pipe,
                    Token::Newline,
                    Token::Select,
                    Token::LeftParen,
                    Token::Identifier("name".to_string()),
                    Token::RightParen,
                    Token::EOF,
                ],
            );
        }

        #[test]
        fn test_whitespace_preservation() {
            // Whitespace should be skipped except newlines
            assert_tokens(
                "  select   filter  ",
                vec![Token::Select, Token::Filter, Token::EOF],
            );

            assert_tokens(
                "\t\tselect\t\tfilter\t\t",
                vec![Token::Select, Token::Filter, Token::EOF],
            );
        }

        #[test]
        fn test_complex_expression_with_special_chars() {
            let input = "data %>% select(name, age) %>% filter(age > 18 & name != \"test\")";
            let expected = vec![
                Token::Identifier("data".to_string()),
                Token::Pipe,
                Token::Select,
                Token::LeftParen,
                Token::Identifier("name".to_string()),
                Token::Comma,
                Token::Identifier("age".to_string()),
                Token::RightParen,
                Token::Pipe,
                Token::Filter,
                Token::LeftParen,
                Token::Identifier("age".to_string()),
                Token::GreaterThan,
                Token::Number(18.0),
                Token::And,
                Token::Identifier("name".to_string()),
                Token::NotEqual,
                Token::String("test".to_string()),
                Token::RightParen,
                Token::EOF,
            ];
            assert_tokens(input, expected);
        }
    }

    // ===== 오류 케이스 테스트 =====

    mod error_cases {
        use super::*;

        #[test]
        fn test_unterminated_string_double_quote() {
            let mut lexer = Lexer::new("\"unterminated".to_string());
            match lexer.next_token() {
                Err(LexError::UnterminatedString(_)) => {}
                other => panic!("Expected UnterminatedString error, got: {other:?}"),
            }
        }

        #[test]
        fn test_unterminated_string_single_quote() {
            let mut lexer = Lexer::new("'unterminated".to_string());
            match lexer.next_token() {
                Err(LexError::UnterminatedString(_)) => {}
                other => panic!("Expected UnterminatedString error, got: {other:?}"),
            }
        }

        #[test]
        fn test_unterminated_string_with_escape() {
            let mut lexer = Lexer::new("\"test\\".to_string());
            match lexer.next_token() {
                Err(LexError::UnterminatedString(_)) => {}
                other => panic!("Expected UnterminatedString error, got: {other:?}"),
            }
        }

        #[test]
        fn test_invalid_pipe_operator_incomplete() {
            let mut lexer = Lexer::new("%>".to_string());
            match lexer.next_token() {
                Err(LexError::InvalidPipeOperator(op, _)) => {
                    assert_eq!(op, "%>");
                }
                other => panic!("Expected InvalidPipeOperator error, got: {other:?}"),
            }
        }

        #[test]
        fn test_invalid_pipe_operator_wrong_sequence() {
            let mut lexer = Lexer::new("%<".to_string());
            match lexer.next_token() {
                Err(LexError::InvalidPipeOperator(op, _)) => {
                    assert_eq!(op, "%<");
                }
                other => panic!("Expected InvalidPipeOperator error, got: {other:?}"),
            }
        }

        #[test]
        fn test_invalid_pipe_operator_partial() {
            let mut lexer = Lexer::new("%".to_string());
            match lexer.next_token() {
                Err(LexError::InvalidPipeOperator(op, _)) => {
                    assert_eq!(op, "%");
                }
                other => panic!("Expected InvalidPipeOperator error, got: {other:?}"),
            }
        }

        #[test]
        fn test_invalid_number_multiple_dots() {
            let mut lexer = Lexer::new("123.45.67".to_string());
            match lexer.next_token() {
                Err(LexError::InvalidNumber(num, _)) => {
                    assert_eq!(num, "123.45.67");
                }
                other => panic!("Expected InvalidNumber error, got: {other:?}"),
            }
        }

        #[test]
        fn test_invalid_number_trailing_dot() {
            // This should actually be valid (parsed as 123. -> 123.0)
            let mut lexer = Lexer::new("123.".to_string());
            assert_eq!(lexer.next_token().unwrap(), Token::Number(123.0));
        }

        #[test]
        fn test_unexpected_character_symbols() {
            let test_cases = vec!['@', '#', '$', '^', '~', '`', '[', ']', '{', '}'];

            for ch in test_cases {
                let mut lexer = Lexer::new(ch.to_string());
                match lexer.next_token() {
                    Err(LexError::UnexpectedCharacter(found_ch, _)) => {
                        assert_eq!(found_ch, ch, "Expected character '{ch}' in error");
                    }
                    other => {
                        panic!("Expected UnexpectedCharacter error for '{ch}', got: {other:?}")
                    }
                }
            }
        }

        #[test]
        fn test_unexpected_character_unicode() {
            let mut lexer = Lexer::new("한글".to_string());
            match lexer.next_token() {
                Err(LexError::UnexpectedCharacter('한', _)) => {}
                other => panic!("Expected UnexpectedCharacter error for Unicode, got: {other:?}"),
            }
        }

        #[test]
        fn test_exclamation_without_equals() {
            let mut lexer = Lexer::new("!".to_string());
            match lexer.next_token() {
                Err(LexError::UnexpectedCharacter('!', _)) => {}
                other => panic!("Expected UnexpectedCharacter error for '!', got: {other:?}"),
            }
        }

        #[test]
        fn test_error_position_tracking() {
            let mut lexer = Lexer::new("select @".to_string());

            // First token should be fine
            assert_eq!(lexer.next_token().unwrap(), Token::Select);

            // Second token should error at position 7
            match lexer.next_token() {
                Err(LexError::UnexpectedCharacter('@', pos)) => {
                    assert_eq!(pos, 7, "Error position should be 7");
                }
                other => panic!("Expected UnexpectedCharacter error, got: {other:?}"),
            }
        }
    }

    // ===== 통합 테스트 =====

    mod integration_tests {
        use super::*;

        #[test]
        fn test_empty_input() {
            assert_tokens("", vec![Token::EOF]);
        }

        #[test]
        fn test_whitespace_only() {
            assert_tokens("   \t  ", vec![Token::EOF]);
        }

        #[test]
        fn test_complex_dplyr_chain() {
            let input = r#"
                data %>%
                select(name, age, salary) %>%
                filter(age >= 18 & salary > 50000) %>%
                mutate(bonus = salary * 0.1) %>%
                arrange(desc(salary)) %>%
                group_by(department) %>%
                summarise(avg_salary = mean(salary))
            "#;

            let tokens = tokenize_all(input).expect("Should tokenize successfully");

            // Verify we have the expected dplyr functions
            let function_count = tokens
                .iter()
                .filter(|t| {
                    matches!(
                        t,
                        Token::Select
                            | Token::Filter
                            | Token::Mutate
                            | Token::Arrange
                            | Token::GroupBy
                            | Token::Summarise
                    )
                })
                .count();

            assert_eq!(function_count, 6, "Should have 6 dplyr functions");
        }

        #[test]
        fn test_mixed_quotes_and_operators() {
            let input = r#"filter(name == "John" | name == 'Jane' & age != 25)"#;
            let tokens = tokenize_all(input).expect("Should tokenize successfully");

            // Should contain both string types and various operators
            assert!(tokens.contains(&Token::Filter));
            assert!(tokens.contains(&Token::String("John".to_string())));
            assert!(tokens.contains(&Token::String("Jane".to_string())));
            assert!(tokens.contains(&Token::Equal));
            assert!(tokens.contains(&Token::Or));
            assert!(tokens.contains(&Token::And));
            assert!(tokens.contains(&Token::NotEqual));
        }

        #[test]
        fn test_numbers_in_expressions() {
            let input = "filter(age > 18.5 & salary >= 1000.0 & score == 95)";
            let tokens = tokenize_all(input).expect("Should tokenize successfully");

            assert!(tokens.contains(&Token::Number(18.5)));
            assert!(tokens.contains(&Token::Number(1000.0)));
            assert!(tokens.contains(&Token::Number(95.0)));
        }

        #[test]
        fn test_boolean_and_null_in_context() {
            let input = "filter(active == TRUE & deleted != FALSE & notes != NULL)";
            let tokens = tokenize_all(input).expect("Should tokenize successfully");

            assert!(tokens.contains(&Token::Boolean(true)));
            assert!(tokens.contains(&Token::Boolean(false)));
            assert!(tokens.contains(&Token::Null));
        }
    }
}
