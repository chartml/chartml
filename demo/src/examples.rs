use std::sync::Arc;
use leptos::prelude::*;
use chartml_core::ChartML;
use chartml_leptos::ChartMLChart;

/// The raw examples.md from the JS chartml docs — included at compile time.
const EXAMPLES_MD: &str = include_str!("../examples_source.md");

/// A parsed section of the examples page.
enum Block {
    Heading { level: usize, text: String },
    Paragraph(String),
    ChartML(String),
    HorizontalRule,
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
                blocks.push(Block::ChartML(yaml));
            }
            continue;
        }

        // Skip regular yaml/json code blocks (non-rendered)
        if line.trim_start().starts_with("```") {
            for inner in lines.by_ref() {
                if inner.trim_start().starts_with("```") {
                    break;
                }
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
            // Merge consecutive text lines
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
/// Parses the markdown, renders headings/descriptions as HTML, and each
/// ```chartml block as a live chart via the Rust WASM renderer.
#[component]
pub fn ExamplesPage(chartml: Arc<ChartML>) -> impl IntoView {
    let blocks = parse_examples(EXAMPLES_MD);

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
                Block::ChartML(yaml) => {
                    let spec = signal(yaml.clone());
                    let chartml = chartml.clone();
                    view! {
                        <div class="examples-chart">
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
