//! ChartML `table` chart — tabular renderer that emits an HTML table
//! via `ChartElement::Div`/`Span` nodes, styled from `Theme` table tokens
//! and tagged with stable `chartml-table-*` class names for consumer CSS
//! overrides.
//!
//! Drop-in: changing `type: bar` to `type: table` on an existing cartesian
//! spec renders the same `columns` + `rows` fields as a table. Column
//! labels come from `FieldSpec.label`, cell formatting from `FieldSpec.format`,
//! and pagination from `visualize.style.pageSize`.

use std::collections::HashMap;

use chartml_core::data::DataTable;
use chartml_core::element::*;
use chartml_core::error::ChartError;
use chartml_core::format::NumberFormatter;
use chartml_core::layout::labels::{measure_text, TextMetrics};
use chartml_core::plugin::{ChartConfig, ChartRenderer};
use chartml_core::spec::{FieldRef, FieldRefItem, FieldSpec, VisualizeSpec};
use chartml_core::theme::{TextTransform, Theme};

const DEFAULT_PAGE_SIZE: usize = 50;
const DEFAULT_HEIGHT: f64 = 400.0;

/// A column's minimum text width, in `em` of the table font, is the width
/// of its header label or its widest value on the page, whichever is wider,
/// kept between these bounds. Columns at their minimum show every value on
/// one line up to `MAX_MIN_TEXT_EM`; a longer value wraps.
const MIN_MIN_TEXT_EM: f64 = 4.0;
const MAX_MIN_TEXT_EM: f64 = 18.0;

/// Text is measured at this font size (the estimator's calibration); dividing
/// by it gives `em`. The estimate is widened by `TEXT_WIDTH_SLACK` because real
/// fonts can run wider than the table.
const TEXT_WIDTH_CALIBRATION_PX: f64 = 12.0;
const TEXT_WIDTH_SLACK: f64 = 1.15;

/// Largest share of the scroll area's width the frozen first column may
/// take at its minimum, so the scrolling columns stay reachable beside it.
const FROZEN_MAX_SHARE: &str = "50cqw";

/// Width of the right-edge fade that signals more columns off to the right
/// (see `fade_layers`).
const FADE_WIDTH_PX: u32 = 32;

/// Resolved column: the source field name + display label + optional format.
struct Column {
    field: String,
    label: String,
    format: Option<String>,
}

impl Column {
    fn plain(field: &str) -> Self {
        Self {
            field: field.to_string(),
            label: field.to_string(),
            format: None,
        }
    }

    /// Returns `None` for range-mark specs, which have no single field name
    /// and don't correspond to a column in a tabular view.
    fn from_field_spec(spec: &FieldSpec) -> Option<Self> {
        let field = spec.field.clone()?;
        Some(Self {
            label: spec.label.clone().unwrap_or_else(|| field.clone()),
            field,
            format: spec.format.clone(),
        })
    }
}

#[derive(Default)]
pub struct TableRenderer;

impl TableRenderer {
    pub fn new() -> Self {
        Self
    }
}

/// Collect columns from `visualize.columns` and `visualize.rows`. The category
/// field (columns) comes first, then each measure (rows) in order. If neither
/// is set, fall back to all fields in the DataTable schema.
fn resolve_columns(viz: &VisualizeSpec, data: &DataTable) -> Vec<Column> {
    let mut out: Vec<Column> = Vec::new();

    if let Some(cols) = &viz.columns {
        append_field_ref(&mut out, cols);
    }
    if let Some(rows) = &viz.rows {
        append_field_ref(&mut out, rows);
    }

    if out.is_empty() {
        // No spec — show every field in the table, in schema order.
        for name in data.field_names() {
            out.push(Column::plain(&name));
        }
    }

    out
}

fn append_field_ref(out: &mut Vec<Column>, field_ref: &FieldRef) {
    match field_ref {
        FieldRef::Simple(name) => out.push(Column::plain(name)),
        FieldRef::Detailed(spec) => {
            if let Some(col) = Column::from_field_spec(spec) {
                out.push(col);
            }
        }
        FieldRef::Multiple(items) => {
            for item in items {
                match item {
                    FieldRefItem::Simple(name) => out.push(Column::plain(name)),
                    FieldRefItem::Detailed(spec) => {
                        if let Some(col) = Column::from_field_spec(spec) {
                            out.push(col);
                        }
                    }
                }
            }
        }
    }
}

/// Whether a column holds numbers, from the Arrow data type of its field.
/// Numeric columns are right-aligned, header included. A field missing from
/// the data is not numeric.
fn is_numeric_column(data: &DataTable, field: &str) -> bool {
    data.schema()
        .field_with_name(field)
        .map(|f| f.data_type().is_numeric())
        .unwrap_or(false)
}

/// Format a cell value for display.
///
/// When an explicit format string is present, numeric values go through
/// `NumberFormatter`. Otherwise `get_string` is used — it returns the
/// type-aware representation for every Arrow type (ISO timestamps, dates,
/// formatted numbers, etc.) so temporal columns are never shown as raw
/// epoch millis.
fn format_cell(data: &DataTable, row: usize, col: &Column) -> String {
    if let Some(fmt) = &col.format {
        if let Some(n) = data.get_f64(row, &col.field) {
            return NumberFormatter::new(fmt).format(n);
        }
    }
    match data.get_string(row, &col.field) {
        Some(s) => s,
        None => "\u{2014}".to_string(),
    }
}

/// Minimum text width of a column in `em`: the wider of its header label
/// (measured with the header's text transform and letter spacing) and its
/// widest value on the page, widened by `TEXT_WIDTH_SLACK`, kept within
/// `MIN_MIN_TEXT_EM..=MAX_MIN_TEXT_EM` and rounded up to 0.01em so output
/// is stable.
fn min_text_em<'a>(
    label: &str,
    header_metrics: &TextMetrics,
    values: impl Iterator<Item = &'a str>,
) -> f64 {
    let body_metrics = TextMetrics::default();
    let widest = values
        .map(|v| measure_text(v, &body_metrics))
        .fold(measure_text(label, header_metrics), f64::max);
    let em = widest / TEXT_WIDTH_CALIBRATION_PX * TEXT_WIDTH_SLACK;
    (em.clamp(MIN_MIN_TEXT_EM, MAX_MIN_TEXT_EM) * 100.0).ceil() / 100.0
}

