use super::*;
use crate::parser::{
    Aggregation, Assignment, ColumnExpr, DplyrNode, DplyrOperation, Expr, OrderDirection,
    OrderExpr, SourceLocation,
};

// Helper function to normalize SQL for comparison
fn normalize_sql(sql: &str) -> String {
    sql.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase()
}

// Helper function to create test AST nodes
fn create_test_select_operation(columns: Vec<&str>) -> DplyrOperation {
    DplyrOperation::Select {
        columns: columns
            .into_iter()
            .map(|col| ColumnExpr {
                expr: Expr::Identifier(col.to_string()),
                alias: None,
            })
            .collect(),
        location: SourceLocation::unknown(),
    }
}

fn create_test_filter_operation(column: &str, value: f64) -> DplyrOperation {
    DplyrOperation::Filter {
        condition: Expr::Binary {
            left: Box::new(Expr::Identifier(column.to_string())),
            operator: BinaryOp::GreaterThan,
            right: Box::new(Expr::Literal(LiteralValue::Number(value))),
        },
        location: SourceLocation::unknown(),
    }
}

// ===== SQL Dialect Tests =====

mod dialect_tests {
    use super::*;

    #[test]
    fn test_postgresql_dialect_identifier_quoting() {
        let dialect = PostgreSqlDialect::new();
        assert_eq!(dialect.quote_identifier("test"), "\"test\"");
        assert_eq!(dialect.quote_identifier("column_name"), "\"column_name\"");
        assert_eq!(dialect.quote_identifier("CamelCase"), "\"CamelCase\"");
    }

    #[test]
    fn test_postgresql_dialect_string_quoting() {
        let dialect = PostgreSqlDialect::new();
        assert_eq!(dialect.quote_string("hello"), "'hello'");
        assert_eq!(dialect.quote_string("it's"), "'it''s'");
        assert_eq!(dialect.quote_string(""), "''");
    }

    #[test]
    fn test_postgresql_dialect_aggregate_functions() {
        let dialect = PostgreSqlDialect::new();
        assert_eq!(dialect.aggregate_function("mean"), "AVG");
        assert_eq!(dialect.aggregate_function("avg"), "AVG");
        assert_eq!(dialect.aggregate_function("sum"), "SUM");
        assert_eq!(dialect.aggregate_function("count"), "COUNT");
        assert_eq!(dialect.aggregate_function("min"), "MIN");
        assert_eq!(dialect.aggregate_function("max"), "MAX");
        assert_eq!(dialect.aggregate_function("n"), "COUNT(*)");
        assert_eq!(dialect.aggregate_function("custom"), "CUSTOM");
    }

    #[test]
    fn test_postgresql_dialect_string_concat() {
        let dialect = PostgreSqlDialect::new();
        assert_eq!(dialect.string_concat("a", "b"), "a || b");
        assert_eq!(
            dialect.string_concat("'hello'", "'world'"),
            "'hello' || 'world'"
        );
    }

    #[test]
    fn test_mysql_dialect_identifier_quoting() {
        let dialect = MySqlDialect::new();
        assert_eq!(dialect.quote_identifier("test"), "`test`");
        assert_eq!(dialect.quote_identifier("column_name"), "`column_name`");
    }

    #[test]
    fn test_mysql_dialect_string_concat() {
        let dialect = MySqlDialect::new();
        assert_eq!(dialect.string_concat("a", "b"), "CONCAT(a, b)");
        assert_eq!(
            dialect.string_concat("'hello'", "'world'"),
            "CONCAT('hello', 'world')"
        );
    }

    #[test]
    fn test_sqlite_dialect() {
        let dialect = SqliteDialect::new();
        assert_eq!(dialect.quote_identifier("test"), "\"test\"");
        assert_eq!(dialect.string_concat("a", "b"), "a || b");
        assert_eq!(dialect.aggregate_function("mean"), "AVG");
    }

    #[test]
    fn test_duckdb_dialect_special_functions() {
        let dialect = DuckDbDialect::new();
        assert_eq!(dialect.aggregate_function("median"), "MEDIAN");
        assert_eq!(dialect.aggregate_function("mode"), "MODE");
        assert_eq!(dialect.aggregate_function("mean"), "AVG");
    }

