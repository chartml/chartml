use chartml_core::element::{ChartElement, ElementData, TextAnchor, Transform};
use chartml_core::error::ChartError;
use chartml_core::format::NumberFormatter;
use chartml_core::format::{detect_date_format, reformat_date_label};
use chartml_core::layout::labels::{LabelStrategy, LabelStrategyConfig, approximate_text_width, truncate_label};
use chartml_core::plugin::ChartConfig;
use chartml_core::scales::{ScaleBand, ScaleLinear};
use chartml_core::spec::{AnnotationSpec, FieldRef, FieldRefItem, MarkEncoding};

/// Grid line configuration resolved from the spec.
#[derive(Debug, Clone)]
pub struct GridConfig {
    pub show_x: bool,
    pub show_y: bool,
    pub color: String,
    pub opacity: f64,
    pub dash_array: Option<String>,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            show_x: false,
            show_y: true, // JS default: horizontal grid on
            color: "#e0e0e0".to_string(),
            opacity: 0.5,
            dash_array: None,
        }
    }
}

impl GridConfig {
    /// Build from spec's style.grid if present.
    pub fn from_config(config: &ChartConfig) -> Self {
        let mut grid = Self::default();
        if let Some(ref style) = config.visualize.style {
            if let Some(ref g) = style.grid {
                if let Some(x) = g.x { grid.show_x = x; }
                if let Some(y) = g.y { grid.show_y = y; }
                if let Some(ref c) = g.color { grid.color = c.clone(); }
                if let Some(o) = g.opacity { grid.opacity = o; }
                if let Some(ref d) = g.dash_array { grid.dash_array = Some(d.clone()); }
            }
        }
        grid
    }
}

/// Extract axis min/max bounds from the spec.
/// Adaptive bar padding matching JS d3CartesianChart.js behavior.
/// More bars = less padding to prevent overlap; fewer bars = more padding.
pub fn adaptive_bar_padding(num_categories: usize) -> f64 {
    if num_categories <= 6 {
        0.2
    } else if num_categories <= 12 {
        0.15
    } else if num_categories <= 20 {
        0.1
    } else {
        0.05
    }
}

pub fn get_y_axis_bounds(config: &ChartConfig) -> (Option<f64>, Option<f64>) {
    let axes = match &config.visualize.axes {
        Some(a) => a,
        None => return (None, None),
    };
    let axis = match &axes.left {
        Some(a) => a,
        None => return (None, None),
    };
    (axis.min, axis.max)
}

/// Get dataLabels config from the rows field spec.
pub fn get_data_labels_config(config: &ChartConfig) -> Option<&chartml_core::spec::DataLabelsSpec> {
    match &config.visualize.rows {
        Some(chartml_core::spec::FieldRef::Detailed(spec)) => spec.data_labels.as_ref(),
        _ => None,
    }
}

/// Extract the field name from a FieldRef (Simple, Detailed, or Multiple).
pub fn get_field_name(field_ref: &Option<FieldRef>) -> Result<String, ChartError> {
    match field_ref {
        Some(FieldRef::Simple(name)) => Ok(name.clone()),
        Some(FieldRef::Detailed(spec)) => Ok(spec.field.clone()),
        Some(FieldRef::Multiple(items)) => match items.first() {
            Some(FieldRefItem::Simple(name)) => Ok(name.clone()),
            Some(FieldRefItem::Detailed(spec)) => Ok(spec.field.clone()),
            None => Err(ChartError::MissingField("rows/columns field".into())),
        },
        None => Err(ChartError::MissingField("rows/columns field".into())),
    }
}

/// Extract color/series field from marks.
pub fn get_color_field(config: &ChartConfig) -> Option<String> {
    config
        .visualize
        .marks
        .as_ref()?
        .color
        .as_ref()
        .map(|enc| match enc {
            MarkEncoding::Simple(name) => name.clone(),
            MarkEncoding::Detailed(spec) => spec.field.clone(),
        })
}

/// Extract the y-axis (rows/left) format string from the spec.
pub fn get_y_format(config: &ChartConfig) -> Option<String> {
    config.visualize.axes.as_ref().and_then(|axes| {
        axes.left.as_ref().and_then(|a| a.format.clone())
    })
}

/// Extract the x-axis (columns) format string from the spec.
pub fn get_x_format(config: &ChartConfig) -> Option<String> {
    config.visualize.axes.as_ref().and_then(|axes| {
        axes.x.as_ref().and_then(|a| a.format.clone())
    })
}

/// Format a numeric value using a format string, or a sensible default.
pub fn format_value(value: f64, format_str: Option<&str>) -> String {
    match format_str {
        Some(fmt) => NumberFormatter::new(fmt).format(value),
        None => default_format_value(value),
    }
}

/// Format a tick label with SI suffix abbreviation when values are large.
///
/// When `tick_step` is >= 1_000_000_000 uses "B" suffix, >= 1_000_000 uses "M",
/// >= 1_000 uses "K". The value is divided by the magnitude before formatting
/// with the given format string, and the suffix is appended after the formatted
/// number. Trailing ".0" or unnecessary decimals are cleaned up so that e.g.
/// "$1.0B" becomes "$1B".
///
/// This is only applied when an explicit format string is provided AND the tick
/// step indicates large magnitude. For tick labels without an explicit format,
/// the standard `format_tick_value` is used unchanged.
fn format_tick_value_si(value: f64, tick_step: f64, fmt: &str) -> String {
    let (divisor, suffix) = if tick_step >= 1_000_000_000.0 {
        (1_000_000_000.0, "B")
    } else if tick_step >= 1_000_000.0 {
        (1_000_000.0, "M")
    } else if tick_step >= 1_000.0 {
        (1_000.0, "K")
    } else {
        // No abbreviation needed — use standard format_value
        return format_value(value, Some(fmt));
    };

    let scaled = value / divisor;

    // Build a modified format string: strip comma grouping since abbreviated
    // values are small (e.g. 1, 2, 3) and commas are meaningless.
    let fmt_no_comma = fmt.replace(',', "");
    let formatted = NumberFormatter::new(&fmt_no_comma).format(scaled);

    // Clean up unnecessary trailing decimals: "$1.0" -> "$1", "$2.00" -> "$2"
    // but preserve meaningful decimals like "$1.5" or "$3.52".
    let cleaned = strip_trailing_zero_decimals(&formatted);

    format!("{}{}", cleaned, suffix)
}

