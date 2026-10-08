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
    // R3-AC1: across() is never an aggregation. Its arguments are selectors and
    // lambdas, and the expander needs the whole call plus the empty-column
    // sentinel that only the expression shape carries.
    if name == "across" {
        return None;
    }
    match args.as_slice() {
        [] => Some((name.clone(), String::new())),
        [Expr::Identifier(column)] => Some((name.clone(), column.clone())),
        _ => None,
    }
}

fn is_across(expr: &Expr) -> bool {
    matches!(expr, Expr::Function { name, .. } if name == "across")
}

fn is_extended_verb(name: &str) -> bool {
    matches!(
        name,
        "union_all"
            | "bind_queries"
            | "add_count"
            | "add_tally"
            | "head"
            | "tail"
            | "relocate"
            | "rename_with"
            | "window_order"
            | "window_frame"
            | "cross_join"
            | "filter_out"
            | "transmute"
            | "pivot_longer"
            | "pivot_wider"
            | "fill"
            | "expand"
            | "complete"
            | "dbplyr_uncount"
            | "rows_append"
            | "rows_insert"
            | "rows_update"
            | "rows_patch"
            | "rows_upsert"
            | "rows_delete"
            | "slice_head"
            | "slice_tail"
            | "slice"
            | "replace_na"
    )
}

fn argument_error(verb: &str, reason: String) -> ParseError {
    ParseError::InvalidOperation {
        operation: format!("{verb}() {reason}"),
        position: 0,
    }
}

/// Returns true if any argument is a named option starting with '.'.
fn has_advanced_options(args: &[Expr]) -> bool {
    args.iter()
        .any(|arg| matches!(arg, Expr::NamedArg { name, .. } if name.starts_with('.')))
}

/// Returns true if any argument is a non-identifier expression (computed).
fn has_computed(args: &[Expr]) -> bool {
    args.iter().any(|arg| !matches!(arg, Expr::Identifier(_)))
}

/// Signed number argument. `n` additionally has to be a whole number.
fn number_argument(expr: &Expr, verb: &str, name: &str) -> ParseResult<f64> {
    let signed = match expr {
        Expr::Literal(LiteralValue::Number(value)) => *value,
        Expr::Unary {
            operator: UnaryOp::Minus,
            expr,
        } => match expr.as_ref() {
            Expr::Literal(LiteralValue::Number(value)) => -*value,
            _ => return Err(argument_error(verb, format!("{name} must be a number"))),
        },
        _ => return Err(argument_error(verb, format!("{name} must be a number"))),
    };
    if name == "n" {
        if signed < 0.0 {
            return Err(argument_error(verb, "n must be nonnegative".to_string()));
        }
        return Ok(signed);
    }
    // A share above one is legal: it truncates the group rather than failing.
    if !signed.is_finite() || signed < 0.0 {
        return Err(argument_error(
            verb,
            "prop must be a finite nonnegative number".to_string(),
        ));
    }
    Ok(signed)
}

fn boolean_argument(expr: &Expr, verb: &str, name: &str) -> ParseResult<bool> {
    match expr {
        Expr::Literal(LiteralValue::Boolean(value)) => Ok(*value),
        _ => Err(argument_error(
            verb,
            format!("{name} must be TRUE or FALSE"),
        )),
    }
}

/// `by` keeps one ColumnExpr per name. `c(grp, sub)` is written as
/// `c(grp, sub = sub)`, so the named entries carry no alias.
fn slice_by_argument(expr: &Expr) -> ParseResult<Vec<ColumnExpr>> {
    let Expr::Function { name, args } = expr else {
        return Ok(vec![ColumnExpr {
            expr: expr.clone(),
            alias: None,
        }]);
    };
    if name != "c" && name != "list" {
        return Ok(vec![ColumnExpr {
            expr: expr.clone(),
            alias: None,
        }]);
    }
    Ok(args
        .iter()
        .map(|arg| ColumnExpr {
            expr: match arg {
                Expr::NamedArg { value, .. } => value.as_ref().clone(),
                other => other.clone(),
            },
            alias: None,
        })
        .collect())
}

