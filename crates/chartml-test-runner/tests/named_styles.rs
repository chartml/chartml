//! CHA-48: a registered `type: style` component, referenced by a chart's
//! `style:` or made the default by a `type: config`, changes how the chart
//! renders, with the precedence the spec states (config < chart `style:` <
//! `visualize.style`, deep-merged field by field).
//!
//! Every test drives the real renderers and reads the rendered SVG (or the
//! element tree for HTML-drawn metric cards), not an internal model.

use chartml_chart_cartesian::CartesianRenderer;
use chartml_chart_metric::MetricRenderer;
use chartml_chart_pie::PieRenderer;
use chartml_chart_scatter::ScatterRenderer;
use chartml_chart_table::TableRenderer;
use chartml_core::{ChartError, ChartML, RenderOptions};
use chartml_render::element_to_svg;

fn chartml() -> ChartML {
    let mut c = ChartML::new();
    for t in ["bar", "line", "area"] {
        c.register_renderer(t, CartesianRenderer::new());
    }
    for t in ["pie", "doughnut"] {
        c.register_renderer(t, PieRenderer::new());
    }
    for t in ["scatter", "bubble"] {
        c.register_renderer(t, ScatterRenderer::new());
    }
    c.register_renderer("metric", MetricRenderer::new());
    c.register_renderer("table", TableRenderer::new());
    c
}

const ROWS: &str = "  provider: inline\n  rows:\n    \
    - { month: Jan, region: East, revenue: 10, budget: 1, sales: 5 }\n    \
    - { month: Jan, region: West, revenue: 14, budget: 2, sales: 7 }\n    \
    - { month: Feb, region: East, revenue: 20, budget: 3, sales: 9 }\n    \
    - { month: Feb, region: West, revenue: 18, budget: 4, sales: 4 }\n";

/// A chart. `top` goes at the chart's top level (e.g. `style: corporate`),
/// `visualize` replaces the default bar visualize block.
fn chart(top: &str, visualize: &str) -> String {
    format!("type: chart\nversion: 1\ntitle: Revenue\n{top}\ndata:\n{ROWS}visualize:\n{visualize}")
}

const BAR: &str = "  type: bar\n  columns: month\n  rows: revenue\n";
const BAR_BY_REGION: &str =
    "  type: bar\n  columns: month\n  rows: revenue\n  marks:\n    color: region\n";
const BAR_WITH_LABELS: &str = "  type: bar\n  columns: month\n  rows:\n    field: revenue\n    dataLabels:\n      show: true\n";
const LINE: &str = "  type: line\n  columns: month\n  rows: revenue\n";
const LINE_BY_REGION: &str = "  type: line\n  columns: month\n  rows: revenue\n  marks:\n    color: region\n";

const CORPORATE: &str = r##"
type: style
version: 1
name: corporate
colors: ["#123456", "#abcdef"]
grid:
  x: true
  y: true
  color: "#00ff00"
  opacity: 0.25
  dashArray: "3,3"
height: 333
showDots: false
strokeWidth: 5
fonts:
  title:
    family: "Georgia, serif"
    size: 21
    weight: 800
    color: "#112233"
  axis:
    family: "Courier New, monospace"
    size: 13
    weight: 600
    color: "#445566"
  dataLabel:
    family: "Verdana, sans-serif"
    size: 15
    weight: bold
"##;

const DARK: &str = r##"
type: style
version: 1
name: dark
colors: ["#ff0000"]
height: 222
"##;

fn render(c: &ChartML, yaml: &str) -> Result<String, ChartError> {
    let element = c.render_from_yaml(yaml)?;
    Ok(element_to_svg(&element, 800.0, 400.0))
}

fn render_ok(c: &ChartML, yaml: &str) -> String {
    render(c, yaml).unwrap_or_else(|e| panic!("render failed: {e}\n{yaml}"))
}

fn with(components: &[&str]) -> ChartML {
    let mut c = chartml();
    for yaml in components {
        c.register_component(yaml).unwrap_or_else(|e| panic!("register failed: {e}\n{yaml}"));
    }
    c
}

/// Every opening tag in `svg` whose `class` attribute has the class `class`.
fn tags<'a>(svg: &'a str, class: &str) -> Vec<&'a str> {
    svg.split('<')
        .filter(|t| {
            attr(t, "class").is_some_and(|c| c.split_whitespace().any(|c| c == class))
        })
        .collect()
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let start = tag.find(&key)? + key.len();
    let end = tag[start..].find('"')?;
    Some(&tag[start..start + end])
}

fn svg_height(svg: &str) -> f64 {
    let root = svg.split('<').find(|t| t.starts_with("svg")).expect("svg root");
    attr(root, "height").expect("height").parse().expect("number")
}

fn bar_fills(svg: &str) -> Vec<String> {
    tags(svg, "bar").into_iter().filter_map(|t| attr(t, "fill").map(str::to_string)).collect()
}

fn assert_invalid(result: Result<String, ChartError>, needles: &[&str]) {
    match result {
        Err(ChartError::InvalidSpec(msg)) => {
            for n in needles {
                assert!(msg.contains(n), "error should contain {n:?}: {msg}");
            }
        }
        Err(other) => panic!("wrong error: {other}"),
        Ok(svg) => panic!("rendered instead of failing:\n{svg}"),
    }
}

// --- Acceptance 1: a chart naming a style renders with its settings ---------

