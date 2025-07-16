//! Parser module
//!
//! Provides functionality to convert tokens to AST (Abstract Syntax Tree).

use crate::error::{ParseError, ParseResult};
use crate::lexer::{Lexer, Token};

/// Source code location information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}

impl SourceLocation {
    pub fn new(line: usize, column: usize, offset: usize) -> Self {
        Self {
            line,
            column,
            offset,
        }
    }

    pub fn unknown() -> Self {
        Self {
            line: 0,
            column: 0,
            offset: 0,
        }
    }
}

/// Top-level node of dplyr AST
#[derive(Debug, Clone, PartialEq)]
pub enum DplyrNode {
    /// Chain of pipeline operations
    Pipeline {
        operations: Vec<DplyrOperation>,
        location: SourceLocation,
    },
    /// Data source reference
    DataSource {
        name: String,
        location: SourceLocation,
    },
}

impl DplyrNode {
    /// Returns the location information of the node.
    pub fn location(&self) -> &SourceLocation {
        match self {
            DplyrNode::Pipeline { location, .. } => location,
            DplyrNode::DataSource { location, .. } => location,
        }
    }

    /// Checks if this is a pipeline node.
    pub fn is_pipeline(&self) -> bool {
        matches!(self, DplyrNode::Pipeline { .. })
    }

    /// Checks if this is a data source node.
    pub fn is_data_source(&self) -> bool {
        matches!(self, DplyrNode::DataSource { .. })
    }
}

/// dplyr operation types
#[derive(Debug, Clone, PartialEq)]
pub enum DplyrOperation {
    /// SELECT operation (column selection)
    Select { columns: Vec<ColumnExpr> },
    /// WHERE operation (row filtering)
    Filter { condition: Expr },
    /// Create/modify new columns
    Mutate { assignments: Vec<Assignment> },
    /// ORDER BY operation (sorting)
    Arrange { columns: Vec<OrderExpr> },
    /// GROUP BY operation (grouping)
    GroupBy { columns: Vec<String> },
    /// Aggregation operation
    Summarise { aggregations: Vec<Aggregation> },
}

/// Expression types
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Identifier (column name, variable name, etc.)
    Identifier(String),
    /// Literal value
    Literal(LiteralValue),
    /// Binary operation
    Binary {
        left: Box<Expr>,
        operator: BinaryOp,
        right: Box<Expr>,
    },
    /// Function call
    Function { name: String, args: Vec<Expr> },
}

/// Literal value types
#[derive(Debug, Clone, PartialEq)]
pub enum LiteralValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
}

/// Binary operator types
#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    // Comparison operators
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,

    // Logical operators
    And,
    Or,

    // Arithmetic operators
    Plus,
    Minus,
    Multiply,
    Divide,
}

/// Column expression (with alias support)
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnExpr {
    pub expr: Expr,
    pub alias: Option<String>,
}

/// Sort expression
#[derive(Debug, Clone, PartialEq)]
pub struct OrderExpr {
    pub column: String,
    pub direction: OrderDirection,
}

/// Sort direction
#[derive(Debug, Clone, PartialEq)]
pub enum OrderDirection {
    Asc,
    Desc,
}

/// Assignment statement (used in mutate)
#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    pub column: String,
    pub expr: Expr,
}

/// Aggregation operation (used in summarise)
#[derive(Debug, Clone, PartialEq)]
pub struct Aggregation {
    pub function: String,
    pub column: String,
    pub alias: Option<String>,
}

/// Parser struct
pub struct Parser {
    lexer: Lexer,
    current_token: Token,
    position: usize,
}

impl Parser {
    /// Creates a new parser instance.
    ///
    /// # Arguments
    ///
    /// * `lexer` - The lexer instance to use
    pub fn new(mut lexer: Lexer) -> ParseResult<Self> {
        let current_token = lexer.next_token()?;
        Ok(Self {
            lexer,
            current_token,
            position: 0,
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

    /// Advances to the next token.
    fn advance(&mut self) -> ParseResult<()> {
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
                expected: format!("{}", expected),
                found: format!("{}", self.current_token),
                position: self.position,
            })
        }
    }

