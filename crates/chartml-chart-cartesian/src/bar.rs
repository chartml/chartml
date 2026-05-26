use chartml_core::data::DataTable;
use chartml_core::element::{ChartElement, ElementData, TextAnchor, TextRole, TextStyle, Transform, ViewBox, emit_dot_halo_if_enabled};
use chartml_core::error::ChartError;
use chartml_core::layout::margins::{calculate_margins, MarginConfig};
use chartml_core::plugin::ChartConfig;
use chartml_core::scales::{ScaleBand, ScaleLinear};
use chartml_core::layout::adaptive_tick_count;
use chartml_core::spec::{ChartMode, Orientation};

use chartml_core::layout::labels::{LabelStrategy, LabelStrategyConfig, TextMetrics, measure_text};

use chartml_core::layout::legend::{calculate_legend_layout, LegendConfig};

use crate::helpers::{GridConfig, emit_zero_line_if_crosses, format_value, generate_annotations, generate_x_axis, generate_x_axis_numeric, generate_x_axis_with_display, generate_y_axis_with_display, generate_y_axis_numeric, generate_y_axis_numeric_right, generate_legend, get_color_field, get_data_labels_config, get_field_name, get_x_format, get_y_axis_bounds, get_y_format, nice_domain, offset_element};

/// Build a single bar element, honoring `theme.bar_corner_radius`.
///
/// Decision tree:
/// - `BarCornerRadius::Uniform(0.0)` or `Top(0.0)` → emit a plain
///   `ChartElement::Rect` with `rx`/`ry` == `None` (byte-identical to the
///   pre-3.1 un-themed output).
/// - `BarCornerRadius::Uniform(r)` with `r > 0.0` → emit `Rect` with
///   `rx = ry = Some(r)`.
/// - `BarCornerRadius::Top(r)` with `r > 0.0` → emit a `ChartElement::Path`
///   with a `d` string that rounds only the two corners at the max-value
///   end of the bar (the top of a vertical positive bar, the bottom of a
///   vertical negative bar, the right end of a horizontal positive bar,
///   the left end of a horizontal negative bar). The radius is clamped to
///   `min(width, height) / 2.0` to prevent degenerate paths.
pub(crate) struct BarRectSpec {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub is_horizontal: bool,
    pub is_negative: bool,
    pub fill: String,
    pub class: String,
    pub data: Option<ElementData>,
    /// For stacked bars, the shared axis baseline coordinate.
    /// When `Some(baseline)`, the animation origin uses this instead of the
    /// segment's own edge so the entire stack grows uniformly from the axis.
    pub stack_baseline: Option<f64>,
}

/// Compute the CSS `transform-origin` anchor for a bar's entrance animation.
///
/// The anchor is the bar's value-baseline edge midpoint, in absolute SVG
/// coordinates, so a `scaleX`/`scaleY` keyframe grows the bar from the axis
/// outward toward its value end:
///
/// - vertical, positive value  → bottom-center  (grows up)
/// - vertical, negative value  → top-center     (grows down)
/// - horizontal, positive value → left-center   (grows right)
/// - horizontal, negative value → right-center  (grows left)
///
/// Computing the anchor at emission time is essential: the renderer cannot
/// recover orientation/sign from `<rect>`/`<path>` geometry alone (the
/// historical `width > height` heuristic guessed wrong for square bars and
/// for any negative bar). See `chartml-leptos/src/element.rs`, where the
/// heuristic was deleted in favor of consuming this value.
pub fn bar_animation_origin(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    is_horizontal: bool,
    is_negative: bool,
) -> (f64, f64) {
    match (is_horizontal, is_negative) {
        // Vertical positive: rect spans [y, y+h]; baseline is y+h (bottom).
        (false, false) => (x + width / 2.0, y + height),
        // Vertical negative: rect spans [y, y+h] BELOW zero line; baseline is y (top).
        (false, true) => (x + width / 2.0, y),
        // Horizontal positive: rect spans [x, x+w]; baseline is x (left).
        (true, false) => (x, y + height / 2.0),
        // Horizontal negative: rect spans [x, x+w] LEFT of zero line; baseline is x+w (right).
        (true, true) => (x + width, y + height / 2.0),
    }
}

pub(crate) fn build_bar_element(
    spec: BarRectSpec,
    theme: &chartml_core::theme::Theme,
) -> ChartElement {
    use chartml_core::theme::BarCornerRadius;
    let BarRectSpec {
        x, y, width, height, is_horizontal, is_negative, fill, class, data,
        stack_baseline,
    } = spec;
    let anim_origin = if let Some(baseline) = stack_baseline {
        // Stacked bars: all segments share the axis baseline so the
        // stack grows uniformly from the axis instead of each segment
        // animating from its own edge.
        if is_horizontal {
            Some((baseline, y + height / 2.0))
        } else {
            Some((x + width / 2.0, baseline))
        }
    } else {
        Some(bar_animation_origin(x, y, width, height, is_horizontal, is_negative))
    };

    // Extract requested radius; short-circuit the zero case to emit a plain
    // Rect (byte-identical contract).
    let (radius, top_only) = match theme.bar_corner_radius {
        BarCornerRadius::Uniform(r) => (r as f64, false),
        BarCornerRadius::Top(r) => (r as f64, true),
    };

    if radius <= 0.0 {
        return ChartElement::Rect {
            x,
            y,
            width,
            height,
            fill,
            stroke: None,
            rx: None,
            ry: None,
            class,
            data,
            animation_origin: anim_origin,
        };
    }

    if !top_only {
        return ChartElement::Rect {
            x,
            y,
            width,
            height,
            fill,
            stroke: None,
            rx: Some(radius),
            ry: Some(radius),
            class,
            data,
            animation_origin: anim_origin,
        };
    }

    // Top-only rounding: emit a Path with custom d.
    // Clamp radius to min(w,h)/2 to prevent degenerate geometry on very
    // thin bars. debug_assert flags regressions in tests.
    let max_r = (width.min(height) / 2.0).max(0.0);
    debug_assert!(
        radius <= max_r + 1e-9 || width <= 0.0 || height <= 0.0,
        "bar_corner_radius {} exceeds min(w,h)/2 = {} (w={}, h={})",
        radius, max_r, width, height
    );
    let r = radius.min(max_r);

    // Degenerate zero-dimension bars (e.g. a value-at-zero bar that has
    // height 0 on vertical orientation) collapse to a plain Rect. Emitting
    // an arc of radius 0 would pollute the path string and confuse
    // consumers.
    if r <= 0.0 {
        return ChartElement::Rect {
            x,
            y,
            width,
            height,
            fill,
            stroke: None,
            rx: None,
            ry: None,
            class,
            data,
            animation_origin: anim_origin,
        };
    }

    // Absolute coordinates of the rect corners.
    let x0 = x;
    let y0 = y;
    let x1 = x + width;
    let y1 = y + height;

    // Which two corners get rounded:
    //   vertical + !negative → top two   (y0 edge)
    //   vertical +  negative → bottom two (y1 edge)
    //   horizontal + !negative → right two (x1 edge)
    //   horizontal +  negative → left two  (x0 edge)
    //
    // Path is always traced clockwise starting from the corner immediately
    // counter-clockwise of the first rounded corner, so the arc sweep flag
    // is always 1 (clockwise).
    let d = match (is_horizontal, is_negative) {
        // Vertical, top rounding (two corners at y0)
        (false, false) => format!(
            "M {x0},{y0r} A {r},{r} 0 0 1 {x0r},{y0} L {x1mr},{y0} A {r},{r} 0 0 1 {x1},{y0r} L {x1},{y1} L {x0},{y1} Z",
            x0 = x0, y0 = y0, x1 = x1, y1 = y1, r = r,
            x0r = x0 + r, x1mr = x1 - r, y0r = y0 + r,
        ),
        // Vertical, negative value → bottom rounding (two corners at y1)
        (false, true) => format!(
            "M {x0},{y0} L {x1},{y0} L {x1},{y1mr} A {r},{r} 0 0 1 {x1mr},{y1} L {x0r},{y1} A {r},{r} 0 0 1 {x0},{y1mr} Z",
            x0 = x0, y0 = y0, x1 = x1, y1 = y1, r = r,
            x0r = x0 + r, x1mr = x1 - r, y1mr = y1 - r,
        ),
        // Horizontal, positive value → right-end rounding (two corners at x1)
        (true, false) => format!(
            "M {x0},{y0} L {x1mr},{y0} A {r},{r} 0 0 1 {x1},{y0r} L {x1},{y1mr} A {r},{r} 0 0 1 {x1mr},{y1} L {x0},{y1} Z",
            x0 = x0, y0 = y0, x1 = x1, y1 = y1, r = r,
            x1mr = x1 - r, y0r = y0 + r, y1mr = y1 - r,
        ),
        // Horizontal, negative value → left-end rounding (two corners at x0)
        (true, true) => format!(
            "M {x0r},{y0} L {x1},{y0} L {x1},{y1} L {x0r},{y1} A {r},{r} 0 0 1 {x0},{y1mr} L {x0},{y0r} A {r},{r} 0 0 1 {x0r},{y0} Z",
            x0 = x0, y0 = y0, x1 = x1, y1 = y1, r = r,
            x0r = x0 + r, y0r = y0 + r, y1mr = y1 - r,
        ),
    };

    ChartElement::Path {
        d,
        fill: Some(fill),
        stroke: None,
        stroke_width: None,
        stroke_dasharray: None,
        stroke_dashoffset: None,
        opacity: None,
        class,
        data,
        animation_origin: anim_origin,
    }
}