#[test]
fn named_style_sets_colours_height_and_grid() {
    let c = with(&[CORPORATE]);
    let svg = render_ok(&c, &chart("style: corporate", BAR));

    assert_eq!(svg_height(&svg), 333.0, "height from the style");
    let fills = bar_fills(&svg);
    assert!(!fills.is_empty());
    assert!(fills.iter().all(|f| f == "#123456"), "colors[0] from the style: {fills:?}");

    let grid = tags(&svg, "grid-line");
    assert!(!grid.is_empty(), "grid lines drawn");
    assert!(!tags(&svg, "grid-line-x").is_empty(), "grid.x: true draws vertical grid lines");
    for g in &grid {
        assert_eq!(attr(g, "stroke"), Some("#00ff00"), "grid.color: {g}");
        assert_eq!(attr(g, "opacity"), Some("0.25"), "grid.opacity: {g}");
        assert_eq!(attr(g, "stroke-dasharray"), Some("3,3"), "grid.dashArray: {g}");
    }
}

#[test]
fn named_style_without_it_renders_defaults() {
    // The same chart without the style reference: none of the style's values.
    let c = with(&[CORPORATE]);
    let svg = render_ok(&c, &chart("", BAR));
    assert_eq!(svg_height(&svg), 400.0);
    assert!(!svg.contains("#123456"));
    assert!(!svg.contains("#00ff00"));
    assert!(tags(&svg, "grid-line-x").is_empty());
}

#[test]
fn named_style_line_settings_show_dots_and_stroke_width() {
    let c = with(&[CORPORATE]);
    let styled = render_ok(&c, &chart("style: corporate", LINE));
    assert!(tags(&styled, "dot-marker").is_empty(), "showDots: false draws no dots");
    let paths = tags(&styled, "series-line");
    assert!(!paths.is_empty());
    for p in paths {
        assert_eq!(attr(p, "stroke-width"), Some("5"), "strokeWidth: {p}");
    }

    let plain = render_ok(&c, &chart("", LINE));
    assert!(!tags(&plain, "dot-marker").is_empty(), "dots are drawn by default");
    for p in tags(&plain, "series-line") {
        assert_eq!(attr(p, "stroke-width"), Some("2"));
    }
}

#[test]
fn named_style_axis_font_reaches_axis_and_tick_labels() {
    let c = with(&[CORPORATE]);
    let svg = render_ok(&c, &chart("style: corporate", BAR));
    let labels = tags(&svg, "tick-label");
    assert!(!labels.is_empty());
    for l in labels {
        assert_eq!(attr(l, "font-family"), Some("Courier New, monospace"), "axis family: {l}");
        assert_eq!(attr(l, "font-size"), Some("13px"), "axis size: {l}");
        assert_eq!(attr(l, "font-weight"), Some("600"), "axis weight: {l}");
        assert_eq!(attr(l, "fill"), Some("#445566"), "axis colour: {l}");
    }
}

#[test]
fn named_style_data_label_font_reaches_data_labels() {
    let c = with(&[CORPORATE]);
    let svg = render_ok(&c, &chart("style: corporate", BAR_WITH_LABELS));
    let labels = tags(&svg, "data-label");
    assert!(!labels.is_empty());
    for l in labels {
        assert_eq!(attr(l, "font-family"), Some("Verdana, sans-serif"), "{l}");
        assert_eq!(attr(l, "font-size"), Some("15px"), "{l}");
        assert_eq!(attr(l, "font-weight"), Some("700"), "bold is 700: {l}");
    }

    // A field's own dataLabels.fontSize / color still win over the style.
    let own = "  type: bar\n  columns: month\n  rows:\n    field: revenue\n    dataLabels:\n      show: true\n      fontSize: 9\n      color: \"#999999\"\n";
    let svg = render_ok(&c, &chart("style: corporate", own));
    for l in tags(&svg, "data-label") {
        assert_eq!(attr(l, "font-size"), Some("9px"), "{l}");
        assert_eq!(attr(l, "fill"), Some("#999999"), "{l}");
        assert_eq!(attr(l, "font-family"), Some("Verdana, sans-serif"), "{l}");
    }
}

#[test]
fn data_label_font_colour_applies_when_set() {
    let style = "type: style\nversion: 1\nname: dl\nfonts:\n  dataLabel:\n    color: \"#0000ff\"\n";
    let c = with(&[style]);
    let svg = render_ok(&c, &chart("style: dl", BAR_WITH_LABELS));
    let labels = tags(&svg, "data-label");
    assert!(!labels.is_empty());
    for l in labels {
        assert_eq!(attr(l, "fill"), Some("#0000ff"), "{l}");
    }
}

#[test]
fn named_style_title_font_reaches_the_theme_and_the_metric_card_title() {
    let c = with(&[CORPORATE]);

    // Hosts that draw the title read it from the chart's resolved theme.
    let resolved = c
        .resolve_chart_from_yaml(&chart("style: corporate", BAR), None)
        .expect("resolves");
    let theme = c.chart_theme(&resolved).expect("theme");
    assert_eq!(theme.title_font_family, "Georgia, serif");
    assert_eq!(theme.title_font_size, 21.0);
    assert_eq!(theme.title_font_weight, 800);
    assert_eq!(theme.title_color.as_deref(), Some("#112233"));

    // A metric card shown without a host heading carries its title inside the
    // card, in the title font.
    let metric = "  type: metric\n  value: revenue\n";
    let element = c.render_from_yaml(&chart("style: corporate", metric)).expect("metric renders");
    let json = serde_json::to_string(&element).expect("serialize");
    let label = json.split("chartml-metric-label").nth(1).expect("metric label");
    assert!(label.contains("Georgia, serif"), "title family on the card title: {label}");
    assert!(label.contains("\"font-weight\":\"800\""), "{label}");
    assert!(label.contains("\"color\":\"#112233\""), "{label}");
}

