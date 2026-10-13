//! Parser-level regression tests for tidy-select, across(), slicing, and join
//! options.
//!
//! These assert the AST shape the rest of the pipeline consumes, so a change in
//! the shared contract shows up here rather than in generated SQL.

use libdplyr::error::ParseError;
use libdplyr::lexer::Lexer;
use libdplyr::parser::{
    BinaryOp, ColumnExpr, DplyrNode, DplyrOperation, Expr, JoinKey, JoinOptions, JoinSpec,
    LiteralValue, Parser, SliceKind, UnaryOp,
};

fn parse(code: &str) -> Result<DplyrNode, ParseError> {
    Parser::new(Lexer::new(code.to_string()))?.parse()
}

fn operations(code: &str) -> Vec<DplyrOperation> {
    let ast = parse(code).unwrap_or_else(|error| panic!("{code} must parse: {error}"));
    let DplyrNode::Pipeline { operations, .. } = ast else {
        panic!("{code} must be a pipeline");
    };
    operations
}

fn selectors(code: &str) -> Vec<ColumnExpr> {
    let mut ops = operations(code);
    assert_eq!(ops.len(), 1);
    let DplyrOperation::Select { columns, .. } = ops.remove(0) else {
        panic!("expected select()");
    };
    columns
}

fn mutate_exprs(code: &str) -> Vec<Expr> {
    let mut ops = operations(code);
    let DplyrOperation::Mutate { assignments, .. } = ops.remove(0) else {
        panic!("expected mutate()");
    };
    assignments.into_iter().map(|entry| entry.expr).collect()
}

fn slice(
    code: &str,
) -> (
    SliceKind,
    Option<Expr>,
    Option<usize>,
    Option<f64>,
    bool,
    bool,
) {
    let mut ops = operations(code);
    let DplyrOperation::Slice { spec, .. } = ops.remove(0) else {
        panic!("expected a slice operation");
    };
    (
        spec.kind,
        spec.order_by,
        spec.n,
        spec.prop,
        spec.with_ties,
        spec.na_rm,
    )
}

fn join(code: &str) -> (Vec<JoinKey>, JoinOptions) {
    let mut ops = operations(code);
    let DplyrOperation::Join { spec, .. } = ops.remove(0) else {
        panic!("expected a join");
    };
    let JoinSpec { by, options, .. } = spec;
    (by, options)
}

fn rejected(code: &str) -> ParseError {
    parse(code).expect_err("must be rejected")
}

fn range(start: Expr, end: Expr) -> Expr {
    Expr::Function {
        name: "__select_range".to_string(),
        args: vec![start, end],
    }
}

fn num(value: f64) -> Expr {
    Expr::Literal(LiteralValue::Number(value))
}

// ----------------------------------------------------------------- selectors

#[test]
fn ranges_parse_into_the_shared_range_marker() {
    assert_eq!(
        selectors("select(x:y)")[0].expr,
        range(
            Expr::Identifier("x".to_string()),
            Expr::Identifier("y".to_string())
        )
    );
    assert_eq!(selectors("select(1:3)")[0].expr, range(num(1.0), num(3.0)));
    assert_eq!(
        selectors("select(-2:-1)")[0].expr,
        range(
            Expr::Unary {
                operator: UnaryOp::Minus,
                expr: Box::new(num(2.0)),
            },
            Expr::Unary {
                operator: UnaryOp::Minus,
                expr: Box::new(num(1.0)),
            }
        )
    );
}

#[test]
fn negative_selectors_use_the_existing_unary_operators() {
    assert_eq!(
        selectors("select(-x)")[0].expr,
        Expr::Unary {
            operator: UnaryOp::Minus,
            expr: Box::new(Expr::Identifier("x".to_string())),
        }
    );
    assert_eq!(
        selectors("select(!x)")[0].expr,
        Expr::Unary {
            operator: UnaryOp::Not,
            expr: Box::new(Expr::Identifier("x".to_string())),
        }
    );
    assert_eq!(
        selectors("select(a & b)")[0].expr,
        Expr::Binary {
            left: Box::new(Expr::Identifier("a".to_string())),
            operator: BinaryOp::And,
            right: Box::new(Expr::Identifier("b".to_string())),
        }
    );
    assert_eq!(
        selectors("select(a | b)")[0].expr,
        Expr::Binary {
            left: Box::new(Expr::Identifier("a".to_string())),
            operator: BinaryOp::Or,
            right: Box::new(Expr::Identifier("b".to_string())),
        }
    );
}

#[test]
fn scalar_select_legacy_shapes_still_parse() {
    let columns = selectors("select(total = x + y)");
    assert_eq!(columns[0].alias.as_deref(), Some("total"));
    assert_eq!(
        columns[0].expr,
        Expr::Binary {
            left: Box::new(Expr::Identifier("x".to_string())),
            operator: BinaryOp::Plus,
            right: Box::new(Expr::Identifier("y".to_string())),
        }
    );
    assert_eq!(
        selectors("select(x, y)")[1].expr,
        Expr::Identifier("y".to_string())
    );
}

