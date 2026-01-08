//! Parser module
//!
//! Provides functionality to convert tokens to AST (Abstract Syntax Tree).

use crate::error::{ParseError, ParseResult};
use crate::lexer::{Lexer, Token};

pub use super::ast::*;

/// Parser struct
///
/// Provides functionality to parse dplyr tokens into an Abstract Syntax Tree (AST).
pub struct Parser {
    lexer: Lexer,
    current_token: Token,
    position: usize,
    line: usize,
    column: usize,
}

impl Parser {
    /// Creates a new parser instance.
    ///
    /// # Arguments
    ///
    /// * `lexer` - The lexer instance to use
    ///
    /// # Returns
    ///
    /// Returns a new Parser instance on success, ParseError on failure.
    ///
    /// # Examples
    ///
    /// ```
    /// use libdplyr::lexer::Lexer;
    /// use libdplyr::parser::Parser;
    ///
    /// let lexer = Lexer::new("select(name)".to_string());
    /// let parser = Parser::new(lexer).unwrap();
    /// ```
    pub fn new(mut lexer: Lexer) -> ParseResult<Self> {
        let current_token = lexer.next_token()?;
        Ok(Self {
            lexer,
            current_token,
            position: 0,
            line: 1,
            column: 1,
        })
    }

    /// Parses dplyr code to generate an AST.
    ///
    /// # Returns
    ///
    /// Returns DplyrNode on success, ParseError on failure.
    pub fn parse(&mut self) -> ParseResult<DplyrNode> {
        self.parse_pipeline()
    }

    /// Returns the current source location.
    fn current_location(&self) -> SourceLocation {
        SourceLocation::new(self.line, self.column, self.position)
    }

    /// Advances to the next token and updates position tracking.
    fn advance(&mut self) -> ParseResult<()> {
        // Update line and column tracking
        if self.current_token == Token::Newline {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }

        self.current_token = self.lexer.next_token()?;
        self.position += 1;
        Ok(())
    }

    /// Checks if the current token matches the expected token and advances.
    fn expect_token(&mut self, expected: Token) -> ParseResult<()> {
        if std::mem::discriminant(&self.current_token) == std::mem::discriminant(&expected) {
            self.advance()
        } else {
            Err(ParseError::UnexpectedToken {
                expected: format!("{expected}"),
                found: format!("{}", self.current_token),
                position: self.position,
            })
        }
    }

    /// Checks if the current token matches any of the expected tokens.
    #[allow(dead_code)]
    fn match_token(&self, tokens: &[Token]) -> bool {
        tokens.iter().any(|token| {
            std::mem::discriminant(&self.current_token) == std::mem::discriminant(token)
        })
    }

    /// Skips newline tokens.
    fn skip_newlines(&mut self) -> ParseResult<()> {
        while self.current_token == Token::Newline {
            self.advance()?;
        }
        Ok(())
    }

    /// Checks if we've reached the end of input.
    #[allow(dead_code)]
    fn is_at_end(&self) -> bool {
        self.current_token == Token::EOF
    }

    /// Parses a pipeline.
    ///
    /// A pipeline can start with:
    /// 1. A data source identifier (e.g., "data %>% select(...)")
    /// 2. A dplyr operation directly (e.g., "select(...) %>% filter(...)")
    fn parse_pipeline(&mut self) -> ParseResult<DplyrNode> {
        let start_location = self.current_location();
        let mut operations = Vec::new();

        // Skip any leading newlines
        self.skip_newlines()?;

        // Check for EOF after skipping newlines
        if self.current_token == Token::EOF {
            return Err(ParseError::InvalidOperation {
                operation: "empty pipeline".to_string(),
                position: self.position,
            });
        }

        // Check if we start with a data source (identifier not followed by parentheses)
        if let Token::Identifier(name) = &self.current_token {
            let name = name.clone();
            self.advance()?;

            // Skip newlines after identifier
            self.skip_newlines()?;

            // If followed by pipe operator, this is a data source
            if self.current_token == Token::Pipe {
                // This is a data source followed by operations
                self.advance()?; // Skip %>%
                self.skip_newlines()?; // Skip newlines after pipe

                // Parse the first operation after the data source
                let first_operation = self.parse_operation()?;
                operations.push(first_operation);

                // Parse additional operations connected by pipe operators
                while self.current_token == Token::Pipe {
                    self.advance()?; // Skip %>%
                    self.skip_newlines()?; // Skip newlines after pipe
                    operations.push(self.parse_operation()?);
                }

                return Ok(DplyrNode::Pipeline {
                    source: Some(name),
                    operations,
                    location: start_location,
                });
            } else if self.current_token == Token::LeftParen {
                // This might be a function call, backtrack and parse as operation
                // We need to handle this case by creating a synthetic identifier token
                // and parsing it as a function call
                return Err(ParseError::UnexpectedToken {
                    expected: "dplyr function or pipe operator".to_string(),
                    found: format!("{name}("),
                    position: self.position,
                });
            } else {
                // Single identifier without pipe - treat as data source
                return Ok(DplyrNode::DataSource {
                    name,
                    location: start_location,
                });
            }
        }

        // Parse first operation (no data source prefix)
        let first_operation = self.parse_operation()?;
        operations.push(first_operation);

        // Parse additional operations connected by pipe operators
        while self.current_token == Token::Pipe {
            self.advance()?; // Skip %>%
            self.skip_newlines()?; // Skip newlines after pipe
            operations.push(self.parse_operation()?);
        }

        // Skip trailing newlines
        self.skip_newlines()?;

        Ok(DplyrNode::Pipeline {
            source: None,
            operations,
            location: start_location,
        })
    }

