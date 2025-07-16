//! Lexer module
//!
//! Provides functionality to tokenize dplyr code.

use crate::error::{LexError, LexResult};

/// Token types used in dplyr code
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // dplyr functions
    Select,
    Filter,
    Mutate,
    Arrange,
    GroupBy,
    Summarise,
    
    // Operators
    Pipe,           // %>%
    Assignment,     // =
    Equal,          // ==
    NotEqual,       // !=
    LessThan,       // <
    LessThanOrEqual, // <=
    GreaterThan,    // >
    GreaterThanOrEqual, // >=
    And,            // &
    Or,             // |
    Plus,           // +
    Minus,          // -
    Multiply,       // *
    Divide,         // /
    
    // Literals
    Identifier(String),
    String(String),
    Number(f64),
    Boolean(bool),
    
    // Structural tokens
    LeftParen,      // (
    RightParen,     // )
    Comma,          // ,
    Dot,            // .
    
    // Special tokens
    EOF,            // End of file
    Newline,        // Line break
    Whitespace,     // Whitespace (usually ignored)
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Select => write!(f, "select"),
            Token::Filter => write!(f, "filter"),
            Token::Mutate => write!(f, "mutate"),
            Token::Arrange => write!(f, "arrange"),
            Token::GroupBy => write!(f, "group_by"),
            Token::Summarise => write!(f, "summarise"),
            Token::Pipe => write!(f, "%>%"),
            Token::Assignment => write!(f, "="),
            Token::Equal => write!(f, "=="),
            Token::NotEqual => write!(f, "!="),
            Token::LessThan => write!(f, "<"),
            Token::LessThanOrEqual => write!(f, "<="),
            Token::GreaterThan => write!(f, ">"),
            Token::GreaterThanOrEqual => write!(f, ">="),
            Token::And => write!(f, "&"),
            Token::Or => write!(f, "|"),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Multiply => write!(f, "*"),
            Token::Divide => write!(f, "/"),
            Token::Identifier(name) => write!(f, "{}", name),
            Token::String(s) => write!(f, "\"{}\"", s),
            Token::Number(n) => write!(f, "{}", n),
            Token::Boolean(b) => write!(f, "{}", b),
            Token::LeftParen => write!(f, "("),
            Token::RightParen => write!(f, ")"),
            Token::Comma => write!(f, ","),
            Token::Dot => write!(f, "."),
            Token::EOF => write!(f, "EOF"),
            Token::Newline => write!(f, "\\n"),
            Token::Whitespace => write!(f, " "),
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
        let current_char = chars.get(0).copied();
        
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
                        self.advance();
                        Ok(Token::Dot)
                    }
                    '+' => {
                        self.advance();
                        Ok(Token::Plus)
                    }
                    '-' => {
                        self.advance();
                        Ok(Token::Minus)
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
                        if self.current_char == Some('=') {
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
                    '"' | '\'' => {
                        self.read_string()
                    }
                    '\n' => {
                        self.advance();
                        Ok(Token::Newline)
                    }
                    _ if ch.is_ascii_digit() => {
                        self.read_number()
                    }
                    _ if ch.is_ascii_alphabetic() || ch == '_' => {
                        self.read_identifier_or_keyword()
                    }
                    _ => Err(LexError::UnexpectedCharacter(ch, self.position))
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
        // Skip %
        self.advance();
        
        if self.current_char == Some('>') {
            self.advance();
            if self.current_char == Some('%') {
                self.advance();
                Ok(Token::Pipe)
            } else {
                Err(LexError::UnexpectedCharacter('>', self.position))
            }
        } else {
            Err(LexError::UnexpectedCharacter('%', self.position))
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

        number_str.parse::<f64>()
            .map(Token::Number)
            .map_err(|_| LexError::InvalidNumber(number_str, self.position))
    }

    /// Reads an identifier or keyword.
    fn read_identifier_or_keyword(&mut self) -> LexResult<Token> {
        let mut identifier = String::new();

        while let Some(ch) = self.current_char {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                identifier.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        // Check for keywords
        let token = match identifier.as_str() {
            "select" => Token::Select,
            "filter" => Token::Filter,
            "mutate" => Token::Mutate,
            "arrange" => Token::Arrange,
            "group_by" => Token::GroupBy,
            "summarise" | "summarize" => Token::Summarise,
            "TRUE" | "true" => Token::Boolean(true),
            "FALSE" | "false" => Token::Boolean(false),
            _ => Token::Identifier(identifier),
        };

        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tokens() {
        let mut lexer = Lexer::new("(),.".to_string());
        
        assert_eq!(lexer.next_token().unwrap(), Token::LeftParen);
        assert_eq!(lexer.next_token().unwrap(), Token::RightParen);
        assert_eq!(lexer.next_token().unwrap(), Token::Comma);
        assert_eq!(lexer.next_token().unwrap(), Token::Dot);
        assert_eq!(lexer.next_token().unwrap(), Token::EOF);
    }

    #[test]
    fn test_pipe_operator() {
        let mut lexer = Lexer::new("%>%".to_string());
        assert_eq!(lexer.next_token().unwrap(), Token::Pipe);
    }

    #[test]
    fn test_keywords() {
        let mut lexer = Lexer::new("select filter mutate".to_string());
        
        assert_eq!(lexer.next_token().unwrap(), Token::Select);
        assert_eq!(lexer.next_token().unwrap(), Token::Filter);
        assert_eq!(lexer.next_token().unwrap(), Token::Mutate);
    }

    #[test]
    fn test_string_literal() {
        let mut lexer = Lexer::new("\"hello world\"".to_string());
        assert_eq!(lexer.next_token().unwrap(), Token::String("hello world".to_string()));
    }

    #[test]
    fn test_number() {
        let mut lexer = Lexer::new("123.45".to_string());
        assert_eq!(lexer.next_token().unwrap(), Token::Number(123.45));
    }
}