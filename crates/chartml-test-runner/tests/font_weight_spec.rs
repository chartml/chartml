//! CHA-47: `fonts.*.weight` is a number, as `chartml_schema.json` and
//! `docs/docs/spec.md` document, in style components and inline chart styles.
//!
//! These tests drive the real parser (`spec::parse` / `register_component`) and
//! the real cartesian renderer, and read the published schema and the docs
//! example straight from the repository so the three cannot drift apart.

use chartml_chart_cartesian::CartesianRenderer;
use chartml_core::spec::{self, ChartMLSpec, Component, FontWeight};
use chartml_core::ChartML;
use std::fs;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn chartml() -> ChartML {
    let mut c = ChartML::new();
    c.register_renderer("bar", CartesianRenderer::new());
    c
}

fn style_yaml(weight: &str) -> String {
    format!(
        r##"
type: style
version: 1
name: weighted
fonts:
  title:
    family: "Inter, sans-serif"
    size: 18
    weight: {weight}
  dataLabel:
    weight: 500
"##
    )
}

fn chart_yaml(weight: &str) -> String {
    format!(
        r##"
type: chart
version: 1
title: "Monthly Revenue"
data:
  provider: inline
  rows:
    - month: "Jan"
      revenue: 125000
    - month: "Feb"
      revenue: 138000
visualize:
  type: bar
  columns: month
  rows: revenue
  style:
    fonts:
      title:
        weight: {weight}
"##
    )
}

fn single(spec: ChartMLSpec) -> Component {
    match spec {
        ChartMLSpec::Single(c) => *c,
        ChartMLSpec::Array(_) => panic!("expected a single component"),
    }
}

#[test]
fn style_component_accepts_numeric_weight() {
    let Component::Style(style) = single(spec::parse(&style_yaml("600")).expect("numeric weight parses")) else {
        panic!("expected a style component");
    };
    let fonts = style.fonts.expect("fonts");
    assert_eq!(fonts.title.expect("title").weight, Some(FontWeight::Number(600.0)));
    assert_eq!(fonts.data_label.expect("dataLabel").weight, Some(FontWeight::Number(500.0)));
}

#[test]
fn style_component_registers_with_numeric_weight() {
    chartml()
        .register_component(&style_yaml("600"))
        .expect("registerComponent accepts a numeric weight");
}

/// Compatibility: Kyomi stores specs and string weights parsed before CHA-47,
/// so they stay accepted (undocumented; the schema documents numbers only).
#[test]
fn string_weights_stay_accepted_unchanged() {
    for raw in ["\"bold\"", "\"600\"", "bold"] {
        let Component::Style(style) = single(spec::parse(&style_yaml(raw)).unwrap_or_else(|e| panic!("{raw} must parse: {e}"))) else {
            panic!("expected a style component");
        };
        let weight = style.fonts.expect("fonts").title.expect("title").weight;
        let expected = raw.trim_matches('"');
        assert_eq!(weight, Some(FontWeight::Text(expected.to_string())));
    }
}

#[test]
fn inline_chart_style_with_numeric_weight_parses_and_renders() {
    chartml()
        .render_from_yaml(&chart_yaml("600"))
        .expect("chart with numeric fonts.title.weight renders");
}

#[test]
fn inline_chart_style_with_string_weight_still_renders() {
    for raw in ["\"bold\"", "\"600\""] {
        chartml()
            .render_from_yaml(&chart_yaml(raw))
            .unwrap_or_else(|e| panic!("string weight {raw} must still render: {e}"));
    }
}

#[test]
fn examples_md_style_block_parses() {
    let md = fs::read_to_string(workspace_root().join("docs/docs/examples.md")).expect("read examples.md");
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;
    for line in md.lines() {
        match (&mut current, line.trim_end()) {
            (None, "```yaml" | "```chartml") => current = Some(String::new()),
            (Some(buf), "```") => {
                blocks.push(std::mem::take(buf));
                current = None;
            }
            (Some(buf), l) => {
                buf.push_str(l);
                buf.push('\n');
            }
            _ => {}
        }
    }
    let styles: Vec<&String> = blocks.iter().filter(|b| b.contains("type: style")).collect();
    assert!(!styles.is_empty(), "examples.md has no style block");
    for block in styles {
        chartml()
            .register_component(block)
            .unwrap_or_else(|e| panic!("examples.md style block failed: {e}\n{block}"));
    }
}

/// Every `weight` property in the published schema is a number (the documented
/// form), and the parser accepts a number there. Strings stay accepted for
/// compatibility but are deliberately not in the schema.
#[test]
fn schema_and_parser_agree_on_weight() {
    let schema: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(workspace_root().join("docs/docs/public/chartml_schema.json")).expect("read schema"),
    )
    .expect("schema is JSON");

    fn collect(v: &serde_json::Value, out: &mut Vec<serde_json::Value>) {
        match v {
            serde_json::Value::Object(m) => {
                for (k, child) in m {
                    if k == "weight" {
                        out.push(child.clone());
                    }
                    collect(child, out);
                }
            }
            serde_json::Value::Array(a) => a.iter().for_each(|c| collect(c, out)),
            _ => {}
        }
    }
    let mut weights = Vec::new();
    collect(&schema, &mut weights);
    // title, axis and dataLabel, in both the Style component and inline styles.
    assert!(weights.len() == 6, "expected the schema's font weight properties, found {}", weights.len());
    for w in &weights {
        assert_eq!(w["type"], "number", "schema weight must be a number: {w}");
    }

    assert!(spec::parse(&style_yaml("600")).is_ok());
}
