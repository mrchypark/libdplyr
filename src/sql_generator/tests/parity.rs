use super::*;

#[path = "parity_sqlite.rs"]
mod sqlite;
use serde_json::json;

fn call(name: &str, args: Vec<Expr>) -> Expr {
    Expr::Function {
        name: name.into(),
        args,
    }
}

fn column(name: &str) -> Expr {
    Expr::Identifier(name.into())
}

fn number(value: f64) -> Expr {
    Expr::Literal(LiteralValue::Number(value))
}

fn values(
    generator: &SqlGenerator,
    expr: &Expr,
    groups: &[String],
    order: &[OrderExpr],
    frame: Option<(i64, i64)>,
) -> serde_json::Value {
    let sql = generator
        .render_expression_with_window(expr, groups, order, frame)
        .expect("render expression");
    sqlite::execute(&format!("SELECT id, {sql} FROM data ORDER BY id"), false)
}

fn asc(column: &str) -> OrderExpr {
    OrderExpr {
        column: column.into(),
        direction: OrderDirection::Asc,
    }
}

#[test]
fn parity_null_ranking_values_and_group_denominators() {
    let generator = SqlGenerator::new(Box::new(SqliteDialect::new()));
    let groups = ["g".into()];
    for name in ["rank", "min_rank"] {
        assert_eq!(
            values(
                &generator,
                &call(name, vec![column("x")]),
                &groups,
                &[],
                None
            ),
            json!([
                [1, 3],
                [2, 1],
                [3, 1],
                [4, null],
                [5, 4],
                [6, null],
                [7, 1],
                [8, 1]
            ])
        );
    }
    assert_eq!(
        values(
            &generator,
            &call("dense_rank", vec![column("x")]),
            &groups,
            &[],
            None
        ),
        json!([
            [1, 2],
            [2, 1],
            [3, 1],
            [4, null],
            [5, 3],
            [6, null],
            [7, 1],
            [8, 1]
        ])
    );
    let percentages = values(
        &generator,
        &call("percent_rank", vec![column("x")]),
        &groups,
        &[],
        None,
    );
    assert!((percentages[0][1].as_f64().expect("percentage") - 2.0 / 3.0).abs() < 1e-12);
    assert_eq!(percentages[3][1], json!(null));
    assert_eq!(percentages[6][1], json!(0.0));
    assert_eq!(
        values(
            &generator,
            &call("cume_dist", vec![column("x")]),
            &groups,
            &[],
            None
        ),
        json!([
            [1, 0.75],
            [2, 0.5],
            [3, 0.5],
            [4, null],
            [5, 1.0],
            [6, null],
            [7, 1.0],
            [8, 1.0]
        ])
    );
    assert_eq!(
        values(
            &generator,
            &call("ntile", vec![column("x"), number(3.0)]),
            &groups,
            &[],
            None
        ),
        json!([
            [1, 2],
            [2, 1],
            [3, 1],
            [4, null],
            [5, 3],
            [6, null],
            [7, 1],
            [8, 2]
        ])
    );
}

#[test]
fn parity_descending_ranks_and_all_null_input() {
    let generator = SqlGenerator::new(Box::new(SqliteDialect::new()));
    let descending = call("min_rank", vec![call("desc", vec![column("x")])]);
    assert_eq!(
        values(&generator, &descending, &["g".into()], &[], None),
        json!([
            [1, 2],
            [2, 3],
            [3, 3],
            [4, null],
            [5, 1],
            [6, null],
            [7, 1],
            [8, 1]
        ])
    );
    for name in [
        "rank",
        "min_rank",
        "dense_rank",
        "percent_rank",
        "cume_dist",
    ] {
        let expr = call(name, vec![Expr::Literal(LiteralValue::Null)]);
        assert_eq!(
            values(&generator, &expr, &[], &[], None),
            json!([
                [1, null],
                [2, null],
                [3, null],
                [4, null],
                [5, null],
                [6, null],
                [7, null],
                [8, null]
            ])
        );
    }
}

