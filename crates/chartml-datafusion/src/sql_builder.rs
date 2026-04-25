//! SQL builder for aggregate specifications.
//!
//! Ports the logic from Kyomi's `transformSQLBuilder.js` to Rust.
//! Generates DataFusion-compatible SQL from `AggregateSpec`.

use chartml_core::spec::{
    AggregateSpec, Dimension, FilterGroup, FilterRule, Measure,
};
use std::collections::HashMap;

/// A symbol in the symbol table — tracks the SQL expression and whether it's aggregated.
#[derive(Debug, Clone)]
struct Symbol {
    sql: String,
    is_aggregated: bool,
}

/// Quote a SQL identifier with double quotes.
/// If the identifier contains parentheses or `*` (i.e., it's an expression), leave it as-is.
fn quote_identifier(id: &str) -> String {
    if id.is_empty() {
        return "\"\"".to_string();
    }
    // If it looks like an expression (contains parens or wildcard), don't quote
    if id.contains('(') || id.contains('*') {
        return id.to_string();
    }
    format!("\"{}\"", id.replace('"', "\"\""))
}

/// Escape a string value for SQL (single quotes, escape internal quotes).
fn escape_string(s: &str) -> String {
    s.replace('\'', "''")
}

/// Map an aggregation function name to its SQL form.
fn aggregation_to_sql(agg: &str, column: &str) -> Option<String> {
    let quoted_col = quote_identifier(column);
    let agg_lower = agg.to_lowercase();
    match agg_lower.as_str() {
        "sum" => Some(format!("SUM({})", quoted_col)),
        "avg" => Some(format!("AVG({})", quoted_col)),
        "count" => Some(format!("COUNT({})", quoted_col)),
        "min" => Some(format!("MIN({})", quoted_col)),
        "max" => Some(format!("MAX({})", quoted_col)),
        "countdistinct" => Some(format!("COUNT(DISTINCT {})", quoted_col)),
        "median" => Some(format!("MEDIAN({})", quoted_col)),
        "stddev" => Some(format!("STDDEV({})", quoted_col)),
        "variance" => Some(format!("VARIANCE({})", quoted_col)),
        _ => {
            // Check for percentile patterns
            if agg_lower.starts_with("percentile") {
                let pct_str = agg_lower.strip_prefix("percentile")?;
                let pct: u32 = pct_str.parse().ok()?;
                let fraction = pct as f64 / 100.0;
                Some(format!(
                    "PERCENTILE_CONT({}) WITHIN GROUP (ORDER BY {})",
                    fraction, quoted_col
                ))
            } else {
                None
            }
        }
    }
}

/// Build a symbol table from dimensions and measures.
/// Maps field names to their SQL expressions.
fn build_symbol_table(
    dimensions: &[Dimension],
    measures: &[Measure],
) -> HashMap<String, Symbol> {
    let mut symbols = HashMap::new();

    // Process dimensions
    for dim in dimensions {
        match dim {
            Dimension::Simple(name) => {
                symbols.insert(
                    name.clone(),
                    Symbol {
                        sql: quote_identifier(name),
                        is_aggregated: false,
                    },
                );
            }
            Dimension::Detailed(spec) => {
                let field_name = spec.name.clone().unwrap_or_else(|| spec.column.clone());
                // Don't quote if it's an expression (contains parens)
                let sql_expr = if spec.column.contains('(') {
                    spec.column.clone()
                } else {
                    quote_identifier(&spec.column)
                };
                symbols.insert(
                    field_name,
                    Symbol {
                        sql: sql_expr,
                        is_aggregated: false,
                    },
                );
            }
        }
    }

    // First pass: aggregated measures
    let mut calculated: Vec<(String, String)> = Vec::new();
    for measure in measures {
        if let Some(ref agg) = measure.aggregation {
            if let Some(ref col) = measure.column {
                if let Some(sql_expr) = aggregation_to_sql(agg, col) {
                    symbols.insert(
                        measure.name.clone(),
                        Symbol {
                            sql: sql_expr,
                            is_aggregated: true,
                        },
                    );
                }
            }
        } else if let Some(ref expr) = measure.expression {
            calculated.push((measure.name.clone(), expr.clone()));
        }
    }

    // Second pass: calculated/expression measures
    for (field_name, expression) in calculated {
        let resolved = resolve_expression(&expression, &symbols);
        symbols.insert(
            field_name,
            Symbol {
                sql: resolved,
                is_aggregated: true,
            },
        );
    }

    symbols
}

