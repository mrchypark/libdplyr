//! Parser AST types.
//!
//! This module defines the AST (Abstract Syntax Tree) nodes produced by the parser.

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
        source: Option<String>,
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
    Select {
        columns: Vec<ColumnExpr>,
        location: SourceLocation,
    },
    /// WHERE operation (row filtering)
    Filter {
        condition: Expr,
        location: SourceLocation,
    },
    /// Create/modify new columns
    Mutate {
        assignments: Vec<Assignment>,
        location: SourceLocation,
    },
    /// Rename one or more columns (dplyr-style: new_name = old_name)
    Rename {
        renames: Vec<RenameSpec>,
        location: SourceLocation,
    },
    /// ORDER BY operation (sorting)
    Arrange {
        columns: Vec<OrderExpr>,
        location: SourceLocation,
    },
    /// GROUP BY operation (grouping)
    GroupBy {
        columns: Vec<String>,
        location: SourceLocation,
    },
    /// Aggregation operation
    Summarise {
        aggregations: Vec<Aggregation>,
        location: SourceLocation,
    },
    /// JOIN operation for combining tables
    Join {
        join_type: JoinType,
        spec: JoinSpec,
        location: SourceLocation,
    },
}

/// Column rename specification (dplyr-style: new_name = old_name).
#[derive(Debug, Clone, PartialEq)]
pub struct RenameSpec {
    pub new_name: String,
    pub old_name: String,
}

impl DplyrOperation {
    /// Returns the location information of the operation.
    pub fn location(&self) -> &SourceLocation {
        match self {
            DplyrOperation::Select { location, .. } => location,
            DplyrOperation::Filter { location, .. } => location,
            DplyrOperation::Mutate { location, .. } => location,
            DplyrOperation::Rename { location, .. } => location,
            DplyrOperation::Arrange { location, .. } => location,
            DplyrOperation::GroupBy { location, .. } => location,
            DplyrOperation::Summarise { location, .. } => location,
            DplyrOperation::Join { location, .. } => location,
        }
    }

    /// Returns the operation name as a string.
    pub fn operation_name(&self) -> &'static str {
        match self {
            DplyrOperation::Select { .. } => "select",
            DplyrOperation::Filter { .. } => "filter",
            DplyrOperation::Mutate { .. } => "mutate",
            DplyrOperation::Rename { .. } => "rename",
            DplyrOperation::Arrange { .. } => "arrange",
            DplyrOperation::GroupBy { .. } => "group_by",
            DplyrOperation::Summarise { .. } => "summarise",
            DplyrOperation::Join { .. } => "join",
        }
    }
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

/// Join type for different join operations
#[derive(Debug, Clone, PartialEq)]
pub enum JoinType {
    Inner,
    Left,
    Right,
    Full,
    Semi,
    Anti,
}

/// Join specification containing table and join condition
#[derive(Debug, Clone, PartialEq)]
pub struct JoinSpec {
    pub table: String,
    pub on: Expr,
}

/// Join operation for combining tables
#[derive(Debug, Clone, PartialEq)]
pub struct Join {
    pub join_type: JoinType,
    pub spec: JoinSpec,
}