// --- Acceptance 2: a config makes a style the default ------------------------

#[test]
fn config_names_the_default_style_for_charts_without_one() {
    let c = with(&[CORPORATE, "type: config\nversion: 1\nstyle: corporate\n"]);
    let svg = render_ok(&c, &chart("", BAR));
    assert_eq!(svg_height(&svg), 333.0);
    assert!(bar_fills(&svg).iter().all(|f| f == "#123456"));
}

#[test]
fn config_may_be_registered_before_its_style() {
    let c = with(&["type: config\nversion: 1\nstyle: corporate\n", CORPORATE]);
    assert_eq!(svg_height(&render_ok(&c, &chart("", BAR))), 333.0);
}

#[test]
fn config_with_inline_style() {
    let config = "type: config\nversion: 1\nstyle:\n  colors: [\"#1a237e\"]\n  height: 500\n";
    let c = with(&[config]);
    let svg = render_ok(&c, &chart("", BAR));
    assert_eq!(svg_height(&svg), 500.0);
    assert!(bar_fills(&svg).iter().all(|f| f == "#1a237e"));
}

#[test]
fn a_later_style_replaces_the_earlier_one_of_that_name() {
    let replaced = "type: style\nversion: 1\nname: corporate\nheight: 250\n";
    let c = with(&[CORPORATE, replaced]);
    let svg = render_ok(&c, &chart("style: corporate", BAR));
    assert_eq!(svg_height(&svg), 250.0);
    assert!(!svg.contains("#123456"), "the replaced style's colours are gone");

}

#[test]
fn configs_cascade_in_registration_order_and_the_document_config_merges_last() {
    // spec.md Resolution Order: workspace (2) -> user (3) -> dashboard (4)
    // configs deep-merge, each overriding only what it sets.
    let c = with(&[
        CORPORATE,
        DARK,
        "type: config\nversion: 1\nstyle: corporate\n",
        "type: config\nversion: 1\nstyle: dark\n",
    ]);
    let svg = render_ok(&c, &chart("", BAR));
    assert_eq!(svg_height(&svg), 222.0, "the later config's height");
    assert!(bar_fills(&svg).iter().all(|f| f == "#ff0000"), "the later config's colours");
    assert!(
        tags(&svg, "grid-line").iter().all(|g| attr(g, "stroke") == Some("#00ff00")),
        "fields the later config does not set stay from the earlier one"
    );

    // A dashboard document's config that sets only height keeps the
    // registered palette and fonts.
    let c = with(&[CORPORATE, "type: config\nversion: 1\nstyle: corporate\n"]);
    let doc = format!("type: config\nversion: 1\nstyle:\n  height: 250\n---\n{}", chart("", BAR));
    let svg = render_ok(&c, &doc);
    assert_eq!(svg_height(&svg), 250.0);
    assert!(bar_fills(&svg).iter().all(|f| f == "#123456"), "registered palette kept");
    for l in tags(&svg, "tick-label") {
        assert_eq!(attr(l, "font-family"), Some("Courier New, monospace"), "registered fonts kept: {l}");
    }
}

// --- Acceptance 3: precedence and unknown names -----------------------------

#[test]
fn chart_style_beats_config_and_visualize_style_beats_both() {
    let c = with(&[CORPORATE, DARK, "type: config\nversion: 1\nstyle: corporate\n"]);

    // Chart `style:` over the config default.
    let svg = render_ok(&c, &chart("style: dark", BAR));
    assert_eq!(svg_height(&svg), 222.0);
    assert!(bar_fills(&svg).iter().all(|f| f == "#ff0000"));
    // Fields dark does not set still come from the config's style (deep merge).
    assert!(tags(&svg, "grid-line").iter().all(|g| attr(g, "stroke") == Some("#00ff00")));

    // visualize.style over both.
    let vis = format!("{BAR}  style:\n    height: 600\n    colors: [\"#00aa00\"]\n");
    let svg = render_ok(&c, &chart("style: dark", &vis));
    assert_eq!(svg_height(&svg), 600.0);
    assert!(bar_fills(&svg).iter().all(|f| f == "#00aa00"));
}

#[test]
fn inline_overrides_deep_merge_grid_and_fonts() {
    let c = with(&[CORPORATE]);
    // spec.md "Deep Merge Behavior": override just grid.color; x, y, opacity
    // and dashArray stay from the style. Override just the axis font size.
    let vis = format!(
        "{BAR}  style:\n    grid:\n      color: \"#ff0000\"\n    fonts:\n      axis:\n        size: 17\n"
    );
    let svg = render_ok(&c, &chart("style: corporate", &vis));
    assert!(!tags(&svg, "grid-line-x").is_empty(), "grid.x from the style");
    for g in tags(&svg, "grid-line") {
        assert_eq!(attr(g, "stroke"), Some("#ff0000"), "{g}");
        assert_eq!(attr(g, "opacity"), Some("0.25"), "{g}");
        assert_eq!(attr(g, "stroke-dasharray"), Some("3,3"), "{g}");
    }
    for l in tags(&svg, "tick-label") {
        assert_eq!(attr(l, "font-size"), Some("17px"), "{l}");
        assert_eq!(attr(l, "font-family"), Some("Courier New, monospace"), "{l}");
    }
    assert!(bar_fills(&svg).iter().all(|f| f == "#123456"), "colours from the style");
}