    /// Parses individual dplyr operations.
    fn parse_operation(&mut self) -> ParseResult<DplyrOperation> {
        match &self.current_token {
            Token::Select => self.parse_select(),
            Token::Filter => self.parse_filter(),
            Token::Mutate => self.parse_mutate(),
            Token::Rename => self.parse_rename(),
            Token::Arrange => self.parse_arrange(),
            Token::GroupBy => self.parse_group_by(),
            Token::Summarise => self.parse_summarise(),
            Token::InnerJoin
            | Token::LeftJoin
            | Token::RightJoin
            | Token::FullJoin
            | Token::SemiJoin
            | Token::AntiJoin => self.parse_join(),
            _ => Err(ParseError::UnexpectedToken {
                expected: "dplyr function".to_string(),
                found: format!("{}", self.current_token),
                position: self.position,
            }),
        }
    }

    /// Parses select() operation.
    fn parse_select(&mut self) -> ParseResult<DplyrOperation> {
        let location = self.current_location();
        self.advance()?; // Skip 'select'
        self.expect_token(Token::LeftParen)?;

        let mut columns = Vec::new();

        // First column
        if self.current_token != Token::RightParen {
            columns.push(self.parse_column_expr()?);

            // Additional columns (comma-separated)
            while self.current_token == Token::Comma {
                self.advance()?; // Skip comma
                columns.push(self.parse_column_expr()?);
            }
        }

        self.expect_token(Token::RightParen)?;
        Ok(DplyrOperation::Select { columns, location })
    }

    /// Parses filter() operation.
    fn parse_filter(&mut self) -> ParseResult<DplyrOperation> {
        let location = self.current_location();
        self.advance()?; // Skip 'filter'
        self.expect_token(Token::LeftParen)?;

        let condition = self.parse_expression()?;

        self.expect_token(Token::RightParen)?;
        Ok(DplyrOperation::Filter {
            condition,
            location,
        })
    }

    /// Parses mutate() operation.
    fn parse_mutate(&mut self) -> ParseResult<DplyrOperation> {
        let location = self.current_location();
        self.advance()?; // Skip 'mutate'
        self.expect_token(Token::LeftParen)?;

        let mut assignments = Vec::new();

        // First assignment
        if self.current_token != Token::RightParen {
            assignments.push(self.parse_assignment()?);

            // Additional assignments (comma-separated)
            while self.current_token == Token::Comma {
                self.advance()?; // Skip comma
                assignments.push(self.parse_assignment()?);
            }
        }

        self.expect_token(Token::RightParen)?;
        Ok(DplyrOperation::Mutate {
            assignments,
            location,
        })
    }

    /// Parses rename() operation.
    ///
    /// dplyr-style syntax: `rename(new_name = old_name, ...)`
    fn parse_rename(&mut self) -> ParseResult<DplyrOperation> {
        let location = self.current_location();
        self.advance()?; // Skip 'rename'
        self.expect_token(Token::LeftParen)?;

        let mut renames = Vec::new();
        if self.current_token != Token::RightParen {
            renames.push(self.parse_rename_spec()?);
            while self.current_token == Token::Comma {
                self.advance()?; // Skip comma
                renames.push(self.parse_rename_spec()?);
            }
        }

        self.expect_token(Token::RightParen)?;
        Ok(DplyrOperation::Rename { renames, location })
    }

