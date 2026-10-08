use libdplyr::error::ParseError;
use libdplyr::lexer::Lexer;
use libdplyr::parser::{DplyrNode, DplyrOperation, Parser, MAX_EXPRESSION_DEPTH};
use libdplyr::PipeSyntax;

fn parse(code: &str) -> Result<DplyrNode, ParseError> {
    Parser::new(Lexer::new(code.to_string()))?.parse()
}

fn expression_label(code: &str) -> String {
    let ast = parse(&format!("data %>% mutate(result = {code})")).expect("expression parses");
    let DplyrNode::Pipeline { operations, .. } = ast else {
        panic!("expected pipeline");
    };
    let DplyrOperation::Mutate { assignments, .. } = &operations[0] else {
        panic!("expected mutate");
    };
    assignments[0].expr.to_string()
}

// R2-AC1: Preserve R precedence, including right-associative exponentiation.
#[test]
fn unary_power_and_membership_precedence() {
    for (code, label) in [
        ("-2^2", "(-(2 ^ 2))"),
        ("(-2)^2", "((-2) ^ 2)"),
        ("2^3^2", "(2 ^ (3 ^ 2))"),
        ("2^-2", "(2 ^ (-2))"),
        ("+x * -y", "((+x) * (-y))"),
        ("!x == 2", "(!(x == 2))"),
        ("!x %in% c(1, 2) & TRUE", "((!(x %in% c(1, 2))) && TRUE)"),
        ("x * y %in% c(1)", "(x * (y %in% c(1)))"),
    ] {
        assert_eq!(expression_label(code), label, "{code}");
    }
}

#[test]
fn membership_distinguishes_missing_values_and_empty_vectors() {
    for (code, label) in [
        ("x %in% c()", "(x %in% c())"),
        ("x %in% NULL", "(x %in% c())"),
        ("x %in% c(NULL)", "(x %in% c())"),
        ("x %in% c(NA, NULL, -2, +3)", "(x %in% c(NULL, -2, 3))"),
        ("x %in% NA", "(x %in% c(NULL))"),
        ("x %in% 2", "(x %in% c(2))"),
    ] {
        assert_eq!(expression_label(code), label);
    }
}

#[test]
fn membership_rejects_vectors_that_need_r_evaluation() {
    for rhs in ["other", "c(x)", "c(1 + 2)", "c(value = 1)", "c(1,)"] {
        assert!(
            parse(&format!("data %>% filter(x %in% {rhs})")).is_err(),
            "{rhs}"
        );
    }
}

#[test]
fn ungroup_supports_partial_groups_and_both_pipe_modes() {
    for (pipe, mode) in [("%>%", PipeSyntax::Magrittr), ("|>", PipeSyntax::Native)] {
        let code = format!("data {pipe} group_by(grp) {pipe} ungroup() {pipe} mutate(grp = 1)");
        let ast = Parser::new(Lexer::with_pipe_syntax(code, mode))
            .expect("lexer")
            .parse()
            .expect("ungroup parses");
        let DplyrNode::Pipeline { operations, .. } = ast else {
            panic!("pipeline");
        };
        assert_eq!(operations[1].operation_name(), "ungroup");
    }
    assert!(parse("data %>% ungroup(grp)").is_ok());
}

// R2-AC2: New recursive grammar retains the shared depth limit.
#[test]
fn deep_unary_and_power_expressions_fail_without_overflow() {
    for expr in [
        format!("{}x", "!".repeat(MAX_EXPRESSION_DEPTH * 4)),
        format!("{}x", "-".repeat(MAX_EXPRESSION_DEPTH * 4)),
        vec!["2"; MAX_EXPRESSION_DEPTH * 4].join(" ^ "),
    ] {
        assert!(matches!(
            parse(&format!("data %>% mutate(result = {expr})")),
            Err(ParseError::InvalidExpression { .. })
        ));
    }
}

#[test]
fn membership_is_independent_of_the_pipe_mode() {
    let mut lexer = Lexer::with_pipe_syntax("x %in% c(1)".to_string(), PipeSyntax::Native);
    assert_eq!(
        lexer.next_token().expect("identifier"),
        libdplyr::lexer::Token::Identifier("x".to_string())
    );
    assert_eq!(
        lexer
            .next_token()
            .expect("membership is not a magrittr pipe"),
        libdplyr::lexer::Token::In
    );
}

#[test]
fn unary_minus_parses_as_tidy_selection() {
    for selection in ["-x", "(-x)", "-1"] {
        parse(&format!("data %>% select({selection})")).expect("selection parses");
    }
    parse("data %>% select(negative = -x)").expect("legacy explicit computed alias");
}