#[test]
fn chart_level_inline_style_sits_between_config_and_visualize_style() {
    let c = with(&[CORPORATE, "type: config\nversion: 1\nstyle: corporate\n"]);
    let top = "style:\n  height: 280\n  colors: [\"#777777\"]";
    let svg = render_ok(&c, &chart(top, BAR));
    assert_eq!(svg_height(&svg), 280.0);
    assert!(bar_fills(&svg).iter().all(|f| f == "#777777"));

    let vis = format!("{BAR}  style:\n    height: 290\n");
    assert_eq!(svg_height(&render_ok(&c, &chart(top, &vis))), 290.0);
}

#[test]
fn a_style_height_beats_the_host_container_height() {
    let c = with(&[CORPORATE]);
    let element = c
        .render_from_yaml_with_size(&chart("style: corporate", BAR), Some(640.0), Some(480.0))
        .expect("renders");
    assert_eq!(svg_height(&element_to_svg(&element, 640.0, 480.0)), 333.0);
}

#[test]
fn unknown_style_name_on_a_chart_fails_naming_it() {
    let c = chartml();
    assert_invalid(
        render(&c, &chart("style: missing_theme", BAR)),
        &["style 'missing_theme' is not registered", "the chart's `style`"],
    );
}

#[test]
fn unknown_style_name_in_the_config_fails_naming_it() {
    let c = with(&["type: config\nversion: 1\nstyle: missing_theme\n"]);
    assert_invalid(
        render(&c, &chart("", BAR)),
        &["style 'missing_theme' is not registered", "a registered config"],
    );
}

// --- Same-document components, every render path ----------------------------

#[test]
fn styles_and_config_in_the_same_document_apply_to_its_charts() {
    let c = chartml();
    let doc = format!(
        "{}\n---\ntype: config\nversion: 1\nstyle: corporate\n---\n{}",
        CORPORATE.trim(),
        chart("", BAR)
    );
    let svg = render_ok(&c, &doc);
    assert_eq!(svg_height(&svg), 333.0);
    assert!(bar_fills(&svg).iter().all(|f| f == "#123456"));
}

#[test]
fn async_pipeline_applies_named_styles() {
    let c = with(&[CORPORATE]);
    let yaml = chart("style: corporate", BAR);
    let rt = tokio::runtime::Builder::new_current_thread().build().expect("runtime");
    let svg = rt
        .block_on(c.render_to_svg_async(&yaml, &RenderOptions::default()))
        .expect("async render");
    assert_eq!(svg_height(&svg), 333.0);
    assert!(bar_fills(&svg).iter().all(|f| f == "#123456"));

    let err = rt
        .block_on(c.render_to_svg_async(&chart("style: nope", BAR), &RenderOptions::default()))
        .expect_err("unknown style fails on the async path too");
    assert!(err.to_string().contains("style 'nope' is not registered"), "{err}");
}

#[test]
fn render_chart_applies_registered_styles() {
    let c = with(&[CORPORATE]);
    let parsed = chartml_core::spec::parse(&chart("style: corporate", BAR)).expect("parse");
    let chartml_core::ChartMLSpec::Single(component) = parsed else { panic!("single") };
    let chartml_core::Component::Chart(spec) = *component else { panic!("chart") };
    let element = c.render_chart(&spec).expect("renders");
    assert_eq!(svg_height(&element_to_svg(&element, 800.0, 400.0)), 333.0);
}

// --- Validation: font weights and paints in named styles --------------------

#[test]
fn invalid_font_weights_fail_clearly() {
    for bad in ["0", "1001", "-1", "550.5", "heavy", ".inf"] {
        let style = format!("type: style\nversion: 1\nname: w\nfonts:\n  title:\n    weight: {bad}\n");
        let err = chartml().register_component(&style).expect_err(&format!("weight {bad} must fail"));
        let msg = err.to_string();
        assert!(msg.contains("style 'w'.fonts.title.weight"), "{msg}");
        assert!(msg.contains("1 to 1000"), "{msg}");

        let vis = format!("{BAR}  style:\n    fonts:\n      axis:\n        weight: {bad}\n");
        assert_invalid(render(&chartml(), &chart("", &vis)), &["visualize.style.fonts.axis.weight"]);
    }
}

#[test]
fn valid_font_weights_render() {
    for (w, expected) in [("1", "1"), ("1000", "1000"), ("normal", ""), ("bold", "700"), ("bolder", "700"), ("lighter", "100"), ("\"600\"", "600")] {
        let vis = format!("{BAR}  style:\n    fonts:\n      axis:\n        weight: {w}\n");
        let svg = render_ok(&chartml(), &chart("", &vis));
        for l in tags(&svg, "tick-label") {
            // 400 (normal, the default) emits no attribute.
            assert_eq!(attr(l, "font-weight").unwrap_or(""), expected, "weight {w}: {l}");
        }
    }
}

#[test]
fn unsafe_colour_in_a_named_style_fails() {
    for (field, yaml) in [
        ("style 'bad'.colors[0]", "colors: [\"url(https://example.com/p.svg#g)\"]"),
        ("style 'bad'.grid.color", "grid:\n  color: \"javascript:alert(1)\""),
        ("style 'bad'.fonts.title.color", "fonts:\n  title:\n    color: \"data:image/svg+xml;base64,AAAA\""),
    ] {
        let style = format!("type: style\nversion: 1\nname: bad\n{yaml}\n");
        let err = chartml().register_component(&style).expect_err("unsafe colour must fail");
        let msg = err.to_string();
        assert!(msg.contains(field), "{msg}");
        assert!(msg.contains("not an allowed colour"), "{msg}");
    }
    let config = "type: config\nversion: 1\nstyle:\n  colors: [\"var(--x)\"]\n";
    let err = chartml().register_component(config).expect_err("unsafe config colour must fail");
    assert!(err.to_string().contains("config.style.colors[0]"), "{err}");
}