#[test]
fn helper_selectors_and_strings_survive_parsing() {
    assert_eq!(
        selectors("select(c(x, y))")[0].expr,
        Expr::Function {
            name: "c".to_string(),
            args: vec![
                Expr::Identifier("x".to_string()),
                Expr::Identifier("y".to_string())
            ],
        }
    );
    assert_eq!(
        selectors("select(where(is.numeric))")[0].expr,
        Expr::Function {
            name: "where".to_string(),
            args: vec![Expr::Identifier("is.numeric".to_string())],
        }
    );
    assert_eq!(
        selectors("select(starts_with('x'))")[0].expr,
        Expr::Function {
            name: "starts_with".to_string(),
            args: vec![Expr::Literal(LiteralValue::String("x".to_string()))],
        }
    );
}

#[test]
fn chained_renames_are_rejected_instead_of_chained() {
    rejected("select(new = old = x)");
    rejected("select(a = b = c = d)");
}

#[test]
fn a_long_assignment_chain_fails_without_overflowing_the_stack() {
    // A recursive implementation would recurse per link here and blow the stack
    // before any AST check ran.
    let mut code = String::from("select(");
    for index in 0..20_000 {
        if index > 0 {
            code.push_str(" = ");
        }
        code.push_str(&format!("n{index}"));
    }
    code.push(')');
    rejected(&code);
}

// -------------------------------------------------------------------- across

#[test]
fn bare_across_uses_the_empty_column_sentinel() {
    let exprs = mutate_exprs("mutate(across(c(x, y), ~ .x * 2))");
    assert_eq!(exprs.len(), 1);
    assert_eq!(
        exprs[0],
        Expr::Function {
            name: "across".to_string(),
            args: vec![
                Expr::Function {
                    name: "c".to_string(),
                    args: vec![
                        Expr::Identifier("x".to_string()),
                        Expr::Identifier("y".to_string())
                    ],
                },
                Expr::Function {
                    name: "__across_lambda".to_string(),
                    args: vec![Expr::Binary {
                        left: Box::new(Expr::Identifier(".x".to_string())),
                        operator: BinaryOp::Multiply,
                        right: Box::new(num(2.0)),
                    }],
                },
            ],
        }
    );
}

#[test]
fn across_arguments_keep_their_named_shapes() {
    let exprs = mutate_exprs(
        "mutate(across(.cols = c(x, y), .fns = list(round, plus = ~ .x + 1), .names = '{.col}_{.fn}'))",
    );
    let Expr::Function { name, args } = &exprs[0] else {
        panic!("expected across()");
    };
    assert_eq!(name, "across");
    assert_eq!(args.len(), 3);
    assert!(matches!(&args[0], Expr::NamedArg { name, .. } if name == ".cols"));
    assert!(matches!(&args[1], Expr::NamedArg { name, .. } if name == ".fns"));
    assert!(matches!(&args[2], Expr::NamedArg { name, .. } if name == ".names"));

    let Expr::NamedArg { value, .. } = &args[1] else {
        panic!("expected .fns");
    };
    let Expr::Function { args: fns, .. } = value.as_ref() else {
        panic!("expected a .fns list");
    };
    let Expr::NamedArg { value, .. } = &fns[1] else {
        panic!("expected the named lambda entry");
    };
    let Expr::Function { name, args } = value.as_ref() else {
        panic!("expected a lambda marker");
    };
    assert_eq!(name, "__across_lambda");
    assert_eq!(args.len(), 1);
}

#[test]
fn across_lambdas_allow_the_bare_dot_and_dot_x_only() {
    for (code, expected) in [
        ("mutate(across(x, ~ . + 1))", "."),
        ("mutate(across(x, ~ .x + 1))", ".x"),
    ] {
        let exprs = mutate_exprs(code);
        let Expr::Function { args, .. } = &exprs[0] else {
            panic!("expected across()");
        };
        let Expr::Function { args: body, .. } = &args[1] else {
            panic!("expected a lambda marker");
        };
        let Expr::Binary { left, .. } = &body[0] else {
            panic!("expected a binary body");
        };
        assert_eq!(**left, Expr::Identifier(expected.to_string()));
    }
    // Any other dot-prefixed variable is not a lambda placeholder.
    rejected("mutate(across(x, ~ .y + 1))");
}