/// Header label metrics at the calibration size, from the theme's CSS
/// `text-transform` and `letter-spacing` (`normal`, `<n>em`, or `<n>px`
/// with a px table font size; any other form adds no spacing).
fn header_metrics(theme: &Theme) -> TextMetrics {
    let text_transform = match theme.table_header_text_transform.trim() {
        "uppercase" => TextTransform::Uppercase,
        "lowercase" => TextTransform::Lowercase,
        _ => TextTransform::None,
    };
    let px = |v: &str| v.trim().strip_suffix("px").and_then(|n| n.trim().parse::<f64>().ok());
    let spacing = theme.table_header_letter_spacing.trim();
    let spacing_em = if let Some(n) = spacing.strip_suffix("em").and_then(|n| n.trim().parse::<f64>().ok()) {
        n
    } else if let (Some(n), Some(font)) = (px(spacing), px(&theme.table_font_size)) {
        n / font
    } else {
        0.0
    };
    TextMetrics {
        font_size_px: TEXT_WIDTH_CALIBRATION_PX,
        letter_spacing_px: spacing_em * TEXT_WIDTH_CALIBRATION_PX,
        text_transform,
        monospace: false,
    }
}

/// Left and right components of a CSS `padding` shorthand (1–4 values).
fn horizontal_padding(shorthand: &str) -> (String, String) {
    let parts: Vec<&str> = shorthand.split_whitespace().collect();
    match parts.as_slice() {
        [] => ("0px".into(), "0px".into()),
        [all] => ((*all).into(), (*all).into()),
        [_, x] | [_, x, _] => ((*x).into(), (*x).into()),
        [_, right, _, left, ..] => ((*left).into(), (*right).into()),
    }
}