/// Strip trailing ".0", ".00", etc. from a formatted number string.
/// Handles currency prefixes/signs: finds the decimal point in the numeric
/// portion and removes it if all digits after it are zeros.
fn strip_trailing_zero_decimals(s: &str) -> String {
    // Find the position of the decimal point
    if let Some(dot_pos) = s.rfind('.') {
        // Check if everything after the dot is zeros
        let after_dot = &s[dot_pos + 1..];
        if !after_dot.is_empty() && after_dot.chars().all(|c| c == '0') {
            return s[..dot_pos].to_string();
        }
    }
    s.to_string()
}

/// Format a numeric value for use as an axis tick label.
///
/// This implements D3's automatic tick formatting: plain number notation with
/// appropriate decimal precision derived from the tick step, and comma separators
/// for large numbers. The user's format string (e.g. ".1%", ".3~s", "$,.0f") is
/// intentionally NOT applied — D3's `scale.tickFormat()` with no specifier uses
/// `",f"` with precision from `precisionFixed(tickStep)`.
///
/// `tick_step` is the distance between consecutive ticks (e.g. 50000 for ticks
/// [0, 50000, 100000, ...] or 0.01 for ticks [0.00, 0.01, 0.02, ...]).
pub fn format_tick_value(value: f64, tick_step: f64) -> String {
    // D3's precisionFixed(step): max(0, -floor(log10(abs(step))))
    let precision = if tick_step.abs() < 1e-15 {
        0usize
    } else {
        let p = -(tick_step.abs().log10().floor()) as i64;
        p.max(0) as usize
    };

    // Format with the computed precision
    let formatted = format!("{:.prec$}", value, prec = precision);

    // Insert comma separators into the integer part
    // Split on decimal point
    let (int_part, dec_part) = if let Some(dot_pos) = formatted.find('.') {
        (&formatted[..dot_pos], Some(&formatted[dot_pos..]))
    } else {
        (formatted.as_str(), None)
    };

    // Handle negative sign
    let (sign, digits) = if int_part.starts_with('-') {
        ("-", &int_part[1..])
    } else {
        ("", int_part)
    };

    let with_commas = insert_commas_str(digits);

    match dec_part {
        Some(dec) => format!("{}{}{}", sign, with_commas, dec),
        None => format!("{}{}", sign, with_commas),
    }
}

/// Format a numeric tick value WITHOUT comma separators.
///
/// This matches JS's `d => d` (plain toString) used by horizontal bar charts.
/// Produces the same precision as `format_tick_value` but omits comma grouping.
pub fn format_tick_value_plain(value: f64, tick_step: f64) -> String {
    let precision = if tick_step.abs() < 1e-15 {
        0usize
    } else {
        let p = -(tick_step.abs().log10().floor()) as i64;
        p.max(0) as usize
    };
    format!("{:.prec$}", value, prec = precision)
}

