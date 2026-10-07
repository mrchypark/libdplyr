//! SQL generator module
//!
//! Provides functionality to convert AST to various SQL dialects.

use crate::error::{GenerationError, GenerationResult};
use crate::parser::{
    Aggregation, BinaryOp, ColumnExpr, DplyrNode, DplyrOperation, Expr, JoinSpec, JoinType,
    LiteralValue, OrderDirection, OrderExpr, RenameSpec, SetOperation,
};

// Decomposition scaffolding (“Tidy First”): these modules are placeholders to
// enable incremental extraction from this large module without behavior changes.
pub mod assemble;
pub mod dialect;
pub mod mutate_support;

use assemble::QueryParts;

pub use dialect::{
    DialectConfig, DuckDbDialect, MySqlDialect, PostgreSqlDialect, SqlDialect, SqliteDialect,
};

/// SQL generator struct
pub struct SqlGenerator {
    dialect: Box<dyn SqlDialect>,
}

#[derive(Clone, Copy)]
struct NamedArgFormal {
    name: &'static str,
    default_sql: Option<&'static str>,
}

/// How a function call at this position must be rendered.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RenderContext {
    /// Plain scalar SQL. Aggregates are not available here.
    Scalar,
    /// Aggregates become window functions with an optional PARTITION BY.
    Window,
    /// Grouped aggregates. `OVER` is never emitted and nesting is rejected.
    Aggregate,
    /// Inside an aggregate's own argument list. A further aggregate here would
    /// render as `SUM(SUM(x))`, so it is rejected instead.
    AggregateInner,
}

const ROUND_FORMALS: &[NamedArgFormal] = &[
    NamedArgFormal {
        name: "x",
        default_sql: None,
    },
    NamedArgFormal {
        name: "digits",
        default_sql: None,
    },
];
const LEAD_LAG_FORMALS: &[NamedArgFormal] = &[
    NamedArgFormal {
        name: "x",
        default_sql: None,
    },
    NamedArgFormal {
        name: "n",
        default_sql: Some("1"),
    },
    NamedArgFormal {
        name: "default",
        default_sql: Some("NULL"),
    },
    NamedArgFormal {
        name: "order_by",
        default_sql: None,
    },
];
const STR_DETECT_FORMALS: &[NamedArgFormal] = &[
    NamedArgFormal {
        name: "string",
        default_sql: None,
    },
    NamedArgFormal {
        name: "pattern",
        default_sql: None,
    },
];
const SUBSTR_FORMALS: &[NamedArgFormal] = &[
    NamedArgFormal {
        name: "x",
        default_sql: None,
    },
    NamedArgFormal {
        name: "start",
        default_sql: None,
    },
    NamedArgFormal {
        name: "stop",
        default_sql: None,
    },
];
const LOG_FORMALS: &[NamedArgFormal] = &[
    NamedArgFormal {
        name: "x",
        default_sql: None,
    },
    NamedArgFormal {
        name: "base",
        default_sql: None,
    },
];
const UNARY_X_FORMALS: &[NamedArgFormal] = &[NamedArgFormal {
    name: "x",
    default_sql: None,
}];
const VALUE_ORDER_FORMALS: &[NamedArgFormal] = &[
    NamedArgFormal {
        name: "x",
        default_sql: None,
    },
    NamedArgFormal {
        name: "order_by",
        default_sql: None,
    },
];
const IFELSE_FORMALS: &[NamedArgFormal] = &[
    NamedArgFormal {
        name: "test",
        default_sql: None,
    },
    NamedArgFormal {
        name: "yes",
        default_sql: None,
    },
    NamedArgFormal {
        name: "no",
        default_sql: None,
    },
];
const IF_ELSE_FORMALS: &[NamedArgFormal] = &[
    NamedArgFormal {
        name: "condition",
        default_sql: None,
    },
    NamedArgFormal {
        name: "true",
        default_sql: None,
    },
    NamedArgFormal {
        name: "false",
        default_sql: None,
    },
];