#[test]
fn parity_order_frames_and_cumulative_values() {
    let generator = SqlGenerator::new(Box::new(SqliteDialect::new()));
    let groups = ["g".into()];
    let order = [asc("id")];
    assert_eq!(
        values(
            &generator,
            &call("sum", vec![column("x")]),
            &groups,
            &order,
            None
        ),
        json!([
            [1, 70],
            [2, 70],
            [3, 70],
            [4, 70],
            [5, 70],
            [6, 10],
            [7, 10],
            [8, 10]
        ])
    );
    assert_eq!(
        values(
            &generator,
            &call("sum", vec![column("x")]),
            &groups,
            &order,
            Some((-1, 0))
        ),
        json!([
            [1, 20],
            [2, 30],
            [3, 20],
            [4, 10],
            [5, 30],
            [6, null],
            [7, 5],
            [8, 10]
        ])
    );
    assert_eq!(
        values(
            &generator,
            &call("cumsum", vec![column("x")]),
            &groups,
            &order,
            Some((-1, 0))
        ),
        json!([
            [1, 20],
            [2, 30],
            [3, 40],
            [4, 40],
            [5, 70],
            [6, null],
            [7, 5],
            [8, 10]
        ])
    );
    assert_eq!(
        values(
            &generator,
            &call("cummin", vec![column("x")]),
            &groups,
            &order,
            None
        ),
        json!([
            [1, 20],
            [2, 10],
            [3, 10],
            [4, 10],
            [5, 10],
            [6, null],
            [7, 5],
            [8, 5]
        ])
    );
    assert_eq!(
        values(
            &generator,
            &call("cummax", vec![column("x")]),
            &groups,
            &order,
            None
        ),
        json!([
            [1, 20],
            [2, 20],
            [3, 20],
            [4, 20],
            [5, 30],
            [6, null],
            [7, 5],
            [8, 5]
        ])
    );
    let means = values(
        &generator,
        &call("cummean", vec![column("x")]),
        &groups,
        &order,
        None,
    );
    assert_eq!(means[1][1], json!(15.0));
    assert!((means[2][1].as_f64().expect("mean") - 40.0 / 3.0).abs() < 1e-12);
    assert_eq!(means[4][1], json!(17.5));
}

#[test]
fn parity_nested_window_order_and_offsets() {
    let generator = SqlGenerator::new(Box::new(SqliteDialect::new()));
    let groups = ["g".into()];
    let order = [asc("id")];
    assert_eq!(
        values(
            &generator,
            &call(
                "coalesce",
                vec![call("lag", vec![column("x")]), number(-1.0)]
            ),
            &groups,
            &order,
            None
        ),
        json!([
            [1, -1],
            [2, 20],
            [3, 10],
            [4, 10],
            [5, -1],
            [6, -1],
            [7, -1],
            [8, 5]
        ])
    );
    let override_order = call(
        "lag",
        vec![
            column("x"),
            number(1.0),
            number(-1.0),
            call("desc", vec![column("id")]),
        ],
    );
    assert_eq!(
        values(&generator, &override_order, &groups, &order, None),
        json!([
            [1, 10],
            [2, 10],
            [3, null],
            [4, 30],
            [5, -1],
            [6, 5],
            [7, 5],
            [8, -1]
        ])
    );
    assert_eq!(
        values(
            &generator,
            &call("lead", vec![column("x")]),
            &groups,
            &order,
            None
        ),
        json!([
            [1, 10],
            [2, 10],
            [3, null],
            [4, 30],
            [5, null],
            [6, 5],
            [7, 5],
            [8, null]
        ])
    );
}

#[test]
fn parity_conditionals_nulls_and_nzchar_values() {
    let generator = SqlGenerator::new(Box::new(SqliteDialect::new()));
    for name in ["ifelse", "if_else"] {
        assert_eq!(
            values(
                &generator,
                &call(name, vec![column("flag"), number(1.0), number(0.0)]),
                &[],
                &[],
                None
            ),
            json!([
                [1, 1],
                [2, 0],
                [3, null],
                [4, null],
                [5, 1],
                [6, null],
                [7, 1],
                [8, 0]
            ])
        );
    }
    assert_eq!(
        values(
            &generator,
            &call(
                "if_else",
                vec![column("flag"), number(1.0), number(0.0), number(-1.0)]
            ),
            &[],
            &[],
            None
        ),
        json!([
            [1, 1],
            [2, 0],
            [3, -1],
            [4, -1],
            [5, 1],
            [6, -1],
            [7, 1],
            [8, 0]
        ])
    );
    assert_eq!(
        values(
            &generator,
            &call("nzchar", vec![column("label")]),
            &[],
            &[],
            None
        ),
        json!([
            [1, 0],
            [2, 1],
            [3, 1],
            [4, 1],
            [5, 1],
            [6, 1],
            [7, 1],
            [8, 0]
        ])
    );
    assert_eq!(
        values(
            &generator,
            &call(
                "nzchar",
                vec![
                    column("label"),
                    Expr::NamedArg {
                        name: "keepNA".into(),
                        value: Box::new(Expr::Literal(LiteralValue::Boolean(true)))
                    }
                ]
            ),
            &[],
            &[],
            None
        ),
        json!([
            [1, 0],
            [2, 1],
            [3, null],
            [4, 1],
            [5, null],
            [6, null],
            [7, 1],
            [8, 0]
        ])
    );
}