    /// Parses a pipeline.
    fn parse_pipeline(&mut self) -> ParseResult<DplyrNode> {
        let mut operations = Vec::new();

        // Parse first operation
        if let Ok(operation) = self.parse_operation() {
            operations.push(operation);

            // Parse additional operations connected by pipe operators
            while self.current_token == Token::Pipe {
                self.advance()?; // Skip %>%
                operations.push(self.parse_operation()?);
            }
        }

        if operations.is_empty() {
            Err(ParseError::InvalidOperation {
                operation: "empty pipeline".to_string(),
                position: self.position,
            })
        } else {
            Ok(DplyrNode::Pipeline {
                operations,
                location: SourceLocation::unknown(),
            })
        }
    }

    /// Parses individual dplyr operations.
    fn parse_operation(&mut self) -> ParseResult<DplyrOperation> {
        match &self.current_token {
            Token::Select => self.parse_select(),
            Token::Filter => self.parse_filter(),
            Token::Mutate => self.parse_mutate(),
            Token::Arrange => self.parse_arrange(),
            Token::GroupBy => self.parse_group_by(),
            Token::Summarise => self.parse_summarise(),
            _ => Err(ParseError::UnexpectedToken {
                expected: "dplyr function".to_string(),
                found: format!("{}", self.current_token),
                position: self.position,
            }),
        }
    }

    /// Parses select() operation.
    fn parse_select(&mut self) -> ParseResult<DplyrOperation> {
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
        Ok(DplyrOperation::Select { columns })
    }

    /// Parses filter() operation.
    fn parse_filter(&mut self) -> ParseResult<DplyrOperation> {
        self.advance()?; // Skip 'filter'
        self.expect_token(Token::LeftParen)?;

        let condition = self.parse_expression()?;

        self.expect_token(Token::RightParen)?;
        Ok(DplyrOperation::Filter { condition })
    }

    /// Parses mutate() operation.
    fn parse_mutate(&mut self) -> ParseResult<DplyrOperation> {
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
        Ok(DplyrOperation::Mutate { assignments })
    }

    /// Parses arrange() operation.
    fn parse_arrange(&mut self) -> ParseResult<DplyrOperation> {
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
        Ok(DplyrOperation::Arrange { columns })
    }

    /// Parses group_by() operation.
    fn parse_group_by(&mut self) -> ParseResult<DplyrOperation> {
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
        Ok(DplyrOperation::GroupBy { columns })
    }

    /// Parses summarise() operation.
    fn parse_summarise(&mut self) -> ParseResult<DplyrOperation> {
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
        Ok(DplyrOperation::Summarise { aggregations })
    }

    /// Parses column expressions.
    fn parse_column_expr(&mut self) -> ParseResult<ColumnExpr> {
        let expr = self.parse_expression()?;

        // Check for alias (using = operator)
        let alias = if self.current_token == Token::Assignment {
            self.advance()?; // Skip =
            if let Token::Identifier(alias_name) = &self.current_token {
                let alias = Some(alias_name.clone());
                self.advance()?;
                alias
            } else {
                return Err(ParseError::UnexpectedToken {
                    expected: "alias identifier".to_string(),
                    found: format!("{}", self.current_token),
                    position: self.position,
                });
            }
        } else {
            None
        };

        Ok(ColumnExpr { expr, alias })
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
        // Check for desc() function
        if let Token::Identifier(name) = &self.current_token {
            if name == "desc" {
                self.advance()?; // Skip 'desc'
                self.expect_token(Token::LeftParen)?;

                if let Token::Identifier(column) = &self.current_token {
                    let column = column.clone();
                    self.advance()?;
                    self.expect_token(Token::RightParen)?;

                    return Ok(OrderExpr {
                        column,
                        direction: OrderDirection::Desc,
                    });
                }
            } else {
                // Regular column (ascending)
                let column = name.clone();
                self.advance()?;
                return Ok(OrderExpr {
                    column,
                    direction: OrderDirection::Asc,
                });
            }
        }

        Err(ParseError::UnexpectedToken {
            expected: "column identifier or desc()".to_string(),
            found: format!("{}", self.current_token),
            position: self.position,
        })
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

                    if let Token::Identifier(column) = &self.current_token {
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
                            expected: "column identifier".to_string(),
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

                if let Token::Identifier(column) = &self.current_token {
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
                        expected: "column identifier".to_string(),
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
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    #[test]
    fn test_parse_simple_select() {
        let lexer = Lexer::new("select(name, age)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Select { columns } = &operations[0] {
                assert_eq!(columns.len(), 2);
            } else {
                panic!("Expected Select operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_parse_filter() {
        let lexer = Lexer::new("filter(age > 18)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            assert!(matches!(operations[0], DplyrOperation::Filter { .. }));
        }
    }
}