fn named_argument_formals(function: &str) -> Option<&'static [NamedArgFormal]> {
    match function.to_ascii_lowercase().as_str() {
        "round" => Some(ROUND_FORMALS),
        "lead" | "lag" => Some(LEAD_LAG_FORMALS),
        "str_detect" => Some(STR_DETECT_FORMALS),
        "substr" => Some(SUBSTR_FORMALS),
        "log" => Some(LOG_FORMALS),
        "abs" | "floor" | "ceiling" | "ceil" | "sqrt" | "sign" | "exp" | "log10" | "sin"
        | "cos" | "tan" | "asin" | "acos" | "atan" | "sinh" | "cosh" | "tanh" | "str_length"
        | "str_to_lower" | "str_to_upper" | "str_trim" | "nchar" | "nzchar" | "trimws"
        | "as.numeric" | "as.double" | "as.integer" | "as.character" | "as.logical" => {
            Some(UNARY_X_FORMALS)
        }
        "first" | "first_value" | "last" | "last_value" => Some(VALUE_ORDER_FORMALS),
        "ifelse" => Some(IFELSE_FORMALS),
        "if_else" => Some(IF_ELSE_FORMALS),
        _ => None,
    }
}

/// Functions that only exist as window functions. They have no grouped
/// aggregate meaning, so aggregate rendering rejects them instead of
/// silently dropping the frame.
fn is_window_only_function(function: &str) -> bool {
    matches!(
        function.to_ascii_lowercase().as_str(),
        "row_number" | "rank" | "dense_rank" | "ntile" | "lead" | "lag" | "nth_value"
    )
}

/// True for functions the aggregate renderer treats as grouped aggregates.
/// `n_distinct` is handled by `render_aggregate`, not by the dialect's
/// aggregate-name table, so it is matched explicitly.
fn is_aggregate_function(dialect: &dyn SqlDialect, function: &str) -> bool {
    function.eq_ignore_ascii_case("n")
        || function.eq_ignore_ascii_case("n_distinct")
        || dialect.translate_aggregate_function(function).is_some()
}

impl SqlGenerator {
    pub(crate) fn dialect(&self) -> &dyn SqlDialect {
        self.dialect.as_ref()
    }

    pub(crate) fn render_expression(
        &self,
        expr: &Expr,
        partition_by: &[String],
    ) -> GenerationResult<String> {
        let partition = partition_by
            .iter()
            .map(|name| self.dialect.quote_identifier(name))
            .collect::<Vec<_>>()
            .join(", ");
        self.generate_expression_context(expr, &partition, RenderContext::Window)
    }

    /// Renders an arbitrary expression inside a grouped aggregate projection.
    ///
    /// Aggregate math and scalar calls are allowed and recurse, but a window
    /// clause is never emitted: an aggregate here belongs to the enclosing
    /// GROUP BY, not to a window frame. Nested aggregates and window-only
    /// functions are rejected rather than silently mis-rendered.
    pub(crate) fn render_aggregate_expression(&self, expr: &Expr) -> GenerationResult<String> {
        self.generate_expression_context(expr, "", RenderContext::Aggregate)
    }

    /// Renders a bare aggregate call in aggregate context.
    ///
    /// R1-AC1: n_distinct keeps its existing NULL-inclusive expansion.
    pub(crate) fn render_aggregate_expression_call(
        &self,
        function: &str,
        args: &[Expr],
    ) -> GenerationResult<String> {
        let argument = match args {
            [] => None,
            [arg] => Some(arg),
            _ => {
                return Err(GenerationError::InvalidAst {
                    reason: format!("invalid arguments for {function}()"),
                })
            }
        };
        self.render_aggregate(function, argument)
    }

    pub(crate) fn render_aggregate(
        &self,
        function: &str,
        argument: Option<&Expr>,
    ) -> GenerationResult<String> {
        let is_n = function.eq_ignore_ascii_case("n");
        if is_n != argument.is_none() {
            return Err(GenerationError::InvalidAst {
                reason: format!(
                    "{function}() requires {} arguments",
                    if is_n { "zero" } else { "one" }
                ),
            });
        }
        let distinct = function.eq_ignore_ascii_case("n_distinct");
        let name = self
            .dialect
            .translate_aggregate_function(if distinct { "count" } else { function })
            .ok_or_else(|| GenerationError::UnsupportedAggregateFunction {
                function: function.to_string(),
                dialect: self.dialect.dialect_name().to_string(),
            })?;
        let argument = match argument {
            Some(expr) => {
                self.generate_expression_context(expr, "", RenderContext::AggregateInner)?
            }
            None => "*".to_string(),
        };
        if distinct {
            // R1-AC1: Preserve the existing NULL-inclusive n_distinct contract.
            Ok(format!("({name}(DISTINCT {argument}) + CASE WHEN {name}(*) > {name}({argument}) THEN 1 ELSE 0 END)"))
        } else {
            Ok(format!("{name}({argument})"))
        }
    }