fn style(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

/// Opaque colour of the surface the table sits on, drawn under the sticky
/// header, the frozen first column and the fade, so a theme that makes rows
/// or the header transparent never lets scrolled cells show through.
fn surface_bg(theme: &Theme) -> String {
    format!("var(--chartml-table-surface, {})", theme.table_surface)
}

/// Grid placement of a header or body cell. Each row is its own grid with
/// the same column tracks, so header and body always line up. The last
/// column also spans the fade's track (see `fade_layers`).
struct CellPlacement {
    /// 1-based grid column.
    column: usize,
    last: bool,
    /// First column: frozen at the left edge while the rest scroll.
    frozen: bool,
    numeric: bool,
}

impl CellPlacement {
    fn grid_column(&self) -> String {
        if self.last {
            format!("{} / span 2", self.column)
        } else {
            self.column.to_string()
        }
    }
}

/// Styles common to header and body cells.
fn cell_style(
    place: &CellPlacement,
    padding: &str,
    border_bottom: &str,
    fade_width: &str,
) -> HashMap<String, String> {
    let mut map = style(&[
        ("grid-column", &place.grid_column()),
        ("grid-row", "1"),
        ("min-width", "0"),
        ("box-sizing", "border-box"),
        ("padding", padding),
        ("text-align", if place.numeric { "right" } else { "left" }),
        ("border-bottom", border_bottom),
    ]);
    if place.last {
        // The last cell reaches under the fade's strip so its border runs to
        // the row's end; its content stays clear of the strip.
        let (_, right) = horizontal_padding(padding);
        map.insert("padding-right".into(), format!("calc({right} + {fade_width})"));
    }
    if place.frozen {
        // Sticky above `frozen_base`. `inherit` takes the row's current
        // background (row, alternate row, a consumer's hover), drawn over
        // the opaque base.
        map.extend(style(&[
            ("position", "sticky"),
            ("left", "0"),
            ("z-index", "2"),
            ("background", "inherit"),
        ]));
    }
    map
}

/// Opaque layer under the frozen first cell, in the same grid area.
fn frozen_base(place: &CellPlacement, theme: &Theme) -> ChartElement {
    ChartElement::Span {
        class: "chartml-table-frozen-base".to_string(),
        style: style(&[
            ("grid-column", &place.grid_column()),
            ("grid-row", "1"),
            ("position", "sticky"),
            ("left", "0"),
            ("z-index", "1"),
            ("background", &surface_bg(theme)),
        ]),
        content: String::new(),
    }
}

/// Build a header cell. The column is sized to fit the label; should the
/// font run wider than estimated, the label wraps (as before) rather than
/// being cut off.
fn header_cell(label: &str, place: &CellPlacement, theme: &Theme, fade_width: &str) -> ChartElement {
    let header_text_var = format!("var(--chartml-table-header-text, {})", theme.table_header_text);
    let weight_var = format!(
        "var(--chartml-table-header-font-weight, {})",
        theme.table_header_font_weight,
    );
    let spacing_var = format!(
        "var(--chartml-table-header-letter-spacing, {})",
        theme.table_header_letter_spacing,
    );
    let transform_var = format!(
        "var(--chartml-table-header-text-transform, {})",
        theme.table_header_text_transform,
    );
    let mut cell_style = cell_style(place, &theme.table_cell_padding, &header_border(theme), fade_width);
    cell_style.extend(style(&[
        ("font-weight", &weight_var),
        ("letter-spacing", &spacing_var),
        ("text-transform", &transform_var),
        ("color", &header_text_var),
        ("overflow-wrap", "anywhere"),
    ]));
    ChartElement::Span {
        class: "chartml-table-header-cell".to_string(),
        style: cell_style,
        content: label.to_string(),
    }
}

/// Build a body cell. Every value stays readable: the column is sized to
/// fit it on one line, and a value wider than the column (beyond
/// `MAX_MIN_TEXT_EM`, past the frozen column's cap, or when the font runs
/// wider than estimated) wraps onto more lines instead of being cut off.
fn body_cell(content: &str, place: &CellPlacement, theme: &Theme, fade_width: &str) -> ChartElement {
    let text_var = format!("var(--chartml-table-text, {})", theme.table_text);
    let mut cell_style = cell_style(place, &theme.table_cell_padding, &body_border(theme), fade_width);
    cell_style.extend(style(&[
        ("color", &text_var),
        ("overflow-wrap", "anywhere"),
    ]));
    ChartElement::Span {
        class: "chartml-table-cell".to_string(),
        style: cell_style,
        content: content.to_string(),
    }
}

/// The right-edge fade of a header or body row: two layers in the row's
/// last track, held at the right edge of the visible area (sticky) while
/// columns are hidden to the right. The lower layer is the opaque surface
/// colour (`surface_bg`), the upper one the row's current background
/// (`inherit`, so a consumer's hover shows), both masked from transparent
/// to solid. They stop 1px short of the row's bottom so the cells' border
/// lines are never faded.
///
/// The track is `fade_width` wide: `FADE_WIDTH_PX` while the columns at
/// their minimum are wider than the scroll area (the table scrolls), 0
/// otherwise (nothing overflows, nothing is drawn and no width is taken
/// from the columns). Scrolled to the right end, the fade sits over its own
/// strip, which holds only the last cell's extra padding, so it covers no
/// content.
fn fade_layers(column: usize, theme: &Theme) -> [ChartElement; 2] {
    let column = column.to_string();
    let layer = |class: &str, z: &str, background: &str| ChartElement::Span {
        class: class.to_string(),
        style: style(&[
            ("grid-column", &column),
            ("grid-row", "1"),
            ("position", "sticky"),
            ("right", "0"),
            ("z-index", z),
            ("background", background),
            ("mask-image", "linear-gradient(to right, transparent, #000)"),
            ("pointer-events", "none"),
            // Stop above the cells' 1px bottom border so row and header
            // lines run unfaded to the end of the row.
            ("margin-bottom", "1px"),
        ]),
        content: String::new(),
    };
    [
        layer("chartml-table-fade-base", "1", &surface_bg(theme)),
        layer("chartml-table-fade", "2", "inherit"),
    ]
}

fn header_border(theme: &Theme) -> String {
    let header_border_color = if theme.table_header_border.is_empty() {
        &theme.table_border
    } else {
        &theme.table_header_border
    };
    format!(
        "1px solid var(--chartml-table-header-border, var(--chartml-table-border, {}))",
        header_border_color,
    )
}

fn body_border(theme: &Theme) -> String {
    format!("1px solid var(--chartml-table-border, {})", theme.table_border)
}

impl ChartRenderer for TableRenderer {
    fn render(&self, data: &DataTable, config: &ChartConfig) -> Result<ChartElement, ChartError> {
        let viz = &config.visualize;
        let theme = &config.theme;

        let columns = resolve_columns(viz, data);
        if columns.is_empty() {
            return Err(ChartError::DataError(
                "Table has no columns to render".into(),
            ));
        }
        let numeric: Vec<bool> = columns
            .iter()
            .map(|c| is_numeric_column(data, &c.field))
            .collect();

        // Pagination: style.pageSize → how many rows per page, page 0.
        // Clamp to >= 1 so a malformed `pageSize: 0` cannot divide-by-zero
        // in the pager footer's `div_ceil`.
        let page_size = viz
            .style
            .as_ref()
            .and_then(|s| s.page_size)
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .max(1);
        let total_rows = data.num_rows();
        let end = total_rows.min(page_size);

        let cells: Vec<Vec<String>> = (0..end)
            .map(|row| columns.iter().map(|c| format_cell(data, row, c)).collect())
            .collect();

        // ── Column tracks ──
        // Each column is at least as wide as its header label or widest
        // value on the page (plus padding) and otherwise takes an equal
        // share of the width. When the columns don't all fit, the frozen
        // first column's minimum gives way first, down to `FROZEN_MAX_SHARE`
        // of the scroll area, so other columns always have room to scroll
        // past it (past the cap its values wrap). Header and body rows are
        // at least the sum of the minimums plus the fade, so in a container
        // narrower than that the columns hold their minimum and the table
        // scrolls sideways.
        // Everything is relative to the table's own container and font,
        // never the viewport: `100cqw` is the scroll area's visible width
        // (it is an inline-size container), so it resolves the same in
        // every element below it.
        let (pad_left, pad_right) = horizontal_padding(&theme.table_cell_padding);
        let header_metrics = header_metrics(theme);
        let mut column_mins: Vec<String> = columns
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let em = min_text_em(&c.label, &header_metrics, cells.iter().map(|r| r[i].as_str()));
                format!("calc({em:.2}em + {pad_left} + {pad_right})")
            })
            .collect();
        if columns.len() > 1 {
            // The frozen column takes whatever width the other columns leave
            // it, up to its own minimum, and never less than
            // `FROZEN_MAX_SHARE`: it gives way before the table scrolls.
            let others = column_mins[1..].join(" + ");
            column_mins[0] = format!(
                "min({}, max({FROZEN_MAX_SHARE}, calc(100cqw - ({others}))))",
                column_mins[0]
            );
        }
        // The fade: `FADE_WIDTH_PX` once the columns at their minimum are
        // wider than the scroll area (the step completes within 0.04px),
        // otherwise 0. Rows are at least the columns plus the fade, so when
        // the table scrolls its right end shows the fade's own empty strip.
        let columns_min = format!("calc({})", column_mins.join(" + "));
        let fade_width = format!(
            "clamp(0px, calc(({columns_min} - 100cqw) * 1000), {FADE_WIDTH_PX}px)",
        );
        let row_min = format!("calc({columns_min} + {fade_width})");
        let tracks: Vec<String> = column_mins
            .iter()
            .map(|min| format!("minmax({min}, 1fr)"))
            .chain(std::iter::once(fade_width.clone()))
            .collect();
        let template = tracks.join(" ");
        let fade_column = columns.len() + 1;
        let place = |i: usize| CellPlacement {
            column: i + 1,
            last: i + 1 == columns.len(),
            frozen: i == 0,
            numeric: numeric[i],
        };

        // The table font: the size the `em` minimums resolve against and the
        // size the npm stylesheet gives cell text, from the same variable.
        let font_size_var = format!("var(--chartml-table-font-size, {})", theme.table_font_size);
        let header_bg_var = format!("var(--chartml-table-header-bg, {})", theme.table_header_bg);
        let row_bg_var = format!("var(--chartml-table-row-bg, {})", theme.table_row_bg);
        let row_bg_alt_var = format!("var(--chartml-table-row-bg-alt, {})", theme.table_row_bg_alt);

        // ── Header row (sticky to the top of the scroll area, above the
        // body's frozen cells; its colour is layered over the opaque
        // surface so rows scrolling under it never show through) ──
        let header_bg = format!(
            "linear-gradient({header_bg_var}, {header_bg_var}), {}",
            surface_bg(theme)
        );
        let mut header_children = vec![frozen_base(&place(0), theme)];
        header_children.extend(
            columns
                .iter()
                .enumerate()
                .map(|(i, c)| header_cell(&c.label, &place(i), theme, &fade_width)),
        );
        header_children.extend(fade_layers(fade_column, theme));
        let header_row = ChartElement::Div {
            class: "chartml-table-header".to_string(),
            style: style(&[
                ("display", "grid"),
                ("grid-template-columns", &template),
                ("min-width", &row_min),
                ("position", "sticky"),
                ("top", "0"),
                ("z-index", "3"),
                ("background", &header_bg),
                ("font-size", &font_size_var),
                ("font-family", &theme.label_font_family),
            ]),
            children: header_children,
        };

        // ── Body rows ──
        let mut body_children: Vec<ChartElement> = Vec::with_capacity(end);
        for (row_idx, row_cells) in cells.iter().enumerate() {
            let is_alt = row_idx % 2 == 1;
            let bg = if is_alt { &row_bg_alt_var } else { &row_bg_var };
            let class = if is_alt {
                "chartml-table-row chartml-table-row-alt"
            } else {
                "chartml-table-row"
            };
            let mut children = vec![frozen_base(&place(0), theme)];
            children.extend(
                row_cells
                    .iter()
                    .enumerate()
                    .map(|(i, text)| body_cell(text, &place(i), theme, &fade_width)),
            );
            children.extend(fade_layers(fade_column, theme));
            body_children.push(ChartElement::Div {
                class: class.to_string(),
                style: style(&[
                    ("display", "grid"),
                    ("grid-template-columns", &template),
                    ("background", bg),
                    ("font-size", &font_size_var),
                    ("font-family", &theme.label_font_family),
                ]),
                children,
            });
        }
        let body = ChartElement::Div {
            class: "chartml-table-body".to_string(),
            style: style(&[
                ("display", "flex"),
                ("flex-direction", "column"),
                // `em` in `row_min` resolves against this element's font,
                // which must be the table font as on the header and rows.
                ("min-width", &row_min),
                ("font-size", &font_size_var),
            ]),
            children: body_children,
        };

        // ── Scroll area: header + body scroll together on both axes, so the
        // header stays aligned with its columns when scrolled sideways and
        // stays on top (sticky) when the rows scroll vertically. `isolation`
        // keeps the sticky layers' z-indexes inside the table. ──
        let scroll = ChartElement::Div {
            class: "chartml-table-scroll".to_string(),
            style: style(&[
                ("flex", "1 1 0"),
                ("min-height", "0"),
                ("overflow", "auto"),
                ("isolation", "isolate"),
                ("container-type", "inline-size"),
            ]),
            children: vec![header_row, body],
        };

        // ── Pager footer (shown when there are more rows than one page),
        // outside the scroll area so it is always visible. ──
        let pager_text_var = format!("var(--chartml-text-secondary, {})", theme.text_secondary);
        let pager_border_var = format!("1px solid var(--chartml-table-border, {})", theme.table_border);
        let mut root_children: Vec<ChartElement> = vec![scroll];
        if total_rows > page_size {
            let page_count = total_rows.div_ceil(page_size);
            let info = ChartElement::Span {
                class: "chartml-table-pager-info".to_string(),
                style: style(&[
                    ("color", &pager_text_var),
                    ("font-size", &theme.table_font_size),
                    ("font-family", &theme.label_font_family),
                ]),
                content: format!(
                    "Showing {}–{} of {} · Page 1 of {}",
                    1, end, total_rows, page_count
                ),
            };
            root_children.push(ChartElement::Div {
                class: "chartml-table-pager".to_string(),
                style: style(&[
                    ("display", "flex"),
                    ("justify-content", "flex-end"),
                    ("padding", &theme.table_cell_padding),
                    ("border-top", &pager_border_var),
                    ("background", &header_bg_var),
                ]),
                children: vec![info],
            });
        }

        let root_text_var = format!("var(--chartml-table-text, {})", theme.table_text);
        let root_border_var = format!("1px solid var(--chartml-table-border, {})", theme.table_border);
        let radius_var = format!(
            "var(--chartml-table-border-radius, {})",
            theme.table_border_radius,
        );
        Ok(ChartElement::Div {
            class: "chartml-table".to_string(),
            style: style(&[
                ("display", "flex"),
                ("flex-direction", "column"),
                ("width", "100%"),
                ("height", "100%"),
                ("background", &row_bg_var),
                ("color", &root_text_var),
                ("border", &root_border_var),
                ("border-radius", &radius_var),
                ("overflow", "hidden"),
                ("box-sizing", "border-box"),
            ]),
            children: root_children,
        })
    }

    fn default_dimensions(&self, _spec: &VisualizeSpec) -> Option<Dimensions> {
        Some(Dimensions::new(DEFAULT_HEIGHT))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use chartml_core::data::Row;
    use chartml_core::element::count_elements;
    use chartml_core::spec::VisualizeSpec;
    use serde_json::json;

    fn data() -> DataTable {
        let rows: Vec<Row> = vec![
            [
                ("month".to_string(), json!("Jan")),
                ("revenue".to_string(), json!(1200.0)),
                ("cost".to_string(), json!(400.0)),
            ]
            .into_iter()
            .collect(),
            [
                ("month".to_string(), json!("Feb")),
                ("revenue".to_string(), json!(1500.5)),
                ("cost".to_string(), json!(450.0)),
            ]
            .into_iter()
            .collect(),
            [
                ("month".to_string(), json!("Mar")),
                ("revenue".to_string(), json!(1800.0)),
                ("cost".to_string(), json!(500.0)),
            ]
            .into_iter()
            .collect(),
        ];
        DataTable::from_rows(&rows).unwrap()
    }

    fn cfg(viz_yaml: &str) -> ChartConfig {
        let visualize: VisualizeSpec = serde_yaml::from_str(viz_yaml).unwrap();
        ChartConfig {
            visualize,
            title: None,
            width: 600.0,
            height: 400.0,
            colors: vec![],
            theme: Theme::default(),
        }
    }

    fn find_contents(el: &ChartElement, out: &mut Vec<String>) {
        match el {
            ChartElement::Span { content, .. } => out.push(content.clone()),
            ChartElement::Div { children, .. } => {
                for c in children {
                    find_contents(c, out);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn renders_drop_in_bar_spec() {
        // Same spec a bar chart would use — just type: table.
        let config = cfg(
            r#"
type: table
columns: month
rows:
  - field: revenue
    label: Revenue
    format: "$,.0f"
  - field: cost
    label: Cost
    format: "$,.0f"
"#,
        );
        let element = TableRenderer::new().render(&data(), &config).unwrap();
        let mut contents = Vec::new();
        find_contents(&element, &mut contents);
        // Header labels present
        assert!(contents.iter().any(|c| c == "month"));
        assert!(contents.iter().any(|c| c == "Revenue"));
        assert!(contents.iter().any(|c| c == "Cost"));
        // Formatted revenue cell present
        assert!(
            contents.iter().any(|c| c == "$1,200"),
            "expected formatted revenue cell, got: {contents:?}"
        );
        // Category cell present
        assert!(contents.iter().any(|c| c == "Jan"));
    }

    #[test]
    fn falls_back_to_all_fields_when_no_spec() {
        let config = cfg("type: table");
        let element = TableRenderer::new().render(&data(), &config).unwrap();
        let mut contents = Vec::new();
        find_contents(&element, &mut contents);
        // All three schema fields appear as headers (DataTable sorts schema alphabetically).
        for name in ["cost", "month", "revenue"] {
            assert!(
                contents.iter().any(|c| c == name),
                "missing header {name}: {contents:?}"
            );
        }
    }

    #[test]
    fn page_size_zero_is_clamped_to_one() {
        // Malformed `pageSize: 0` must not divide-by-zero in the pager.
        let config = cfg(
            r#"
type: table
columns: month
rows:
  - field: revenue
style:
  pageSize: 0
"#,
        );
        let element = TableRenderer::new().render(&data(), &config).unwrap();
        let mut contents = Vec::new();
        find_contents(&element, &mut contents);
        // Page size clamped to 1 → only Jan visible, pager shows page 1 of 3.
        assert!(contents.iter().any(|c| c == "Jan"));
        assert!(!contents.iter().any(|c| c == "Feb"));
        assert!(contents.iter().any(|c| c.contains("Page 1 of 3")));
    }

    #[test]
    fn pagination_limits_rows() {
        let config = cfg(
            r#"
type: table
columns: month
rows:
  - field: revenue
style:
  pageSize: 2
"#,
        );
        let element = TableRenderer::new().render(&data(), &config).unwrap();
        let mut contents = Vec::new();
        find_contents(&element, &mut contents);
        // Only first two month values present
        assert!(contents.iter().any(|c| c == "Jan"));
        assert!(contents.iter().any(|c| c == "Feb"));
        assert!(
            !contents.iter().any(|c| c == "Mar"),
            "Mar should be paginated out: {contents:?}"
        );
        // Pager info span should be present
        assert!(
            contents.iter().any(|c| c.contains("Page 1 of")),
            "missing pager info: {contents:?}"
        );
    }

    #[test]
    fn empty_columns_errors() {
        let config = cfg("type: table");
        let empty = DataTable::from_rows(&Vec::<Row>::new()).unwrap();
        // Empty DataTable → no fields → no columns → error.
        assert!(TableRenderer::new().render(&empty, &config).is_err());
    }

    #[test]
    fn null_cell_renders_em_dash() {
        // A JSON null in a numeric column must render "—" (U+2014) rather than
        // an empty string, so the table always has visible, informative content.
        let rows: Vec<Row> = vec![
            [
                ("month".to_string(), json!("Jan")),
                ("revenue".to_string(), serde_json::Value::Null),
            ]
            .into_iter()
            .collect(),
        ];
        let data = DataTable::from_rows(&rows).unwrap();
        let config = cfg(
            r#"
type: table
columns: month
rows:
  - field: revenue
    label: Revenue
    format: "$,.0f"
"#,
        );
        let element = TableRenderer::new().render(&data, &config).unwrap();
        let mut contents = Vec::new();
        find_contents(&element, &mut contents);
        assert!(
            contents.iter().any(|c| c == "\u{2014}"),
            "null cell must render em dash, got: {contents:?}"
        );
    }

    #[test]
    fn null_string_cell_renders_em_dash() {
        // A JSON null in a string column must also render "—".
        let rows: Vec<Row> = vec![
            [
                ("month".to_string(), serde_json::Value::Null),
                ("revenue".to_string(), json!(1200.0)),
            ]
            .into_iter()
            .collect(),
        ];
        let data = DataTable::from_rows(&rows).unwrap();
        let config = cfg("type: table");
        let element = TableRenderer::new().render(&data, &config).unwrap();
        let mut contents = Vec::new();
        find_contents(&element, &mut contents);
        assert!(
            contents.iter().any(|c| c == "\u{2014}"),
            "null string cell must render em dash, got: {contents:?}"
        );
    }

    #[test]
    fn root_has_table_class() {
        let config = cfg("type: table");
        let element = TableRenderer::new().render(&data(), &config).unwrap();
        match &element {
            ChartElement::Div { class, .. } => assert_eq!(class, "chartml-table"),
            _ => panic!("root must be a Div"),
        }
    }

    #[test]
    fn theme_applied_to_cells() {
        let mut config = cfg("type: table");
        config.theme = Theme::dark();
        let element = TableRenderer::new().render(&data(), &config).unwrap();
        // Header bg should be the dark-theme header color somewhere in the tree.
        let header_bg = Theme::dark().table_header_bg;
        let mut found = false;
        fn walk(el: &ChartElement, needle: &str, out: &mut bool) {
            match el {
                ChartElement::Div { style, children, .. } => {
                    if style.values().any(|v| v.contains(needle)) {
                        *out = true;
                    }
                    for c in children {
                        walk(c, needle, out);
                    }
                }
                ChartElement::Span { style, .. } => {
                    if style.values().any(|v| v.contains(needle)) {
                        *out = true;
                    }
                }
                _ => {}
            }
        }
        walk(&element, &header_bg, &mut found);
        assert!(found, "dark theme header bg {header_bg} not found in tree");
    }

    #[test]
    fn default_dimensions() {
        let dims = TableRenderer::new().default_dimensions(
            &serde_yaml::from_str::<VisualizeSpec>("type: table").unwrap(),
        );
        assert_eq!(dims.unwrap().height, 400.0);
    }

    /// Collect every Div/Span whose class list contains `class`, in document order.
    fn find_by_class<'a>(el: &'a ChartElement, class: &str, out: &mut Vec<&'a ChartElement>) {
        let (own, children): (&str, Option<&Vec<ChartElement>>) = match el {
            ChartElement::Div { class, children, .. } => (class, Some(children)),
            ChartElement::Span { class, .. } => (class, None),
            _ => ("", None),
        };
        if own.split_whitespace().any(|c| c == class) {
            out.push(el);
        }
        if let Some(children) = children {
            for c in children {
                find_by_class(c, class, out);
            }
        }
    }

    fn by_class<'a>(el: &'a ChartElement, class: &str) -> Vec<&'a ChartElement> {
        let mut out = Vec::new();
        find_by_class(el, class, &mut out);
        out
    }

    fn style_of(el: &ChartElement) -> &HashMap<String, String> {
        match el {
            ChartElement::Div { style, .. } | ChartElement::Span { style, .. } => style,
            _ => panic!("not a Div/Span"),
        }
    }

    fn children_of(el: &ChartElement) -> &Vec<ChartElement> {
        match el {
            ChartElement::Div { children, .. } => children,
            _ => panic!("not a Div"),
        }
    }

    fn class_of(el: &ChartElement) -> &str {
        match el {
            ChartElement::Div { class, .. } | ChartElement::Span { class, .. } => class,
            _ => panic!("not a Div/Span"),
        }
    }

    fn get<'a>(el: &'a ChartElement, key: &str) -> Option<&'a str> {
        style_of(el).get(key).map(|s| s.as_str())
    }

    /// Twelve columns: one text category, a text measure, nine numeric
    /// measures and a trailing text measure — the narrow-container case.
    fn wide_data() -> DataTable {
        let rows: Vec<Row> = (0..4)
            .map(|i| {
                let mut row: Row = [
                    ("region".to_string(), json!(format!("Region {i}"))),
                    ("manager".to_string(), json!(format!("Manager {i}"))),
                    ("tier".to_string(), json!("Gold")),
                ]
                .into_iter()
                .collect();
                for m in ["m1", "m2", "m3", "m4", "m5", "m6", "m7", "m8", "m9"] {
                    row.insert(m.to_string(), json!(f64::from(i) * 10.5));
                }
                row
            })
            .collect();
        DataTable::from_rows(&rows).unwrap()
    }

    const WIDE_SPEC: &str = r#"