    #[test]
    fn test_dialect_limit_clause() {
        let pg_dialect = PostgreSqlDialect::new();
        let mysql_dialect = MySqlDialect::new();
        let sqlite_dialect = SqliteDialect::new();

        assert_eq!(pg_dialect.limit_clause(10), "LIMIT 10");
        assert_eq!(mysql_dialect.limit_clause(5), "LIMIT 5");
        assert_eq!(sqlite_dialect.limit_clause(100), "LIMIT 100");
    }

    #[test]
    fn test_dialect_case_sensitivity() {
        let pg_dialect = PostgreSqlDialect::new();
        let mysql_dialect = MySqlDialect::new();
        let sqlite_dialect = SqliteDialect::new();
        let duckdb_dialect = DuckDbDialect::new();

        assert!(!pg_dialect.is_case_sensitive());
        assert!(!mysql_dialect.is_case_sensitive());
        assert!(!sqlite_dialect.is_case_sensitive());
        assert!(!duckdb_dialect.is_case_sensitive());
    }
}

// ===== SQL Clause Generation Tests =====

mod clause_generation_tests {
    use super::*;

    #[test]
    fn test_select_clause_generation() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

        let columns = vec![
            ColumnExpr {
                expr: Expr::Identifier("name".to_string()),
                alias: None,
            },
            ColumnExpr {
                expr: Expr::Identifier("age".to_string()),
                alias: Some("user_age".to_string()),
            },
        ];

        let parts = QueryParts::new();
        let result = generator
            .generate_select_columns_with_mutations(&columns, &parts)
            .unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "\"name\"");
        assert_eq!(result[1], "\"age\" AS \"user_age\"");
    }

    #[test]
    fn test_where_clause_generation() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

        let condition = Expr::Binary {
            left: Box::new(Expr::Identifier("age".to_string())),
            operator: BinaryOp::GreaterThanOrEqual,
            right: Box::new(Expr::Literal(LiteralValue::Number(18.0))),
        };

        let result = generator.generate_expression(&condition).unwrap();
        assert_eq!(result, "(\"age\" >= 18)");
    }

    #[test]
    fn test_order_by_clause_generation() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

        let columns = vec![
            OrderExpr {
                column: "name".to_string(),
                direction: OrderDirection::Asc,
            },
            OrderExpr {
                column: "age".to_string(),
                direction: OrderDirection::Desc,
            },
        ];

        let result = generator.generate_order_by(&columns).unwrap();
        assert_eq!(result, "\"name\" ASC, \"age\" DESC");
    }

    #[test]
    fn test_aggregation_generation() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

        let aggregations = vec![
            Aggregation {
                function: "mean".to_string(),
                column: "salary".to_string(),
                alias: Some("avg_salary".to_string()),
            },
            Aggregation {
                function: "n".to_string(),
                column: "".to_string(),
                alias: Some("count".to_string()),
            },
        ];

        let result = generator.generate_aggregations(&aggregations).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "AVG(\"salary\") AS \"avg_salary\"");
        assert_eq!(result[1], "COUNT(*) AS \"count\"");
    }

    #[test]
    fn test_complex_expression_generation() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

        // Test nested binary expressions: (age > 18) AND (status = 'active')
        let condition = Expr::Binary {
            left: Box::new(Expr::Binary {
                left: Box::new(Expr::Identifier("age".to_string())),
                operator: BinaryOp::GreaterThan,
                right: Box::new(Expr::Literal(LiteralValue::Number(18.0))),
            }),
            operator: BinaryOp::And,
            right: Box::new(Expr::Binary {
                left: Box::new(Expr::Identifier("status".to_string())),
                operator: BinaryOp::Equal,
                right: Box::new(Expr::Literal(LiteralValue::String("active".to_string()))),
            }),
        };

        let result = generator.generate_expression(&condition).unwrap();
        assert_eq!(result, "((\"age\" > 18) AND (\"status\" = 'active'))");
    }

    #[test]
    fn test_function_expression_generation() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

        let function_expr = Expr::Function {
            name: "upper".to_string(),
            args: vec![Expr::Identifier("name".to_string())],
        };

        let result = generator.generate_expression(&function_expr).unwrap();
        assert_eq!(result, "UPPER(\"name\")");
    }

    #[test]
    fn test_literal_generation() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

        assert_eq!(
            generator
                .generate_literal(&LiteralValue::String("test".to_string()))
                .unwrap(),
            "'test'"
        );
        assert_eq!(
            generator
                .generate_literal(&LiteralValue::Number(42.5))
                .unwrap(),
            "42.5"
        );
        assert_eq!(
            generator
                .generate_literal(&LiteralValue::Boolean(true))
                .unwrap(),
            "TRUE"
        );
        assert_eq!(
            generator
                .generate_literal(&LiteralValue::Boolean(false))
                .unwrap(),
            "FALSE"
        );
        assert_eq!(
            generator.generate_literal(&LiteralValue::Null).unwrap(),
            "NULL"
        );
    }
}