#[test]
fn unsafe_colour_in_a_same_document_style_fails_the_render() {
    let doc = format!(
        "type: style\nversion: 1\nname: bad\ncolors: [\"url(https://e.com/p#g)\"]\n---\n{}",
        chart("style: bad", BAR)
    );
    assert_invalid(render(&chartml(), &doc), &["style 'bad'.colors[0]", "not an allowed colour"]);
}

// --- Legend position and orientation ----------------------------------------

/// The legend label tags and the plot's translate offset.
fn legend_labels(svg: &str) -> Vec<(f64, f64)> {
    tags(svg, "legend-label")
        .into_iter()
        .map(|t| {
            (
                attr(t, "x").expect("x").parse().expect("x"),
                attr(t, "y").expect("y").parse().expect("y"),
            )
        })
        .collect()
}

fn legend_style(position: &str, orientation: Option<&str>) -> String {
    let mut s = format!("type: style\nversion: 1\nname: legend\nlegend:\n  position: {position}\n");
    if let Some(o) = orientation {
        s.push_str(&format!("  orientation: {o}\n"));
    }
    s
}

#[test]
fn legend_positions_place_the_legend_on_each_side() {
    for vis in [BAR_BY_REGION, LINE_BY_REGION] {
        let default = legend_labels(&render_ok(&chartml(), &chart("", vis)));
        assert_eq!(default.len(), 2);
        assert!(default.iter().all(|&(_, y)| y > 350.0), "default legend at the bottom: {default:?}");

        let top = legend_labels(&render_ok(&with(&[&legend_style("top", None)]), &chart("style: legend", vis)));
        assert!(top.iter().all(|&(_, y)| y < 40.0), "top: {top:?}");
        assert_eq!(top[0].1, top[1].1, "top legend is one horizontal row");

        let left = legend_labels(&render_ok(&with(&[&legend_style("left", None)]), &chart("style: legend", vis)));
        assert!(left.iter().all(|&(x, _)| x < 100.0), "left: {left:?}");
        assert_eq!(left[0].0, left[1].0, "side legend is vertical by default");
        assert!(left[1].1 > left[0].1);

        let right = legend_labels(&render_ok(&with(&[&legend_style("right", None)]), &chart("style: legend", vis)));
        assert!(right.iter().all(|&(x, _)| x > 600.0), "right: {right:?}");

        let none_svg = render_ok(&with(&[&legend_style("none", None)]), &chart("style: legend", vis));
        assert!(legend_labels(&none_svg).is_empty(), "position none draws no legend");
    }
}

#[test]
fn legend_orientation_overrides_the_default_for_its_position() {
    let vis = BAR_BY_REGION;
    let bottom_vertical =
        legend_labels(&render_ok(&with(&[&legend_style("bottom", Some("vertical"))]), &chart("style: legend", vis)));
    assert_eq!(bottom_vertical[0].0, bottom_vertical[1].0, "{bottom_vertical:?}");
    assert!(bottom_vertical[1].1 > bottom_vertical[0].1);

    let left_horizontal =
        legend_labels(&render_ok(&with(&[&legend_style("left", Some("horizontal"))]), &chart("style: legend", vis)));
    assert_eq!(left_horizontal[0].1, left_horizontal[1].1, "{left_horizontal:?}");
}

#[test]
fn side_and_top_legends_keep_the_plot_clear() {
    // The bars group (the plot) moves right of a left legend and below a top
    // legend.
    fn plot_translate(svg: &str) -> (f64, f64) {
        let t = tags(svg, "bars").into_iter().find(|t| t.starts_with('g')).expect("bars group");
        let inner = attr(t, "transform").expect("transform");
        let nums: Vec<f64> = inner
            .trim_start_matches("translate(")
            .trim_end_matches(')')
            .split(',')
            .map(|n| n.trim().parse().expect("number"))
            .collect();
        (nums[0], nums[1])
    }
    let base = plot_translate(&render_ok(&chartml(), &chart("", BAR_BY_REGION)));
    let left = plot_translate(&render_ok(&with(&[&legend_style("left", None)]), &chart("style: legend", BAR_BY_REGION)));
    assert!(left.0 > base.0 + 40.0, "left legend pushes the plot right: {base:?} -> {left:?}");
    let top = plot_translate(&render_ok(&with(&[&legend_style("top", None)]), &chart("style: legend", BAR_BY_REGION)));
    assert!(top.1 > base.1 + 20.0, "top legend pushes the plot down: {base:?} -> {top:?}");
}

#[test]
fn legend_placement_applies_to_pie_and_scatter() {
    let pie = "  type: pie\n  columns: region\n  rows: revenue\n";
    let scatter = "  type: scatter\n  columns: budget\n  rows: sales\n  marks:\n    color: region\n";
    for vis in [pie, scatter] {
        let default = legend_labels(&render_ok(&chartml(), &chart("", vis)));
        assert!(!default.is_empty());
        assert!(default.iter().all(|&(_, y)| y > 340.0), "default bottom: {default:?}");
        let right = legend_labels(&render_ok(&with(&[&legend_style("right", None)]), &chart("style: legend", vis)));
        assert!(right.iter().all(|&(x, _)| x > 600.0), "right: {right:?}");
        let top = legend_labels(&render_ok(&with(&[&legend_style("top", None)]), &chart("style: legend", vis)));
        assert!(top.iter().all(|&(_, y)| y < 60.0), "top: {top:?}");
        let none = render_ok(&with(&[&legend_style("none", None)]), &chart("style: legend", vis));
        assert!(legend_labels(&none).is_empty());
    }
}