#[test]
fn parity_null_helpers_dates_and_native_calls() {
    let generator = SqlGenerator::new(Box::new(SqliteDialect::new()));
    assert_eq!(
        values(
            &generator,
            &call("na_if", vec![column("x"), column("y")]),
            &[],
            &[],
            None
        ),
        json!([
            [1, null],
            [2, 10],
            [3, null],
            [4, null],
            [5, null],
            [6, null],
            [7, null],
            [8, null]
        ])
    );
    assert_eq!(
        values(
            &generator,
            &call("is.null", vec![column("x")]),
            &[],
            &[],
            None
        ),
        json!([
            [1, 0],
            [2, 0],
            [3, 0],
            [4, 1],
            [5, 0],
            [6, 1],
            [7, 0],
            [8, 0]
        ])
    );
    assert_eq!(
        values(
            &generator,
            &call("as.Date", vec![column("date_value")]),
            &[],
            &[],
            None
        ),
        json!([
            [1, "2026-01-02"],
            [2, "2026-01-02"],
            [3, null],
            [4, null],
            [5, "2026-02-03"],
            [6, null],
            [7, "2026-01-04"],
            [8, "2026-01-04"]
        ])
    );
    assert_eq!(
        values(
            &generator,
            &call("length", vec![column("label")]),
            &[],
            &[],
            None
        ),
        json!([
            [1, 0],
            [2, 2],
            [3, null],
            [4, 1],
            [5, null],
            [6, null],
            [7, 1],
            [8, 0]
        ])
    );
}

#[test]
fn parity_frame_and_window_errors() {
    let generator = SqlGenerator::new(Box::new(SqliteDialect::new()));
    for frame in [(1, -1), (i64::MAX, i64::MAX), (i64::MIN, i64::MIN)] {
        assert!(generator
            .render_expression_with_window(&column("x"), &[], &[asc("id")], Some(frame))
            .is_err());
    }
    for name in ["cumsum", "cummean", "cummin", "cummax"] {
        assert!(matches!(
            generator.render_expression(&call(name, vec![column("x")]), &[]),
            Err(GenerationError::InvalidAst { .. })
        ));
        assert!(generator
            .render_aggregate_expression(&call(name, vec![column("x")]))
            .is_err());
        assert!(generator
            .render_expression_with_window(
                &call(name, vec![call("sum", vec![column("x")])]),
                &[],
                &[asc("id")],
                None
            )
            .is_err());
    }
    assert!(generator
        .render_expression(&call("n_distinct", vec![column("x")]), &[])
        .expect_err("unsupported exact distinct window")
        .to_string()
        .contains("query stage"));
}