// ===== Dialect-Specific SQL Generation Tests =====

mod dialect_specific_tests {
    use super::*;

    #[test]
    fn test_postgresql_vs_mysql_identifier_quoting() {
        let pg_generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));
        let mysql_generator = SqlGenerator::new(Box::new(MySqlDialect::new()));

        let ast = DplyrNode::Pipeline {
            source: None,
            target: None,
            operations: vec![create_test_select_operation(vec!["name", "age"])],
            location: SourceLocation::unknown(),
        };

        let pg_sql = pg_generator.generate(&ast).unwrap();
        let mysql_sql = mysql_generator.generate(&ast).unwrap();

        assert!(pg_sql.contains("\"name\""));
        assert!(pg_sql.contains("\"age\""));
        assert!(mysql_sql.contains("`name`"));
        assert!(mysql_sql.contains("`age`"));
    }

    #[test]
    fn test_string_concatenation_differences() {
        let pg_generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));
        let mysql_generator = SqlGenerator::new(Box::new(MySqlDialect::new()));

        let concat_expr = Expr::Function {
            name: "concat".to_string(),
            args: vec![
                Expr::Identifier("first_name".to_string()),
                Expr::Literal(LiteralValue::String(" ".to_string())),
                Expr::Identifier("last_name".to_string()),
            ],
        };

        let pg_result = pg_generator.generate_expression(&concat_expr).unwrap();
        let mysql_result = mysql_generator.generate_expression(&concat_expr).unwrap();

        assert_eq!(pg_result, "CONCAT(\"first_name\", ' ', \"last_name\")");
        assert_eq!(mysql_result, "CONCAT(`first_name`, ' ', `last_name`)");
    }

    #[test]
    fn test_aggregate_function_mapping_consistency() {
        let dialects: Vec<Box<dyn SqlDialect>> = vec![
            Box::new(PostgreSqlDialect::new()),
            Box::new(MySqlDialect::new()),
            Box::new(SqliteDialect::new()),
            Box::new(DuckDbDialect::new()),
        ];

        let common_functions = vec!["mean", "sum", "count", "min", "max", "n"];

        for dialect in dialects {
            for func in &common_functions {
                let result = dialect.aggregate_function(func);
                assert!(
                    !result.is_empty(),
                    "Function {func} should map to something"
                );

                // Common mappings should be consistent
                match *func {
                    "mean" => assert_eq!(result, "AVG"),
                    "sum" => assert_eq!(result, "SUM"),
                    "count" => assert_eq!(result, "COUNT"),
                    "min" => assert_eq!(result, "MIN"),
                    "max" => assert_eq!(result, "MAX"),
                    "n" => assert_eq!(result, "COUNT(*)"),
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn test_duckdb_specific_functions() {
        let duckdb_generator = SqlGenerator::new(Box::new(DuckDbDialect::new()));

        let aggregations = vec![
            Aggregation {
                function: "median".to_string(),
                column: "salary".to_string(),
                alias: None,
            },
            Aggregation {
                function: "mode".to_string(),
                column: "category".to_string(),
                alias: None,
            },
        ];

        let result = duckdb_generator
            .generate_aggregations(&aggregations)
            .unwrap();
        assert_eq!(result[0], "MEDIAN(\"salary\")");
        assert_eq!(result[1], "MODE(\"category\")");
    }
}

// ===== Complex Query Generation Tests =====

mod complex_query_tests {
    use super::*;

    #[test]
    fn test_complete_pipeline_generation() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

        let ast = DplyrNode::Pipeline {
            source: None,
            target: None,
            operations: vec![
                create_test_select_operation(vec!["name", "age", "salary"]),
                create_test_filter_operation("age", 25.0),
                DplyrOperation::Arrange {
                    columns: vec![OrderExpr {
                        column: "salary".to_string(),
                        direction: OrderDirection::Desc,
                    }],
                    location: SourceLocation::unknown(),
                },
            ],
            location: SourceLocation::unknown(),
        };

        let sql = generator.generate(&ast).unwrap();
        let normalized = normalize_sql(&sql);

        assert!(normalized.contains("SELECT"));
        assert!(normalized.contains("\"NAME\""));
        assert!(normalized.contains("\"AGE\""));
        assert!(normalized.contains("\"SALARY\""));
        assert!(normalized.contains("WHERE"));
        assert!(normalized.contains("\"AGE\" > 25"));
        assert!(normalized.contains("ORDER BY"));
        assert!(normalized.contains("\"SALARY\" DESC"));
    }

    #[test]
    fn test_group_by_with_aggregation() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

        let ast = DplyrNode::Pipeline {
            source: None,
            target: None,
            operations: vec![
                DplyrOperation::GroupBy {
                    columns: vec!["department".to_string()],
                    location: SourceLocation::unknown(),
                },
                DplyrOperation::Summarise {
                    aggregations: vec![
                        Aggregation {
                            function: "mean".to_string(),
                            column: "salary".to_string(),
                            alias: Some("avg_salary".to_string()),
                        },
                        Aggregation {
                            function: "n".to_string(),
                            column: "".to_string(),
                            alias: Some("count".to_string()),
                        },
                    ],
                    location: SourceLocation::unknown(),
                },
            ],
            location: SourceLocation::unknown(),
        };

        let sql = generator.generate(&ast).unwrap();
        let normalized = normalize_sql(&sql);

        assert!(normalized.contains("SELECT"));
        assert!(normalized.contains("AVG(\"SALARY\") AS \"AVG_SALARY\""));
        assert!(normalized.contains("COUNT(*) AS \"COUNT\""));
        assert!(normalized.contains("GROUP BY"));
        assert!(normalized.contains("\"DEPARTMENT\""));
    }

    #[test]
    fn test_multiple_filter_conditions() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

        let ast = DplyrNode::Pipeline {
            source: None,
            target: None,
            operations: vec![
                create_test_select_operation(vec!["name"]),
                DplyrOperation::Filter {
                    condition: Expr::Binary {
                        left: Box::new(Expr::Identifier("age".to_string())),
                        operator: BinaryOp::GreaterThan,
                        right: Box::new(Expr::Literal(LiteralValue::Number(18.0))),
                    },
                    location: SourceLocation::unknown(),
                },
                DplyrOperation::Filter {
                    condition: Expr::Binary {
                        left: Box::new(Expr::Identifier("status".to_string())),
                        operator: BinaryOp::Equal,
                        right: Box::new(Expr::Literal(LiteralValue::String("active".to_string()))),
                    },
                    location: SourceLocation::unknown(),
                },
            ],
            location: SourceLocation::unknown(),
        };

        let sql = generator.generate(&ast).unwrap();
        let normalized = normalize_sql(&sql);

        assert!(normalized.contains("WHERE"));
        assert!(normalized.contains("\"AGE\" > 18"));
        assert!(normalized.contains("AND"));
        assert!(normalized.contains("\"STATUS\" = 'ACTIVE'"));
    }

    #[test]
    fn test_mutate_operation_integration() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

        let ast = DplyrNode::Pipeline {
            source: None,
            target: None,
            operations: vec![DplyrOperation::Mutate {
                assignments: vec![
                    Assignment {
                        column: "adult".to_string(),
                        expr: Expr::Binary {
                            left: Box::new(Expr::Identifier("age".to_string())),
                            operator: BinaryOp::GreaterThanOrEqual,
                            right: Box::new(Expr::Literal(LiteralValue::Number(18.0))),
                        },
                    },
                    Assignment {
                        column: "salary_bonus".to_string(),
                        expr: Expr::Binary {
                            left: Box::new(Expr::Identifier("salary".to_string())),
                            operator: BinaryOp::Multiply,
                            right: Box::new(Expr::Literal(LiteralValue::Number(1.1))),
                        },
                    },
                ],
                location: SourceLocation::unknown(),
            }],
            location: SourceLocation::unknown(),
        };

        let sql = generator.generate(&ast).unwrap();
        let normalized = normalize_sql(&sql);

        assert!(normalized.contains("SELECT"));
        assert!(normalized.contains("\"AGE\" >= 18"));
        assert!(normalized.contains("AS \"ADULT\""));
        assert!(normalized.contains("\"SALARY\" * 1.1"));
        assert!(normalized.contains("AS \"SALARY_BONUS\""));
    }
}

