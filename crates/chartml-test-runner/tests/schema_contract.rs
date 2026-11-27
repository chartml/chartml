//! CHA-51: the published JSON Schema (`docs/docs/public/chartml_schema.json`)
//! accepts every core structure the renderer supports and rejects what the
//! renderer's parser rejects.
//!
//! Every case is a spec. It is validated against the real published schema with
//! a real JSON Schema validator, and parsed by the real `chartml_core::spec::parse`.
//! A case states what both must say, so the schema and the parser cannot drift
//! apart on it. `data` descriptors are checked with the discrimination the parser
//! uses (an untagged `DataRef`).

use chartml_core::spec::{self, ChartMLSpec, Component, DataRef, StyleRefOrInline};
use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn validator() -> jsonschema::Validator {
    let text = fs::read_to_string(workspace_root().join("docs/docs/public/chartml_schema.json"))
        .expect("read the published schema");
    let schema: serde_json::Value = serde_json::from_str(&text).expect("schema is JSON");
    jsonschema::validator_for(&schema).expect("schema compiles")
}

fn to_json(yaml: &str) -> serde_json::Value {
    let value: serde_yaml::Value = serde_yaml::from_str(yaml).expect("case is YAML");
    serde_json::to_value(value).expect("YAML converts to JSON")
}

fn schema_errors(v: &jsonschema::Validator, yaml: &str) -> Vec<String> {
    v.iter_errors(&to_json(yaml)).map(|e| e.to_string()).collect()
}

/// The spec is valid against the schema and the parser accepts it.
fn assert_valid(name: &str, yaml: &str) -> ChartMLSpec {
    let errors = schema_errors(&validator(), yaml);
    assert!(errors.is_empty(), "{name}: schema rejects a spec the renderer supports: {errors:?}");
    spec::parse(yaml).unwrap_or_else(|e| panic!("{name}: parser rejects the case: {e}"))
}

/// The spec is rejected by the schema and by the parser.
fn assert_rejected(name: &str, yaml: &str) {
    assert!(
        !schema_errors(&validator(), yaml).is_empty(),
        "{name}: schema accepts a spec the renderer rejects"
    );
    assert!(spec::parse(yaml).is_err(), "{name}: the parser accepts the case, so it is not a negative case");
}

fn chart(yaml_after_version: &str) -> String {
    format!("type: chart\nversion: 1\n{yaml_after_version}")
}

fn single_chart(spec: ChartMLSpec) -> Box<spec::ChartSpec> {
    match spec {
        ChartMLSpec::Single(c) => match *c {
            Component::Chart(chart) => chart,
            other => panic!("expected a chart, got {other:?}"),
        },
        other => panic!("expected a single component, got {other:?}"),
    }
}

const BAR: &str = "visualize:\n  type: bar\n  columns: month\n  rows: revenue\n";

// --- named source maps -------------------------------------------------------

#[test]
fn named_source_map_with_datasource_and_query() {
    let yaml = chart(&format!(
        "data:\n  visitors:\n    datasource: plausible-analytics\n    query: SELECT 1\n    cache:\n      ttl: 6h\n      autoRefresh: true\n  revenue:\n    datasource: billing-postgres\n    query: SELECT 2\n{BAR}"
    ));
    let parsed = single_chart(assert_valid("named source map", &yaml));
    assert!(matches!(parsed.data, DataRef::NamedMap(ref m) if m.len() == 2));
}

#[test]
fn named_source_map_with_inline_rows() {
    let yaml = chart(&format!(
        "data:\n  a:\n    provider: inline\n    rows:\n      - {{ month: Jan, revenue: 1 }}\n  b:\n    rows:\n      - {{ month: Jan, revenue: 2 }}\n{BAR}"
    ));
    let parsed = single_chart(assert_valid("named source map with rows", &yaml));
    assert!(matches!(parsed.data, DataRef::NamedMap(_)));
}

#[test]
fn named_source_map_entry_with_unknown_key_is_rejected() {
    let yaml = chart(&format!("data:\n  visitors:\n    datasource: x\n    sql: SELECT 1\n{BAR}"));
    assert_rejected("named source entry with an unknown key", &yaml);
}

