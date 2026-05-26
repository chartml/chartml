//! Regression test for the bar entrance-animation `transform-origin`
//! contract (the third reintroduction of the same root cause is the
//! reason this file exists; do not delete it).
//!
//! Every bar element emitted by `chartml-chart-cartesian` — regardless of
//! orientation, value sign, or `BarCornerRadius` shape — must carry
//! `animation_origin: Some((ox, oy))` set to the bar's value-baseline
//! anchor:
//!
//! - vertical positive → bottom-center  (`x + w/2, y + h`)
//! - vertical negative → top-center     (`x + w/2, y`)
//! - horizontal positive → left-center  (`x, y + h/2`)
//! - horizontal negative → right-center (`x + w, y + h/2`)
//!
//! These four anchors hold under all three corner-radius shapes the
//! emitter supports:
//!   - `Uniform(0.0)`  → plain `<rect>`
//!   - `Uniform(r>0)`  → rounded `<rect>` (rx/ry set)
//!   - `Top(r>0)`      → `<path>` with two arcs on the value-end edge
//!
//! The emitter is `chartml_chart_cartesian::bar::build_bar_element`. The
//! anchor function is `bar_animation_origin` in the same module. A direct
//! unit test on `build_bar_element` would be tighter, but it's `pub(crate)`
//! — so we drive the public renderer with a hand-rolled `Theme` per shape
//! and walk the resulting element tree. If any future emission site forgets
//! to populate `animation_origin`, the per-bar `assert_eq!` below will
//! fail loudly.

use chartml_chart_cartesian::CartesianRenderer;
use chartml_core::element::ChartElement;
use chartml_core::theme::{BarCornerRadius, Theme};
use chartml_core::ChartML;

fn make_chartml(corner: &BarCornerRadius) -> ChartML {
    let mut c = ChartML::new();
    c.register_renderer("bar", CartesianRenderer::new());
    let mut theme = Theme::default();
    theme.bar_corner_radius = corner.clone();
    c.set_theme(theme);
    c
}

fn has_class(class: &str, name: &str) -> bool {
    class.split_whitespace().any(|c| c == name)
}

#[derive(Debug, Clone, Copy)]
struct BarBox {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    origin: Option<(f64, f64)>,
    /// "rect" or "path", to make assertion failure messages traceable
    /// back to which arm of `build_bar_element` produced the bar.
    tag: &'static str,
}