/// Resolve field references in an expression to their SQL expressions.
/// Replaces field names with their SQL from the symbol table.
fn resolve_expression(expression: &str, symbols: &HashMap<String, Symbol>) -> String {
    let mut resolved = expression.to_string();

    // Sort field names by length (longest first) to avoid partial replacements
    let mut field_names: Vec<&String> = symbols.keys().collect();
    field_names.sort_by_key(|b| std::cmp::Reverse(b.len()));

    for field_name in field_names {
        if let Some(symbol) = symbols.get(field_name) {
            // Replace whole-word occurrences
            // Use a simple approach: split by word boundaries and replace
            resolved = replace_whole_word(&resolved, field_name, &symbol.sql);
        }
    }

    format!("({})", resolved)
}

/// Replace whole-word occurrences of `target` with `replacement` in `text`.
fn replace_whole_word(text: &str, target: &str, replacement: &str) -> String {
    if target.is_empty() {
        return text.to_string();
    }

    let mut result = String::new();
    let mut remaining = text;

    while let Some(pos) = remaining.find(target) {
        // Check character before match (UTF-8 safe)
        let before_ok = if pos == 0 {
            true
        } else {
            match remaining[..pos].chars().last() {
                Some(ch) => !ch.is_alphanumeric() && ch != '_',
                None => true,
            }
        };

        // Check character after match (UTF-8 safe)
        let after_pos = pos + target.len();
        let after_ok = if after_pos >= remaining.len() {
            true
        } else {
            match remaining[after_pos..].chars().next() {
                Some(ch) => !ch.is_alphanumeric() && ch != '_',
                None => true,
            }
        };

        if before_ok && after_ok {
            result.push_str(&remaining[..pos]);
            result.push_str(replacement);
            remaining = &remaining[after_pos..];
        } else {
            result.push_str(&remaining[..after_pos]);
            remaining = &remaining[after_pos..];
        }
    }
    result.push_str(remaining);

    result
}

/// Format a filter value for SQL.
fn format_filter_value(value: &Option<serde_json::Value>, operator: &str) -> String {
    // NULL operators don't need values
    if operator == "isNull" || operator == "isNotNull" {
        return String::new();
    }

    let value = match value {
        Some(v) => v,
        None => return String::new(),
    };

    match operator {
        "in" | "notIn" => {
            let items = match value {
                serde_json::Value::Array(arr) => arr.clone(),
                other => vec![other.clone()],
            };
            let formatted: Vec<String> = items
                .iter()
                .map(|v| match v {
                    serde_json::Value::String(s) => format!("'{}'", escape_string(s)),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    _ => format!("'{}'", v),
                })
                .collect();
            format!("({})", formatted.join(", "))
        }
        "between" => {
            if let serde_json::Value::Array(arr) = value {
                if arr.len() == 2 {
                    let v1 = format_scalar_value(&arr[0]);
                    let v2 = format_scalar_value(&arr[1]);
                    return format!("{} AND {}", v1, v2);
                }
            }
            String::new()
        }
        "contains" => {
            let s = value.as_str().unwrap_or("");
            format!("'%{}%'", escape_string(s))
        }
        "startsWith" => {
            let s = value.as_str().unwrap_or("");
            format!("'{}%'", escape_string(s))
        }
        "endsWith" => {
            let s = value.as_str().unwrap_or("");
            format!("'%{}'", escape_string(s))
        }
        _ => format_scalar_value(value),
    }
}