#[test]
fn across_lambdas_allow_nested_named_arguments() {
    let exprs = mutate_exprs("mutate(across(x, ~ round(.x, na.rm = TRUE)))");
    let Expr::Function { args, .. } = &exprs[0] else {
        panic!("expected across()");
    };
    let Expr::Function { args: body, .. } = &args[1] else {
        panic!("expected a lambda marker");
    };
    let Expr::Function { name, args } = &body[0] else {
        panic!("expected round()");
    };
    assert_eq!(name, "round");
    assert!(matches!(&args[1], Expr::NamedArg { name, .. } if name == "na.rm"));
}

#[test]
fn across_is_parsed_in_mixed_mutate_and_summarise_lists() {
    let exprs = mutate_exprs("mutate(k = x * 2, across(x, ~ .x + k), z = y + 1)");
    assert_eq!(exprs.len(), 3);
    assert!(matches!(&exprs[1], Expr::Function { name, .. } if name == "across"));

    let mut ops = operations("group_by(g) %>% summarise(across(x, ~ sum(.x)), n = n())");
    let DplyrOperation::SummariseExpressions { assignments, .. } = ops.remove(1) else {
        panic!("expected a mixed summarise");
    };
    assert_eq!(assignments.len(), 2);
    assert!(matches!(
        &assignments[0].expr,
        Expr::Function { name, .. } if name == "across"
    ));
}

#[test]
fn named_across_output_is_rejected() {
    rejected("mutate(out = across(c(x, y), ~ .x))");
    rejected("summarise(out = across(x, ~ sum(.x)))");
}

#[test]
fn across_never_takes_the_compact_aggregation_shape() {
    // across() with a single identifier or no arguments is not the
    // function(identifier) shape, so it stays an expression and keeps the
    // empty-column sentinel.
    let exprs = mutate_exprs("mutate(across(x))");
    assert!(matches!(&exprs[0], Expr::Function { name, .. } if name == "across"));

    let mut ops = operations("summarise(across(x))");
    let DplyrOperation::SummariseExpressions { assignments, .. } = ops.remove(0) else {
        panic!("across() in summarise must use the expression shape");
    };
    assert!(assignments[0].column.is_empty());
}

// -------------------------------------------------------------------- slices

#[test]
fn slice_min_max_defaults() {
    let (kind, order_by, n, prop, with_ties, na_rm) = slice("data %>% slice_min(val)");
    assert_eq!(kind, SliceKind::Min);
    assert_eq!(order_by, Some(Expr::Identifier("val".to_string())));
    assert_eq!(n, Some(1));
    assert_eq!(prop, None);
    assert!(with_ties, "with_ties defaults to TRUE");
    assert!(na_rm, "na_rm defaults to TRUE");

    let (kind, _, n, _, _, _) = slice("data %>% slice_max(val, n = 2)");
    assert_eq!(kind, SliceKind::Max);
    assert_eq!(n, Some(2));
}

#[test]
fn slice_sample_defaults() {
    let (kind, _, n, _, with_ties, _) = slice("data %>% slice_sample()");
    assert_eq!(kind, SliceKind::Sample);
    assert_eq!(n, Some(1));
    assert!(!with_ties);
}

#[test]
fn slice_order_by_and_by_selector_parse() {
    let (kind, order_by, n, _, _, _) =
        slice("data %>% slice_min(order_by = alt, n = 2, by = 'grp')");
    assert_eq!(kind, SliceKind::Min);
    assert_eq!(order_by, Some(Expr::Identifier("alt".to_string())));
    assert_eq!(n, Some(2));

    let mut ops = operations("data %>% slice_max(val, n = 1, by = c('grp', 'sub'))");
    let DplyrOperation::Slice { spec, .. } = ops.remove(0) else {
        panic!("expected a slice");
    };
    assert_eq!(spec.by.len(), 2);
    assert_eq!(
        spec.by[0].expr,
        Expr::Literal(LiteralValue::String("grp".to_string()))
    );
    assert_eq!(
        spec.by[1].expr,
        Expr::Literal(LiteralValue::String("sub".to_string()))
    );
}

#[test]
fn slice_order_by_is_accepted_as_a_named_argument() {
    let (kind, order_by, n, _, _, _) = slice("data %>% slice_min(order_by = alt, n = 2)");
    assert_eq!(kind, SliceKind::Min);
    assert_eq!(order_by, Some(Expr::Identifier("alt".to_string())));
    assert_eq!(n, Some(2));

    rejected("data %>% slice_min(val, order_by = alt, order_by = alt)");
    rejected("data %>% slice_min(val, n = 2, n = 3)");
}

#[test]
fn slice_rejects_n_and_prop_together() {
    rejected("data %>% slice_min(val, n = 2, prop = 0.5)");
    rejected("data %>% slice_sample(n = 1, prop = 0.5)");
}

#[test]
fn slice_rejects_out_of_range_counts() {
    rejected("data %>% slice_max(val, n = -1)");
    assert_eq!(slice("data %>% slice_max(val, n = 1.5)").2, Some(1));
    rejected("data %>% slice_max(val, prop = -0.2)");
    rejected("data %>% slice_sample(prop = -1)");
}

