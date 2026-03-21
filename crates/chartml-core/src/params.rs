//! Parameter resolution for ChartML.
//!
//! Resolves `$blockname.param_id` and `$param_id` references in YAML specs
//! by substituting them with actual parameter values or defaults.
//! Matches the JS `parameterResolver.js` implementation.

use std::collections::HashMap;
use crate::spec::{ParamsSpec, ParamDef};

/// Collected parameter values — flat map from "blockname.param_id" or "param_id" to value.
pub type ParamValues = HashMap<String, serde_json::Value>;

/// Collect default parameter values from params component definitions.
/// Named params (with a block name): stored as "blockname.param_id" → default
/// Chart-level params (no block name): stored as "param_id" → default
pub fn collect_param_defaults(params_specs: &[&ParamsSpec]) -> ParamValues {
    let mut values = ParamValues::new();

    for spec in params_specs {
        for param in &spec.params {
            let key = if let Some(ref name) = spec.name {
                format!("{}.{}", name, param.id)
            } else {
                param.id.clone()
            };

            if let Some(ref default) = param.default {
                values.insert(key, default.clone());
            }
        }
    }

    values
}

/// Resolve parameter references in a YAML string.
///
/// Replaces `"$identifier"` and `"$identifier.path"` with their values.
/// Works on the raw YAML string before parsing, matching the JS approach
/// of JSON.stringify → regex replace → JSON.parse.
///
/// If a reference is not found in `param_values`, it is left unchanged
/// (the filter/transform will see the raw `$...` string).
pub fn resolve_param_references(yaml: &str, param_values: &ParamValues) -> String {
    let mut result = yaml.to_string();

    // Sort keys by length descending so longer paths are replaced first
    // (prevents "$dashboard_filters.region" matching "$dashboard_filters" first)
    let mut keys: Vec<&String> = param_values.keys().collect();
    keys.sort_by(|a, b| b.len().cmp(&a.len()));

    for key in keys {
        let value = &param_values[key];
        let pattern = format!("\"${}\"", key);

        if result.contains(&pattern) {
            let replacement = value_to_yaml(value);
            result = result.replace(&pattern, &replacement);
        }

        // Also handle unquoted references (e.g., value: $top_n without quotes)
        let _unquoted_pattern = format!("${}",  key);
        // Only replace if it's a standalone reference (not inside a longer string)
        // This is trickier — skip for now, quoted refs cover the spec
    }

    result
}

/// Convert a serde_json::Value to its YAML representation for substitution.
fn value_to_yaml(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => format!("\"{}\"", s),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(value_to_yaml).collect();
            format!("[{}]", items.join(", "))
        }
        serde_json::Value::Object(_obj) => {
            // For objects, use JSON format which YAML also accepts
            serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string())
        }
    }
}

/// Extract chart-level param defaults from a YAML string by attempting
/// to parse just the params section. This handles the case where params
/// are defined inside the chart spec itself (not as a separate component).
///
/// Scans for inline `params:` blocks and extracts id/default pairs.
pub fn extract_inline_param_defaults(yaml: &str) -> ParamValues {
    let mut values = ParamValues::new();

    // Try to parse the YAML as a mapping and look for a "params" key
    // that contains an array of param definitions
    if let Ok(serde_yaml::Value::Mapping(map)) = serde_yaml::from_str::<serde_yaml::Value>(yaml) {
        if let Some(params_val) = map.get(&serde_yaml::Value::String("params".into())) {
            if let Ok(params) = serde_yaml::from_value::<Vec<ParamDef>>(params_val.clone()) {
                for param in &params {
                    if let Some(ref default) = param.default {
                        values.insert(param.id.clone(), default.clone());
                    }
                }
            }
        }
    }

    values
}

/// Extract the set of parameter references from a YAML string.
/// Returns paths like "dashboard_filters.region" or "top_n".
pub fn extract_param_references(yaml: &str) -> Vec<String> {
    let mut refs = Vec::new();
    // Match "$identifier" or "$identifier.path" in the YAML
    let mut i = 0;
    let bytes = yaml.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'$' && i > 0 && (bytes[i - 1] == b'"' || bytes[i - 1] == b' ' || bytes[i - 1] == b':') {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len()
                && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_' || bytes[end] == b'.')
            {
                end += 1;
            }
            if end > start {
                refs.push(yaml[start..end].to_string());
            }
            i = end;
        } else {
            i += 1;
        }
    }
    refs.sort();
    refs.dedup();
    refs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_defaults_named_params() {
        let spec = ParamsSpec {
            version: 1,
            name: Some("dashboard_filters".to_string()),
            params: vec![
                ParamDef {
                    id: "region".to_string(),
                    param_type: "multiselect".to_string(),
                    label: "Region".to_string(),
                    options: Some(vec!["US".into(), "EU".into()]),
                    default: Some(serde_json::json!(["US", "EU"])),
                    placeholder: None,
                    layout: None,
                },
                ParamDef {
                    id: "min_revenue".to_string(),
                    param_type: "number".to_string(),
                    label: "Min Revenue".to_string(),
                    options: None,
                    default: Some(serde_json::json!(50000)),
                    placeholder: None,
                    layout: None,
                },
            ],
        };

        let defaults = collect_param_defaults(&[&spec]);
        assert_eq!(defaults.get("dashboard_filters.region"), Some(&serde_json::json!(["US", "EU"])));
        assert_eq!(defaults.get("dashboard_filters.min_revenue"), Some(&serde_json::json!(50000)));
    }

    #[test]
    fn resolve_string_param() {
        let mut params = ParamValues::new();
        params.insert("top_n".to_string(), serde_json::json!(10));

        let yaml = r#"limit: "$top_n""#;
        let resolved = resolve_param_references(yaml, &params);
        assert_eq!(resolved, "limit: 10");
    }

    #[test]
    fn resolve_array_param() {
        let mut params = ParamValues::new();
        params.insert("dashboard_filters.region".to_string(), serde_json::json!(["US", "EU"]));

        let yaml = r#"value: "$dashboard_filters.region""#;
        let resolved = resolve_param_references(yaml, &params);
        assert_eq!(resolved, r#"value: ["US", "EU"]"#);
    }

    #[test]
    fn unresolved_param_stays() {
        let params = ParamValues::new();
        let yaml = r#"value: "$unknown.param""#;
        let resolved = resolve_param_references(yaml, &params);
        assert_eq!(resolved, r#"value: "$unknown.param""#);
    }

    #[test]
    fn extract_refs() {
        let yaml = r#"
          value: "$dashboard_filters.region"
          limit: "$top_n"
        "#;
        let refs = extract_param_references(yaml);
        assert!(refs.contains(&"dashboard_filters.region".to_string()));
        assert!(refs.contains(&"top_n".to_string()));
    }
}
