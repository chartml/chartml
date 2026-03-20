use crate::data::{get_f64, get_string, Row};
use crate::error::ChartError;
use crate::spec::{AggregateSpec, Dimension, FilterGroup, FilterRule, SortSpec, TransformSpec};
use std::collections::HashMap;

/// Apply transforms to data.
pub fn apply_transforms(data: Vec<Row>, spec: &TransformSpec) -> Result<Vec<Row>, ChartError> {
    let mut result = data;

    if let Some(ref agg) = spec.aggregate {
        result = aggregate(&result, agg)?;
    }

    Ok(result)
}

/// Aggregate data by dimensions with measures.
fn aggregate(data: &[Row], spec: &AggregateSpec) -> Result<Vec<Row>, ChartError> {
    // 1. Extract dimension output names and source column names
    let dim_fields: Vec<String> = spec
        .dimensions
        .iter()
        .map(|d| match d {
            Dimension::Simple(name) => name.clone(),
            Dimension::Detailed(spec) => spec.name.clone().unwrap_or_else(|| spec.column.clone()),
        })
        .collect();

    let dim_columns: Vec<String> = spec
        .dimensions
        .iter()
        .map(|d| match d {
            Dimension::Simple(name) => name.clone(),
            Dimension::Detailed(spec) => spec.column.clone(),
        })
        .collect();

    // 2. Group rows by dimension values
    let mut groups: HashMap<Vec<String>, Vec<&Row>> = HashMap::new();
    for row in data {
        let key: Vec<String> = dim_columns
            .iter()
            .map(|field| get_string(row, field).unwrap_or_default())
            .collect();
        groups.entry(key).or_default().push(row);
    }

    // 3. For each group, compute measures
    let mut result: Vec<Row> = Vec::new();
    for (key, rows) in &groups {
        let mut out_row = Row::new();

        // Add dimension values
        for (i, field_name) in dim_fields.iter().enumerate() {
            out_row.insert(
                field_name.clone(),
                serde_json::Value::String(key[i].clone()),
            );
        }

        // Compute each measure
        for measure in &spec.measures {
            let value = if let Some(ref col) = measure.column {
                if let Some(ref agg_fn) = measure.aggregation {
                    compute_aggregation(rows, col, agg_fn)?
                } else {
                    // No aggregation specified, take first value
                    rows.first()
                        .and_then(|r| get_f64(r, col))
                        .unwrap_or(0.0)
                }
            } else if let Some(ref expr) = measure.expression {
                evaluate_expression(&out_row, expr)?
            } else {
                0.0
            };
            out_row.insert(measure.name.clone(), serde_json::json!(value));
        }

        result.push(out_row);
    }

    // 4. Apply filters (if any)
    if let Some(ref filters) = spec.filters {
        result = apply_filters(result, filters);
    }

    // 5. Apply sort (if any)
    if let Some(ref sorts) = spec.sort {
        apply_sort(&mut result, sorts);
    }

    // 6. Apply limit (if any)
    if let Some(limit) = spec.limit {
        result.truncate(limit as usize);
    }

    Ok(result)
}

fn compute_aggregation(rows: &[&Row], column: &str, agg: &str) -> Result<f64, ChartError> {
    let values: Vec<f64> = rows.iter().filter_map(|r| get_f64(r, column)).collect();
    if values.is_empty() {
        return Ok(0.0);
    }

    Ok(match agg {
        "sum" => values.iter().sum(),
        "avg" => values.iter().sum::<f64>() / values.len() as f64,
        "count" => values.len() as f64,
        "min" => values.iter().cloned().fold(f64::INFINITY, f64::min),
        "max" => values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        "countDistinct" => {
            let mut seen = std::collections::HashSet::new();
            for v in &values {
                seen.insert(v.to_bits());
            }
            seen.len() as f64
        }
        "median" => {
            let mut sorted = values.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let mid = sorted.len() / 2;
            if sorted.len() % 2 == 0 {
                (sorted[mid - 1] + sorted[mid]) / 2.0
            } else {
                sorted[mid]
            }
        }
        other => {
            return Err(ChartError::InvalidSpec(format!(
                "Unknown aggregation: {}",
                other
            )))
        }
    })
}