#[test]
fn named_source_map_entry_that_is_not_a_descriptor_is_rejected() {
    let yaml = chart(&format!("data:\n  visitors: 3\n{BAR}"));
    assert_rejected("named source entry that is a number", &yaml);
}

// --- datasource/query descriptors, optional providers, rows-only -------------

#[test]
fn datasource_query_descriptor_without_provider() {
    let yaml = chart(&format!("data:\n  datasource: production-postgres\n  query: SELECT month, revenue FROM sales\n{BAR}"));
    let parsed = single_chart(assert_valid("datasource/query descriptor", &yaml));
    match parsed.data {
        DataRef::Inline(d) => assert!(d.provider.is_none()),
        other => panic!("expected a single descriptor, got {other:?}"),
    }
}

#[test]
fn rows_only_descriptor() {
    let yaml = chart(&format!("data:\n  rows:\n    - {{ month: Jan, revenue: 1 }}\n{BAR}"));
    let parsed = single_chart(assert_valid("rows-only descriptor", &yaml));
    assert!(matches!(parsed.data, DataRef::Inline(ref d) if d.provider.is_none() && d.rows.is_some()));
}

#[test]
fn url_descriptor_with_http_and_custom_providers() {
    let http = chart(&format!("data:\n  provider: http\n  url: https://api.example.com/sales\n  cache:\n    ttl: 1d\n{BAR}"));
    assert_valid("http descriptor", &http);
    let custom = chart(&format!("data:\n  provider: warehouse\n  query: SELECT 1\n{BAR}"));
    assert_valid("host-provided provider", &custom);
}

#[test]
fn descriptor_with_unknown_key_is_rejected() {
    let yaml = chart(&format!("data:\n  provider: inline\n  row:\n    - {{ month: Jan }}\n{BAR}"));
    assert_rejected("descriptor with a misspelled rows key", &yaml);
}

#[test]
fn data_that_is_not_a_reference_or_descriptor_is_rejected() {
    assert_rejected("data is a list", &chart(&format!("data:\n  - a\n{BAR}")));
    assert_rejected("data is a number", &chart(&format!("data: 3\n{BAR}")));
}

#[test]
fn string_data_reference_still_validates() {
    let parsed = single_chart(assert_valid("named reference", &chart(&format!("data: sales\n{BAR}"))));
    assert!(matches!(parsed.data, DataRef::Named(ref n) if n == "sales"));
}

#[test]
fn source_component_with_http_provider_and_url() {
    assert_valid(
        "http source",
        "type: source\nversion: 1\nname: sales\nprovider: http\nurl: https://api.example.com/sales\ncache:\n  ttl: 6h\n  autoRefresh: true\n",
    );
    assert_valid(
        "inline source",
        "type: source\nversion: 1\nname: sales\nprovider: inline\nrows:\n  - { month: Jan, revenue: 1 }\n",
    );
}

#[test]
fn source_component_without_provider_or_name_is_rejected() {
    assert_rejected("source without provider", "type: source\nversion: 1\nname: sales\nrows: []\n");
    assert_rejected("source without name", "type: source\nversion: 1\nprovider: inline\nrows: []\n");
}

// --- range marks ---------------------------------------------------------------

#[test]
fn range_mark_rows_without_a_field() {
    let yaml = chart(
        "data: forecast\nvisualize:\n  type: line\n  columns: date\n  rows:\n    - field: visitors\n    - field: forecast\n    - mark: range\n      upper: upper_bound\n      lower: lower_bound\n      color: \"#4285f4\"\n      opacity: 0.15\n",
    );
    assert_valid("range mark rows", &yaml);
}

#[test]
fn range_mark_must_name_both_bounds() {
    let yaml = chart(
        "data: forecast\nvisualize:\n  type: line\n  columns: date\n  rows:\n    - field: visitors\n    - mark: range\n      upper: upper_bound\n",
    );
    assert!(
        !schema_errors(&validator(), &yaml).is_empty(),
        "a range mark with no lower bound draws nothing, so the schema rejects it"
    );
}

#[test]
fn rows_item_without_field_or_range_mark_is_rejected_by_the_schema() {
    let yaml = chart("data: d\nvisualize:\n  type: bar\n  columns: month\n  rows:\n    - label: Revenue\n");
    assert!(!schema_errors(&validator(), &yaml).is_empty());
}