    /// Creates a new SQL generator instance.
    ///
    /// # Arguments
    ///
    /// * `dialect` - The SQL dialect to use
    pub fn new(dialect: Box<dyn SqlDialect>) -> Self {
        Self { dialect }
    }

    /// Converts AST to SQL query.
    ///
    /// # Arguments
    ///
    /// * `ast` - The AST node to convert
    ///
    /// # Returns
    ///
    /// Returns SQL query string on success, GenerationError on failure.
    pub fn generate(&self, ast: &DplyrNode) -> GenerationResult<String> {
        match ast {
            DplyrNode::Pipeline {
                source,
                target,
                operations,
                ..
            } => self.generate_pipeline(source, target, operations),
            DplyrNode::DataSource { name, .. } => Ok(format!(
                "SELECT * FROM {}",
                self.dialect.quote_identifier(name)
            )),
        }
    }

    /// Converts pipeline to SQL.
    fn generate_pipeline(
        &self,
        source: &Option<String>,
        target: &Option<String>,
        operations: &[DplyrOperation],
    ) -> GenerationResult<String> {
        // Allow empty operations if we have a direct table assignment
        if operations.is_empty() && target.is_none() {
            return Err(GenerationError::InvalidAst {
                reason: "Empty pipeline: at least one operation is required".to_string(),
            });
        }

        let mut query_parts = QueryParts::new();
        let mut aggregation_group_by = None;

        // Get the source table name for join operations
        let source_table = source.as_deref().unwrap_or("data");

        // Process each operation in order
        for (index, operation) in operations.iter().enumerate() {
            if matches!(operation, DplyrOperation::Count { .. })
                && operations[index + 1..]
                    .iter()
                    .any(|next| !matches!(next, DplyrOperation::Arrange { .. }))
            {
                return Err(GenerationError::InvalidAst {
                    reason: "operations after count()/tally() require a subquery".to_string(),
                });
            }
            self.process_operation(operation, &mut query_parts, source_table)?;
            if matches!(
                operation,
                DplyrOperation::Summarise { .. } | DplyrOperation::Count { .. }
            ) {
                aggregation_group_by = if query_parts.group_by.is_empty() {
                    None
                } else {
                    Some(query_parts.group_by.clone())
                };
            }
        }

        query_parts.group_by = aggregation_group_by.unwrap_or_default();

        // Assemble final SQL query
        self.assemble_query(source, &query_parts)
    }