// ===== Error Case Tests =====

mod error_case_tests {
    use super::*;

    #[test]
    fn test_empty_pipeline_error() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

        let ast = DplyrNode::Pipeline {
            source: None,
            target: None,
            operations: vec![],
            location: SourceLocation::unknown(),
        };

        let result = generator.generate(&ast);
        assert!(result.is_err());

        match result.unwrap_err() {
            GenerationError::InvalidAst { reason } => {
                assert!(reason.contains("Empty pipeline"));
            }
            _ => panic!("Expected InvalidAst error"),
        }
    }

    #[test]
    fn test_invalid_expression_handling() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

        // Test with deeply nested expressions that might cause issues
        let mut nested_expr = Expr::Identifier("base".to_string());
        for i in 0..100 {
            nested_expr = Expr::Binary {
                left: Box::new(nested_expr),
                operator: BinaryOp::Plus,
                right: Box::new(Expr::Literal(LiteralValue::Number(i as f64))),
            };
        }

        // This should not panic or cause stack overflow
        let result = generator.generate_expression(&nested_expr);
        assert!(result.is_ok(), "Should handle deeply nested expressions");
    }

    #[test]
    fn test_data_source_generation() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

        let ast = DplyrNode::DataSource {
            name: "users".to_string(),
            location: SourceLocation::unknown(),
        };

        let sql = generator.generate(&ast).unwrap();
        assert_eq!(normalize_sql(&sql), "SELECT * FROM \"USERS\"");
    }

    #[test]
    fn test_binary_operator_coverage() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

        let operators = vec![
            (BinaryOp::Equal, "="),
            (BinaryOp::NotEqual, "!="),
            (BinaryOp::LessThan, "<"),
            (BinaryOp::LessThanOrEqual, "<="),
            (BinaryOp::GreaterThan, ">"),
            (BinaryOp::GreaterThanOrEqual, ">="),
            (BinaryOp::And, "AND"),
            (BinaryOp::Or, "OR"),
            (BinaryOp::Plus, "+"),
            (BinaryOp::Minus, "-"),
            (BinaryOp::Multiply, "*"),
            (BinaryOp::Divide, "/"),
        ];

        for (op, expected) in operators {
            let result = generator.generate_binary_operator(&op);
            assert_eq!(result, expected, "Operator {op:?} should map to {expected}");
        }
    }

    #[test]
    fn test_special_characters_in_strings() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

        let test_strings = vec![
            ("simple", "'simple'"),
            ("it's", "'it''s'"),
            ("", "''"),
            ("line\nbreak", "'line\nbreak'"),
            ("tab\there", "'tab\there'"),
        ];

        for (input, expected) in test_strings {
            let literal = LiteralValue::String(input.to_string());
            let result = generator.generate_literal(&literal).unwrap();
            assert_eq!(
                result, expected,
                "String '{input}' should be quoted as {expected}"
            );
        }
    }

    #[test]
    fn test_edge_case_numbers() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

        let test_numbers = vec![
            (0.0, "0"),
            (-1.0, "-1"),
            (std::f64::consts::PI, "3.141592653589793"),
            (1e6, "1000000"),
            (1e-6, "0.000001"),
        ];

        for (input, expected) in test_numbers {
            let literal = LiteralValue::Number(input);
            let result = generator.generate_literal(&literal).unwrap();
            assert_eq!(
                result, expected,
                "Number {input} should be formatted as {expected}"
            );
        }
    }
}

