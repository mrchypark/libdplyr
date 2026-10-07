//! Parser module
//!
//! Provides functionality to convert tokens to AST (Abstract Syntax Tree).

use crate::error::{ParseError, ParseResult};
use crate::lexer::{Lexer, Token};
use crate::PipeSyntax;

pub use super::ast::*;

/// Maximum nesting depth allowed inside a single expression.
///
/// Bounds both the recursive descent in `parse_expression` and the depth of
/// the resulting AST. Breadth is not bounded: any number of function arguments,
/// pipeline steps, or `case_when()` branches is fine. A chain of binary
/// operators such as `a + b + c + ...` is *not* breadth, though. It parses in a
/// loop but still builds a left-deep tree, so each operator loop counts its own
/// chain and rejects an over-long one before the tree is built.
///
/// The conservative bound also limits stack use in unoptimized recursive parsing.
pub const MAX_EXPRESSION_DEPTH: usize = 64;

fn depth_exceeded(position: usize) -> ParseError {
    ParseError::InvalidExpression {
        expr: format!("expression nesting depth exceeds {MAX_EXPRESSION_DEPTH}"),
        position,
    }
}

/// Returns the `(function, column)` pair when `expr` is exactly the shape an
/// [`Aggregation`] can hold: `function()` or `function(identifier)`.
///
/// A compound argument such as `sum(x * y)`, a scalar wrapping an aggregate
/// such as `round(sum(x), 2)`, or anything nested is deliberately rejected so
/// it stays on the expression path.
fn simple_aggregation(expr: &Expr) -> Option<(String, String)> {
    let Expr::Function { name, args } = expr else {
        return None;
    };
    match args.as_slice() {
        [] => Some((name.clone(), String::new())),
        [Expr::Identifier(column)] => Some((name.clone(), column.clone())),
        _ => None,
    }
}

