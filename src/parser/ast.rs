//! Parser AST types.
//!
//! This module defines the AST (Abstract Syntax Tree) nodes produced by the parser.

use std::borrow::Cow;

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
    /// SELECT DISTINCT operation (optional identifier projection)
    Distinct {
        columns: Vec<String>,
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
    /// Clear grouping metadata for subsequent operations.
    Ungroup { location: SourceLocation },
    /// Row slicing: `slice_min()`, `slice_max()`, or `slice_sample()`.
    Slice {
        spec: SliceSpec,
        location: SourceLocation,
    },
    /// Aggregation operation
    Summarise {
        aggregations: Vec<Aggregation>,
        location: SourceLocation,
    },
    /// Aggregation operation whose entries are arbitrary expressions.
    ///
    /// Used when at least one entry is not the `function(identifier)` or
    /// `function()` shape that [`Aggregation`] can represent, such as
    /// `sum(x * y)`, `sum(x) / n()`, or a scalar wrapping an aggregate.
    SummariseExpressions {
        assignments: Vec<Assignment>,
        location: SourceLocation,
    },
    /// Count rows, optionally adding identifier-only grouping keys.
    Count {
        columns: Vec<String>,
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
    /// Advanced query forms the relational planner compiles generically.
    Extended {
        name: String,
        args: Vec<Expr>,
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
            Self::Distinct { location, .. } => location,
            Self::Filter { location, .. } => location,
            Self::Mutate { location, .. } => location,
            Self::Rename { location, .. } => location,
            Self::Arrange { location, .. } => location,
            Self::GroupBy { location, .. } => location,
            Self::Ungroup { location } => location,
            Self::Slice { location, .. } => location,
            Self::Summarise { location, .. } => location,
            Self::SummariseExpressions { location, .. } => location,
            Self::Count { location, .. } => location,
            Self::Join { location, .. } => location,
            Self::SetOp { location, .. } => location,
            Self::Extended { location, .. } => location,
        }
    }

    /// Returns the operation name as a string.
    pub fn operation_name(&self) -> Cow<'static, str> {
        match self {
            Self::Select { .. } => Cow::Borrowed("select"),
            Self::Distinct { .. } => Cow::Borrowed("distinct"),
            Self::Filter { .. } => Cow::Borrowed("filter"),
            Self::Mutate { .. } => Cow::Borrowed("mutate"),
            Self::Rename { .. } => Cow::Borrowed("rename"),
            Self::Arrange { .. } => Cow::Borrowed("arrange"),
            Self::GroupBy { .. } => Cow::Borrowed("group_by"),
            Self::Ungroup { .. } => Cow::Borrowed("ungroup"),
            Self::Slice { spec, .. } => match spec.kind {
                SliceKind::Min => Cow::Borrowed("slice_min"),
                SliceKind::Max => Cow::Borrowed("slice_max"),
                SliceKind::Sample => Cow::Borrowed("slice_sample"),
            },
            Self::Summarise { .. } => Cow::Borrowed("summarise"),
            Self::SummariseExpressions { .. } => Cow::Borrowed("summarise"),
            Self::Count { .. } => Cow::Borrowed("count/tally"),
            Self::Join { .. } => Cow::Borrowed("join"),
            Self::SetOp { operation, .. } => match operation {
                SetOperation::Intersect => Cow::Borrowed("intersect"),
                SetOperation::Union => Cow::Borrowed("union"),
                SetOperation::UnionAll => Cow::Borrowed("union_all"),
                SetOperation::SetDiff => Cow::Borrowed("setdiff"),
            },
            Self::Extended { name, .. } => Cow::Owned(name.clone()),
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
    /// Unary arithmetic or logical negation.
    Unary { operator: UnaryOp, expr: Box<Expr> },
    /// Membership in a constant vector. NULL members represent R's NA.
    In {
        expr: Box<Expr>,
        values: Vec<LiteralValue>,
    },
    /// Function call
    Function { name: String, args: Vec<Expr> },
    /// Ordered `case_when()` formulas with an optional default.
    CaseWhen {
        branches: Vec<(Expr, Expr)>,
        default: Option<Box<Expr>>,
    },
    /// Named function argument, e.g. `sep = " "`.
    NamedArg { name: String, value: Box<Expr> },
}

/// Renders an expression as a readable label for use as an output column name.
///
/// This is a *label*, not a round-trippable R deparse: booleans print in SQL
/// spelling (`TRUE`), and strings are escaped only enough to stay unambiguous
/// rather than to be re-lexable. It guarantees one distinct label per distinct
/// expression tree, which is what naming an unnamed `summarise()` entry needs.
///
/// Every `Binary` is parenthesized. That is more parentheses than R precedence
/// strictly requires, but it keeps `x * y + 1` and `x * (y + 1)` distinct, so
/// two structurally different unnamed entries can never collide on one name.
impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Identifier(name) => write!(f, "{name}"),
            Self::Literal(LiteralValue::String(value)) => {
                let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
                write!(f, "\"{escaped}\"")
            }
            Self::Literal(LiteralValue::Number(value)) => write!(f, "{value}"),
            Self::Literal(LiteralValue::Boolean(value)) => {
                f.write_str(if *value { "TRUE" } else { "FALSE" })
            }
            Self::Literal(LiteralValue::Null) => write!(f, "NULL"),
            Self::Binary {
                left,
                operator,
                right,
            } => write!(f, "({left} {operator} {right})"),
            Self::Unary { operator, expr } => write!(f, "({operator}{expr})"),
            Self::In { expr, values } => {
                write!(f, "({expr} %in% c(")?;
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", Self::Literal(value.clone()))?;
                }
                write!(f, "))")
            }
            Self::Function { name, args } if name == "__missing_value" && args.is_empty() => {
                f.write_str("NA")
            }
            Self::Function { name, args } => {
                write!(f, "{name}(")?;
                for (index, arg) in args.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{arg}")?;
                }
                write!(f, ")")
            }
            Self::CaseWhen { branches, default } => {
                write!(f, "case_when(")?;
                for (condition, value) in branches {
                    write!(f, "{condition} ~ {value}, ")?;
                }
                match default {
                    Some(default) => write!(f, "default = {default}"),
                    None => f.write_str("default = NULL"),
                }?;
                write!(f, ")")
            }
            Self::NamedArg { name, value } => write!(f, "{name} = {value}"),
        }
    }
}

