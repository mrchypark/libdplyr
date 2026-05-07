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
        _ => None,
    }
}

impl SqlGenerator {
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
        for operation in operations {
            self.process_operation(operation, &mut query_parts, source_table)?;
            if matches!(operation, DplyrOperation::Summarise { .. }) {
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
        match operation {
            DplyrOperation::Select { columns, .. } => {
                query_parts.select_columns =
                    self.generate_select_columns_with_mutations(columns, query_parts)?;
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
                query_parts.group_by = columns
                    .iter()
                    .map(|col| self.dialect.quote_identifier(col))
                    .collect::<Vec<_>>()
                    .join(", ");
            }
            DplyrOperation::Summarise { aggregations, .. } => {
                query_parts.select_columns = self.generate_aggregations(aggregations)?;
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

        // For SEMI and ANTI joins, non-DuckDB dialects need subquery transformation
        match join_type {
            JoinType::Semi | JoinType::Anti if !is_duckdb => {
                // Generate EXISTS/NOT EXISTS subquery for non-DuckDB dialects
                let exists_keyword = match join_type {
                    JoinType::Semi => "EXISTS",
                    JoinType::Anti => "NOT EXISTS",
                    _ => unreachable!(),
                };

                // Generate the condition
                let condition = if let Some(by_column) = &spec.by_column {
                    format!(
                        "{} = {}",
                        self.dialect
                            .quote_identifier(&format!("{}.{}", source_table, by_column)),
                        self.dialect
                            .quote_identifier(&format!("{}.{}", spec.table, by_column))
                    )
                } else if let Some(expr) = &spec.on_expr {
                    self.generate_expression(expr)?
                } else {
                    return Err(GenerationError::InvalidAst {
                        reason: "join operation requires either 'by' parameter or 'on' condition"
                            .to_string(),
                    });
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

        // Generate ON clause based on join specification
        let on_clause = if let Some(by_column) = &spec.by_column {
            // by = "column_name" -> ON "source"."column" = "right_table"."column"
            format!(
                "{} = {}",
                self.dialect
                    .quote_identifier(&format!("{}.{}", source_table, by_column)),
                self.dialect
                    .quote_identifier(&format!("{}.{}", spec.table, by_column))
            )
        } else if let Some(expr) = &spec.on_expr {
            // Fallback to expression-based ON clause
            self.generate_expression(expr)?
        } else {
            // No join condition specified
            return Err(GenerationError::InvalidAst {
                reason: "join operation requires either 'by' parameter or 'on' condition"
                    .to_string(),
            });
        };

        query_parts.joins.push(format!(
            "{} {} ON {}",
            join_sql,
            self.dialect.quote_identifier(&spec.table),
            on_clause
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
                let func_name = self
                    .dialect
                    .translate_aggregate_function(&agg.function)
                    .ok_or_else(|| GenerationError::UnsupportedAggregateFunction {
                        function: agg.function.clone(),
                        dialect: self.dialect.dialect_name().to_string(),
                    })?;
                let column_ref = if agg.function.to_lowercase() == "n" {
                    "*".to_string()
                } else {
                    self.dialect.quote_identifier(&agg.column)
                };

                let expr = format!("{func_name}({column_ref})");

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
        match expr {
            Expr::Identifier(name) => Ok(self.dialect.quote_identifier(name)),
            Expr::Literal(literal) => self.generate_literal(literal),
            Expr::Binary {
                left,
                operator,
                right,
            } => {
                let left_sql =
                    self.generate_expression_with_window_partition(left, partition_by)?;
                let right_sql =
                    self.generate_expression_with_window_partition(right, partition_by)?;
                let op_sql = self.generate_binary_operator(operator);
                Ok(format!("({left_sql} {op_sql} {right_sql})"))
            }
            Expr::Function { name, args } => {
                self.generate_function_expression_with_window_partition(name, args, partition_by)
            }
            Expr::NamedArg { name, .. } => Err(GenerationError::InvalidAst {
                reason: format!("named argument '{name}' cannot be used outside a function call"),
            }),
        }
    }

    fn generate_function_expression_with_window_partition(
        &self,
        name: &str,
        args: &[Expr],
        partition_by: &str,
    ) -> GenerationResult<String> {
        if name.eq_ignore_ascii_case("paste") {
            return self.generate_paste_expression_with_window_partition(name, args, partition_by);
        }

        let args_str =
            self.generate_function_arguments_with_window_partition(name, args, partition_by)?;

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

    fn generate_function_arguments_with_window_partition(
        &self,
        function: &str,
        args: &[Expr],
        partition_by: &str,
    ) -> GenerationResult<Vec<String>> {
        let has_named_args = args.iter().any(|arg| matches!(arg, Expr::NamedArg { .. }));
        if !has_named_args {
            return args
                .iter()
                .map(|arg| self.generate_expression_with_window_partition(arg, partition_by))
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
                        Some(self.generate_expression_with_window_partition(value, partition_by)?);
                }
                _ => {
                    let sql = self.generate_expression_with_window_partition(arg, partition_by)?;
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

    fn generate_paste_expression_with_window_partition(
        &self,
        name: &str,
        args: &[Expr],
        partition_by: &str,
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
                    separator =
                        self.generate_expression_with_window_partition(value, partition_by)?;
                    seen_separator = true;
                }
                Expr::NamedArg { name: arg_name, .. } => {
                    return Err(GenerationError::UnsupportedNamedArgument {
                        function: name.to_string(),
                        argument: arg_name.to_string(),
                        dialect: self.dialect.dialect_name().to_string(),
                    });
                }
                _ => positional_args
                    .push(self.generate_expression_with_window_partition(arg, partition_by)?),
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