fn parse_path_bounds(d: &str) -> (f64, f64, f64, f64) {
    // Walk the SVG path command stream and accumulate every coordinate
    // pair *except* the leading `rx,ry` pair of each `A` command (which is
    // a radius, not a point on the curve). `build_bar_element` always
    // traces every corner of the rectangle, so min/max over the remaining
    // pairs equals the bar's bounding rect.
    let (mut min_x, mut min_y) = (f64::INFINITY, f64::INFINITY);
    let (mut max_x, mut max_y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    let tokens: Vec<&str> = d.split_ascii_whitespace().collect();
    let mut i = 0;
    while i < tokens.len() {
        let t = tokens[i];
        match t {
            "M" | "L" => {
                if let Some((xs, ys)) = tokens[i + 1].split_once(',') {
                    if let (Ok(x), Ok(y)) = (xs.parse::<f64>(), ys.parse::<f64>()) {
                        min_x = min_x.min(x);
                        min_y = min_y.min(y);
                        max_x = max_x.max(x);
                        max_y = max_y.max(y);
                    }
                }
                i += 2;
            }
            "A" => {
                // A rx,ry x-axis-rotation large-arc sweep x,y
                // We want only the final x,y pair (tokens[i+5]).
                if let Some((xs, ys)) = tokens[i + 5].split_once(',') {
                    if let (Ok(x), Ok(y)) = (xs.parse::<f64>(), ys.parse::<f64>()) {
                        min_x = min_x.min(x);
                        min_y = min_y.min(y);
                        max_x = max_x.max(x);
                        max_y = max_y.max(y);
                    }
                }
                i += 6;
            }
            "Z" => i += 1,
            _ => i += 1,
        }
    }
    (min_x, min_y, max_x - min_x, max_y - min_y)
}

fn collect_bars(el: &ChartElement) -> Vec<BarBox> {
    fn walk(el: &ChartElement, out: &mut Vec<BarBox>) {
        match el {
            ChartElement::Rect {
                x, y, width, height, class, animation_origin, ..
            } if has_class(class, "bar-rect") => {
                out.push(BarBox {
                    x: *x,
                    y: *y,
                    w: *width,
                    h: *height,
                    origin: *animation_origin,
                    tag: "rect",
                });
            }
            ChartElement::Path {
                class, animation_origin, d, ..
            } if has_class(class, "bar-rect") => {
                let (x, y, w, h) = parse_path_bounds(d);
                out.push(BarBox {
                    x,
                    y,
                    w,
                    h,
                    origin: *animation_origin,
                    tag: "path",
                });
            }
            ChartElement::Svg { children, .. }
            | ChartElement::Group { children, .. }
            | ChartElement::Div { children, .. } => {
                for c in children {
                    walk(c, out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(el, &mut out);
    out
}

#[derive(Debug, Clone, Copy)]
enum Direction {
    VerticalPositive,
    VerticalNegative,
    HorizontalPositive,
    HorizontalNegative,
}

impl Direction {
    fn expected_origin(self, b: &BarBox) -> (f64, f64) {
        match self {
            Direction::VerticalPositive => (b.x + b.w / 2.0, b.y + b.h),
            Direction::VerticalNegative => (b.x + b.w / 2.0, b.y),
            Direction::HorizontalPositive => (b.x, b.y + b.h / 2.0),
            Direction::HorizontalNegative => (b.x + b.w, b.y + b.h / 2.0),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Direction::VerticalPositive => "vertical+",
            Direction::VerticalNegative => "vertical-",
            Direction::HorizontalPositive => "horizontal+",
            Direction::HorizontalNegative => "horizontal-",
        }
    }
}

const VERTICAL_POSITIVE_YAML: &str = r#"
type: chart
version: 1
data:
  provider: inline
  rows:
    - cat: "A"
      v: 10
    - cat: "B"
      v: 25
    - cat: "C"
      v: 40
visualize:
  type: bar
  columns: cat
  rows: v
"#;

const VERTICAL_NEGATIVE_YAML: &str = r#"
type: chart
version: 1
data:
  provider: inline
  rows:
    - cat: "A"
      v: -10
    - cat: "B"
      v: -25
    - cat: "C"
      v: -40
visualize:
  type: bar
  columns: cat
  rows: v
"#;

const HORIZONTAL_POSITIVE_YAML: &str = r#"
type: chart
version: 1
data:
  provider: inline
  rows:
    - cat: "A"
      v: 10
    - cat: "B"
      v: 25
    - cat: "C"
      v: 40
visualize:
  type: bar
  orientation: horizontal
  columns: cat
  rows: v
"#;

const HORIZONTAL_NEGATIVE_YAML: &str = r#"
type: chart
version: 1
data:
  provider: inline
  rows:
    - cat: "A"
      v: -10
    - cat: "B"
      v: -25
    - cat: "C"
      v: -40
visualize:
  type: bar
  orientation: horizontal
  columns: cat
  rows: v
"#;

fn run_case(corner: &BarCornerRadius, direction: Direction, yaml: &str) {
    let chartml = make_chartml(corner);
    let el = chartml
        .render_from_yaml(yaml)
        .expect("render bar fixture under test");
    let bars = collect_bars(&el);
    assert!(
        !bars.is_empty(),
        "{:?} / {} — no bars emitted",
        corner,
        direction.label(),
    );
    for b in &bars {
        if b.w <= 0.0 || b.h <= 0.0 {
            // Degenerate zero-extent bars (e.g. value-zero categories).
            // They have no visible animation, so the origin doesn't matter.
            continue;
        }
        let origin = b.origin.unwrap_or_else(|| {
            panic!(
                "{:?} / {} — bar tag={} x={} y={} w={} h={} has \
                 animation_origin == None. Every bar emission site MUST \
                 populate this field via build_bar_element. See \
                 chartml-chart-cartesian/src/bar.rs.",
                corner,
                direction.label(),
                b.tag,
                b.x,
                b.y,
                b.w,
                b.h,
            )
        });
        let (ex, ey) = direction.expected_origin(b);
        let dx = (origin.0 - ex).abs();
        let dy = (origin.1 - ey).abs();
        assert!(
            dx < 1e-6 && dy < 1e-6,
            "{:?} / {} — bar tag={} x={} y={} w={} h={} expected \
             origin ({}, {}), got ({}, {}). The animation will grow from \
             the wrong anchor — see bar_animation_origin in \
             chartml-chart-cartesian/src/bar.rs.",
            corner,
            direction.label(),
            b.tag,
            b.x,
            b.y,
            b.w,
            b.h,
            ex,
            ey,
            origin.0,
            origin.1,
        );
    }
}

fn corner_variants() -> [BarCornerRadius; 3] {
    [
        BarCornerRadius::Uniform(0.0),
        BarCornerRadius::Uniform(4.0),
        BarCornerRadius::Top(4.0),
    ]
}

#[test]
fn bar_animation_origin_vertical_positive() {
    for corner in corner_variants().iter() {
        run_case(corner, Direction::VerticalPositive, VERTICAL_POSITIVE_YAML);
    }
}

#[test]
fn bar_animation_origin_vertical_negative() {
    for corner in corner_variants().iter() {
        run_case(corner, Direction::VerticalNegative, VERTICAL_NEGATIVE_YAML);
    }
}

#[test]
fn bar_animation_origin_horizontal_positive() {
    for corner in corner_variants().iter() {
        run_case(corner, Direction::HorizontalPositive, HORIZONTAL_POSITIVE_YAML);
    }
}

#[test]
fn bar_animation_origin_horizontal_negative() {
    for corner in corner_variants().iter() {
        run_case(corner, Direction::HorizontalNegative, HORIZONTAL_NEGATIVE_YAML);
    }
}

// ---------------------------------------------------------------------------
// Stacked bar animation origin tests
//
// In stacked bar charts, ALL bar segments in a column must share the same
// axis-baseline animation origin so the stack grows uniformly from the axis.
// The per-segment `bar_animation_origin` would give each segment its own
// edge, causing upper segments to appear to float.
// ---------------------------------------------------------------------------

const STACKED_VERTICAL_YAML: &str = r#"
type: chart
version: 1
data:
  provider: inline
  rows:
    - cat: "A"
      v: 10
      g: "X"
    - cat: "A"
      v: 20
      g: "Y"
    - cat: "B"
      v: 15
      g: "X"
    - cat: "B"
      v: 25
      g: "Y"
visualize:
  type: bar
  mode: stacked
  columns: cat
  rows: v
  marks:
    color: g
"#;

const STACKED_HORIZONTAL_YAML: &str = r#"
type: chart
version: 1
data:
  provider: inline
  rows:
    - cat: "A"
      v: 10
      g: "X"
    - cat: "A"
      v: 20
      g: "Y"
    - cat: "B"
      v: 15
      g: "X"
    - cat: "B"
      v: 25
      g: "Y"
visualize:
  type: bar
  mode: stacked
  orientation: horizontal
  columns: cat
  rows: v
  marks:
    color: g
"#;

/// Assert that every bar in a stacked chart shares the same axis-baseline
/// animation origin coordinate (y for vertical, x for horizontal).
fn run_stacked_case(corner: &BarCornerRadius, yaml: &str, is_horizontal: bool) {
    let chartml = make_chartml(corner);
    let el = chartml
        .render_from_yaml(yaml)
        .expect("render stacked bar fixture under test");
    let bars = collect_bars(&el);
    assert!(
        !bars.is_empty(),
        "{:?} / stacked {} — no bars emitted",
        corner,
        if is_horizontal { "horizontal" } else { "vertical" },
    );

    // Filter out degenerate zero-extent bars.
    let real_bars: Vec<&BarBox> = bars.iter().filter(|b| b.w > 0.0 && b.h > 0.0).collect();
    assert!(
        real_bars.len() >= 2,
        "{:?} / stacked {} — need at least 2 non-degenerate bars for \
         a stacked test, got {}",
        corner,
        if is_horizontal { "horizontal" } else { "vertical" },
        real_bars.len(),
    );

    // Every bar must have an animation_origin.
    for b in &real_bars {
        assert!(
            b.origin.is_some(),
            "{:?} / stacked {} — bar tag={} x={} y={} w={} h={} has \
             animation_origin == None",
            corner,
            if is_horizontal { "horizontal" } else { "vertical" },
            b.tag, b.x, b.y, b.w, b.h,
        );
    }

    if is_horizontal {
        // All bars must share the same x-origin (the y-axis baseline).
        let baseline_x = real_bars[0].origin.unwrap().0;
        for b in &real_bars {
            let ox = b.origin.unwrap().0;
            assert!(
                (ox - baseline_x).abs() < 1e-6,
                "{:?} / stacked horizontal — bar tag={} x={} y={} w={} h={} \
                 has origin.x = {}, expected shared baseline {}. Upper \
                 segments must NOT animate from their own left edge.",
                corner, b.tag, b.x, b.y, b.w, b.h, ox, baseline_x,
            );
        }

        // The y-component of the origin must still be each bar's own center.
        for b in &real_bars {
            let oy = b.origin.unwrap().1;
            let expected_y = b.y + b.h / 2.0;
            assert!(
                (oy - expected_y).abs() < 1e-6,
                "{:?} / stacked horizontal — bar y-origin {} != bar center {}",
                corner, oy, expected_y,
            );
        }

        // Verify this is NOT the per-segment edge: at least one upper
        // segment must differ from what bar_animation_origin would produce.
        let any_differs = real_bars.iter().any(|b| {
            let per_segment_x = b.x; // horizontal positive: left edge
            (b.origin.unwrap().0 - per_segment_x).abs() > 1e-6
        });
        assert!(
            any_differs,
            "{:?} / stacked horizontal — all bars have origin.x == their \
             own left edge; the stack_baseline fix is not working",
            corner,
        );
    } else {
        // All bars must share the same y-origin (the x-axis baseline).
        let baseline_y = real_bars[0].origin.unwrap().1;
        for b in &real_bars {
            let oy = b.origin.unwrap().1;
            assert!(
                (oy - baseline_y).abs() < 1e-6,
                "{:?} / stacked vertical — bar tag={} x={} y={} w={} h={} \
                 has origin.y = {}, expected shared baseline {}. Upper \
                 segments must NOT animate from their own bottom edge.",
                corner, b.tag, b.x, b.y, b.w, b.h, oy, baseline_y,
            );
        }

        // The x-component of the origin must still be each bar's own center.
        for b in &real_bars {
            let ox = b.origin.unwrap().0;
            let expected_x = b.x + b.w / 2.0;
            assert!(
                (ox - expected_x).abs() < 1e-6,
                "{:?} / stacked vertical — bar x-origin {} != bar center {}",
                corner, ox, expected_x,
            );
        }

        // Verify this is NOT the per-segment edge: at least one upper
        // segment must differ from what bar_animation_origin would produce.
        let any_differs = real_bars.iter().any(|b| {
            let per_segment_y = b.y + b.h; // vertical positive: bottom edge
            (b.origin.unwrap().1 - per_segment_y).abs() > 1e-6
        });
        assert!(
            any_differs,
            "{:?} / stacked vertical — all bars have origin.y == their \
             own bottom edge; the stack_baseline fix is not working",
            corner,
        );
    }
}

#[test]
fn bar_animation_origin_stacked_vertical() {
    for corner in corner_variants().iter() {
        run_stacked_case(corner, STACKED_VERTICAL_YAML, false);
    }
}

#[test]
fn bar_animation_origin_stacked_horizontal() {
    for corner in corner_variants().iter() {
        run_stacked_case(corner, STACKED_HORIZONTAL_YAML, true);
    }
}