type: table
columns: region
rows: [manager, m1, m2, m3, m4, m5, m6, m7, m8, m9, tier]
style:
  pageSize: 3
"#;

    fn wide() -> ChartElement {
        TableRenderer::new().render(&wide_data(), &cfg(WIDE_SPEC)).unwrap()
    }

    /// The table's own cells in a header or body row (no frozen base or fade layers).
    fn real_cells(row: &ChartElement) -> Vec<&ChartElement> {
        children_of(row)
            .iter()
            .filter(|c| matches!(class_of(c), "chartml-table-cell" | "chartml-table-header-cell"))
            .collect()
    }

    fn header_and_rows(el: &ChartElement) -> Vec<&ChartElement> {
        let mut rows = by_class(el, "chartml-table-header");
        rows.extend(by_class(el, "chartml-table-row"));
        rows
    }

    fn em_of(track: &str) -> f64 {
        // "minmax(calc(6.42em + 12px + 12px), 1fr)" or
        // "minmax(min(calc(6.42em + 12px + 12px), 50cqw), 1fr)" -> 6.42
        track
            .trim_start_matches("minmax(")
            .trim_start_matches("min(")
            .trim_start_matches("calc(")
            .split("em")
            .next()
            .unwrap()
            .parse()
            .unwrap()
    }

    fn tracks(row: &ChartElement) -> Vec<String> {
        // Split the template at top-level spaces (not inside parentheses).
        let template = get(row, "grid-template-columns").unwrap();
        let (mut out, mut depth, mut cur) = (Vec::new(), 0, String::new());
        for ch in template.chars() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                ' ' if depth == 0 => {
                    out.push(std::mem::take(&mut cur));
                    continue;
                }
                _ => {}
            }
            cur.push(ch);
        }
        out.push(cur);
        out
    }

    /// The column minimum inside a `minmax(<min>, 1fr)` track.
    fn track_min(track: &str) -> &str {
        track.strip_prefix("minmax(").unwrap().strip_suffix(", 1fr)").unwrap()
    }

    #[test]
    fn columns_share_width_above_a_content_minimum() {
        // Point 1: every header and body row is a grid with the same tracks:
        // one per column, each at least the column's minimum (its widest
        // label/value plus padding) and otherwise an equal share (1fr), then
        // the fade's track.
        let el = wide();
        let rows = header_and_rows(&el);
        assert_eq!(rows.len(), 4);
        let first = get(rows[0], "grid-template-columns").unwrap();
        for row in &rows {
            assert_eq!(get(row, "display"), Some("grid"));
            assert_eq!(get(row, "grid-template-columns"), Some(first), "every row shares the tracks");
        }
        let t = tracks(rows[0]);
        assert_eq!(t.len(), 13);
        // The frozen first column's minimum gives way only when the other
        // columns and the fade don't fit beside it, and never below half the
        // scroll area, so the other columns can always scroll into view.
        let others: Vec<&str> = t[1..12].iter().map(|s| track_min(s)).collect();
        let first = t[0].strip_prefix("minmax(min(").unwrap();
        assert!(first.starts_with("calc(") && first.contains("em + 12px + 12px), "), "{}", t[0]);
        assert!(
            t[0].ends_with(&format!(", max(50cqw, calc(100cqw - ({})))), 1fr)", others.join(" + "))),
            "{}",
            t[0]
        );
        for track in &t[1..12] {
            assert!(track.starts_with("minmax(calc(") && track.ends_with("em + 12px + 12px), 1fr)"), "{track}");
        }
        // The minimum follows the content: "Manager 0" is wider than "m1" / "10.5".
        assert!(em_of(&t[1]) > em_of(&t[2]), "{t:?}");
        // The fade's track: 32px once the column minimums exceed the scroll
        // area, else 0; rows are at least the columns plus the fade.
        let mins: Vec<&str> = t[..12].iter().map(|s| track_min(s)).collect();
        let columns_min = format!("calc({})", mins.join(" + "));
        let fade = format!("clamp(0px, calc(({columns_min} - 100cqw) * 1000), 32px)");
        assert_eq!(t[12], fade);
        let row_min = format!("calc({columns_min} + {fade})");
        assert_eq!(get(rows[0], "min-width"), Some(row_min.as_str()));
        // Cells fill their track.
        for row in &rows {
            for c in real_cells(row) {
                assert_eq!(get(c, "min-width"), Some("0"));
                assert_eq!(get(c, "box-sizing"), Some("border-box"));
                assert_eq!(get(c, "grid-row"), Some("1"));
            }
        }
        // No value or label is ever cut off: each is measured into its
        // column's minimum and, beyond the caps, wraps.
        let mut all = by_class(&el, "chartml-table-cell");
        all.extend(by_class(&el, "chartml-table-header-cell"));
        for c in all {
            assert_eq!(get(c, "overflow"), None);
            assert_eq!(get(c, "white-space"), None);
            assert_eq!(get(c, "text-overflow"), None);
            assert_eq!(get(c, "overflow-wrap"), Some("anywhere"));
        }
    }

    #[test]
    fn min_text_width_follows_label_and_values_within_bounds() {
        let plain = TextMetrics::default();
        // Short content: the lower bound.
        assert_eq!(min_text_em("n", &plain, ["1", "2"].into_iter()), MIN_MIN_TEXT_EM);
        // A long value: its estimated width (12px calibration) with slack, in em.
        let v = "Sub-Saharan Africa";
        let expected = measure_text(v, &plain) / 12.0 * TEXT_WIDTH_SLACK;
        let got = min_text_em("region", &plain, [v, "Oceania"].into_iter());
        assert!((got - expected).abs() < 0.011, "{got} vs {expected}");
        // A label wider than every value sets the width.
        let label = "Customer lifetime value";
        assert!(min_text_em(label, &plain, ["1"].into_iter()) > min_text_em("x", &plain, ["1"].into_iter()));
        // Very long values are capped (beyond it they wrap).
        let long = "x".repeat(200);
        assert_eq!(min_text_em("n", &plain, [long.as_str()].into_iter()), MAX_MIN_TEXT_EM);
        // Uppercase, letter-spaced headers measure wider.
        let mut theme = Theme::default();
        let normal = header_metrics(&theme);
        theme.table_header_text_transform = "uppercase".into();
        theme.table_header_letter_spacing = "0.08em".into();
        let kyomi = header_metrics(&theme);
        assert!(matches!(kyomi.text_transform, TextTransform::Uppercase));
        assert!((kyomi.letter_spacing_px - 0.96).abs() < 1e-9);
        assert!(min_text_em("Customers", &kyomi, [].into_iter()) > min_text_em("Customers", &normal, [].into_iter()));
        theme.table_header_letter_spacing = "1.3px".into();
        theme.table_font_size = "13px".into();
        assert!((header_metrics(&theme).letter_spacing_px - 1.2).abs() < 1e-9);
    }

    #[test]
    fn padding_shorthand_left_and_right() {
        assert_eq!(horizontal_padding("8px"), ("8px".into(), "8px".into()));
        assert_eq!(horizontal_padding("8px 12px"), ("12px".into(), "12px".into()));
        assert_eq!(horizontal_padding("8px 12px 4px"), ("12px".into(), "12px".into()));
        assert_eq!(horizontal_padding("1px 2px 3px 4px"), ("4px".into(), "2px".into()));
        assert_eq!(horizontal_padding(""), ("0px".into(), "0px".into()));
    }

    #[test]
    fn header_and_body_scroll_together_and_span_full_width() {
        // Point 1: header and body sit in one scroll container, so they scroll
        // sideways together, and both are at least as wide as every column
        // at its minimum plus the fade, so every row's background and border
        // span the whole scrolled width. `em` resolves against the table font
        // on both.
        let el = wide();
        let scroll = by_class(&el, "chartml-table-scroll")[0];
        assert_eq!(get(scroll, "overflow"), Some("auto"));
        let kids: Vec<&str> = children_of(scroll).iter().map(class_of).collect();
        assert_eq!(kids, vec!["chartml-table-header", "chartml-table-body"]);
        let header = &children_of(scroll)[0];
        let body = &children_of(scroll)[1];
        let min = get(header, "min-width").unwrap();
        assert!(min.starts_with("calc(calc(min(calc(") && min.ends_with(" * 1000), 32px))"), "{min}");
        assert_eq!(get(body, "min-width"), Some(min));
        // `em` minimums resolve against the same font variable the npm
        // stylesheet gives cell text, on the header, body and every row.
        for el in [header, body].into_iter().chain(by_class(&el, "chartml-table-row")) {
            assert_eq!(get(el, "font-size"), Some("var(--chartml-table-font-size, 13px)"));
        }
        // Body rows stretch to the body's width (column flex, default stretch).
        assert_eq!(get(body, "display"), Some("flex"));
        assert_eq!(get(body, "flex-direction"), Some("column"));
        assert_eq!(get(body, "align-items"), None);
        // Header and body cells of a column sit in the same grid column.
        for row in by_class(&el, "chartml-table-row") {
            let h = real_cells(header);
            let b = real_cells(row);
            assert_eq!(h.len(), b.len());
            for (i, (hc, bc)) in h.iter().zip(b.iter()).enumerate() {
                for key in ["grid-column", "padding", "padding-right", "text-align", "position", "left"] {
                    assert_eq!(get(hc, key), get(bc, key), "column {i}: {key} differs");
                }
            }
        }
    }

    #[test]
    fn first_column_is_frozen_over_an_opaque_base() {
        // Point 2: in the header and every body row, the first cell is
        // frozen at the left and takes its row's current background
        // (`inherit`: row, alternate row, a consumer's hover). Under it, in
        // the same grid area, an opaque base in the chart background colour
        // keeps scrolled cells from showing through even when a theme makes
        // rows or the header transparent: the table surface colour
        // (`--chartml-table-surface`). No other column is frozen.
        let mut config = cfg(WIDE_SPEC);
        config.theme.table_header_bg = "transparent".into();
        config.theme.table_row_bg = "transparent".into();
        config.theme.table_row_bg_alt = "transparent".into();
        config.theme.table_surface = "#24201E".into();
        let el = TableRenderer::new().render(&wide_data(), &config).unwrap();
        for row in header_and_rows(&el) {
            let kids = children_of(row);
            let base = &kids[0];
            assert_eq!(class_of(base), "chartml-table-frozen-base");
            assert_eq!(get(base, "background"), Some("var(--chartml-table-surface, #24201E)"));
            let cells = real_cells(row);
            let first = cells[0];
            for el in [base, first] {
                assert_eq!(get(el, "grid-column"), Some("1"));
                assert_eq!(get(el, "grid-row"), Some("1"));
                assert_eq!(get(el, "position"), Some("sticky"));
                assert_eq!(get(el, "left"), Some("0"));
            }
            assert_eq!(get(base, "z-index"), Some("1"));
            assert_eq!(get(first, "z-index"), Some("2"));
            assert_eq!(get(first, "background"), Some("inherit"));
            for c in &cells[1..] {
                assert_eq!(get(c, "position"), None);
                assert_eq!(get(c, "background"), None);
            }
        }
        // The header is sticky above the body's frozen cells, and its own
        // colour is layered over the same opaque base, so rows scrolling
        // under it never show through (and its frozen cell inherits that).
        let header = by_class(&el, "chartml-table-header")[0];
        assert_eq!(get(header, "position"), Some("sticky"));
        assert_eq!(get(header, "top"), Some("0"));
        assert_eq!(get(header, "z-index"), Some("3"));
        assert_eq!(
            get(header, "background"),
            Some("linear-gradient(var(--chartml-table-header-bg, transparent), var(--chartml-table-header-bg, transparent)), var(--chartml-table-surface, #24201E)")
        );
        // Sticky layers stay inside the table's own stacking context.
        let scroll = by_class(&el, "chartml-table-scroll")[0];
        assert_eq!(get(scroll, "isolation"), Some("isolate"));
    }

    #[test]
    fn numeric_columns_right_aligned_from_data_type() {
        // Point 3: alignment comes from each column's data type: Float64
        // measures (formatted or not) are right-aligned, header included;
        // text columns, digit-only text and a field missing from the data
        // stay left.
        let el = wide();
        let expected = [
            "left", "left", "right", "right", "right", "right", "right", "right", "right",
            "right", "right", "left",
        ];
        for row in header_and_rows(&el) {
            let got: Vec<&str> = real_cells(row).iter().map(|c| get(c, "text-align").unwrap()).collect();
            assert_eq!(got, expected);
        }

        let rows: Vec<Row> = vec![[
            ("code".to_string(), json!("007")),
            ("amount".to_string(), json!(5)),
        ]
        .into_iter()
        .collect()];
        let data = DataTable::from_rows(&rows).unwrap();
        let config = cfg("type: table\ncolumns: code\nrows: [amount, missing]\n");
        let el = TableRenderer::new().render(&data, &config).unwrap();
        let header = by_class(&el, "chartml-table-header")[0];
        let got: Vec<&str> = real_cells(header).iter().map(|c| get(c, "text-align").unwrap()).collect();
        assert_eq!(got, vec!["left", "right", "left"]);
    }

    #[test]
    fn numeric_alignment_covers_arrow_integer_and_decimal_types() {
        // Point 3, typed sources (e.g. a warehouse query): Int64 and Decimal128
        // are numeric and right-aligned; Date32 and Utf8 are not.
        use arrow::array::{ArrayRef, Date32Array, Decimal128Array, Int64Array, StringArray};
        use arrow::datatypes::{DataType, Field, Schema};
        use arrow::record_batch::RecordBatch;
        use std::sync::Arc;
        let schema = Arc::new(Schema::new(vec![
            Field::new("name", DataType::Utf8, true),
            Field::new("count", DataType::Int64, true),
            Field::new("price", DataType::Decimal128(10, 2), true),
            Field::new("day", DataType::Date32, true),
        ]));
        let arrays: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(vec!["a"])),
            Arc::new(Int64Array::from(vec![3])),
            Arc::new(Decimal128Array::from(vec![1234]).with_precision_and_scale(10, 2).unwrap()),
            Arc::new(Date32Array::from(vec![19000])),
        ];
        let data = DataTable::from_record_batch(RecordBatch::try_new(schema, arrays).unwrap());
        let config = cfg("type: table\ncolumns: name\nrows: [count, price, day]\n");
        let el = TableRenderer::new().render(&data, &config).unwrap();
        for row in header_and_rows(&el) {
            let got: Vec<&str> = real_cells(row).iter().map(|c| get(c, "text-align").unwrap()).collect();
            assert_eq!(got, vec!["left", "right", "right", "left"]);
        }
    }

    #[test]
    fn right_edge_fade_only_while_columns_are_hidden() {
        // Point 4: every header and body row ends with two sticky layers in
        // the fade's track (an opaque base and the row's current colour,
        // both masked into a gradient). The track is 32px only while the
        // columns at their minimum are wider than the scroll area (`100cqw`,
        // an inline-size container), and 0 otherwise. The last cell reaches under
        // the track so its border runs to the row's end, with its padding
        // widened by the same amount so its text stays clear of the fade.
        let el = wide();
        let scroll = by_class(&el, "chartml-table-scroll")[0];
        assert_eq!(get(scroll, "container-type"), Some("inline-size"));
        for row in header_and_rows(&el) {
            let fade_track = tracks(row).pop().unwrap();
            let kids = children_of(row);
            let n = kids.len();
            let (base, fade) = (&kids[n - 2], &kids[n - 1]);
            assert_eq!(class_of(base), "chartml-table-fade-base");
            assert_eq!(class_of(fade), "chartml-table-fade");
            assert_eq!(get(base, "background"), Some(format!("var(--chartml-table-surface, {})", Theme::default().table_surface).as_str()));
            assert_eq!(get(fade, "background"), Some("inherit"));
            for (layer, z) in [(base, "1"), (fade, "2")] {
                assert_eq!(get(layer, "grid-column"), Some("13"));
                assert_eq!(get(layer, "position"), Some("sticky"));
                assert_eq!(get(layer, "right"), Some("0"));
                assert_eq!(get(layer, "z-index"), Some(z));
                assert_eq!(get(layer, "mask-image"), Some("linear-gradient(to right, transparent, #000)"));
                assert_eq!(get(layer, "pointer-events"), Some("none"));
                assert_eq!(get(layer, "border-bottom"), None, "no border of its own");
                assert_eq!(get(layer, "margin-bottom"), Some("1px"), "row lines stay visible under it");
            }
            let last = *real_cells(row).last().unwrap();
            assert_eq!(get(last, "grid-column"), Some("12 / span 2"));
            assert_eq!(get(last, "padding-right"), Some(format!("calc(12px + {fade_track})").as_str()));
        }
    }

    #[test]
    fn single_column_table_is_frozen_and_spans_the_fade_track() {
        let config = cfg("type: table\ncolumns: month\n");
        let el = TableRenderer::new().render(&data(), &config).unwrap();
        for row in header_and_rows(&el) {
            // A lone column is not capped: there is nothing beside it.
            let t = tracks(row);
            assert_eq!(t.len(), 2);
            assert!(t[0].starts_with("minmax(calc("), "{}", t[0]);
            let base = &children_of(row)[0];
            let cell = real_cells(row)[0];
            assert_eq!(get(base, "grid-column"), Some("1 / span 2"));
            assert_eq!(get(cell, "grid-column"), Some("1 / span 2"));
            assert_eq!(get(cell, "position"), Some("sticky"));
        }
    }

    #[test]
    fn sizes_from_container_not_viewport() {
        // Point 5: no viewport units anywhere; sizes are em of the table font,
        // px, percentages of the table's own box, and `cqw` of its scroll area.
        fn walk(el: &ChartElement) {
            if let ChartElement::Div { .. } | ChartElement::Span { .. } = el {
                for (k, v) in style_of(el) {
                    for tok in v.split(|c: char| !c.is_ascii_alphanumeric() && c != '.') {
                        let unit = tok.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
                        let numeric = tok.starts_with(|c: char| c.is_ascii_digit());
                        assert!(
                            !(numeric && ["vw", "vh", "vmin", "vmax", "dvw", "svw", "lvw", "dvh", "svh", "lvh"].contains(&unit)),
                            "viewport unit in {k}: {v}"
                        );
                    }
                }
            }
            if let ChartElement::Div { children, .. } = el {
                children.iter().for_each(walk);
            }
        }
        let el = wide();
        walk(&el);
        assert_eq!(get(&el, "width"), Some("100%"));
    }

    #[test]
    fn vertical_scroll_and_pager_unchanged() {
        // Point 6: the rows still scroll vertically inside a flex-filling
        // area under the header, and the pager footer stays outside it,
        // below, always visible.
        let el = wide();
        let kids: Vec<&str> = children_of(&el).iter().map(class_of).collect();
        assert_eq!(kids, vec!["chartml-table-scroll", "chartml-table-pager"]);
        let scroll = &children_of(&el)[0];
        assert_eq!(get(scroll, "flex"), Some("1 1 0"));
        assert_eq!(get(scroll, "min-height"), Some("0"));
        assert_eq!(get(scroll, "overflow"), Some("auto"));
        let body = by_class(&el, "chartml-table-body")[0];
        assert_eq!(get(body, "overflow-y"), None, "only the scroll area scrolls");
        let mut contents = Vec::new();
        find_contents(&el, &mut contents);
        assert!(contents.iter().any(|c| c == "Showing 1–3 of 4 · Page 1 of 2"));
    }

    #[test]
    fn existing_class_names_kept() {
        // Point 6: consumer CSS overrides keep applying.
        let el = wide();
        for class in [
            "chartml-table",
            "chartml-table-header",
            "chartml-table-header-cell",
            "chartml-table-body",
            "chartml-table-row",
            "chartml-table-row-alt",
            "chartml-table-cell",
            "chartml-table-pager",
            "chartml-table-pager-info",
        ] {
            assert!(!by_class(&el, class).is_empty(), "missing {class}");
        }
    }

    #[test]
    fn element_counts_reasonable() {
        let config = cfg(
            r#"
type: table
columns: month
rows:
  - field: revenue
"#,
        );
        let element = TableRenderer::new().render(&data(), &config).unwrap();
        let span_count =
            count_elements(&element, &|e| matches!(e, ChartElement::Span { .. }));
        // 2 header cells + (3 rows * 2 cells) = 8 spans minimum
        assert!(span_count >= 8, "got {span_count} spans");
    }
}
