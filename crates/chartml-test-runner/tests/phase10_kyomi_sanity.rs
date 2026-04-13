//! Phase 10 — Kyomi sanity-check integration test.
//!
//! Constructs a `Theme` with the full Kyomi target values and renders a
//! handful of chart types through `ChartML::render_from_yaml`, then asserts
//! each of the 9 expected visual properties from the theme-hooks plan is
//! present in the emitted element tree.
//!
//! The 9 sanity checks from the plan:
//!   1. Serif chart titles (not sans)
//!   2. Horizontal-only gridlines (zero vertical gridlines)
//!   3. 2px series line weight
//!   4. Bars with 2px rounded corners
//!   5. Dot markers with a 1.5px visible halo
//!   6. Uppercase axis category labels with letter-spacing
//!   7. Tabular-num numeric tick values in monospace
//!   8. Emphasized zero-line when data crosses zero
//!   9. Transparent chart background (no white fill rectangle)
//!
//! Checks 1, 7, and 9 are partially verified at different layers; see the
//! per-check comments below.

use chartml_chart_cartesian::CartesianRenderer;
use chartml_chart_pie::PieRenderer;
use chartml_chart_scatter::ScatterRenderer;
use chartml_core::element::{count_elements, ChartElement, TextAnchor, Transform as ElTransform};
use chartml_core::layout::labels::{measure_text, TextMetrics};
use chartml_core::theme::{BarCornerRadius, GridStyle, TextTransform, Theme, ZeroLineSpec};
use chartml_core::ChartML;

/// Build the Kyomi target theme exactly as specified in the Phase 10 plan.
fn kyomi_theme() -> Theme {
    Theme {
        // Chrome colors
        text: "#1C1917".into(),
        text_secondary: "#6B6660".into(),
        text_strong: "#1C1917".into(),
        axis_line: "#1C1917".into(),
        tick: "#1C1917".into(),
        grid: "#EDE9E0".into(),
        bg: String::new(), // transparent

        // Title typography
        title_font_family: "Instrument Serif, Georgia, serif".into(),
        title_font_size: 22.0,
        title_font_weight: 400,
        title_font_style: "normal".into(),

        // Label typography
        label_font_family: "DM Sans, system-ui, sans-serif".into(),
        label_font_size: 10.0,
        label_font_weight: 500,
        label_letter_spacing: 1.2,
        label_text_transform: TextTransform::Uppercase,

        // Numeric tick typography
        numeric_font_family: "Geist Mono, monospace".into(),
        numeric_font_size: 11.0,

        // Legend typography
        legend_font_family: "DM Sans, system-ui, sans-serif".into(),
        legend_font_size: 11.0,
        legend_font_weight: 500,

        // Shape / stroke — defaults already match plan targets
        axis_line_weight: 1.0,
        grid_line_weight: 1.0,
        series_line_weight: 2.0,
        annotation_line_weight: 1.0,
        bar_corner_radius: BarCornerRadius::Top(2.0),
        dot_radius: 4.0,
        dot_halo_color: Some("#FAFAF8".into()),
        dot_halo_width: 1.5,

        grid_style: GridStyle::HorizontalOnly,
        zero_line: Some(ZeroLineSpec {
            color: "#1C1917".into(),
            width: 1.5,
        }),
    }
}

fn new_chartml() -> ChartML {
    let mut c = ChartML::new();
    c.register_renderer("bar", CartesianRenderer::new());
    c.register_renderer("line", CartesianRenderer::new());
    c.register_renderer("area", CartesianRenderer::new());
    c.register_renderer("scatter", ScatterRenderer::new());
    c.register_renderer("pie", PieRenderer::new());
    c.register_renderer("doughnut", PieRenderer::new());
    c.set_theme(kyomi_theme());
    c
}

// ---------------- YAML fixtures ----------------

const BAR_CROSSING_ZERO_YAML: &str = r#"
type: chart
version: 1
title: "Monthly Delta"
data:
  provider: inline
  rows:
    - month: "Jan"
      delta: -5
    - month: "Feb"
      delta: 0
    - month: "Mar"
      delta: 10