    /// Processes individual operations.
    fn process_operation(
        &self,
        operation: &DplyrOperation,
        query_parts: &mut QueryParts,
        source_table: &str,
    ) -> GenerationResult<()> {
        if query_parts.distinct {
            return Err(GenerationError::InvalidAst {
                reason: "operations after distinct() require a subquery".to_string(),
            });
        }

        // R1-AC1: Preserve stage order.
        // ponytail: reject extra query stages until subquery lowering is implemented.
        if query_parts.set_operation.is_some() {
            return Err(GenerationError::InvalidAst {
                reason: "operations after a set operation require a subquery".to_string(),
            });
        }
        if query_parts.has_aggregation
            && !matches!(
                operation,
                DplyrOperation::Arrange { .. } | DplyrOperation::GroupBy { .. }
            )
        {
            return Err(GenerationError::InvalidAst {
                reason: "operations after aggregation require a subquery".to_string(),
            });
        }

        match operation {
            DplyrOperation::Select { columns, .. } => {
                query_parts
                    .derived_columns
                    .extend(columns.iter().filter_map(|column| column.alias.clone()));
                query_parts.select_columns =
                    self.generate_select_columns_with_mutations(columns, query_parts)?;
            }
            DplyrOperation::Distinct { columns, .. } => {
                if !query_parts.group_by.is_empty() {
                    return Err(GenerationError::InvalidAst {
                        reason: "distinct() after group_by() requires a subquery".to_string(),
                    });
                }
                query_parts.distinct = true;
                if !columns.is_empty() {
                    let column_exprs: Vec<_> = columns
                        .iter()
                        .map(|column| ColumnExpr {
                            expr: Expr::Identifier(column.clone()),
                            alias: None,
                        })
                        .collect();
                    query_parts.select_columns =
                        self.generate_select_columns_with_mutations(&column_exprs, query_parts)?;
                }
            }
            DplyrOperation::Filter { condition, .. } => {
                let where_clause = self.generate_expression(condition)?;
                if query_parts.where_clauses.is_empty() {
                    query_parts.where_clauses.push(where_clause);
                } else {
                    query_parts
                        .where_clauses
                        .push(format!("AND ({where_clause})"));
                }
            }
            DplyrOperation::Mutate { assignments, .. } => {
                // Handle mutate operations - may need subqueries for complex cases
                self.process_mutate_operation(assignments, query_parts)?;
            }
            DplyrOperation::Rename { renames, .. } => {
                self.process_rename_operation(renames, query_parts)?;
            }
            DplyrOperation::Arrange { columns, .. } => {
                query_parts.order_by = self.generate_order_by(columns)?;
            }
            DplyrOperation::GroupBy { columns, .. } => {
                query_parts.group_columns = columns.clone();
                query_parts.group_by = columns
                    .iter()
                    .map(|col| self.dialect.quote_identifier(col))
                    .collect::<Vec<_>>()
                    .join(", ");
            }
            DplyrOperation::Summarise { aggregations, .. } => {
                let mut select_columns = Vec::new();
                if !query_parts.group_by.is_empty() {
                    select_columns.push(query_parts.group_by.clone());
                }
                select_columns.extend(self.generate_aggregations(aggregations)?);
                query_parts.select_columns = select_columns;
                query_parts.has_aggregation = true;
            }
            // R1-AC2: a compound summary cannot be expressed as one SELECT here:
            // QueryParts has no subquery lowering, so any later operation would
            // silently attach to the aggregated projection instead of the summary.
            // Only the schema-aware path can honour it.
            DplyrOperation::SummariseExpressions { .. } => {
                return Err(GenerationError::UnsupportedOperation {
                    operation: "summarise() with expressions; use a schema source".to_string(),
                    dialect: self.dialect.dialect_name().to_string(),
                });
            }
            DplyrOperation::Count { columns, .. } => {
                if !columns.is_empty() && !query_parts.joins.is_empty() {
                    return Err(GenerationError::InvalidAst {
                        reason: "count() keys after a join require qualified columns".to_string(),
                    });
                }

                let mut group_columns = query_parts.group_columns.clone();
                for column in columns {
                    if !group_columns.contains(column) {
                        group_columns.push(column.clone());
                    }
                }
                if group_columns.iter().any(|column| {
                    query_parts.mutated_columns.contains_key(column)
                        || query_parts.derived_columns.contains(column)
                }) {
                    return Err(GenerationError::InvalidAst {
                        reason: "computed count() keys require a subquery".to_string(),
                    });
                }

                let mut alias = "n".to_string();
                while group_columns.contains(&alias) {
                    alias.push('n');
                }
                query_parts.group_columns = group_columns;
                query_parts.group_by = query_parts
                    .group_columns
                    .iter()
                    .map(|column| self.dialect.quote_identifier(column))
                    .collect::<Vec<_>>()
                    .join(", ");

                let mut select_columns = Vec::new();
                if !query_parts.group_by.is_empty() {
                    select_columns.push(query_parts.group_by.clone());
                }
                select_columns.extend(self.generate_aggregations(&[Aggregation {
                    function: "n".to_string(),
                    column: String::new(),
                    alias: Some(alias),
                }])?);
                query_parts.select_columns = select_columns;
                query_parts.order_by.clear();
                query_parts.has_aggregation = true;
            }
            DplyrOperation::Join {
                join_type, spec, ..
            } => {
                self.process_join_operation(join_type, spec, query_parts, source_table)?;
            }
            DplyrOperation::SetOp {
                operation,
                right_table,
                ..
            } => {
                let set_op_sql = match operation {
                    SetOperation::Intersect => "INTERSECT",
                    SetOperation::Union => "UNION",
                    SetOperation::SetDiff => "EXCEPT",
                };
                query_parts.set_operation = Some((set_op_sql.to_string(), right_table.clone()));
            }
        }
        Ok(())
    }

