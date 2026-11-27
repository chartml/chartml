//! CHA-16: chart specs are untrusted, so every colour that reaches an SVG
//! `fill` / `stroke` is checked first. An unsafe paint (external `url()`,
//! `javascript:`, `data:`) fails the render with a clear error; ordinary
//! colours and same-document gradient references render unchanged.
//!
//! These tests drive the real renderers for every chart type and read the
//! rendered SVG attributes, not an internal model.

use chartml_chart_cartesian::CartesianRenderer;
use chartml_chart_metric::MetricRenderer;
use chartml_chart_pie::PieRenderer;
use chartml_chart_scatter::ScatterRenderer;
use chartml_chart_table::TableRenderer;
use chartml_core::{ChartError, ChartML};
use chartml_render::element_to_svg;

const UNSAFE: &[&str] = &[
    "url(https://example.com/paint.svg#gradient)",
    "url(//example.com/paint.svg#gradient)",
    "url(data:image/svg+xml;base64,AAAA)",
    "javascript:alert(1)",
    "data:image/svg+xml;base64,AAAA",
    "url(#g) url(https://example.com/p.svg#x)",
];

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

/// A complete chart of `chart_type`. `{VIS}` extras are appended to `visualize`.
fn chart(chart_type: &str, visualize_extra: &str) -> String {
    let visualize = match chart_type {
        "pie" | "doughnut" => format!("type: {chart_type}\n    columns: region\n    rows: revenue"),
        "scatter" => "type: scatter\n    columns: budget\n    rows: sales".to_string(),
        "bubble" => "type: bubble\n    columns: budget\n    rows: sales\n    marks:\n      size: revenue".to_string(),
        "metric" => "type: metric\n    value: revenue\n    label: Revenue".to_string(),
        "table" => "type: table\n    rows: [region, revenue]".to_string(),
        t => format!("type: {t}\n    columns: region\n    rows: revenue"),
    };
    format!(
        "type: chart\nversion: 1\ndata:\n  provider: inline\n  rows:\n    \
         - {{ region: A, revenue: 10, budget: 1, sales: 5 }}\n    \
         - {{ region: B, revenue: 20, budget: 2, sales: 9 }}\n    \
         - {{ region: C, revenue: 30, budget: 3, sales: 4 }}\n\
         visualize:\n    {visualize}\n{visualize_extra}"
    )
}

const TYPES: &[&str] =
    &["bar", "line", "area", "pie", "doughnut", "scatter", "bubble", "metric", "table"];

fn render_svg(yaml: &str) -> Result<String, ChartError> {
    let element = chartml().render_from_yaml(yaml)?;
    Ok(element_to_svg(&element, 800.0, 400.0))
}

fn assert_unsafe_rejected(yaml: &str, field: &str, value: &str) {
    match render_svg(yaml) {
        Err(ChartError::InvalidSpec(msg)) => {
            assert!(msg.contains(field), "error should name {field}: {msg}");
            assert!(msg.contains("not an allowed colour"), "{msg}");
        }
        Err(other) => panic!("{field}={value:?}: wrong error: {other}"),
        Ok(svg) => panic!("{field}={value:?}: rendered instead of failing: {svg}"),
    }
}

#[test]
fn style_colors_with_unsafe_paint_fail_for_every_chart_type() {
    for t in TYPES {
        for v in UNSAFE {
            let yaml = chart(t, &format!("    style:\n      colors: ['#2E7D9A', '{v}']"));
            assert_unsafe_rejected(&yaml, "visualize.style.colors[1]", v);
        }
    }
}

#[test]
fn field_colour_overrides_with_unsafe_paint_fail() {
    for t in ["bar", "line", "area"] {
        for v in UNSAFE {
            let yaml = format!(
                "type: chart\nversion: 1\ndata:\n  provider: inline\n  rows:\n    \
                 - {{ region: A, revenue: 10, target: 5 }}\n    - {{ region: B, revenue: 20, target: 6 }}\n\
                 visualize:\n    type: {t}\n    columns: region\n    rows:\n      \
                 - {{ field: revenue, color: '#2E7D9A' }}\n      - {{ field: target, color: '{v}' }}\n"
            );
            assert_unsafe_rejected(&yaml, "visualize.rows[1].color", v);
        }
    }
}

#[test]
fn single_field_colour_override_with_unsafe_paint_fails() {
    for v in UNSAFE {
        let yaml = format!(
            "type: chart\nversion: 1\ndata:\n  provider: inline\n  rows:\n    - {{ region: A, revenue: 10 }}\n\
             visualize:\n    type: bar\n    columns: region\n    rows: {{ field: revenue, color: '{v}' }}\n"
        );
        assert_unsafe_rejected(&yaml, "visualize.rows.color", v);
    }
}

#[test]
fn other_spec_paint_fields_with_unsafe_paint_fail() {
    let v = "url(https://example.com/paint.svg#gradient)";
    let cases = [
        ("visualize.style.grid.color", format!("    style:\n      grid: {{ y: true, color: '{v}' }}")),
        ("visualize.style.fonts.axis.color", format!("    style:\n      fonts:\n        axis: {{ color: '{v}' }}")),
        ("visualize.style.fonts.title.color", format!("    style:\n      fonts:\n        title: {{ color: '{v}' }}")),
        ("visualize.style.fonts.dataLabel.color", format!("    style:\n      fonts:\n        dataLabel: {{ color: '{v}' }}")),
        ("visualize.dataLabels.color", format!("    dataLabels: {{ show: true, color: '{v}' }}")),
        (
            "visualize.annotations[0].color",
            format!("    annotations:\n      - {{ type: line, axis: y, value: 15, color: '{v}' }}"),
        ),
        (
            "visualize.annotations[0].strokeColor",
            format!("    annotations:\n      - {{ type: line, axis: y, value: 15, strokeColor: '{v}' }}"),
        ),
        ("style.colors[0]", format!("style:\n  colors: ['{v}']")),
    ];
    for (field, extra) in cases {
        assert_unsafe_rejected(&chart("bar", &extra), field, v);
    }
}