#[test]
fn parity_statistics_and_date_backend_support() {
    for dialect in [
        Box::new(PostgreSqlDialect::new()) as Box<dyn SqlDialect>,
        Box::new(MySqlDialect::new()),
        Box::new(DuckDbDialect::new()),
    ] {
        let generator = SqlGenerator::new(dialect);
        for (name, sql_name) in [
            ("sd", "STDDEV_SAMP"),
            ("var", "VAR_SAMP"),
            ("stddev_pop", "STDDEV_POP"),
            ("var_pop", "VAR_POP"),
        ] {
            let expr = call(
                name,
                vec![
                    column("x"),
                    Expr::NamedArg {
                        name: "na.rm".into(),
                        value: Box::new(Expr::Literal(LiteralValue::Boolean(true))),
                    },
                ],
            );
            assert!(generator
                .render_aggregate_expression(&expr)
                .expect("supported sample statistic")
                .starts_with(sql_name));
            assert!(generator
                .render_expression_with_window(&expr, &["g".into()], &[asc("id")], None)
                .expect("statistic window")
                .contains("UNBOUNDED FOLLOWING"));
        }
        assert!(generator
            .render_expression(&call("as.Date", vec![column("date_value")]), &[])
            .expect("date cast")
            .contains(" AS DATE)"));
    }
    let sqlite = SqlGenerator::new(Box::new(SqliteDialect::new()));
    for name in ["sd", "var", "stddev_pop", "var_pop"] {
        assert!(sqlite
            .render_expression(&call(name, vec![column("x")]), &[])
            .is_err());
        assert!(sqlite
            .render_aggregate_expression(&call(name, vec![column("x")]))
            .is_err());
    }
}

#[test]
fn parity_native_function_validation_and_common_gate() {
    let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));
    for name in ["my_extension", "schema.my_extension"] {
        assert!(generator
            .render_expression(&call(name, vec![column("x")]), &[])
            .expect("native fallback")
            .starts_with(name));
        assert!(!dialect::is_supported_common_function(name));
    }
    for name in [
        "",
        "x; DROP TABLE data",
        "f--comment",
        "f()",
        "schema..f",
        ".f",
        "f/*x*/",
        "x y",
        "9func",
    ] {
        assert!(
            generator
                .render_expression(&call(name, vec![]), &[])
                .is_err(),
            "{name}"
        );
        assert!(
            generator
                .dialect()
                .translate_unknown_function(name, &[])
                .is_none(),
            "{name}"
        );
    }
    for name in [
        "min_rank",
        "percent_rank",
        "cume_dist",
        "cumsum",
        "na_if",
        "is.null",
        "as.Date",
        "sd",
        "var",
    ] {
        assert!(dialect::is_supported_common_function(name), "{name}");
    }
    assert!(generator
        .render_expression(&call("if_else", vec![column("flag")]), &[])
        .is_err());
    assert!(generator
        .render_expression(&call("tolower", vec![]), &[])
        .is_err());
}

#[derive(Clone)]
struct NativeFunctionDialect;

impl SqlDialect for NativeFunctionDialect {
    fn quote_identifier(&self, name: &str) -> String {
        PostgreSqlDialect::new().quote_identifier(name)
    }
    fn quote_string(&self, value: &str) -> String {
        PostgreSqlDialect::new().quote_string(value)
    }
    fn limit_clause(&self, limit: usize) -> String {
        PostgreSqlDialect::new().limit_clause(limit)
    }
    fn string_concat(&self, left: &str, right: &str) -> String {
        PostgreSqlDialect::new().string_concat(left, right)
    }
    fn aggregate_function(&self, function: &str) -> String {
        PostgreSqlDialect::new().aggregate_function(function)
    }
    fn is_case_sensitive(&self) -> bool {
        false
    }
    fn clone_box(&self) -> Box<dyn SqlDialect> {
        Box::new(self.clone())
    }
    fn is_supported_function(&self, function: &str) -> bool {
        dialect::is_supported_common_function(function) || function == "installed_function"
    }
    fn translate_unknown_function(&self, function: &str, args: &[String]) -> Option<String> {
        Some(format!("resolved_{function}({})", args.join(", ")))
    }
}

#[test]
fn parity_native_hook_respects_supported_function_gate() {
    let generator = SqlGenerator::new(Box::new(NativeFunctionDialect));
    assert_eq!(
        generator
            .render_expression(&call("installed_function", vec![column("x")]), &[])
            .expect("installed hook"),
        "resolved_installed_function(\"x\")"
    );
    assert!(generator
        .render_expression(&call("uninstalled_function", vec![column("x")]), &[])
        .is_err());
    // A custom hook cannot hide an invalid call to an explicitly mapped helper.
    assert!(generator
        .render_expression(&call("ifelse", vec![column("flag")]), &[])
        .is_err());
    assert!(generator
        .render_expression(&call("installed_function; SELECT 1", vec![]), &[])
        .is_err());
}

