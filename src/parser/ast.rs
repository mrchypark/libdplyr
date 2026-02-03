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
    pub const fn new(line: usize, column: usize, offset: usize) -> Self {
        Self {
            line,
            column,
            offset,
        }
    }

    pub const fn unknown() -> Self {
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
        target: Option<String>,
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
    pub const fn location(&self) -> &SourceLocation {
        match self {
            Self::Pipeline { location, .. } => location,
            Self::DataSource { location, .. } => location,
        }
    }

    /// Checks if this is a pipeline node.
    pub const fn is_pipeline(&self) -> bool {
        matches!(self, Self::Pipeline { .. })
    }

    /// Checks if this is a data source node.
    pub const fn is_data_source(&self) -> bool {
        matches!(self, Self::DataSource { .. })
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
    /// Set operation (INTERSECT, UNION, EXCEPT)
    SetOp {
        operation: SetOperation,
        right_table: String,
        location: SourceLocation,
    },
}

/// Column rename specification (dplyr-style: new_name = old_name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameSpec {
    pub new_name: String,
    pub old_name: String,
}

impl DplyrOperation {
    /// Returns the location information of the operation.
    pub const fn location(&self) -> &SourceLocation {
        match self {
            Self::Select { location, .. } => location,
            Self::Filter { location, .. } => location,
            Self::Mutate { location, .. } => location,
            Self::Rename { location, .. } => location,
            Self::Arrange { location, .. } => location,
            Self::GroupBy { location, .. } => location,
            Self::Summarise { location, .. } => location,
            Self::Join { location, .. } => location,
            Self::SetOp { location, .. } => location,
        }
    }

    /// Returns the operation name as a string.
    pub const fn operation_name(&self) -> &'static str {
        match self {
            Self::Select { .. } => "select",
            Self::Filter { .. } => "filter",
            Self::Mutate { .. } => "mutate",
            Self::Rename { .. } => "rename",
            Self::Arrange { .. } => "arrange",
            Self::GroupBy { .. } => "group_by",
            Self::Summarise { .. } => "summarise",
            Self::Join { .. } => "join",
            Self::SetOp { operation, .. } => match operation {
                SetOperation::Intersect => "intersect",
                SetOperation::Union => "union",
                SetOperation::SetDiff => "setdiff",
            },
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderExpr {
    pub column: String,
    pub direction: OrderDirection,
}

/// Sort direction
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aggregation {
    pub function: String,
    pub column: String,
    pub alias: Option<String>,
}

/// Join type for different join operations
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// Single column name for simple joins (e.g., `by = "id"`)
    pub by_column: Option<String>,
    /// Fallback: general expression for complex joins
    pub on_expr: Option<Expr>,
}

/// Join operation for combining tables
#[derive(Debug, Clone, PartialEq)]
pub struct Join {
    pub join_type: JoinType,
    pub spec: JoinSpec,
}

/// Set operation type (INTERSECT, UNION, EXCEPT)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetOperation {
    Intersect,
    Union,
    SetDiff, // EXCEPT in SQL
}

/// Set operation combining two queries
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetOp {
    pub operation: SetOperation,
    pub right_table: String,
}