/// Insert commas into a string of digits (no sign).
fn insert_commas_str(digits: &str) -> String {
    let len = digits.len();
    if len <= 3 {
        return digits.to_string();
    }
    let mut result = String::with_capacity(len + len / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result
}

/// Compute the tick step from a slice of ticks, falling back to domain range.
///
/// Prefers the difference between the first two ticks (most reliable). If the
/// ticks slice has fewer than 2 entries, falls back to the domain extent.
fn compute_tick_step(ticks: &[f64], domain: (f64, f64)) -> f64 {
    if ticks.len() >= 2 {
        (ticks[1] - ticks[0]).abs()
    } else {
        (domain.1 - domain.0).abs().max(1.0)
    }
}

/// Default numeric formatting: integers without decimals, floats with 1 decimal.
fn default_format_value(value: f64) -> String {
    if value == value.floor() && value.abs() < 1e15 {
        // Use comma separator for large integers
        let abs = value.abs() as u64;
        let formatted = insert_commas(abs);
        if value < 0.0 {
            format!("-{}", formatted)
        } else {
            formatted
        }
    } else {
        // Use enough decimal places to show significant digits.
        // For values like 0.007, we need 3 decimals; for 1.5, 1 decimal suffices.
        // Compute precision from the value's magnitude: at least 1, and for small
        // values, enough to reveal the first significant fractional digit plus two.
        let abs_val = value.abs();
        let precision = if abs_val < 1e-15 {
            1usize
        } else if abs_val >= 1.0 {
            // For values >= 1, one decimal is fine (e.g. 3.5 -> "3.5")
            1usize
        } else {
            // For values < 1, compute digits needed: -floor(log10(abs)) gives the
            // position of the first significant digit. Add 1 to show at least two
            // significant fractional digits (e.g. 0.007 -> precision 3 -> "0.007").
            let digits = -(abs_val.log10().floor()) as usize;
            digits.max(1)
        };
        // Format and strip unnecessary trailing zeros after the decimal point,
        // but keep at least one decimal digit.
        let formatted = format!("{:.prec$}", value, prec = precision);
        let trimmed = formatted.trim_end_matches('0');
        // Ensure we don't end with just a decimal point (e.g. "3." -> "3.0")
        if trimmed.ends_with('.') {
            format!("{}0", trimmed)
        } else {
            trimmed.to_string()
        }
    }
}

fn insert_commas(n: u64) -> String {
    let s = n.to_string();
    let len = s.len();
    if len <= 3 {
        return s;
    }
    let mut result = String::with_capacity(len + len / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result
}

/// Result of x-axis generation, including the computed label strategy.
pub struct XAxisResult {
    pub elements: Vec<ChartElement>,
}

/// Generate x-axis elements with smart label handling.
/// Applies LabelStrategy (horizontal/rotated/truncated/sampled) based on available space.
/// Auto-detects date labels and reformats them if no explicit format is provided.
pub fn generate_x_axis(
    labels: &[String],
    range: (f64, f64),
    y_position: f64,
    available_width: f64,
    x_format: Option<&str>,
    chart_height: Option<f64>,
    grid: &GridConfig,
) -> XAxisResult {
    generate_x_axis_with_display(labels, None, range, y_position, available_width, x_format, chart_height, grid)
}

/// Generate x-axis with optional separate display labels.
/// `band_keys` are used for ScaleBand positioning (must be unique).
/// `display_label_overrides`, when Some, provides the text to show (may contain duplicates).
pub fn generate_x_axis_with_display(
    band_keys: &[String],
    display_label_overrides: Option<&[String]>,
    range: (f64, f64),
    y_position: f64,
    available_width: f64,
    x_format: Option<&str>,
    chart_height: Option<f64>,
    grid: &GridConfig,
) -> XAxisResult {
    let band = ScaleBand::new(band_keys.to_vec(), range);
    let bandwidth = band.bandwidth();

    // Use display overrides if provided, otherwise use band_keys as labels
    let raw_labels: &[String] = display_label_overrides.unwrap_or(band_keys);

    // Step 1: Format labels (date detection or explicit format)
    let display_labels: Vec<String> = if let Some(fmt) = x_format {
        raw_labels.iter().map(|l| reformat_date_label(l, fmt)).collect()
    } else if let Some(detected_fmt) = detect_date_format(raw_labels) {
        raw_labels.iter().map(|l| reformat_date_label(l, &detected_fmt)).collect()
    } else {
        raw_labels.to_vec()
    };

    // Step 2: Determine label strategy
    let strategy = LabelStrategy::determine(&display_labels, available_width, &LabelStrategyConfig::default());

    let mut elements = Vec::new();
    // Margin for rotated labels is pre-computed via MarginConfig.x_label_strategy_margin

    // Axis line
    elements.push(ChartElement::Line {
        x1: range.0,
        y1: y_position,
        x2: range.1,
        y2: y_position,
        stroke: "#ccc".to_string(),
        stroke_width: Some(1.0),
        stroke_dasharray: None,
        class: "axis-line".to_string(),
    });

    // Vertical grid lines (if grid.show_x and chart_height provided)
    if grid.show_x {
        if let Some(ch) = chart_height {
            for (_i, band_key) in band_keys.iter().enumerate() {
                let x = match band.map(band_key) {
                    Some(x) => x + bandwidth / 2.0,
                    None => continue,
                };
                elements.push(ChartElement::Line {
                    x1: x, y1: y_position, x2: x, y2: y_position - ch,
                    stroke: grid.color.clone(), stroke_width: Some(1.0),
                    stroke_dasharray: grid.dash_array.clone(),
                    class: "grid-line grid-line-x".to_string(),
                });
            }
        }
    }

    // Step 3: Apply strategy
    match &strategy {
        LabelStrategy::Horizontal => {
            // no extra margin needed
            for (i, label) in display_labels.iter().enumerate() {
                let orig_label = &band_keys[i];
                let x = match band.map(orig_label) {
                    Some(x) => x + bandwidth / 2.0,
                    None => continue,
                };
                // Tick mark
                elements.push(ChartElement::Line {
                    x1: x, y1: y_position, x2: x, y2: y_position + 5.0,
                    stroke: "#999".to_string(), stroke_width: Some(1.0),
                    stroke_dasharray: None, class: "tick".to_string(),
                });
                // Label
                elements.push(ChartElement::Text {
                    x, y: y_position + 18.0,
                    content: label.clone(),
                    anchor: TextAnchor::Middle,
                    dominant_baseline: None,
                    transform: None,
                    font_size: Some("11px".to_string()),
                    font_weight: None,
                    fill: Some("#666".to_string()),
                    class: "tick-label".to_string(),
                    data: None,
                });
            }
        }

        LabelStrategy::Rotated { margin: _, skip_factor } => {
            // rotation margin handled by MarginConfig
            for (i, label) in display_labels.iter().enumerate() {
                let orig_label = &band_keys[i];
                let x = match band.map(orig_label) {
                    Some(x) => x + bandwidth / 2.0,
                    None => continue,
                };
                // Tick mark (always shown)
                elements.push(ChartElement::Line {
                    x1: x, y1: y_position, x2: x, y2: y_position + 5.0,
                    stroke: "#999".to_string(), stroke_width: Some(1.0),
                    stroke_dasharray: None, class: "tick".to_string(),
                });
                // Label — skip if skip_factor says so
                let should_show = match skip_factor {
                    Some(factor) => i % factor == 0,
                    None => true,
                };
                if should_show {
                    elements.push(ChartElement::Text {
                        x, y: y_position + 10.0,
                        content: label.clone(),
                        anchor: TextAnchor::End,
                        dominant_baseline: None,
                        transform: Some(Transform::Rotate(-45.0, x, y_position + 10.0)),
                        font_size: Some("11px".to_string()),
                        font_weight: None,
                        fill: Some("#666".to_string()),
                        class: "tick-label".to_string(),
                        data: None,
                    });
                }
            }
        }

        LabelStrategy::Truncated { max_width } => {
            // no extra margin needed
            for (i, label) in display_labels.iter().enumerate() {
                let orig_label = &band_keys[i];
                let x = match band.map(orig_label) {
                    Some(x) => x + bandwidth / 2.0,
                    None => continue,
                };
                elements.push(ChartElement::Line {
                    x1: x, y1: y_position, x2: x, y2: y_position + 5.0,
                    stroke: "#999".to_string(), stroke_width: Some(1.0),
                    stroke_dasharray: None, class: "tick".to_string(),
                });
                let truncated = truncate_label(label, *max_width);
                let is_truncated = truncated != *label;
                elements.push(ChartElement::Text {
                    x, y: y_position + 18.0,
                    content: truncated,
                    anchor: TextAnchor::Middle,
                    dominant_baseline: None,
                    transform: None,
                    font_size: Some("11px".to_string()),
                    font_weight: None,
                    fill: Some("#666".to_string()),
                    class: "tick-label".to_string(),
                    data: if is_truncated {
                        Some(ElementData::new(label.clone(), ""))
                    } else {
                        None
                    },
                });
            }
        }

        LabelStrategy::Sampled { indices } => {
            // no extra margin needed
            for (i, label) in display_labels.iter().enumerate() {
                let orig_label = &band_keys[i];
                let x = match band.map(orig_label) {
                    Some(x) => x + bandwidth / 2.0,
                    None => continue,
                };
                // Tick mark for all
                elements.push(ChartElement::Line {
                    x1: x, y1: y_position, x2: x, y2: y_position + 5.0,
                    stroke: "#999".to_string(), stroke_width: Some(1.0),
                    stroke_dasharray: None, class: "tick".to_string(),
                });
                // Label only for sampled indices
                if indices.contains(&i) {
                    elements.push(ChartElement::Text {
                        x, y: y_position + 18.0,
                        content: label.clone(),
                        anchor: TextAnchor::Middle,
                        dominant_baseline: None,
                        transform: None,
                        font_size: Some("11px".to_string()),
                        font_weight: None,
                        fill: Some("#666".to_string()),
                        class: "tick-label".to_string(),
                        data: None,
                    });
                }
            }
        }
    }

    XAxisResult { elements }
}

/// Generate y-axis elements for category data (used in horizontal bar charts).
pub fn generate_y_axis(
    labels: &[String],
    range: (f64, f64),
    x_position: f64,
    _formatter: Option<&str>,
) -> Vec<ChartElement> {
    generate_y_axis_with_display(labels, None, range, x_position, _formatter)
}

/// Generate y-axis with optional separate display labels.
/// `band_keys` are used for ScaleBand positioning (must be unique).
/// `display_label_overrides`, when Some, provides the text to show (may contain duplicates).
pub fn generate_y_axis_with_display(
    band_keys: &[String],
    display_label_overrides: Option<&[String]>,
    range: (f64, f64),
    x_position: f64,
    _formatter: Option<&str>,
) -> Vec<ChartElement> {
    let band = ScaleBand::new(band_keys.to_vec(), range);
    let bandwidth = band.bandwidth();
    let mut elements = Vec::new();

    let display_labels: &[String] = display_label_overrides.unwrap_or(band_keys);

    // Axis line
    elements.push(ChartElement::Line {
        x1: x_position,
        y1: range.0.min(range.1),
        x2: x_position,
        y2: range.0.max(range.1),
        stroke: "#ccc".to_string(),
        stroke_width: Some(1.0),
        stroke_dasharray: None,
        class: "axis-line".to_string(),
    });

    for (i, band_key) in band_keys.iter().enumerate() {
        let y = match band.map(band_key) {
            Some(y) => y + bandwidth / 2.0,
            None => continue,
        };

        // Tick mark
        elements.push(ChartElement::Line {
            x1: x_position - 5.0,
            y1: y,
            x2: x_position,
            y2: y,
            stroke: "#999".to_string(),
            stroke_width: Some(1.0),
            stroke_dasharray: None,
            class: "tick".to_string(),
        });

        // Label
        elements.push(ChartElement::Text {
            x: x_position - 8.0,
            y,
            content: display_labels[i].clone(),
            anchor: TextAnchor::End,
            dominant_baseline: Some("middle".to_string()),
            transform: None,
            font_size: Some("11px".to_string()),
            font_weight: None,
            fill: Some("#666".to_string()),
            class: "tick-label".to_string(),
            data: None,
        });
    }

    elements
}

/// Generate y-axis elements for numeric data (used by bar, line, and area charts).
/// Grid lines are controlled by `grid` config and `chart_width`.
pub fn generate_y_axis_numeric(
    domain: (f64, f64),
    range: (f64, f64),
    x_position: f64,
    fmt: Option<&str>,
    tick_count: usize,
    chart_width: Option<f64>,
    grid: &GridConfig,
    axis_label: Option<&str>,
) -> Vec<ChartElement> {
    let scale = ScaleLinear::new(domain, range);
    // Match JS: d3.axisLeft(yLeft).ticks(5) — fixed count of 5 regardless of tick_count param.
    // tick_count is kept for future use / callers that may pass it.
    let _ = tick_count;
    let ticks = d3_ticks(domain.0, domain.1, 5);
    // Compute the tick step for automatic formatting (D3's tickStep)
    let tick_step = compute_tick_step(&ticks, domain);
    let mut elements = Vec::new();

    // Axis line
    elements.push(ChartElement::Line {
        x1: x_position,
        y1: range.0.min(range.1),
        x2: x_position,
        y2: range.0.max(range.1),
        stroke: "#ccc".to_string(),
        stroke_width: Some(1.0),
        stroke_dasharray: None,
        class: "axis-line".to_string(),
    });

    for val in &ticks {
        let y = scale.map(*val);
        // When an explicit format string is provided (e.g. ".0%" for normalized
        // stacked charts), use it with SI abbreviation for large values.
        // Otherwise fall back to D3-style auto-formatting.
        let label = match fmt {
            Some(f) => format_tick_value_si(*val, tick_step, f),
            None => format_tick_value(*val, tick_step),
        };

        // Horizontal grid line
        if grid.show_y {
            if let Some(cw) = chart_width {
                elements.push(ChartElement::Line {
                    x1: x_position,
                    y1: y,
                    x2: x_position + cw,
                    y2: y,
                    stroke: grid.color.clone(),
                    stroke_width: Some(1.0),
                    stroke_dasharray: grid.dash_array.clone(),
                    class: format!("grid-line grid-line-y"),
                });
            }
        }

        // Tick mark
        elements.push(ChartElement::Line {
            x1: x_position - 5.0,
            y1: y,
            x2: x_position,
            y2: y,
            stroke: "#999".to_string(),
            stroke_width: Some(1.0),
            stroke_dasharray: None,
            class: "tick".to_string(),
        });

        elements.push(ChartElement::Text {
            x: x_position - 8.0,
            y,
            content: label,
            anchor: TextAnchor::End,
            dominant_baseline: Some("middle".to_string()),
            transform: None,
            font_size: Some("11px".to_string()),
            font_weight: None,
            fill: Some("#666".to_string()),
            class: "tick-label".to_string(),
            data: None,
        });
    }

    // Axis label (rotated -90°, centered along the axis)
    if let Some(label_text) = axis_label {
        let mid_y = (range.0 + range.1) / 2.0;
        // Position left of the widest tick label: estimate max tick width and offset
        let max_tick_width = ticks.iter()
            .map(|val| {
                let label = match fmt {
                    Some(f) => format_value(*val, Some(f)),
                    None => format_tick_value(*val, tick_step),
                };
                approximate_text_width(&label)
            })
            .fold(0.0_f64, f64::max);
        let label_x = (x_position - 8.0 - max_tick_width - 12.0).max(10.0);
        elements.push(ChartElement::Text {
            x: label_x,
            y: mid_y,
            content: label_text.to_string(),
            anchor: TextAnchor::Middle,
            dominant_baseline: Some("middle".to_string()),
            transform: Some(Transform::Rotate(-90.0, label_x, mid_y)),
            font_size: Some("12px".to_string()),
            font_weight: None,
            fill: Some("#666".to_string()),
            class: "axis-label".to_string(),
            data: None,
        });
    }

    elements
}

/// Generate a right-side y-axis (ticks and labels to the right of x_position).
pub fn generate_y_axis_numeric_right(
    domain: (f64, f64),
    range: (f64, f64),
    x_position: f64,
    _fmt: Option<&str>,
    tick_count: usize,
    axis_label: Option<&str>,
) -> Vec<ChartElement> {
    let scale = ScaleLinear::new(domain, range);
    let ticks = scale.ticks(tick_count);
    let tick_step = compute_tick_step(&ticks, domain);
    let mut elements = Vec::new();

    // Axis line
    elements.push(ChartElement::Line {
        x1: x_position, y1: range.0.min(range.1),
        x2: x_position, y2: range.0.max(range.1),
        stroke: "#ccc".to_string(), stroke_width: Some(1.0),
        stroke_dasharray: None, class: "axis-line".to_string(),
    });

    for val in &ticks {
        let y = scale.map(*val);
        let label = format_tick_value(*val, tick_step);

        // Tick mark (to the right)
        elements.push(ChartElement::Line {
            x1: x_position, y1: y,
            x2: x_position + 5.0, y2: y,
            stroke: "#999".to_string(), stroke_width: Some(1.0),
            stroke_dasharray: None, class: "tick".to_string(),
        });

        // Label (to the right)
        elements.push(ChartElement::Text {
            x: x_position + 8.0, y,
            content: label,
            anchor: TextAnchor::Start,
            dominant_baseline: Some("middle".to_string()),
            transform: None,
            font_size: Some("11px".to_string()),
            font_weight: None,
            fill: Some("#666".to_string()),
            class: "tick-label".to_string(),
            data: None,
        });
    }

    // Axis label (rotated 90°, centered along the axis, to the right of tick labels)
    if let Some(label_text) = axis_label {
        let mid_y = (range.0 + range.1) / 2.0;
        let label_x = x_position + 45.0;
        elements.push(ChartElement::Text {
            x: label_x,
            y: mid_y,
            content: label_text.to_string(),
            anchor: TextAnchor::Middle,
            dominant_baseline: Some("middle".to_string()),
            transform: Some(Transform::Rotate(90.0, label_x, mid_y)),
            font_size: Some("12px".to_string()),
            font_weight: None,
            fill: Some("#666".to_string()),
            class: "axis-label".to_string(),
            data: None,
        });
    }

    elements
}

/// Generate x-axis elements for numeric data (used by horizontal bar charts).
/// Grid lines controlled by `grid` config and `chart_height`.
pub fn generate_x_axis_numeric(
    domain: (f64, f64),
    range: (f64, f64),
    y_position: f64,
    fmt: Option<&str>,
    tick_count: usize,
    chart_height: Option<f64>,
    grid: &GridConfig,
) -> Vec<ChartElement> {
    let scale = ScaleLinear::new(domain, range);
    let ticks = scale.ticks(tick_count);
    let tick_step = compute_tick_step(&ticks, domain);
    let mut elements = Vec::new();

    // Axis line
    elements.push(ChartElement::Line {
        x1: range.0,
        y1: y_position,
        x2: range.1,
        y2: y_position,
        stroke: "#ccc".to_string(),
        stroke_width: Some(1.0),
        stroke_dasharray: None,
        class: "axis-line".to_string(),
    });

    for val in &ticks {
        let x = scale.map(*val);
        // JS horizontal bar uses `d => d` (plain toString) as default tick format,
        // which does NOT add comma separators. Only use formatted output when an
        // explicit format string is provided. Apply SI abbreviation for large values.
        let label = match fmt {
            Some(f) => format_tick_value_si(*val, tick_step, f),
            None => format_tick_value_plain(*val, tick_step),
        };

        // Vertical grid line
        if grid.show_x {
            if let Some(ch) = chart_height {
                elements.push(ChartElement::Line {
                    x1: x,
                    y1: y_position,
                    x2: x,
                    y2: y_position - ch,
                    stroke: grid.color.clone(),
                    stroke_width: Some(1.0),
                    stroke_dasharray: grid.dash_array.clone(),
                    class: "grid-line grid-line-x".to_string(),
                });
            }
        }

        elements.push(ChartElement::Line {
            x1: x,
            y1: y_position,
            x2: x,
            y2: y_position + 5.0,
            stroke: "#999".to_string(),
            stroke_width: Some(1.0),
            stroke_dasharray: None,
            class: "tick".to_string(),
        });

        elements.push(ChartElement::Text {
            x,
            y: y_position + 18.0,
            content: label,
            anchor: TextAnchor::Middle,
            dominant_baseline: None,
            transform: None,
            font_size: Some("11px".to_string()),
            font_weight: None,
            fill: Some("#666".to_string()),
            class: "tick-label".to_string(),
            data: None,
        });
    }

    elements
}

// Re-export from core for backward compatibility
pub use chartml_core::layout::legend::LegendMark;

/// Generate legend elements for multi-series charts (default: Rect marks).
pub fn generate_legend(
    series_names: &[String],
    colors: &[String],
    chart_width: f64,
    y_position: f64,
) -> Vec<ChartElement> {
    chartml_core::layout::legend::generate_legend_elements(series_names, colors, chart_width, y_position, LegendMark::Rect)
}

/// Generate legend with a specific symbol mark type.
pub fn generate_legend_with_mark(
    series_names: &[String],
    colors: &[String],
    chart_width: f64,
    y_position: f64,
    mark: LegendMark,
) -> Vec<ChartElement> {
    chartml_core::layout::legend::generate_legend_elements(series_names, colors, chart_width, y_position, mark)
}

/// Generate ticks matching D3's `ticks(start, stop, count)` algorithm exactly.
///
/// This is a direct port of `ticks()` from d3-array/src/ticks.js, using the same
/// `tickSpec` logic (via `d3_tick_spec`) to compute `i1`, `i2`, and `inc`, then
/// producing tick values identical to D3.
///
/// For domain (0, 200000) with count=5 this returns [0, 50000, 100000, 150000, 200000].
pub fn d3_ticks(start: f64, stop: f64, count: usize) -> Vec<f64> {
    if count == 0 {
        return vec![];
    }
    if start == stop {
        return vec![start];
    }
    let reverse = stop < start;
    let (s0, s1) = if reverse { (stop, start) } else { (start, stop) };
    let (i1, i2, inc) = d3_tick_spec(s0, s1, count as f64);
    if i2 < i1 {
        return vec![];
    }
    let n = (i2 - i1 + 1.0).round() as usize;
    let mut ticks = Vec::with_capacity(n);
    if reverse {
        if inc < 0.0 {
            for i in 0..n {
                ticks.push((i2 - i as f64) / -inc);
            }
        } else {
            for i in 0..n {
                ticks.push((i2 - i as f64) * inc);
            }
        }
    } else {
        if inc < 0.0 {
            for i in 0..n {
                ticks.push((i1 + i as f64) / -inc);
            }
        } else {
            for i in 0..n {
                ticks.push((i1 + i as f64) * inc);
            }
        }
    }
    ticks
}

/// Compute D3's `tickSpec(start, stop, count)` — returns `(i1, i2, inc)`.
///
/// This is a direct port of the internal `tickSpec` function from d3-array/src/ticks.js.
/// - When `power >= 0`: `inc = 10^power * factor`, `i1/i2` are integer multiples of `inc`
/// - When `power < 0`: `inc = -(10^(-power) / factor)`, `i1/i2` are integer multiples of `1/(-inc)`
fn d3_tick_spec(start: f64, stop: f64, count: f64) -> (f64, f64, f64) {
    let e10: f64 = 50_f64.sqrt(); // ≈ 7.071
    let e5: f64 = 10_f64.sqrt();  // ≈ 3.162
    let e2: f64 = 2_f64.sqrt();   // ≈ 1.414

    let step = (stop - start) / count.max(0.0);
    let power = step.log10().floor();
    let error = step / 10_f64.powf(power);
    let factor = if error >= e10 {
        10.0
    } else if error >= e5 {
        5.0
    } else if error >= e2 {
        2.0
    } else {
        1.0
    };

    if power < 0.0 {
        let inc = 10_f64.powf(-power) / factor;
        let mut i1 = (start * inc).round();
        let mut i2 = (stop * inc).round();
        if i1 / inc < start { i1 += 1.0; }
        if i2 / inc > stop  { i2 -= 1.0; }
        (i1, i2, -inc)
    } else {
        let inc = 10_f64.powf(power) * factor;
        let mut i1 = (start / inc).round();
        let mut i2 = (stop / inc).round();
        if i1 * inc < start { i1 += 1.0; }
        if i2 * inc > stop  { i2 -= 1.0; }
        (i1, i2, inc)
    }
}

/// Compute the D3 `tickIncrement(start, stop, count)` value.
///
/// This matches D3's `tickSpec` return value `inc` exactly:
/// - When `power >= 0`: returns `10^power * factor` (positive integer step)
/// - When `power < 0`: returns `-(10^(-power) / factor)` (negative, meaning step = 1/(-inc))
///
/// This is the Rust port of `tickIncrement` from d3-array/src/ticks.js.
fn d3_tick_increment(start: f64, stop: f64, count: f64) -> f64 {
    let e10: f64 = 50_f64.sqrt(); // ≈ 7.071
    let e5: f64 = 10_f64.sqrt();  // ≈ 3.162
    let e2: f64 = 2_f64.sqrt();   // ≈ 1.414

    let step = (stop - start) / count.max(0.0);
    let power = step.log10().floor();
    let error = step / 10_f64.powf(power);
    let factor = if error >= e10 {
        10.0
    } else if error >= e5 {
        5.0
    } else if error >= e2 {
        2.0
    } else {
        1.0
    };

    if power < 0.0 {
        // Negative inc: actual step = 1/(-inc)
        -(10_f64.powf(-power) / factor)
    } else {
        10_f64.powf(power) * factor
    }
}

/// Compute a "nice" domain for a numeric axis.
///
/// This is a direct port of D3's `scale.nice(count)` from d3-scale/src/linear.js.
/// It iterates up to 10 times using `tickIncrement`, flooring the start and ceiling
/// the stop at each step, until the step stabilises. This produces identical results
/// to D3 for any input range and count.
///
/// Example: `nice_domain(0.0, 152_000.0, 5)` → `(0.0, 200_000.0)` (step 50_000),
/// matching D3's output of ticks 0, 50k, 100k, 150k, 200k.
pub fn nice_domain(domain_min: f64, domain_max: f64, tick_count: usize) -> (f64, f64) {
    let reversed = domain_min > domain_max;
    let (mut start, mut stop) = if reversed {
        (domain_max, domain_min)
    } else {
        (domain_min, domain_max)
    };

    if start == stop {
        return (domain_min, domain_max);
    }

    let count = tick_count.max(1) as f64;
    let mut prestep = f64::NAN;
    let mut max_iter = 10i32;

    while max_iter > 0 {
        max_iter -= 1;
        let step = d3_tick_increment(start, stop, count);
        if step == prestep {
            // Converged
            break;
        } else if step > 0.0 {
            start = (start / step).floor() * step;
            stop = (stop / step).ceil() * step;
        } else if step < 0.0 {
            // D3: start = Math.ceil(start * step) / step;
            //     stop  = Math.floor(stop * step) / step;
            // step is NEGATIVE, so multiplying by step flips sign.
            // floor(negative) rounds towards -∞, then dividing by negative
            // flips back — net effect is rounding OUTWARD (expanding domain).
            start = (start * step).ceil() / step;
            stop = (stop * step).floor() / step;
        } else {
            break;
        }
        prestep = step;
    }

    if reversed {
        (stop, start)
    } else {
        (start, stop)
    }
}

#[cfg(test)]
mod tick_tests {
    use super::d3_ticks;

    #[test]
    fn d3_ticks_200k_domain_count5() {
        // Verification case from the bug report:
        // domain (0, 200000) with count=5 must produce [0, 50000, 100000, 150000, 200000]
        let ticks = d3_ticks(0.0, 200_000.0, 5);
        let expected = vec![0.0, 50_000.0, 100_000.0, 150_000.0, 200_000.0];
        assert_eq!(ticks.len(), expected.len(), "wrong tick count: {:?}", ticks);
        for (got, exp) in ticks.iter().zip(expected.iter()) {
            assert!((got - exp).abs() < 1e-6, "tick mismatch: got {}, expected {}", got, exp);
        }
    }

    #[test]
    fn d3_ticks_0_to_100_count5() {
        // Standard case: should produce [0, 20, 40, 60, 80, 100]
        let ticks = d3_ticks(0.0, 100.0, 5);
        let expected = vec![0.0, 20.0, 40.0, 60.0, 80.0, 100.0];
        assert_eq!(ticks.len(), expected.len(), "wrong tick count: {:?}", ticks);
        for (got, exp) in ticks.iter().zip(expected.iter()) {
            assert!((got - exp).abs() < 1e-6, "tick mismatch: got {}, expected {}", got, exp);
        }
    }

    #[test]
    fn d3_ticks_empty_on_zero_count() {
        assert!(d3_ticks(0.0, 100.0, 0).is_empty());
    }

    #[test]
    fn d3_ticks_single_on_equal_bounds() {
        let ticks = d3_ticks(50.0, 50.0, 5);
        assert_eq!(ticks, vec![50.0]);
    }
}

/// Offset an element's position by wrapping it in a Group with a Translate transform.
pub fn offset_element(element: ChartElement, dx: f64, dy: f64) -> ChartElement {
    if dx == 0.0 && dy == 0.0 {
        return element;
    }
    ChartElement::Group {
        class: String::new(),
        transform: Some(Transform::Translate(dx, dy)),
        children: vec![element],
    }
}

/// Convert a hex color string (e.g. "#34a853") and opacity (0.0–1.0) to an rgba() CSS string.
/// Falls back to the original color string if parsing fails.
fn hex_to_rgba(hex: &str, opacity: f64) -> String {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return format!("rgba(0,0,0,{})", opacity);
    }
    let r = u8::from_str_radix(&hex[0..2], 16);
    let g = u8::from_str_radix(&hex[2..4], 16);
    let b = u8::from_str_radix(&hex[4..6], 16);
    match (r, g, b) {
        (Ok(r), Ok(g), Ok(b)) => format!("rgba({},{},{},{})", r, g, b, opacity),
        _ => format!("rgba(0,0,0,{})", opacity),
    }
}

/// Extract a numeric f64 from a serde_json::Value (number or numeric string).
fn json_to_f64(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

/// Generate annotation elements for a chart.
///
/// Annotations are rendered in the chart's inner coordinate space (before margin offset).
/// The caller must apply `offset_element` with `(margins.left, margins.top)` to position
/// them correctly.
///
/// - `scale_y`: maps data values to pixel y-coordinates (higher data value → smaller y)
/// - `x_start`: left edge of the chart area (0.0 in inner coordinates)
/// - `x_end`: right edge of the chart area (= inner_width)
/// - `inner_height`: height of the chart area (used for clipping reference)
pub fn generate_annotations(
    annotations: &[AnnotationSpec],
    scale_y: &ScaleLinear,
    x_start: f64,
    x_end: f64,
    inner_height: f64,
    // Optional x-axis categories for vertical annotations (used to map category value → pixel x)
    x_categories: Option<&[String]>,
) -> Vec<ChartElement> {
    let mut elements = Vec::new();

    // Resolve the effective dash array for an annotation: explicit dash_array takes
    // precedence, then the style shorthand ("dashed" → "6,4", "dotted" → "2,3").
    let resolve_dash_array = |ann: &AnnotationSpec| -> Option<String> {
        if ann.dash_array.is_some() {
            return ann.dash_array.clone();
        }
        match ann.style.as_deref() {
            Some("dashed") => Some("6,4".to_string()),
            Some("dotted") => Some("2,3".to_string()),
            _ => None,
        }
    };

    for ann in annotations {
        let ann_type = ann.annotation_type.as_str();
        let orientation = ann.orientation.as_deref().unwrap_or("horizontal");

        if ann_type == "line" && orientation == "vertical" {
            // Vertical line annotation — value is an x-axis category label
            let value_str = match ann.value.as_ref() {
                Some(v) => v.as_str().unwrap_or("").to_string(),
                None => continue,
            };
            let x_px = if let Some(cats) = x_categories {
                if let Some(idx) = cats.iter().position(|c| c == &value_str) {
                    let step = (x_end - x_start) / cats.len() as f64;
                    x_start + step * idx as f64 + step / 2.0
                } else {
                    continue;
                }
            } else {
                continue;
            };
            let color = ann.color.as_deref().unwrap_or("#666").to_string();
            let stroke_width = ann.stroke_width;
            let dash_array = resolve_dash_array(ann);

            elements.push(ChartElement::Line {
                x1: x_px, y1: 0.0,
                x2: x_px, y2: inner_height,
                stroke: color.clone(),
                stroke_width,
                stroke_dasharray: dash_array,
                class: "annotation-line annotation-vertical".to_string(),
            });

            if let Some(ref label) = ann.label {
                elements.push(ChartElement::Text {
                    x: x_px + 4.0,
                    y: 14.0,
                    content: label.clone(),
                    anchor: TextAnchor::Start,
                    dominant_baseline: None,
                    transform: None,
                    font_size: Some("11px".to_string()),
                    font_weight: None,
                    fill: Some(color.clone()),
                    class: "annotation-label".to_string(),
                    data: None,
                });
            }
        } else if ann_type == "line" && orientation == "horizontal" {
            let value = match ann.value.as_ref().and_then(json_to_f64) {
                Some(v) => v,
                None => continue,
            };
            let y_px = scale_y.map(value);
            let color = ann.color.as_deref().unwrap_or("#666").to_string();
            let stroke_width = ann.stroke_width;
            let dash_array = resolve_dash_array(ann);

            elements.push(ChartElement::Line {
                x1: x_start,
                y1: y_px,
                x2: x_end,
                y2: y_px,
                stroke: color.clone(),
                stroke_width,
                stroke_dasharray: dash_array,
                class: "annotation-line".to_string(),
            });

            if let Some(ref label) = ann.label {
                let label_position = ann.label_position.as_deref().unwrap_or("end");
                let (label_x, anchor) = if label_position == "end" {
                    (x_end, TextAnchor::End)
                } else {
                    (x_start, TextAnchor::Start)
                };
                elements.push(ChartElement::Text {
                    x: label_x,
                    y: y_px - 4.0,
                    content: label.clone(),
                    anchor,
                    dominant_baseline: None,
                    transform: None,
                    font_size: Some("11px".to_string()),
                    font_weight: None,
                    fill: Some(color.clone()),
                    class: "annotation-label".to_string(),
                    data: None,
                });
            }
        } else if ann_type == "band" && orientation == "horizontal" {
            let from_val = match ann.from.as_ref().and_then(json_to_f64) {
                Some(v) => v,
                None => continue,
            };
            let to_val = match ann.to.as_ref().and_then(json_to_f64) {
                Some(v) => v,
                None => continue,
            };

            let y_from = scale_y.map(from_val);
            let y_to = scale_y.map(to_val);
            let y_top = y_from.min(y_to);
            let band_height = (y_from - y_to).abs();
            let band_width = x_end - x_start;

            let color = ann.color.as_deref().unwrap_or("#666");
            let opacity = ann.opacity.unwrap_or(0.15);
            let fill_color = hex_to_rgba(color, opacity);

            elements.push(ChartElement::Rect {
                x: x_start,
                y: y_top,
                width: band_width,
                height: band_height,
                fill: fill_color,
                stroke: ann.stroke_color.clone(),
                class: "annotation-band".to_string(),
                data: None,
            });

            if let Some(ref label) = ann.label {
                elements.push(ChartElement::Text {
                    x: x_start + 4.0,
                    y: y_top + 12.0,
                    content: label.clone(),
                    anchor: TextAnchor::Start,
                    dominant_baseline: None,
                    transform: None,
                    font_size: Some("11px".to_string()),
                    font_weight: None,
                    fill: Some(ann.color.clone().unwrap_or_else(|| "#666".to_string())),
                    class: "annotation-label".to_string(),
                    data: None,
                });
            }
        }
    }

    elements
}
