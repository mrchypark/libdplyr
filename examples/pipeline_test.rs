//! Pipeline test examples
//!
//! This example demonstrates complex dplyr pipeline operations and showcases
//! the power of chaining multiple operations together.

use libdplyr::{Transpiler, PostgreSqlDialect, MySqlDialect, SqliteDialect, DuckDbDialect};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== libdplyr Pipeline Examples ===\n");

    // Test different types of pipelines
    test_data_analysis_pipeline()?;
    test_business_intelligence_pipeline()?;
    test_data_cleaning_pipeline()?;
    test_reporting_pipeline()?;

    println!("✅ All pipeline examples completed successfully!");
    Ok(())
}

/// Demonstrates a typical data analysis pipeline
fn test_data_analysis_pipeline() -> Result<(), Box<dyn std::error::Error>> {
    println!("📊 Data Analysis Pipeline");
    println!("========================");

    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));

    let analysis_pipeline = "select(customer_id, product_category, purchase_amount) %>% filter(purchase_amount > 0) %>% mutate(amount_category = purchase_amount * 1.1) %>% group_by(product_category) %>% summarise(total_purchases = n(), total_revenue = sum(purchase_amount), avg_purchase = mean(purchase_amount)) %>% arrange(desc(total_revenue))";

    println!("Input dplyr code:");
    println!("{}", analysis_pipeline.trim());
    println!("\nGenerated SQL:");
    let sql = transpiler.transpile(analysis_pipeline)?;
    println!("{}\n", sql);

    Ok(())
}

/// Demonstrates a business intelligence pipeline
fn test_business_intelligence_pipeline() -> Result<(), Box<dyn std::error::Error>> {
    println!("💼 Business Intelligence Pipeline");
    println!("================================");

    let transpiler = Transpiler::new(Box::new(MySqlDialect::new()));

    let bi_pipeline = "select(employee_id, name, department, salary, performance_score) %>% filter(performance_score >= 3.0) %>% mutate(performance_bonus = salary * 0.1) %>% group_by(department) %>% summarise(employee_count = n(), avg_salary = mean(salary), total_bonus_budget = sum(performance_bonus)) %>% arrange(desc(avg_salary))";

    println!("Input dplyr code:");
    println!("{}", bi_pipeline.trim());
    println!("\nGenerated SQL (MySQL):");
    let sql = transpiler.transpile(bi_pipeline)?;
    println!("{}\n", sql);

    Ok(())
}

/// Demonstrates a data cleaning pipeline
fn test_data_cleaning_pipeline() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧹 Data Cleaning Pipeline");
    println!("=========================");

    let transpiler = Transpiler::new(Box::new(SqliteDialect::new()));

    let cleaning_pipeline = "select(id, name, email, phone) %>% filter(name != \"\" & email != \"\") %>% mutate(clean_name = upper(name), clean_email = lower(email)) %>% arrange(clean_name)";

    println!("Input dplyr code:");
    println!("{}", cleaning_pipeline.trim());
    println!("\nGenerated SQL (SQLite):");
    let sql = transpiler.transpile(cleaning_pipeline)?;
    println!("{}\n", sql);

    Ok(())
}