impl std::fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::Equal => "==",
            Self::NotEqual => "!=",
            Self::LessThan => "<",
            Self::LessThanOrEqual => "<=",
            Self::GreaterThan => ">",
            Self::GreaterThanOrEqual => ">=",
            Self::And => "&&",
            Self::Or => "||",
            Self::Plus => "+",
            Self::Minus => "-",
            Self::Multiply => "*",
            Self::Divide => "/",
            Self::Power => "^",
        };
        f.write_str(text)
    }
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
    Power,
}

/// Unary operators retain their distinct precedence in the parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnaryOp {
    Plus,
    Minus,
    Not,
}

impl std::fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Plus => "+",
            Self::Minus => "-",
            Self::Not => "!",
        })
    }
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

/// One `summarise()` entry, kept in its narrowest representable form.
pub(crate) enum SummariseEntry {
    Aggregation(Aggregation),
    Expression(Assignment),
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
    /// Equality keys from `by = "id"` or `by = c("left" = "right", "same")`.
    pub by: Vec<JoinKey>,
    /// Fallback: general expression for complex joins
    pub on_expr: Option<Expr>,
    /// Rendering options that change the join output.
    pub options: JoinOptions,
    /// Optional RHS pipeline operations (e.g., `right %>% filter(...)`)
    pub right_operations: Vec<DplyrOperation>,
}

/// Join options that the SQL layer can honour exactly.
///
/// Options whose effect depends on row data (`multiple`, `unmatched`,
/// `relationship`) are validated in the parser and then discarded, because
/// either their accepted values are a no-op or they need validation the
/// database must perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinOptions {
    /// Suffixes for the left and right copies of a colliding column name.
    pub suffix: (String, String),
    /// Keep the right-hand key column in the output.
    pub keep: bool,
    /// Whether `keep` was explicitly specified by the user.
    pub keep_explicit: bool,
    /// Treat NULL as a matching join key.
    pub na_matches: bool,
    /// Documented relationship assertion; preserved for future planning.
    pub relationship: Option<String>,
    /// Documented multiple-match policy; preserved for future planning.
    pub multiple: Option<String>,
    /// Documented unmatched-row policy; preserved for future planning.
    pub unmatched: Option<String>,
}

impl Default for JoinOptions {
    fn default() -> Self {
        Self {
            suffix: (".x".to_string(), ".y".to_string()),
            keep: false,
            keep_explicit: false,
            na_matches: false,
            relationship: None,
            multiple: None,
            unmatched: None,
        }
    }
}

/// Which rows a slice keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SliceKind {
    Min,
    Max,
    Sample,
}

/// One `slice_*()` call, already validated for mutual exclusivity.
#[derive(Debug, Clone, PartialEq)]
pub struct SliceSpec {
    pub kind: SliceKind,
    /// Tie-breaker expression; when present it fully determines the cut.
    pub order_by: Option<Expr>,
    pub n: Option<usize>,
    pub prop: Option<f64>,
    pub with_ties: bool,
    pub na_rm: bool,
    /// Per-group slicing selector.
    pub by: Vec<ColumnExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinKey {
    pub left: String,
    pub right: String,
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
    UnionAll,
    SetDiff, // EXCEPT in SQL
}

/// Set operation combining two queries
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetOp {
    pub operation: SetOperation,
    pub right_table: String,
}
