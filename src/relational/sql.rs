//! Minimal structured SQL AST for the schema-aware relational compiler.
//!
//! Additive by design: the existing generator in crate::sql_generator stays
//! authoritative until the parent wires this module in.

use crate::error::{GenerationError, GenerationResult};
use crate::parser::{Expr, JoinType, OrderDirection, OrderExpr, SetOperation};
use crate::sql_generator::SqlGenerator;

/// Fixed aliases for the two branches of a structured join.
const LEFT_ALIAS: &str = "__libdplyr_left";
const RIGHT_ALIAS: &str = "__libdplyr_right";

/// Where a query reads rows from.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SqlSource {
    /// A base table, quoted with the active dialect.
    Table(String),
    /// A derived table rendered from a nested query plus its alias.
    Subquery(Box<SqlQuery>, String),
    /// Two derived branches combined by join type on equality keys.
    Join {
        left: Box<SqlQuery>,
        right: Box<SqlQuery>,
        join_type: JoinType,
        keys: Vec<(String, String)>,
    },
    /// Two derived branches combined by a set operation, then re-projected.
    Set {
        left: Box<SqlQuery>,
        right: Box<SqlQuery>,
        operation: SetOperation,
    },
}

/// A single SELECT statement.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SqlQuery {
    pub(crate) source: SqlSource,
    pub(crate) projection: Vec<SelectItem>,
    pub(crate) filter: Option<Expr>,
    pub(crate) group_by: Vec<String>,
    pub(crate) order_by: Vec<OrderExpr>,
    pub(crate) distinct: bool,
}

/// A projected expression together with the output name it must carry.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SelectItem {
    pub(crate) expression: SelectExpression,
    pub(crate) alias: String,
}

/// What a projection item computes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SelectExpression {
    /// A scalar expression, optionally evaluated per window partition.
    Scalar {
        expr: Expr,
        partition_by: Vec<String>,
    },
    /// A grouped aggregate built from an arbitrary expression, e.g. `sum(x) / n()`.
    AggregateExpression(Expr),
    /// A column reference qualified by a relation alias.
    Qualified { relation: String, column: String },
    /// `COALESCE(left, right)`, used to coalesce two same-named join keys.
    Coalesce {
        left: (String, String),
        right: (String, String),
    },
}