    fn process_rename_operation(
        &self,
        renames: &[RenameSpec],
        query_parts: &mut QueryParts,
    ) -> GenerationResult<()> {
        if renames.is_empty() {
            return Err(GenerationError::InvalidAst {
                reason: "rename() requires at least one mapping".to_string(),
            });
        }

        let excluded = renames
            .iter()
            .map(|spec| spec.old_name.clone())
            .collect::<Vec<_>>();

        let star_exclude = self.dialect.select_star_exclude(&excluded).ok_or_else(|| {
            GenerationError::UnsupportedOperation {
                operation: "rename".to_string(),
                dialect: self.dialect.dialect_name().to_string(),
            }
        })?;

        if query_parts.select_columns.is_empty() {
            query_parts.select_columns.push(star_exclude);
        } else {
            let mut replaced_star = false;
            for col in &mut query_parts.select_columns {
                if col == "*" {
                    *col = star_exclude.clone();
                    replaced_star = true;
                }
            }
            if !replaced_star {
                return Err(GenerationError::InvalidAst {
                    reason:
                        "rename() currently requires an implicit '*' projection (no prior select())"
                            .to_string(),
                });
            }
        }

        for spec in renames {
            query_parts.derived_columns.insert(spec.new_name.clone());
            query_parts.select_columns.push(format!(
                "{} AS {}",
                self.dialect.quote_identifier(&spec.old_name),
                self.dialect.quote_identifier(&spec.new_name)
            ));
        }

        Ok(())
    }

    fn process_join_operation(
        &self,
        join_type: &JoinType,
        spec: &JoinSpec,
        query_parts: &mut QueryParts,
        source_table: &str,
    ) -> GenerationResult<()> {
        use crate::parser::JoinType;

        // Check if dialect supports SEMI/ANTI JOIN natively (DuckDB only)
        let is_duckdb = self.dialect.dialect_name() == "duckdb";

        let condition = if !spec.by.is_empty() {
            spec.by
                .iter()
                .map(|key| {
                    format!(
                        "{} = {}",
                        self.dialect
                            .quote_identifier_path(&[source_table, &key.left]),
                        self.dialect
                            .quote_identifier_path(&[&spec.table, &key.right])
                    )
                })
                .collect::<Vec<_>>()
                .join(" AND ")
        } else if let Some(expr) = &spec.on_expr {
            self.generate_expression(expr)?
        } else {
            return Err(GenerationError::InvalidAst {
                reason: "join operation requires either 'by' parameter or 'on' condition"
                    .to_string(),
            });
        };

        // For SEMI and ANTI joins, non-DuckDB dialects need subquery transformation
        match join_type {
            JoinType::Semi | JoinType::Anti if !is_duckdb => {
                // Generate EXISTS/NOT EXISTS subquery for non-DuckDB dialects
                let exists_keyword = match join_type {
                    JoinType::Semi => "EXISTS",
                    JoinType::Anti => "NOT EXISTS",
                    _ => unreachable!(),
                };

                // Create subquery: WHERE (NOT) EXISTS (SELECT 1 FROM right_table ON condition)
                let subquery = format!(
                    "{exists_keyword} (SELECT 1 FROM {} WHERE {condition})",
                    self.dialect.quote_identifier(&spec.table)
                );

                // Add as WHERE clause (SEMI/ANTI don't need actual JOIN)
                if query_parts.where_clauses.is_empty() {
                    query_parts.where_clauses.push(subquery);
                } else {
                    query_parts.where_clauses.push(format!("AND ({subquery})"));
                }

                return Ok(());
            }
            _ => {}
        }

        // For DuckDB or standard joins, use native JOIN syntax
        let join_sql = match join_type {
            JoinType::Inner => "INNER JOIN",
            JoinType::Left => "LEFT JOIN",
            JoinType::Right => "RIGHT JOIN",
            JoinType::Full => "FULL JOIN",
            JoinType::Semi => "SEMI JOIN",
            JoinType::Anti => "ANTI JOIN",
        };

        query_parts.joins.push(format!(
            "{} {} ON {}",
            join_sql,
            self.dialect.quote_identifier(&spec.table),
            condition
        ));

        Ok(())
    }