visualize:
  type: bar
  columns: month
  rows: delta
  style:
    grid:
      x: true
      y: true
"#;

const LINE_CROSSING_ZERO_YAML: &str = r#"
type: chart
version: 1
title: "Quarterly Trend"
data:
  provider: inline
  rows:
    - q: "Q1"
      v: -5
    - q: "Q2"
      v: 0
    - q: "Q3"
      v: 10
visualize:
  type: line
  columns: q
  rows: v
  style:
    grid:
      x: true
      y: true
"#;

// ---------------- Walkers ----------------

fn collect<'a, F: Fn(&'a ChartElement) -> bool>(
    el: &'a ChartElement,
    pred: &F,
    out: &mut Vec<&'a ChartElement>,
) {
    if pred(el) {
        out.push(el);
    }
    match el {
        ChartElement::Svg { children, .. }
        | ChartElement::Group { children, .. }
        | ChartElement::Div { children, .. } => {
            for c in children {
                collect(c, pred, out);
            }
        }
        _ => {}
    }
}

fn has_class(class: &str, name: &str) -> bool {
    class.split_whitespace().any(|c| c == name)
}

fn render_bar_crossing_zero() -> ChartElement {
    let chartml = new_chartml();
    chartml
        .render_from_yaml(BAR_CROSSING_ZERO_YAML)
        .expect("bar render")
}

fn render_line_crossing_zero() -> ChartElement {
    let chartml = new_chartml();
    chartml
        .render_from_yaml(LINE_CROSSING_ZERO_YAML)
        .expect("line render")
}

// ---------------- Sanity checks ----------------

/// Check 1 — Serif chart titles. The chart title is rendered by
/// `chartml-leptos::build_title_style`, not as an SVG `<text>` element.
/// That helper is `pub(crate)`, so this sanity check is verified by a
/// companion unit test in `chartml-leptos/src/chart.rs` (see
/// `phase10_kyomi_title_uses_serif_family`).
#[test]
fn phase10_check1_serif_title_deferred_to_leptos_unit_test() {
    // Documented deferral — see chartml-leptos chart.rs tests module.
}

/// Check 2 — Horizontal-only gridlines (zero grid-line-x, at least one grid-line-y).
#[test]
fn phase10_check2_horizontal_only_gridlines() {
    let el = render_line_crossing_zero();
    let mut lines = Vec::new();
    collect(
        &el,
        &|e| matches!(e, ChartElement::Line { .. }),
        &mut lines,
    );
    let (mut vx, mut hy) = (0usize, 0usize);
    for l in &lines {
        if let ChartElement::Line { class, .. } = l {
            if has_class(class, "grid-line-x") {
                vx += 1;
            }
            if has_class(class, "grid-line-y") {
                hy += 1;
            }
        }
    }
    assert_eq!(vx, 0, "Kyomi theme must suppress vertical gridlines");
    assert!(
        hy > 0,
        "Kyomi theme must still emit horizontal gridlines (got {hy})"
    );
}

/// Check 3 — All `series-line` paths carry `stroke_width = Some(2.0)`.
#[test]
fn phase10_check3_series_line_weight_is_2px() {
    let el = render_line_crossing_zero();
    let mut series = Vec::new();
    collect(
        &el,
        &|e| matches!(e, ChartElement::Path { class, .. } if has_class(class, "series-line")),
        &mut series,
    );
    assert!(!series.is_empty(), "expected at least one series-line path");
    for p in &series {
        if let ChartElement::Path { stroke_width, .. } = p {
            assert_eq!(
                *stroke_width,
                Some(2.0),
                "series-line must have stroke_width 2.0"
            );
        }
    }
}