struct SingleSeriesBarParams<'a> {
    category_field: &'a str,
    value_field: &'a str,
    categories: &'a [String],
    inner_width: f64,
    inner_height: f64,
    is_horizontal: bool,
    y_fmt_ref: Option<&'a str>,
    domain_min: f64,
    domain_max: f64,
}

struct MultiSeriesBarParams<'a> {
    category_field: &'a str,
    value_field: &'a str,
    color_field: &'a str,
    categories: &'a [String],
    inner_width: f64,
    inner_height: f64,
    is_stacked: bool,
    is_normalized: bool,
    is_horizontal: bool,
    y_fmt_ref: Option<&'a str>,
    domain_min: f64,
    domain_max: f64,
}

pub fn render_bar(data: &DataTable, config: &ChartConfig) -> Result<ChartElement, ChartError> {
    use chartml_core::spec::{FieldRef, FieldRefItem, FieldSpec};
    

    // Detect multi-field rows (combo chart pattern)
    let multi_fields: Vec<FieldSpec> = match &config.visualize.rows {
        Some(FieldRef::Multiple(items)) => items.iter().map(|item| match item {
            FieldRefItem::Detailed(spec) => spec.as_ref().clone(),
            FieldRefItem::Simple(name) => FieldSpec {
                field: Some(name.clone()), mark: None, axis: None, label: None,
                color: None, format: None, data_labels: None,
                line_style: None, upper: None, lower: None, opacity: None,
            },
        }).collect(),
        _ => vec![],
    };

    if !multi_fields.is_empty() {
        let is_horizontal = matches!(config.visualize.orientation, Some(Orientation::Horizontal));
        let has_line_fields = multi_fields.iter().any(|f| f.mark.as_deref() == Some("line"));
        let has_right_axis = multi_fields.iter().any(|f| f.axis.as_deref() == Some("right"));

        // When horizontal with only bar fields (no lines/right axis), delegate to the
        // standard bar renderer which already supports horizontal layout. The combo path
        // only handles vertical because swapping axes for lines doesn't make sense.
        if is_horizontal && !has_line_fields && !has_right_axis {
            // Build a grouped bar config: use color field to separate the bar fields.
            // Reshape wide-format data (revenue+target columns) into long-format
            // (field_name column + value column) so the standard grouped bar path handles it.
            let category_field = get_field_name(&config.visualize.columns)?;
            let mut long_rows: Vec<chartml_core::data::Row> = Vec::new();
            for i in 0..data.num_rows() {
                for field_spec in &multi_fields {
                    // Range marks have no `field` — they shade an area between
                    // `upper` and `lower`. Horizontal bar charts have no notion
                    // of a shaded band, so skip them here (JS chartml does the
                    // same; range marks are line-chart only).
                    if field_spec.mark.as_deref() == Some("range") {
                        continue;
                    }
                    let Some(field_name) = field_spec.field.as_deref() else { continue };
                    let cat = data.get_string(i, &category_field).unwrap_or_default();
                    let val = data.get_f64(i, field_name).unwrap_or(0.0);
                    let label = field_spec.label.clone().unwrap_or_else(|| field_name.to_string());
                    let mut row = std::collections::HashMap::new();
                    row.insert(category_field.clone(), serde_json::json!(cat));
                    row.insert("_value".to_string(), serde_json::json!(val));
                    row.insert("_series".to_string(), serde_json::json!(label));
                    long_rows.push(row);
                }
            }
            let long_data = DataTable::from_rows(&long_rows)
                .map_err(|e| ChartError::DataError(format!("Failed to reshape data: {}", e)))?;

            // Build a config that uses the long-format columns
            let mut viz = config.visualize.clone();
            viz.rows = Some(FieldRef::Simple("_value".to_string()));
            viz.marks = Some(chartml_core::spec::MarksSpec {
                color: Some(chartml_core::spec::MarkEncoding::Simple("_series".to_string())),
                size: None, shape: None, text: None,
            });
            viz.mode = Some(ChartMode::Grouped);
            // Assign colors from field specs or config palette
            let mut colors = Vec::new();
            for (i, f) in multi_fields.iter().enumerate() {
                colors.push(f.color.clone().unwrap_or_else(|| {
                    config.colors.get(i).cloned().unwrap_or_else(|| "#2E7D9A".to_string())
                }));
            }
            let long_config = ChartConfig {
                visualize: viz,
                title: config.title.clone(),
                width: config.width,
                height: config.height,
                colors,
                theme: config.theme.clone(),
            };
            return render_bar(&long_data, &long_config);
        }

        return render_combo(data, config, &multi_fields);
    }

    let category_field = get_field_name(&config.visualize.columns)?;
    let value_field = get_field_name(&config.visualize.rows)?;

    let color_field = get_color_field(config);

    // For single-series bars (no color field), use per-row categories to support
    // duplicate category names. Each row gets a unique band key for positioning,
    // while display_labels preserves the original (possibly duplicate) text.
    // For multi-series (with color field), use unique categories as before since
    // stacking/grouping logic depends on deduplication.
    let (categories, display_labels): (Vec<String>, Option<Vec<String>>) = if color_field.is_none() {
        let all_vals = data.all_values(&category_field);
        if all_vals.is_empty() {
            return Err(ChartError::DataError("No category values found".into()));
        }
        // Check if there are any duplicates; if so, create indexed band keys
        let has_duplicates = {
            let mut seen = std::collections::HashSet::new();
            all_vals.iter().any(|v| !seen.insert(v.as_str()))
        };
        if has_duplicates {
            let band_keys: Vec<String> = all_vals.iter().enumerate()
                .map(|(i, v)| format!("{}\x00{}", v, i))
                .collect();
            (band_keys, Some(all_vals))
        } else {
            (all_vals, None)
        }
    } else {
        let unique = data.unique_values(&category_field);
        if unique.is_empty() {
            return Err(ChartError::DataError("No category values found".into()));
        }
        (unique, None)
    };

    let is_horizontal = matches!(config.visualize.orientation, Some(Orientation::Horizontal));
    let is_normalized = matches!(config.visualize.mode, Some(ChartMode::Normalized));
    let is_stacked = matches!(config.visualize.mode, Some(ChartMode::Stacked)) || is_normalized;
    let _is_grouped = matches!(config.visualize.mode, Some(ChartMode::Grouped));

    // Step 1: Compute label strategy for margin estimation (only for vertical bars)
    let x_format = get_x_format(config);
    let y_fmt = get_y_format(config);
    let y_fmt_ref = y_fmt.as_deref();
    let (axis_min, axis_max) = get_y_axis_bounds(config);

    // Format labels the same way generate_x_axis will — margin estimation must
    // use the actual display strings, not raw data values.
    let raw_for_strategy = display_labels.as_deref().unwrap_or(&categories);
    let formatted_for_strategy = crate::helpers::format_display_labels(raw_for_strategy, x_format.as_deref());
    let x_extra_margin = if !is_horizontal {
        let estimated_width = config.width - 80.0;
        let label_strategy_config = LabelStrategyConfig {
            text_metrics: TextMetrics::from_theme_axis_label(&config.theme),
            ..LabelStrategyConfig::default()
        };
        let x_strategy = LabelStrategy::determine(&formatted_for_strategy, estimated_width, &label_strategy_config);
        match &x_strategy {
            LabelStrategy::Rotated { margin, .. } => *margin,
            _ => 0.0,
        }
    } else {
        0.0
    };

    // Step 1b: Pre-compute domain for left margin estimation (matches JS two-pass approach).
    // JS computes finalMarginLeft from actual y-axis tick label widths; we approximate here.
    let (prelim_data_min, prelim_data_max): (f64, f64) = if let Some(ref color_f) = color_field {
        if is_stacked {
            let groups = data.group_by(color_f);
            let series_names = data.unique_values(color_f);
            let stacked_vals: Vec<f64> = categories.iter().map(|cat| {
                series_names.iter().map(|s| {
                    groups.get(s).and_then(|series_data| {
                        (0..series_data.num_rows()).find_map(|i| {
                            if series_data.get_string(i, &category_field).as_deref() == Some(cat.as_str()) {
                                series_data.get_f64(i, &value_field)
                            } else {
                                None
                            }
                        })
                    }).unwrap_or(0.0)
                }).sum::<f64>()
            }).collect();
            let mn = stacked_vals.iter().cloned().fold(f64::INFINITY, f64::min);
            let mx = stacked_vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            (mn, mx)
        } else {
            let vals: Vec<f64> = (0..data.num_rows()).filter_map(|i| data.get_f64(i, &value_field)).collect();
            let mn = vals.iter().cloned().fold(f64::INFINITY, f64::min);
            let mx = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            (mn, mx)
        }
    } else {
        let vals: Vec<f64> = (0..data.num_rows()).filter_map(|i| data.get_f64(i, &value_field)).collect();
        let mn = vals.iter().cloned().fold(f64::INFINITY, f64::min);
        let mx = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        (mn, mx)
    };
    let prelim_data_max = if prelim_data_max <= 0.0 { 1.0 } else { prelim_data_max };
    // Keep data_min at 0 when all values are non-negative (standard bar chart behavior)
    let prelim_data_min = if prelim_data_min >= 0.0 { 0.0 } else { prelim_data_min };

    let prelim_domain_max = if is_normalized {
        1.0
    } else {
        let raw_max = axis_max.unwrap_or(prelim_data_max);
        if axis_max.is_none() { nice_domain(axis_min.unwrap_or(prelim_data_min), raw_max, 5).1 } else { raw_max }
    };
    let prelim_domain_min = if is_normalized { 0.0 } else { axis_min.unwrap_or(prelim_data_min) };

    // Generate representative tick label (domain max is typically widest label).
    // For horizontal charts the y-axis shows categories, so use those for left margin.
    let y_tick_labels_for_margin: Vec<String> = if !is_horizontal {
        let prelim_fmt = if is_normalized { Some(".0%") } else { y_fmt_ref };
        vec![
            format_value(prelim_domain_max, prelim_fmt),
            format_value(prelim_domain_min, prelim_fmt),
        ]
    } else {
        let display = display_labels.as_deref().unwrap_or(&categories);
        display.to_vec()
    };

    // Pre-compute legend height so the bottom margin accounts for multi-row legends.
    let legend_height = if let Some(ref color_f) = color_field {
        let series_names = data.unique_values(color_f);
        let legend_config = LegendConfig {
            text_metrics: TextMetrics::from_theme_legend(&config.theme),
            ..LegendConfig::default()
        };
        calculate_legend_layout(&series_names, &config.colors, config.width, &legend_config).total_height
    } else {
        0.0
    };

    // Step 2: Calculate margins including rotation
    let has_x_axis_label = config.visualize.axes.as_ref()
        .and_then(|a| a.x.as_ref())
        .and_then(|a| a.label.as_ref())
        .is_some();
    let has_y_axis_label = config.visualize.axes.as_ref()
        .and_then(|a| a.left.as_ref())
        .and_then(|a| a.label.as_ref())
        .is_some();
    let margin_config = MarginConfig {
        has_title: config.title.is_some(),
        legend_height,
        has_x_axis_label,
        has_y_axis_label,
        x_label_strategy_margin: x_extra_margin,
        y_tick_labels: y_tick_labels_for_margin,
        // For horizontal charts the Y-axis displays category labels (axis
        // label metrics); for vertical charts it shows numeric tick values.
        tick_value_metrics: if is_horizontal {
            TextMetrics::from_theme_axis_label(&config.theme)
        } else {
            TextMetrics::from_theme_tick_value(&config.theme)
        },
        axis_label_metrics: TextMetrics::from_theme_axis_label(&config.theme),
        ..Default::default()
    };
    let margins = calculate_margins(&margin_config);

    let inner_width = margins.inner_width(config.width);
    let inner_height = margins.inner_height(config.height);

    let mut children = Vec::new();

    // Title is rendered as an HTML div outside the SVG (matches JS chartml behaviour)
    // — do NOT add it here as a SVG text element.

    let grid = GridConfig::from_config(config);

    let _tick_count = adaptive_tick_count(inner_height);

    // Compute final domain (same as prelim for vertical bars).
    let raw_data_max = prelim_data_max;

    // For normalized mode, domain is always 0-1 (the NumberFormatter handles % display).
    let (domain_min, domain_max) = if is_normalized {
        (0.0, 1.0)
    } else {
        let raw_domain_min = axis_min.unwrap_or(prelim_data_min);
        let raw_domain_max = axis_max.unwrap_or(raw_data_max);
        // Apply nice rounding to domain so ticks are round numbers with headroom (Regressions 2 & 3).
        // Only apply when no explicit axis bounds are set by the user.
        if axis_min.is_none() && axis_max.is_none() {
            // Match JS: yLeft.nice() uses default count=10 for domain rounding.
            nice_domain(raw_domain_min, raw_domain_max, 5)
        } else {
            (raw_domain_min, raw_domain_max)
        }
    };
    // For normalized mode, override Y-axis format to show percentages.
    // Otherwise, use the format from config (axes.left.format).
    let effective_y_fmt: Option<String> = if is_normalized {
        Some(".0%".to_string())
    } else {
        y_fmt.clone()
    };
    let effective_y_fmt_ref = effective_y_fmt.as_deref();

    let (_, bar_elements) = if let Some(ref color_f) = color_field {
        render_multi_series_bars(
            data,
            config,
            &MultiSeriesBarParams {
                category_field: &category_field,
                value_field: &value_field,
                color_field: color_f,
                categories: &categories,
                inner_width,
                inner_height,
                is_stacked,
                is_normalized,
                is_horizontal,
                y_fmt_ref,
                domain_min,
                domain_max,
            },
        )?
    } else {
        render_single_series_bars(
            data,
            config,
            &SingleSeriesBarParams {
                category_field: &category_field,
                value_field: &value_field,
                categories: &categories,
                inner_width,
                inner_height,
                is_horizontal,
                y_fmt_ref,
                domain_min,
                domain_max,
            },
        )?
    };

    // Axes (use domain_min/domain_max instead of 0.0/value_max)
    let axis_elements = if is_horizontal {
        // Category y-axis: generate at x=0 relative, then offset by margins.left
        let x_axis = generate_y_axis_with_display(&categories, display_labels.as_deref(), (0.0, inner_height), 0.0, None, &config.theme);
        let y_axis = generate_x_axis_numeric(&crate::helpers::XAxisNumericParams {
            domain: (domain_min, domain_max),
            range: (0.0, inner_width),
            y_position: margins.top + inner_height,
            fmt: effective_y_fmt_ref,
            tick_count: 5,
            chart_height: Some(inner_height),
            grid: &grid,
            theme: &config.theme,
        });
        let mut axes = Vec::new();
        axes.extend(x_axis.into_iter().map(|e| offset_element(e, margins.left, margins.top)));
        axes.extend(y_axis.into_iter().map(|e| offset_element(e, margins.left, 0.0)));
        // Zero-line (Phase 7): for horizontal bars the numeric axis is x,
        // so the zero line is vertical — emitted here after axes and before
        // the series group below. No-op when theme.zero_line is None or the
        // x-domain doesn't strictly cross zero.
        if let Some(zl) = emit_zero_line_if_crosses(
            &config.theme,
            (domain_min, domain_max),
            inner_width,
            inner_height,
            true,
        ) {
            axes.push(offset_element(zl, margins.left, margins.top));
        }
        axes
    } else {
        let bottom_axis_label = config.visualize.axes.as_ref()
            .and_then(|a| a.x.as_ref())
            .and_then(|a| a.label.as_deref());
        let x_axis_result = generate_x_axis_with_display(&crate::helpers::XAxisParams {
            labels: &categories,
            display_label_overrides: display_labels.as_deref(),
            range: (0.0, inner_width),
            y_position: margins.top + inner_height,
            available_width: inner_width,
            x_format: x_format.as_deref(),
            chart_height: Some(inner_height),
            grid: &grid,
            axis_label: bottom_axis_label,
            theme: &config.theme,
        });
        let left_axis_label = config.visualize.axes.as_ref()
            .and_then(|a| a.left.as_ref())
            .and_then(|a| a.label.as_deref());
        let y_axis = generate_y_axis_numeric(&crate::helpers::YAxisNumericParams {
            domain: (domain_min, domain_max),
            range: (inner_height, 0.0),
            x_position: margins.left,
            fmt: effective_y_fmt_ref,
            tick_count: adaptive_tick_count(inner_height),
            chart_width: Some(inner_width),
            grid: &grid,
            axis_label: left_axis_label,
            theme: &config.theme,
        });
        let mut axes = Vec::new();
        axes.extend(x_axis_result.elements.into_iter().map(|e| offset_element(e, margins.left, 0.0)));
        axes.extend(y_axis.into_iter().map(|e| offset_element(e, 0.0, margins.top)));
        // Zero-line (Phase 7): emitted after grid lines, before the series group
        // is pushed below — so the series paints over it. No-op when theme.zero_line
        // is None (default) or when the domain doesn't strictly cross zero.
        if let Some(zl) = emit_zero_line_if_crosses(
            &config.theme,
            (domain_min, domain_max),
            inner_width,
            inner_height,
            is_horizontal,
        ) {
            axes.push(offset_element(zl, margins.left, margins.top));
        }
        axes
    };

    children.push(ChartElement::Group {
        class: "axes".to_string(),
        transform: None,
        children: axis_elements,
    });

    children.push(ChartElement::Group {
        class: "bars".to_string(),
        transform: Some(Transform::Translate(margins.left, margins.top)),
        children: bar_elements,
    });

    // Annotations — rendered on top of bars, in inner coordinate space
    if !is_horizontal {
        if let Some(annotations) = config.visualize.annotations.as_deref() {
            if !annotations.is_empty() {
                use chartml_core::scales::ScaleLinear;
                let ann_scale = ScaleLinear::new((domain_min, domain_max), (inner_height, 0.0));
                let ann_cats = display_labels.as_deref().unwrap_or(&categories);
                let ann_elements = generate_annotations(
                    annotations,
                    &ann_scale,
                    0.0,
                    inner_width,
                    inner_height,
                    Some(ann_cats),
                    &config.theme,
                );
                if !ann_elements.is_empty() {
                    children.push(ChartElement::Group {
                        class: "annotations".to_string(),
                        transform: Some(Transform::Translate(margins.left, margins.top)),
                        children: ann_elements,
                    });
                }
            }
        }
    }

    // Legend
    if let Some(ref color_f) = color_field {
        let series_names = data.unique_values(color_f);
        let legend_config = LegendConfig {
            text_metrics: TextMetrics::from_theme_legend(&config.theme),
            ..LegendConfig::default()
        };
        let legend_layout = calculate_legend_layout(&series_names, &config.colors, config.width, &legend_config);
        let legend_y = config.height - legend_layout.total_height - 8.0;
        let legend_elements = generate_legend(&series_names, &config.colors, config.width, legend_y, &config.theme);
        children.push(ChartElement::Group {
            class: "legend".to_string(),
            transform: None,
            children: legend_elements,
        });
    }

    let svg_class = if is_horizontal { "chartml-bar chartml-horizontal" } else { "chartml-bar" };
    Ok(ChartElement::Svg {
        viewbox: ViewBox::new(0.0, 0.0, config.width, config.height),
        width: Some(config.width),
        height: Some(config.height),
        class: svg_class.to_string(),
        children,
    })
}