    /// Generates ORDER BY clause.
    fn generate_order_by(&self, columns: &[OrderExpr]) -> GenerationResult<String> {
        let order_items: Result<Vec<_>, _> = columns
            .iter()
            .map(|col| {
                let direction = match col.direction {
                    OrderDirection::Asc => "ASC",
                    OrderDirection::Desc => "DESC",
                };
                Ok(format!(
                    "{} {}",
                    self.dialect.quote_identifier(&col.column),
                    direction
                ))
            })
            .collect();

        Ok(order_items?.join(", "))
    }

    /// Generates aggregate functions.
    fn generate_aggregations(&self, aggregations: &[Aggregation]) -> GenerationResult<Vec<String>> {
        aggregations
            .iter()
            .map(|agg| {
                let is_n_distinct = agg.function.eq_ignore_ascii_case("n_distinct");
                if is_n_distinct && agg.column.is_empty() {
                    return Err(GenerationError::InvalidAst {
                        reason: "n_distinct() requires a column".to_string(),
                    });
                }
                let func_name = self
                    .dialect
                    .translate_aggregate_function(if is_n_distinct {
                        "count"
                    } else {
                        &agg.function
                    })
                    .ok_or_else(|| GenerationError::UnsupportedAggregateFunction {
                        function: agg.function.clone(),
                        dialect: self.dialect.dialect_name().to_string(),
                    })?;
                let column_ref = if agg.function.to_lowercase() == "n" {
                    "*".to_string()
                } else {
                    self.dialect.quote_identifier(&agg.column)
                };

                let expr = if is_n_distinct {
                    format!(
                        "{func_name}(DISTINCT {column_ref}) + CASE WHEN {func_name}(*) > {func_name}({column_ref}) THEN 1 ELSE 0 END"
                    )
                } else {
                    format!("{func_name}({column_ref})")
                };

                if let Some(alias) = &agg.alias {
                    Ok(format!(
                        "{} AS {}",
                        expr,
                        self.dialect.quote_identifier(alias)
                    ))
                } else {
                    Ok(expr)
                }
            })
            .collect()
    }

    /// Converts expressions to SQL.
    fn generate_expression(&self, expr: &Expr) -> GenerationResult<String> {
        self.generate_expression_with_window_partition(expr, "")
    }

    fn generate_expression_with_window_partition(
        &self,
        expr: &Expr,
        partition_by: &str,
    ) -> GenerationResult<String> {
        self.generate_expression_context(expr, partition_by, RenderContext::Scalar)
    }

    fn generate_expression_context(
        &self,
        expr: &Expr,
        partition_by: &str,
        context: RenderContext,
    ) -> GenerationResult<String> {
        match expr {
            Expr::Identifier(name) => Ok(self.dialect.quote_identifier(name)),
            Expr::Literal(literal) => self.generate_literal(literal),
            Expr::Binary {
                left,
                operator,
                right,
            } => {
                let left_sql = self.generate_expression_context(left, partition_by, context)?;
                let right_sql = self.generate_expression_context(right, partition_by, context)?;
                // R sums divide as floating point; SQL divides integers as
                // integers. Only the aggregate/window paths need this, and
                // legacy scalar SQL is left byte-identical.
                if matches!(operator, BinaryOp::Divide) && !matches!(context, RenderContext::Scalar)
                {
                    return Ok(format!("(({left_sql} * 1.0) / {right_sql})"));
                }
                let op_sql = self.generate_binary_operator(operator);
                Ok(format!("({left_sql} {op_sql} {right_sql})"))
            }
            Expr::Function { name, args } => {
                self.generate_function_expression_context(name, args, partition_by, context)
            }
            Expr::CaseWhen { branches, default } => {
                let mut sql = String::from("CASE");
                for (condition, value) in branches {
                    let condition_sql =
                        self.generate_expression_context(condition, partition_by, context)?;
                    let value_sql =
                        self.generate_expression_context(value, partition_by, context)?;
                    sql.push_str(&format!(" WHEN {condition_sql} THEN {value_sql}"));
                }
                let default_sql = match default {
                    Some(expr) => self.generate_expression_context(expr, partition_by, context)?,
                    None => "NULL".to_string(),
                };
                sql.push_str(&format!(" ELSE {default_sql} END"));
                Ok(sql)
            }
            Expr::NamedArg { name, .. } => Err(GenerationError::InvalidAst {
                reason: format!("named argument '{name}' cannot be used outside a function call"),
            }),
        }
    }