#[test]
fn data_label_colour_inside_a_field_with_unsafe_paint_fails() {
    let v = "javascript:alert(1)";
    let yaml = format!(
        "type: chart\nversion: 1\ndata:\n  provider: inline\n  rows:\n    - {{ region: A, revenue: 10 }}\n\
         visualize:\n    type: bar\n    columns: region\n    rows:\n      \
         - {{ field: revenue, dataLabels: {{ show: true, color: '{v}' }} }}\n"
    );
    assert_unsafe_rejected(&yaml, "visualize.rows[0].dataLabels.color", v);
}

#[test]
fn unsafe_default_palette_fails_instead_of_being_replaced() {
    let mut c = chartml();
    c.set_default_palette(vec!["#2E7D9A".into(), "url(https://example.com/p.svg#g)".into()]);
    let err = c.render_from_yaml(&chart("bar", "")).expect_err("unsafe palette must fail");
    assert!(err.to_string().contains("palette[1]"), "{err}");
}

#[test]
fn unsafe_paint_fails_on_the_async_path_too() {
    let yaml = chart("bar", "    style:\n      colors: ['url(https://example.com/p.svg#g)']");
    let rt = tokio::runtime::Builder::new_current_thread().build().expect("runtime");
    let err = rt
        .block_on(chartml().render_from_yaml_with_params_async(&yaml, None, None, None))
        .expect_err("unsafe paint must fail");
    assert!(matches!(err, ChartError::InvalidSpec(_)), "{err}");
}

/// Ordinary colours of every allowed form render, and reach the SVG verbatim.
#[test]
fn ordinary_colours_render_into_svg_attributes() {
    let colours = ["#2E7D9A", "#abc", "rgb(46, 125, 154)", "hsl(194, 54%, 39%)", "rebeccapurple"];
    for t in ["bar", "line", "area", "pie", "doughnut", "scatter", "bubble"] {
        for c in colours {
            let svg = render_svg(&chart(t, &format!("    style:\n      colors: ['{c}']"))).unwrap_or_else(|e| panic!("{t} {c}: {e}"));
            assert!(
                svg.contains(&format!("fill=\"{c}\"")) || svg.contains(&format!("stroke=\"{c}\"")),
                "{t}: {c} missing from SVG paint attributes"
            );
        }
    }
    for t in ["metric", "table"] {
        render_svg(&chart(t, "    style:\n      colors: ['#2E7D9A']")).unwrap_or_else(|e| panic!("{t}: {e}"));
    }
}

#[test]
fn same_document_gradient_reference_is_kept() {
    for t in ["bar", "pie", "scatter"] {
        let svg = render_svg(&chart(t, "    style:\n      colors: ['url(#brand-gradient)']")).unwrap_or_else(|e| panic!("{t}: {e}"));
        assert!(svg.contains("fill=\"url(#brand-gradient)\""), "{t}: internal gradient dropped: {svg}");
    }
}

#[test]
fn rendered_svg_never_contains_an_external_paint_for_valid_specs() {
    for t in TYPES {
        let svg = render_svg(&chart(t, "")).unwrap_or_else(|e| panic!("{t}: {e}"));
        assert!(!svg.contains("url(http") && !svg.contains("url(//") && !svg.contains("javascript:"), "{t}");
    }
}

/// A prepared chart is parsed and fetched once and rendered many times, and
/// its public `spec` can be changed in between: every render re-checks it.
#[test]
fn prepared_chart_with_unsafe_paint_fails_at_render() {
    use chartml_core::pipeline::RenderOptions;

    let rt = tokio::runtime::Builder::new_current_thread().build().expect("runtime");
    let c = chartml();
    let opts = RenderOptions::default();
    let yaml = chart("bar", "    style:\n      colors: ['#2E7D9A']");
    let mut prepared = rt.block_on(async {
        let fetched = c.fetch(&yaml, &opts).await.expect("fetch");
        c.transform(fetched, &opts).await.expect("transform")
    });
    assert!(c.render_prepared_to_svg(&prepared, &opts).is_ok());

    let style = prepared.spec.visualize.style.as_mut().expect("style");
    style.colors = Some(vec!["url(https://example.com/p.svg#g)".into()]);
    match c.render_prepared_to_svg(&prepared, &opts) {
        Err(ChartError::InvalidSpec(msg)) => assert!(msg.contains("visualize.style.colors[0]"), "{msg}"),
        other => panic!("expected InvalidSpec, got {other:?}"),
    }
}

/// The check runs before any data is resolved: a spec that names a missing
/// data source still reports the paint, not the source.
#[test]
fn unsafe_paint_is_reported_before_data_is_resolved() {
    let yaml = "type: chart\nversion: 1\ndata: no_such_source\nvisualize:\n  type: bar\n  columns: a\n  rows: b\n  \
                style:\n    colors: ['javascript:alert(1)']\n";
    let c = chartml();
    let sync = c.render_from_yaml(yaml).map(|_| ()).expect_err("must fail");
    assert!(matches!(sync, ChartError::InvalidSpec(_)), "sync: {sync}");

    let rt = tokio::runtime::Builder::new_current_thread().build().expect("runtime");
    let opts = chartml_core::pipeline::RenderOptions::default();
    let fetched = rt.block_on(c.fetch(yaml, &opts)).map(|_| ()).expect_err("must fail");
    assert!(matches!(fetched, ChartError::InvalidSpec(_)), "fetch: {fetched}");
}