#[test]
fn unknown_legend_values_fail() {
    let err = chartml().register_component(&legend_style("middle", None)).expect_err("bad position");
    assert!(err.to_string().contains("legend.position must be one of top, right, bottom, left or none"), "{err}");
    let err = chartml().register_component(&legend_style("top", Some("diagonal"))).expect_err("bad orientation");
    assert!(err.to_string().contains("legend.orientation must be horizontal or vertical"), "{err}");
}

// --- Grid on scatter --------------------------------------------------------

#[test]
fn scatter_reads_style_grid() {
    let style = "type: style\nversion: 1\nname: g\ngrid:\n  x: false\n  color: \"#00ff00\"\n  opacity: 0.4\n";
    let scatter = "  type: scatter\n  columns: budget\n  rows: sales\n";
    let svg = render_ok(&with(&[style]), &chart("style: g", scatter));
    let grid = tags(&svg, "grid-line");
    assert!(!grid.is_empty(), "horizontal grid lines stay");
    for g in &grid {
        assert_eq!(attr(g, "stroke"), Some("#00ff00"), "{g}");
        assert_eq!(attr(g, "opacity"), Some("0.4"), "{g}");
        // x: false — only horizontal lines (y1 == y2).
        assert_eq!(attr(g, "y1"), attr(g, "y2"), "{g}");
    }
}

// --- Acceptance 4: the examples.md theme section ----------------------------

/// The docs site renders each ```chartml fence in page order on one renderer:
/// a fence of only components is registered, any other is rendered
/// (docs/docs/.vitepress/config.js). Replay the theme section that way.
#[test]
fn examples_md_theme_section_renders_with_the_theme() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let md = std::fs::read_to_string(root.join("docs/docs/examples.md")).expect("read examples.md");
    let section = md
        .split("## Reusable Styles and Configuration")
        .nth(1)
        .and_then(|s| s.split("\n## ").next())
        .expect("theme section");

    let mut fences = Vec::new();
    let mut current: Option<String> = None;
    for line in section.lines() {
        match (&mut current, line.trim_end()) {
            (None, "```chartml") => current = Some(String::new()),
            (Some(buf), "```") => {
                fences.push(std::mem::take(buf));
                current = None;
            }
            (Some(buf), l) => {
                buf.push_str(l);
                buf.push('\n');
            }
            _ => {}
        }
    }

    let mut c = chartml();
    let mut charts = Vec::new();
    for fence in &fences {
        if fence.contains("type: style") || fence.contains("type: config") {
            c.register_component(fence).expect("style/config fence registers");
        } else {
            charts.push(render_ok(&c, fence));
        }
    }
    assert_eq!(charts.len(), 3, "three themed charts");

    for svg in &charts {
        for l in tags(svg, "tick-label") {
            assert_eq!(attr(l, "font-family"), Some("Georgia, serif"), "axis font from the theme: {l}");
            assert_eq!(attr(l, "fill"), Some("#5f6368"), "{l}");
        }
        for g in tags(svg, "grid-line") {
            assert_eq!(attr(g, "opacity"), Some("0.5"), "grid opacity from the theme: {g}");
        }
    }
    // Chart 1 (bar) and the override chart (bar) use the theme's first colour.
    assert!(bar_fills(&charts[0]).iter().all(|f| f == "#4285f4"));
    assert_eq!(svg_height(&charts[0]), 400.0);
    // Chart 2 (line) draws its line in the theme's first colour.
    assert!(tags(&charts[1], "series-line").iter().all(|p| attr(p, "stroke") == Some("#4285f4")));
    // The override chart: its own height and grid colour, the theme's rest.
    assert_eq!(svg_height(&charts[2]), 300.0);
    assert!(bar_fills(&charts[2]).iter().all(|f| f == "#4285f4"));
    for g in tags(&charts[2], "grid-line") {
        assert_eq!(attr(g, "stroke"), Some("#ff0000"), "{g}");
    }
}

// --- Also fixed: fillOpacity, documented as read from visualize.style ------

#[test]
fn area_fill_opacity_comes_from_the_style() {
    let area = "  type: area\n  columns: month\n  rows: revenue\n";
    let fills = |svg: &str| -> Vec<String> {
        tags(svg, "chartml-area-path").into_iter().filter_map(|t| attr(t, "opacity").map(str::to_string)).collect()
    };
    let default = fills(&render_ok(&chartml(), &chart("", area)));
    assert!(!default.is_empty() && default.iter().all(|o| o == "0.6"), "{default:?}");

    let config = "type: config\nversion: 1\nstyle:\n  fillOpacity: 0.3\n";
    let styled = fills(&render_ok(&with(&[config]), &chart("", area)));
    assert!(!styled.is_empty() && styled.iter().all(|o| o == "0.3"), "{styled:?}");

    let vis = format!("{area}  style:\n    fillOpacity: 1.5\n");
    assert_invalid(render(&chartml(), &chart("", &vis)), &["visualize.style.fillOpacity"]);
}

// --- Registration is all-or-nothing -------------------------------------------