    fn generate_function_expression_context(
        &self,
        name: &str,
        args: &[Expr],
        partition_by: &str,
        context: RenderContext,
    ) -> GenerationResult<String> {
        if name.eq_ignore_ascii_case("paste") {
            return self.generate_paste_expression_context(name, args, partition_by, context);
        }

        let is_aggregate = is_aggregate_function(self.dialect.as_ref(), name);
        // Legacy scalar rendering (mutate/filter) keeps its original fallthrough
        // so an out-of-context aggregate still reports the dialect's error.
        if is_aggregate && context == RenderContext::AggregateInner {
            return Err(GenerationError::InvalidAst {
                reason: format!("aggregate {name}() cannot be nested inside another aggregate"),
            });
        }
        if is_aggregate && context != RenderContext::Scalar {
            let aggregate = self.render_aggregate_expression_call(name, args)?;
            return match context {
                RenderContext::Aggregate | RenderContext::AggregateInner => Ok(aggregate),
                RenderContext::Window | RenderContext::Scalar => {
                    let window = if partition_by.is_empty() {
                        String::new()
                    } else {
                        format!("PARTITION BY {partition_by}")
                    };
                    Ok(format!("{aggregate} OVER ({window})"))
                }
            };
        }

        // Window-only functions have no meaning inside a grouped aggregate, and
        // translating them here would drop the frame rather than report the
        // mismatch. AggregateInner is included so SUM(lag(x)) is rejected too.
        if matches!(
            context,
            RenderContext::Aggregate | RenderContext::AggregateInner
        ) && is_window_only_function(name)
        {
            return Err(GenerationError::InvalidAst {
                reason: format!("window function {name}() is not allowed in a grouped aggregate"),
            });
        }

        let args_str =
            self.generate_function_arguments_context(name, args, partition_by, context)?;

        if let Some(translated) =
            self.dialect
                .translate_function_with_window_partition(name, &args_str, partition_by)
        {
            return Ok(translated);
        }

        Err(GenerationError::UnsupportedFunction {
            function: name.to_string(),
            dialect: self.dialect.dialect_name().to_string(),
        })
    }