/// Check 4 — Bars use `BarCornerRadius::Top(2.0)` — i.e. each bar is
/// emitted as a `Path` whose `d` attribute contains exactly two 2px arcs on
/// the max-value end of the bar. Top-only rounding is what the Kyomi visual
/// target requires; uniform rounding produces a pill/toy look on bars that
/// sit on the baseline.
#[test]
fn phase10_check4_bars_have_2px_top_rounding() {
    let el = render_bar_crossing_zero();
    let mut bars = Vec::new();
    collect(
        &el,
        &|e| matches!(e, ChartElement::Path { class, .. } if has_class(class, "bar-rect")),
        &mut bars,
    );
    assert!(
        !bars.is_empty(),
        "expected at least one bar-rect Path (Top(2.0) must emit Path, not Rect)"
    );
    for b in &bars {
        if let ChartElement::Path { d, .. } = b {
            // Exactly two arcs of radius 2 per bar.
            assert_eq!(
                d.matches("A 2,2").count(),
                2,
                "bar path must contain exactly two 2px arcs, got d={d}"
            );
        }
    }

    // Sanity: any leftover bar-rect Rect under Top(2.0) must be a
    // degenerate zero-dimension bar (e.g. a value-at-zero category whose
    // height collapses to 0). Those can't host an arc so the helper emits
    // a plain Rect — but they must not have rx/ry.
    let mut leftover_rects = Vec::new();
    collect(
        &el,
        &|e| matches!(e, ChartElement::Rect { class, .. } if has_class(class, "bar-rect")),
        &mut leftover_rects,
    );
    for r in &leftover_rects {
        if let ChartElement::Rect { width, height, rx, ry, .. } = r {
            assert!(
                *width <= 0.0 || *height <= 0.0,
                "leftover bar Rect under Top(2.0) must be zero-dimension, got w={width} h={height}"
            );
            assert!(rx.is_none() && ry.is_none());
        }
    }
}

/// Check 5 — Every `dot-marker` is immediately preceded inside its parent
/// group by a `dot-halo` Path with stroke `#FAFAF8` and stroke_width 1.5.
#[test]
fn phase10_check5_dot_halo_visible() {
    let el = render_line_crossing_zero();

    // Count halos and dots, then inspect halo attributes.
    let halo_count = count_elements(&el, &|e| {
        matches!(e, ChartElement::Path { class, .. } if has_class(class, "dot-halo"))
    });
    let dot_count = count_elements(&el, &|e| {
        matches!(e, ChartElement::Circle { class, .. } if has_class(class, "dot-marker"))
    });
    assert!(dot_count > 0, "expected at least one dot-marker");
    assert_eq!(
        halo_count, dot_count,
        "expected one dot-halo per dot-marker (halos={halo_count}, dots={dot_count})"
    );

    let mut halos = Vec::new();
    collect(
        &el,
        &|e| matches!(e, ChartElement::Path { class, .. } if has_class(class, "dot-halo")),
        &mut halos,
    );
    for h in &halos {
        if let ChartElement::Path {
            stroke,
            stroke_width,
            ..
        } = h
        {
            assert_eq!(stroke.as_deref(), Some("#FAFAF8"));
            assert_eq!(*stroke_width, Some(1.5));
        }
    }

    // Ordering: inside the direct parent group of any dot-marker, the halo
    // for that marker must appear before the marker itself.
    fn check_ordering(el: &ChartElement) -> bool {
        match el {
            ChartElement::Svg { children, .. } | ChartElement::Group { children, .. } => {
                let mut last_halo_idx: Option<usize> = None;
                for (i, c) in children.iter().enumerate() {
                    if matches!(c, ChartElement::Path { class, .. } if has_class(class, "dot-halo")) {
                        last_halo_idx = Some(i);
                    }
                    if let ChartElement::Circle { class, .. } = c {
                        if has_class(class, "dot-marker") {
                            match last_halo_idx {
                                Some(idx) if idx < i => {}
                                _ => return false,
                            }
                        }
                    }
                }
                children.iter().all(check_ordering)
            }
            _ => true,
        }
    }
    assert!(
        check_ordering(&el),
        "every dot-marker must be preceded by a dot-halo in its parent group"
    );
}

