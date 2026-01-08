use super::*;
use crate::lexer::Lexer;

#[test]
fn test_parse_simple_select() {
    let lexer = Lexer::new("select(name, age)".to_string());
    let mut parser = Parser::new(lexer).unwrap();

    let ast = parser.parse().unwrap();

    if let DplyrNode::Pipeline { operations, .. } = ast {
        assert_eq!(operations.len(), 1);
        if let DplyrOperation::Select { columns, .. } = &operations[0] {
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

#[test]
fn test_parse_mutate() {
    let lexer = Lexer::new("mutate(new_col = age * 2)".to_string());
    let mut parser = Parser::new(lexer).unwrap();

    let ast = parser.parse().unwrap();

    if let DplyrNode::Pipeline { operations, .. } = ast {
        assert_eq!(operations.len(), 1);
        if let DplyrOperation::Mutate { assignments, .. } = &operations[0] {
            assert_eq!(assignments.len(), 1);
            assert_eq!(assignments[0].column, "new_col");
        } else {
            panic!("Expected Mutate operation");
        }
    }
}

#[test]
fn test_parse_single_table_inner_join() {
    let input = "inner_join(df2, by = \"id\")";
    let lexer = Lexer::new(input.to_string());
    let mut parser = Parser::new(lexer).unwrap();

    let ast = parser.parse();

    assert!(ast.is_ok(), "Should parse single-table join");

    let ast = ast.unwrap();

    if let DplyrNode::Pipeline { operations, .. } = ast {
        assert_eq!(operations.len(), 1);
        if let DplyrOperation::Join {
            join_type, spec, ..
        } = &operations[0]
        {
            assert!(matches!(join_type, JoinType::Inner));
            assert_eq!(spec.table, "df2");
        } else {
            panic!("Expected Join operation");
        }
    } else {
        panic!("Expected Pipeline node");
    }
}

#[test]
fn test_parse_single_table_left_join() {
    let input = "left_join(df2, by = \"id\")";
    let lexer = Lexer::new(input.to_string());
    let mut parser = Parser::new(lexer).unwrap();

    let ast = parser.parse();

    assert!(ast.is_ok(), "Should parse single-table join");

    let ast = ast.unwrap();

    if let DplyrNode::Pipeline { operations, .. } = ast {
        assert_eq!(operations.len(), 1);
        if let DplyrOperation::Join {
            join_type, spec, ..
        } = &operations[0]
        {
            assert!(matches!(join_type, JoinType::Left));
            assert_eq!(spec.table, "df2");
        } else {
            panic!("Expected Join operation");
        }
    } else {
        panic!("Expected Pipeline node");
    }
}

#[test]
fn test_parse_single_table_right_join() {
    let input = "right_join(df2, by = \"id\")";
    let lexer = Lexer::new(input.to_string());
    let mut parser = Parser::new(lexer).unwrap();

    let ast = parser.parse();

    assert!(ast.is_ok(), "Should parse single-table join");

    let ast = ast.unwrap();

    if let DplyrNode::Pipeline { operations, .. } = ast {
        assert_eq!(operations.len(), 1);
        if let DplyrOperation::Join {
            join_type, spec, ..
        } = &operations[0]
        {
            assert!(matches!(join_type, JoinType::Right));
            assert_eq!(spec.table, "df2");
        } else {
            panic!("Expected Join operation");
        }
    } else {
        panic!("Expected Pipeline node");
    }
}

#[test]
fn test_parse_single_table_full_join() {
    let input = "full_join(df2, by = \"id\")";
    let lexer = Lexer::new(input.to_string());
    let mut parser = Parser::new(lexer).unwrap();

    let ast = parser.parse();

    assert!(ast.is_ok(), "Should parse single-table join");

    let ast = ast.unwrap();

    if let DplyrNode::Pipeline { operations, .. } = ast {
        assert_eq!(operations.len(), 1);
        if let DplyrOperation::Join {
            join_type, spec, ..
        } = &operations[0]
        {
            assert!(matches!(join_type, JoinType::Full));
            assert_eq!(spec.table, "df2");
        } else {
            panic!("Expected Join operation");
        }
    } else {
        panic!("Expected Pipeline node");
    }
}

#[test]
fn test_parse_single_table_semi_join() {
    let input = "semi_join(df2, by = \"id\")";
    let lexer = Lexer::new(input.to_string());
    let mut parser = Parser::new(lexer).unwrap();

    let ast = parser.parse();

    assert!(ast.is_ok(), "Should parse single-table join");

    let ast = ast.unwrap();

    if let DplyrNode::Pipeline { operations, .. } = ast {
        assert_eq!(operations.len(), 1);
        if let DplyrOperation::Join {
            join_type, spec, ..
        } = &operations[0]
        {
            assert!(matches!(join_type, JoinType::Semi));
            assert_eq!(spec.table, "df2");
        } else {
            panic!("Expected Join operation");
        }
    } else {
        panic!("Expected Pipeline node");
    }
}

#[test]
fn test_parse_single_table_anti_join() {
    let input = "anti_join(df2, by = \"id\")";
    let lexer = Lexer::new(input.to_string());
    let mut parser = Parser::new(lexer).unwrap();

    let ast = parser.parse();

    assert!(ast.is_ok(), "Should parse single-table join");

    let ast = ast.unwrap();

    if let DplyrNode::Pipeline { operations, .. } = ast {
        assert_eq!(operations.len(), 1);
        if let DplyrOperation::Join {
            join_type, spec, ..
        } = &operations[0]
        {
            assert!(matches!(join_type, JoinType::Anti));
            assert_eq!(spec.table, "df2");
        } else {
            panic!("Expected Join operation");
        }
    } else {
        panic!("Expected Pipeline node");
    }
}

#[test]
fn test_parse_arrange() {
    let lexer = Lexer::new("arrange(desc(age), name)".to_string());
    let mut parser = Parser::new(lexer).unwrap();

    let ast = parser.parse().unwrap();

    if let DplyrNode::Pipeline { operations, .. } = ast {
        assert_eq!(operations.len(), 1);
        if let DplyrOperation::Arrange { columns, .. } = &operations[0] {
            assert_eq!(columns.len(), 2);
            assert_eq!(columns[0].direction, OrderDirection::Desc);
            assert_eq!(columns[1].direction, OrderDirection::Asc);
        } else {
            panic!("Expected Arrange operation");
        }
    }
}

#[test]
fn test_parse_group_by() {
    let lexer = Lexer::new("group_by(department, team)".to_string());
    let mut parser = Parser::new(lexer).unwrap();

    let ast = parser.parse().unwrap();

    if let DplyrNode::Pipeline { operations, .. } = ast {
        assert_eq!(operations.len(), 1);
        if let DplyrOperation::GroupBy { columns, .. } = &operations[0] {
            assert_eq!(columns.len(), 2);
            assert_eq!(columns[0], "department");
            assert_eq!(columns[1], "team");
        } else {
            panic!("Expected GroupBy operation");
        }
    }
}

#[test]
fn test_parse_summarise() {
    let lexer = Lexer::new("summarise(avg_age = mean(age), count = n())".to_string());
    let mut parser = Parser::new(lexer).unwrap();

    let ast = parser.parse().unwrap();

    if let DplyrNode::Pipeline { operations, .. } = ast {
        assert_eq!(operations.len(), 1);
        if let DplyrOperation::Summarise { aggregations, .. } = &operations[0] {
            assert_eq!(aggregations.len(), 2);
            assert_eq!(aggregations[0].alias, Some("avg_age".to_string()));
            assert_eq!(aggregations[0].function, "mean");
            assert_eq!(aggregations[1].alias, Some("count".to_string()));
            assert_eq!(aggregations[1].function, "n");
        } else {
            panic!("Expected Summarise operation");
        }
    }
}

#[test]
fn test_parse_pipeline() {
    let lexer =
        Lexer::new("select(name, age) %>% filter(age > 18) %>% arrange(desc(age))".to_string());
    let mut parser = Parser::new(lexer).unwrap();

    let ast = parser.parse().unwrap();

    if let DplyrNode::Pipeline { operations, .. } = ast {
        assert_eq!(operations.len(), 3);
        assert!(matches!(operations[0], DplyrOperation::Select { .. }));
        assert!(matches!(operations[1], DplyrOperation::Filter { .. }));
        assert!(matches!(operations[2], DplyrOperation::Arrange { .. }));
    } else {
        panic!("Expected Pipeline node");
    }
}

#[test]
fn test_parse_complex_expression() {
    let lexer = Lexer::new("filter(age > 18 & salary >= 50000 | department == \"IT\")".to_string());
    let mut parser = Parser::new(lexer).unwrap();

    let ast = parser.parse().unwrap();

    if let DplyrNode::Pipeline { operations, .. } = ast {
        assert_eq!(operations.len(), 1);
        if let DplyrOperation::Filter { condition, .. } = &operations[0] {
            // Verify that we have a complex binary expression
            assert!(matches!(condition, Expr::Binary { .. }));
        } else {
            panic!("Expected Filter operation");
        }
    }
}

#[test]
fn test_parse_literals() {
    let lexer = Lexer::new("filter(active == TRUE & score > 85.5 & name != NULL)".to_string());
    let mut parser = Parser::new(lexer).unwrap();

    let ast = parser.parse().unwrap();

    if let DplyrNode::Pipeline { operations, .. } = ast {
        assert_eq!(operations.len(), 1);
        assert!(matches!(operations[0], DplyrOperation::Filter { .. }));
    }
}

#[test]
fn test_parse_function_calls() {
    let lexer =
        Lexer::new("mutate(upper_name = toupper(name), len = length(description))".to_string());
    let mut parser = Parser::new(lexer).unwrap();

    let ast = parser.parse().unwrap();

    if let DplyrNode::Pipeline { operations, .. } = ast {
        assert_eq!(operations.len(), 1);
        if let DplyrOperation::Mutate { assignments, .. } = &operations[0] {
            assert_eq!(assignments.len(), 2);
            // Verify function calls in expressions
            if let Expr::Function { name, args } = &assignments[0].expr {
                assert_eq!(name, "toupper");
                assert_eq!(args.len(), 1);
            }
        }
    }
}

// Error handling tests
#[test]
fn test_parse_error_unexpected_token() {
    let lexer = Lexer::new("invalid_function(test)".to_string());
    let mut parser = Parser::new(lexer).unwrap();

    let result = parser.parse();
    assert!(result.is_err());

    match result.unwrap_err() {
        ParseError::UnexpectedToken {
            expected, found, ..
        } => {
            assert_eq!(expected, "dplyr function or pipe operator");
            assert!(found.contains("invalid_function"));
        }
        other => panic!("Expected UnexpectedToken error, got: {other:?}"),
    }
}

#[test]
fn test_parse_error_empty_pipeline() {
    let lexer = Lexer::new("".to_string());
    let mut parser = Parser::new(lexer).unwrap();

    let result = parser.parse();
    assert!(result.is_err());

    if let Err(ParseError::InvalidOperation { operation, .. }) = result {
        assert_eq!(operation, "empty pipeline");
    } else {
        panic!("Expected InvalidOperation error");
    }
}

#[test]
fn test_parse_error_missing_parentheses() {
    let lexer = Lexer::new("select name, age".to_string());
    let mut parser = Parser::new(lexer).unwrap();

    let result = parser.parse();
    assert!(result.is_err());

    match result.unwrap_err() {
        ParseError::UnexpectedToken {
            expected, found, ..
        } => {
            assert_eq!(expected, "(");
            assert!(found.contains("name"));
        }
        other => panic!("Expected UnexpectedToken error, got: {other:?}"),
    }
}

#[test]
fn test_source_location_tracking() {
    let lexer = Lexer::new("select(name)".to_string());
    let mut parser = Parser::new(lexer).unwrap();

    let ast = parser.parse().unwrap();

    if let DplyrNode::Pipeline {
        operations,
        location,
        ..
    } = ast
    {
        // Verify location information is present (start_location from current_location())
        assert_eq!(location.line, 1);
        assert_eq!(location.column, 1);

        // Verify operation location (actual location from parser)
        let op_location = operations[0].location();
        assert!(op_location.line > 0);
        assert!(op_location.column > 0);
    }
}

#[test]
fn test_operation_name_method() {
    let lexer = Lexer::new("select(name) %>% filter(age > 18) %>% mutate(new_col = 1)".to_string());
    let mut parser = Parser::new(lexer).unwrap();

    let ast = parser.parse().unwrap();

    if let DplyrNode::Pipeline { operations, .. } = ast {
        assert_eq!(operations[0].operation_name(), "select");
        assert_eq!(operations[1].operation_name(), "filter");
        assert_eq!(operations[2].operation_name(), "mutate");
    }
}

#[test]
fn test_dplyr_node_methods() {
    let lexer = Lexer::new("select(name)".to_string());
    let mut parser = Parser::new(lexer).unwrap();

    let ast = parser.parse().unwrap();

    assert!(ast.is_pipeline());
    assert!(!ast.is_data_source());
    // Note: location() returns start_location from current_location() which has line = 1
    assert_eq!(ast.location().line, 1);
}

// ===== select() 함수 파싱 테스트 =====

mod select_parsing_tests {
    use super::*;

    #[test]
    fn test_select_single_column() {
        let lexer = Lexer::new("select(name)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Select { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 1);
                assert_eq!(columns[0].expr, Expr::Identifier("name".to_string()));
                assert_eq!(columns[0].alias, None);
            } else {
                panic!("Expected Select operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_select_multiple_columns() {
        let lexer = Lexer::new("select(name, age, salary)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Select { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 3);

                // Check first column
                assert_eq!(columns[0].expr, Expr::Identifier("name".to_string()));
                assert_eq!(columns[0].alias, None);

                // Check second column
                assert_eq!(columns[1].expr, Expr::Identifier("age".to_string()));
                assert_eq!(columns[1].alias, None);

                // Check third column
                assert_eq!(columns[2].expr, Expr::Identifier("salary".to_string()));
                assert_eq!(columns[2].alias, None);
            } else {
                panic!("Expected Select operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_select_with_alias() {
        let lexer = Lexer::new("select(full_name = name, years = age)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Select { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 2);

                // Check first column with alias
                assert_eq!(columns[0].expr, Expr::Identifier("name".to_string()));
                assert_eq!(columns[0].alias, Some("full_name".to_string()));

                // Check second column with alias
                assert_eq!(columns[1].expr, Expr::Identifier("age".to_string()));
                assert_eq!(columns[1].alias, Some("years".to_string()));
            } else {
                panic!("Expected Select operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_select_mixed_alias_and_regular() {
        let lexer = Lexer::new("select(name, full_name = first_name, age)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Select { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 3);

                // Check first column (no alias)
                assert_eq!(columns[0].expr, Expr::Identifier("name".to_string()));
                assert_eq!(columns[0].alias, None);

                // Check second column (with alias)
                assert_eq!(columns[1].expr, Expr::Identifier("first_name".to_string()));
                assert_eq!(columns[1].alias, Some("full_name".to_string()));

                // Check third column (no alias)
                assert_eq!(columns[2].expr, Expr::Identifier("age".to_string()));
                assert_eq!(columns[2].alias, None);
            } else {
                panic!("Expected Select operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_select_with_function_call() {
        let lexer = Lexer::new("select(upper(name), length(description))".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Select { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 2);

                // Check first column (function call)
                if let Expr::Function { name, args } = &columns[0].expr {
                    assert_eq!(name, "upper");
                    assert_eq!(args.len(), 1);
                    assert_eq!(args[0], Expr::Identifier("name".to_string()));
                } else {
                    panic!("Expected function call expression");
                }
                assert_eq!(columns[0].alias, None);

                // Check second column (function call)
                if let Expr::Function { name, args } = &columns[1].expr {
                    assert_eq!(name, "length");
                    assert_eq!(args.len(), 1);
                    assert_eq!(args[0], Expr::Identifier("description".to_string()));
                } else {
                    panic!("Expected function call expression");
                }
                assert_eq!(columns[1].alias, None);
            } else {
                panic!("Expected Select operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_select_with_function_call_and_alias() {
        let lexer = Lexer::new(
            "select(name_upper = upper(name), desc_len = length(description))".to_string(),
        );
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Select { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 2);

                // Check first column (function call with alias)
                if let Expr::Function { name, args } = &columns[0].expr {
                    assert_eq!(name, "upper");
                    assert_eq!(args.len(), 1);
                    assert_eq!(args[0], Expr::Identifier("name".to_string()));
                } else {
                    panic!("Expected function call expression");
                }
                assert_eq!(columns[0].alias, Some("name_upper".to_string()));

                // Check second column (function call with alias)
                if let Expr::Function { name, args } = &columns[1].expr {
                    assert_eq!(name, "length");
                    assert_eq!(args.len(), 1);
                    assert_eq!(args[0], Expr::Identifier("description".to_string()));
                } else {
                    panic!("Expected function call expression");
                }
                assert_eq!(columns[1].alias, Some("desc_len".to_string()));
            } else {
                panic!("Expected Select operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_select_empty() {
        let lexer = Lexer::new("select()".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Select { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 0);
            } else {
                panic!("Expected Select operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_select_with_string_literal() {
        let lexer = Lexer::new("select(\"name\", 'age')".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Select { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 2);

                // Check first column (string literal)
                assert_eq!(
                    columns[0].expr,
                    Expr::Literal(LiteralValue::String("name".to_string()))
                );
                assert_eq!(columns[0].alias, None);

                // Check second column (string literal)
                assert_eq!(
                    columns[1].expr,
                    Expr::Literal(LiteralValue::String("age".to_string()))
                );
                assert_eq!(columns[1].alias, None);
            } else {
                panic!("Expected Select operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_select_with_arithmetic_expression() {
        let lexer = Lexer::new("select(salary_doubled = salary * 2)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Select { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 1);

                // Check arithmetic expression with alias
                if let Expr::Binary {
                    left,
                    operator,
                    right,
                } = &columns[0].expr
                {
                    assert_eq!(**left, Expr::Identifier("salary".to_string()));
                    assert_eq!(*operator, BinaryOp::Multiply);
                    assert_eq!(**right, Expr::Literal(LiteralValue::Number(2.0)));
                } else {
                    panic!("Expected binary expression");
                }
                assert_eq!(columns[0].alias, Some("salary_doubled".to_string()));
            } else {
                panic!("Expected Select operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }
}

// ===== filter() 함수 파싱 테스트 =====

mod filter_parsing_tests {
    use super::*;

    #[test]
    fn test_filter_simple_comparison() {
        let lexer = Lexer::new("filter(age > 18)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Filter { condition, .. } = &operations[0] {
                // Check binary expression
                if let Expr::Binary {
                    left,
                    operator,
                    right,
                } = condition
                {
                    assert_eq!(**left, Expr::Identifier("age".to_string()));
                    assert_eq!(*operator, BinaryOp::GreaterThan);
                    assert_eq!(**right, Expr::Literal(LiteralValue::Number(18.0)));
                } else {
                    panic!("Expected binary expression");
                }
            } else {
                panic!("Expected Filter operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_filter_equality_comparison() {
        let lexer = Lexer::new("filter(name == \"John\")".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Filter { condition, .. } = &operations[0] {
                if let Expr::Binary {
                    left,
                    operator,
                    right,
                } = condition
                {
                    assert_eq!(**left, Expr::Identifier("name".to_string()));
                    assert_eq!(*operator, BinaryOp::Equal);
                    assert_eq!(
                        **right,
                        Expr::Literal(LiteralValue::String("John".to_string()))
                    );
                } else {
                    panic!("Expected binary expression");
                }
            } else {
                panic!("Expected Filter operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_filter_logical_and() {
        let lexer = Lexer::new("filter(age > 18 & salary > 30000)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Filter { condition, .. } = &operations[0] {
                // Check top-level AND operation
                if let Expr::Binary {
                    left,
                    operator,
                    right,
                } = condition
                {
                    assert_eq!(*operator, BinaryOp::And);

                    // Check left side (age > 18)
                    if let Expr::Binary {
                        left: left_left,
                        operator: left_op,
                        right: left_right,
                    } = &**left
                    {
                        assert_eq!(**left_left, Expr::Identifier("age".to_string()));
                        assert_eq!(*left_op, BinaryOp::GreaterThan);
                        assert_eq!(**left_right, Expr::Literal(LiteralValue::Number(18.0)));
                    } else {
                        panic!("Expected binary expression on left side");
                    }

                    // Check right side (salary > 30000)
                    if let Expr::Binary {
                        left: right_left,
                        operator: right_op,
                        right: right_right,
                    } = &**right
                    {
                        assert_eq!(**right_left, Expr::Identifier("salary".to_string()));
                        assert_eq!(*right_op, BinaryOp::GreaterThan);
                        assert_eq!(**right_right, Expr::Literal(LiteralValue::Number(30000.0)));
                    } else {
                        panic!("Expected binary expression on right side");
                    }
                } else {
                    panic!("Expected binary expression");
                }
            } else {
                panic!("Expected Filter operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_filter_logical_or() {
        let lexer = Lexer::new("filter(department == \"IT\" | department == \"HR\")".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Filter { condition, .. } = &operations[0] {
                // Check top-level OR operation
                if let Expr::Binary {
                    left,
                    operator,
                    right,
                } = condition
                {
                    assert_eq!(*operator, BinaryOp::Or);

                    // Check left side (department == "IT")
                    if let Expr::Binary {
                        left: left_left,
                        operator: left_op,
                        right: left_right,
                    } = &**left
                    {
                        assert_eq!(**left_left, Expr::Identifier("department".to_string()));
                        assert_eq!(*left_op, BinaryOp::Equal);
                        assert_eq!(
                            **left_right,
                            Expr::Literal(LiteralValue::String("IT".to_string()))
                        );
                    } else {
                        panic!("Expected binary expression on left side");
                    }

                    // Check right side (department == "HR")
                    if let Expr::Binary {
                        left: right_left,
                        operator: right_op,
                        right: right_right,
                    } = &**right
                    {
                        assert_eq!(**right_left, Expr::Identifier("department".to_string()));
                        assert_eq!(*right_op, BinaryOp::Equal);
                        assert_eq!(
                            **right_right,
                            Expr::Literal(LiteralValue::String("HR".to_string()))
                        );
                    } else {
                        panic!("Expected binary expression on right side");
                    }
                } else {
                    panic!("Expected binary expression");
                }
            } else {
                panic!("Expected Filter operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_filter_with_function_call() {
        let lexer = Lexer::new("filter(length(name) > 5)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Filter { condition, .. } = &operations[0] {
                if let Expr::Binary {
                    left,
                    operator,
                    right,
                } = condition
                {
                    // Check left side is a function call
                    if let Expr::Function { name, args } = &**left {
                        assert_eq!(name, "length");
                        assert_eq!(args.len(), 1);
                        assert_eq!(args[0], Expr::Identifier("name".to_string()));
                    } else {
                        panic!("Expected function call on left side");
                    }

                    assert_eq!(*operator, BinaryOp::GreaterThan);
                    assert_eq!(**right, Expr::Literal(LiteralValue::Number(5.0)));
                } else {
                    panic!("Expected binary expression");
                }
            } else {
                panic!("Expected Filter operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_filter_with_arithmetic_expression() {
        let lexer = Lexer::new("filter(salary * 12 > 600000)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Filter { condition, .. } = &operations[0] {
                if let Expr::Binary {
                    left,
                    operator,
                    right,
                } = condition
                {
                    // Check left side is an arithmetic expression
                    if let Expr::Binary {
                        left: arith_left,
                        operator: arith_op,
                        right: arith_right,
                    } = &**left
                    {
                        assert_eq!(**arith_left, Expr::Identifier("salary".to_string()));
                        assert_eq!(*arith_op, BinaryOp::Multiply);
                        assert_eq!(**arith_right, Expr::Literal(LiteralValue::Number(12.0)));
                    } else {
                        panic!("Expected arithmetic expression on left side");
                    }

                    assert_eq!(*operator, BinaryOp::GreaterThan);
                    assert_eq!(**right, Expr::Literal(LiteralValue::Number(600000.0)));
                } else {
                    panic!("Expected binary expression");
                }
            } else {
                panic!("Expected Filter operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_filter_complex_nested_conditions() {
        let lexer = Lexer::new(
            "filter((age > 18 & age < 65) | (status == \"VIP\" & salary > 100000))".to_string(),
        );
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Filter { condition, .. } = &operations[0] {
                // This should parse as a complex nested binary expression
                // We'll just verify it's a binary expression with OR at the top level
                if let Expr::Binary { operator, .. } = condition {
                    assert_eq!(*operator, BinaryOp::Or);
                } else {
                    panic!("Expected binary expression");
                }
            } else {
                panic!("Expected Filter operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_filter_with_boolean_and_null() {
        let lexer = Lexer::new("filter(active == TRUE & description != NULL)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Filter { condition, .. } = &operations[0] {
                if let Expr::Binary { operator, .. } = condition {
                    assert_eq!(*operator, BinaryOp::And);
                } else {
                    panic!("Expected binary expression");
                }
            } else {
                panic!("Expected Filter operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_filter_all_comparison_operators() {
        let test_cases = vec![
            ("filter(a < b)", BinaryOp::LessThan),
            ("filter(a <= b)", BinaryOp::LessThanOrEqual),
            ("filter(a > b)", BinaryOp::GreaterThan),
            ("filter(a >= b)", BinaryOp::GreaterThanOrEqual),
            ("filter(a == b)", BinaryOp::Equal),
            ("filter(a != b)", BinaryOp::NotEqual),
        ];

        for (input, expected_op) in test_cases {
            let lexer = Lexer::new(input.to_string());
            let mut parser = Parser::new(lexer).unwrap();

            let ast = parser.parse().unwrap();

            if let DplyrNode::Pipeline { operations, .. } = ast {
                assert_eq!(operations.len(), 1);
                if let DplyrOperation::Filter { condition, .. } = &operations[0] {
                    if let Expr::Binary { operator, .. } = condition {
                        assert_eq!(*operator, expected_op, "Failed for input: {input}");
                    } else {
                        panic!("Expected binary expression for input: {input}");
                    }
                } else {
                    panic!("Expected Filter operation for input: {input}");
                }
            } else {
                panic!("Expected Pipeline node for input: {input}");
            }
        }
    }
}

// ===== mutate() 함수 파싱 테스트 =====

mod mutate_parsing_tests {
    use super::*;

    #[test]
    fn test_mutate_single_assignment() {
        let lexer = Lexer::new("mutate(new_col = age * 2)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Mutate { assignments, .. } = &operations[0] {
                assert_eq!(assignments.len(), 1);

                // Check assignment
                assert_eq!(assignments[0].column, "new_col");

                // Check expression (age * 2)
                if let Expr::Binary {
                    left,
                    operator,
                    right,
                } = &assignments[0].expr
                {
                    assert_eq!(**left, Expr::Identifier("age".to_string()));
                    assert_eq!(*operator, BinaryOp::Multiply);
                    assert_eq!(**right, Expr::Literal(LiteralValue::Number(2.0)));
                } else {
                    panic!("Expected binary expression");
                }
            } else {
                panic!("Expected Mutate operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_mutate_multiple_assignments() {
        let lexer = Lexer::new("mutate(doubled = age * 2, halved = age / 2)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Mutate { assignments, .. } = &operations[0] {
                assert_eq!(assignments.len(), 2);

                // Check first assignment (doubled = age * 2)
                assert_eq!(assignments[0].column, "doubled");
                if let Expr::Binary {
                    left,
                    operator,
                    right,
                } = &assignments[0].expr
                {
                    assert_eq!(**left, Expr::Identifier("age".to_string()));
                    assert_eq!(*operator, BinaryOp::Multiply);
                    assert_eq!(**right, Expr::Literal(LiteralValue::Number(2.0)));
                } else {
                    panic!("Expected binary expression for first assignment");
                }

                // Check second assignment (halved = age / 2)
                assert_eq!(assignments[1].column, "halved");
                if let Expr::Binary {
                    left,
                    operator,
                    right,
                } = &assignments[1].expr
                {
                    assert_eq!(**left, Expr::Identifier("age".to_string()));
                    assert_eq!(*operator, BinaryOp::Divide);
                    assert_eq!(**right, Expr::Literal(LiteralValue::Number(2.0)));
                } else {
                    panic!("Expected binary expression for second assignment");
                }
            } else {
                panic!("Expected Mutate operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_mutate_with_function_call() {
        let lexer = Lexer::new("mutate(name_upper = upper(name))".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Mutate { assignments, .. } = &operations[0] {
                assert_eq!(assignments.len(), 1);

                // Check assignment (name_upper = upper(name))
                assert_eq!(assignments[0].column, "name_upper");
                if let Expr::Function { name, args } = &assignments[0].expr {
                    assert_eq!(name, "upper");
                    assert_eq!(args.len(), 1);
                    assert_eq!(args[0], Expr::Identifier("name".to_string()));
                } else {
                    panic!("Expected function call");
                }
            } else {
                panic!("Expected Mutate operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_mutate_with_complex_expression() {
        let lexer = Lexer::new("mutate(bonus = salary * 0.1 + 1000)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Mutate { assignments, .. } = &operations[0] {
                assert_eq!(assignments.len(), 1);

                // Check assignment (bonus = salary * 0.1 + 1000)
                assert_eq!(assignments[0].column, "bonus");

                // This should parse as: (salary * 0.1) + 1000
                // So top level should be addition
                if let Expr::Binary {
                    left,
                    operator,
                    right,
                } = &assignments[0].expr
                {
                    assert_eq!(*operator, BinaryOp::Plus);

                    // Left side should be salary * 0.1
                    if let Expr::Binary {
                        left: mult_left,
                        operator: mult_op,
                        right: mult_right,
                    } = &**left
                    {
                        assert_eq!(**mult_left, Expr::Identifier("salary".to_string()));
                        assert_eq!(*mult_op, BinaryOp::Multiply);
                        assert_eq!(**mult_right, Expr::Literal(LiteralValue::Number(0.1)));
                    } else {
                        panic!("Expected multiplication on left side");
                    }

                    // Right side should be 1000
                    assert_eq!(**right, Expr::Literal(LiteralValue::Number(1000.0)));
                } else {
                    panic!("Expected binary expression");
                }
            } else {
                panic!("Expected Mutate operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_mutate_with_string_and_boolean() {
        let lexer = Lexer::new("mutate(status = \"active\", is_valid = TRUE)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Mutate { assignments, .. } = &operations[0] {
                assert_eq!(assignments.len(), 2);

                // Check first assignment (status = "active")
                assert_eq!(assignments[0].column, "status");
                assert_eq!(
                    assignments[0].expr,
                    Expr::Literal(LiteralValue::String("active".to_string()))
                );

                // Check second assignment (is_valid = TRUE)
                assert_eq!(assignments[1].column, "is_valid");
                assert_eq!(
                    assignments[1].expr,
                    Expr::Literal(LiteralValue::Boolean(true))
                );
            } else {
                panic!("Expected Mutate operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_mutate_with_column_reference() {
        let lexer = Lexer::new("mutate(age_copy = age)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Mutate { assignments, .. } = &operations[0] {
                assert_eq!(assignments.len(), 1);

                // Check assignment (age_copy = age)
                assert_eq!(assignments[0].column, "age_copy");
                assert_eq!(assignments[0].expr, Expr::Identifier("age".to_string()));
            } else {
                panic!("Expected Mutate operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_mutate_with_nested_function_calls() {
        let lexer = Lexer::new("mutate(processed = upper(substr(name, 1, 3)))".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Mutate { assignments, .. } = &operations[0] {
                assert_eq!(assignments.len(), 1);

                // Check assignment (processed = upper(substr(name, 1, 3)))
                assert_eq!(assignments[0].column, "processed");

                // Check outer function call (upper)
                if let Expr::Function { name, args } = &assignments[0].expr {
                    assert_eq!(name, "upper");
                    assert_eq!(args.len(), 1);

                    // Check inner function call (substr)
                    if let Expr::Function {
                        name: inner_name,
                        args: inner_args,
                    } = &args[0]
                    {
                        assert_eq!(inner_name, "substr");
                        assert_eq!(inner_args.len(), 3);
                        assert_eq!(inner_args[0], Expr::Identifier("name".to_string()));
                        assert_eq!(inner_args[1], Expr::Literal(LiteralValue::Number(1.0)));
                        assert_eq!(inner_args[2], Expr::Literal(LiteralValue::Number(3.0)));
                    } else {
                        panic!("Expected inner function call");
                    }
                } else {
                    panic!("Expected function call");
                }
            } else {
                panic!("Expected Mutate operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_mutate_empty() {
        let lexer = Lexer::new("mutate()".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Mutate { assignments, .. } = &operations[0] {
                assert_eq!(assignments.len(), 0);
            } else {
                panic!("Expected Mutate operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_mutate_arithmetic_operators() {
        let test_cases = vec![
            ("mutate(result = a + b)", BinaryOp::Plus),
            ("mutate(result = a - b)", BinaryOp::Minus),
            ("mutate(result = a * b)", BinaryOp::Multiply),
            ("mutate(result = a / b)", BinaryOp::Divide),
        ];

        for (input, expected_op) in test_cases {
            let lexer = Lexer::new(input.to_string());
            let mut parser = Parser::new(lexer).unwrap();

            let ast = parser.parse().unwrap();

            if let DplyrNode::Pipeline { operations, .. } = ast {
                assert_eq!(operations.len(), 1);
                if let DplyrOperation::Mutate { assignments, .. } = &operations[0] {
                    assert_eq!(assignments.len(), 1);
                    assert_eq!(assignments[0].column, "result");

                    if let Expr::Binary { operator, .. } = &assignments[0].expr {
                        assert_eq!(*operator, expected_op, "Failed for input: {input}");
                    } else {
                        panic!("Expected binary expression for input: {input}");
                    }
                } else {
                    panic!("Expected Mutate operation for input: {input}");
                }
            } else {
                panic!("Expected Pipeline node for input: {input}");
            }
        }
    }
}

// ===== arrange() 함수 파싱 테스트 =====

mod arrange_parsing_tests {
    use super::*;

    #[test]
    fn test_arrange_single_column_ascending() {
        let lexer = Lexer::new("arrange(age)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Arrange { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 1);
                assert_eq!(columns[0].column, "age");
                assert_eq!(columns[0].direction, OrderDirection::Asc);
            } else {
                panic!("Expected Arrange operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_arrange_single_column_descending() {
        let lexer = Lexer::new("arrange(desc(age))".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Arrange { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 1);
                assert_eq!(columns[0].column, "age");
                assert_eq!(columns[0].direction, OrderDirection::Desc);
            } else {
                panic!("Expected Arrange operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_arrange_single_column_explicit_ascending() {
        let lexer = Lexer::new("arrange(asc(age))".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Arrange { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 1);
                assert_eq!(columns[0].column, "age");
                assert_eq!(columns[0].direction, OrderDirection::Asc);
            } else {
                panic!("Expected Arrange operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_arrange_multiple_columns() {
        let lexer = Lexer::new("arrange(name, age, desc(salary))".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Arrange { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 3);

                // First column: name (ascending by default)
                assert_eq!(columns[0].column, "name");
                assert_eq!(columns[0].direction, OrderDirection::Asc);

                // Second column: age (ascending by default)
                assert_eq!(columns[1].column, "age");
                assert_eq!(columns[1].direction, OrderDirection::Asc);

                // Third column: salary (descending)
                assert_eq!(columns[2].column, "salary");
                assert_eq!(columns[2].direction, OrderDirection::Desc);
            } else {
                panic!("Expected Arrange operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_arrange_mixed_directions() {
        let lexer = Lexer::new("arrange(asc(name), desc(age), salary)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Arrange { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 3);

                // First column: name (explicit ascending)
                assert_eq!(columns[0].column, "name");
                assert_eq!(columns[0].direction, OrderDirection::Asc);

                // Second column: age (descending)
                assert_eq!(columns[1].column, "age");
                assert_eq!(columns[1].direction, OrderDirection::Desc);

                // Third column: salary (ascending by default)
                assert_eq!(columns[2].column, "salary");
                assert_eq!(columns[2].direction, OrderDirection::Asc);
            } else {
                panic!("Expected Arrange operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_arrange_empty() {
        let lexer = Lexer::new("arrange()".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Arrange { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 0);
            } else {
                panic!("Expected Arrange operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_arrange_with_underscore_columns() {
        let lexer = Lexer::new("arrange(first_name, desc(last_name))".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Arrange { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 2);

                // First column: first_name (ascending by default)
                assert_eq!(columns[0].column, "first_name");
                assert_eq!(columns[0].direction, OrderDirection::Asc);

                // Second column: last_name (descending)
                assert_eq!(columns[1].column, "last_name");
                assert_eq!(columns[1].direction, OrderDirection::Desc);
            } else {
                panic!("Expected Arrange operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_arrange_complex_column_names() {
        let lexer = Lexer::new("arrange(column_1, desc(column_2), asc(column_3))".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Arrange { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 3);

                // First column: column_1 (ascending by default)
                assert_eq!(columns[0].column, "column_1");
                assert_eq!(columns[0].direction, OrderDirection::Asc);

                // Second column: column_2 (descending)
                assert_eq!(columns[1].column, "column_2");
                assert_eq!(columns[1].direction, OrderDirection::Desc);

                // Third column: column_3 (explicit ascending)
                assert_eq!(columns[2].column, "column_3");
                assert_eq!(columns[2].direction, OrderDirection::Asc);
            } else {
                panic!("Expected Arrange operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }
}

// ===== group_by() 함수 파싱 테스트 =====

mod group_by_parsing_tests {
    use super::*;

    #[test]
    fn test_group_by_single_column() {
        let lexer = Lexer::new("group_by(department)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::GroupBy { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 1);
                assert_eq!(columns[0], "department");
            } else {
                panic!("Expected GroupBy operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_group_by_multiple_columns() {
        let lexer = Lexer::new("group_by(department, team, region)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::GroupBy { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 3);
                assert_eq!(columns[0], "department");
                assert_eq!(columns[1], "team");
                assert_eq!(columns[2], "region");
            } else {
                panic!("Expected GroupBy operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_group_by_with_underscore_columns() {
        let lexer = Lexer::new("group_by(department_id, team_name)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::GroupBy { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 2);
                assert_eq!(columns[0], "department_id");
                assert_eq!(columns[1], "team_name");
            } else {
                panic!("Expected GroupBy operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_group_by_empty() {
        let lexer = Lexer::new("group_by()".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::GroupBy { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 0);
            } else {
                panic!("Expected GroupBy operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_group_by_single_character_columns() {
        let lexer = Lexer::new("group_by(a, b, c)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::GroupBy { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 3);
                assert_eq!(columns[0], "a");
                assert_eq!(columns[1], "b");
                assert_eq!(columns[2], "c");
            } else {
                panic!("Expected GroupBy operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_group_by_mixed_column_names() {
        let lexer = Lexer::new("group_by(dept, team_id, region123)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::GroupBy { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 3);
                assert_eq!(columns[0], "dept");
                assert_eq!(columns[1], "team_id");
                assert_eq!(columns[2], "region123");
            } else {
                panic!("Expected GroupBy operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }
}

// ===== summarise() 함수 파싱 테스트 =====

mod summarise_parsing_tests {
    use super::*;

    #[test]
    fn test_summarise_single_aggregation_with_alias() {
        let lexer = Lexer::new("summarise(avg_age = mean(age))".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Summarise { aggregations, .. } = &operations[0] {
                assert_eq!(aggregations.len(), 1);

                // Check aggregation (avg_age = mean(age))
                assert_eq!(aggregations[0].function, "mean");
                assert_eq!(aggregations[0].column, "age");
                assert_eq!(aggregations[0].alias, Some("avg_age".to_string()));
            } else {
                panic!("Expected Summarise operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_summarise_single_aggregation_without_alias() {
        let lexer = Lexer::new("summarise(mean(age))".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Summarise { aggregations, .. } = &operations[0] {
                assert_eq!(aggregations.len(), 1);

                // Check aggregation (mean(age))
                assert_eq!(aggregations[0].function, "mean");
                assert_eq!(aggregations[0].column, "age");
                assert_eq!(aggregations[0].alias, None);
            } else {
                panic!("Expected Summarise operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_summarise_multiple_aggregations() {
        let lexer = Lexer::new(
            "summarise(avg_age = mean(age), total_count = n(), max_salary = max(salary))"
                .to_string(),
        );
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Summarise { aggregations, .. } = &operations[0] {
                assert_eq!(aggregations.len(), 3);

                // First aggregation: avg_age = mean(age)
                assert_eq!(aggregations[0].function, "mean");
                assert_eq!(aggregations[0].column, "age");
                assert_eq!(aggregations[0].alias, Some("avg_age".to_string()));

                // Second aggregation: total_count = n()
                assert_eq!(aggregations[1].function, "n");
                assert_eq!(aggregations[1].column, ""); // n() has no column
                assert_eq!(aggregations[1].alias, Some("total_count".to_string()));

                // Third aggregation: max_salary = max(salary)
                assert_eq!(aggregations[2].function, "max");
                assert_eq!(aggregations[2].column, "salary");
                assert_eq!(aggregations[2].alias, Some("max_salary".to_string()));
            } else {
                panic!("Expected Summarise operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_summarise_count_function() {
        let lexer = Lexer::new("summarise(count = n())".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Summarise { aggregations, .. } = &operations[0] {
                assert_eq!(aggregations.len(), 1);

                // Check n() function (no column argument)
                assert_eq!(aggregations[0].function, "n");
                assert_eq!(aggregations[0].column, "");
                assert_eq!(aggregations[0].alias, Some("count".to_string()));
            } else {
                panic!("Expected Summarise operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_summarise_count_function_without_alias() {
        let lexer = Lexer::new("summarise(n())".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Summarise { aggregations, .. } = &operations[0] {
                assert_eq!(aggregations.len(), 1);

                // Check n() function without alias
                assert_eq!(aggregations[0].function, "n");
                assert_eq!(aggregations[0].column, "");
                assert_eq!(aggregations[0].alias, None);
            } else {
                panic!("Expected Summarise operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_summarise_various_aggregation_functions() {
        let test_cases = vec![
            ("summarise(result = sum(value))", "sum", "value"),
            ("summarise(result = avg(value))", "avg", "value"),
            ("summarise(result = min(value))", "min", "value"),
            ("summarise(result = max(value))", "max", "value"),
            ("summarise(result = count(value))", "count", "value"),
            ("summarise(result = std(value))", "std", "value"),
            ("summarise(result = var(value))", "var", "value"),
        ];

        for (input, expected_func, expected_col) in test_cases {
            let lexer = Lexer::new(input.to_string());
            let mut parser = Parser::new(lexer).unwrap();

            let ast = parser.parse().unwrap();

            if let DplyrNode::Pipeline { operations, .. } = ast {
                assert_eq!(operations.len(), 1);
                if let DplyrOperation::Summarise { aggregations, .. } = &operations[0] {
                    assert_eq!(aggregations.len(), 1);
                    assert_eq!(
                        aggregations[0].function, expected_func,
                        "Failed for input: {input}"
                    );
                    assert_eq!(
                        aggregations[0].column, expected_col,
                        "Failed for input: {input}"
                    );
                    assert_eq!(
                        aggregations[0].alias,
                        Some("result".to_string()),
                        "Failed for input: {input}"
                    );
                } else {
                    panic!("Expected Summarise operation for input: {input}");
                }
            } else {
                panic!("Expected Pipeline node for input: {}", input);
            }
        }
    }

    #[test]
    fn test_summarise_mixed_with_and_without_alias() {
        let lexer = Lexer::new("summarise(mean(age), total = n(), max(salary))".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Summarise { aggregations, .. } = &operations[0] {
                assert_eq!(aggregations.len(), 3);

                // First aggregation: mean(age) - no alias
                assert_eq!(aggregations[0].function, "mean");
                assert_eq!(aggregations[0].column, "age");
                assert_eq!(aggregations[0].alias, None);

                // Second aggregation: total = n() - with alias
                assert_eq!(aggregations[1].function, "n");
                assert_eq!(aggregations[1].column, "");
                assert_eq!(aggregations[1].alias, Some("total".to_string()));

                // Third aggregation: max(salary) - no alias
                assert_eq!(aggregations[2].function, "max");
                assert_eq!(aggregations[2].column, "salary");
                assert_eq!(aggregations[2].alias, None);
            } else {
                panic!("Expected Summarise operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_summarise_empty() {
        let lexer = Lexer::new("summarise()".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Summarise { aggregations, .. } = &operations[0] {
                assert_eq!(aggregations.len(), 0);
            } else {
                panic!("Expected Summarise operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_summarise_with_underscore_columns() {
        let lexer = Lexer::new(
            "summarise(avg_salary = mean(base_salary), count_employees = n())".to_string(),
        );
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Summarise { aggregations, .. } = &operations[0] {
                assert_eq!(aggregations.len(), 2);

                // First aggregation: avg_salary = mean(base_salary)
                assert_eq!(aggregations[0].function, "mean");
                assert_eq!(aggregations[0].column, "base_salary");
                assert_eq!(aggregations[0].alias, Some("avg_salary".to_string()));

                // Second aggregation: count_employees = n()
                assert_eq!(aggregations[1].function, "n");
                assert_eq!(aggregations[1].column, "");
                assert_eq!(aggregations[1].alias, Some("count_employees".to_string()));
            } else {
                panic!("Expected Summarise operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_summarise_with_numeric_column_names() {
        let lexer = Lexer::new("summarise(avg1 = mean(col1), sum2 = sum(col2))".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Summarise { aggregations, .. } = &operations[0] {
                assert_eq!(aggregations.len(), 2);

                // First aggregation: avg1 = mean(col1)
                assert_eq!(aggregations[0].function, "mean");
                assert_eq!(aggregations[0].column, "col1");
                assert_eq!(aggregations[0].alias, Some("avg1".to_string()));

                // Second aggregation: sum2 = sum(col2)
                assert_eq!(aggregations[1].function, "sum");
                assert_eq!(aggregations[1].column, "col2");
                assert_eq!(aggregations[1].alias, Some("sum2".to_string()));
            } else {
                panic!("Expected Summarise operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }
}

// ===== 파이프라인 파싱 테스트 =====

mod pipeline_parsing_tests {
    use super::*;

    #[test]
    fn test_simple_pipeline_two_operations() {
        let lexer = Lexer::new("select(name) %>% filter(age > 18)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 2);

            // First operation: select(name)
            if let DplyrOperation::Select { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 1);
                assert_eq!(columns[0].expr, Expr::Identifier("name".to_string()));
            } else {
                panic!("Expected Select operation");
            }

            // Second operation: filter(age > 18)
            if let DplyrOperation::Filter { condition, .. } = &operations[1] {
                if let Expr::Binary {
                    left,
                    operator,
                    right,
                } = condition
                {
                    assert_eq!(**left, Expr::Identifier("age".to_string()));
                    assert_eq!(*operator, BinaryOp::GreaterThan);
                    assert_eq!(**right, Expr::Literal(LiteralValue::Number(18.0)));
                } else {
                    panic!("Expected binary expression in filter");
                }
            } else {
                panic!("Expected Filter operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_complex_pipeline_multiple_operations() {
        let input = "select(name, age) %>% filter(age > 18) %>% mutate(adult = TRUE) %>% arrange(desc(age))";
        let lexer = Lexer::new(input.to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 4);

            // Check operation types
            assert!(matches!(operations[0], DplyrOperation::Select { .. }));
            assert!(matches!(operations[1], DplyrOperation::Filter { .. }));
            assert!(matches!(operations[2], DplyrOperation::Mutate { .. }));
            assert!(matches!(operations[3], DplyrOperation::Arrange { .. }));
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_pipeline_with_data_source() {
        let lexer = Lexer::new("data %>% select(name, age) %>% filter(age > 18)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 2);

            // First operation: select(name, age)
            if let DplyrOperation::Select { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 2);
                assert_eq!(columns[0].expr, Expr::Identifier("name".to_string()));
                assert_eq!(columns[1].expr, Expr::Identifier("age".to_string()));
            } else {
                panic!("Expected Select operation");
            }

            // Second operation: filter(age > 18)
            assert!(matches!(operations[1], DplyrOperation::Filter { .. }));
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_pipeline_with_newlines() {
        let input = r#"data %>%
                select(name, age) %>%
                filter(age > 18) %>%
                arrange(desc(age))"#;
        let lexer = Lexer::new(input.to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 3);
            assert!(matches!(operations[0], DplyrOperation::Select { .. }));
            assert!(matches!(operations[1], DplyrOperation::Filter { .. }));
            assert!(matches!(operations[2], DplyrOperation::Arrange { .. }));
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_pipeline_with_complex_expressions() {
        let input = "select(name, age) %>% filter(age >= 18 & age <= 65) %>% mutate(category = age * 2 + 1)";
        let lexer = Lexer::new(input.to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 3);

            // Check filter with complex condition
            if let DplyrOperation::Filter { condition, .. } = &operations[1] {
                if let Expr::Binary {
                    operator: BinaryOp::And,
                    ..
                } = condition
                {
                    // Complex AND condition parsed correctly
                } else {
                    panic!("Expected AND condition in filter");
                }
            } else {
                panic!("Expected Filter operation");
            }

            // Check mutate with complex expression
            if let DplyrOperation::Mutate { assignments, .. } = &operations[2] {
                assert_eq!(assignments.len(), 1);
                assert_eq!(assignments[0].column, "category");
                // The expression should be parsed as a complex binary operation
                if let Expr::Binary { .. } = &assignments[0].expr {
                    // Complex expression parsed correctly
                } else {
                    panic!("Expected complex expression in mutate");
                }
            } else {
                panic!("Expected Mutate operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_pipeline_with_group_by_and_summarise() {
        let input = "group_by(department) %>% summarise(avg_salary = mean(salary), count = n())";
        let lexer = Lexer::new(input.to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 2);

            // Check group_by
            if let DplyrOperation::GroupBy { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 1);
                assert_eq!(columns[0], "department");
            } else {
                panic!("Expected GroupBy operation");
            }

            // Check summarise
            if let DplyrOperation::Summarise { aggregations, .. } = &operations[1] {
                assert_eq!(aggregations.len(), 2);
                assert_eq!(aggregations[0].function, "mean");
                assert_eq!(aggregations[0].column, "salary");
                assert_eq!(aggregations[0].alias, Some("avg_salary".to_string()));
                assert_eq!(aggregations[1].function, "n");
                assert_eq!(aggregations[1].column, "");
                assert_eq!(aggregations[1].alias, Some("count".to_string()));
            } else {
                panic!("Expected Summarise operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_single_data_source() {
        let lexer = Lexer::new("data".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::DataSource { name, .. } = ast {
            assert_eq!(name, "data");
        } else {
            panic!("Expected DataSource node");
        }
    }

    #[test]
    fn test_single_operation_no_data_source() {
        let lexer = Lexer::new("select(name, age)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 1);
            if let DplyrOperation::Select { columns, .. } = &operations[0] {
                assert_eq!(columns.len(), 2);
                assert_eq!(columns[0].expr, Expr::Identifier("name".to_string()));
                assert_eq!(columns[1].expr, Expr::Identifier("age".to_string()));
            } else {
                panic!("Expected Select operation");
            }
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_pipeline_operation_order_preservation() {
        let input = "filter(age > 18) %>% select(name) %>% mutate(adult = TRUE) %>% arrange(name)";
        let lexer = Lexer::new(input.to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { operations, .. } = ast {
            assert_eq!(operations.len(), 4);

            // Verify operation order is preserved
            assert!(matches!(operations[0], DplyrOperation::Filter { .. }));
            assert!(matches!(operations[1], DplyrOperation::Select { .. }));
            assert!(matches!(operations[2], DplyrOperation::Mutate { .. }));
            assert!(matches!(operations[3], DplyrOperation::Arrange { .. }));

            // Verify operation names
            assert_eq!(operations[0].operation_name(), "filter");
            assert_eq!(operations[1].operation_name(), "select");
            assert_eq!(operations[2].operation_name(), "mutate");
            assert_eq!(operations[3].operation_name(), "arrange");
        } else {
            panic!("Expected Pipeline node");
        }
    }

    #[test]
    fn test_empty_pipeline_error() {
        let lexer = Lexer::new("".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        match parser.parse() {
            Err(ParseError::InvalidOperation { operation, .. }) => {
                assert_eq!(operation, "empty pipeline");
            }
            other => panic!("Expected InvalidOperation error, got: {other:?}"),
        }
    }

    #[test]
    fn test_pipeline_with_whitespace_only_error() {
        let lexer = Lexer::new("   \n\t  \n  ".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        match parser.parse() {
            Err(ParseError::InvalidOperation { operation, .. }) => {
                assert_eq!(operation, "empty pipeline");
            }
            other => panic!("Expected InvalidOperation error, got: {other:?}"),
        }
    }

    #[test]
    fn test_pipeline_with_trailing_pipe_error() {
        let lexer = Lexer::new("select(name) %>%".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        match parser.parse() {
            Err(ParseError::UnexpectedToken { .. }) => {
                // Should get an error about unexpected token after pipe
            }
            other => panic!("Expected UnexpectedToken error, got: {other:?}"),
        }
    }

    #[test]
    fn test_pipeline_location_tracking() {
        let lexer = Lexer::new("select(name) %>% filter(age > 18)".to_string());
        let mut parser = Parser::new(lexer).unwrap();

        let ast = parser.parse().unwrap();

        if let DplyrNode::Pipeline { location, .. } = ast {
            // Location should be tracked (not unknown)
            assert_ne!(location.line, 0);
        } else {
            panic!("Expected Pipeline node");
        }
    }

    /// Additional comprehensive tests for parser functionality
    mod comprehensive_tests {
        use super::*;

        /// Tests for individual dplyr function parsing
        mod dplyr_function_tests {
            use super::*;

            #[test]
            fn test_select_single_column() {
                let lexer = Lexer::new("select(name)".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    assert_eq!(operations.len(), 1);
                    if let DplyrOperation::Select { columns, .. } = &operations[0] {
                        assert_eq!(columns.len(), 1);
                        assert_eq!(columns[0].expr, Expr::Identifier("name".to_string()));
                        assert_eq!(columns[0].alias, None);
                    } else {
                        panic!("Expected Select operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_select_multiple_columns() {
                let lexer = Lexer::new("select(name, age, salary)".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    assert_eq!(operations.len(), 1);
                    if let DplyrOperation::Select { columns, .. } = &operations[0] {
                        assert_eq!(columns.len(), 3);
                        assert_eq!(columns[0].expr, Expr::Identifier("name".to_string()));
                        assert_eq!(columns[1].expr, Expr::Identifier("age".to_string()));
                        assert_eq!(columns[2].expr, Expr::Identifier("salary".to_string()));
                    } else {
                        panic!("Expected Select operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_select_with_alias() {
                let lexer = Lexer::new("select(full_name = name, years = age)".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    assert_eq!(operations.len(), 1);
                    if let DplyrOperation::Select { columns, .. } = &operations[0] {
                        assert_eq!(columns.len(), 2);
                        assert_eq!(columns[0].expr, Expr::Identifier("name".to_string()));
                        assert_eq!(columns[0].alias, Some("full_name".to_string()));
                        assert_eq!(columns[1].expr, Expr::Identifier("age".to_string()));
                        assert_eq!(columns[1].alias, Some("years".to_string()));
                    } else {
                        panic!("Expected Select operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_select_with_function_call() {
                let lexer = Lexer::new("select(upper(name), round(salary))".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    assert_eq!(operations.len(), 1);
                    if let DplyrOperation::Select { columns, .. } = &operations[0] {
                        assert_eq!(columns.len(), 2);

                        // First column: upper(name)
                        if let Expr::Function { name, args } = &columns[0].expr {
                            assert_eq!(name, "upper");
                            assert_eq!(args.len(), 1);
                            assert_eq!(args[0], Expr::Identifier("name".to_string()));
                        } else {
                            panic!("Expected function call expression");
                        }

                        // Second column: round(salary)
                        if let Expr::Function { name, args } = &columns[1].expr {
                            assert_eq!(name, "round");
                            assert_eq!(args.len(), 1);
                            assert_eq!(args[0], Expr::Identifier("salary".to_string()));
                        } else {
                            panic!("Expected function call expression");
                        }
                    } else {
                        panic!("Expected Select operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_select_empty() {
                let lexer = Lexer::new("select()".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    assert_eq!(operations.len(), 1);
                    if let DplyrOperation::Select { columns, .. } = &operations[0] {
                        assert_eq!(columns.len(), 0);
                    } else {
                        panic!("Expected Select operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_filter_simple_condition() {
                let lexer = Lexer::new("filter(age > 18)".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    assert_eq!(operations.len(), 1);
                    if let DplyrOperation::Filter { condition, .. } = &operations[0] {
                        if let Expr::Binary {
                            left,
                            operator,
                            right,
                        } = condition
                        {
                            assert_eq!(**left, Expr::Identifier("age".to_string()));
                            assert_eq!(*operator, BinaryOp::GreaterThan);
                            assert_eq!(**right, Expr::Literal(LiteralValue::Number(18.0)));
                        } else {
                            panic!("Expected binary expression");
                        }
                    } else {
                        panic!("Expected Filter operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_filter_complex_condition() {
                let lexer = Lexer::new("filter(age >= 18 & age <= 65)".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    assert_eq!(operations.len(), 1);
                    if let DplyrOperation::Filter { condition, .. } = &operations[0] {
                        if let Expr::Binary {
                            left,
                            operator,
                            right,
                        } = condition
                        {
                            assert_eq!(*operator, BinaryOp::And);

                            // Left side: age >= 18
                            if let Expr::Binary {
                                left: l_left,
                                operator: l_op,
                                right: l_right,
                            } = left.as_ref()
                            {
                                assert_eq!(**l_left, Expr::Identifier("age".to_string()));
                                assert_eq!(*l_op, BinaryOp::GreaterThanOrEqual);
                                assert_eq!(**l_right, Expr::Literal(LiteralValue::Number(18.0)));
                            } else {
                                panic!("Expected binary expression on left side");
                            }

                            // Right side: age <= 65
                            if let Expr::Binary {
                                left: r_left,
                                operator: r_op,
                                right: r_right,
                            } = right.as_ref()
                            {
                                assert_eq!(**r_left, Expr::Identifier("age".to_string()));
                                assert_eq!(*r_op, BinaryOp::LessThanOrEqual);
                                assert_eq!(**r_right, Expr::Literal(LiteralValue::Number(65.0)));
                            } else {
                                panic!("Expected binary expression on right side");
                            }
                        } else {
                            panic!("Expected binary expression");
                        }
                    } else {
                        panic!("Expected Filter operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_filter_string_comparison() {
                let lexer = Lexer::new("filter(name == \"John\")".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    assert_eq!(operations.len(), 1);
                    if let DplyrOperation::Filter { condition, .. } = &operations[0] {
                        if let Expr::Binary {
                            left,
                            operator,
                            right,
                        } = condition
                        {
                            assert_eq!(**left, Expr::Identifier("name".to_string()));
                            assert_eq!(*operator, BinaryOp::Equal);
                            assert_eq!(
                                **right,
                                Expr::Literal(LiteralValue::String("John".to_string()))
                            );
                        } else {
                            panic!("Expected binary expression");
                        }
                    } else {
                        panic!("Expected Filter operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_mutate_single_assignment() {
                let lexer = Lexer::new("mutate(adult = age >= 18)".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    assert_eq!(operations.len(), 1);
                    if let DplyrOperation::Mutate { assignments, .. } = &operations[0] {
                        assert_eq!(assignments.len(), 1);
                        assert_eq!(assignments[0].column, "adult");

                        if let Expr::Binary {
                            left,
                            operator,
                            right,
                        } = &assignments[0].expr
                        {
                            assert_eq!(**left, Expr::Identifier("age".to_string()));
                            assert_eq!(*operator, BinaryOp::GreaterThanOrEqual);
                            assert_eq!(**right, Expr::Literal(LiteralValue::Number(18.0)));
                        } else {
                            panic!("Expected binary expression");
                        }
                    } else {
                        panic!("Expected Mutate operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_mutate_multiple_assignments() {
                let lexer =
                    Lexer::new("mutate(adult = age >= 18, salary_k = salary / 1000)".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    assert_eq!(operations.len(), 1);
                    if let DplyrOperation::Mutate { assignments, .. } = &operations[0] {
                        assert_eq!(assignments.len(), 2);

                        // First assignment: adult = age >= 18
                        assert_eq!(assignments[0].column, "adult");
                        if let Expr::Binary { operator, .. } = &assignments[0].expr {
                            assert_eq!(*operator, BinaryOp::GreaterThanOrEqual);
                        } else {
                            panic!("Expected binary expression");
                        }

                        // Second assignment: salary_k = salary / 1000
                        assert_eq!(assignments[1].column, "salary_k");
                        if let Expr::Binary {
                            left,
                            operator,
                            right,
                        } = &assignments[1].expr
                        {
                            assert_eq!(**left, Expr::Identifier("salary".to_string()));
                            assert_eq!(*operator, BinaryOp::Divide);
                            assert_eq!(**right, Expr::Literal(LiteralValue::Number(1000.0)));
                        } else {
                            panic!("Expected binary expression");
                        }
                    } else {
                        panic!("Expected Mutate operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_arrange_single_column() {
                let lexer = Lexer::new("arrange(name)".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    assert_eq!(operations.len(), 1);
                    if let DplyrOperation::Arrange { columns, .. } = &operations[0] {
                        assert_eq!(columns.len(), 1);
                        assert_eq!(columns[0].column, "name");
                        assert_eq!(columns[0].direction, OrderDirection::Asc);
                    } else {
                        panic!("Expected Arrange operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_arrange_desc() {
                let lexer = Lexer::new("arrange(desc(age))".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    assert_eq!(operations.len(), 1);
                    if let DplyrOperation::Arrange { columns, .. } = &operations[0] {
                        assert_eq!(columns.len(), 1);
                        assert_eq!(columns[0].column, "age");
                        assert_eq!(columns[0].direction, OrderDirection::Desc);
                    } else {
                        panic!("Expected Arrange operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_arrange_multiple_columns() {
                let lexer = Lexer::new("arrange(department, desc(salary), name)".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    assert_eq!(operations.len(), 1);
                    if let DplyrOperation::Arrange { columns, .. } = &operations[0] {
                        assert_eq!(columns.len(), 3);

                        assert_eq!(columns[0].column, "department");
                        assert_eq!(columns[0].direction, OrderDirection::Asc);

                        assert_eq!(columns[1].column, "salary");
                        assert_eq!(columns[1].direction, OrderDirection::Desc);

                        assert_eq!(columns[2].column, "name");
                        assert_eq!(columns[2].direction, OrderDirection::Asc);
                    } else {
                        panic!("Expected Arrange operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_group_by_single_column() {
                let lexer = Lexer::new("group_by(department)".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    assert_eq!(operations.len(), 1);
                    if let DplyrOperation::GroupBy { columns, .. } = &operations[0] {
                        assert_eq!(columns.len(), 1);
                        assert_eq!(columns[0], "department");
                    } else {
                        panic!("Expected GroupBy operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_group_by_multiple_columns() {
                let lexer = Lexer::new("group_by(department, location, team)".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    assert_eq!(operations.len(), 1);
                    if let DplyrOperation::GroupBy { columns, .. } = &operations[0] {
                        assert_eq!(columns.len(), 3);
                        assert_eq!(columns[0], "department");
                        assert_eq!(columns[1], "location");
                        assert_eq!(columns[2], "team");
                    } else {
                        panic!("Expected GroupBy operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_summarise_single_aggregation() {
                let lexer = Lexer::new("summarise(avg_salary = mean(salary))".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    assert_eq!(operations.len(), 1);
                    if let DplyrOperation::Summarise { aggregations, .. } = &operations[0] {
                        assert_eq!(aggregations.len(), 1);
                        assert_eq!(aggregations[0].function, "mean");
                        assert_eq!(aggregations[0].column, "salary");
                        assert_eq!(aggregations[0].alias, Some("avg_salary".to_string()));
                    } else {
                        panic!("Expected Summarise operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_summarise_multiple_aggregations() {
                let lexer = Lexer::new(
                    "summarise(avg_salary = mean(salary), total_count = n(), max_age = max(age))"
                        .to_string(),
                );
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    assert_eq!(operations.len(), 1);
                    if let DplyrOperation::Summarise { aggregations, .. } = &operations[0] {
                        assert_eq!(aggregations.len(), 3);

                        // First aggregation: avg_salary = mean(salary)
                        assert_eq!(aggregations[0].function, "mean");
                        assert_eq!(aggregations[0].column, "salary");
                        assert_eq!(aggregations[0].alias, Some("avg_salary".to_string()));

                        // Second aggregation: total_count = n()
                        assert_eq!(aggregations[1].function, "n");
                        assert_eq!(aggregations[1].column, "");
                        assert_eq!(aggregations[1].alias, Some("total_count".to_string()));

                        // Third aggregation: max_age = max(age)
                        assert_eq!(aggregations[2].function, "max");
                        assert_eq!(aggregations[2].column, "age");
                        assert_eq!(aggregations[2].alias, Some("max_age".to_string()));
                    } else {
                        panic!("Expected Summarise operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_summarise_without_alias() {
                let lexer = Lexer::new("summarise(mean(salary), n())".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    assert_eq!(operations.len(), 1);
                    if let DplyrOperation::Summarise { aggregations, .. } = &operations[0] {
                        assert_eq!(aggregations.len(), 2);

                        // First aggregation: mean(salary)
                        assert_eq!(aggregations[0].function, "mean");
                        assert_eq!(aggregations[0].column, "salary");
                        assert_eq!(aggregations[0].alias, None);

                        // Second aggregation: n()
                        assert_eq!(aggregations[1].function, "n");
                        assert_eq!(aggregations[1].column, "");
                        assert_eq!(aggregations[1].alias, None);
                    } else {
                        panic!("Expected Summarise operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }
        }

        /// Tests for complex pipeline parsing
        mod complex_pipeline_tests {
            use super::*;

            #[test]
            fn test_full_data_analysis_pipeline() {
                let input = "data %>% select(name, age, salary, department) %>% filter(age >= 18 & salary > 30000) %>% mutate(adult = TRUE, salary_k = salary / 1000, age_group = age / 10) %>% group_by(department, age_group) %>% summarise(avg_salary = mean(salary_k), count = n(), max_age = max(age)) %>% arrange(desc(avg_salary), department)";

                let lexer = Lexer::new(input.to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    assert_eq!(operations.len(), 6);

                    // Verify operation sequence
                    assert!(matches!(operations[0], DplyrOperation::Select { .. }));
                    assert!(matches!(operations[1], DplyrOperation::Filter { .. }));
                    assert!(matches!(operations[2], DplyrOperation::Mutate { .. }));
                    assert!(matches!(operations[3], DplyrOperation::GroupBy { .. }));
                    assert!(matches!(operations[4], DplyrOperation::Summarise { .. }));
                    assert!(matches!(operations[5], DplyrOperation::Arrange { .. }));

                    // Verify select operation
                    if let DplyrOperation::Select { columns, .. } = &operations[0] {
                        assert_eq!(columns.len(), 4);
                        assert_eq!(columns[0].expr, Expr::Identifier("name".to_string()));
                        assert_eq!(columns[1].expr, Expr::Identifier("age".to_string()));
                        assert_eq!(columns[2].expr, Expr::Identifier("salary".to_string()));
                        assert_eq!(columns[3].expr, Expr::Identifier("department".to_string()));
                    }

                    // Verify mutate operation with multiple assignments
                    if let DplyrOperation::Mutate { assignments, .. } = &operations[2] {
                        assert_eq!(assignments.len(), 3);
                        assert_eq!(assignments[0].column, "adult");
                        assert_eq!(assignments[1].column, "salary_k");
                        assert_eq!(assignments[2].column, "age_group");
                    }

                    // Verify group_by operation
                    if let DplyrOperation::GroupBy { columns, .. } = &operations[3] {
                        assert_eq!(columns.len(), 2);
                        assert_eq!(columns[0], "department");
                        assert_eq!(columns[1], "age_group");
                    }

                    // Verify summarise operation
                    if let DplyrOperation::Summarise { aggregations, .. } = &operations[4] {
                        assert_eq!(aggregations.len(), 3);
                        assert_eq!(aggregations[0].alias, Some("avg_salary".to_string()));
                        assert_eq!(aggregations[1].alias, Some("count".to_string()));
                        assert_eq!(aggregations[2].alias, Some("max_age".to_string()));
                    }

                    // Verify arrange operation
                    if let DplyrOperation::Arrange { columns, .. } = &operations[5] {
                        assert_eq!(columns.len(), 2);
                        assert_eq!(columns[0].direction, OrderDirection::Desc);
                        assert_eq!(columns[1].direction, OrderDirection::Asc);
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_nested_function_calls_in_pipeline() {
                let input = "select(upper(trim(name)), round(sqrt(salary), 2)) %>% filter(length(name) > 3)";
                let lexer = Lexer::new(input.to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    assert_eq!(operations.len(), 2);

                    // Verify select with nested function calls
                    if let DplyrOperation::Select { columns, .. } = &operations[0] {
                        assert_eq!(columns.len(), 2);

                        // First column: upper(trim(name))
                        if let Expr::Function { name, args } = &columns[0].expr {
                            assert_eq!(name, "upper");
                            assert_eq!(args.len(), 1);
                            if let Expr::Function {
                                name: inner_name,
                                args: inner_args,
                            } = &args[0]
                            {
                                assert_eq!(inner_name, "trim");
                                assert_eq!(inner_args.len(), 1);
                                assert_eq!(inner_args[0], Expr::Identifier("name".to_string()));
                            } else {
                                panic!("Expected nested function call");
                            }
                        } else {
                            panic!("Expected function call");
                        }

                        // Second column: round(sqrt(salary), 2)
                        if let Expr::Function { name, args } = &columns[1].expr {
                            assert_eq!(name, "round");
                            assert_eq!(args.len(), 2);
                            if let Expr::Function {
                                name: inner_name, ..
                            } = &args[0]
                            {
                                assert_eq!(inner_name, "sqrt");
                            } else {
                                panic!("Expected nested function call");
                            }
                            assert_eq!(args[1], Expr::Literal(LiteralValue::Number(2.0)));
                        } else {
                            panic!("Expected function call");
                        }
                    }

                    // Verify filter with function call
                    if let DplyrOperation::Filter { condition, .. } = &operations[1] {
                        if let Expr::Binary {
                            left,
                            operator,
                            right,
                        } = condition
                        {
                            if let Expr::Function { name, args } = left.as_ref() {
                                assert_eq!(name, "length");
                                assert_eq!(args.len(), 1);
                                assert_eq!(args[0], Expr::Identifier("name".to_string()));
                            } else {
                                panic!("Expected function call in filter");
                            }
                            assert_eq!(*operator, BinaryOp::GreaterThan);
                            assert_eq!(**right, Expr::Literal(LiteralValue::Number(3.0)));
                        } else {
                            panic!("Expected binary expression in filter");
                        }
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_complex_arithmetic_expressions() {
                let input =
                    "mutate(score = (math + science) * 0.5 + english * 0.3, grade = score / 10)";
                let lexer = Lexer::new(input.to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    assert_eq!(operations.len(), 1);

                    if let DplyrOperation::Mutate { assignments, .. } = &operations[0] {
                        assert_eq!(assignments.len(), 2);

                        // First assignment: score = (math + science) * 0.5 + english * 0.3
                        assert_eq!(assignments[0].column, "score");
                        if let Expr::Binary { operator, .. } = &assignments[0].expr {
                            assert_eq!(*operator, BinaryOp::Plus);
                        } else {
                            panic!("Expected complex arithmetic expression");
                        }

                        // Second assignment: grade = score / 10
                        assert_eq!(assignments[1].column, "grade");
                        if let Expr::Binary {
                            left,
                            operator,
                            right,
                        } = &assignments[1].expr
                        {
                            assert_eq!(**left, Expr::Identifier("score".to_string()));
                            assert_eq!(*operator, BinaryOp::Divide);
                            assert_eq!(**right, Expr::Literal(LiteralValue::Number(10.0)));
                        } else {
                            panic!("Expected binary expression");
                        }
                    } else {
                        panic!("Expected Mutate operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_pipeline_with_mixed_data_types() {
                let input = r#"filter(active == TRUE & score >= 85.5 & name != "test")"#;
                let lexer = Lexer::new(input.to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    assert_eq!(operations.len(), 1);

                    if let DplyrOperation::Filter { condition, .. } = &operations[0] {
                        // The expression should be parsed as a complex AND expression
                        // We'll just verify it's a binary expression with AND operator
                        if let Expr::Binary { operator, .. } = condition {
                            assert_eq!(*operator, BinaryOp::And);
                        } else {
                            panic!("Expected binary expression");
                        }
                    } else {
                        panic!("Expected Filter operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }
        }

        /// Tests for parsing error cases
        mod error_case_tests {
            use super::*;

            #[test]
            fn test_invalid_function_name() {
                let lexer = Lexer::new("invalid_function(name)".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                match parser.parse() {
                    Err(ParseError::UnexpectedToken {
                        expected, found, ..
                    }) => {
                        assert!(expected.contains("dplyr function"));
                        assert!(found.contains("invalid_function"));
                    }
                    other => panic!("Expected UnexpectedToken error, got: {other:?}"),
                }
            }

            #[test]
            fn test_missing_parentheses() {
                let lexer = Lexer::new("select name, age".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                match parser.parse() {
                    Err(ParseError::UnexpectedToken {
                        expected, found, ..
                    }) => {
                        assert!(expected.contains("("));
                        assert_eq!(found, "name");
                    }
                    other => panic!("Expected UnexpectedToken error, got: {other:?}"),
                }
            }

            #[test]
            fn test_unclosed_parentheses() {
                let lexer = Lexer::new("select(name, age".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                match parser.parse() {
                    Err(ParseError::UnexpectedToken {
                        expected, found, ..
                    }) => {
                        assert!(expected.contains(")"));
                        assert_eq!(found, "EOF");
                    }
                    other => panic!("Expected UnexpectedToken error, got: {other:?}"),
                }
            }

            #[test]
            fn test_missing_comma_in_select() {
                let lexer = Lexer::new("select(name age)".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                match parser.parse() {
                    Err(ParseError::UnexpectedToken {
                        expected, found, ..
                    }) => {
                        assert!(expected.contains(",") || expected.contains(")"));
                        assert_eq!(found, "age");
                    }
                    other => panic!("Expected UnexpectedToken error, got: {other:?}"),
                }
            }

            #[test]
            fn test_invalid_assignment_in_mutate() {
                let lexer = Lexer::new("mutate(new_col age + 1)".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                match parser.parse() {
                    Err(ParseError::UnexpectedToken {
                        expected, found, ..
                    }) => {
                        assert!(expected.contains("="));
                        assert_eq!(found, "age");
                    }
                    other => panic!("Expected UnexpectedToken error, got: {other:?}"),
                }
            }

            #[test]
            fn test_empty_filter_condition() {
                let lexer = Lexer::new("filter()".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                match parser.parse() {
                    Err(ParseError::UnexpectedToken {
                        expected, found, ..
                    }) => {
                        assert!(expected.contains("expression") || expected.contains("identifier"));
                        assert_eq!(found, ")");
                    }
                    other => panic!("Expected UnexpectedToken error, got: {other:?}"),
                }
            }

            #[test]
            fn test_invalid_pipe_operator() {
                // Test with an invalid token that should cause a lexer error
                let lexer = Lexer::new("select(name) @ filter(age > 18)".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                match parser.parse() {
                    Err(_) => {
                        // Should fail because @ is not a valid token
                    }
                    other => panic!("Expected error, got: {other:?}"),
                }
            }

            #[test]
            fn test_trailing_comma() {
                let lexer = Lexer::new("select(name, age,)".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                match parser.parse() {
                    Err(ParseError::UnexpectedToken {
                        expected, found, ..
                    }) => {
                        assert!(expected.contains("expression") || expected.contains("identifier"));
                        assert_eq!(found, ")");
                    }
                    other => panic!("Expected UnexpectedToken error, got: {other:?}"),
                }
            }

            #[test]
            fn test_invalid_desc_usage() {
                let lexer = Lexer::new("arrange(desc())".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                match parser.parse() {
                    Err(ParseError::UnexpectedToken {
                        expected, found, ..
                    }) => {
                        assert!(expected.contains("column identifier"));
                        assert_eq!(found, ")");
                    }
                    other => panic!("Expected UnexpectedToken error, got: {other:?}"),
                }
            }

            #[test]
            fn test_invalid_aggregation_function() {
                let lexer = Lexer::new("summarise(result = invalid_agg())".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                // This should parse successfully but the function name will be "invalid_agg"
                // The validation of function names should happen at a later stage
                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    if let DplyrOperation::Summarise { aggregations, .. } = &operations[0] {
                        assert_eq!(aggregations[0].function, "invalid_agg");
                    }
                }
            }

            #[test]
            fn test_nested_pipe_operators() {
                let lexer = Lexer::new("select(name) %>% %>% filter(age > 18)".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                match parser.parse() {
                    Err(ParseError::UnexpectedToken {
                        expected, found, ..
                    }) => {
                        assert!(expected.contains("dplyr function"));
                        assert!(found.contains("%>%"));
                    }
                    other => panic!("Expected UnexpectedToken error, got: {other:?}"),
                }
            }
        }

        /// Tests for AST structure verification
        mod ast_structure_tests {
            use super::*;

            #[test]
            fn test_ast_node_location_tracking() {
                let lexer = Lexer::new("select(name) %>% filter(age > 18)".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                // Verify that location information is tracked
                let location = ast.location();
                assert_ne!(location.line, 0);
                assert_ne!(location.column, 0);

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    for operation in operations {
                        let op_location = operation.location();
                        assert_ne!(op_location.line, 0);
                        assert_ne!(op_location.column, 0);
                    }
                }
            }

            #[test]
            fn test_ast_node_type_checking() {
                let lexer = Lexer::new("data %>% select(name)".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                assert!(ast.is_pipeline());
                assert!(!ast.is_data_source());

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    assert_eq!(operations.len(), 1);
                    assert_eq!(operations[0].operation_name(), "select");
                }
            }

            #[test]
            fn test_data_source_node_structure() {
                let lexer = Lexer::new("my_data".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                assert!(!ast.is_pipeline());
                assert!(ast.is_data_source());

                if let DplyrNode::DataSource { name, location } = ast {
                    assert_eq!(name, "my_data");
                    assert_ne!(location.line, 0);
                } else {
                    panic!("Expected DataSource node");
                }
            }

            #[test]
            fn test_expression_tree_structure() {
                let lexer = Lexer::new(
                    "filter(age >= 18 & (status == \"active\" | priority > 5))".to_string(),
                );
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    if let DplyrOperation::Filter { condition, .. } = &operations[0] {
                        // Verify the complex expression tree structure
                        if let Expr::Binary {
                            left,
                            operator,
                            right,
                        } = condition
                        {
                            assert_eq!(*operator, BinaryOp::And);

                            // Left side: age >= 18
                            if let Expr::Binary {
                                left: l_left,
                                operator: l_op,
                                right: l_right,
                            } = left.as_ref()
                            {
                                assert_eq!(**l_left, Expr::Identifier("age".to_string()));
                                assert_eq!(*l_op, BinaryOp::GreaterThanOrEqual);
                                assert_eq!(**l_right, Expr::Literal(LiteralValue::Number(18.0)));
                            } else {
                                panic!("Expected binary expression on left");
                            }

                            // Right side: (status == "active" | priority > 5)
                            if let Expr::Binary { operator: r_op, .. } = right.as_ref() {
                                assert_eq!(*r_op, BinaryOp::Or);
                            } else {
                                panic!("Expected binary expression on right");
                            }
                        } else {
                            panic!("Expected binary expression");
                        }
                    } else {
                        panic!("Expected Filter operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_column_expression_structure() {
                let lexer = Lexer::new(
                    "select(full_name = concat(first_name, last_name), age)".to_string(),
                );
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    if let DplyrOperation::Select { columns, .. } = &operations[0] {
                        assert_eq!(columns.len(), 2);

                        // First column: full_name = concat(first_name, last_name)
                        assert_eq!(columns[0].alias, Some("full_name".to_string()));
                        if let Expr::Function { name, args } = &columns[0].expr {
                            assert_eq!(name, "concat");
                            assert_eq!(args.len(), 2);
                            assert_eq!(args[0], Expr::Identifier("first_name".to_string()));
                            assert_eq!(args[1], Expr::Identifier("last_name".to_string()));
                        } else {
                            panic!("Expected function call expression");
                        }

                        // Second column: age
                        assert_eq!(columns[1].alias, None);
                        assert_eq!(columns[1].expr, Expr::Identifier("age".to_string()));
                    } else {
                        panic!("Expected Select operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_assignment_structure() {
                let lexer = Lexer::new(
                    "mutate(full_name = concat(first, last), age_months = age * 12)".to_string(),
                );
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    if let DplyrOperation::Mutate { assignments, .. } = &operations[0] {
                        assert_eq!(assignments.len(), 2);

                        // First assignment: full_name = concat(first, last)
                        assert_eq!(assignments[0].column, "full_name");
                        if let Expr::Function { name, args } = &assignments[0].expr {
                            assert_eq!(name, "concat");
                            assert_eq!(args.len(), 2);
                        } else {
                            panic!("Expected function call");
                        }

                        // Second assignment: age_months = age * 12
                        assert_eq!(assignments[1].column, "age_months");
                        if let Expr::Binary {
                            left,
                            operator,
                            right,
                        } = &assignments[1].expr
                        {
                            assert_eq!(**left, Expr::Identifier("age".to_string()));
                            assert_eq!(*operator, BinaryOp::Multiply);
                            assert_eq!(**right, Expr::Literal(LiteralValue::Number(12.0)));
                        } else {
                            panic!("Expected binary expression");
                        }
                    } else {
                        panic!("Expected Mutate operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_aggregation_structure() {
                let lexer = Lexer::new(
                    "summarise(avg_score = mean(score), total = sum(points), count = n())"
                        .to_string(),
                );
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    if let DplyrOperation::Summarise { aggregations, .. } = &operations[0] {
                        assert_eq!(aggregations.len(), 3);

                        // First aggregation: avg_score = mean(score)
                        assert_eq!(aggregations[0].function, "mean");
                        assert_eq!(aggregations[0].column, "score");
                        assert_eq!(aggregations[0].alias, Some("avg_score".to_string()));

                        // Second aggregation: total = sum(points)
                        assert_eq!(aggregations[1].function, "sum");
                        assert_eq!(aggregations[1].column, "points");
                        assert_eq!(aggregations[1].alias, Some("total".to_string()));

                        // Third aggregation: count = n()
                        assert_eq!(aggregations[2].function, "n");
                        assert_eq!(aggregations[2].column, ""); // n() has no column
                        assert_eq!(aggregations[2].alias, Some("count".to_string()));
                    } else {
                        panic!("Expected Summarise operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }

            #[test]
            fn test_order_expression_structure() {
                let lexer = Lexer::new("arrange(name, desc(age), salary)".to_string());
                let mut parser = Parser::new(lexer).unwrap();

                let ast = parser.parse().unwrap();

                if let DplyrNode::Pipeline { operations, .. } = ast {
                    if let DplyrOperation::Arrange { columns, .. } = &operations[0] {
                        assert_eq!(columns.len(), 3);

                        // First column: name (ascending by default)
                        assert_eq!(columns[0].column, "name");
                        assert_eq!(columns[0].direction, OrderDirection::Asc);

                        // Second column: desc(age)
                        assert_eq!(columns[1].column, "age");
                        assert_eq!(columns[1].direction, OrderDirection::Desc);

                        // Third column: salary (ascending by default)
                        assert_eq!(columns[2].column, "salary");
                        assert_eq!(columns[2].direction, OrderDirection::Asc);
                    } else {
                        panic!("Expected Arrange operation");
                    }
                } else {
                    panic!("Expected Pipeline node");
                }
            }
        }
    }
}
