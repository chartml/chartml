use std::sync::Arc;
use leptos::prelude::*;
use chartml_core::ChartML;
use chartml_core::params::ParamValues;
use chartml_core::spec::ParamsSpec;
use chartml_leptos::{ChartMLChart, ParamsControls};

/// The raw examples.md from the JS chartml docs — included at compile time.
const EXAMPLES_MD: &str = include_str!("../examples_source.md");

/// Extract all chartml source and params blocks from the markdown and register them.
pub fn register_page_sources(chartml: &mut ChartML) {
    let mut lines = EXAMPLES_MD.lines().peekable();
    while let Some(line) = lines.next() {
        if line.trim_start().starts_with("```chartml") {
            let mut yaml = String::new();
            for inner in lines.by_ref() {
                if inner.trim_start().starts_with("```") {
                    break;
                }
                if !yaml.is_empty() {
                    yaml.push('\n');
                }
                yaml.push_str(inner);
            }
            let trimmed = yaml.trim();
            if trimmed.starts_with("type: source") || trimmed.starts_with("type: params") {
                let _ = chartml.register_component(&yaml);
            }
        }
    }
}

/// A parsed section of the examples page.
enum Block {
    Heading { level: usize, text: String },
    Paragraph(String),
    Chart(String),
    Params(String),
    CodeBlock(String),
    HorizontalRule,
}

fn is_chart_yaml(yaml: &str) -> bool {
    let t = yaml.trim();
    t.starts_with("type: chart") || t.starts_with("- type: chart")
}

fn is_params_yaml(yaml: &str) -> bool {
    yaml.trim().starts_with("type: params")
}

/// Returns true if ALL charts in this YAML block use `visualize.type: metric`.
/// Metric charts render as HTML (no SVG), so they don't appear in JS SVG indices.
/// We skip numbering these so Rust chart #N matches JS SVG #N.
fn is_metric_only_yaml(yaml: &str) -> bool {
    let has_metric = yaml.contains("type: metric");
    let has_svg_type = yaml.contains("type: bar")
        || yaml.contains("type: line")
        || yaml.contains("type: area")
        || yaml.contains("type: pie")
        || yaml.contains("type: scatter")
        || yaml.contains("type: bubble")
        || yaml.contains("type: combo");
    has_metric && !has_svg_type
}

fn parse_examples(md: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut lines = md.lines().peekable();

    while let Some(line) = lines.next() {
        if line.trim_start().starts_with("```chartml") {
            let mut yaml = String::new();
            for inner in lines.by_ref() {
                if inner.trim_start().starts_with("```") {
                    break;
                }
                if !yaml.is_empty() {
                    yaml.push('\n');
                }
                yaml.push_str(inner);
            }
            if !yaml.trim().is_empty() {
                if is_chart_yaml(&yaml) {
                    blocks.push(Block::Chart(yaml));
                } else if is_params_yaml(&yaml) {
                    blocks.push(Block::Params(yaml));
                } else {
                    blocks.push(Block::CodeBlock(yaml));
                }
            }
            continue;
        }

        if line.trim_start().starts_with("```") {
            let mut code = String::new();
            for inner in lines.by_ref() {
                if inner.trim_start().starts_with("```") {
                    break;
                }
                if !code.is_empty() {
                    code.push('\n');
                }
                code.push_str(inner);
            }
            if !code.trim().is_empty() && code.contains("type:") {
                blocks.push(Block::CodeBlock(code));
            }
            continue;
        }

        if line.trim() == "---" {
            blocks.push(Block::HorizontalRule);
            continue;
        }

        if line.starts_with("### ") {
            blocks.push(Block::Heading { level: 3, text: line[4..].to_string() });
            continue;
        }
        if line.starts_with("## ") {
            blocks.push(Block::Heading { level: 2, text: line[3..].to_string() });
            continue;
        }
        if line.starts_with("# ") {
            blocks.push(Block::Heading { level: 1, text: line[2..].to_string() });
            continue;
        }

        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with("**Related") {
            let mut text = trimmed.to_string();
            while let Some(&next) = lines.peek() {
                let nt = next.trim();
                if nt.is_empty() || nt.starts_with('#') || nt.starts_with("```") || nt == "---" {
                    break;
                }
                lines.next();
                text.push(' ');
                text.push_str(nt);
            }
            blocks.push(Block::Paragraph(text));
        }
    }

    blocks
}

/// Parse a params YAML string into ParamsSpec for the UI controls.
fn parse_params_spec(yaml: &str) -> Option<ParamsSpec> {
    serde_yaml::from_str::<ParamsSpec>(yaml).ok()
}

/// Full examples page — mirrors the JS chartml docs/examples.md layout exactly.
/// Params blocks render as interactive controls that write to a shared signal.
/// Chart blocks read from the same signal so they re-render when params change.
#[component]
pub fn ExamplesPage(chartml: Arc<ChartML>) -> impl IntoView {
    let blocks = parse_examples(EXAMPLES_MD);
    let mut chart_number = 0_usize;

    // Shared reactive param values — params UI writes, charts read
    let param_values = RwSignal::new(ParamValues::new());

    // Initialize with defaults from the ChartML instance
    // (registered by register_page_sources in app.rs)
    // No need to pre-populate — the ChartML instance already has defaults
    // and render_from_yaml_with_params merges them

    let elements: Vec<AnyView> = blocks
        .into_iter()
        .map(|block| {
            match block {
                Block::Heading { level: 1, text } => {
                    view! { <h1 class="examples-h1">{text}</h1> }.into_any()
                }
                Block::Heading { level: 2, text } => {
                    view! { <h2 class="examples-h2">{text}</h2> }.into_any()
                }
                Block::Heading { level: _, text } => {
                    view! { <h3 class="examples-h3">{text}</h3> }.into_any()
                }
                Block::Paragraph(text) => {
                    view! { <p class="examples-p">{text}</p> }.into_any()
                }
                Block::HorizontalRule => {
                    view! { <hr class="examples-hr" /> }.into_any()
                }
                Block::CodeBlock(yaml) => {
                    view! {
                        <pre class="examples-code"><code>{yaml}</code></pre>
                    }.into_any()
                }
                Block::Params(yaml) => {
                    if let Some(spec) = parse_params_spec(&yaml) {
                        let block_name = spec.name.clone().unwrap_or_default();
                        let params = spec.params.clone();
                        view! {
                            <div class="examples-chart examples-params">
                                <ParamsControls
                                    params=params
                                    param_values=param_values
                                    block_name=block_name
                                />
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <pre class="examples-code"><code>{yaml}</code></pre>
                        }.into_any()
                    }
                }
                Block::Chart(yaml) => {
                    let is_metric = is_metric_only_yaml(&yaml);
                    let num = if is_metric {
                        None
                    } else {
                        chart_number += 1;
                        Some(chart_number)
                    };
                    let spec = signal(yaml.clone());
                    let chartml = chartml.clone();
                    view! {
                        <div class="examples-chart">
                            {num.map(|n| view! {
                                <div class="chart-number">{format!("Chart #{}", n)}</div>
                            })}
                            <ChartMLChart
                                spec=spec.0
                                chartml=chartml
                                param_values=param_values
                            />
                        </div>
                    }.into_any()
                }
            }
        })
        .collect();

    view! {
        <div class="examples-page">
            {elements}
        </div>
    }
}