    fn generate_function_arguments_context(
        &self,
        function: &str,
        args: &[Expr],
        partition_by: &str,
        context: RenderContext,
    ) -> GenerationResult<Vec<String>> {
        let has_named_args = args.iter().any(|arg| matches!(arg, Expr::NamedArg { .. }));
        if !has_named_args {
            return args
                .iter()
                .map(|arg| self.generate_expression_context(arg, partition_by, context))
                .collect();
        }

        let formals = named_argument_formals(function).ok_or_else(|| {
            GenerationError::UnsupportedNamedArgument {
                function: function.to_string(),
                argument: args
                    .iter()
                    .find_map(|arg| match arg {
                        Expr::NamedArg { name, .. } => Some(name.to_string()),
                        _ => None,
                    })
                    .unwrap_or_default(),
                dialect: self.dialect.dialect_name().to_string(),
            }
        })?;

        let mut slots = vec![None::<String>; formals.len()];
        let mut overflow = Vec::new();
        let mut next_positional = 0;

        for arg in args {
            match arg {
                Expr::NamedArg { name, value } => {
                    let Some(index) = formals
                        .iter()
                        .position(|formal| formal.name.eq_ignore_ascii_case(name))
                    else {
                        return Err(GenerationError::UnsupportedNamedArgument {
                            function: function.to_string(),
                            argument: name.to_string(),
                            dialect: self.dialect.dialect_name().to_string(),
                        });
                    };

                    if slots[index].is_some() {
                        return Err(GenerationError::InvalidAst {
                            reason: format!(
                                "duplicate argument '{name}' for function '{function}'"
                            ),
                        });
                    }

                    slots[index] =
                        Some(self.generate_expression_context(value, partition_by, context)?);
                }
                _ => {
                    let sql = self.generate_expression_context(arg, partition_by, context)?;
                    while next_positional < slots.len() && slots[next_positional].is_some() {
                        next_positional += 1;
                    }
                    if next_positional < slots.len() {
                        slots[next_positional] = Some(sql);
                        next_positional += 1;
                    } else {
                        overflow.push(sql);
                    }
                }
            }
        }

        let last_explicit = slots.iter().rposition(Option::is_some);
        let mut normalized = Vec::new();
        if let Some(last_explicit) = last_explicit {
            for index in 0..=last_explicit {
                if let Some(sql) = slots[index].take() {
                    normalized.push(sql);
                } else if let Some(default_sql) = formals[index].default_sql {
                    normalized.push(default_sql.to_string());
                } else {
                    return Err(GenerationError::InvalidAst {
                        reason: format!(
                            "named argument for function '{function}' requires preceding argument '{}'",
                            formals[index].name
                        ),
                    });
                }
            }
        }
        normalized.extend(overflow);

        Ok(normalized)
    }

    fn generate_paste_expression_context(
        &self,
        name: &str,
        args: &[Expr],
        partition_by: &str,
        context: RenderContext,
    ) -> GenerationResult<String> {
        let mut positional_args = Vec::new();
        let mut separator = self.dialect.quote_string(" ");
        let mut seen_separator = false;

        for arg in args {
            match arg {
                Expr::NamedArg {
                    name: arg_name,
                    value,
                } if arg_name.eq_ignore_ascii_case("sep") => {
                    if seen_separator {
                        return Err(GenerationError::UnsupportedFunction {
                            function: name.to_string(),
                            dialect: self.dialect.dialect_name().to_string(),
                        });
                    }
                    separator = self.generate_expression_context(value, partition_by, context)?;
                    seen_separator = true;
                }
                Expr::NamedArg { name: arg_name, .. } => {
                    return Err(GenerationError::UnsupportedNamedArgument {
                        function: name.to_string(),
                        argument: arg_name.to_string(),
                        dialect: self.dialect.dialect_name().to_string(),
                    });
                }
                _ => positional_args.push(self.generate_expression_context(
                    arg,
                    partition_by,
                    context,
                )?),
            }
        }

        self.dialect
            .concat_with_separator(&separator, &positional_args)
            .ok_or_else(|| GenerationError::UnsupportedFunction {
                function: name.to_string(),
                dialect: self.dialect.dialect_name().to_string(),
            })
    }

    /// Converts literal values to SQL.
    fn generate_literal(&self, literal: &LiteralValue) -> GenerationResult<String> {
        match literal {
            LiteralValue::String(s) => Ok(self.dialect.quote_string(s)),
            LiteralValue::Number(n) => Ok(n.to_string()),
            LiteralValue::Boolean(b) => Ok(if *b {
                "TRUE".to_string()
            } else {
                "FALSE".to_string()
            }),
            LiteralValue::Null => Ok("NULL".to_string()),
        }
    }

    /// Converts binary operators to SQL.
    const fn generate_binary_operator(&self, operator: &BinaryOp) -> &'static str {
        match operator {
            BinaryOp::Equal => "=",
            BinaryOp::NotEqual => "!=",
            BinaryOp::LessThan => "<",
            BinaryOp::LessThanOrEqual => "<=",
            BinaryOp::GreaterThan => ">",
            BinaryOp::GreaterThanOrEqual => ">=",
            BinaryOp::And => "AND",
            BinaryOp::Or => "OR",
            BinaryOp::Plus => "+",
            BinaryOp::Minus => "-",
            BinaryOp::Multiply => "*",
            BinaryOp::Divide => "/",
        }
    }
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
