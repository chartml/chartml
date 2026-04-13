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
use chartml_chart_scatter::ScatterRenderer;
use chartml_core::element::{count_elements, ChartElement};
use chartml_core::theme::{GridStyle, TextTransform, Theme, ZeroLineSpec};
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
        bar_corner_radius: 2.0,
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
    c.register_renderer("scatter", ScatterRenderer::new());
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

/// Check 4 — Bars carry `rx == Some(2.0)` AND `ry == Some(2.0)`.
///
/// NOTE: the Phase 10 plan says "2px rounded top corners", but
/// `Theme::bar_corner_radius` is a uniform radius applied to all four corners.
/// This is a documented distinction: uniform rounding is what is wired through
/// the theme today. A top-only variant is out of scope for Phase 10.
#[test]
fn phase10_check4_bars_have_2px_uniform_rounding() {
    let el = render_bar_crossing_zero();
    let mut bars = Vec::new();
    collect(
        &el,
        &|e| matches!(e, ChartElement::Rect { class, .. } if has_class(class, "bar-rect")),
        &mut bars,
    );
    assert!(!bars.is_empty(), "expected at least one bar-rect");
    for b in &bars {
        if let ChartElement::Rect { rx, ry, .. } = b {
            assert_eq!(*rx, Some(2.0), "bar rx must be 2.0");
            assert_eq!(*ry, Some(2.0), "bar ry must be 2.0");
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