/// Format a single scalar value for SQL.
fn format_scalar_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => format!("'{}'", escape_string(s)),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "NULL".to_string(),
        _ => format!("'{}'", value),
    }
}

/// Map operator name to SQL operator.
fn operator_to_sql(op: &str) -> Option<&'static str> {
    match op {
        "=" | "==" => Some("="),
        "!=" => Some("!="),
        "<" => Some("<"),
        ">" => Some(">"),
        "<=" => Some("<="),
        ">=" => Some(">="),
        "contains" | "startsWith" | "endsWith" => Some("LIKE"),
        "isNull" => Some("IS NULL"),
        "isNotNull" => Some("IS NOT NULL"),
        "in" => Some("IN"),
        "notIn" => Some("NOT IN"),
        "between" => Some("BETWEEN"),
        _ => None,
    }
}

/// Build a single filter condition SQL string.
fn build_filter_condition(rule: &FilterRule, symbols: &HashMap<String, Symbol>) -> String {
    let sql_op = match operator_to_sql(&rule.operator) {
        Some(op) => op,
        None => return String::new(),
    };

    // Resolve field to SQL expression
    let sql_expr = if let Some(sym) = symbols.get(&rule.field) {
        sym.sql.clone()
    } else {
        quote_identifier(&rule.field)
    };

    // Handle empty arrays for IN/NOT IN
    if rule.operator == "in" || rule.operator == "notIn" {
        if let Some(serde_json::Value::Array(arr)) = &rule.value {
            if arr.is_empty() {
                return if rule.operator == "in" {
                    "(1=0)".to_string()
                } else {
                    "(1=1)".to_string()
                };
            }
        }
    }

    let formatted_value = format_filter_value(&rule.value, &rule.operator);

    if rule.operator == "isNull" || rule.operator == "isNotNull" {
        return format!("{} {}", sql_expr, sql_op);
    }

    format!("{} {} {}", sql_expr, sql_op, formatted_value)
}

/// Build a filter clause (conditions joined by combinator).
fn build_filter_clause(
    filter: &FilterGroup,
    symbols: &HashMap<String, Symbol>,
) -> String {
    if filter.rules.is_empty() {
        return String::new();
    }

    let combinator = filter
        .combinator
        .as_deref()
        .unwrap_or("and")
        .to_uppercase();

    let conditions: Vec<String> = filter
        .rules
        .iter()
        .map(|rule| build_filter_condition(rule, symbols))
        .filter(|c| !c.is_empty())
        .collect();

    if conditions.is_empty() {
        return String::new();
    }

    conditions.join(&format!(" {} ", combinator))
}

/// Partition filters into WHERE (pre-aggregation on dimensions/raw columns)
/// and HAVING (post-aggregation on measures/expressions).
fn partition_filters(
    filter: &FilterGroup,
    symbols: &HashMap<String, Symbol>,
) -> (Option<FilterGroup>, Option<FilterGroup>) {
    let mut where_rules = Vec::new();
    let mut having_rules = Vec::new();

    for rule in &filter.rules {
        let symbol = symbols.get(&rule.field);
        if symbol.is_some_and(|s| s.is_aggregated) {
            having_rules.push(rule.clone());
        } else {
            where_rules.push(rule.clone());
        }
    }

    let combinator = filter.combinator.clone();

    let where_filter = if where_rules.is_empty() {
        None
    } else {
        Some(FilterGroup {
            combinator: combinator.clone(),
            rules: where_rules,
        })
    };

    let having_filter = if having_rules.is_empty() {
        None
    } else {
        Some(FilterGroup {
            combinator,
            rules: having_rules,
        })
    };

    (where_filter, having_filter)
}