impl SqlQuery {
    /// Renders this query to SQL through the generator's dialect and
    /// expression rendering. Projections are always explicit and always
    /// aliased, so every derived table has a stable column set.
    pub(crate) fn render(&self, generator: &SqlGenerator) -> GenerationResult<String> {
        if self.projection.is_empty() {
            return Err(GenerationError::InvalidAst {
                reason: "projection must name at least one column".to_string(),
            });
        }

        let dialect = generator.dialect();
        let mut sql = String::from("SELECT ");
        if self.distinct {
            sql.push_str("DISTINCT ");
        }

        let mut items = Vec::with_capacity(self.projection.len());
        for item in &self.projection {
            let rendered = match &item.expression {
                SelectExpression::Scalar { expr, partition_by } => {
                    generator.render_expression(expr, partition_by)?
                }
                SelectExpression::AggregateExpression(expr) => {
                    generator.render_aggregate_expression(expr)?
                }
                SelectExpression::Qualified { relation, column } => {
                    dialect.quote_identifier_path(&[relation, column])
                }
                SelectExpression::Coalesce { left, right } => format!(
                    "COALESCE({}, {})",
                    dialect.quote_identifier_path(&[&left.0, &left.1]),
                    dialect.quote_identifier_path(&[&right.0, &right.1])
                ),
            };
            items.push(format!(
                "{rendered} AS {}",
                dialect.quote_identifier(&item.alias)
            ));
        }
        sql.push_str(&items.join(", "));

        sql.push_str("\nFROM ");
        let mut join_predicate = None;
        match &self.source {
            SqlSource::Table(table) => sql.push_str(&dialect.quote_identifier(table)),
            SqlSource::Subquery(inner, alias) => {
                sql.push('(');
                sql.push_str(&inner.render(generator)?);
                sql.push_str(&format!(") AS {}", dialect.quote_identifier(alias)));
            }
            SqlSource::Join {
                left,
                right,
                join_type,
                keys,
            } => {
                join_predicate =
                    self.render_join(generator, left, right, join_type, keys, &mut sql)?
            }
            SqlSource::Set {
                left,
                right,
                operation,
            } => self.render_set(generator, left, right, operation, &mut sql)?,
        }

        // Semi/anti joins filter the left branch, so their EXISTS predicate is
        // ANDed with this query's own filter instead of becoming a second WHERE.
        let mut predicates = Vec::new();
        if let Some(predicate) = join_predicate {
            predicates.push(predicate);
        }
        if let Some(filter) = &self.filter {
            predicates.push(generator.render_expression(filter, &[])?);
        }
        if !predicates.is_empty() {
            sql.push_str("\nWHERE ");
            sql.push_str(&predicates.join(" AND "));
        }

        if !self.group_by.is_empty() {
            sql.push_str("\nGROUP BY ");
            let columns = self
                .group_by
                .iter()
                .map(|column| dialect.quote_identifier(column))
                .collect::<Vec<_>>()
                .join(", ");
            sql.push_str(&columns);
        }

        if !self.order_by.is_empty() {
            sql.push_str("\nORDER BY ");
            let terms = self
                .order_by
                .iter()
                .map(|item| {
                    let direction = match item.direction {
                        OrderDirection::Asc => "ASC",
                        OrderDirection::Desc => "DESC",
                    };
                    format!("{} {direction}", dialect.quote_identifier(&item.column))
                })
                .collect::<Vec<_>>()
                .join(", ");
            sql.push_str(&terms);
        }

        Ok(sql)
    }

    /// Renders a join source. Branch projections are wrapped in aliased
    /// subqueries so every key reference can be qualified, which keeps
    /// same-named columns on both sides unambiguous.
    #[allow(clippy::too_many_arguments)]
    fn render_join(
        &self,
        generator: &SqlGenerator,
        left: &SqlQuery,
        right: &SqlQuery,
        join_type: &JoinType,
        keys: &[(String, String)],
        sql: &mut String,
    ) -> GenerationResult<Option<String>> {
        let dialect = generator.dialect();
        if keys.is_empty() {
            return Err(GenerationError::InvalidAst {
                reason: "join requires at least one equality key".to_string(),
            });
        }
        // ponytail: FULL JOIN is rejected for dialects without native support;
        // add a UNION-based emulation only if a caller needs it.
        if matches!(join_type, JoinType::Full) && dialect.dialect_name() == "mysql" {
            return Err(GenerationError::UnsupportedOperation {
                operation: "full join".to_string(),
                dialect: dialect.dialect_name().to_string(),
            });
        }

        let left_sql = left.render(generator)?;
        let right_sql = right.render(generator)?;

        // Semi and anti keep left cardinality, so they filter the left branch
        // with EXISTS rather than joining (which would duplicate left rows).
        let exists_keyword = match join_type {
            JoinType::Semi => Some("EXISTS"),
            JoinType::Anti => Some("NOT EXISTS"),
            _ => None,
        };
        if let Some(keyword) = exists_keyword {
            let predicate = render_join_predicate(generator, keys)?;
            // Only the left branch enters the outer FROM; the right branch is
            // re-rendered inside the EXISTS.
            sql.push_str(&format!(
                "({left_sql}) AS {}",
                dialect.quote_identifier(LEFT_ALIAS)
            ));
            return Ok(Some(format!(
                "{keyword} (SELECT 1 FROM ({right_sql}) AS {} WHERE {predicate})",
                dialect.quote_identifier(RIGHT_ALIAS)
            )));
        }

        let keyword = match join_type {
            JoinType::Inner => "INNER JOIN",
            JoinType::Left => "LEFT JOIN",
            JoinType::Right => "RIGHT JOIN",
            JoinType::Full => "FULL JOIN",
            JoinType::Semi | JoinType::Anti => unreachable!("handled above"),
        };
        let predicate = render_join_predicate(generator, keys)?;
        sql.push_str(&format!(
            "({left_sql}) AS {}\n{keyword} ({right_sql}) AS {} ON {predicate}",
            dialect.quote_identifier(LEFT_ALIAS),
            dialect.quote_identifier(RIGHT_ALIAS)
        ));
        Ok(None)
    }

