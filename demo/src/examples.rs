use std::sync::Arc;
use leptos::prelude::*;
use chartml_core::ChartML;
use chartml_leptos::ChartMLChart;

/// The raw examples.md from the JS chartml docs — included at compile time.
const EXAMPLES_MD: &str = include_str!("../examples_source.md");

/// Extract all chartml source blocks from the markdown and register them
/// on the ChartML instance. Call this before wrapping in Arc.
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
            if is_source_yaml(&yaml) || yaml.trim().starts_with("type: params") {
                let _ = chartml.register_component(&yaml);
            }
        }
    }
}

/// A parsed section of the examples page.
enum Block {
    Heading { level: usize, text: String },
    Paragraph(String),
    /// A renderable chart spec (type: chart or array of charts).
    Chart(String),
    /// A non-renderable spec (type: style, config, source, params) — show as code.
    CodeBlock(String),
    HorizontalRule,
}

/// Check if a YAML block is a renderable chart.
fn is_chart_yaml(yaml: &str) -> bool {
    let trimmed = yaml.trim();
    if trimmed.starts_with("type: chart") || trimmed.starts_with("type:chart") {
        return true;
    }
    if trimmed.starts_with("- type: chart") || trimmed.starts_with("-type: chart") {
        return true;
    }
    false
}

/// Check if a YAML block is a source definition.
fn is_source_yaml(yaml: &str) -> bool {
    yaml.trim().starts_with("type: source")
}

/// Check if a chart YAML references a named source (not inline data).
fn uses_named_source(yaml: &str) -> bool {
    for line in yaml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("data:") {
            let value = trimmed["data:".len()..].trim();
            // If the value is a simple name (not a map/object), it's a named reference
            return !value.is_empty() && !value.starts_with('{') && !value.is_empty() && value != "";
        }
    }
    false
}

/// Parse the markdown into blocks.
fn parse_examples(md: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut lines = md.lines().peekable();

    while let Some(line) = lines.next() {
        // Chartml code block
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
                } else {
                    blocks.push(Block::CodeBlock(yaml));
                }
            }
            continue;
        }

        // Skip regular yaml/json code blocks (non-rendered)
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
            // Show yaml code blocks as styled code
            if !code.trim().is_empty() && code.contains("type:") {
                blocks.push(Block::CodeBlock(code));
            }
            continue;
        }

        // Horizontal rule
        if line.trim() == "---" {
            blocks.push(Block::HorizontalRule);
            continue;
        }

        // Headings
        if line.starts_with("### ") {
            blocks.push(Block::Heading {
                level: 3,
                text: line[4..].to_string(),
            });
            continue;
        }
        if line.starts_with("## ") {
            blocks.push(Block::Heading {
                level: 2,
                text: line[3..].to_string(),
            });
            continue;
        }
        if line.starts_with("# ") {
            blocks.push(Block::Heading {
                level: 1,
                text: line[2..].to_string(),
            });
            continue;
        }

        // Paragraph text (skip empty lines, accumulate text)
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with("**Related") {
            let mut text = trimmed.to_string();
            while let Some(&next) = lines.peek() {
                let nt = next.trim();
                if nt.is_empty()
                    || nt.starts_with('#')
                    || nt.starts_with("```")
                    || nt == "---"
                {
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

/// Full examples page — mirrors the JS chartml docs/examples.md layout exactly.
#[component]
pub fn ExamplesPage(chartml: Arc<ChartML>) -> impl IntoView {
    let blocks = parse_examples(EXAMPLES_MD);
    let mut chart_number = 0_usize;

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
                    // Non-renderable spec — show as styled code block
                    view! {
                        <pre class="examples-code"><code>{yaml}</code></pre>
                    }
                    .into_any()
                }
                Block::Chart(yaml) => {
                    chart_number += 1;
                    let num = chart_number;
                    let spec = signal(yaml.clone());
                    let chartml = chartml.clone();
                    view! {
                        <div class="examples-chart">
                            <div class="chart-number">{format!("Chart #{}", num)}</div>
                            <ChartMLChart spec=spec.0 chartml=chartml />
                        </div>
                    }
                    .into_any()
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