fn render_single_series_bars(
    data: &DataTable,
    config: &ChartConfig,
    params: &SingleSeriesBarParams,
) -> Result<(f64, Vec<ChartElement>), ChartError> {
    let category_field = params.category_field;
    let value_field = params.value_field;
    let categories = params.categories;
    let inner_width = params.inner_width;
    let inner_height = params.inner_height;
    let is_horizontal = params.is_horizontal;
    let y_fmt_ref = params.y_fmt_ref;
    let domain_min = params.domain_min;
    let domain_max = params.domain_max;
    // Find the max value (for return value only — domain_max is already caller-computed)
    let values: Vec<f64> = (0..data.num_rows())
        .filter_map(|i| data.get_f64(i, value_field))
        .collect();
    let value_max = values.iter().cloned().fold(0.0_f64, f64::max);
    let value_max = if value_max <= 0.0 { 1.0 } else { value_max };
    // Use the passed domain_max directly (caller already applied nice rounding if needed)
    let effective_max = domain_max;

    let mut elements = Vec::new();
    // Single-series bars always use one color (the first palette color).
    // Color is per-series, not per-category — matches JS d3ChartMapper.js behavior.
    let fill_color = config.colors.first()
        .cloned()
        .unwrap_or_else(|| "#2E7D9A".to_string());

    if is_horizontal {
        let band = ScaleBand::new(categories.to_vec(), (0.0, inner_height))
            .padding(crate::helpers::adaptive_bar_padding(categories.len()));
        let linear = ScaleLinear::new((domain_min, effective_max), (0.0, inner_width));
        // Match JS: barHeight = min(bandwidth, 40), centered in band
        let bar_render_height = band.bandwidth().min(40.0);
        let y_inset = (band.bandwidth() - bar_render_height) / 2.0;

        for i in 0..data.num_rows() {
            let cat = match data.get_string(i, category_field) {
                Some(c) => c,
                None => continue,
            };
            let val = data.get_f64(i, value_field).unwrap_or(0.0);
            // Use indexed band key for positioning (handles duplicate categories)
            let band_key = categories.get(i).map(|k| k.as_str()).unwrap_or(&cat);
            let y = match band.map(band_key) {
                Some(y) => y,
                None => continue,
            };
            let bar_width = linear.map(val);

            elements.push(build_bar_element(
                BarRectSpec {
                    x: 0.0,
                    y: y + y_inset,
                    width: bar_width,
                    height: bar_render_height,
                    is_horizontal: true,
                    is_negative: val < 0.0,
                    fill: fill_color.clone(),
                    class: "bar bar-rect".to_string(),
                    data: Some(ElementData::new(&cat, format_value(val, y_fmt_ref))),
                    stack_baseline: None,
                },
                &config.theme,
            ));
        }
    } else {
        let band = ScaleBand::new(categories.to_vec(), (0.0, inner_width))
            .padding(crate::helpers::adaptive_bar_padding(categories.len()));
        let linear = ScaleLinear::new((domain_min, effective_max), (inner_height, 0.0));
        // Match JS: barWidth = min(bandwidth, chartWidth * 0.2), centered in band
        let max_bar_width = inner_width * 0.2;
        let bar_render_width = band.bandwidth().min(max_bar_width);
        let x_inset = (band.bandwidth() - bar_render_width) / 2.0;

        for i in 0..data.num_rows() {
            let cat = match data.get_string(i, category_field) {
                Some(c) => c,
                None => continue,
            };
            let val = data.get_f64(i, value_field).unwrap_or(0.0);
            // Use indexed band key for positioning (handles duplicate categories)
            let band_key = categories.get(i).map(|k| k.as_str()).unwrap_or(&cat);
            let x = match band.map(band_key) {
                Some(x) => x,
                None => continue,
            };
            let bar_val_y = linear.map(val);
            let bar_zero_y = linear.map(0.0);
            let bar_height = (bar_zero_y - bar_val_y).abs();
            // For positive bars, rect y is at the value (above zero line).
            // For negative bars, rect y is at zero line (bar extends downward).
            let rect_y = bar_val_y.min(bar_zero_y);

            elements.push(build_bar_element(
                BarRectSpec {
                    x: x + x_inset,
                    y: rect_y,
                    width: bar_render_width,
                    height: bar_height,
                    is_horizontal: false,
                    is_negative: val < 0.0,
                    fill: fill_color.clone(),
                    class: "bar bar-rect".to_string(),
                    data: Some(ElementData::new(&cat, format_value(val, y_fmt_ref))),
                    stack_baseline: None,
                },
                &config.theme,
            ));

            // Data label above bar (if configured)
            if let Some(dl) = get_data_labels_config(config) {
                if dl.show == Some(true) {
                    let label_fmt = dl.format.as_deref().or(y_fmt_ref);
                    let label_y = match dl.position.as_deref() {
                        Some("center") => rect_y + bar_height / 2.0,
                        Some("bottom") => rect_y + bar_height - 5.0,
                        _ => if val >= 0.0 { rect_y - 5.0 } else { rect_y + bar_height + 12.0 }, // "top" or default
                    };
                    elements.push(ChartElement::Text {
                        x: x + band.bandwidth() / 2.0,
                        y: label_y,
                        content: format_value(val, label_fmt),
                        anchor: TextAnchor::Middle,
                        dominant_baseline: None,
                        transform: None,
                        font_family: None,
                        font_size: Some(dl.font_size.map(|s| format!("{}px", s)).unwrap_or_else(|| "12px".to_string())),
                        font_weight: None,
                        letter_spacing: None,
                        text_transform: None,
                        fill: Some(dl.color.clone().unwrap_or_else(|| config.theme.text_secondary.clone())),
                        class: "data-label".to_string(),
                        data: None,
                    });
                }
            }
        }
    }

    Ok((value_max, elements))
}