#[test]
fn parity_preserves_distinct_division_and_global_cardinality() {
    let generator = SqlGenerator::new(Box::new(SqliteDialect::new()));
    let distinct = generator
        .render_aggregate_expression(&call("n_distinct", vec![column("x")]))
        .expect("distinct summary");
    assert_eq!(
        sqlite::execute(&format!("SELECT {distinct} FROM data"), false),
        json!([[5]])
    );
    assert_eq!(
        sqlite::execute(&format!("SELECT {distinct} FROM data"), true),
        json!([[0]])
    );
    let divided = Expr::Binary {
        left: Box::new(call("sum", vec![column("x")])),
        operator: BinaryOp::Divide,
        right: Box::new(call("n", vec![])),
    };
    let expression = generator
        .render_aggregate_expression(&divided)
        .expect("floating summary");
    assert_eq!(
        sqlite::execute(&format!("SELECT {expression} FROM data"), false),
        json!([[10.0]])
    );
    let fractional = sqlite::execute(
        &format!("SELECT {expression} FROM data WHERE id <= 3"),
        false,
    );
    assert!((fractional[0][0].as_f64().expect("floating division") - 40.0 / 3.0).abs() < 1e-12);
    let membership = Expr::In {
        expr: Box::new(call("sum", vec![column("x")])),
        values: vec![],
    };
    let expression = generator
        .render_aggregate_expression(&membership)
        .expect("constant summary with aggregate subject");
    assert_eq!(
        sqlite::execute(&format!("SELECT {expression} FROM data"), true),
        json!([[0]])
    );
}

#[test]
fn parity_legacy_extended_error_and_union_all() {
    let generator = SqlGenerator::new(Box::new(SqliteDialect::new()));
    let pipeline = |operation| DplyrNode::Pipeline {
        source: Some("data".into()),
        target: None,
        operations: vec![operation],
        location: SourceLocation::unknown(),
    };
    let operation = DplyrOperation::Extended {
        name: "window_order".into(),
        args: vec![column("id")],
        location: SourceLocation::unknown(),
    };
    assert!(generator
        .generate(&pipeline(operation))
        .expect_err("schema required")
        .to_string()
        .contains("use a schema source"));
    let operation = DplyrOperation::SetOp {
        operation: SetOperation::UnionAll,
        right_table: "other".into(),
        location: SourceLocation::unknown(),
    };
    assert!(generator
        .generate(&pipeline(operation))
        .expect("legacy union all")
        .contains("UNION ALL"));
}

#[test]
fn parity_ordered_window_api() {
    let generator = SqlGenerator::new(Box::new(SqliteDialect::new()));
    let order = [OrderExpr {
        column: "id".into(),
        direction: OrderDirection::Desc,
    }];
    let groups = ["grp".into()];
    let lag = generator
        .render_expression_with_window(&call("lag", vec![column("x")]), &groups, &order, None)
        .expect("ordered lag");
    assert!(lag.contains("PARTITION BY \"grp\""));
    assert!(lag.contains("\"id\" DESC"));
    let total = generator
        .render_expression_with_window(&call("sum", vec![column("x")]), &groups, &order, None)
        .expect("partition sum");
    assert!(total.contains("ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING"));
    let running = generator
        .render_expression_with_window(&call("cumsum", vec![column("x")]), &groups, &order, None)
        .expect("cumulative sum");
    assert!(running.contains("ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW"));
    assert!(generator
        .render_expression(&call("cumsum", vec![column("x")]), &groups)
        .is_err());
}

#[test]
fn parity_null_and_missing_arguments() {
    let generator = SqlGenerator::new(Box::new(SqliteDialect::new()));
    let conditional = call(
        "if_else",
        vec![
            column("x"),
            column("yes"),
            column("no"),
            Expr::NamedArg {
                name: "missing".into(),
                value: Box::new(Expr::Literal(LiteralValue::Number(7.0))),
            },
        ],
    );
    let sql = generator
        .render_expression(&conditional, &[])
        .expect("missing branch");
    assert!(sql.contains("IS NULL THEN 7"));
    assert!(generator
        .render_expression(&call("ifelse", vec![column("x")]), &[])
        .is_err());
    assert!(generator
        .render_expression(&call("nzchar", vec![column("x")]), &[])
        .expect("nzchar")
        .contains("COALESCE"));
}
