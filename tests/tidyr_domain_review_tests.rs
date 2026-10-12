use libdplyr::{SourceSchema, SqliteDialect, Transpiler};

fn transpile(code: &str) -> Result<libdplyr::CompiledQuery, libdplyr::TranspileError> {
    Transpiler::new(Box::new(SqliteDialect::new()))
        .transpile_with_schema(code, &SourceSchema::new("data", ["g", "x", "y"]))
}

#[test]
fn named_domains_cannot_target_grouping_columns() {
    for code in [
        "data %>% group_by(g) %>% expand(g = c('a', 'b'))",
        "data %>% group_by(g) %>% complete(g = c('a', 'b'))",
    ] {
        assert!(transpile(code).is_err(), "{code}");
    }
}

#[test]
fn expand_rejects_overlapping_domain_columns() {
    for code in ["data %>% expand(x, x)", "data %>% expand(x, nesting(x, y))"] {
        assert!(transpile(code).is_err(), "{code}");
    }
}

#[test]
fn nonoverlapping_and_named_domains_still_compile() {
    transpile("data %>% complete(x = c(1, 2), y)").expect("named complete domain");
    transpile("data %>% group_by(g) %>% expand(x, y)").expect("grouped expansion");
}

#[test]
fn replace_na_accepts_one_replacement_list_only() {
    transpile("data %>% replace_na(replace = list(x = 0))").expect("named replacement list");
    for code in [
        "data %>% replace_na(list(x = 0), list(y = 1))",
        "data %>% replace_na(data = list(x = 0))",
        "data %>% replace_na(unknown = list(x = 0))",
    ] {
        assert!(transpile(code).is_err(), "{code}");
    }
}

#[test]
fn replace_na_keeps_tidyr_list_name_behavior() {
    let query = transpile("data %>% replace_na(list(unknown = 1, x = 0, x = 9))")
        .expect("unknown and duplicate replacement names are ignored or use the first value");
    assert!(query.sql.contains("COALESCE(\"x\", 0)"), "{}", query.sql);
    assert!(!query.sql.contains("COALESCE(\"x\", 9)"), "{}", query.sql);
}