/// Evaluate a simple arithmetic expression referencing fields already computed in the row.
/// Supports: field_a / field_b, field_a * field_b, field_a + field_b, field_a - field_b
fn evaluate_expression(row: &Row, expr: &str) -> Result<f64, ChartError> {
    // Find the operator
    let operators = ['/', '*', '+', '-'];
    for op in &operators {
        if let Some(pos) = expr.find(*op) {
            // Make sure it's not a negative sign at position 0
            if *op == '-' && pos == 0 {
                continue;
            }
            let left_name = expr[..pos].trim();
            let right_name = expr[pos + 1..].trim();

            let left_val = get_f64(row, left_name).ok_or_else(|| {
                ChartError::DataError(format!(
                    "Expression field '{}' not found in row",
                    left_name
                ))
            })?;
            let right_val = get_f64(row, right_name).ok_or_else(|| {
                ChartError::DataError(format!(
                    "Expression field '{}' not found in row",
                    right_name
                ))
            })?;

            return Ok(match op {
                '+' => left_val + right_val,
                '-' => left_val - right_val,
                '*' => left_val * right_val,
                '/' => {
                    if right_val == 0.0 {
                        0.0
                    } else {
                        left_val / right_val
                    }
                }
                _ => unreachable!(),
            });
        }
    }

    // No operator found — try interpreting as a field reference
    get_f64(row, expr.trim()).ok_or_else(|| {
        ChartError::DataError(format!(
            "Cannot evaluate expression '{}': field not found",
            expr
        ))
    })
}

fn apply_filters(data: Vec<Row>, filters: &FilterGroup) -> Vec<Row> {
    let is_and = filters.combinator.as_deref() != Some("or");

    data.into_iter()
        .filter(|row| {
            let results: Vec<bool> = filters.rules.iter().map(|rule| eval_filter_rule(row, rule)).collect();

            if is_and {
                results.iter().all(|&r| r)
            } else {
                results.iter().any(|&r| r)
            }
        })
        .collect()
}

fn eval_filter_rule(row: &Row, rule: &FilterRule) -> bool {
    let field_val = row.get(&rule.field);

    match rule.operator.as_str() {
        "isNull" => field_val.is_none() || field_val == Some(&serde_json::Value::Null),
        "isNotNull" => field_val.is_some() && field_val != Some(&serde_json::Value::Null),
        "=" | "==" => rule
            .value
            .as_ref()
            .map_or(false, |v| field_val == Some(v)),
        "!=" => rule
            .value
            .as_ref()
            .map_or(false, |v| field_val != Some(v)),
        ">" => compare_values(field_val, rule.value.as_ref(), |a, b| a > b),
        ">=" => compare_values(field_val, rule.value.as_ref(), |a, b| a >= b),
        "<" => compare_values(field_val, rule.value.as_ref(), |a, b| a < b),
        "<=" => compare_values(field_val, rule.value.as_ref(), |a, b| a <= b),
        "in" => {
            if let (Some(fv), Some(serde_json::Value::Array(arr))) =
                (field_val, rule.value.as_ref())
            {
                arr.contains(fv)
            } else {
                false
            }
        }
        "contains" => {
            if let (Some(serde_json::Value::String(fv)), Some(serde_json::Value::String(rv))) =
                (field_val, rule.value.as_ref())
            {
                fv.contains(rv.as_str())
            } else {
                false
            }
        }
        _ => true, // unknown operator, pass through
    }
}

fn compare_values(
    field: Option<&serde_json::Value>,
    rule_val: Option<&serde_json::Value>,
    cmp: impl Fn(f64, f64) -> bool,
) -> bool {
    match (field, rule_val) {
        (Some(serde_json::Value::Number(a)), Some(serde_json::Value::Number(b))) => {
            if let (Some(fa), Some(fb)) = (a.as_f64(), b.as_f64()) {
                cmp(fa, fb)
            } else {
                false
            }
        }
        _ => false,
    }
}