/// Check 6 — Axis category labels are uppercased with letter-spacing.
#[test]
fn phase10_check6_uppercase_labels_with_spacing() {
    let el = render_bar_crossing_zero();
    let mut labels = Vec::new();
    collect(
        &el,
        &|e| matches!(e, ChartElement::Text { class, .. } if has_class(class, "axis-label")),
        &mut labels,
    );
    assert!(!labels.is_empty(), "expected at least one axis-label text");
    for l in &labels {
        if let ChartElement::Text {
            text_transform,
            letter_spacing,
            font_family,
            ..
        } = l
        {
            assert_eq!(
                text_transform.as_deref(),
                Some("uppercase"),
                "axis-label must carry text-transform=uppercase"
            );
            assert_eq!(
                letter_spacing.as_deref(),
                Some("1.2"),
                "axis-label must carry letter-spacing=1.2"
            );
            assert_eq!(
                font_family.as_deref(),
                Some("DM Sans, system-ui, sans-serif"),
                "axis-label must use the Kyomi label font"
            );
        }
    }
}

/// Check 7 — Numeric tick values use the monospace Kyomi numeric family.
/// tabular-nums is set in Phase 9 CSS (`.tick-value { font-variant-numeric:
/// tabular-nums }`), not as an SVG attribute — verified at the CSS layer.
#[test]
fn phase10_check7_tick_values_use_monospace_family() {
    let el = render_bar_crossing_zero();
    let mut ticks = Vec::new();
    collect(
        &el,
        &|e| matches!(e, ChartElement::Text { class, .. } if has_class(class, "tick-value")),
        &mut ticks,
    );
    assert!(!ticks.is_empty(), "expected at least one tick-value text");
    for t in &ticks {
        if let ChartElement::Text { font_family, .. } = t {
            assert_eq!(
                font_family.as_deref(),
                Some("Geist Mono, monospace"),
                "tick-value must use the Kyomi numeric font"
            );
        }
    }
}

/// Check 8 — Data that crosses zero emits exactly one `zero-line` Line
/// with the configured color and width.
#[test]
fn phase10_check8_zero_line_emitted_on_crossing_data() {
    let el = render_bar_crossing_zero();
    let mut zlines = Vec::new();
    collect(
        &el,
        &|e| matches!(e, ChartElement::Line { class, .. } if has_class(class, "zero-line")),
        &mut zlines,
    );
    assert_eq!(zlines.len(), 1, "expected exactly one zero-line");
    if let ChartElement::Line {
        stroke,
        stroke_width,
        ..
    } = zlines[0]
    {
        assert_eq!(stroke, "#1C1917");
        assert_eq!(*stroke_width, Some(1.5));
    }
}