// --- inline Chart.style --------------------------------------------------------

#[test]
fn chart_style_as_a_name() {
    let parsed = single_chart(assert_valid("named style", &chart(&format!("style: brand\ndata: d\n{BAR}"))));
    assert!(matches!(parsed.style, Some(StyleRefOrInline::Named(_))));
}

#[test]
fn chart_style_as_an_inline_object() {
    let yaml = chart(&format!(
        "style:\n  height: 320\n  width: 640\n  colors: [\"#4285f4\", \"#ea4335\"]\n  showDots: false\n  strokeWidth: 2\n  curveType: step\n  fillOpacity: 0.3\n  pageSize: 25\n  grid:\n    y: true\n  fonts:\n    title:\n      weight: 600\n  legend:\n    position: bottom\ndata: d\n{BAR}"
    ));
    let parsed = single_chart(assert_valid("inline style", &yaml));
    match parsed.style {
        Some(StyleRefOrInline::Inline(s)) => {
            assert_eq!(s.height, Some(320.0));
            assert_eq!(s.width, Some(640.0));
            assert_eq!(s.curve_type.as_deref(), Some("step"));
            assert_eq!(s.fill_opacity, Some(0.3));
            assert_eq!(s.page_size, Some(25));
        }
        other => panic!("expected an inline style, got {other:?}"),
    }
}

#[test]
fn chart_style_of_the_wrong_type_is_rejected() {
    assert_rejected("style is a list", &chart(&format!("style:\n  - a\ndata: d\n{BAR}")));
    assert_rejected("style is a number", &chart(&format!("style: 3\ndata: d\n{BAR}")));
    assert_rejected("inline style with a text height", &chart(&format!("style:\n  height: tall\ndata: d\n{BAR}")));
}

#[test]
fn visualize_style_accepts_the_same_keys_as_chart_style() {
    let yaml = chart(
        "data: d\nvisualize:\n  type: line\n  columns: month\n  rows: revenue\n  style:\n    width: 600\n    curveType: linear\n    fillOpacity: 0.2\n",
    );
    assert_valid("visualize.style", &yaml);
}

#[test]
fn config_style_accepts_a_name_or_the_same_inline_style() {
    assert_valid("config with a named style", "type: config\nversion: 1\nstyle: brand\n");
    assert_valid(
        "config with an inline style",
        "type: config\nversion: 1\nstyle:\n  height: 300\n  curveType: step\n  pageSize: 10\n",
    );
}

#[test]
fn config_without_a_style_is_rejected() {
    assert_rejected("config without style", "type: config\nversion: 1\n");
}

// --- chart types -------------------------------------------------------------------

#[test]
fn every_registered_chart_type_validates() {
    for chart_type in ["bar", "line", "area", "pie", "doughnut", "scatter", "bubble", "table", "metric"] {
        let yaml = chart(&format!("data: d\nvisualize:\n  type: {chart_type}\n"));
        assert_valid(chart_type, &yaml);
    }
}

#[test]
fn an_unregistered_chart_type_is_rejected_by_the_schema() {
    let yaml = chart("data: d\nvisualize:\n  type: sankey\n");
    assert!(!schema_errors(&validator(), &yaml).is_empty());
}

// --- other core structures ------------------------------------------------------------

#[test]
fn params_component_with_a_name() {
    assert_valid(
        "named params",
        "type: params\nversion: 1\nname: filters\nparams:\n  - id: region\n    type: select\n    label: Region\n    options: [US, EU]\n    default: US\n",
    );
}

#[test]
fn axis_nice_and_axis_aliases() {
    let yaml = chart(
        "data: d\nvisualize:\n  type: bar\n  columns: month\n  rows: revenue\n  axes:\n    bottom:\n      label: Month\n    rows:\n      min: 0\n      nice: true\n    right:\n      format: \"$,.0f\"\n",
    );
    assert_valid("axis nice and aliases", &yaml);
}

#[test]
fn axis_aliases_are_checked_like_the_axes_they_stand_for() {
    for alias in ["bottom", "columns", "rows"] {
        let bad = chart(&format!(
            "data: d\nvisualize:\n  type: bar\n  columns: month\n  rows: revenue\n  axes:\n    {alias}:\n      min: low\n"
        ));
        assert_rejected(&format!("axes.{alias} with a text min"), &bad);
    }
}