#[test]
fn a_failing_component_leaves_the_registration_unchanged() {
    let mut c = with(&[CORPORATE, "type: config\nversion: 1\nstyle: corporate\n"]);
    // A config naming `brand`, then an invalid `brand` style: neither applies.
    let bad = "type: config\nversion: 1\nstyle: brand\n---\ntype: style\nversion: 1\nname: brand\ncolors: [\"javascript:alert(1)\"]\n";
    let err = c.register_component(bad).expect_err("the invalid style fails the registration");
    assert!(err.to_string().contains("style 'brand'.colors[0]"), "{err}");
    let svg = render_ok(&c, &chart("", BAR));
    assert_eq!(svg_height(&svg), 333.0, "the earlier config still applies alone");
    assert!(bar_fills(&svg).iter().all(|f| f == "#123456"));

    // A replacing style before a failing component does not replace.
    let replacing = "type: style\nversion: 1\nname: corporate\nheight: 100\n---\ntype: style\nversion: 1\nname: x\nfonts:\n  axis:\n    weight: 0\n";
    c.register_component(replacing).expect_err("bad weight fails");
    assert_eq!(svg_height(&render_ok(&c, &chart("style: corporate", BAR))), 333.0);
}

// --- Tables keep their own typography ---------------------------------------

#[test]
fn axis_fonts_do_not_reach_tables() {
    let table = "  type: table\n  rows: [month, revenue]\n";
    let plain = serde_json::to_value(chartml().render_from_yaml(&chart("", table)).expect("table")).expect("json");
    let styled = serde_json::to_value(with(&[CORPORATE]).render_from_yaml(&chart("style: corporate", table)).expect("table"))
        .expect("json");
    assert!(!styled.to_string().contains("Courier New"), "fonts.axis must not reach table cells");
    assert_eq!(plain, styled, "a table renders the same with or without an axis-font style");
}

#[test]
fn grid_opacity_outside_0_to_1_fails() {
    for bad in ["5", "-0.1", ".nan"] {
        let style = format!("type: style\nversion: 1\nname: g\ngrid:\n  opacity: {bad}\n");
        let err = chartml().register_component(&style).expect_err("bad grid opacity");
        assert!(err.to_string().contains("style 'g'.grid.opacity must be a number from 0 to 1"), "{err}");
    }
}

// --- Colours the chart chose survive the browser stylesheet ------------------

#[test]
fn chart_chosen_colours_are_marked_for_the_browser_stylesheet() {
    let style = "type: style\nversion: 1\nname: paint\ngrid:\n  color: \"#00ff00\"\nfonts:\n  axis:\n    color: \"#445566\"\n  dataLabel:\n    color: \"#0000ff\"\n";
    let c = with(&[style]);
    let svg = render_ok(&c, &chart("style: paint", BAR_WITH_LABELS));
    for class in ["tick-label", "grid-line", "data-label"] {
        let els = tags(&svg, class);
        assert!(!els.is_empty(), "{class}");
        for e in els {
            assert!(attr(e, "class").expect("class").split_whitespace().any(|c| c == "explicit-paint"), "{class}: {e}");
        }
    }

    // Theme colours stay unmarked, so the page theme still styles them.
    let plain = render_ok(&chartml(), &chart("", BAR_WITH_LABELS));
    assert!(!plain.contains("explicit-paint"), "{plain}");

    // A field's own dataLabels.color, inline grid.color, and a line's
    // series-coloured data labels are the chart's choice too.
    let own = "  type: bar\n  columns: month\n  rows:\n    field: revenue\n    dataLabels:\n      show: true\n      color: \"#999999\"\n  style:\n    grid:\n      color: \"#ff0000\"\n";
    let svg = render_ok(&chartml(), &chart("", own));
    for class in ["data-label", "grid-line"] {
        for e in tags(&svg, class) {
            assert!(attr(e, "class").expect("class").contains("explicit-paint"), "{e}");
        }
    }
    let line = "  type: line\n  columns: month\n  rows:\n    - field: revenue\n      dataLabels:\n        show: true\n";
    let svg = render_ok(&chartml(), &chart("", line));
    let labels = tags(&svg, "data-label");
    assert!(!labels.is_empty());
    for e in labels {
        assert!(attr(e, "class").expect("class").contains("explicit-paint"), "{e}");
    }
}

// --- Also fixed: data labels on single-field lines and horizontal bars -------

#[test]
fn single_field_line_draws_its_data_labels() {
    let line = "  type: line\n  columns: month\n  rows:\n    field: revenue\n    dataLabels:\n      show: true\n";
    let c = with(&[CORPORATE]);
    let svg = render_ok(&c, &chart("style: corporate", line));
    let labels = tags(&svg, "data-label");
    // One label per category (Jan, Feb) at each point, in the data label font.
    assert_eq!(labels.len(), 2, "{svg}");
    for l in labels {
        assert_eq!(attr(l, "font-family"), Some("Verdana, sans-serif"), "{l}");
        assert_eq!(attr(l, "font-size"), Some("15px"), "{l}");
        assert_eq!(attr(l, "fill"), Some("#123456"), "defaults to the line's colour: {l}");
    }
}