/// Check 9 — `theme.bg` propagates as the "background-colored" stroke on
/// elements that rely on it for visual separation. ChartML does not emit a
/// dedicated chart-background `<rect>`, so "transparent chart background" is
/// enforced indirectly: with `theme.bg = ""` (the Kyomi value), every element
/// that reads `theme.bg` must emit that empty string.
///
/// Consumers of `theme.bg` (grepped across the workspace):
///   - `chartml-chart-cartesian/src/line.rs` — line-chart dot-marker stroke,
///     area-chart dot stroke, dual-axis line-overlay dot stroke
///   - `chartml-chart-cartesian/src/bar.rs`  — stacked-bar segment separator
///   - `chartml-chart-scatter/src/lib.rs`    — scatter dot-marker stroke
///   - `chartml-chart-pie/src/lib.rs`        — pie slice border stroke
///
/// The line-crossing-zero fixture already exercises the line-chart dot path,
/// so we assert every `dot-marker` circle carries `stroke == Some("")` — i.e.
/// the empty Kyomi `theme.bg` actually reached the render tree. This is a
/// true propagation check, not a vacuous scan.
#[test]
fn phase10_check9_theme_bg_propagates_to_dot_marker_stroke() {
    // Sanity: the theme we're rendering with really does use empty bg.
    assert_eq!(
        kyomi_theme().bg,
        "",
        "kyomi theme must declare bg as empty string"
    );

    let el = render_line_crossing_zero();
    let mut dots = Vec::new();
    collect(
        &el,
        &|e| matches!(e, ChartElement::Circle { class, .. } if has_class(class, "dot-marker")),
        &mut dots,
    );
    assert!(
        !dots.is_empty(),
        "expected at least one dot-marker circle to probe theme.bg propagation"
    );
    for d in &dots {
        if let ChartElement::Circle { stroke, .. } = d {
            assert_eq!(
                stroke.as_deref(),
                Some(""),
                "dot-marker stroke must echo theme.bg (empty string for Kyomi), got {stroke:?}"
            );
        }
    }

    // Belt-and-braces: with an empty-string bg in the theme, no element in
    // the tree should carry a literal white fill `#ffffff` — that would mean
    // some code path hardcoded white instead of honoring the theme.
    let mut white_rects = Vec::new();
    collect(
        &el,
        &|e| {
            matches!(
                e,
                ChartElement::Rect { fill, .. }
                    if fill.eq_ignore_ascii_case("#ffffff") || fill.eq_ignore_ascii_case("#fff")
            )
        },
        &mut white_rects,
    );
    assert!(
        white_rects.is_empty(),
        "found {} white-fill rect(s) — Kyomi theme expects transparent background",
        white_rects.len()
    );
}

// ---------------------------------------------------------------------------
// Phase 11 — text overlap acceptance gate.
//
// This is the gate the upstream consumer (Kyomi) needs to flip: when a chart
// is rendered with the aggressive Kyomi typography overrides (uppercase
// labels with letter-spacing, monospace numeric ticks, oversize serif
// title), the layout passes must not allow text to overlap inside any of
// the four label groups: tick labels, axis labels, legend items, chart
// titles.
//
// The check measures each rendered `<text>` element with the same theme-
// aware `measure_text` the layout uses, projects it to a screen-space bbox
// (handling text-anchor and -45° rotated tick labels), then asserts that
// no two boxes inside the same group intersect.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct TextBox {
    role: String,
    content: String,
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
}

fn parse_px(value: Option<&String>) -> Option<f64> {
    value.and_then(|s| s.trim_end_matches("px").parse::<f64>().ok())
}

fn metrics_from_text_attrs(
    font_size: Option<&String>,
    letter_spacing: Option<&String>,
    text_transform: Option<&String>,
    font_family: Option<&String>,
) -> TextMetrics {
    let mut m = TextMetrics::default();
    if let Some(px) = parse_px(font_size) {
        m.font_size_px = px;
    }
    if let Some(px) = parse_px(letter_spacing) {
        m.letter_spacing_px = px;
    }
    if let Some(t) = text_transform {
        m.text_transform = match t.as_str() {
            "uppercase" => TextTransform::Uppercase,
            "lowercase" => TextTransform::Lowercase,
            _ => TextTransform::None,
        };
    }
    if let Some(family) = font_family {
        let lower = family.to_ascii_lowercase();
        if lower.contains("mono") || lower.contains("menlo") || lower.contains("consolas")
            || lower.contains("ui-monospace") || lower.contains("courier")
        {
            m.monospace = true;
        }
    }
    m
}

fn role_for(class: &str) -> Option<&'static str> {
    if class.split_whitespace().any(|c| c == "tick-value") {
        Some("tick-value")
    } else if class.split_whitespace().any(|c| c == "tick-label") {
        Some("tick-label")
    } else if class.split_whitespace().any(|c| c == "legend-label") {
        Some("legend-label")
    } else if class.split_whitespace().any(|c| c == "axis-label") {
        Some("axis-label")
    } else if class.split_whitespace().any(|c| c == "chart-title") {
        Some("chart-title")
    } else {
        None
    }
}

