//! CHA-49: a metric card with a `title` and no `label` shows its title exactly
//! once, wherever it is shown.
//!
//! A host that draws the title as an HTML heading above the SVG (the Leptos
//! component) passes `RenderOptions::title_heading`, and the card then holds
//! no copy of the title. Every host that shows the SVG alone (the npm
//! components, markdown plugins, `chartml-render`'s PNG, this crate's golden
//! runner) has no heading, so the card must hold the title itself. These tests
//! drive the real metric renderer through the real pipeline and count the
//! title in the SVG each path produces.

use chartml_chart_metric::MetricRenderer;
use chartml_core::{ChartML, RenderOptions};
use chartml_render::element_to_svg;

const TITLE: &str = "Quarterly Signups";

fn chartml() -> ChartML {
    let mut c = ChartML::new();
    c.register_renderer("metric", MetricRenderer::new());
    c
}

fn yaml(title_line: &str, label_line: &str) -> String {
    format!(
        "type: chart\nversion: 1\n{title_line}data:\n  provider: inline\n  rows:\n    - {{ new_signups: 4827 }}\nvisualize:\n  type: metric\n  value: new_signups\n{label_line}"
    )
}

fn count(svg: &str, needle: &str) -> usize {
    svg.matches(&format!(">{needle}<")).count()
}

async fn prepared_svg(yaml: &str, title_heading: bool) -> String {
    let chartml = chartml();
    let opts = RenderOptions { title_heading, ..RenderOptions::default() };
    let fetched = chartml.fetch(yaml, &opts).await.expect("fetch");
    let prepared = chartml.transform(fetched, &opts).await.expect("transform");
    chartml.render_prepared_to_svg(&prepared, &opts).expect("render")
}

#[tokio::test]
async fn title_only_metric_with_no_host_heading_shows_the_title_once_on_every_heading_less_path() {
    let spec = yaml(&format!("title: \"{TITLE}\"\n"), "");

    // `chartml-render` (PNG and SVG), the test runner and `renderToElement`.
    let element = chartml().render_from_yaml(&spec).expect("render_from_yaml");
    let svg = element_to_svg(&element, 800.0, 150.0);
    assert_eq!(count(&svg, TITLE), 1, "{svg}");
    assert_eq!(count(&svg, "new_signups"), 0, "{svg}");

    // `renderToSvg` / `renderToSvgAsync` (npm components, markdown plugins).
    let svg = prepared_svg(&spec, false).await;
    assert_eq!(count(&svg, TITLE), 1, "{svg}");
    assert_eq!(count(&svg, "new_signups"), 0, "{svg}");
}

#[tokio::test]
async fn title_only_metric_under_a_host_heading_holds_no_copy_of_the_title() {
    let svg = prepared_svg(&yaml(&format!("title: \"{TITLE}\"\n"), ""), true).await;
    assert_eq!(count(&svg, TITLE), 0, "{svg}");
    // No field-name fallback in the card either: the heading names it.
    assert_eq!(count(&svg, "new_signups"), 0, "{svg}");
}

#[tokio::test]
async fn label_wins_and_the_field_name_stays_only_for_a_card_with_neither() {
    for heading in [false, true] {
        let svg = prepared_svg(&yaml(&format!("title: \"{TITLE}\"\n"), "  label: \"Signups\"\n"), heading).await;
        assert_eq!(count(&svg, "Signups"), 1, "{svg}");
        assert_eq!(count(&svg, TITLE), 0, "{svg}");

        let svg = prepared_svg(&yaml("", ""), heading).await;
        assert_eq!(count(&svg, "new_signups"), 1, "{svg}");
    }
}