    fn parse_rename_spec(&mut self) -> ParseResult<RenameSpec> {
        let new_name = self.parse_identifier_like("new column name")?;
        self.expect_token(Token::Assignment)?;
        let old_name = self.parse_identifier_like("existing column name")?;
        Ok(RenameSpec { new_name, old_name })
    }

    fn parse_identifier_like(&mut self, expected: &str) -> ParseResult<String> {
        match &self.current_token {
            Token::Identifier(name) => {
                let name = name.clone();
                self.advance()?;
                Ok(name)
            }
            Token::String(name) => {
                let name = name.clone();
                self.advance()?;
                Ok(name)
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: expected.to_string(),
                found: format!("{}", self.current_token),
                position: self.position,
            }),
        }
    }

    /// Parses arrange() operation.
    fn parse_arrange(&mut self) -> ParseResult<DplyrOperation> {
        let location = self.current_location();
        self.advance()?; // Skip 'arrange'
        self.expect_token(Token::LeftParen)?;

        let mut columns = Vec::new();

        // First sort column
        if self.current_token != Token::RightParen {
            columns.push(self.parse_order_expr()?);

            // Additional sort columns (comma-separated)
            while self.current_token == Token::Comma {
                self.advance()?; // Skip comma
                columns.push(self.parse_order_expr()?);
            }
        }

        self.expect_token(Token::RightParen)?;
        Ok(DplyrOperation::Arrange { columns, location })
    }

    /// Parses group_by() operation.
    fn parse_group_by(&mut self) -> ParseResult<DplyrOperation> {
        let location = self.current_location();
        self.advance()?; // Skip 'group_by'
        self.expect_token(Token::LeftParen)?;

        let mut columns = Vec::new();

        // First group column
        if self.current_token != Token::RightParen {
            if let Token::Identifier(name) = &self.current_token {
                columns.push(name.clone());
                self.advance()?;

                // Additional group columns (comma-separated)
                while self.current_token == Token::Comma {
                    self.advance()?; // Skip comma
                    if let Token::Identifier(name) = &self.current_token {
                        columns.push(name.clone());
                        self.advance()?;
                    } else {
                        return Err(ParseError::UnexpectedToken {
                            expected: "identifier".to_string(),
                            found: format!("{}", self.current_token),
                            position: self.position,
                        });
                    }
                }
            }
        }

        self.expect_token(Token::RightParen)?;
        Ok(DplyrOperation::GroupBy { columns, location })
    }

    /// Parses summarise() operation.
    fn parse_summarise(&mut self) -> ParseResult<DplyrOperation> {
        let location = self.current_location();
        self.advance()?; // Skip 'summarise'
        self.expect_token(Token::LeftParen)?;

        let mut aggregations = Vec::new();

        // First aggregation
        if self.current_token != Token::RightParen {
            aggregations.push(self.parse_aggregation()?);

            // Additional aggregations (comma-separated)
            while self.current_token == Token::Comma {
                self.advance()?; // Skip comma
                aggregations.push(self.parse_aggregation()?);
            }
        }

        self.expect_token(Token::RightParen)?;
        Ok(DplyrOperation::Summarise {
            aggregations,
            location,
        })
    }

    /// Parses join operations (inner_join, left_join, right_join, full_join, semi_join, anti_join).
    fn parse_join(&mut self) -> ParseResult<DplyrOperation> {
        let join_type = match &self.current_token {
            Token::InnerJoin => JoinType::Inner,
            Token::LeftJoin => JoinType::Left,
            Token::RightJoin => JoinType::Right,
            Token::FullJoin => JoinType::Full,
            Token::SemiJoin => JoinType::Semi,
            Token::AntiJoin => JoinType::Anti,
            _ => {
                return Err(ParseError::UnexpectedToken {
                    expected: "join function".to_string(),
                    found: format!("{}", self.current_token),
                    position: self.position,
                })
            }
        };

        let location = self.current_location();
        self.advance()?; // Skip join function name
        self.expect_token(Token::LeftParen)?;

        // Parse first argument: table name
        let table_name = match &self.current_token {
            Token::Identifier(name) => name.clone(),
            _ => {
                return Err(ParseError::UnexpectedToken {
                    expected: "table name".to_string(),
                    found: format!("{}", self.current_token),
                    position: self.position,
                })
            }
        };
        self.advance()?;

        // Parse by parameter
        if self.current_token != Token::RightParen && self.current_token != Token::Comma {
            return Err(ParseError::UnexpectedToken {
                expected: "comma or closing paren".to_string(),
                found: format!("{}", self.current_token),
                position: self.position,
            });
        }

        self.expect_token(Token::Comma)?;
        self.expect_token(Token::Identifier("by".to_string()))?;
        self.expect_token(Token::Assignment)?;

        // Parse by parameter - handle string literal as column name
        let (by_column, on_expr) = match &self.current_token {
            Token::String(s) => {
                // by = "column_name" - simple join on same column name
                let col_name = s.clone();
                self.advance()?;
                (Some(col_name), None)
            }
            Token::Identifier(_) => {
                // Could be a column reference or complex expression
                // For now, parse as expression
                let expr = self.parse_expression()?;
                (None, Some(expr))
            }
            _ => {
                return Err(ParseError::UnexpectedToken {
                    expected: "string literal or identifier for join column".to_string(),
                    found: format!("{}", self.current_token),
                    position: self.position,
                })
            }
        };

        self.expect_token(Token::RightParen)?;

        Ok(DplyrOperation::Join {
            join_type,
            spec: JoinSpec {
                table: table_name,
                by_column,
                on_expr,
            },
            location,
        })
    }

    /// Parses column expressions.
    fn parse_column_expr(&mut self) -> ParseResult<ColumnExpr> {
        // Check if this is an alias assignment (alias = expr)
        if let Token::Identifier(first_name) = &self.current_token {
            let first_name = first_name.clone();

            // Advance past the identifier
            self.advance()?;

            // Check if next token is assignment
            if self.current_token == Token::Assignment {
                // This is an alias assignment: alias = expr
                self.advance()?; // Skip =
                let expr = self.parse_expression()?;
                return Ok(ColumnExpr {
                    expr,
                    alias: Some(first_name),
                });
            } else if self.current_token == Token::LeftParen {
                // This is a function call, we need to backtrack and parse as expression
                // Put the identifier back and parse as a full expression
                // Since we can't backtrack easily, we'll handle function call here
                self.advance()?; // Skip (

                let mut args = Vec::new();
                if self.current_token != Token::RightParen {
                    args.push(self.parse_expression()?);

                    while self.current_token == Token::Comma {
                        self.advance()?; // Skip ,
                        args.push(self.parse_expression()?);
                    }
                }

                self.expect_token(Token::RightParen)?;
                let expr = Expr::Function {
                    name: first_name,
                    args,
                };
                return Ok(ColumnExpr { expr, alias: None });
            } else {
                // Not an alias or function call, treat the identifier as a regular expression
                // We already consumed the identifier, so create an Identifier expression
                return Ok(ColumnExpr {
                    expr: Expr::Identifier(first_name),
                    alias: None,
                });
            }
        }

        // Regular expression without alias (for non-identifier expressions)
        let expr = self.parse_expression()?;
        Ok(ColumnExpr { expr, alias: None })
    }

    /// Parses assignment statements.
    fn parse_assignment(&mut self) -> ParseResult<Assignment> {
        if let Token::Identifier(column) = &self.current_token {
            let column = column.clone();
            self.advance()?;

            self.expect_token(Token::Assignment)?;
            let expr = self.parse_expression()?;

            Ok(Assignment { column, expr })
        } else {
            Err(ParseError::UnexpectedToken {
                expected: "column identifier".to_string(),
                found: format!("{}", self.current_token),
                position: self.position,
            })
        }
    }

    /// Parses sort expressions.
    fn parse_order_expr(&mut self) -> ParseResult<OrderExpr> {
        // Check for desc() or asc() functions
        match &self.current_token {
            Token::Desc => {
                self.advance()?; // Skip 'desc'
                self.expect_token(Token::LeftParen)?;

                if let Token::Identifier(column) = &self.current_token {
                    let column = column.clone();
                    self.advance()?;
                    self.expect_token(Token::RightParen)?;

                    Ok(OrderExpr {
                        column,
                        direction: OrderDirection::Desc,
                    })
                } else {
                    Err(ParseError::UnexpectedToken {
                        expected: "column identifier".to_string(),
                        found: format!("{}", self.current_token),
                        position: self.position,
                    })
                }
            }
            Token::Asc => {
                self.advance()?; // Skip 'asc'
                self.expect_token(Token::LeftParen)?;

                if let Token::Identifier(column) = &self.current_token {
                    let column = column.clone();
                    self.advance()?;
                    self.expect_token(Token::RightParen)?;

                    Ok(OrderExpr {
                        column,
                        direction: OrderDirection::Asc,
                    })
                } else {
                    Err(ParseError::UnexpectedToken {
                        expected: "column identifier".to_string(),
                        found: format!("{}", self.current_token),
                        position: self.position,
                    })
                }
            }
            Token::Identifier(name) => {
                if name == "desc" {
                    self.advance()?; // Skip 'desc'
                    self.expect_token(Token::LeftParen)?;

                    if let Token::Identifier(column) = &self.current_token {
                        let column = column.clone();
                        self.advance()?;
                        self.expect_token(Token::RightParen)?;

                        Ok(OrderExpr {
                            column,
                            direction: OrderDirection::Desc,
                        })
                    } else {
                        Err(ParseError::UnexpectedToken {
                            expected: "column identifier".to_string(),
                            found: format!("{}", self.current_token),
                            position: self.position,
                        })
                    }
                } else if name == "asc" {
                    self.advance()?; // Skip 'asc'
                    self.expect_token(Token::LeftParen)?;

                    if let Token::Identifier(column) = &self.current_token {
                        let column = column.clone();
                        self.advance()?;
                        self.expect_token(Token::RightParen)?;

                        Ok(OrderExpr {
                            column,
                            direction: OrderDirection::Asc,
                        })
                    } else {
                        Err(ParseError::UnexpectedToken {
                            expected: "column identifier".to_string(),
                            found: format!("{}", self.current_token),
                            position: self.position,
                        })
                    }
                } else {
                    // Regular column (ascending by default)
                    let column = name.clone();
                    self.advance()?;
                    Ok(OrderExpr {
                        column,
                        direction: OrderDirection::Asc,
                    })
                }
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: "column identifier, desc(), or asc()".to_string(),
                found: format!("{}", self.current_token),
                position: self.position,
            }),
        }
    }

    /// Parses aggregation operations.
    fn parse_aggregation(&mut self) -> ParseResult<Aggregation> {
        // Handle alias = aggregation_function(column) format
        if let Token::Identifier(first_name) = &self.current_token {
            let first_name = first_name.clone();
            self.advance()?;

            // If = token exists, it's an alias
            if self.current_token == Token::Assignment {
                self.advance()?; // Skip =

                // Aggregation function name
                if let Token::Identifier(function) = &self.current_token {
                    let function = function.clone();
                    self.advance()?;

                    self.expect_token(Token::LeftParen)?;

                    // Handle functions with no arguments (like n())
                    if self.current_token == Token::RightParen {
                        self.advance()?; // Skip )
                        Ok(Aggregation {
                            function,
                            column: "".to_string(), // Empty column for functions like n()
                            alias: Some(first_name),
                        })
                    } else if let Token::Identifier(column) = &self.current_token {
                        let column = column.clone();
                        self.advance()?;
                        self.expect_token(Token::RightParen)?;

                        Ok(Aggregation {
                            function,
                            column,
                            alias: Some(first_name),
                        })
                    } else {
                        Err(ParseError::UnexpectedToken {
                            expected: "column identifier or closing parenthesis".to_string(),
                            found: format!("{}", self.current_token),
                            position: self.position,
                        })
                    }
                } else {
                    Err(ParseError::UnexpectedToken {
                        expected: "aggregation function name".to_string(),
                        found: format!("{}", self.current_token),
                        position: self.position,
                    })
                }
            } else {
                // Function(column) format without alias
                self.expect_token(Token::LeftParen)?;

                // Handle functions with no arguments (like n())
                if self.current_token == Token::RightParen {
                    self.advance()?; // Skip )
                    Ok(Aggregation {
                        function: first_name,
                        column: "".to_string(), // Empty column for functions like n()
                        alias: None,
                    })
                } else if let Token::Identifier(column) = &self.current_token {
                    let column = column.clone();
                    self.advance()?;
                    self.expect_token(Token::RightParen)?;

                    Ok(Aggregation {
                        function: first_name,
                        column,
                        alias: None,
                    })
                } else {
                    Err(ParseError::UnexpectedToken {
                        expected: "column identifier or closing parenthesis".to_string(),
                        found: format!("{}", self.current_token),
                        position: self.position,
                    })
                }
            }
        } else {
            Err(ParseError::UnexpectedToken {
                expected: "aggregation function name or alias".to_string(),
                found: format!("{}", self.current_token),
                position: self.position,
            })
        }
    }

    /// Parses expressions.
    fn parse_expression(&mut self) -> ParseResult<Expr> {
        self.parse_or_expression()
    }

    /// Parses OR expressions.
    fn parse_or_expression(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_and_expression()?;

        while self.current_token == Token::Or {
            self.advance()?;
            let right = self.parse_and_expression()?;
            left = Expr::Binary {
                left: Box::new(left),
                operator: BinaryOp::Or,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parses AND expressions.
    fn parse_and_expression(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_equality_expression()?;

        while self.current_token == Token::And {
            self.advance()?;
            let right = self.parse_equality_expression()?;
            left = Expr::Binary {
                left: Box::new(left),
                operator: BinaryOp::And,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parses equality expressions.
    fn parse_equality_expression(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_comparison_expression()?;

        while matches!(self.current_token, Token::Equal | Token::NotEqual) {
            let operator = match self.current_token {
                Token::Equal => BinaryOp::Equal,
                Token::NotEqual => BinaryOp::NotEqual,
                _ => unreachable!(),
            };
            self.advance()?;
            let right = self.parse_comparison_expression()?;
            left = Expr::Binary {
                left: Box::new(left),
                operator,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parses comparison expressions.
    fn parse_comparison_expression(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_additive_expression()?;

        while matches!(
            self.current_token,
            Token::LessThan
                | Token::LessThanOrEqual
                | Token::GreaterThan
                | Token::GreaterThanOrEqual
        ) {
            let operator = match self.current_token {
                Token::LessThan => BinaryOp::LessThan,
                Token::LessThanOrEqual => BinaryOp::LessThanOrEqual,
                Token::GreaterThan => BinaryOp::GreaterThan,
                Token::GreaterThanOrEqual => BinaryOp::GreaterThanOrEqual,
                _ => unreachable!(),
            };
            self.advance()?;
            let right = self.parse_additive_expression()?;
            left = Expr::Binary {
                left: Box::new(left),
                operator,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parses addition/subtraction expressions.
    fn parse_additive_expression(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_multiplicative_expression()?;

        while matches!(self.current_token, Token::Plus | Token::Minus) {
            let operator = match self.current_token {
                Token::Plus => BinaryOp::Plus,
                Token::Minus => BinaryOp::Minus,
                _ => unreachable!(),
            };
            self.advance()?;
            let right = self.parse_multiplicative_expression()?;
            left = Expr::Binary {
                left: Box::new(left),
                operator,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parses multiplication/division expressions.
    fn parse_multiplicative_expression(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_primary_expression()?;

        while matches!(self.current_token, Token::Multiply | Token::Divide) {
            let operator = match self.current_token {
                Token::Multiply => BinaryOp::Multiply,
                Token::Divide => BinaryOp::Divide,
                _ => unreachable!(),
            };
            self.advance()?;
            let right = self.parse_primary_expression()?;
            left = Expr::Binary {
                left: Box::new(left),
                operator,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parses primary expressions.
    fn parse_primary_expression(&mut self) -> ParseResult<Expr> {
        match &self.current_token {
            Token::Identifier(name) => {
                let name = name.clone();
                self.advance()?;

                // Check for function call
                if self.current_token == Token::LeftParen {
                    self.advance()?; // Skip (

                    let mut args = Vec::new();
                    if self.current_token != Token::RightParen {
                        args.push(self.parse_expression()?);

                        while self.current_token == Token::Comma {
                            self.advance()?; // Skip ,
                            args.push(self.parse_expression()?);
                        }
                    }

                    self.expect_token(Token::RightParen)?;
                    Ok(Expr::Function { name, args })
                } else {
                    Ok(Expr::Identifier(name))
                }
            }
            Token::String(s) => {
                let s = s.clone();
                self.advance()?;
                Ok(Expr::Literal(LiteralValue::String(s)))
            }
            Token::Number(n) => {
                let n = *n;
                self.advance()?;
                Ok(Expr::Literal(LiteralValue::Number(n)))
            }
            Token::Boolean(b) => {
                let b = *b;
                self.advance()?;
                Ok(Expr::Literal(LiteralValue::Boolean(b)))
            }
            Token::Null => {
                self.advance()?;
                Ok(Expr::Literal(LiteralValue::Null))
            }
            Token::LeftParen => {
                self.advance()?; // Skip (
                let expr = self.parse_expression()?;
                self.expect_token(Token::RightParen)?;
                Ok(expr)
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: "expression".to_string(),
                found: format!("{}", self.current_token),
                position: self.position,
            }),
        }
    }
}

#[cfg(test)]
#[path = "tests/parse_tests.rs"]
mod tests;