/// Walk the element tree and return one `TextBox` per visible text element
/// whose class identifies it as a label/title role we want to police.
fn collect_text_boxes(root: &ChartElement) -> Vec<TextBox> {
    let mut texts = Vec::new();
    collect(root, &|e| matches!(e, ChartElement::Text { .. }), &mut texts);

    let mut boxes = Vec::new();
    for el in texts {
        let ChartElement::Text {
            x,
            y,
            content,
            anchor,
            transform,
            font_family,
            font_size,
            letter_spacing,
            text_transform,
            class,
            ..
        } = el
        else {
            continue;
        };
        let Some(role) = role_for(class) else {
            continue;
        };
        if content.is_empty() {
            continue;
        }
        let metrics = metrics_from_text_attrs(
            font_size.as_ref(),
            letter_spacing.as_ref(),
            text_transform.as_ref(),
            font_family.as_ref(),
        );
        let width = measure_text(content, &metrics);
        let height = metrics.font_size_px;

        // x range from anchor (unrotated baseline math).
        let (mut x_min, mut x_max) = match anchor {
            TextAnchor::Start => (*x, *x + width),
            TextAnchor::Middle => (*x - width / 2.0, *x + width / 2.0),
            TextAnchor::End => (*x - width, *x),
        };
        let (mut y_min, mut y_max) = (*y - height * 0.80, *y + height * 0.20);

        // For rotated tick labels (-45°), the horizontal projection shrinks
        // by cos(45°) and the vertical projection grows. Approximate the
        // axis-aligned bounding box of the rotated rect.
        if let Some(ElTransform::Rotate(angle, ..)) = transform {
            if (*angle - -45.0_f64).abs() < 1e-6 {
                let cos45 = std::f64::consts::FRAC_PI_4.cos();
                let sin45 = std::f64::consts::FRAC_PI_4.sin();
                let proj_w = width * cos45;
                let proj_h = width * sin45 + height;
                // Tick labels rotate around an end-anchored point at (x, y).
                x_min = *x - proj_w;
                x_max = *x;
                y_min = *y - height * 0.5;
                y_max = *y - height * 0.5 + proj_h;
            }
        }

        boxes.push(TextBox {
            role: role.to_string(),
            content: content.clone(),
            x_min,
            x_max,
            y_min,
            y_max,
        });
    }
    boxes
}

fn boxes_overlap(a: &TextBox, b: &TextBox) -> bool {
    // Allow a 0.25px slack so abutting glyph cells are not flagged.
    let pad = 0.25;
    a.x_min < b.x_max - pad
        && b.x_min < a.x_max - pad
        && a.y_min < b.y_max - pad
        && b.y_min < a.y_max - pad
}

fn assert_no_overlap_within_group(label: &str, boxes: &[TextBox]) {
    let mut by_role: std::collections::HashMap<&str, Vec<&TextBox>> =
        std::collections::HashMap::new();
    for b in boxes {
        by_role.entry(b.role.as_str()).or_default().push(b);
    }
    for (role, items) in &by_role {
        for i in 0..items.len() {
            for j in (i + 1)..items.len() {
                if boxes_overlap(items[i], items[j]) {
                    panic!(
                        "[{label}] text overlap in role '{role}': '{a}' ({a_x_min:.1}..{a_x_max:.1}, {a_y_min:.1}..{a_y_max:.1}) overlaps '{b}' ({b_x_min:.1}..{b_x_max:.1}, {b_y_min:.1}..{b_y_max:.1})",
                        a = items[i].content,
                        a_x_min = items[i].x_min,
                        a_x_max = items[i].x_max,
                        a_y_min = items[i].y_min,
                        a_y_max = items[i].y_max,
                        b = items[j].content,
                        b_x_min = items[j].x_min,
                        b_x_max = items[j].x_max,
                        b_y_min = items[j].y_min,
                        b_y_max = items[j].y_max,
                    );
                }
            }
        }
    }
}