#[test]
fn horizontal_bars_draw_their_data_labels_past_the_bar_end() {
    let rows = "  provider: inline\n  rows:\n    - { product: A, arr: 10 }\n    - { product: B, arr: 30 }\n";
    let yaml = |position: &str| {
        format!(
            "type: chart\nversion: 1\ndata:\n{rows}visualize:\n  type: bar\n  orientation: horizontal\n  columns: product\n  rows:\n    field: arr\n    dataLabels:\n      show: true\n{position}"
        )
    };
    let svg = render_ok(&chartml(), &yaml(""));
    let bars = tags(&svg, "bar-rect");
    let labels = tags(&svg, "data-label");
    assert_eq!(labels.len(), 2, "{svg}");
    for (bar, label) in bars.iter().zip(&labels) {
        let end: f64 = attr(bar, "width").expect("width").parse().expect("number");
        let x: f64 = attr(label, "x").expect("x").parse().expect("number");
        assert_eq!(x, end + 5.0, "label just past the bar end: {label}");
        assert_eq!(attr(label, "text-anchor"), Some("start"));
        assert_eq!(attr(label, "dominant-baseline"), Some("middle"));
    }
    // The widest label stays inside the chart.
    let root_width = 800.0;
    let wide = "  provider: inline\n  rows:\n    - { product: A, arr: 1000000 }\n    - { product: B, arr: 3100000 }\n";
    let wide_svg = render_ok(
        &chartml(),
        &format!("type: chart\nversion: 1\ndata:\n{wide}visualize:\n  type: bar\n  orientation: horizontal\n  columns: product\n  rows:\n    field: arr\n    dataLabels:\n      show: true\n"),
    );
    let plot = tags(&wide_svg, "bars").into_iter().find(|t| t.starts_with('g')).expect("bars group");
    let plot_x: f64 = attr(plot, "transform").expect("transform").trim_start_matches("translate(").split(',').next().expect("x").parse().expect("number");
    for (bar, label) in tags(&wide_svg, "bar-rect").iter().zip(tags(&wide_svg, "data-label")) {
        let end: f64 = attr(bar, "width").expect("width").parse().expect("number");
        // "3,100,000" is 9 characters at about 7px each at 12px.
        assert!(plot_x + end + 5.0 + 9.0 * 7.0 <= root_width, "label runs off the chart: {label}");
    }

    let centred = render_ok(&chartml(), &yaml("      position: center\n"));
    for (bar, label) in tags(&centred, "bar-rect").iter().zip(tags(&centred, "data-label")) {
        let end: f64 = attr(bar, "width").expect("width").parse().expect("number");
        assert_eq!(attr(label, "x").expect("x").parse::<f64>().expect("number"), end / 2.0);
        assert_eq!(attr(label, "text-anchor"), Some("middle"));
    }
}

// --- Also fixed: every path resolves document `type: params` the same way ---

fn params_doc() -> String {
    format!(
        "type: params\nversion: 1\nname: f\nparams:\n  - id: brand\n    type: text\n    label: Brand\n    default: \"#abcdef\"\n---\n{}",
        chart("", "  type: bar\n  columns: month\n  rows: revenue\n  style:\n    colors: [\"$f.brand\"]\n")
    )
}

#[test]
fn document_params_resolve_on_the_async_path_as_on_the_sync_path() {
    let c = chartml();
    let sync = render_ok(&c, &params_doc());
    assert!(bar_fills(&sync).iter().all(|f| f == "#abcdef"), "sync: {:?}", bar_fills(&sync));

    let rt = tokio::runtime::Builder::new_current_thread().build().expect("runtime");
    let svg = rt
        .block_on(c.render_to_svg_async(&params_doc(), &RenderOptions::default()))
        .expect("async render resolves document params");
    assert!(bar_fills(&svg).iter().all(|f| f == "#abcdef"), "async: {:?}", bar_fills(&svg));
}

#[test]
fn param_overrides_beat_document_param_defaults() {
    let mut overrides = chartml_core::params::ParamValues::new();
    overrides.insert("f.brand".to_string(), serde_json::json!("#111111"));
    let c = chartml();
    let element = c
        .render_from_yaml_with_params(&params_doc(), None, None, Some(&overrides))
        .expect("renders");
    let svg = element_to_svg(&element, 800.0, 400.0);
    assert!(bar_fills(&svg).iter().all(|f| f == "#111111"), "{:?}", bar_fills(&svg));

    let opts = RenderOptions { params: Some(overrides), ..RenderOptions::default() };
    let rt = tokio::runtime::Builder::new_current_thread().build().expect("runtime");
    let svg = rt.block_on(c.render_to_svg_async(&params_doc(), &opts)).expect("async");
    assert!(bar_fills(&svg).iter().all(|f| f == "#111111"), "{:?}", bar_fills(&svg));
}

// --- Also fixed: the PNG canvas is the size the chart rendered at ------------

/// Width and height from a PNG's IHDR chunk.
fn png_size(png: &[u8]) -> (u32, u32) {
    let be = |b: &[u8]| u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
    (be(&png[16..20]), be(&png[20..24]))
}

#[test]
fn png_canvas_uses_the_style_height_not_the_requested_height() {
    let c = with(&[CORPORATE, "type: config\nversion: 1\nstyle: corporate\n"]);
    let png = chartml_render::render_to_png(&c, &chart("", BAR), 640, 480, 72).expect("png");
    // chartml-render pads the canvas by 16px on each side at 72 dpi.
    let pad = 2 * 16;
    assert_eq!(png_size(&png), (640 + pad, 333 + pad), "canvas height is the style's 333px chart");

    let rt = tokio::runtime::Builder::new_current_thread().build().expect("runtime");
    let png = rt
        .block_on(chartml_render::render_to_png_async(&c, &chart("", BAR), 640, 480, 72))
        .expect("png");
    assert_eq!(png_size(&png), (640 + pad, 333 + pad));

    // Without a style the requested size is used.
    let png = chartml_render::render_to_png(&chartml(), &chart("", BAR), 640, 480, 72).expect("png");
    assert_eq!(png_size(&png), (640 + pad, 480 + pad));
}