/// Resolves a `by` value into explicit keys, or leaves it as a predicate the
/// planner extracts equalities from. A missing or NULL `by` yields no keys,
/// which the planner reads as the natural join.
fn join_by_argument(expr: &Expr, position: usize) -> ParseResult<(Vec<JoinKey>, Option<Expr>)> {
    if let Expr::Function { name, args } = expr {
        if name == "join_by" {
            fn key(expr: &Expr, keys: &mut Vec<JoinKey>, position: usize) -> ParseResult<()> {
                match expr {
                    Expr::Identifier(name) => keys.push(JoinKey {
                        left: name.clone(),
                        right: name.clone(),
                    }),
                    Expr::Binary {
                        left,
                        operator: BinaryOp::Equal,
                        right,
                    } => {
                        let (Expr::Identifier(left), Expr::Identifier(right)) =
                            (left.as_ref(), right.as_ref())
                        else {
                            return Err(argument_error(
                                "join_by",
                                "keys must be column identifiers".to_string(),
                            ));
                        };
                        keys.push(JoinKey {
                            left: left.clone(),
                            right: right.clone(),
                        });
                    }
                    Expr::Binary {
                        left,
                        operator: BinaryOp::And,
                        right,
                    } => {
                        key(left, keys, position)?;
                        key(right, keys, position)?;
                    }
                    _ => {
                        return Err(ParseError::InvalidOperation {
                            operation: "join_by() supports equality keys only".to_string(),
                            position,
                        })
                    }
                }
                Ok(())
            }
            if args.is_empty() {
                return Err(argument_error(
                    "join_by",
                    "requires at least one equality key".to_string(),
                ));
            }
            fn equality(expr: &Expr) -> bool {
                match expr {
                    Expr::Identifier(_) => true,
                    Expr::Binary {
                        operator: BinaryOp::Equal,
                        ..
                    } => true,
                    Expr::Binary {
                        left,
                        operator: BinaryOp::And,
                        right,
                    } => equality(left) && equality(right),
                    _ => false,
                }
            }
            if args.iter().any(|expr| !equality(expr)) {
                let predicate = args.iter().cloned().reduce(|left, right| Expr::Binary {
                    left: Box::new(left),
                    operator: BinaryOp::And,
                    right: Box::new(right),
                });
                return Ok((Vec::new(), predicate));
            }
            let mut keys = Vec::new();
            for expr in args {
                key(expr, &mut keys, position)?;
            }
            return Ok((keys, None));
        }
    }
    Ok(match expr {
        Expr::Literal(LiteralValue::Null) => (Vec::new(), None),
        Expr::Literal(LiteralValue::String(name)) => (
            vec![JoinKey {
                left: name.clone(),
                right: name.clone(),
            }],
            None,
        ),
        other => (Vec::new(), Some(other.clone())),
    })
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
    /// Nonzero while the arguments of an `across()` call are being parsed, so
    /// `~` is read as a lambda rather than a case_when() formula.
    across_depth: usize,
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
            across_depth: 0,
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
        if matches!(&self.current_token, Token::Identifier(name) if is_extended_verb(name))
            && self.peek_token()? == Token::LeftParen
        {
            let mut operations = self.parse_pipeline_step()?;
            while self.current_token == Token::Pipe {
                self.advance()?;
                operations.extend(self.parse_pipeline_step()?);
            }
            return Ok(DplyrNode::Pipeline {
                source: None,
                target: None,
                operations,
                location: start_location,
            });
        }
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
            Token::Identifier(name) if name == "ungroup" => {
                let location = self.current_location();
                self.advance()?;
                self.expect_token(Token::LeftParen)?;
                self.consume_optional_lazy_data_argument()?;
                let mut args = Vec::new();
                if self.current_token != Token::RightParen {
                    args.push(self.parse_function_argument()?);
                    while self.current_token == Token::Comma {
                        self.advance()?;
                        self.skip_newlines()?;
                        if self.current_token == Token::RightParen {
                            break;
                        }
                        args.push(self.parse_function_argument()?);
                    }
                }
                self.expect_token(Token::RightParen)?;
                if args.is_empty() {
                    Ok(DplyrOperation::Ungroup { location })
                } else {
                    Ok(DplyrOperation::Extended {
                        name: "ungroup".to_string(),
                        args,
                        location,
                    })
                }
            }
            Token::Identifier(name)
                if matches!(name.as_str(), "slice_min" | "slice_max" | "slice_sample") =>
            {
                let kind = match name.as_str() {
                    "slice_min" => SliceKind::Min,
                    "slice_max" => SliceKind::Max,
                    _ => SliceKind::Sample,
                };
                self.parse_slice(kind)
            }
            Token::Summarise => self.parse_summarise(),
            Token::Identifier(name) if name == "count" => self.parse_count(),
            Token::Identifier(name) if name == "tally" => self.parse_tally(),
            Token::Identifier(name) if is_extended_verb(name) => {
                let name = name.clone();
                self.parse_extended_verb(&name)
            }
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

        let mut args = Vec::new();
        if self.current_token != Token::RightParen {
            args.push(self.parse_function_argument()?);
            while self.current_token == Token::Comma {
                self.advance()?;
                self.skip_newlines()?;
                if self.current_token == Token::RightParen {
                    break;
                }
                args.push(self.parse_function_argument()?);
            }
        }

        self.expect_token(Token::RightParen)?;

        // Route to Extended if advanced options or computed expressions are present
        if has_advanced_options(&args) || has_computed(&args) {
            return Ok(DplyrOperation::Extended {
                name: "distinct".to_string(),
                args,
                location,
            });
        }

        // Convert to column names
        let mut columns = Vec::new();
        for arg in args {
            match arg {
                Expr::Identifier(name) => columns.push(name),
                _ => {
                    return Err(ParseError::UnexpectedToken {
                        expected: "column identifier".to_string(),
                        found: format!("{}", arg),
                        position: self.position,
                    })
                }
            }
        }
        Ok(DplyrOperation::Distinct { columns, location })
    }

    /// Parses filter() operation.
    fn parse_filter(&mut self) -> ParseResult<DplyrOperation> {
        let location = self.current_location();
        self.advance()?; // Skip 'filter'
        self.expect_token(Token::LeftParen)?;
        self.consume_optional_lazy_data_argument()?;

        let mut args = Vec::new();
        if self.current_token != Token::RightParen {
            args.push(self.parse_function_argument()?);
            while self.current_token == Token::Comma {
                self.advance()?;
                self.skip_newlines()?;
                if self.current_token == Token::RightParen {
                    break;
                }
                args.push(self.parse_function_argument()?);
            }
        }

        self.expect_token(Token::RightParen)?;

        // Route to Extended if advanced options are present
        if has_advanced_options(&args) || args.len() > 1 {
            return Ok(DplyrOperation::Extended {
                name: "filter".to_string(),
                args,
                location,
            });
        }

        let condition = args
            .into_iter()
            .next()
            .unwrap_or(Expr::Literal(LiteralValue::Boolean(true)));
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

        let mut args = Vec::new();

        // First assignment
        if self.current_token != Token::RightParen {
            args.push(self.parse_function_argument()?);

            // Additional assignments (comma-separated)
            while self.current_token == Token::Comma {
                self.advance()?; // Skip comma
                if self.current_token == Token::RightParen {
                    break;
                }
                args.push(self.parse_function_argument()?);
            }
        }

        self.expect_token(Token::RightParen)?;

        if has_advanced_options(&args) {
            return Ok(DplyrOperation::Extended {
                name: "mutate".to_string(),
                args,
                location,
            });
        }

        // Convert NamedArg to Assignment
        let mut assignments = Vec::new();
        for arg in args {
            match arg {
                Expr::NamedArg { name, value } if !is_across(&value) => {
                    assignments.push(Assignment {
                        column: name,
                        expr: *value,
                    });
                }
                Expr::NamedArg { value, .. } if is_across(&value) => {
                    return Err(argument_error(
                        "mutate",
                        "across() must be an unnamed call".into(),
                    ))
                }
                expr if is_across(&expr) => assignments.push(Assignment {
                    column: String::new(),
                    expr,
                }),
                _ => {
                    return Err(ParseError::UnexpectedToken {
                        expected: "assignment (name = expr) or unnamed call to across()"
                            .to_string(),
                        found: "expression".to_string(),
                        position: self.position,
                    })
                }
            }
        }

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

        let mut args = Vec::new();

        // First sort column
        if self.current_token != Token::RightParen {
            args.push(self.parse_arrange_argument()?);

            // Additional sort columns (comma-separated)
            while self.current_token == Token::Comma {
                self.advance()?; // Skip comma
                self.skip_newlines()?;
                if self.current_token == Token::RightParen {
                    break;
                }
                args.push(self.parse_arrange_argument()?);
            }
        }

        self.expect_token(Token::RightParen)?;

        // Route to Extended if advanced options or computed expressions are present
        if has_advanced_options(&args) || args.iter().any(|arg| !matches!(arg, Expr::Identifier(_)) && !matches!(arg, Expr::Function { name, args } if matches!(name.as_str(), "desc" | "asc") && matches!(args.as_slice(), [Expr::Identifier(_)]))) {
            return Ok(DplyrOperation::Extended {
                name: "arrange".to_string(),
                args,
                location,
            });
        }

        // Convert to OrderExpr
        let mut columns = Vec::new();
        for arg in args {
            match arg {
                Expr::Identifier(name) => columns.push(OrderExpr {
                    column: name,
                    direction: OrderDirection::Asc,
                }),
                Expr::Function { name, args } if name == "desc" && args.len() == 1 => {
                    match &args[0] {
                        Expr::Identifier(col) => columns.push(OrderExpr {
                            column: col.clone(),
                            direction: OrderDirection::Desc,
                        }),
                        _ => {
                            return Err(ParseError::UnexpectedToken {
                                expected: "column identifier".to_string(),
                                found: format!("{}", args[0]),
                                position: self.position,
                            })
                        }
                    }
                }
                Expr::Function { name, args } if name == "asc" && args.len() == 1 => {
                    match &args[0] {
                        Expr::Identifier(col) => columns.push(OrderExpr {
                            column: col.clone(),
                            direction: OrderDirection::Asc,
                        }),
                        _ => {
                            return Err(ParseError::UnexpectedToken {
                                expected: "column identifier".to_string(),
                                found: format!("{}", args[0]),
                                position: self.position,
                            })
                        }
                    }
                }
                _ => {
                    return Err(ParseError::UnexpectedToken {
                        expected: "column identifier or desc()/asc()".to_string(),
                        found: format!("{}", arg),
                        position: self.position,
                    })
                }
            }
        }
        Ok(DplyrOperation::Arrange { columns, location })
    }

    /// Parses one arrange() argument, handling desc()/asc() keywords and named options.
    fn parse_arrange_argument(&mut self) -> ParseResult<Expr> {
        match &self.current_token {
            Token::Desc | Token::Asc => {
                let is_desc = matches!(self.current_token, Token::Desc);
                self.advance()?;
                self.expect_token(Token::LeftParen)?;
                let column = self.parse_expression()?;
                self.expect_token(Token::RightParen)?;
                let name = if is_desc { "desc" } else { "asc" };
                Ok(Expr::Function {
                    name: name.to_string(),
                    args: vec![column],
                })
            }
            _ => self.parse_function_argument(),
        }
    }

    /// Parses group_by() operation.
    fn parse_group_by(&mut self) -> ParseResult<DplyrOperation> {
        let location = self.current_location();
        self.advance()?; // Skip 'group_by'
        self.expect_token(Token::LeftParen)?;
        self.consume_optional_lazy_data_argument()?;

        let mut args = Vec::new();

        // First group column
        if self.current_token != Token::RightParen {
            args.push(self.parse_function_argument()?);

            // Additional group columns (comma-separated)
            while self.current_token == Token::Comma {
                self.advance()?; // Skip comma
                self.skip_newlines()?;
                if self.current_token == Token::RightParen {
                    break;
                }
                args.push(self.parse_function_argument()?);
            }
        }

        self.expect_token(Token::RightParen)?;

        // Route to Extended if advanced options or computed expressions are present
        if has_advanced_options(&args) || has_computed(&args) {
            return Ok(DplyrOperation::Extended {
                name: "group_by".to_string(),
                args,
                location,
            });
        }

        // Convert to column names
        let mut columns = Vec::new();
        for arg in args {
            match arg {
                Expr::Identifier(name) => columns.push(name),
                _ => {
                    return Err(ParseError::UnexpectedToken {
                        expected: "column identifier".to_string(),
                        found: format!("{}", arg),
                        position: self.position,
                    })
                }
            }
        }
        Ok(DplyrOperation::GroupBy { columns, location })
    }

    /// Parses summarise() operation.
    // R3-AC1: `n`/`prop` are mutually exclusive, `n` must be a nonnegative
    // whole number, and `prop` only has to be finite and nonnegative. A share
    // above one is legal because it simply truncates to the whole group.
    fn parse_slice(&mut self, kind: SliceKind) -> ParseResult<DplyrOperation> {
        let location = self.current_location();
        self.advance()?;
        self.expect_token(Token::LeftParen)?;
        self.consume_optional_lazy_data_argument()?;

        let verb = match kind {
            SliceKind::Min => "slice_min",
            SliceKind::Max => "slice_max",
            SliceKind::Sample => "slice_sample",
        };

        // Parse all arguments first
        let mut args = Vec::new();
        if self.current_token != Token::RightParen {
            loop {
                self.skip_newlines()?;
                if self.current_token == Token::RightParen {
                    break;
                }
                args.push(self.parse_slice_argument()?);
                if self.current_token == Token::Comma {
                    self.advance()?;
                    continue;
                }
                break;
            }
        }
        self.expect_token(Token::RightParen)?;

        // For slice_sample, check if weight or replace options are present
        if matches!(kind, SliceKind::Sample) {
            let has_weight_or_replace = args.iter().any(|arg| {
                matches!(arg, Expr::NamedArg { name, .. } if name == "weight_by" || name == "replace")
            });
            if has_weight_or_replace {
                return Ok(DplyrOperation::Extended {
                    name: "slice_sample".to_string(),
                    args,
                    location,
                });
            }
        }

        // Process arguments for standard slice operations
        let mut order_by = None;
        let mut n = None;
        let mut prop = None;
        let mut with_ties = !matches!(kind, SliceKind::Sample);
        let mut na_rm = !matches!(kind, SliceKind::Sample);
        let mut by = Vec::new();
        let mut seen: Vec<String> = Vec::new();

        for positional in args {
            let (name, value) = match positional {
                Expr::NamedArg { name, value } => (Some(name), value),
                other => (None, Box::new(other)),
            };

            match name {
                Some(name) => {
                    if seen.contains(&name) {
                        return Err(self.slice_error(verb, format!("{name} was given twice")));
                    }
                    seen.push(name.clone());
                    match name.as_str() {
                        "order_by" => {
                            if matches!(kind, SliceKind::Sample) || order_by.is_some() {
                                return Err(self.slice_error(
                                    verb,
                                    "order_by is not accepted here or was given twice".to_string(),
                                ));
                            }
                            order_by = Some(*value);
                        }
                        "n" => n = Some(number_argument(&value, verb, "n")? as usize),
                        "prop" => prop = Some(number_argument(&value, verb, "prop")?),
                        "with_ties" if !matches!(kind, SliceKind::Sample) => {
                            with_ties = boolean_argument(&value, verb, "with_ties")?
                        }
                        "na_rm" if !matches!(kind, SliceKind::Sample) => {
                            na_rm = boolean_argument(&value, verb, "na_rm")?
                        }
                        "by" => by = slice_by_argument(&value)?,
                        other => {
                            return Err(
                                self.slice_error(verb, format!("unknown argument '{other}'"))
                            );
                        }
                    }
                }
                None => {
                    if matches!(kind, SliceKind::Sample) || order_by.is_some() {
                        return Err(
                            self.slice_error(verb, "unexpected positional argument".to_string())
                        );
                    }
                    order_by = Some(*value);
                }
            }
        }

        if matches!(kind, SliceKind::Min | SliceKind::Max) && order_by.is_none() {
            return Err(self.slice_error(verb, "requires the column to order by".to_string()));
        }
        if n.is_some() && prop.is_some() {
            return Err(self.slice_error(verb, "takes n or prop, not both".to_string()));
        }

        if n.is_none() && prop.is_none() {
            n = Some(1);
        }
        Ok(DplyrOperation::Slice {
            spec: SliceSpec {
                kind,
                order_by,
                n,
                prop,
                with_ties,
                na_rm,
                by,
            },
            location,
        })
    }

    fn slice_error(&self, verb: &str, reason: String) -> ParseError {
        ParseError::InvalidOperation {
            operation: format!("{verb}() {reason}"),
            position: self.position,
        }
    }

    /// `by = c(grp, sub)` parses as `c(grp, sub = sub)`, so the `=` cases can
    /// never nest: the left side is always an identifier or a boolean.
    fn parse_slice_argument(&mut self) -> ParseResult<Expr> {
        let start = self.parse_expression()?;
        if self.current_token != Token::Assignment {
            return Ok(start);
        }
        let name = match start {
            Expr::Identifier(name) => name,
            _ => {
                return Err(ParseError::UnexpectedToken {
                    expected: "named argument identifier".to_string(),
                    found: format!("{}", self.current_token),
                    position: self.position,
                });
            }
        };
        self.advance()?;
        self.skip_newlines()?;
        Ok(Expr::NamedArg {
            name,
            value: Box::new(self.parse_expression()?),
        })
    }

    fn parse_summarise(&mut self) -> ParseResult<DplyrOperation> {
        let location = self.current_location();
        self.advance()?; // Skip 'summarise'
        self.expect_token(Token::LeftParen)?;
        self.consume_optional_lazy_data_argument()?;

        let mut args = Vec::new();

        if self.current_token != Token::RightParen {
            loop {
                args.push(self.parse_summarise_entry_as_expr()?);
                if self.current_token != Token::Comma {
                    break;
                }
                self.advance()?;
                self.skip_newlines()?;
                if self.current_token == Token::RightParen {
                    break;
                }
            }
        }

        self.expect_token(Token::RightParen)?;

        // Route to Extended if advanced options are present
        if has_advanced_options(&args) {
            return Ok(DplyrOperation::Extended {
                name: "summarise".to_string(),
                args,
                location,
            });
        }

        // Convert entries back to the original shape
        let mut entries: Vec<SummariseEntry> = Vec::new();
        for arg in &args {
            entries.push(Self::expr_to_summarise_entry(arg)?);
        }

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

    /// Parses one summarise() entry as an expression (for Extended routing).
    fn parse_summarise_entry_as_expr(&mut self) -> ParseResult<Expr> {
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
        if is_across(&expr) && alias.is_some() {
            return Err(argument_error(
                "summarise",
                "across() must not have a single output name".into(),
            ));
        }
        Ok(match alias {
            Some(name) => Expr::NamedArg {
                name,
                value: Box::new(expr),
            },
            None => expr,
        })
    }

    fn expr_to_summarise_entry(expr: &Expr) -> ParseResult<SummariseEntry> {
        let (value, alias) = match expr {
            Expr::NamedArg { name, value } => (value.as_ref(), Some(name.clone())),
            _ => (expr, None),
        };
        if let Some((function, column)) = simple_aggregation(value) {
            return Ok(SummariseEntry::Aggregation(Aggregation {
                function,
                column,
                alias,
            }));
        }
        Ok(SummariseEntry::Expression(Assignment {
            column: if is_across(value) {
                String::new()
            } else {
                alias.unwrap_or_else(|| value.to_string())
            },
            expr: value.clone(),
        }))
    }

    /// Parses the identifier-only subset of count().
    fn parse_count(&mut self) -> ParseResult<DplyrOperation> {
        let location = self.current_location();
        self.advance()?;
        self.expect_token(Token::LeftParen)?;
        self.consume_optional_lazy_data_argument()?;

        let mut columns = Vec::new();
        let mut advanced_args = Vec::new();

        if self.current_token != Token::RightParen {
            loop {
                self.skip_newlines()?;
                if self.current_token == Token::RightParen {
                    break;
                }

                // Check for named argument (identifier followed by =)
                if let Token::Identifier(name) = &self.current_token {
                    let name_clone = name.clone();
                    if matches!(self.peek_token()?, Token::Assignment) {
                        self.advance()?; // Skip identifier
                        self.advance()?; // Skip =
                        self.skip_newlines()?;
                        let value = self.parse_expression()?;
                        advanced_args.push(Expr::NamedArg {
                            name: name_clone,
                            value: Box::new(value),
                        });
                        if self.current_token == Token::Comma {
                            self.advance()?;
                            continue;
                        }
                        break;
                    }
                }

                // Plain column identifier
                let Token::Identifier(column) = &self.current_token else {
                    return Err(ParseError::UnexpectedToken {
                        expected: "column identifier".to_string(),
                        found: format!("{}", self.current_token),
                        position: self.position,
                    });
                };
                columns.push(column.clone());
                self.advance()?;

                if self.current_token == Token::Comma {
                    self.advance()?;
                    continue;
                }
                break;
            }
        }

        self.expect_token(Token::RightParen)?;

        // If advanced options are present, route to Extended
        if !advanced_args.is_empty() {
            let mut args = columns
                .into_iter()
                .map(Expr::Identifier)
                .collect::<Vec<_>>();
            args.extend(advanced_args);
            return Ok(DplyrOperation::Extended {
                name: "count".to_string(),
                args,
                location,
            });
        }

        Ok(DplyrOperation::Count { columns, location })
    }

    /// Parses the unweighted, argument-free subset of tally().
    fn parse_tally(&mut self) -> ParseResult<DplyrOperation> {
        let location = self.current_location();
        self.advance()?;
        self.expect_token(Token::LeftParen)?;
        self.consume_optional_lazy_data_argument()?;

        let mut advanced_args = Vec::new();
        if self.current_token != Token::RightParen {
            loop {
                self.skip_newlines()?;
                if self.current_token == Token::RightParen {
                    break;
                }

                // Check for named argument (identifier followed by =)
                if let Token::Identifier(name) = &self.current_token {
                    let name_clone = name.clone();
                    if matches!(self.peek_token()?, Token::Assignment) {
                        self.advance()?; // Skip identifier
                        self.advance()?; // Skip =
                        self.skip_newlines()?;
                        let value = self.parse_expression()?;
                        advanced_args.push(Expr::NamedArg {
                            name: name_clone,
                            value: Box::new(value),
                        });
                        if self.current_token == Token::Comma {
                            self.advance()?;
                            continue;
                        }
                        break;
                    }
                }

                // tally() doesn't accept plain column arguments
                return Err(ParseError::UnexpectedToken {
                    expected: "named argument".to_string(),
                    found: format!("{}", self.current_token),
                    position: self.position,
                });
            }
        }

        self.expect_token(Token::RightParen)?;

        // If advanced options are present, route to Extended
        if !advanced_args.is_empty() {
            return Ok(DplyrOperation::Extended {
                name: "tally".to_string(),
                args: advanced_args,
                location,
            });
        }

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
        let mut right_operations = Vec::new();
        while self.current_token == Token::Pipe {
            self.advance()?;
            self.skip_newlines()?;
            right_operations.push(self.parse_operation()?);
        }

        // Every argument after the table is named, so `by`, `suffix`, and the
        // data-dependent options can appear in any order.
        let mut by: Vec<JoinKey> = Vec::new();
        let mut on_expr: Option<Expr> = None;
        let mut options = JoinOptions::default();
        let mut seen: Vec<String> = Vec::new();

        while self.current_token == Token::Comma {
            self.advance()?;
            self.skip_newlines()?;
            if self.current_token == Token::RightParen {
                break;
            }
            if !seen.contains(&"by".to_string()) && self.peek_token()? != Token::Assignment {
                seen.push("by".to_string());
                if self.current_token == Token::Identifier("c".to_string()) {
                    by = self.parse_join_key_vector()?;
                } else {
                    let value = self.parse_expression()?;
                    let (keys, expr) = join_by_argument(&value, self.position)?;
                    by = keys;
                    on_expr = expr;
                }
                continue;
            }
            let Token::Identifier(name) = self.current_token.clone() else {
                return Err(ParseError::UnexpectedToken {
                    expected: "named join argument".to_string(),
                    found: format!("{}", self.current_token),
                    position: self.position,
                });
            };
            self.advance()?;
            self.expect_token(Token::Assignment)?;
            self.skip_newlines()?;
            if seen.contains(&name) {
                return Err(ParseError::InvalidOperation {
                    operation: format!("join() argument {name} was given twice"),
                    position: self.position,
                });
            }
            seen.push(name.clone());

            match name.as_str() {
                "by" => {
                    if self.current_token == Token::Identifier("c".to_string()) {
                        by = self.parse_join_key_vector()?;
                    } else {
                        let value = self.parse_expression()?;
                        let (keys, expr) = join_by_argument(&value, self.position)?;
                        by = keys;
                        on_expr = expr;
                    }
                }
                "suffix" => {
                    if self.current_token == Token::Null {
                        self.advance()?;
                    } else {
                        options.suffix = self.parse_join_suffix()?;
                    }
                }
                "keep" => {
                    // NULL means the default, so it leaves keep at false.
                    if self.current_token != Token::Null {
                        options.keep_explicit = true;
                        match self.parse_expression()? {
                            Expr::Literal(LiteralValue::Boolean(value)) => options.keep = value,
                            other => {
                                return Err(ParseError::InvalidOperation {
                                    operation: format!(
                                        "join() keep must be TRUE, FALSE, or NULL, got {other}"
                                    ),
                                    position: self.position,
                                });
                            }
                        }
                    } else {
                        self.advance()?;
                    }
                }
                "na_matches" => {
                    options.na_matches = match self.parse_join_string()?.as_str() {
                        "na" => true,
                        "never" => false,
                        other => {
                            return Err(ParseError::InvalidOperation {
                                operation: format!(
                                    "join() na_matches must be 'na' or 'never', got '{other}'"
                                ),
                                position: self.position,
                            });
                        }
                    };
                }
                // Data-dependent options. Only their no-op values are honoured;
                // anything else needs a check the database must run, which this
                // planner cannot promise.
                "multiple" => match &self.current_token {
                    Token::Null => {
                        self.advance()?;
                    }
                    Token::String(value)
                        if matches!(value.as_str(), "all" | "first" | "last" | "any") =>
                    {
                        options.multiple = Some(value.clone());
                        self.advance()?;
                    }
                    _ => {
                        return Err(ParseError::InvalidOperation {
                            operation: "join() multiple has no no-op value beyond NULL and 'all'"
                                .to_string(),
                            position: self.position,
                        });
                    }
                },
                "unmatched" => match &self.current_token {
                    Token::String(value) if matches!(value.as_str(), "drop" | "error") => {
                        options.unmatched = Some(value.clone());
                        self.advance()?;
                    }
                    _ => {
                        return Err(ParseError::InvalidOperation {
                            operation: "join() unmatched only supports 'drop'".to_string(),
                            position: self.position,
                        });
                    }
                },
                "relationship" => match &self.current_token {
                    Token::Null => {
                        self.advance()?;
                    }
                    Token::String(value)
                        if matches!(
                            value.as_str(),
                            "many-to-many" | "one-to-one" | "one-to-many" | "many-to-one"
                        ) =>
                    {
                        options.relationship = Some(value.clone());
                        self.advance()?;
                    }
                    _ => {
                        return Err(ParseError::InvalidOperation {
                            operation: "join() relationship only supports NULL and 'many-to-many'"
                                .to_string(),
                            position: self.position,
                        });
                    }
                },
                other => {
                    return Err(ParseError::InvalidOperation {
                        operation: format!("unknown join() argument '{other}'"),
                        position: self.position,
                    });
                }
            }
        }

        self.expect_token(Token::RightParen)?;

        Ok(DplyrOperation::Join {
            join_type,
            spec: JoinSpec {
                table: table_name,
                by,
                on_expr,
                options,
                right_operations,
            },
            location,
        })
    }

    fn parse_join_string(&mut self) -> ParseResult<String> {
        let Token::String(value) = self.current_token.clone() else {
            return Err(ParseError::UnexpectedToken {
                expected: "string literal".to_string(),
                found: format!("{}", self.current_token),
                position: self.position,
            });
        };
        self.advance()?;
        Ok(value)
    }

    fn parse_join_suffix(&mut self) -> ParseResult<(String, String)> {
        self.expect_identifier_name("c")?;
        self.expect_token(Token::LeftParen)?;
        let left = self.parse_join_string()?;
        self.expect_token(Token::Comma)?;
        let right = self.parse_join_string()?;
        self.expect_token(Token::RightParen)?;
        Ok((left, right))
    }

    fn parse_join_key_vector(&mut self) -> ParseResult<Vec<JoinKey>> {
        self.advance()?; // Skip c
        self.expect_token(Token::LeftParen)?;

        let mut keys = Vec::new();
        loop {
            let left = match &self.current_token {
                Token::String(name) => name.clone(),
                // `by = c(id, grp)` joins each name to itself.
                Token::Identifier(name) => name.clone(),
                _ => {
                    return Err(ParseError::UnexpectedToken {
                        expected: "join key name".to_string(),
                        found: format!("{}", self.current_token),
                        position: self.position,
                    })
                }
            };
            self.advance()?;

            let right = if self.current_token == Token::Assignment {
                self.advance()?;
                let name = match &self.current_token {
                    Token::String(name) | Token::Identifier(name) => name.clone(),
                    _ => {
                        return Err(ParseError::UnexpectedToken {
                            expected: "join key name".to_string(),
                            found: format!("{}", self.current_token),
                            position: self.position,
                        });
                    }
                };
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

        let mut args = vec![self.parse_function_argument()?];
        while self.current_token == Token::Comma {
            self.advance()?;
            if self.current_token == Token::RightParen {
                break;
            }
            args.push(self.parse_function_argument()?);
        }
        self.expect_token(Token::RightParen)?;
        if let [Expr::Identifier(right_table)] = args.as_slice() {
            return Ok(DplyrOperation::SetOp {
                operation,
                right_table: right_table.clone(),
                location,
            });
        }
        let name = match operation {
            SetOperation::Union => "union",
            SetOperation::UnionAll => "union_all",
            SetOperation::Intersect => "intersect",
            SetOperation::SetDiff => "setdiff",
        };
        Ok(DplyrOperation::Extended {
            name: name.into(),
            args,
            location,
        })
    }

    /// Parses a generic verb call into Extended.
    fn parse_extended_verb(&mut self, name: &str) -> ParseResult<DplyrOperation> {
        let location = self.current_location();
        self.advance()?; // Skip function name
        self.expect_token(Token::LeftParen)?;
        self.consume_optional_lazy_data_argument()?;

        let mut args = Vec::new();
        if self.current_token != Token::RightParen {
            args.push(self.parse_function_argument()?);
            while self.current_token == Token::Comma {
                self.advance()?;
                self.skip_newlines()?;
                if self.current_token == Token::RightParen {
                    break;
                }
                args.push(self.parse_function_argument()?);
            }
        }

        self.expect_token(Token::RightParen)?;
        Ok(DplyrOperation::Extended {
            name: name.to_string(),
            args,
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

        // Only an alias needs its own path; a bare identifier, a call, a range
        // and a logical selector all go through the expression parser so the
        // shared selector shapes reach the schema-aware resolver.
        if let Token::Identifier(first_name) = self.current_token.clone() {
            if self.peek_token()? == Token::Assignment {
                self.advance()?; // Skip the name
                return self.parse_aliased_column(first_name);
            }
        }

        // Regular expression without alias (for non-identifier expressions).
        // R3-AC1: `select(-x)` is a tidy-select exclusion, so a leading minus
        // reaches here unchanged and the schema decides what it means.
        Ok(ColumnExpr {
            expr: self.parse_expression()?,
            alias: None,
        })
    }

    /// Parses the right-hand side of `new = ...`.
    ///
    /// A chained rename such as `new = old = x` is out of scope: it leaves the
    /// second `=` unconsumed, so the caller's loop stops and the closing paren
    /// or comma check rejects it without recursing.
    fn parse_aliased_column(&mut self, alias: String) -> ParseResult<ColumnExpr> {
        self.expect_token(Token::Assignment)?;
        let expr = self.parse_expression()?;
        Ok(ColumnExpr {
            expr,
            alias: Some(alias),
        })
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
        let mut left = self.parse_not_expression()?;
        let mut chain = 0usize;

        while self.current_token == Token::And {
            self.advance()?;
            let right = self.parse_not_expression()?;
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

    // R2-AC1: Logical negation binds below comparisons, above AND/OR.
    fn parse_not_expression(&mut self) -> ParseResult<Expr> {
        if self.current_token != Token::Not {
            return self.parse_equality_expression();
        }
        if self.peek_token()? == Token::Not {
            return Err(ParseError::InvalidExpression {
                expr: "tidy injection requires typed host bindings; use transpile_with_bindings()"
                    .into(),
                position: self.position,
            });
        }
        self.enter_expression()?;
        self.advance()?;
        self.expression_depth += 1;
        let expr = self.parse_not_expression();
        self.expression_depth -= 1;
        Ok(Expr::Unary {
            operator: UnaryOp::Not,
            expr: Box::new(expr?),
        })
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
        let mut left = self.parse_membership_expression()?;
        let mut chain = 0usize;

        while matches!(
            self.current_token,
            Token::Multiply | Token::Divide | Token::Mod
        ) {
            let operator = match self.current_token {
                Token::Multiply => BinaryOp::Multiply,
                Token::Divide => BinaryOp::Divide,
                Token::Mod => {
                    self.advance()?;
                    let right = self.parse_membership_expression()?;
                    chain += 1;
                    self.check_chain_depth(chain)?;
                    left = Expr::Function {
                        name: "mod".to_string(),
                        args: vec![left, right],
                    };
                    continue;
                }
                _ => unreachable!(),
            };
            self.advance()?;
            let right = self.parse_membership_expression()?;
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

    fn parse_membership_expression(&mut self) -> ParseResult<Expr> {
        let mut expr = self.parse_range_expression()?;
        let mut chain = 0;
        while self.current_token == Token::In {
            self.advance()?;
            let values = self.parse_membership_values()?;
            chain += 1;
            self.check_chain_depth(chain)?;
            expr = Expr::In {
                expr: Box::new(expr),
                values,
            };
        }
        Ok(expr)
    }

    /// Only constant vectors are accepted. A column RHS requires a separate
    /// relation rather than an SQL IN list of per-row column values.
    fn parse_membership_values(&mut self) -> ParseResult<Vec<LiteralValue>> {
        if self.current_token != Token::Identifier("c".to_string()) {
            return Ok(self.parse_membership_literal()?.into_iter().collect());
        }
        self.advance()?;
        self.expect_token(Token::LeftParen)?;
        self.skip_newlines()?;
        let mut values = Vec::new();
        if self.current_token != Token::RightParen {
            loop {
                if let Some(value) = self.parse_membership_literal()? {
                    values.push(value);
                }
                self.skip_newlines()?;
                if self.current_token != Token::Comma {
                    break;
                }
                self.advance()?;
                self.skip_newlines()?;
            }
        }
        self.expect_token(Token::RightParen)?;
        Ok(values)
    }

    fn parse_membership_literal(&mut self) -> ParseResult<Option<LiteralValue>> {
        let sign = match self.current_token {
            Token::Minus => Some(-1.0),
            Token::Plus => Some(1.0),
            _ => None,
        };
        if sign.is_some() {
            self.advance()?;
        }
        let value = match self.current_token.clone() {
            Token::Number(value) => Some(LiteralValue::Number(value * sign.unwrap_or(1.0))),
            Token::String(value) if sign.is_none() => Some(LiteralValue::String(value)),
            Token::Boolean(value) if sign.is_none() => Some(LiteralValue::Boolean(value)),
            Token::Na if sign.is_none() => Some(LiteralValue::Null),
            // R2-AC3: c(NULL) has no elements, whereas c(NA) has a missing member.
            Token::Null if sign.is_none() => None,
            _ => {
                return Err(ParseError::UnexpectedToken {
                    expected: "literal value or c(literal values) after %in%".to_string(),
                    found: self.current_token.to_string(),
                    position: self.position,
                });
            }
        };
        self.advance()?;
        Ok(value)
    }

    fn parse_arithmetic_unary(&mut self) -> ParseResult<Expr> {
        let operator = match self.current_token {
            Token::Plus => UnaryOp::Plus,
            Token::Minus => UnaryOp::Minus,
            _ => return self.parse_power_expression(),
        };
        self.enter_expression()?;
        self.advance()?;
        self.expression_depth += 1;
        let expr = self.parse_arithmetic_unary();
        self.expression_depth -= 1;
        Ok(Expr::Unary {
            operator,
            expr: Box::new(expr?),
        })
    }

    // R2-AC1: Power binds above unary arithmetic and associates right to left.
    fn parse_power_expression(&mut self) -> ParseResult<Expr> {
        let left = self.parse_primary_expression()?;
        if self.current_token != Token::Power {
            return Ok(left);
        }
        self.enter_expression()?;
        self.advance()?;
        self.expression_depth += 1;
        let right = self.parse_arithmetic_unary();
        self.expression_depth -= 1;
        Ok(Expr::Binary {
            left: Box::new(left),
            operator: BinaryOp::Power,
            right: Box::new(right?),
        })
    }

    // R3-AC1: Ranges bind above membership and below unary arithmetic.
    fn parse_range_expression(&mut self) -> ParseResult<Expr> {
        let mut expr = self.parse_arithmetic_unary()?;
        let mut chain = 0;
        while self.current_token == Token::Colon {
            self.advance()?;
            let end = self.parse_arithmetic_unary()?;
            chain += 1;
            self.check_chain_depth(chain)?;
            expr = Expr::Function {
                name: "__select_range".to_string(),
                args: vec![expr, end],
            };
        }
        Ok(expr)
    }

    /// Parses primary expressions.
    fn parse_primary_expression(&mut self) -> ParseResult<Expr> {
        match &self.current_token {
            Token::Desc | Token::Asc => {
                let name = if self.current_token == Token::Desc {
                    "desc"
                } else {
                    "asc"
                };
                self.advance()?;
                self.expect_token(Token::LeftParen)?;
                let args = self.parse_function_arguments()?;
                if args.len() != 1 {
                    return Err(argument_error(name, "requires one expression".into()));
                }
                Ok(Expr::Function {
                    name: name.into(),
                    args,
                })
            }
            Token::Identifier(name) => {
                let name = name.clone();
                self.advance()?;

                if self.current_token == Token::Dollar {
                    if !matches!(name.as_str(), ".data" | ".env") {
                        return Err(argument_error(
                            "member access",
                            "only .data and .env are supported".into(),
                        ));
                    }
                    self.advance()?;
                    let member = self.parse_identifier_like("pronoun member name")?;
                    return Ok(Expr::Function {
                        name: if name == ".data" {
                            "__data_column"
                        } else {
                            "__environment_value"
                        }
                        .into(),
                        args: vec![Expr::Literal(LiteralValue::String(member))],
                    });
                }
                // Check for function call
                if self.current_token == Token::LeftParen {
                    if name == "function" && self.across_depth > 0 {
                        return self.parse_function_closure();
                    }
                    self.advance()?; // Skip (

                    if name == "case_when" {
                        return self.parse_case_when();
                    }

                    // A purrr formula is only a formula inside across(); the
                    // depth guard keeps `~` elsewhere on the old code path.
                    let across = matches!(name.as_str(), "across" | "if_any" | "if_all");
                    if across {
                        self.across_depth += 1;
                    }
                    let parsed = self.parse_function_arguments();
                    if across {
                        self.across_depth -= 1;
                    }
                    let args = parsed?;

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
            // NA is a SQL missing value; bare NULL deletes a mutate column.
            Token::Na => {
                self.advance()?;
                Ok(Expr::Function {
                    name: "__missing_value".into(),
                    args: Vec::new(),
                })
            }
            Token::Null => {
                self.advance()?;
                Ok(Expr::Literal(LiteralValue::Null))
            }
            // The magrittr pronoun. A pipeline's data argument is consumed
            // before expression parsing starts, so a dot reaching here is the
            // across() lambda variable.
            Token::Dot => {
                self.advance()?;
                Ok(Expr::Identifier(".".to_string()))
            }
            // R3-AC1: `~` inside across() captures one lambda body. Elsewhere
            // it belongs to case_when(), which reads the token itself.
            Token::Tilde if self.across_depth > 0 => {
                self.enter_expression()?;
                self.advance()?;
                self.expression_depth += 1;
                let body = self.parse_expression();
                self.expression_depth -= 1;
                let body = body?;
                check_lambda_variables(&body, self.position)?;
                Ok(Expr::Function {
                    name: "__across_lambda".to_string(),
                    args: vec![body],
                })
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

    /// Parses an R `function(v)` closure inside across() and converts it
    /// to the existing `__across_lambda` marker representation.
    fn parse_function_closure(&mut self) -> ParseResult<Expr> {
        // Current token is the opening parenthesis after "function"
        self.expect_token(Token::LeftParen)?;
        self.skip_newlines()?;

        // Parse the parameter name
        let param = match &self.current_token {
            Token::Identifier(name) => {
                let name = name.clone();
                self.advance()?;
                name
            }
            _ => {
                return Err(ParseError::UnexpectedToken {
                    expected: "function parameter name".to_string(),
                    found: format!("{}", self.current_token),
                    position: self.position,
                });
            }
        };

        self.skip_newlines()?;
        self.expect_token(Token::RightParen)?;
        self.skip_newlines()?;

        // Parse the body expression
        let mut body = self.parse_expression()?;
        fn replace_parameter(expr: &mut Expr, param: &str) {
            match expr {
                Expr::Identifier(name) if name == param => *name = ".x".into(),
                Expr::Function { args, .. } => {
                    for arg in args {
                        replace_parameter(arg, param);
                    }
                }
                Expr::Unary { expr, .. } | Expr::In { expr, .. } => replace_parameter(expr, param),
                Expr::NamedArg { value, .. } => replace_parameter(value, param),
                Expr::Binary { left, right, .. } => {
                    replace_parameter(left, param);
                    replace_parameter(right, param);
                }
                Expr::CaseWhen { branches, default } => {
                    for (a, b) in branches {
                        replace_parameter(a, param);
                        replace_parameter(b, param);
                    }
                    if let Some(value) = default {
                        replace_parameter(value, param);
                    }
                }
                _ => {}
            }
        }
        replace_parameter(&mut body, &param);
        check_lambda_variables(&body, self.position)?;

        Ok(Expr::Function {
            name: "__across_lambda".to_string(),
            args: vec![body],
        })
    }

    /// Parses a comma-separated argument list, with the opening parenthesis
    /// already consumed.
    fn parse_function_arguments(&mut self) -> ParseResult<Vec<Expr>> {
        self.skip_newlines()?;
        let mut args = Vec::new();
        if self.current_token != Token::RightParen {
            args.push(self.parse_function_argument()?);

            while self.current_token == Token::Comma {
                self.advance()?; // Skip ,
                self.skip_newlines()?;
                args.push(self.parse_function_argument()?);
            }
        }
        self.expect_token(Token::RightParen)?;
        Ok(args)
    }

    fn parse_case_when(&mut self) -> ParseResult<Expr> {
        let mut branches = Vec::new();
        let mut default = None;

        while self.current_token != Token::RightParen {
            if self.current_token == Token::Dot
                || self.current_token == Token::Identifier(".default".to_string())
            {
                let split = self.current_token == Token::Dot;
                self.advance()?;
                if split {
                    self.expect_identifier_name("default")?;
                }
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

    /// Parses an expression that may contain pipe operators (`%>%`).
    /// Used in function argument context to support pipeline expressions
    /// like `source %>% verb(args)` inside function calls.
    fn parse_pipe_tail(&mut self, source: Expr) -> ParseResult<Expr> {
        if self.current_token != Token::Pipe {
            return Ok(source);
        }
        let mut args = vec![source];
        while self.current_token == Token::Pipe {
            self.advance()?;
            self.skip_newlines()?;
            let name = match &self.current_token {
                Token::Select => "select",
                Token::Distinct => "distinct",
                Token::Filter => "filter",
                Token::Mutate => "mutate",
                Token::Rename => "rename",
                Token::Arrange => "arrange",
                Token::GroupBy => "group_by",
                Token::Summarise => "summarise",
                Token::Union => "union",
                Token::Intersect => "intersect",
                Token::SetDiff => "setdiff",
                Token::Identifier(name) => name,
                _ => {
                    return Err(argument_error(
                        "pipeline",
                        "expected a relation operation".into(),
                    ))
                }
            }
            .to_owned();
            let DplyrOperation::Extended { args: values, .. } = self.parse_extended_verb(&name)?
            else {
                return Err(argument_error(
                    "pipeline",
                    "invalid nested operation".into(),
                ));
            };
            args.push(Expr::Function { name, args: values });
        }
        Ok(Expr::Function {
            name: "__pipeline".into(),
            args,
        })
    }

    fn parse_function_argument(&mut self) -> ParseResult<Expr> {
        self.skip_newlines()?;
        let expr = self.parse_expression()?;
        let expr = if self.current_token == Token::Pipe {
            self.parse_pipe_tail(expr)?
        } else {
            expr
        };
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

/// Rejects any dot-prefixed identifier that is not the across() lambda
/// variable, so `.y` cannot reach SQL as a bare column.
fn check_lambda_variables(expr: &Expr, position: usize) -> ParseResult<()> {
    let mut stack = vec![expr];
    while let Some(expr) = stack.pop() {
        match expr {
            Expr::Identifier(name) if name.starts_with('.') && name != "." && name != ".x" => {
                return Err(ParseError::InvalidExpression {
                    expr: format!("'{name}' is not an across() lambda variable"),
                    position,
                });
            }
            Expr::Identifier(_) | Expr::Literal(_) => {}
            Expr::Unary { expr, .. } | Expr::In { expr, .. } => stack.push(expr),
            Expr::Binary { left, right, .. } => {
                stack.push(left);
                stack.push(right);
            }
            Expr::Function { args, .. } => stack.extend(args.iter()),
            Expr::NamedArg { value, .. } => stack.push(value),
            Expr::CaseWhen { branches, default } => {
                for (condition, value) in branches {
                    stack.push(condition);
                    stack.push(value);
                }
                if let Some(default) = default {
                    stack.push(default);
                }
            }
        }
    }
    Ok(())
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
                DplyrOperation::Extended { args, .. } => {
                    roots.extend(args.iter());
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
            Expr::Unary { expr, .. } | Expr::In { expr, .. } => stack.push((expr, depth + 1)),
            Expr::Identifier(_) | Expr::Literal(_) => {}
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/parse_tests.rs"]
mod tests;