/// Demonstrates a reporting pipeline with window functions
fn test_reporting_pipeline() -> Result<(), Box<dyn std::error::Error>> {
    println!("📈 Reporting Pipeline");
    println!("====================");

    let transpiler = Transpiler::new(Box::new(DuckDbDialect::new()));

    let reporting_pipeline = "select(sales_rep_id, region, quarter, sales_amount) %>% filter(sales_amount > 0) %>% mutate(achievement_rate = sales_amount / 1000) %>% group_by(region, quarter) %>% summarise(rep_count = n(), total_sales = sum(sales_amount), median_sales = median(sales_amount)) %>% arrange(desc(total_sales))";

    println!("Input dplyr code:");
    println!("{}", reporting_pipeline.trim());
    println!("\nGenerated SQL (DuckDB):");
    let sql = transpiler.transpile(reporting_pipeline)?;
    println!("{}\n", sql);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complex_pipeline_parsing() {
        let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
        
        let complex_pipeline = r#"
            select(id, name, category, price) %>%
            filter(price > 10 & category == "electronics") %>%
            mutate(discounted_price = price * 0.9) %>%
            arrange(desc(discounted_price)) %>%
            group_by(category) %>%
            summarise(avg_price = mean(discounted_price))
        "#;

        let result = transpiler.transpile(complex_pipeline);
        assert!(result.is_ok(), "Complex pipeline should parse successfully: {:?}", result);
        
        let sql = result.unwrap();
        assert!(sql.contains("SELECT"), "Should contain SELECT");
        assert!(sql.contains("WHERE"), "Should contain WHERE");
        assert!(sql.contains("GROUP BY"), "Should contain GROUP BY");
        assert!(sql.contains("AVG"), "Should contain AVG function");
    }

    #[test]
    fn test_nested_conditions() {
        let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
        
        let nested_pipeline = r#"
            select(name, age, salary, department) %>%
            filter(
                (age >= 25 & age <= 65) &
                (salary > 50000 | department == "Executive") &
                department != "Temp"
            )
        "#;

        let result = transpiler.transpile(nested_pipeline);
        assert!(result.is_ok(), "Nested conditions should parse: {:?}", result);
        
        let sql = result.unwrap();
        assert!(sql.contains("WHERE"), "Should generate WHERE clause");
        assert!(sql.contains("AND"), "Should contain AND operators");
        assert!(sql.contains("OR"), "Should contain OR operators");
    }

    #[test]
    fn test_multiple_mutations() {
        let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
        
        let mutation_pipeline = r#"
            select(first_name, last_name, birth_date, salary) %>%
            mutate(
                full_name = first_name || " " || last_name,
                age = 2024 - year(birth_date),
                annual_salary = salary * 12,
                salary_category = case
                    when salary < 5000 then "Low"
                    when salary < 10000 then "Medium"
                    else "High"
                end
            )
        "#;

        let result = transpiler.transpile(mutation_pipeline);
        assert!(result.is_ok(), "Multiple mutations should work: {:?}", result);
        
        let sql = result.unwrap();
        assert!(sql.contains("SELECT"), "Should contain SELECT");
        assert!(sql.contains("||"), "Should contain string concatenation");
        assert!(sql.contains("CASE"), "Should contain CASE statement");
    }

    #[test]
    fn test_aggregation_with_grouping() {
        let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
        
        let agg_pipeline = r#"
            group_by(department, location) %>%
            summarise(
                employee_count = n(),
                avg_salary = mean(salary),
                min_salary = min(salary),
                max_salary = max(salary),
                total_budget = sum(salary)
            )
        "#;

        let result = transpiler.transpile(agg_pipeline);
        assert!(result.is_ok(), "Aggregation with grouping should work: {:?}", result);
        
        let sql = result.unwrap();
        assert!(sql.contains("GROUP BY"), "Should contain GROUP BY");
        assert!(sql.contains("COUNT(*)"), "Should convert n() to COUNT(*)");
        assert!(sql.contains("AVG"), "Should contain AVG function");
        assert!(sql.contains("MIN"), "Should contain MIN function");
        assert!(sql.contains("MAX"), "Should contain MAX function");
        assert!(sql.contains("SUM"), "Should contain SUM function");
    }

    #[test]
    fn test_ordering_with_multiple_columns() {
        let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
        
        let order_pipeline = r#"
            select(name, department, salary, hire_date) %>%
            arrange(department, desc(salary), hire_date)
        "#;

        let result = transpiler.transpile(order_pipeline);
        assert!(result.is_ok(), "Multiple column ordering should work: {:?}", result);
        
        let sql = result.unwrap();
        assert!(sql.contains("ORDER BY"), "Should contain ORDER BY");
        assert!(sql.contains("DESC"), "Should contain DESC for salary");
    }
}