/// Parser struct
///
/// Provides functionality to parse dplyr tokens into an Abstract Syntax Tree (AST).
pub struct Parser {
    lexer: Lexer,
    pipe_syntax: PipeSyntax,
    lazy_input_context: Option<LazyInput>,
    lazy_input_consumed: bool,
    current_token: Token,
    position: usize,
    line: usize,
    column: usize,
    expression_depth: usize,
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
        let pipe_syntax = lexer.pipe_syntax();
        let current_token = lexer.next_token()?;
        Ok(Self {
            lexer,
            pipe_syntax,
            lazy_input_context: None,
            lazy_input_consumed: false,
            current_token,
            position: 0,
            line: 1,
            column: 1,
            expression_depth: 0,
        })
    }

    /// Parses dplyr code to generate an AST.
    ///
    /// # Returns
    ///
    /// Returns DplyrNode on success, ParseError on failure.
    pub fn parse(&mut self) -> ParseResult<DplyrNode> {
        let node = self.parse_pipeline()?;
        self.skip_newlines()?;
        if self.current_token != Token::EOF {
            return Err(ParseError::UnexpectedToken {
                expected: "end of input".to_string(),
                found: format!("{}", self.current_token),
                position: self.position,
            });
        }
        validate_ast_depth(&node, self.position)?;
        Ok(node)
    }

    /// Returns the current source location.
    const fn current_location(&self) -> SourceLocation {
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

    fn expect_identifier_name(&mut self, expected_name: &str) -> ParseResult<()> {
        match &self.current_token {
            Token::Identifier(name) if name == expected_name => self.advance(),
            _ => Err(ParseError::UnexpectedToken {
                expected: expected_name.to_string(),
                found: format!("{}", self.current_token),
                position: self.position,
            }),
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

            // If followed by pipe operator, this is a data source with pipeline
            if self.current_token == Token::Pipe {
                // This is a data source followed by operations
                self.advance()?; // Skip %>%
                self.skip_newlines()?; // Skip newlines after pipe

                operations.extend(self.parse_pipeline_step()?);

                // Parse additional operations connected by pipe operators
                while self.current_token == Token::Pipe {
                    self.advance()?; // Skip %>%
                    self.skip_newlines()?; // Skip newlines after pipe
                    operations.extend(self.parse_pipeline_step()?);
                }

                // Skip trailing newlines
                self.skip_newlines()?;

                // Check for arrow operators (-> or <-) for table assignment
                let target = if self.current_token == Token::ArrowRight
                    || self.current_token == Token::ArrowLeft
                {
                    self.advance()?;
                    self.skip_newlines()?;
                    match &self.current_token {
                        Token::Identifier(target_name) => {
                            let name = target_name.clone();
                            self.advance()?;
                            Some(name)
                        }
                        _ => {
                            return Err(ParseError::UnexpectedToken {
                                expected: "target table name".to_string(),
                                found: format!("{}", self.current_token),
                                position: self.position,
                            });
                        }
                    }
                } else {
                    None
                };

                return Ok(DplyrNode::Pipeline {
                    source: Some(name),
                    target,
                    operations,
                    location: start_location,
                });
            } else if self.current_token == Token::ArrowRight
                || self.current_token == Token::ArrowLeft
            {
                // Direct assignment: iris -> iris2 or iris2 <- iris
                // For ->: source is left side, target is right side
                // For <-: source is right side, target is left side
                let (source, target) = if self.current_token == Token::ArrowRight {
                    // iris -> iris2
                    self.advance()?;
                    self.skip_newlines()?;
                    match &self.current_token {
                        Token::Identifier(target_name) => {
                            let target = target_name.clone();
                            self.advance()?;
                            (Some(name), Some(target))
                        }
                        _ => {
                            return Err(ParseError::UnexpectedToken {
                                expected: "target table name".to_string(),
                                found: format!("{}", self.current_token),
                                position: self.position,
                            });
                        }
                    }
                } else {
                    // iris2 <- iris
                    // For <-, the right side is the source and may contain a pipeline
                    let target = name;

                    // Save current position after <-
                    self.advance()?;
                    self.skip_newlines()?;

                    if let Token::Identifier(source_name) = &self.current_token {
                        let source = source_name.clone();
                        self.advance()?;
                        self.skip_newlines()?;

                        if self.current_token == Token::Pipe {
                            // Pipeline on the right side
                            self.advance()?;
                            self.skip_newlines()?;

                            operations.extend(self.parse_pipeline_step()?);

                            // Parse additional operations
                            while self.current_token == Token::Pipe {
                                self.advance()?;
                                self.skip_newlines()?;
                                operations.extend(self.parse_pipeline_step()?);
                            }
                        }

                        (Some(source), Some(target))
                    } else {
                        return Err(ParseError::UnexpectedToken {
                            expected: "source table name".to_string(),
                            found: format!("{}", self.current_token),
                            position: self.position,
                        });
                    }
                };

                return Ok(DplyrNode::Pipeline {
                    source,
                    target,
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
        operations.extend(self.parse_pipeline_step()?);

        // Parse additional operations connected by pipe operators
        while self.current_token == Token::Pipe {
            self.advance()?; // Skip %>%
            self.skip_newlines()?; // Skip newlines after pipe
            operations.extend(self.parse_pipeline_step()?);
        }

        // Skip trailing newlines
        self.skip_newlines()?;

        // Check for arrow operators (-> or <-) for table assignment
        let target =
            if self.current_token == Token::ArrowRight || self.current_token == Token::ArrowLeft {
                self.advance()?; // Skip -> or <-
                self.skip_newlines()?;
                // Parse target table name
                match &self.current_token {
                    Token::Identifier(name) => {
                        let target_name = name.clone();
                        self.advance()?;
                        Some(target_name)
                    }
                    _ => {
                        return Err(ParseError::UnexpectedToken {
                            expected: "target table name".to_string(),
                            found: format!("{}", self.current_token),
                            position: self.position,
                        });
                    }
                }
            } else {
                None
            };

        Ok(DplyrNode::Pipeline {
            source: None,
            target,
            operations,
            location: start_location,
        })
    }

    /// Parses one pipeline step. A native-pipe lambda RHS like
    /// `(\(x) x |> select(col))()` is normalized to the operations in its body.
    fn parse_pipeline_step(&mut self) -> ParseResult<Vec<DplyrOperation>> {
        match (&self.pipe_syntax, &self.current_token) {
            (PipeSyntax::Native, Token::LeftParen) => {
                self.parse_native_lambda_pipeline_application()
            }
            (PipeSyntax::Magrittr, Token::LeftBrace) => {
                self.parse_magrittr_lambda_pipeline_application(Token::LeftBrace, Token::RightBrace)
            }
            (PipeSyntax::Magrittr, Token::LeftParen) => {
                self.parse_magrittr_lambda_pipeline_application(Token::LeftParen, Token::RightParen)
            }
            (PipeSyntax::Magrittr, _) => {
                Ok(vec![self.parse_operation_with_lazy_input(
                    LazyInput::MagrittrDot,
                    false,
                )?])
            }
            _ => Ok(vec![self.parse_operation()?]),
        }
    }

    fn parse_operation_with_lazy_input(
        &mut self,
        input: LazyInput,
        require_input: bool,
    ) -> ParseResult<DplyrOperation> {
        let previous_context = self.lazy_input_context.clone();
        let previous_consumed = self.lazy_input_consumed;

        self.lazy_input_context = Some(input);
        self.lazy_input_consumed = false;

        let result = self.parse_operation();
        let consumed = self.lazy_input_consumed;

        self.lazy_input_context = previous_context;
        self.lazy_input_consumed = previous_consumed;

        let operation = result?;
        if require_input && !consumed {
            return Err(ParseError::InvalidOperation {
                operation: "lambda body must consume the piped data argument".to_string(),
                position: self.position,
            });
        }

        Ok(operation)
    }

    fn consume_optional_lazy_data_argument(&mut self) -> ParseResult<()> {
        let should_consume = match &self.lazy_input_context {
            Some(LazyInput::MagrittrDot) => self.current_token == Token::Dot,
            Some(LazyInput::NativeParameter(param)) => {
                if let Token::Identifier(name) = &self.current_token {
                    name == param && matches!(self.peek_token()?, Token::Comma | Token::RightParen)
                } else {
                    false
                }
            }
            None => false,
        };

        if !should_consume {
            return Ok(());
        }

        self.advance()?;
        self.lazy_input_consumed = true;

        if self.current_token == Token::Comma {
            self.advance()?;
        } else if self.current_token != Token::RightParen {
            return Err(ParseError::UnexpectedToken {
                expected: "comma or closing parenthesis after lambda data argument".to_string(),
                found: format!("{}", self.current_token),
                position: self.position,
            });
        }

        Ok(())
    }

    fn peek_token(&mut self) -> ParseResult<Token> {
        Ok(self.lexer.peek_token()?)
    }

    fn parse_magrittr_lambda_pipeline_application(
        &mut self,
        opening: Token,
        closing: Token,
    ) -> ParseResult<Vec<DplyrOperation>> {
        self.expect_token(opening)?;
        self.skip_newlines()?;

        let mut operations = if self.current_token == Token::Dot {
            self.expect_token(Token::Dot)?;
            self.skip_newlines()?;
            self.expect_token(Token::Pipe)?;
            self.skip_newlines()?;
            vec![self.parse_operation_with_lazy_input(LazyInput::MagrittrDot, false)?]
        } else {
            vec![self.parse_operation_with_lazy_input(LazyInput::MagrittrDot, true)?]
        };

        while self.current_token == Token::Pipe {
            self.advance()?;
            self.skip_newlines()?;
            operations.push(self.parse_operation_with_lazy_input(LazyInput::MagrittrDot, false)?);
        }

        self.skip_newlines()?;
        self.expect_token(closing)?;

        Ok(operations)
    }

    fn parse_native_lambda_pipeline_application(&mut self) -> ParseResult<Vec<DplyrOperation>> {
        self.expect_token(Token::LeftParen)?;
        self.expect_token(Token::Backslash)?;
        self.expect_token(Token::LeftParen)?;

        let param = match &self.current_token {
            Token::Identifier(name) => {
                let name = name.clone();
                self.advance()?;
                name
            }
            _ => {
                return Err(ParseError::UnexpectedToken {
                    expected: "lambda parameter name".to_string(),
                    found: format!("{}", self.current_token),
                    position: self.position,
                });
            }
        };

        self.expect_token(Token::RightParen)?;
        self.skip_newlines()?;

        let mut operations = if let Token::Identifier(body_source) = &self.current_token {
            if body_source == &param {
                self.advance()?;
                self.skip_newlines()?;
                self.expect_token(Token::Pipe)?;
                self.skip_newlines()?;
                vec![self.parse_operation()?]
            } else {
                vec![self.parse_operation_with_lazy_input(
                    LazyInput::NativeParameter(param.clone()),
                    true,
                )?]
            }
        } else {
            vec![self
                .parse_operation_with_lazy_input(LazyInput::NativeParameter(param.clone()), true)?]
        };

        while self.current_token == Token::Pipe {
            self.advance()?;
            self.skip_newlines()?;
            operations.push(self.parse_operation_with_lazy_input(
                LazyInput::NativeParameter(param.clone()),
                false,
            )?);
        }

        self.skip_newlines()?;
        self.expect_token(Token::RightParen)?;
        self.expect_token(Token::LeftParen)?;
        self.expect_token(Token::RightParen)?;

        Ok(operations)
    }

    /// Parses individual dplyr operations.
    fn parse_operation(&mut self) -> ParseResult<DplyrOperation> {
        match &self.current_token {
            Token::Select => self.parse_select(),
            Token::Distinct => self.parse_distinct(),
            Token::Filter => self.parse_filter(),
            Token::Mutate => self.parse_mutate(),
            Token::Rename => self.parse_rename(),
            Token::Arrange => self.parse_arrange(),
            Token::GroupBy => self.parse_group_by(),
            Token::Summarise => self.parse_summarise(),
            Token::Identifier(name) if name == "count" => self.parse_count(),
            Token::Identifier(name) if name == "tally" => self.parse_tally(),
            Token::InnerJoin
            | Token::LeftJoin
            | Token::RightJoin
            | Token::FullJoin
            | Token::SemiJoin
            | Token::AntiJoin => self.parse_join(),
            Token::Intersect => self.parse_set_op(SetOperation::Intersect),
            Token::Union => self.parse_set_op(SetOperation::Union),
            Token::SetDiff => self.parse_set_op(SetOperation::SetDiff),
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
        self.consume_optional_lazy_data_argument()?;

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

    /// Parses the portable subset of distinct(): no arguments or identifiers only.
    fn parse_distinct(&mut self) -> ParseResult<DplyrOperation> {
        let location = self.current_location();
        self.advance()?; // Skip 'distinct'
        self.expect_token(Token::LeftParen)?;
        self.consume_optional_lazy_data_argument()?;

        let mut columns = Vec::new();
        if self.current_token != Token::RightParen {
            let Token::Identifier(column) = &self.current_token else {
                return Err(ParseError::UnexpectedToken {
                    expected: "column identifier".to_string(),
                    found: format!("{}", self.current_token),
                    position: self.position,
                });
            };
            columns.push(column.clone());
            self.advance()?;
            while self.current_token == Token::Comma {
                self.advance()?;
                let Token::Identifier(column) = &self.current_token else {
                    return Err(ParseError::UnexpectedToken {
                        expected: "column identifier".to_string(),
                        found: format!("{}", self.current_token),
                        position: self.position,
                    });
                };
                columns.push(column.clone());
                self.advance()?;
            }
        }

        self.expect_token(Token::RightParen)?;
        Ok(DplyrOperation::Distinct { columns, location })
    }

    /// Parses filter() operation.
    fn parse_filter(&mut self) -> ParseResult<DplyrOperation> {
        let location = self.current_location();
        self.advance()?; // Skip 'filter'
        self.expect_token(Token::LeftParen)?;
        self.consume_optional_lazy_data_argument()?;

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
        self.consume_optional_lazy_data_argument()?;

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
        self.consume_optional_lazy_data_argument()?;

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
        self.consume_optional_lazy_data_argument()?;

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
        self.consume_optional_lazy_data_argument()?;

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
        self.consume_optional_lazy_data_argument()?;

        // `function(identifier)` and `function()` keep the compact `Aggregation`
        // shape; anything else is an expression. The choice is per entry, and a
        // single expression entry promotes the whole list, so order and aliases
        // are preserved by reparsing the simple entries as assignments.
        let mut entries: Vec<SummariseEntry> = Vec::new();

        if self.current_token != Token::RightParen {
            loop {
                entries.push(self.parse_summarise_entry()?);
                if self.current_token != Token::Comma {
                    break;
                }
                self.advance()?;
                self.skip_newlines()?;
            }
        }

        self.expect_token(Token::RightParen)?;

        if entries
            .iter()
            .any(|entry| matches!(entry, SummariseEntry::Expression(_)))
        {
            Ok(DplyrOperation::SummariseExpressions {
                assignments: entries
                    .into_iter()
                    .map(|entry| match entry {
                        SummariseEntry::Aggregation(aggregation) => Assignment {
                            column: aggregation.alias.unwrap_or_else(|| {
                                format!("{}({})", aggregation.function, aggregation.column)
                            }),
                            expr: if aggregation.column.is_empty() {
                                Expr::Function {
                                    name: aggregation.function,
                                    args: Vec::new(),
                                }
                            } else {
                                Expr::Function {
                                    name: aggregation.function,
                                    args: vec![Expr::Identifier(aggregation.column)],
                                }
                            },
                        },
                        SummariseEntry::Expression(assignment) => assignment,
                    })
                    .collect(),
                location,
            })
        } else {
            Ok(DplyrOperation::Summarise {
                aggregations: entries
                    .into_iter()
                    .map(|entry| match entry {
                        SummariseEntry::Aggregation(aggregation) => aggregation,
                        SummariseEntry::Expression(_) => unreachable!("checked above"),
                    })
                    .collect(),
                location,
            })
        }
    }

    /// Parses one `summarise()` entry, choosing the narrower `Aggregation`
    /// shape only when the entry really is `function(identifier)` or `function()`.
    ///
    /// Every entry is parsed as a general expression first, then classified by
    /// its shape. Deciding before consuming would need multi-token lookahead to
    /// tell `mean(age)` from `mean(age * y)`, and classifying afterwards gets
    /// the same answer from the tree that was built anyway.
    fn parse_summarise_entry(&mut self) -> ParseResult<SummariseEntry> {
        let alias = match self.current_token.clone() {
            Token::Identifier(name) if self.peek_token()? == Token::Assignment => {
                self.advance()?; // identifier
                self.advance()?; // =
                self.skip_newlines()?;
                Some(name)
            }
            _ => None,
        };

        let expr = self.parse_expression()?;
        match (alias, simple_aggregation(&expr)) {
            (alias, Some((function, column))) => Ok(SummariseEntry::Aggregation(Aggregation {
                function,
                column,
                alias,
            })),
            (Some(alias), None) => Ok(SummariseEntry::Expression(Assignment {
                column: alias,
                expr,
            })),
            (None, None) => Ok(SummariseEntry::Expression(Assignment {
                column: expr.to_string(),
                expr,
            })),
        }
    }

    /// Parses the identifier-only subset of count().
    fn parse_count(&mut self) -> ParseResult<DplyrOperation> {
        let location = self.current_location();
        self.advance()?;
        self.expect_token(Token::LeftParen)?;
        self.consume_optional_lazy_data_argument()?;

        let mut columns = Vec::new();
        if self.current_token != Token::RightParen {
            let Token::Identifier(column) = &self.current_token else {
                return Err(ParseError::UnexpectedToken {
                    expected: "column identifier".to_string(),
                    found: format!("{}", self.current_token),
                    position: self.position,
                });
            };
            columns.push(column.clone());
            self.advance()?;
            while self.current_token == Token::Comma {
                self.advance()?;
                let Token::Identifier(column) = &self.current_token else {
                    return Err(ParseError::UnexpectedToken {
                        expected: "column identifier".to_string(),
                        found: format!("{}", self.current_token),
                        position: self.position,
                    });
                };
                columns.push(column.clone());
                self.advance()?;
            }
        }

        self.expect_token(Token::RightParen)?;
        Ok(DplyrOperation::Count { columns, location })
    }

    /// Parses the unweighted, argument-free subset of tally().
    fn parse_tally(&mut self) -> ParseResult<DplyrOperation> {
        let location = self.current_location();
        self.advance()?;
        self.expect_token(Token::LeftParen)?;
        self.consume_optional_lazy_data_argument()?;
        self.expect_token(Token::RightParen)?;
        Ok(DplyrOperation::Count {
            columns: Vec::new(),
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
        self.consume_optional_lazy_data_argument()?;

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
        self.expect_identifier_name("by")?;
        self.expect_token(Token::Assignment)?;

        // Parse by parameter - handle string literal as column name
        let (by, on_expr) = match &self.current_token {
            Token::String(s) => {
                // by = "column_name" - simple join on same column name
                let col_name = s.clone();
                self.advance()?;
                (
                    vec![JoinKey {
                        left: col_name.clone(),
                        right: col_name,
                    }],
                    None,
                )
            }
            Token::Identifier(name) if name == "c" => (self.parse_join_key_vector()?, None),
            Token::Identifier(_) => {
                // Could be a column reference or complex expression
                // For now, parse as expression
                let expr = self.parse_expression()?;
                (Vec::new(), Some(expr))
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
                by,
                on_expr,
            },
            location,
        })
    }

    fn parse_join_key_vector(&mut self) -> ParseResult<Vec<JoinKey>> {
        self.advance()?; // Skip c
        self.expect_token(Token::LeftParen)?;

        let mut keys = Vec::new();
        loop {
            let left = match &self.current_token {
                Token::String(name) => name.clone(),
                _ => {
                    return Err(ParseError::UnexpectedToken {
                        expected: "non-empty string join key vector".to_string(),
                        found: format!("{}", self.current_token),
                        position: self.position,
                    })
                }
            };
            self.advance()?;

            let right = if self.current_token == Token::Assignment {
                self.advance()?;
                let Token::String(name) = &self.current_token else {
                    return Err(ParseError::UnexpectedToken {
                        expected: "string join key".to_string(),
                        found: format!("{}", self.current_token),
                        position: self.position,
                    });
                };
                let name = name.clone();
                self.advance()?;
                name
            } else {
                left.clone()
            };
            keys.push(JoinKey { left, right });

            if self.current_token != Token::Comma {
                break;
            }
            self.advance()?;
        }

        self.expect_token(Token::RightParen)?;
        Ok(keys)
    }

    /// Parses set operations (intersect, union, setdiff).
    fn parse_set_op(&mut self, operation: SetOperation) -> ParseResult<DplyrOperation> {
        let location = self.current_location();
        self.advance()?; // Skip function name
        self.expect_token(Token::LeftParen)?;
        self.consume_optional_lazy_data_argument()?;

        // Parse table name
        let right_table = match &self.current_token {
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

        self.expect_token(Token::RightParen)?;

        Ok(DplyrOperation::SetOp {
            operation,
            right_table,
            location,
        })
    }

    /// Parses column expressions.
    fn parse_column_expr(&mut self) -> ParseResult<ColumnExpr> {
        if self.current_token == Token::Multiply {
            self.advance()?;
            return Ok(ColumnExpr {
                expr: Expr::Identifier("*".to_string()),
                alias: None,
            });
        }

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
                    args.push(self.parse_function_argument()?);

                    while self.current_token == Token::Comma {
                        self.advance()?; // Skip ,
                        args.push(self.parse_function_argument()?);
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

    /// Parses expressions.
    fn parse_expression(&mut self) -> ParseResult<Expr> {
        self.enter_expression()?;
        self.expression_depth += 1;
        let expr = self.parse_or_expression();
        self.expression_depth -= 1;
        expr
    }

    fn enter_expression(&self) -> ParseResult<()> {
        if self.expression_depth >= MAX_EXPRESSION_DEPTH {
            return Err(depth_exceeded(self.position));
        }
        Ok(())
    }

    /// Rejects an over-long left-deep operator chain as it is built.
    ///
    /// The loop below runs once per operator, so `a + b + c + ...` never
    /// recurses and `expression_depth` never grows. The tree it produces is
    /// still `chain` levels deep, so the bound is checked here instead of being
    /// left to the post-parse AST walk: by then a chain of unbounded length has
    /// already been allocated, and dropping that tree is itself a deep drop.
    fn check_chain_depth(&self, chain: usize) -> ParseResult<()> {
        if self.expression_depth + chain >= MAX_EXPRESSION_DEPTH {
            return Err(depth_exceeded(self.position));
        }
        Ok(())
    }

    /// Parses OR expressions.
    fn parse_or_expression(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_and_expression()?;
        let mut chain = 0usize;

        while self.current_token == Token::Or {
            self.advance()?;
            let right = self.parse_and_expression()?;
            chain += 1;
            self.check_chain_depth(chain)?;
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
        let mut chain = 0usize;

        while self.current_token == Token::And {
            self.advance()?;
            let right = self.parse_equality_expression()?;
            chain += 1;
            self.check_chain_depth(chain)?;
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
        let mut chain = 0usize;

        while matches!(self.current_token, Token::Equal | Token::NotEqual) {
            let operator = match self.current_token {
                Token::Equal => BinaryOp::Equal,
                Token::NotEqual => BinaryOp::NotEqual,
                _ => unreachable!(),
            };
            self.advance()?;
            let right = self.parse_comparison_expression()?;
            chain += 1;
            self.check_chain_depth(chain)?;
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
        let mut chain = 0usize;

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
            chain += 1;
            self.check_chain_depth(chain)?;
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
        let mut chain = 0usize;

        while matches!(self.current_token, Token::Plus | Token::Minus) {
            let operator = match self.current_token {
                Token::Plus => BinaryOp::Plus,
                Token::Minus => BinaryOp::Minus,
                _ => unreachable!(),
            };
            self.advance()?;
            let right = self.parse_multiplicative_expression()?;
            chain += 1;
            self.check_chain_depth(chain)?;
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
        let mut chain = 0usize;

        while matches!(self.current_token, Token::Multiply | Token::Divide) {
            let operator = match self.current_token {
                Token::Multiply => BinaryOp::Multiply,
                Token::Divide => BinaryOp::Divide,
                _ => unreachable!(),
            };
            self.advance()?;
            let right = self.parse_primary_expression()?;
            chain += 1;
            self.check_chain_depth(chain)?;
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

                    if name == "case_when" {
                        return self.parse_case_when();
                    }

                    let mut args = Vec::new();
                    if self.current_token != Token::RightParen {
                        args.push(self.parse_function_argument()?);

                        while self.current_token == Token::Comma {
                            self.advance()?; // Skip ,
                            args.push(self.parse_function_argument()?);
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

    fn parse_case_when(&mut self) -> ParseResult<Expr> {
        let mut branches = Vec::new();
        let mut default = None;

        while self.current_token != Token::RightParen {
            if self.current_token == Token::Dot {
                self.advance()?;
                self.expect_identifier_name("default")?;
                self.expect_token(Token::Assignment)?;
                default = Some(Box::new(self.parse_expression()?));
                break;
            }

            let condition = self.parse_expression()?;
            self.expect_token(Token::Tilde)?;
            let value = self.parse_expression()?;
            branches.push((condition, value));

            if self.current_token != Token::Comma {
                break;
            }
            self.advance()?;
        }

        if branches.is_empty() {
            return Err(ParseError::UnexpectedToken {
                expected: "at least one case_when condition ~ value formula".to_string(),
                found: format!("{}", self.current_token),
                position: self.position,
            });
        }

        self.expect_token(Token::RightParen)?;
        Ok(Expr::CaseWhen { branches, default })
    }

    fn parse_function_argument(&mut self) -> ParseResult<Expr> {
        let expr = self.parse_expression()?;
        if self.current_token != Token::Assignment {
            return Ok(expr);
        }

        let name = match expr {
            Expr::Identifier(name) => name,
            Expr::Literal(LiteralValue::Boolean(true)) => "true".to_string(),
            Expr::Literal(LiteralValue::Boolean(false)) => "false".to_string(),
            _ => {
                return Err(ParseError::UnexpectedToken {
                    expected: "named argument identifier".to_string(),
                    found: format!("{}", self.current_token),
                    position: self.position,
                });
            }
        };

        self.advance()?; // Skip =
        let value = self.parse_expression()?;
        Ok(Expr::NamedArg {
            name,
            value: Box::new(value),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum LazyInput {
    MagrittrDot,
    NativeParameter(String),
}

/// Iteratively rejects an AST whose expressions nest deeper than the bound.
///
/// Left-deep chains such as `a + b + c + ...` parse in a loop, so the
/// recursive descent counter never sees them. Their AST is still deep, and the
/// SQL generator and the relational binder walk it recursively, so the shape is
/// checked here with an explicit stack. Bounding the tree once, at parse time,
/// covers consumers of parsed expressions.
fn validate_ast_depth(node: &DplyrNode, position: usize) -> ParseResult<()> {
    let mut roots = Vec::new();
    if let DplyrNode::Pipeline { operations, .. } = node {
        for operation in operations {
            match operation {
                DplyrOperation::Select { columns, .. } => {
                    roots.extend(columns.iter().map(|column| &column.expr));
                }
                DplyrOperation::Filter { condition, .. } => roots.push(condition),
                DplyrOperation::Mutate { assignments, .. } => {
                    roots.extend(assignments.iter().map(|assignment| &assignment.expr));
                }
                DplyrOperation::SummariseExpressions { assignments, .. } => {
                    roots.extend(assignments.iter().map(|assignment| &assignment.expr));
                }
                DplyrOperation::Join { spec, .. } => {
                    if let Some(on_expr) = &spec.on_expr {
                        roots.push(on_expr);
                    }
                }
                _ => {}
            }
        }
    }

    let mut stack: Vec<(&Expr, usize)> = roots.into_iter().map(|expr| (expr, 1)).collect();
    while let Some((expr, depth)) = stack.pop() {
        if depth > MAX_EXPRESSION_DEPTH {
            return Err(depth_exceeded(position));
        }
        match expr {
            Expr::Binary { left, right, .. } => {
                stack.push((left, depth + 1));
                stack.push((right, depth + 1));
            }
            Expr::Function { args, .. } => {
                stack.extend(args.iter().map(|arg| (arg, depth + 1)));
            }
            Expr::CaseWhen { branches, default } => {
                for (condition, value) in branches {
                    stack.push((condition, depth + 1));
                    stack.push((value, depth + 1));
                }
                if let Some(default) = default {
                    stack.push((default, depth + 1));
                }
            }
            Expr::NamedArg { value, .. } => stack.push((value, depth + 1)),
            Expr::Identifier(_) | Expr::Literal(_) => {}
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/parse_tests.rs"]
mod tests;