fn render_multi_series_bars(
    data: &DataTable,
    config: &ChartConfig,
    params: &MultiSeriesBarParams,
) -> Result<(f64, Vec<ChartElement>), ChartError> {
    let category_field = params.category_field;
    let value_field = params.value_field;
    let color_field = params.color_field;
    let categories = params.categories;
    let inner_width = params.inner_width;
    let inner_height = params.inner_height;
    let is_stacked = params.is_stacked;
    let is_normalized = params.is_normalized;
    let is_horizontal = params.is_horizontal;
    let y_fmt_ref = params.y_fmt_ref;
    let domain_min = params.domain_min;
    let domain_max = params.domain_max;
    use chartml_core::layout::stack::{StackLayout, StackOffset};

    let series_names = data.unique_values(color_field);
    let groups = data.group_by(color_field);

    let mut elements = Vec::new();

    if is_stacked {
        // Build values matrix: values[series_idx][category_idx]
        let mut values_matrix: Vec<Vec<f64>> = Vec::new();
        for series in &series_names {
            let mut series_vals = Vec::new();
            let series_data = groups.get(series);
            for cat in categories {
                let val = series_data
                    .map(|sd| {
                        (0..sd.num_rows())
                            .find_map(|i| {
                                if sd.get_string(i, category_field).as_deref() == Some(cat.as_str()) {
                                    sd.get_f64(i, value_field)
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(0.0)
                    })
                    .unwrap_or(0.0);
                series_vals.push(val);
            }
            values_matrix.push(series_vals);
        }

        let stack = if is_normalized {
            StackLayout::new().offset(StackOffset::Normalize)
        } else {
            StackLayout::new()
        };
        let stacked_points = stack.layout(categories, &series_names, &values_matrix);

        // For normalized mode, domain is 0-1; for regular stacked, use the raw max.
        let (effective_min, effective_max) = if is_normalized {
            (0.0, 1.0)
        } else {
            let value_max = stacked_points
                .iter()
                .map(|p| p.y1)
                .fold(0.0_f64, f64::max);
            let value_max = if value_max <= 0.0 { 1.0 } else { value_max };
            (domain_min, if domain_max < f64::MAX { domain_max } else { value_max })
        };

        if is_horizontal {
            // Horizontal stacked: band on y-axis (height), linear on x-axis (width)
            let band = ScaleBand::new(categories.to_vec(), (0.0, inner_height))
                .padding(crate::helpers::adaptive_bar_padding(categories.len()));
            let linear = ScaleLinear::new((effective_min, effective_max), (0.0, inner_width));
            let bar_render_height = band.bandwidth().min(40.0);
            let y_inset = (band.bandwidth() - bar_render_height) / 2.0;
            let baseline_x = linear.map(0.0);

            for point in &stacked_points {
                let y = match band.map(&point.key) {
                    Some(y) => y,
                    None => continue,
                };
                let x_left = linear.map(point.y0);
                let x_right = linear.map(point.y1);
                let bar_width = (x_right - x_left).abs();

                let series_idx = series_names.iter().position(|s| s == &point.series).unwrap_or(0);
                let fill = config
                    .colors
                    .get(series_idx)
                    .cloned()
                    .unwrap_or_else(|| "#2E7D9A".to_string());

                elements.push(build_bar_element(
                    BarRectSpec {
                        x: x_left.min(x_right),
                        y: y + y_inset,
                        width: bar_width,
                        height: bar_render_height,
                        is_horizontal: true,
                        is_negative: point.value < 0.0,
                        fill,
                        class: "bar bar-rect".to_string(),
                        data: Some(
                            ElementData::new(&point.key, format_value(point.value, y_fmt_ref))
                                .with_series(&point.series),
                        ),
                        stack_baseline: Some(baseline_x),
                    },
                    &config.theme,
                ));
            }
        } else {
            // Vertical stacked: band on x-axis (width), linear on y-axis (height)
            let band = ScaleBand::new(categories.to_vec(), (0.0, inner_width))
                .padding(crate::helpers::adaptive_bar_padding(categories.len()));
            let linear = ScaleLinear::new((effective_min, effective_max), (inner_height, 0.0));
            // Match JS: barWidth = min(bandwidth, chartWidth * 0.2), centered in band
            let max_bar_width = inner_width * 0.2;
            let bar_render_width = band.bandwidth().min(max_bar_width);
            let x_inset = (band.bandwidth() - bar_render_width) / 2.0;
            let baseline_y = linear.map(0.0);

            for point in &stacked_points {
                let x = match band.map(&point.key) {
                    Some(x) => x,
                    None => continue,
                };
                let y_top = linear.map(point.y1);
                let y_bottom = linear.map(point.y0);
                let bar_height = (y_bottom - y_top).abs();

                let series_idx = series_names.iter().position(|s| s == &point.series).unwrap_or(0);
                let fill = config
                    .colors
                    .get(series_idx)
                    .cloned()
                    .unwrap_or_else(|| "#2E7D9A".to_string());

                elements.push(build_bar_element(
                    BarRectSpec {
                        x: x + x_inset,
                        y: y_top,
                        width: bar_render_width,
                        height: bar_height,
                        is_horizontal: false,
                        is_negative: point.value < 0.0,
                        fill,
                        class: "bar bar-rect".to_string(),
                        data: Some(
                            ElementData::new(&point.key, format_value(point.value, y_fmt_ref))
                                .with_series(&point.series),
                        ),
                        stack_baseline: Some(baseline_y),
                    },
                    &config.theme,
                ));
            }
        }

        Ok((effective_max, elements))
    } else {
        // Grouped (or default multi-series)
        // Find overall max value
        let value_max = (0..data.num_rows())
            .filter_map(|i| data.get_f64(i, value_field))
            .fold(0.0_f64, f64::max);
        let value_max = if value_max <= 0.0 { 1.0 } else { value_max };
        let effective_max = if domain_max < f64::MAX { domain_max } else { value_max };

        // Build per-category series presence map (unique, insertion-ordered).
        // When color == category, each category has only 1 series; bars should
        // be sized and centered based on that local count, not the global one.
        let mut category_series: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for i in 0..data.num_rows() {
            let cat = match data.get_string(i, category_field) {
                Some(c) => c,
                None => continue,
            };
            let series = match data.get_string(i, color_field) {
                Some(s) => s,
                None => continue,
            };
            let present = category_series.entry(cat).or_default();
            if !present.contains(&series) {
                present.push(series);
            }
        }

        if is_horizontal {
            // Horizontal grouped: band on y-axis (height), linear on x-axis (width)
            let band = ScaleBand::new(categories.to_vec(), (0.0, inner_height))
                .padding(0.05);
            let linear = ScaleLinear::new((domain_min, effective_max), (0.0, inner_width));

            for i in 0..data.num_rows() {
                let cat = match data.get_string(i, category_field) {
                    Some(c) => c,
                    None => continue,
                };
                let series = match data.get_string(i, color_field) {
                    Some(s) => s,
                    None => continue,
                };
                let val = data.get_f64(i, value_field).unwrap_or(0.0);

                let y_base = match band.map(&cat) {
                    Some(y) => y,
                    None => continue,
                };

                // Per-category sizing: divide the band by how many series
                // are actually present in this category.
                let local_series = category_series.get(&cat);
                let local_count = local_series.map_or(1, |v| v.len()).max(1);
                let sub_band_height = band.bandwidth() / local_count as f64;

                // Cap individual bar height (matches single-series horizontal
                // capping at 40px) and center within sub-band.
                let bar_render_height = sub_band_height.min(40.0);
                let y_inset = (sub_band_height - bar_render_height) / 2.0;

                let local_idx = local_series
                    .and_then(|v| v.iter().position(|s| s == &series))
                    .unwrap_or(0);
                let y = y_base + local_idx as f64 * sub_band_height + y_inset;

                let series_idx = series_names.iter().position(|s| s == &series).unwrap_or(0);

                let bar_left = linear.map(0.0);
                let bar_right = linear.map(val);
                let bar_width = (bar_right - bar_left).abs();

                let fill = config
                    .colors
                    .get(series_idx)
                    .cloned()
                    .unwrap_or_else(|| "#2E7D9A".to_string());

                elements.push(build_bar_element(
                    BarRectSpec {
                        x: bar_left.min(bar_right),
                        y,
                        width: bar_width,
                        height: bar_render_height,
                        is_horizontal: true,
                        is_negative: val < 0.0,
                        fill,
                        class: "bar bar-rect".to_string(),
                        data: Some(
                            ElementData::new(&cat, format_value(val, y_fmt_ref))
                                .with_series(&series),
                        ),
                        stack_baseline: None,
                    },
                    &config.theme,
                ));
            }
        } else {
            // Vertical grouped: band on x-axis (width), linear on y-axis (height)
            let band = ScaleBand::new(categories.to_vec(), (0.0, inner_width))
                .padding(0.05);
            let linear = ScaleLinear::new((domain_min, effective_max), (inner_height, 0.0));

            // Max bar width cap (matches single-series vertical path).
            let max_bar_width = inner_width * 0.2;

            for i in 0..data.num_rows() {
                let cat = match data.get_string(i, category_field) {
                    Some(c) => c,
                    None => continue,
                };
                let series = match data.get_string(i, color_field) {
                    Some(s) => s,
                    None => continue,
                };
                let val = data.get_f64(i, value_field).unwrap_or(0.0);

                let x_base = match band.map(&cat) {
                    Some(x) => x,
                    None => continue,
                };

                // Per-category sizing: divide the band by how many series
                // are actually present in this category.
                let local_series = category_series.get(&cat);
                let local_count = local_series.map_or(1, |v| v.len()).max(1);
                let sub_band_width = band.bandwidth() / local_count as f64;

                // Cap individual bar width and center within sub-band.
                let bar_render_width = sub_band_width.min(max_bar_width);
                let x_inset = (sub_band_width - bar_render_width) / 2.0;

                let local_idx = local_series
                    .and_then(|v| v.iter().position(|s| s == &series))
                    .unwrap_or(0);
                let x = x_base + local_idx as f64 * sub_band_width + x_inset;

                let series_idx = series_names.iter().position(|s| s == &series).unwrap_or(0);

                let bar_top = linear.map(val);
                let bar_bottom = linear.map(0.0);
                let bar_height = (bar_bottom - bar_top).abs();

                let fill = config
                    .colors
                    .get(series_idx)
                    .cloned()
                    .unwrap_or_else(|| "#2E7D9A".to_string());

                elements.push(build_bar_element(
                    BarRectSpec {
                        x,
                        y: bar_top,
                        width: bar_render_width,
                        height: bar_height,
                        is_horizontal: false,
                        is_negative: val < 0.0,
                        fill,
                        class: "bar bar-rect".to_string(),
                        data: Some(
                            ElementData::new(&cat, format_value(val, y_fmt_ref))
                                .with_series(&series),
                        ),
                        stack_baseline: None,
                    },
                    &config.theme,
                ));
            }
        }

        Ok((value_max, elements))
    }
}

/// Render a combo chart: multiple fields with different marks (bar/line) and optional dual axis.
fn render_combo(
    data: &DataTable,
    config: &ChartConfig,
    fields: &[chartml_core::spec::FieldSpec],
) -> Result<ChartElement, ChartError> {
    use chartml_core::shapes::LineGenerator;
    use chartml_core::layout::stack::StackLayout;

    let category_field = get_field_name(&config.visualize.columns)?;
    let categories = data.unique_values(&category_field);
    if categories.is_empty() {
        return Err(ChartError::DataError("No category values found".into()));
    }

    let y_fmt = get_y_format(config);
    let y_fmt_ref = y_fmt.as_deref();
    let grid = GridConfig::from_config(config);
    let x_format = get_x_format(config);

    // Detect stacking mode and color field for bar sub-series
    let color_field = get_color_field(config);
    let is_stacked = matches!(config.visualize.mode, Some(ChartMode::Stacked));

    // Margins — account for right axis if present
    let has_right = fields.iter().any(|f| f.axis.as_deref() == Some("right"));
    let right_fmt = config.visualize.axes.as_ref()
        .and_then(|a| a.right.as_ref())
        .and_then(|a| a.format.as_deref());

    // Pre-compute right tick labels to measure their width
    let right_tick_labels: Vec<String> = if has_right {
        // Estimate right-axis values for label width measurement
        let right_max = fields.iter()
            .filter(|f| f.axis.as_deref() == Some("right"))
            .filter_map(|f| f.field.as_deref())
            .flat_map(|name| (0..data.num_rows()).filter_map(move |i| data.get_f64(i, name)))
            .fold(0.0_f64, f64::max);
        let right_domain_max = config.visualize.axes.as_ref()
            .and_then(|a| a.right.as_ref())
            .and_then(|a| a.max)
            .unwrap_or(if right_max <= 0.0 { 1.0 } else { right_max });
        let tmp_scale = ScaleLinear::new((0.0, right_domain_max), (0.0, 100.0));
        tmp_scale.ticks(5).iter().map(|v| format_value(*v, right_fmt)).collect()
    } else {
        vec![]
    };

    let has_x_axis_label = config.visualize.axes.as_ref()
        .and_then(|a| a.x.as_ref())
        .and_then(|a| a.label.as_ref())
        .is_some();
    // Pre-compute combo legend height from field labels. Range marks are
    // skipped in the render loop and don't get a legend entry, so exclude
    // them from the legend-height calculation too.
    let combo_legend_labels: Vec<String> = fields.iter()
        .filter(|f| f.mark.as_deref() != Some("range"))
        .map(|f| {
            f.label
                .clone()
                .unwrap_or_else(|| f.field.clone().unwrap_or_default())
        })
        .collect();
    let combo_legend_height = if combo_legend_labels.len() > 1 || color_field.is_some() {
        let legend_config = LegendConfig {
            text_metrics: TextMetrics::from_theme_legend(&config.theme),
            ..LegendConfig::default()
        };
        calculate_legend_layout(&combo_legend_labels, &config.colors, config.width, &legend_config).total_height
    } else {
        0.0
    };
    let margin_config = MarginConfig {
        has_title: config.title.is_some(),
        legend_height: combo_legend_height,
        // Left Y-axis label is not rendered for combo charts (see comment below),
        // so do not reserve extra left-margin space for it.
        has_y_axis_label: false,
        has_x_axis_label,
        has_right_axis: has_right,
        right_tick_labels,
        tick_value_metrics: TextMetrics::from_theme_tick_value(&config.theme),
        axis_label_metrics: TextMetrics::from_theme_axis_label(&config.theme),
        ..Default::default()
    };
    let margins = calculate_margins(&margin_config);
    let inner_width = margins.inner_width(config.width);
    let inner_height = margins.inner_height(config.height);

    let band = ScaleBand::new(categories.clone(), (0.0, inner_width))
        .padding(crate::helpers::adaptive_bar_padding(categories.len()));
    let bandwidth = band.bandwidth();

    // Separate fields by axis
    let left_fields: Vec<&chartml_core::spec::FieldSpec> = fields.iter()
        .filter(|f| f.axis.as_deref() != Some("right"))
        .collect();
    let right_fields: Vec<&chartml_core::spec::FieldSpec> = fields.iter()
        .filter(|f| f.axis.as_deref() == Some("right"))
        .collect();

    // Compute left-axis domain with D3-style nice rounding (Regressions 2 & 3).
    // When stacked with a color field, the domain max is the per-category sum of all series.
    let left_max = if let (true, Some(color_f)) = (is_stacked, color_field.as_ref()) {
        let color_series = data.unique_values(color_f);
        let mut max_stack = 0.0_f64;
        for f in &left_fields {
            let Some(field_name) = f.field.as_deref() else { continue };
            for cat in &categories {
                let mut stack_total = 0.0_f64;
                for series in &color_series {
                    let val = (0..data.num_rows())
                        .find(|&i| {
                            data.get_string(i, &category_field).as_deref() == Some(cat.as_str())
                                && data.get_string(i, color_f).as_deref() == Some(series.as_str())
                        })
                        .and_then(|i| data.get_f64(i, field_name))
                        .unwrap_or(0.0);
                    stack_total += val;
                }
                max_stack = max_stack.max(stack_total);
            }
        }
        max_stack
    } else {
        left_fields.iter()
            .filter_map(|f| f.field.as_deref())
            .flat_map(|name| (0..data.num_rows()).filter_map(move |i| data.get_f64(i, name)))
            .fold(0.0_f64, f64::max)
    };
    // Compute left-axis data minimum to support negative bar values.
    let left_data_min = left_fields.iter()
        .filter_map(|f| f.field.as_deref())
        .flat_map(|name| (0..data.num_rows()).filter_map(move |i| data.get_f64(i, name)))
        .fold(0.0_f64, f64::min);
    // Keep data_min at 0 when all values are non-negative (standard bar chart behavior)
    let left_data_min = if left_data_min >= 0.0 { 0.0 } else { left_data_min };
    let axes_left = config.visualize.axes.as_ref().and_then(|a| a.left.as_ref());
    let left_explicit_min = axes_left.and_then(|a| a.min);
    let left_explicit_max = axes_left.and_then(|a| a.max);
    let raw_left_domain_min = left_explicit_min.unwrap_or(left_data_min);
    let raw_left_domain_max = left_explicit_max.unwrap_or(if left_max <= 0.0 { 1.0 } else { left_max });
    let (left_domain_min, left_domain_max) = if left_explicit_min.is_none() && left_explicit_max.is_none() {
        // Use count=5 to align with generate_y_axis_numeric's hardcoded tick count of 5.
        nice_domain(raw_left_domain_min, raw_left_domain_max, 5)
    } else {
        (raw_left_domain_min, raw_left_domain_max)
    };
    let left_scale = ScaleLinear::new((left_domain_min, left_domain_max), (inner_height, 0.0));

    // Compute right-axis domain with D3-style nice rounding (Regressions 2 & 3).
    let right_scale = if !right_fields.is_empty() {
        let right_max = right_fields.iter()
            .filter_map(|f| f.field.as_deref())
            .flat_map(|name| (0..data.num_rows()).filter_map(move |i| data.get_f64(i, name)))
            .fold(0.0_f64, f64::max);
        let right_data_min = right_fields.iter()
            .filter_map(|f| f.field.as_deref())
            .flat_map(|name| (0..data.num_rows()).filter_map(move |i| data.get_f64(i, name)))
            .fold(0.0_f64, f64::min);
        let axes_right = config.visualize.axes.as_ref().and_then(|a| a.right.as_ref());
        let right_explicit_min = axes_right.and_then(|a| a.min);
        let right_explicit_max = axes_right.and_then(|a| a.max);
        let raw_right_domain_min = right_explicit_min.unwrap_or(if right_data_min < 0.0 { right_data_min } else { 0.0 });
        let raw_right_domain_max = right_explicit_max.unwrap_or(if right_max <= 0.0 { 1.0 } else { right_max });
        let (right_domain_min, right_domain_max) = if right_explicit_min.is_none() && right_explicit_max.is_none() {
            // Use count=5 to align with generate_y_axis_numeric's hardcoded tick count of 5.
            nice_domain(raw_right_domain_min, raw_right_domain_max, 5)
        } else {
            (raw_right_domain_min, raw_right_domain_max)
        };
        Some(ScaleLinear::new((right_domain_min, right_domain_max), (inner_height, 0.0)))
    } else {
        None
    };

    let mut children = Vec::new();

    // Title is rendered as HTML outside the SVG — not added here.

    // Axes
    let bottom_axis_label = config.visualize.axes.as_ref()
        .and_then(|a| a.x.as_ref())
        .and_then(|a| a.label.as_deref());
    let x_axis_result = generate_x_axis(&crate::helpers::XAxisParams {
        labels: &categories,
        display_label_overrides: None,
        range: (0.0, inner_width),
        y_position: margins.top + inner_height,
        available_width: inner_width,
        x_format: x_format.as_deref(),
        chart_height: Some(inner_height),
        grid: &grid,
        axis_label: bottom_axis_label,
        theme: &config.theme,
    });
    let left_axis_label = axes_left.and_then(|a| a.label.as_deref());
    let y_axis_left = generate_y_axis_numeric(&crate::helpers::YAxisNumericParams {
        domain: (left_domain_min, left_domain_max),
        range: (inner_height, 0.0),
        x_position: margins.left,
        fmt: y_fmt_ref,
        tick_count: adaptive_tick_count(inner_height),
        chart_width: Some(inner_width),
        grid: &grid,
        axis_label: left_axis_label,
        theme: &config.theme,
    });

    let mut axis_elements = Vec::new();
    axis_elements.extend(x_axis_result.elements.into_iter().map(|e| offset_element(e, margins.left, 0.0)));
    axis_elements.extend(y_axis_left.into_iter().map(|e| offset_element(e, 0.0, margins.top)));
    // Zero-line (Phase 7): applied to the LEFT numeric axis domain only. The
    // combo/grouped path is always vertically-oriented (grouped horizontal bars
    // are not supported here). No-op when theme.zero_line is None (default) or
    // when the left domain doesn't strictly cross zero.
    if let Some(zl) = emit_zero_line_if_crosses(
        &config.theme,
        (left_domain_min, left_domain_max),
        inner_width,
        inner_height,
        false,
    ) {
        axis_elements.push(offset_element(zl, margins.left, margins.top));
    }

    // Right axis — ticks and labels on the right side
    if let Some(ref rs) = right_scale {
        let right_fmt = config.visualize.axes.as_ref()
            .and_then(|a| a.right.as_ref())
            .and_then(|a| a.format.as_deref());
        // Right axis label is rendered manually below (outside this block),
        // so pass None here to avoid duplication.
        let right_axis = generate_y_axis_numeric_right(
            rs.domain(), (inner_height, 0.0), margins.left + inner_width,
            right_fmt, adaptive_tick_count(inner_height),
            None, &config.theme,
        );
        axis_elements.extend(right_axis.into_iter().map(|e| offset_element(e, 0.0, margins.top)));
    }

    // Right axis title label — rendered manually here with absolute positioning
    // (the left axis label is already handled by generate_y_axis_numeric above).
    if let Some(label) = config.visualize.axes.as_ref().and_then(|a| a.right.as_ref()).and_then(|a| a.label.clone()) {
        let rx = config.width - 12.0;
        let ts = TextStyle::for_role(&config.theme, TextRole::AxisLabel);
        axis_elements.push(ChartElement::Text {
            x: rx,
            y: margins.top + inner_height / 2.0,
            content: label,
            anchor: TextAnchor::Middle,
            dominant_baseline: None,
            transform: Some(Transform::Rotate(90.0, rx, margins.top + inner_height / 2.0)),
            font_family: ts.font_family,
            font_size: ts.font_size,
            font_weight: ts.font_weight,
            letter_spacing: ts.letter_spacing,
            text_transform: ts.text_transform,
            fill: Some(config.theme.text_secondary.clone()),
            class: "axis-label".to_string(),
            data: None,
        });
    }

    children.push(ChartElement::Group {
        class: "axes".to_string(), transform: None, children: axis_elements,
    });

    // Render each field
    let mut mark_elements = Vec::new();
    let line_gen = LineGenerator::new().curve(chartml_core::shapes::CurveType::MonotoneX);

    // Count bar fields for grouped subdivision
    let num_bar_fields = fields.iter()
        .filter(|f| f.mark.as_deref().unwrap_or("bar") == "bar")
        .count()
        .max(1);
    // Match JS: barWidth = min(bandwidth, chartWidth * 0.2), centered within band
    let max_bar_width = inner_width * 0.2;
    let effective_bandwidth = bandwidth.min(max_bar_width);
    let combo_x_inset = (bandwidth - effective_bandwidth) / 2.0;
    let sub_bar_padding = effective_bandwidth * 0.05;
    let sub_bar_width = (effective_bandwidth - sub_bar_padding * (num_bar_fields as f64 - 1.0).max(0.0)) / num_bar_fields as f64;
    let mut bar_field_idx = 0_usize;
    let mut series_names = Vec::new();
    let mut series_colors = Vec::new();
    let mut series_marks = Vec::new();

    // Pre-compute stacked bar layout if stacking with color field
    let stacked_bar_rendered = if let (true, Some(color_f)) = (is_stacked, color_field.as_ref()) {
        let color_series = data.unique_values(color_f);

        // For each bar field, render stacked bars by color series
        for field_spec in fields.iter() {
            let mark = field_spec.mark.as_deref().unwrap_or("bar");
            if mark != "bar" { continue; }

            let field_name = field_spec.field.as_deref().unwrap_or("");
            let is_right = field_spec.axis.as_deref() == Some("right");
            let scale = if is_right { right_scale.as_ref().unwrap_or(&left_scale) } else { &left_scale };
            let fmt_ref = if is_right {
                config.visualize.axes.as_ref().and_then(|a| a.right.as_ref()).and_then(|a| a.format.as_deref())
            } else {
                y_fmt_ref
            };

            // Build values matrix: values[series_idx][category_idx]
            let mut values_matrix: Vec<Vec<f64>> = Vec::new();
            for series in &color_series {
                let mut series_vals = Vec::new();
                for cat in &categories {
                    let val = (0..data.num_rows())
                        .find(|&i| {
                            data.get_string(i, &category_field).as_deref() == Some(cat.as_str())
                                && data.get_string(i, color_f).as_deref() == Some(series.as_str())
                        })
                        .and_then(|i| data.get_f64(i, field_name))
                        .unwrap_or(0.0);
                    series_vals.push(val);
                }
                values_matrix.push(series_vals);
            }

            let stack = StackLayout::new();
            let stacked_points = stack.layout(&categories, &color_series, &values_matrix);

            let bar_render_width = bandwidth.min(max_bar_width);
            let x_inset = (bandwidth - bar_render_width) / 2.0;
            let combo_baseline_y = scale.map(0.0) + margins.top;

            for point in &stacked_points {
                let x = match band.map(&point.key) { Some(x) => x, None => continue };
                let y_top = scale.map(point.y1);
                let y_bottom = scale.map(point.y0);
                let bar_height = (y_bottom - y_top).abs();

                let series_idx = color_series.iter().position(|s| s == &point.series).unwrap_or(0);
                let fill = config.colors.get(series_idx).cloned().unwrap_or_else(|| "#2E7D9A".to_string());

                mark_elements.push(build_bar_element(
                    BarRectSpec {
                        x: x + x_inset + margins.left,
                        y: y_top + margins.top,
                        width: bar_render_width,
                        height: bar_height,
                        is_horizontal: false,
                        is_negative: point.value < 0.0,
                        fill,
                        class: "bar bar-rect".to_string(),
                        data: Some(
                            ElementData::new(&point.key, format_value(point.value, fmt_ref))
                                .with_series(&point.series),
                        ),
                        stack_baseline: Some(combo_baseline_y),
                    },
                    &config.theme,
                ));
            }
        }

        // Add color series to legend tracking
        for (si, series_name) in color_series.iter().enumerate() {
            let color = config.colors.get(si).cloned().unwrap_or_else(|| "#2E7D9A".to_string());
            series_names.push(series_name.clone());
            series_colors.push(color);
            series_marks.push("bar".to_string());
        }

        true
    } else {
        false
    };

    for (field_idx, field_spec) in fields.iter().enumerate() {
        // Range marks have no scalar `field` — they shade a band between
        // `upper` and `lower`. Combo (bar+line) charts don't render shaded
        // bands (that's a line-chart-only feature in JS chartml too), so skip
        // them entirely here — otherwise the outer loop would emit a phantom
        // legend entry and attempt to fetch data under an empty field name.
        if field_spec.mark.as_deref() == Some("range") {
            continue;
        }
        let field_name = field_spec.field.as_deref().unwrap_or("");
        let is_right = field_spec.axis.as_deref() == Some("right");
        let scale = if is_right { right_scale.as_ref().unwrap_or(&left_scale) } else { &left_scale };
        let mark = field_spec.mark.as_deref().unwrap_or("bar");
        let color = field_spec.color.clone()
            .unwrap_or_else(|| config.colors.get(field_idx).cloned().unwrap_or_else(|| "#2E7D9A".to_string()));
        let label = field_spec.label.clone().unwrap_or_else(|| field_name.to_string());
        let fmt_ref = if is_right {
            config.visualize.axes.as_ref().and_then(|a| a.right.as_ref()).and_then(|a| a.format.as_deref())
        } else {
            y_fmt_ref
        };

        match mark {
            "bar" if stacked_bar_rendered => {
                // Already rendered above via stacked layout — skip
            }
            "bar" => {
                let this_bar_idx = bar_field_idx;
                bar_field_idx += 1;

                for row_i in 0..data.num_rows() {
                    let cat = match data.get_string(row_i, &category_field) { Some(c) => c, None => continue };
                    let val = data.get_f64(row_i, field_name).unwrap_or(0.0);
                    let x = match band.map(&cat) { Some(x) => x, None => continue };
                    let bar_x = x + combo_x_inset + this_bar_idx as f64 * (sub_bar_width + sub_bar_padding);
                    let bar_val_y = scale.map(val);
                    let bar_zero_y = scale.map(0.0);
                    let bar_height = (bar_zero_y - bar_val_y).abs();
                    let rect_y = bar_val_y.min(bar_zero_y);

                    mark_elements.push(build_bar_element(
                        BarRectSpec {
                            x: bar_x + margins.left,
                            y: rect_y + margins.top,
                            width: sub_bar_width,
                            height: bar_height,
                            is_horizontal: false,
                            is_negative: val < 0.0,
                            fill: color.clone(),
                            class: "bar bar-rect".to_string(),
                            data: Some(
                                ElementData::new(&cat, format_value(val, fmt_ref))
                                    .with_series(&label),
                            ),
                            stack_baseline: None,
                        },
                        &config.theme,
                    ));

                    // Data labels
                    if let Some(ref dl) = field_spec.data_labels {
                        if dl.show == Some(true) {
                            let dl_fmt = dl.format.as_deref().or(fmt_ref);
                            mark_elements.push(ChartElement::Text {
                                x: bar_x + sub_bar_width / 2.0 + margins.left,
                                y: rect_y + margins.top - 5.0,
                                content: format_value(val, dl_fmt),
                                anchor: TextAnchor::Middle, dominant_baseline: None,
                                transform: None,
                                font_family: None,
                                font_size: Some(dl.font_size.map(|s| format!("{}px", s)).unwrap_or_else(|| "12px".to_string())),
                                font_weight: None,
                                letter_spacing: None,
                                text_transform: None,
                                fill: Some(dl.color.clone().unwrap_or_else(|| config.theme.text_secondary.clone())),
                                class: "data-label".to_string(), data: None,
                            });
                        }
                    }
                }
            }
            _ => {
                let mut points = Vec::new();
                let mut point_data = Vec::new();
                for cat in &categories {
                    let row_i = match (0..data.num_rows()).find(|&i| data.get_string(i, &category_field).as_deref() == Some(cat.as_str())) {
                        Some(i) => i, None => continue,
                    };
                    let val = match data.get_f64(row_i, field_name) { Some(v) => v, None => continue };
                    let x = match band.map(cat) { Some(x) => x + bandwidth / 2.0, None => continue };
                    let y = scale.map(val);
                    points.push((x + margins.left, y + margins.top));
                    point_data.push((cat.clone(), val));
                }

                if !points.is_empty() {
                    let path_d = line_gen.generate(&points);
                    mark_elements.push(ChartElement::Path {
                        d: path_d, fill: None, stroke: Some(color.clone()),
                        stroke_width: Some(config.theme.series_line_weight as f64), stroke_dasharray: None,
                        stroke_dashoffset: None,
                        opacity: None,
                        class: "chartml-line-path series-line".to_string(),
                        data: Some(ElementData::new(&label, "").with_series(&label)),
                        animation_origin: None,
                    });

                    // Dots
                    let dot_r = config.theme.dot_radius as f64;
                    for (i, &(px, py)) in points.iter().enumerate() {
                        let (ref cat, val) = point_data[i];
                        if let Some(halo) = emit_dot_halo_if_enabled(&config.theme, px, py, dot_r) {
                            mark_elements.push(halo);
                        }
                        mark_elements.push(ChartElement::Circle {
                            cx: px, cy: py, r: dot_r,
                            fill: color.clone(), stroke: Some(config.theme.bg.clone()),
                            class: "chartml-line-dot dot-marker".to_string(),
                            data: Some(ElementData::new(cat, format_value(val, fmt_ref)).with_series(&label)),
                        });
                    }

                    // Data labels
                    if let Some(ref dl) = field_spec.data_labels {
                        if dl.show == Some(true) {
                            let dl_fmt = dl.format.as_deref().or(fmt_ref);
                            for (i, &(px, py)) in points.iter().enumerate() {
                                let (_, val) = &point_data[i];
                                mark_elements.push(ChartElement::Text {
                                    x: px, y: py - 10.0,
                                    content: format_value(*val, dl_fmt),
                                    anchor: TextAnchor::Middle, dominant_baseline: None,
                                    transform: None,
                                    font_family: None,
                                    font_size: Some(dl.font_size.map(|s| format!("{}px", s)).unwrap_or_else(|| "12px".to_string())),
                                    font_weight: None,
                                    letter_spacing: None,
                                    text_transform: None,
                                    fill: Some(dl.color.clone().unwrap_or_else(|| color.clone())),
                                    class: "data-label".to_string(), data: None,
                                });
                            }
                        }
                    }
                }
            }
        }

        // When stacked bars were rendered via color field, the color series are already
        // tracked for legend — skip adding the bar field itself.
        if !(stacked_bar_rendered && mark == "bar") {
            series_names.push(label);
            series_colors.push(color);
            series_marks.push(mark.to_string());
        }
    }

    children.push(ChartElement::Group {
        class: "marks".to_string(), transform: None, children: mark_elements,
    });

    // Annotations — rendered on top of marks, in inner coordinate space
    if let Some(annotations) = config.visualize.annotations.as_deref() {
        if !annotations.is_empty() {
            let ann_elements = generate_annotations(
                annotations,
                &left_scale,
                0.0,
                inner_width,
                inner_height,
                Some(&categories),
                &config.theme,
            );
            if !ann_elements.is_empty() {
                children.push(ChartElement::Group {
                    class: "annotations".to_string(),
                    transform: Some(Transform::Translate(margins.left, margins.top)),
                    children: ann_elements,
                });
            }
        }
    }

    // Legend with mixed marks — anchor using pre-computed legend height
    if series_names.len() > 1 {
        let combo_legend_metrics = TextMetrics::from_theme_legend(&config.theme);
        let mut legend_elements = Vec::new();
        let total_w: f64 = series_names.iter().map(|name| {
            let tw = measure_text(name, &combo_legend_metrics);
            12.0 + 6.0 + tw + 16.0
        }).sum();
        let mut x_offset = (config.width - total_w).max(0.0) / 2.0;
        let legend_y = config.height - combo_legend_height - 8.0;

        for (i, name) in series_names.iter().enumerate() {
            let color = &series_colors[i];
            let mark = series_marks[i].as_str();
            let y = legend_y;

            match mark {
                "line" => {
                    legend_elements.push(ChartElement::Line {
                        x1: x_offset, y1: y + 6.0, x2: x_offset + 12.0, y2: y + 6.0,
                        stroke: color.clone(), stroke_width: Some(2.5),
                        stroke_dasharray: None, class: "legend-symbol legend-line".to_string(),
                    });
                }
                _ => {
                    legend_elements.push(ChartElement::Rect {
                        x: x_offset, y, width: 12.0, height: 12.0,
                        fill: color.clone(), stroke: None,
                        rx: None, ry: None,
                        class: "legend-symbol".to_string(), data: None,
                        animation_origin: None,
                    });
                }
            }

            let ts = TextStyle::for_role(&config.theme, TextRole::LegendLabel);
            legend_elements.push(ChartElement::Text {
                x: x_offset + 18.0, y: y + 10.0, content: name.clone(),
                anchor: TextAnchor::Start, dominant_baseline: None,
                transform: None,
                font_family: ts.font_family,
                font_size: ts.font_size,
                font_weight: ts.font_weight,
                letter_spacing: ts.letter_spacing,
                text_transform: ts.text_transform,
                fill: Some(config.theme.text_secondary.clone()), class: "legend-label".to_string(), data: None,
            });

            let tw = measure_text(name, &combo_legend_metrics);
            x_offset += 12.0 + 6.0 + tw + 16.0;
        }

        children.push(ChartElement::Group {
            class: "legend".to_string(), transform: None, children: legend_elements,
        });
    }

    Ok(ChartElement::Svg {
        viewbox: ViewBox::new(0.0, 0.0, config.width, config.height),
        width: Some(config.width),
        height: Some(config.height),
        class: "chartml-bar chartml-combo".to_string(),
        children,
    })
}