#[test]
fn row_line_style_and_annotation_style() {
    let yaml = chart(
        "data: d\nvisualize:\n  type: line\n  columns: month\n  rows:\n    - field: revenue\n      lineStyle: dashed\n  annotations:\n    - type: line\n      axis: left\n      value: 100\n      style: dotted\n",
    );
    assert_valid("lineStyle and annotation style", &yaml);
}

#[test]
fn version_must_be_the_integer_one() {
    // The language version is 1. The parser reads any integer and does not
    // enforce it, so the pin to 1 is the schema's published contract only.
    assert_rejected("text version", &format!("type: chart\nversion: \"one\"\ndata: d\n{BAR}"));
    let two = format!("type: chart\nversion: 2\ndata: d\n{BAR}");
    assert!(!schema_errors(&validator(), &two).is_empty(), "the language version is 1");
    let fractional = format!("type: chart\nversion: 1.5\ndata: d\n{BAR}");
    assert!(!schema_errors(&validator(), &fractional).is_empty());
}

// --- shapes that must keep validating ---------------------------------------------------

#[test]
fn transform_stages_stay_extensible_for_host_stages() {
    let yaml = chart(&format!(
        "data: d\ntransform:\n  hostStage:\n    anything: [1, 2]\n  aggregate:\n    dimensions: [month]\n    measures:\n      - column: revenue\n        aggregation: sum\n        name: total\n{BAR}"
    ));
    assert_valid("host transform stage", &yaml);
}

fn yaml_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            yaml_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "yaml") {
            out.push(path);
        }
    }
}

/// Split a multi-document YAML file the way `spec::parse` does: on a line that
/// is exactly `---` (trailing whitespace ignored).
fn documents(text: &str) -> Vec<String> {
    let mut docs = vec![String::new()];
    for line in text.lines() {
        if line.trim_end() == "---" {
            docs.push(String::new());
        } else if let Some(last) = docs.last_mut() {
            last.push_str(line);
            last.push('\n');
        }
    }
    docs.into_iter().filter(|d| !d.trim().is_empty()).collect()
}

/// Every chart spec the repository renders in its tests validates against the
/// schema: the `chart:` block of each `tests/charts` spec (except the ones that
/// expect an error) and each document of `fixtures/`.
#[test]
fn every_chart_spec_in_the_repository_validates() {
    let v = validator();
    let mut instances: Vec<(String, serde_json::Value)> = Vec::new();

    let mut files = Vec::new();
    yaml_files(&workspace_root().join("tests/charts"), &mut files);
    files.sort();
    let chart_spec_files = files.len();
    for file in &files {
        let text = fs::read_to_string(file).expect("read spec");
        let value: serde_yaml::Value =
            serde_yaml::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        let expects_error = value
            .get("assertions")
            .and_then(|a| a.get("expect_error"))
            .and_then(serde_yaml::Value::as_bool)
            .unwrap_or(false);
        if expects_error {
            continue;
        }
        let chart = value.get("chart").unwrap_or_else(|| panic!("{}: no `chart` key", file.display()));
        instances.push((file.display().to_string(), serde_json::to_value(chart).expect("to json")));
    }

    let mut fixtures = Vec::new();
    yaml_files(&workspace_root().join("fixtures"), &mut fixtures);
    fixtures.sort();
    for file in &fixtures {
        for doc in documents(&fs::read_to_string(file).expect("read fixture")) {
            let value: serde_yaml::Value =
                serde_yaml::from_str(&doc).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
            instances.push((file.display().to_string(), serde_json::to_value(value).expect("to json")));
        }
    }

    assert!(
        instances.len() > 150 && chart_spec_files > 150,
        "only {} specs were validated, from {chart_spec_files} chart spec files",
        instances.len()
    );
    let failures: Vec<String> = instances
        .iter()
        .filter_map(|(name, json)| {
            let errors: Vec<String> = v.iter_errors(json).map(|e| e.to_string()).collect();
            (!errors.is_empty()).then(|| format!("{name}: {}", errors.join("; ")))
        })
        .collect();
    assert!(failures.is_empty(), "{} specs fail the schema:\n{}", failures.len(), failures.join("\n"));
}
