//! CHA-48: the stylesheet the `ChartMLChart` component injects maps chart
//! chrome to theme CSS properties (`--chartml-text-secondary`,
//! `--chartml-grid`). Author CSS beats SVG presentation attributes, so those
//! rules must skip elements whose colour the chart chose (marked
//! `explicit-paint` by the renderers), and the default grid opacity must not
//! replace a style's `grid.opacity` attribute.

const CSS: &str = include_str!("../style/chartml.css");

/// `(selector, declarations)` for every rule in `css`, comments removed.
fn rules(css: &str) -> Vec<(String, String)> {
    let mut text = String::new();
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        text.push_str(&rest[..start]);
        rest = rest[start..].find("*/").map_or("", |end| &rest[start + end + 2..]);
    }
    text.push_str(rest);
    text.split('}')
        .filter_map(|block| block.split_once('{'))
        .map(|(sel, body)| (sel.trim().to_string(), body.trim().to_string()))
        .collect()
}

#[test]
fn theme_colour_rules_skip_explicitly_painted_elements() {
    let mut checked = 0;
    for (selector, body) in rules(CSS) {
        for part in selector.split(',').map(str::trim) {
            for class in [".tick-label", ".axis-label", ".data-label", ".grid-line"] {
                let targets = part.split(|c: char| c.is_whitespace() || c == ':').any(|t| t == class);
                let paints = body.contains("fill:") || body.contains("stroke:");
                if targets && paints && body.contains("var(--chartml-") {
                    checked += 1;
                    assert!(
                        part.contains(":not(.explicit-paint)"),
                        "`{part} {{ {body} }}` would override a chart's own colour"
                    );
                }
            }
        }
    }
    assert_eq!(checked, 4, "expected the four theme colour rules");
}

#[test]
fn default_grid_opacity_keeps_a_styles_opacity() {
    let opacity_rules: Vec<_> = rules(CSS)
        .into_iter()
        .filter(|(sel, body)| sel.contains(".grid-line") && body.contains("opacity:"))
        .collect();
    assert!(!opacity_rules.is_empty());
    for (sel, body) in opacity_rules {
        assert!(sel.contains(":not([opacity])"), "`{sel} {{ {body} }}` would override grid.opacity");
    }
}