// ===== Mutate Operation Advanced Tests =====

mod mutate_advanced_tests {
    use super::*;

    #[test]
    fn test_mutate_column_dependency_detection() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));
        let assignments = vec![
            Assignment {
                column: "doubled".to_string(),
                expr: Expr::Binary {
                    left: Box::new(Expr::Identifier("value".to_string())),
                    operator: BinaryOp::Multiply,
                    right: Box::new(Expr::Literal(LiteralValue::Number(2.0))),
                },
            },
            Assignment {
                column: "quadrupled".to_string(),
                expr: Expr::Binary {
                    left: Box::new(Expr::Identifier("doubled".to_string())),
                    operator: BinaryOp::Multiply,
                    right: Box::new(Expr::Literal(LiteralValue::Number(2.0))),
                },
            },
        ];

        let query_parts = QueryParts::new();
        let needs_subquery = generator.mutate_needs_subquery(&assignments, &query_parts);
        assert!(needs_subquery, "Should detect column dependencies");
    }

    #[test]
    fn test_mutate_with_window_functions() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));
        let assignments = vec![Assignment {
            column: "row_num".to_string(),
            expr: Expr::Function {
                name: "row_number".to_string(),
                args: vec![],
            },
        }];

        let query_parts = QueryParts::new();
        let is_complex = generator.expression_is_complex(&assignments[0].expr);
        assert!(is_complex, "Should detect window function as complex");

        let needs_subquery = generator.mutate_needs_subquery(&assignments, &query_parts);
        assert!(needs_subquery, "Should need subquery for window functions");
    }

    #[test]
    fn test_mutate_subquery_generation() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));
        let base_query = "SELECT * FROM employees";
        let assignments = vec![Assignment {
            column: "bonus".to_string(),
            expr: Expr::Binary {
                left: Box::new(Expr::Identifier("salary".to_string())),
                operator: BinaryOp::Multiply,
                right: Box::new(Expr::Literal(LiteralValue::Number(0.1))),
            },
        }];

        let result = generator.generate_mutate_subquery(base_query, &assignments);
        assert!(
            result.is_ok(),
            "Subquery generation should succeed: {result:?}"
        );

        let sql = result.unwrap();
        assert!(sql.contains("SELECT *, (\"salary\" * 0.1) AS \"bonus\""));
        assert!(sql.contains("FROM ("));
        assert!(sql.contains("SELECT * FROM employees"));
        assert!(sql.contains(") AS subquery"));
    }

    #[test]
    fn test_nested_pipeline_processing() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));
        let operations = vec![
            DplyrOperation::Filter {
                condition: Expr::Binary {
                    left: Box::new(Expr::Identifier("active".to_string())),
                    operator: BinaryOp::Equal,
                    right: Box::new(Expr::Literal(LiteralValue::Boolean(true))),
                },
                location: SourceLocation::unknown(),
            },
            DplyrOperation::Mutate {
                assignments: vec![Assignment {
                    column: "category".to_string(),
                    expr: Expr::Function {
                        name: "case".to_string(),
                        args: vec![
                            Expr::Identifier("score".to_string()),
                            Expr::Literal(LiteralValue::String("high".to_string())),
                        ],
                    },
                }],
                location: SourceLocation::unknown(),
            },
        ];

        let result = generator.generate_nested_pipeline(&operations);
        assert!(result.is_ok(), "Nested pipeline should succeed: {result:?}");

        let sql = result.unwrap();
        assert!(sql.contains("WHERE"));
        assert!(sql.contains("\"active\" = TRUE"));
        assert!(sql.contains("CASE"));
        assert!(sql.contains("AS \"category\""));
    }

    #[test]
    fn test_expression_reference_detection() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));
        let mut columns = std::collections::HashSet::new();
        columns.insert("existing_col".to_string());

        // Expression that references existing column
        let expr1 = Expr::Identifier("existing_col".to_string());
        assert!(generator.expression_references_columns(&expr1, &columns));

        // Expression that doesn't reference existing column
        let expr2 = Expr::Identifier("other_col".to_string());
        assert!(!generator.expression_references_columns(&expr2, &columns));

        // Binary expression with reference
        let expr3 = Expr::Binary {
            left: Box::new(Expr::Identifier("existing_col".to_string())),
            operator: BinaryOp::Plus,
            right: Box::new(Expr::Literal(LiteralValue::Number(1.0))),
        };
        assert!(generator.expression_references_columns(&expr3, &columns));
    }

    #[test]
    fn test_complex_expression_detection() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

        // Window functions should be detected as complex
        let window_functions = vec![
            "row_number",
            "rank",
            "dense_rank",
            "lag",
            "lead",
            "first_value",
            "last_value",
            "nth_value",
        ];
        for func_name in window_functions {
            let expr = Expr::Function {
                name: func_name.to_string(),
                args: vec![],
            };
            assert!(
                generator.expression_is_complex(&expr),
                "Function {} should be detected as complex",
                func_name
            );
        }

        // Regular functions should not be complex
        let regular_expr = Expr::Function {
            name: "upper".to_string(),
            args: vec![Expr::Identifier("name".to_string())],
        };
        assert!(!generator.expression_is_complex(&regular_expr));

        // Literals should not be complex
        let literal_expr = Expr::Literal(LiteralValue::Number(42.0));
        assert!(!generator.expression_is_complex(&literal_expr));
    }
}