const KYOMI_BAR_YAML: &str = r#"
type: chart
version: 1
title: "Monthly Revenue"
data:
  provider: inline
  rows:
    - month: "January"
      product: "Widgets"
      revenue: 12500
    - month: "January"
      product: "Gadgets"
      revenue: 8400
    - month: "February"
      product: "Widgets"
      revenue: 14200
    - month: "February"
      product: "Gadgets"
      revenue: 9100
    - month: "March"
      product: "Widgets"
      revenue: 16800
    - month: "March"
      product: "Gadgets"
      revenue: 9700
    - month: "April"
      product: "Widgets"
      revenue: 15400
    - month: "April"
      product: "Gadgets"
      revenue: 10500
    - month: "May"
      product: "Widgets"
      revenue: 17900
    - month: "May"
      product: "Gadgets"
      revenue: 11200
    - month: "June"
      product: "Widgets"
      revenue: 18600
    - month: "June"
      product: "Gadgets"
      revenue: 12100
visualize:
  type: bar
  columns: month
  rows: revenue
  marks:
    color: product
"#;

const KYOMI_LINE_YAML: &str = r#"
type: chart
version: 1
title: "Quarterly Trend"
data:
  provider: inline
  rows:
    - q: "Q1 2024"
      v: 1234567
    - q: "Q2 2024"
      v: 2345678
    - q: "Q3 2024"
      v: 3456789
    - q: "Q4 2024"
      v: 2987654
visualize:
  type: line
  columns: q
  rows: v
"#;

const KYOMI_SCATTER_YAML: &str = r#"
type: chart
version: 1
title: "Revenue vs Cost"
data:
  provider: inline
  rows:
    - revenue: 1000
      cost: 800
      category: "alpha bravo"
    - revenue: 2000
      cost: 1500
      category: "alpha bravo"
    - revenue: 3000
      cost: 2100
      category: "charlie delta"
    - revenue: 4000
      cost: 2800
      category: "charlie delta"
    - revenue: 5000
      cost: 3300
      category: "echo foxtrot"
    - revenue: 6000
      cost: 4000
      category: "echo foxtrot"
visualize:
  type: scatter
  columns: revenue
  rows: cost
  marks:
    color: category
"#;

const KYOMI_PIE_YAML: &str = r#"
type: chart
version: 1
title: "Market Share"
data:
  provider: inline
  rows:
    - segment: "Enterprise"
      revenue: 4500
    - segment: "Mid-Market"
      revenue: 3200
    - segment: "Small Business"
      revenue: 2100
    - segment: "Consumer"
      revenue: 1700
visualize:
  type: pie
  columns: segment
  rows: revenue
"#;

fn render_with_kyomi(yaml: &str) -> ChartElement {
    let chartml = new_chartml();
    chartml.render_from_yaml(yaml).expect("render under kyomi theme")
}

#[test]
fn phase11_kyomi_typography_no_overlap_bar() {
    let el = render_with_kyomi(KYOMI_BAR_YAML);
    let boxes = collect_text_boxes(&el);
    assert!(
        boxes.iter().any(|b| b.role == "tick-label"),
        "expected at least one tick label in bar render"
    );
    assert!(
        boxes.iter().any(|b| b.role == "legend-label"),
        "expected at least one legend label in bar render"
    );
    assert_no_overlap_within_group("bar", &boxes);
}

#[test]
fn phase11_kyomi_typography_no_overlap_line() {
    let el = render_with_kyomi(KYOMI_LINE_YAML);
    let boxes = collect_text_boxes(&el);
    assert_no_overlap_within_group("line", &boxes);
}

#[test]
fn phase11_kyomi_typography_no_overlap_scatter() {
    let el = render_with_kyomi(KYOMI_SCATTER_YAML);
    let boxes = collect_text_boxes(&el);
    assert!(
        boxes.iter().any(|b| b.role == "legend-label"),
        "expected legend labels in scatter render"
    );
    assert_no_overlap_within_group("scatter", &boxes);
}

#[test]
fn phase11_kyomi_typography_no_overlap_pie() {
    let el = render_with_kyomi(KYOMI_PIE_YAML);
    let boxes = collect_text_boxes(&el);
    assert_no_overlap_within_group("pie", &boxes);
}