    /// Renders a set operation. Both branches are aliased subqueries so the
    /// outer projection can name output columns and align them by name.
    fn render_set(
        &self,
        generator: &SqlGenerator,
        left: &SqlQuery,
        right: &SqlQuery,
        operation: &SetOperation,
        sql: &mut String,
    ) -> GenerationResult<()> {
        let dialect = generator.dialect();
        let left_sql = left.render(generator)?;
        let right_sql = right.render(generator)?;
        let keyword = match operation {
            SetOperation::Intersect => "INTERSECT",
            SetOperation::Union => "UNION",
            SetOperation::SetDiff => "EXCEPT",
        };
        // The union is wrapped as one derived table so the outer projection can
        // reference its output columns. Aliasing a branch individually here is
        // not possible: `FROM (x) UNION (y)` is a syntax error.
        sql.push_str(&format!(
            "(\n{left_sql}\n{keyword}\n{right_sql}\n) AS {}",
            dialect.quote_identifier(LEFT_ALIAS)
        ));
        Ok(())
    }
}

/// Builds the equality predicate joining the two fixed branch aliases.
/// `=` is used directly, so NULL keys never match, matching SQL join semantics.
fn render_join_predicate(
    generator: &SqlGenerator,
    keys: &[(String, String)],
) -> GenerationResult<String> {
    let dialect = generator.dialect();
    Ok(keys
        .iter()
        .map(|(left, right)| {
            format!(
                "{} = {}",
                dialect.quote_identifier_path(&[LEFT_ALIAS, left]),
                dialect.quote_identifier_path(&[RIGHT_ALIAS, right])
            )
        })
        .collect::<Vec<_>>()
        .join(" AND "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{BinaryOp, LiteralValue};
    use crate::sql_generator::{DuckDbDialect, SqlGenerator};

    fn generator() -> SqlGenerator {
        SqlGenerator::new(Box::new(DuckDbDialect::new()))
    }

    fn scalar(expr: Expr, alias: &str) -> SelectItem {
        SelectItem {
            expression: SelectExpression::Scalar {
                expr,
                partition_by: Vec::new(),
            },
            alias: alias.to_string(),
        }
    }

    fn query(source: SqlSource, projection: Vec<SelectItem>) -> SqlQuery {
        SqlQuery {
            source,
            projection,
            filter: None,
            group_by: Vec::new(),
            order_by: Vec::new(),
            distinct: false,
        }
    }

    #[test]
    fn renders_table_projection_and_filter() {
        let mut q = query(
            SqlSource::Table("users".to_string()),
            vec![scalar(Expr::Identifier("user id".to_string()), "user id")],
        );
        q.filter = Some(Expr::Binary {
            left: Box::new(Expr::Identifier("user id".to_string())),
            operator: BinaryOp::GreaterThan,
            right: Box::new(Expr::Literal(LiteralValue::Number(10.0))),
        });

        assert_eq!(
            q.render(&generator()).unwrap(),
            "SELECT \"user id\" AS \"user id\"\nFROM \"users\"\nWHERE (\"user id\" > 10)"
        );
    }

    #[test]
    fn renders_nested_subquery_with_quoted_alias() {
        let mut inner = query(
            SqlSource::Table("users".to_string()),
            vec![scalar(Expr::Identifier("id".to_string()), "id")],
        );
        inner.distinct = true;

        let q = query(
            SqlSource::Subquery(Box::new(inner), "inner users".to_string()),
            vec![scalar(Expr::Identifier("id".to_string()), "id")],
        );

        assert_eq!(
            q.render(&generator()).unwrap(),
            "SELECT \"id\" AS \"id\"\nFROM (SELECT DISTINCT \"id\" AS \"id\"\nFROM \"users\") AS \"inner users\""
        );
    }

    #[test]
    fn renders_aggregate_group_and_sort() {
        let mut q = query(
            SqlSource::Table("orders".to_string()),
            vec![
                scalar(Expr::Identifier("region".to_string()), "region"),
                SelectItem {
                    expression: SelectExpression::AggregateExpression(Expr::Function {
                        name: "sum".to_string(),
                        args: vec![Expr::Identifier("amount".to_string())],
                    }),
                    alias: "total".to_string(),
                },
            ],
        );
        q.group_by = vec!["region".to_string()];
        q.order_by = vec![OrderExpr {
            column: "total".to_string(),
            direction: OrderDirection::Desc,
        }];

        assert_eq!(
            q.render(&generator()).unwrap(),
            "SELECT \"region\" AS \"region\", SUM(\"amount\") AS \"total\"\nFROM \"orders\"\nGROUP BY \"region\"\nORDER BY \"total\" DESC"
        );
    }

    #[test]
    fn rejects_empty_projection() {
        let q = query(SqlSource::Table("users".to_string()), Vec::new());

        assert!(q.render(&generator()).is_err());
    }

    fn branch(table: &str, columns: &[&str]) -> SqlQuery {
        query(
            SqlSource::Table(table.to_string()),
            columns
                .iter()
                .map(|column| scalar(Expr::Identifier((*column).to_string()), column))
                .collect(),
        )
    }

    #[test]
    fn renders_inner_join_on_equality_keys() {
        let q = query(
            SqlSource::Join {
                left: Box::new(branch("orders", &["id", "amount"])),
                right: Box::new(branch("users", &["id", "name"])),
                join_type: JoinType::Inner,
                keys: vec![("id".to_string(), "id".to_string())],
            },
            vec![SelectItem {
                alias: "amount".to_string(),
                expression: SelectExpression::Qualified {
                    relation: LEFT_ALIAS.to_string(),
                    column: "amount".to_string(),
                },
            }],
        );

        assert_eq!(
            q.render(&generator()).unwrap(),
            concat!(
                "SELECT \"__libdplyr_left\".\"amount\" AS \"amount\"\n",
                "FROM (SELECT \"id\" AS \"id\", \"amount\" AS \"amount\"\n",
                "FROM \"orders\") AS \"__libdplyr_left\"\n",
                "INNER JOIN (SELECT \"id\" AS \"id\", \"name\" AS \"name\"\n",
                "FROM \"users\") AS \"__libdplyr_right\" ",
                "ON \"__libdplyr_left\".\"id\" = \"__libdplyr_right\".\"id\""
            )
        );
    }

    #[test]
    fn semi_join_keeps_left_cardinality_via_exists() {
        let q = query(
            SqlSource::Join {
                left: Box::new(branch("orders", &["id"])),
                right: Box::new(branch("users", &["id"])),
                join_type: JoinType::Semi,
                keys: vec![("id".to_string(), "id".to_string())],
            },
            vec![SelectItem {
                alias: "id".to_string(),
                expression: SelectExpression::Qualified {
                    relation: LEFT_ALIAS.to_string(),
                    column: "id".to_string(),
                },
            }],
        );

        let sql = q.render(&generator()).unwrap();
        // Left branch alone in FROM; right branch only inside EXISTS.
        assert!(
            sql.contains("AS \"__libdplyr_left\"\nWHERE EXISTS"),
            "{sql}"
        );
        assert!(!sql.contains("JOIN"), "semi join must not join: {sql}");
        assert!(
            sql.contains("\"__libdplyr_left\".\"id\" = \"__libdplyr_right\".\"id\""),
            "{sql}"
        );
    }

    #[test]
    fn semi_join_predicate_is_anded_with_existing_filter() {
        let mut q = query(
            SqlSource::Join {
                left: Box::new(branch("orders", &["id"])),
                right: Box::new(branch("users", &["id"])),
                join_type: JoinType::Semi,
                keys: vec![("id".to_string(), "id".to_string())],
            },
            vec![SelectItem {
                alias: "id".to_string(),
                expression: SelectExpression::Qualified {
                    relation: LEFT_ALIAS.to_string(),
                    column: "id".to_string(),
                },
            }],
        );
        q.filter = Some(Expr::Binary {
            left: Box::new(Expr::Identifier("id".to_string())),
            operator: BinaryOp::GreaterThan,
            right: Box::new(Expr::Literal(LiteralValue::Number(1.0))),
        });

        let sql = q.render(&generator()).unwrap();
        assert_eq!(sql.matches("\nWHERE ").count(), 1, "{sql}");
        assert!(sql.contains(" AND "), "{sql}");
    }

    #[test]
    fn rejects_full_join_on_mysql() {
        let q = query(
            SqlSource::Join {
                left: Box::new(branch("a", &["id"])),
                right: Box::new(branch("b", &["id"])),
                join_type: JoinType::Full,
                keys: vec![("id".to_string(), "id".to_string())],
            },
            vec![SelectItem {
                alias: "id".to_string(),
                expression: SelectExpression::Qualified {
                    relation: LEFT_ALIAS.to_string(),
                    column: "id".to_string(),
                },
            }],
        );
        let mysql = SqlGenerator::new(Box::new(crate::sql_generator::MySqlDialect::new()));

        assert!(matches!(
            q.render(&mysql),
            Err(GenerationError::UnsupportedOperation { operation, dialect })
                if operation == "full join" && dialect == "mysql"
        ));
    }

    #[test]
    fn renders_set_operation_between_aliased_subqueries() {
        let q = query(
            SqlSource::Set {
                left: Box::new(branch("a", &["id"])),
                right: Box::new(branch("b", &["id"])),
                operation: SetOperation::Union,
            },
            vec![SelectItem {
                alias: "id".to_string(),
                expression: SelectExpression::Qualified {
                    relation: LEFT_ALIAS.to_string(),
                    column: "id".to_string(),
                },
            }],
        );

        assert_eq!(
            q.render(&generator()).unwrap(),
            concat!(
                "SELECT \"__libdplyr_left\".\"id\" AS \"id\"\n",
                "FROM (\n",
                "SELECT \"id\" AS \"id\"\n",
                "FROM \"a\"\n",
                "UNION\n",
                "SELECT \"id\" AS \"id\"\n",
                "FROM \"b\"\n",
                ") AS \"__libdplyr_left\""
            )
        );
    }

    #[test]
    fn renders_coalesced_join_key() {
        let q = query(
            SqlSource::Table("joined".to_string()),
            vec![SelectItem {
                alias: "id".to_string(),
                expression: SelectExpression::Coalesce {
                    left: (LEFT_ALIAS.to_string(), "id".to_string()),
                    right: (RIGHT_ALIAS.to_string(), "id".to_string()),
                },
            }],
        );

        assert_eq!(
            q.render(&generator()).unwrap(),
            "SELECT COALESCE(\"__libdplyr_left\".\"id\", \"__libdplyr_right\".\"id\") AS \"id\"\nFROM \"joined\""
        );
    }

    #[test]
    fn join_without_keys_is_rejected() {
        let q = query(
            SqlSource::Join {
                left: Box::new(branch("a", &["id"])),
                right: Box::new(branch("b", &["id"])),
                join_type: JoinType::Inner,
                keys: Vec::new(),
            },
            vec![SelectItem {
                alias: "id".to_string(),
                expression: SelectExpression::Qualified {
                    relation: LEFT_ALIAS.to_string(),
                    column: "id".to_string(),
                },
            }],
        );

        assert!(q.render(&generator()).is_err());
    }

    #[test]
    fn aggregate_expression_renders_math_without_over() {
        let generator = generator();
        // mean(x) / n() must not emit OVER in a grouped aggregate.
        let expr = Expr::Binary {
            left: Box::new(Expr::Function {
                name: "mean".to_string(),
                args: vec![Expr::Identifier("x".to_string())],
            }),
            operator: BinaryOp::Divide,
            right: Box::new(Expr::Function {
                name: "n".to_string(),
                args: Vec::new(),
            }),
        };

        assert_eq!(
            generator.render_aggregate_expression(&expr).unwrap(),
            "((AVG(\"x\") * 1.0) / COUNT(*))"
        );
    }

    #[test]
    fn aggregate_division_uses_floating_point_numerator() {
        let generator = generator();
        // sum(x) / n() must not inherit integer division on SQLite/Postgres.
        let expr = Expr::Binary {
            left: Box::new(Expr::Function {
                name: "sum".to_string(),
                args: vec![Expr::Identifier("x".to_string())],
            }),
            operator: BinaryOp::Divide,
            right: Box::new(Expr::Function {
                name: "n".to_string(),
                args: Vec::new(),
            }),
        };

        assert_eq!(
            generator.render_aggregate_expression(&expr).unwrap(),
            "((SUM(\"x\") * 1.0) / COUNT(*))"
        );
    }

    #[test]
    fn aggregate_rejects_window_only_function_inside_another_aggregate() {
        let generator = generator();
        // SUM(lag(x)) is not valid SQL; it must be rejected, not rendered.
        let expr = Expr::Function {
            name: "sum".to_string(),
            args: vec![Expr::Function {
                name: "lag".to_string(),
                args: vec![Expr::Identifier("x".to_string())],
            }],
        };

        assert!(matches!(
            generator.render_aggregate_expression(&expr),
            Err(GenerationError::InvalidAst { reason }) if reason.contains("window function")
        ));
    }

    #[test]
    fn aggregate_expression_preserves_n_distinct_null_semantics() {
        let generator = generator();
        let expr = Expr::Function {
            name: "n_distinct".to_string(),
            args: vec![Expr::Identifier("x".to_string())],
        };

        assert_eq!(
            generator.render_aggregate_expression(&expr).unwrap(),
            "(COUNT(DISTINCT \"x\") + CASE WHEN COUNT(*) > COUNT(\"x\") THEN 1 ELSE 0 END)"
        );
    }

    #[test]
    fn aggregate_expression_rejects_nested_aggregate() {
        let generator = generator();
        let expr = Expr::Function {
            name: "sum".to_string(),
            args: vec![Expr::Function {
                name: "sum".to_string(),
                args: vec![Expr::Identifier("x".to_string())],
            }],
        };

        assert!(matches!(
            generator.render_aggregate_expression(&expr),
            Err(GenerationError::InvalidAst { reason }) if reason.contains("cannot be nested")
        ));
    }

    #[test]
    fn aggregate_expression_rejects_window_function() {
        let generator = generator();
        let expr = Expr::Function {
            name: "row_number".to_string(),
            args: Vec::new(),
        };

        assert!(matches!(
            generator.render_aggregate_expression(&expr),
            Err(GenerationError::InvalidAst { reason }) if reason.contains("window function")
        ));
    }

    #[test]
    fn aggregate_expression_enforces_arity() {
        let generator = generator();
        // n() takes no argument.
        let expr = Expr::Function {
            name: "n".to_string(),
            args: vec![Expr::Identifier("x".to_string())],
        };

        assert!(generator.render_aggregate_expression(&expr).is_err());
    }
}