fn apply_sort(data: &mut Vec<Row>, sorts: &[SortSpec]) {
    data.sort_by(|a, b| {
        for sort in sorts {
            let a_val = get_f64(a, &sort.field);
            let b_val = get_f64(b, &sort.field);
            let ord = match (a_val, b_val) {
                (Some(av), Some(bv)) => av
                    .partial_cmp(&bv)
                    .unwrap_or(std::cmp::Ordering::Equal),
                _ => std::cmp::Ordering::Equal,
            };
            let ord = if sort.direction.as_deref() == Some("desc") {
                ord.reverse()
            } else {
                ord
            };
            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
        }
        std::cmp::Ordering::Equal
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{AggregateSpec, Dimension, Measure, SortSpec, TransformSpec};
    use serde_json::json;

    fn make_row(pairs: Vec<(&str, serde_json::Value)>) -> Row {
        pairs
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect()
    }

    fn sales_data() -> Vec<Row> {
        vec![
            make_row(vec![
                ("region", json!("North")),
                ("product", json!("Widget")),
                ("revenue", json!(100.0)),
                ("units", json!(10)),
            ]),
            make_row(vec![
                ("region", json!("North")),
                ("product", json!("Gadget")),
                ("revenue", json!(200.0)),
                ("units", json!(15)),
            ]),
            make_row(vec![
                ("region", json!("South")),
                ("product", json!("Widget")),
                ("revenue", json!(150.0)),
                ("units", json!(12)),
            ]),
            make_row(vec![
                ("region", json!("South")),
                ("product", json!("Widget")),
                ("revenue", json!(50.0)),
                ("units", json!(5)),
            ]),
            make_row(vec![
                ("region", json!("East")),
                ("product", json!("Gadget")),
                ("revenue", json!(300.0)),
                ("units", json!(20)),
            ]),
        ]
    }

    #[test]
    fn aggregate_sum() {
        let data = sales_data();
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

        let result = aggregate(&data, &spec).unwrap();
        assert_eq!(result.len(), 3);

        // Find each region and check totals
        let north = result
            .iter()
            .find(|r| get_string(r, "region") == Some("North".to_string()))
            .unwrap();
        assert_eq!(get_f64(north, "total_revenue"), Some(300.0));

        let south = result
            .iter()
            .find(|r| get_string(r, "region") == Some("South".to_string()))
            .unwrap();
        assert_eq!(get_f64(south, "total_revenue"), Some(200.0));

        let east = result
            .iter()
            .find(|r| get_string(r, "region") == Some("East".to_string()))
            .unwrap();
        assert_eq!(get_f64(east, "total_revenue"), Some(300.0));
    }

    #[test]
    fn aggregate_avg() {
        let data = sales_data();
        let spec = AggregateSpec {
            dimensions: vec![Dimension::Simple("region".to_string())],
            measures: vec![Measure {
                column: Some("revenue".to_string()),
                aggregation: Some("avg".to_string()),
                name: "avg_revenue".to_string(),
                expression: None,
            }],
            filters: None,
            sort: None,
            limit: None,
        };

        let result = aggregate(&data, &spec).unwrap();
        let north = result
            .iter()
            .find(|r| get_string(r, "region") == Some("North".to_string()))
            .unwrap();
        assert_eq!(get_f64(north, "avg_revenue"), Some(150.0)); // (100+200)/2

        let south = result
            .iter()
            .find(|r| get_string(r, "region") == Some("South".to_string()))
            .unwrap();
        assert_eq!(get_f64(south, "avg_revenue"), Some(100.0)); // (150+50)/2
    }

    #[test]
    fn aggregate_count() {
        let data = sales_data();
        let spec = AggregateSpec {
            dimensions: vec![Dimension::Simple("region".to_string())],
            measures: vec![Measure {
                column: Some("revenue".to_string()),
                aggregation: Some("count".to_string()),
                name: "count".to_string(),
                expression: None,
            }],
            filters: None,
            sort: None,
            limit: None,
        };

        let result = aggregate(&data, &spec).unwrap();
        let north = result
            .iter()
            .find(|r| get_string(r, "region") == Some("North".to_string()))
            .unwrap();
        assert_eq!(get_f64(north, "count"), Some(2.0));

        let south = result
            .iter()
            .find(|r| get_string(r, "region") == Some("South".to_string()))
            .unwrap();
        assert_eq!(get_f64(south, "count"), Some(2.0));
    }

    #[test]
    fn aggregate_min_max() {
        let data = sales_data();
        let spec = AggregateSpec {
            dimensions: vec![Dimension::Simple("region".to_string())],
            measures: vec![
                Measure {
                    column: Some("revenue".to_string()),
                    aggregation: Some("min".to_string()),
                    name: "min_rev".to_string(),
                    expression: None,
                },
                Measure {
                    column: Some("revenue".to_string()),
                    aggregation: Some("max".to_string()),
                    name: "max_rev".to_string(),
                    expression: None,
                },
            ],
            filters: None,
            sort: None,
            limit: None,
        };

        let result = aggregate(&data, &spec).unwrap();
        let south = result
            .iter()
            .find(|r| get_string(r, "region") == Some("South".to_string()))
            .unwrap();
        assert_eq!(get_f64(south, "min_rev"), Some(50.0));
        assert_eq!(get_f64(south, "max_rev"), Some(150.0));
    }

    #[test]
    fn aggregate_count_distinct() {
        let data = sales_data();
        let spec = AggregateSpec {
            dimensions: vec![Dimension::Simple("region".to_string())],
            measures: vec![Measure {
                column: Some("revenue".to_string()),
                aggregation: Some("countDistinct".to_string()),
                name: "distinct_rev".to_string(),
                expression: None,
            }],
            filters: None,
            sort: None,
            limit: None,
        };

        let result = aggregate(&data, &spec).unwrap();
        let north = result
            .iter()
            .find(|r| get_string(r, "region") == Some("North".to_string()))
            .unwrap();
        assert_eq!(get_f64(north, "distinct_rev"), Some(2.0)); // 100 and 200
    }

    #[test]
    fn aggregate_median() {
        let data = vec![
            make_row(vec![("g", json!("A")), ("v", json!(1.0))]),
            make_row(vec![("g", json!("A")), ("v", json!(3.0))]),
            make_row(vec![("g", json!("A")), ("v", json!(5.0))]),
            make_row(vec![("g", json!("B")), ("v", json!(2.0))]),
            make_row(vec![("g", json!("B")), ("v", json!(4.0))]),
        ];
        let spec = AggregateSpec {
            dimensions: vec![Dimension::Simple("g".to_string())],
            measures: vec![Measure {
                column: Some("v".to_string()),
                aggregation: Some("median".to_string()),
                name: "med".to_string(),
                expression: None,
            }],
            filters: None,
            sort: None,
            limit: None,
        };

        let result = aggregate(&data, &spec).unwrap();
        let a = result
            .iter()
            .find(|r| get_string(r, "g") == Some("A".to_string()))
            .unwrap();
        assert_eq!(get_f64(a, "med"), Some(3.0)); // odd count: middle

        let b = result
            .iter()
            .find(|r| get_string(r, "g") == Some("B".to_string()))
            .unwrap();
        assert_eq!(get_f64(b, "med"), Some(3.0)); // even count: (2+4)/2
    }

    #[test]
    fn aggregate_with_sort() {
        let data = sales_data();
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
            limit: None,
        };

        let result = aggregate(&data, &spec).unwrap();
        assert_eq!(result.len(), 3);
        // North=300, East=300, South=200 — both 300s first, then 200
        let first_val = get_f64(&result[0], "total_revenue").unwrap();
        let second_val = get_f64(&result[1], "total_revenue").unwrap();
        let third_val = get_f64(&result[2], "total_revenue").unwrap();
        assert!(first_val >= second_val);
        assert!(second_val >= third_val);
        assert_eq!(third_val, 200.0);
    }

    #[test]
    fn aggregate_with_limit() {
        let data = sales_data();
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
            limit: Some(2),
        };

        let result = aggregate(&data, &spec).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn aggregate_with_detailed_dimension() {
        let data = sales_data();
        let spec = AggregateSpec {
            dimensions: vec![Dimension::Detailed(crate::spec::DimensionSpec {
                column: "region".to_string(),
                name: Some("Region".to_string()),
                dim_type: None,
            })],
            measures: vec![Measure {
                column: Some("revenue".to_string()),
                aggregation: Some("sum".to_string()),
                name: "total".to_string(),
                expression: None,
            }],
            filters: None,
            sort: None,
            limit: None,
        };

        let result = aggregate(&data, &spec).unwrap();
        // Output field name should be "Region" not "region"
        let north = result
            .iter()
            .find(|r| get_string(r, "Region") == Some("North".to_string()))
            .unwrap();
        assert_eq!(get_f64(north, "total"), Some(300.0));
    }

    #[test]
    fn aggregate_with_filter_gt() {
        let data = sales_data();
        let spec = AggregateSpec {
            dimensions: vec![Dimension::Simple("region".to_string())],
            measures: vec![Measure {
                column: Some("revenue".to_string()),
                aggregation: Some("sum".to_string()),
                name: "total_revenue".to_string(),
                expression: None,
            }],
            filters: Some(FilterGroup {
                combinator: None, // defaults to "and"
                rules: vec![FilterRule {
                    field: "total_revenue".to_string(),
                    operator: ">".to_string(),
                    value: Some(json!(250)),
                }],
            }),
            sort: None,
            limit: None,
        };

        let result = aggregate(&data, &spec).unwrap();
        // North=300, East=300 pass; South=200 filtered out
        assert_eq!(result.len(), 2);
        for row in &result {
            assert!(get_f64(row, "total_revenue").unwrap() > 250.0);
        }
    }

    #[test]
    fn aggregate_with_filter_eq() {
        let data = sales_data();
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
                rules: vec![FilterRule {
                    field: "region".to_string(),
                    operator: "=".to_string(),
                    value: Some(json!("North")),
                }],
            }),
            sort: None,
            limit: None,
        };

        let result = aggregate(&data, &spec).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            get_string(&result[0], "region"),
            Some("North".to_string())
        );
    }

    #[test]
    fn aggregate_with_filter_in() {
        let data = sales_data();
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
                rules: vec![FilterRule {
                    field: "region".to_string(),
                    operator: "in".to_string(),
                    value: Some(json!(["North", "East"])),
                }],
            }),
            sort: None,
            limit: None,
        };

        let result = aggregate(&data, &spec).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn aggregate_with_filter_or_combinator() {
        let data = sales_data();
        let spec = AggregateSpec {
            dimensions: vec![Dimension::Simple("region".to_string())],
            measures: vec![Measure {
                column: Some("revenue".to_string()),
                aggregation: Some("sum".to_string()),
                name: "total_revenue".to_string(),
                expression: None,
            }],
            filters: Some(FilterGroup {
                combinator: Some("or".to_string()),
                rules: vec![
                    FilterRule {
                        field: "region".to_string(),
                        operator: "=".to_string(),
                        value: Some(json!("North")),
                    },
                    FilterRule {
                        field: "region".to_string(),
                        operator: "=".to_string(),
                        value: Some(json!("East")),
                    },
                ],
            }),
            sort: None,
            limit: None,
        };

        let result = aggregate(&data, &spec).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn aggregate_with_expression_measure() {
        let data = sales_data();
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

        let result = aggregate(&data, &spec).unwrap();
        let north = result
            .iter()
            .find(|r| get_string(r, "region") == Some("North".to_string()))
            .unwrap();
        // North: revenue=300, units=25, avg_price=12
        assert_eq!(get_f64(north, "total_revenue"), Some(300.0));
        assert_eq!(get_f64(north, "total_units"), Some(25.0));
        assert_eq!(get_f64(north, "avg_price"), Some(12.0));
    }

    #[test]
    fn apply_transforms_full_pipeline() {
        let data = sales_data();
        let spec = TransformSpec {
            aggregate: Some(AggregateSpec {
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
                limit: Some(2),
            }),
        };

        let result = apply_transforms(data, &spec).unwrap();
        assert_eq!(result.len(), 2);
        let first_val = get_f64(&result[0], "total_revenue").unwrap();
        let second_val = get_f64(&result[1], "total_revenue").unwrap();
        assert!(first_val >= second_val);
    }

    #[test]
    fn unknown_aggregation_returns_error() {
        let data = sales_data();
        let spec = AggregateSpec {
            dimensions: vec![Dimension::Simple("region".to_string())],
            measures: vec![Measure {
                column: Some("revenue".to_string()),
                aggregation: Some("bogus".to_string()),
                name: "x".to_string(),
                expression: None,
            }],
            filters: None,
            sort: None,
            limit: None,
        };

        let result = aggregate(&data, &spec);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown aggregation: bogus"));
    }

    #[test]
    fn filter_contains() {
        let row = make_row(vec![("name", json!("Hello World"))]);
        let rule = FilterRule {
            field: "name".to_string(),
            operator: "contains".to_string(),
            value: Some(json!("World")),
        };
        assert!(eval_filter_rule(&row, &rule));

        let rule_miss = FilterRule {
            field: "name".to_string(),
            operator: "contains".to_string(),
            value: Some(json!("xyz")),
        };
        assert!(!eval_filter_rule(&row, &rule_miss));
    }

    #[test]
    fn filter_is_null_and_is_not_null() {
        let row_with = make_row(vec![("a", json!(42))]);
        let row_null = make_row(vec![("a", serde_json::Value::Null)]);
        let row_missing: Row = HashMap::new();

        let rule_null = FilterRule {
            field: "a".to_string(),
            operator: "isNull".to_string(),
            value: None,
        };
        let rule_not_null = FilterRule {
            field: "a".to_string(),
            operator: "isNotNull".to_string(),
            value: None,
        };

        assert!(!eval_filter_rule(&row_with, &rule_null));
        assert!(eval_filter_rule(&row_with, &rule_not_null));

        assert!(eval_filter_rule(&row_null, &rule_null));
        assert!(!eval_filter_rule(&row_null, &rule_not_null));

        assert!(eval_filter_rule(&row_missing, &rule_null));
        assert!(!eval_filter_rule(&row_missing, &rule_not_null));
    }

    #[test]
    fn filter_ne() {
        let row = make_row(vec![("x", json!("A"))]);
        let rule = FilterRule {
            field: "x".to_string(),
            operator: "!=".to_string(),
            value: Some(json!("B")),
        };
        assert!(eval_filter_rule(&row, &rule));

        let rule_same = FilterRule {
            field: "x".to_string(),
            operator: "!=".to_string(),
            value: Some(json!("A")),
        };
        assert!(!eval_filter_rule(&row, &rule_same));
    }

    #[test]
    fn filter_lte_gte() {
        let row = make_row(vec![("v", json!(10))]);

        assert!(eval_filter_rule(
            &row,
            &FilterRule {
                field: "v".to_string(),
                operator: "<=".to_string(),
                value: Some(json!(10)),
            }
        ));
        assert!(eval_filter_rule(
            &row,
            &FilterRule {
                field: "v".to_string(),
                operator: ">=".to_string(),
                value: Some(json!(10)),
            }
        ));
        assert!(!eval_filter_rule(
            &row,
            &FilterRule {
                field: "v".to_string(),
                operator: "<".to_string(),
                value: Some(json!(10)),
            }
        ));
    }

    #[test]
    fn sort_asc_default() {
        let mut data = vec![
            make_row(vec![("v", json!(30))]),
            make_row(vec![("v", json!(10))]),
            make_row(vec![("v", json!(20))]),
        ];
        apply_sort(
            &mut data,
            &[SortSpec {
                field: "v".to_string(),
                direction: None, // defaults to asc
            }],
        );
        assert_eq!(get_f64(&data[0], "v"), Some(10.0));
        assert_eq!(get_f64(&data[1], "v"), Some(20.0));
        assert_eq!(get_f64(&data[2], "v"), Some(30.0));
    }

    #[test]
    fn empty_data_aggregation() {
        let data: Vec<Row> = vec![];
        let spec = AggregateSpec {
            dimensions: vec![Dimension::Simple("region".to_string())],
            measures: vec![Measure {
                column: Some("revenue".to_string()),
                aggregation: Some("sum".to_string()),
                name: "total".to_string(),
                expression: None,
            }],
            filters: None,
            sort: None,
            limit: None,
        };

        let result = aggregate(&data, &spec).unwrap();
        assert!(result.is_empty());
    }
}