#[test]
fn slice_accepts_a_prop_above_one() {
    // prop > 1 truncates the group instead of failing, so it is legal.
    let (kind, _, _, prop, _, _) = slice("data %>% slice_max(val, prop = 1.5)");
    assert_eq!(kind, SliceKind::Max);
    assert_eq!(prop, Some(1.5));
}

#[test]
fn slice_requires_an_order_expression_for_min_and_max() {
    rejected("data %>% slice_min(n = 2)");
    let (_, order_by, _, _, _, _) = slice("data %>% slice_max(order_by = NULL)");
    assert_eq!(order_by, Some(Expr::Literal(LiteralValue::Null)));
}

// --------------------------------------------------------------------- joins

#[test]
fn join_options_parse_with_documented_defaults() {
    let (by, options) = join("data %>% inner_join(other, by = 'id')");
    assert_eq!(
        by,
        vec![JoinKey {
            left: "id".to_string(),
            right: "id".to_string()
        }]
    );
    assert_eq!(options, JoinOptions::default());
    assert_eq!(options.suffix, (".x".to_string(), ".y".to_string()));
    assert!(!options.keep);
    assert!(!options.na_matches);
}

#[test]
fn join_by_forms_and_options_parse() {
    let (by, _) = join("data %>% left_join(other, by = c('a', b = 'B'))");
    assert_eq!(by.len(), 2);
    assert_eq!(
        by[1],
        JoinKey {
            left: "b".to_string(),
            right: "B".to_string()
        }
    );

    let (_, options) = join(
        "data %>% left_join(other, by = 'id', suffix = c('_l', '_r'), keep = TRUE, na_matches = 'na')",
    );
    assert_eq!(options.suffix, ("_l".to_string(), "_r".to_string()));
    assert!(options.keep);
    assert!(options.na_matches);

    let (_, options) = join("data %>% inner_join(other, by = 'id', keep = NULL)");
    assert!(!options.keep);
    let (_, options) = join("data %>% inner_join(other, by = 'id', na_matches = 'never')");
    assert!(!options.na_matches);
}

#[test]
fn join_by_null_defers_to_the_shared_columns() {
    let (by, _) = join("data %>% inner_join(other, by = NULL)");
    assert!(by.is_empty(), "a natural join is resolved from the schema");
}

#[test]
fn join_by_accepts_equal_identifiers() {
    let (by, _) = join("data %>% inner_join(other, by = c(id, grp))");
    assert_eq!(by.len(), 2);
    assert_eq!(
        by[0],
        JoinKey {
            left: "id".to_string(),
            right: "id".to_string()
        }
    );

    let (by, _) = join("data %>% inner_join(other, join_by(id == other_id))");
    assert_eq!(
        by,
        vec![JoinKey {
            left: "id".to_string(),
            right: "other_id".to_string()
        }]
    );
}

#[test]
fn join_accepts_only_no_op_data_dependent_options() {
    join("data %>% inner_join(other, by = 'id', multiple = NULL)");
    join("data %>% inner_join(other, by = 'id', multiple = 'all')");
    join("data %>% inner_join(other, by = 'id', unmatched = 'drop')");
    join("data %>% inner_join(other, by = 'id', relationship = NULL)");
    join("data %>% inner_join(other, by = 'id', relationship = 'many-to-many')");
}

#[test]
fn join_rejects_invalid_options() {
    for code in [
        "data %>% inner_join(other, by = 'id', na_matches = 'maybe')",
        "data %>% inner_join(other, by = 'id', suffix = '_l')",
        "data %>% inner_join(other, by = 'id', keep = 'yes')",
        "data %>% inner_join(other, by = 'id', na_matches = 'na', na_matches = 'never')",
        "data %>% inner_join(other, by = 'id', nope = TRUE)",
    ] {
        rejected(code);
    }
}

#[test]
fn join_requires_a_table_argument() {
    rejected("data %>% inner_join(by = 'id')");
}

// --------------------------------------------------------------------- depth

#[test]
fn across_nesting_is_bounded_at_64_levels() {
    // Each lambda adds one nesting level on top of the function call itself, so
    // 31 wrapped bodies sit just inside the bound and one more crosses it.
    let mut body = String::from("x");
    for _ in 0..30 {
        body = format!("abs({body})");
    }
    parse(&format!("mutate(across(x, ~ {body}))")).expect("legal nesting parses");

    let mut too_deep = String::from("x");
    for _ in 0..80 {
        too_deep = format!("abs({too_deep})");
    }
    let error = parse(&format!("mutate(across(x, ~ {too_deep}))"))
        .expect_err("over-deep nesting is rejected");
    assert!(error.to_string().contains("depth"), "{error}");
}