/// Build a complete SQL query from an `AggregateSpec`.
///
/// Generates: `SELECT ... FROM table [WHERE ...] [GROUP BY ...] [HAVING ...] [ORDER BY ...] [LIMIT ...]`
pub fn build_aggregate_sql(table_name: &str, spec: &AggregateSpec) -> String {
    let is_passthrough = spec.dimensions.is_empty() && spec.measures.is_empty();

    if is_passthrough {
        // Simple SELECT * with optional filters/sort/limit
        let mut sql = format!("SELECT * FROM {}", table_name);

        if let Some(ref filters) = spec.filters {
            let symbols = HashMap::new();
            let clause = build_filter_clause(filters, &symbols);
            if !clause.is_empty() {
                sql.push_str(&format!("\nWHERE {}", clause));
            }
        }

        if let Some(ref sorts) = spec.sort {
            if !sorts.is_empty() {
                let order_clauses: Vec<String> = sorts
                    .iter()
                    .map(|s| {
                        let dir = s
                            .direction
                            .as_deref()
                            .unwrap_or("ASC")
                            .to_uppercase();
                        format!("{} {}", quote_identifier(&s.field), dir)
                    })
                    .collect();
                sql.push_str(&format!("\nORDER BY {}", order_clauses.join(", ")));
            }
        }

        if let Some(limit) = spec.limit {
            sql.push_str(&format!("\nLIMIT {}", limit));
        }

        return sql;
    }

    let symbols = build_symbol_table(&spec.dimensions, &spec.measures);
    let has_aggregation = spec
        .measures
        .iter()
        .any(|m| m.aggregation.is_some() || m.expression.is_some());

    // Build SELECT columns and GROUP BY columns
    let mut select_cols = Vec::new();
    let mut group_by_cols = Vec::new();

    for dim in &spec.dimensions {
        match dim {
            Dimension::Simple(name) => {
                let quoted = quote_identifier(name);
                select_cols.push(quoted.clone());
                if has_aggregation {
                    group_by_cols.push(quoted);
                }
            }
            Dimension::Detailed(dspec) => {
                let field_name = dspec
                    .name
                    .clone()
                    .unwrap_or_else(|| dspec.column.clone());
                let sql_expr = if dspec.column.contains('(') {
                    dspec.column.clone()
                } else {
                    quote_identifier(&dspec.column)
                };

                if field_name == dspec.column && !dspec.column.contains('(') {
                    select_cols.push(quote_identifier(&field_name));
                } else {
                    select_cols.push(format!(
                        "{} as {}",
                        sql_expr,
                        quote_identifier(&field_name)
                    ));
                }

                if has_aggregation {
                    group_by_cols.push(sql_expr);
                }
            }
        }
    }

    // Add measures to SELECT
    for measure in &spec.measures {
        if let Some(symbol) = symbols.get(&measure.name) {
            if measure.name == symbol.sql {
                select_cols.push(quote_identifier(&measure.name));
            } else {
                select_cols.push(format!(
                    "{} as {}",
                    symbol.sql,
                    quote_identifier(&measure.name)
                ));
            }
        }
    }

    // Build WHERE and HAVING
    let mut where_clause = String::new();
    let mut having_clause = String::new();

    if let Some(ref filters) = spec.filters {
        let (where_filter, having_filter) = partition_filters(filters, &symbols);

        if let Some(ref wf) = where_filter {
            let clause = build_filter_clause(wf, &symbols);
            if !clause.is_empty() {
                where_clause = format!("\nWHERE {}", clause);
            }
        }

        if let Some(ref hf) = having_filter {
            let clause = build_filter_clause(hf, &symbols);
            if !clause.is_empty() {
                having_clause = format!("\nHAVING {}", clause);
            }
        }
    }

    // GROUP BY
    let group_by_clause = if has_aggregation && !group_by_cols.is_empty() {
        format!("\nGROUP BY {}", group_by_cols.join(", "))
    } else {
        String::new()
    };

    // ORDER BY
    let order_by_clause = if let Some(ref sorts) = spec.sort {
        if sorts.is_empty() {
            String::new()
        } else {
            let clauses: Vec<String> = sorts
                .iter()
                .map(|s| {
                    let dir = s
                        .direction
                        .as_deref()
                        .unwrap_or("ASC")
                        .to_uppercase();
                    format!("{} {}", quote_identifier(&s.field), dir)
                })
                .collect();
            format!("\nORDER BY {}", clauses.join(", "))
        }
    } else {
        String::new()
    };

    // LIMIT
    let limit_clause = if let Some(limit) = spec.limit {
        format!("\nLIMIT {}", limit)
    } else {
        String::new()
    };

    let select_str = select_cols.join(",\n  ");
    format!(
        "SELECT\n  {}\nFROM {}{}{}{}{}{}",
        select_str,
        table_name,
        where_clause,
        group_by_clause,
        having_clause,
        order_by_clause,
        limit_clause
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use chartml_core::spec::*;

    #[test]
    fn test_aggregate_sql_basic() {
        let spec = AggregateSpec {
            dimensions: vec![Dimension::Simple("region".to_string())],
            measures: vec![Measure {
                column: Some("revenue".to_string()),
                aggregation: Some("sum".to_string()),
                name: "total_revenue".to_string(),
                expression: None,
            }],
            filters: None,
            sort: None,
            limit: None,
        };

        let sql = build_aggregate_sql("source", &spec);
        assert!(sql.contains("SELECT"), "SQL: {}", sql);
        assert!(sql.contains("\"region\""), "SQL: {}", sql);
        assert!(sql.contains("SUM(\"revenue\") as \"total_revenue\""), "SQL: {}", sql);
        assert!(sql.contains("FROM source"), "SQL: {}", sql);
        assert!(sql.contains("GROUP BY \"region\""), "SQL: {}", sql);
    }

    #[test]
    fn test_aggregate_sql_with_filters() {
        let spec = AggregateSpec {
            dimensions: vec![Dimension::Simple("region".to_string())],
            measures: vec![Measure {
                column: Some("revenue".to_string()),
                aggregation: Some("sum".to_string()),
                name: "total_revenue".to_string(),
                expression: None,
            }],
            filters: Some(FilterGroup {
                combinator: None,
                rules: vec![
                    FilterRule {
                        field: "category".to_string(),
                        operator: "=".to_string(),
                        value: Some(serde_json::json!("Electronics")),
                    },
                    FilterRule {
                        field: "total_revenue".to_string(),
                        operator: ">=".to_string(),
                        value: Some(serde_json::json!(50000)),
                    },
                ],
            }),
            sort: None,
            limit: None,
        };

        let sql = build_aggregate_sql("source", &spec);
        // "category" should be in WHERE (it's a dimension/raw column)
        assert!(sql.contains("WHERE"), "SQL should have WHERE: {}", sql);
        assert!(
            sql.contains("\"category\" = 'Electronics'"),
            "WHERE should filter category: {}",
            sql
        );
        // "total_revenue" should be in HAVING (it's a measure)
        assert!(sql.contains("HAVING"), "SQL should have HAVING: {}", sql);
        assert!(
            sql.contains("SUM(\"revenue\") >= 50000"),
            "HAVING should filter total_revenue: {}",
            sql
        );
    }

    #[test]
    fn test_aggregate_sql_with_expressions() {
        let spec = AggregateSpec {
            dimensions: vec![Dimension::Simple("region".to_string())],
            measures: vec![
                Measure {
                    column: Some("revenue".to_string()),
                    aggregation: Some("sum".to_string()),
                    name: "total_revenue".to_string(),
                    expression: None,
                },
                Measure {
                    column: Some("units".to_string()),
                    aggregation: Some("sum".to_string()),
                    name: "total_units".to_string(),
                    expression: None,
                },
                Measure {
                    column: None,
                    aggregation: None,
                    name: "avg_price".to_string(),
                    expression: Some("total_revenue / total_units".to_string()),
                },
            ],
            filters: None,
            sort: None,
            limit: None,
        };

        let sql = build_aggregate_sql("source", &spec);
        assert!(sql.contains("SUM(\"revenue\") as \"total_revenue\""), "SQL: {}", sql);
        assert!(sql.contains("SUM(\"units\") as \"total_units\""), "SQL: {}", sql);
        // Expression should be resolved with the aggregated SQL
        assert!(
            sql.contains("(SUM(\"revenue\") / SUM(\"units\")) as \"avg_price\""),
            "Expression measure should be inlined: {}",
            sql
        );
    }

    #[test]
    fn test_aggregate_sql_passthrough() {
        let spec = AggregateSpec {
            dimensions: vec![],
            measures: vec![],
            filters: None,
            sort: Some(vec![SortSpec {
                field: "name".to_string(),
                direction: Some("asc".to_string()),
            }]),
            limit: Some(10),
        };

        let sql = build_aggregate_sql("source", &spec);
        assert!(sql.contains("SELECT * FROM source"), "SQL: {}", sql);
        assert!(sql.contains("ORDER BY"), "SQL: {}", sql);
        assert!(sql.contains("LIMIT 10"), "SQL: {}", sql);
    }

    #[test]
    fn test_quote_identifier() {
        assert_eq!(quote_identifier("region"), "\"region\"");
        assert_eq!(
            quote_identifier("DATE_TRUNC(sale_date, 'MONTH')"),
            "DATE_TRUNC(sale_date, 'MONTH')"
        );
        assert_eq!(quote_identifier("*"), "*");
        assert_eq!(quote_identifier(""), "\"\"");
    }

    #[test]
    fn test_aggregate_sql_sort_and_limit() {
        let spec = AggregateSpec {
            dimensions: vec![Dimension::Simple("region".to_string())],
            measures: vec![Measure {
                column: Some("revenue".to_string()),
                aggregation: Some("sum".to_string()),
                name: "total_revenue".to_string(),
                expression: None,
            }],
            filters: None,
            sort: Some(vec![SortSpec {
                field: "total_revenue".to_string(),
                direction: Some("desc".to_string()),
            }]),
            limit: Some(5),
        };

        let sql = build_aggregate_sql("source", &spec);
        assert!(sql.contains("ORDER BY \"total_revenue\" DESC"), "SQL: {}", sql);
        assert!(sql.contains("LIMIT 5"), "SQL: {}", sql);
    }

    #[test]
    fn test_aggregate_sql_count_distinct() {
        let spec = AggregateSpec {
            dimensions: vec![Dimension::Simple("region".to_string())],
            measures: vec![Measure {
                column: Some("customer_id".to_string()),
                aggregation: Some("countdistinct".to_string()),
                name: "unique_customers".to_string(),
                expression: None,
            }],
            filters: None,
            sort: None,
            limit: None,
        };

        let sql = build_aggregate_sql("source", &spec);
        assert!(
            sql.contains("COUNT(DISTINCT \"customer_id\") as \"unique_customers\""),
            "SQL: {}",
            sql
        );
    }

    #[test]
    fn test_aggregate_sql_detailed_dimension() {
        let spec = AggregateSpec {
            dimensions: vec![Dimension::Detailed(DimensionSpec {
                column: "DATE_TRUNC(sale_date, 'MONTH')".to_string(),
                name: Some("month".to_string()),
                dim_type: None,
            })],
            measures: vec![Measure {
                column: Some("revenue".to_string()),
                aggregation: Some("sum".to_string()),
                name: "total_revenue".to_string(),
                expression: None,
            }],
            filters: None,
            sort: None,
            limit: None,
        };

        let sql = build_aggregate_sql("source", &spec);
        assert!(
            sql.contains("DATE_TRUNC(sale_date, 'MONTH') as \"month\""),
            "SQL: {}",
            sql
        );
        assert!(
            sql.contains("GROUP BY DATE_TRUNC(sale_date, 'MONTH')"),
            "SQL: {}",
            sql
        );
    }
}
