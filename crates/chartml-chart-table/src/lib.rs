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

/// A column's minimum text width, in `em` of the table font, comes from its
/// header label and values (see `ColumnText`) and is at most this, so one
/// very long value (a URL, an ID) cannot make the table scroll in a wide
/// container; past it the value wraps.
const MAX_MIN_TEXT_EM: f64 = 18.0;

/// Text is measured at this font size (the estimator's calibration); dividing
/// by it gives `em`. The estimate is widened by `TEXT_WIDTH_SLACK` because real
/// fonts can run wider than the table.
const TEXT_WIDTH_CALIBRATION_PX: f64 = 12.0;
const TEXT_WIDTH_SLACK: f64 = 1.15;

/// While the table scrolls, the frozen first column takes at most this share
/// of the scroll area's width (or its widest word if wider, see
/// `MIN_SCROLL_WINDOW`), so the scrolling columns stay reachable beside it.
const FROZEN_MAX_SHARE: &str = "50cqw";

/// The least width left beside the frozen column (and the fade) through
/// which the other columns scroll, when the frozen column's widest word
/// would otherwise take more.
const MIN_SCROLL_WINDOW: &str = "3em";

/// Width of the right-edge fade that signals more columns off to the right
/// (see `fade_cell`).
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

/// Text widths of a column in `em`, from its header label (measured as the
/// header draws it, see `HeaderText`) and its values on the page, each
/// widened by `TEXT_WIDTH_SLACK` and rounded up to 0.01em so output is
/// stable.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ColumnText {
    /// The widest label or value on one line, at most `MAX_MIN_TEXT_EM`.
    line: f64,
    /// The widest single word (a run without spaces, e.g. a number): the
    /// narrowest the column can be without breaking a word, also at most
    /// `MAX_MIN_TEXT_EM` (a longer word breaks).
    word: f64,
}

fn column_text<'a>(label: &str, header: &HeaderText, values: impl Iterator<Item = &'a str>) -> ColumnText {
    let body_metrics = TextMetrics::default();
    let label_width = |t: &str| measure_text(t, &header.metrics) * header.weight_scale;
    let value_width = |t: &str| measure_text(t, &body_metrics);
    let mut line = label_width(label);
    let mut word = label.split_whitespace().map(label_width).fold(0.0, f64::max);
    for v in values {
        line = line.max(value_width(v));
        word = v.split_whitespace().map(value_width).fold(word, f64::max);
    }
    let em = |w: f64| (w / TEXT_WIDTH_CALIBRATION_PX * TEXT_WIDTH_SLACK * 100.0).ceil() / 100.0;
    ColumnText { line: em(line).min(MAX_MIN_TEXT_EM), word: em(word).min(MAX_MIN_TEXT_EM) }
}

/// How header labels are drawn, for measuring them: the theme's CSS
/// `text-transform` and `letter-spacing` (`normal`, `<n>em`, or `<n>px`
/// with a px table font size; any other form adds no spacing) as
/// `TextMetrics` at the calibration size, and `weight_scale` for the font
/// weight: the estimator is calibrated on regular-weight text, and bold
/// glyphs (`font-weight` 600 and up, `bold`, `bolder`) run about 10% wider.
struct HeaderText {
    metrics: TextMetrics,
    weight_scale: f64,
}

const BOLD_WIDTH_SCALE: f64 = 1.1;

fn header_text(theme: &Theme) -> HeaderText {
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
    let weight = theme.table_header_font_weight.trim();
    let bold = matches!(weight, "bold" | "bolder") || weight.parse::<f64>().is_ok_and(|w| w >= 600.0);
    HeaderText {
        metrics: TextMetrics {
            font_size_px: TEXT_WIDTH_CALIBRATION_PX,
            letter_spacing_px: spacing_em * TEXT_WIDTH_CALIBRATION_PX,
            text_transform,
            monospace: false,
        },
        weight_scale: if bold { BOLD_WIDTH_SCALE } else { 1.0 },
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

/// A header or row background that is always opaque: its own colour (which
/// a theme may make transparent) layered over the opaque colour of the
/// surface the table sits on. The frozen first column and the fade take
/// their row's background (`inherit`), so they are opaque too and scrolled
/// cells never show through them. A consumer rule that replaces a row's
/// background (e.g. a hover colour) carries through to both.
fn opaque_bg(colour: &str, theme: &Theme) -> String {
    format!(
        "linear-gradient({colour}, {colour}), var(--chartml-table-surface, {})",
        theme.table_surface
    )
}

/// Grid placement of a header or body cell. Each row is its own grid with
/// the same column tracks, so header and body always line up. The last
/// column also spans the fade's track (see `fade_cell`).
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
        // `inherit` takes the row's current, opaque background (row,
        // alternate row, a consumer's hover).
        map.extend(style(&[
            ("position", "sticky"),
            ("left", "0"),
            ("z-index", "1"),
            ("background", "inherit"),
        ]));
        if !place.last {
            map.extend(frozen_gutter(padding));
            // Keep the frozen cell's own text where it was.
            let (left, right) = horizontal_padding(padding);
            map.insert("padding-right".into(), format!("calc({right} + {left})"));
        }
    }
    map
}

/// The frozen cell also covers a gutter beside it, as wide as a cell's left
/// padding: solid for its first half, then fading out. A column scrolled
/// partly under the frozen one would otherwise show slivers of glyphs at the
/// frozen edge. At rest the gutter lies over the next cell's empty left
/// padding in the same row colour, so it changes nothing visible.
fn frozen_gutter(padding: &str) -> HashMap<String, String> {
    let (left, _) = horizontal_padding(padding);
    style(&[
        ("margin-right", &format!("calc(-1 * {left})")),
        (
            "mask-image",
            &format!("linear-gradient(to right, #000 calc(100% - {left} / 2), transparent)"),
        ),
    ])
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

/// The right-edge fade of a header or body row: a layer in the row's last
/// track, held at the right edge of the visible area (sticky) while columns
/// are hidden to the right. It takes the row's current, opaque background
/// (`inherit`, so a consumer's hover shows) masked from transparent to
/// solid, and stops 1px short of the row's bottom so the cells' border lines
/// are never faded.
///
/// The track is `fade_width` wide: `FADE_WIDTH_PX` while the columns at
/// their minimum are wider than the scroll area (the table scrolls), 0
/// otherwise (nothing overflows, nothing is drawn and no width is taken
/// from the columns). Scrolled to the right end, the fade sits over its own
/// strip, which holds only the last cell's extra padding, so it covers no
/// content.
fn fade_cell(column: usize) -> ChartElement {
    ChartElement::Span {
        class: "chartml-table-fade".to_string(),
        style: style(&[
            ("grid-column", &column.to_string()),
            ("grid-row", "1"),
            ("position", "sticky"),
            ("right", "0"),
            ("z-index", "1"),
            ("background", "inherit"),
            ("mask-image", "linear-gradient(to right, transparent, #000)"),
            ("pointer-events", "none"),
            ("margin-bottom", "1px"),
        ]),
        content: String::new(),
    }
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
        // first column gives way first, down to its scrolling width (see
        // below; its values wrap at spaces), then the table scrolls
        // sideways: header and body rows are at least the sum of the
        // minimums plus the fade.
        //
        // The expressions are set once, as custom properties on the scroll
        // area, and used by every row. Everything is relative to the
        // table's own container and font, never the viewport: `em` resolves
        // where a property is used (rows and the body carry the table
        // font) and `100cqw` is the scroll area's visible width (it is an
        // inline-size container for everything inside it).
        let (pad_left, pad_right) = horizontal_padding(&theme.table_cell_padding);
        let header = header_text(theme);
        let texts: Vec<ColumnText> = columns
            .iter()
            .enumerate()
            .map(|(i, c)| column_text(&c.label, &header, cells.iter().map(|r| r[i].as_str())))
            .collect();
        let width = |em: f64| format!("calc({em:.2}em + {pad_left} + {pad_right})");
        let min_var = |i: usize| format!("--chartml-table-min-{}", i + 1);
        let mut sizing: Vec<(String, String)> = Vec::new();
        if columns.len() > 1 {
            // While the table scrolls, the frozen column is `FROZEN_MAX_SHARE`
            // wide or its own one-line width if smaller, but never narrower
            // than its widest word unless that would leave less than
            // `MIN_SCROLL_WINDOW` beside it (and the fade) to scroll the
            // other columns through.
            sizing.push((
                "--chartml-table-frozen-scrolling".into(),
                format!(
                    "max(min({}, calc(100cqw - {FADE_WIDTH_PX}px - {MIN_SCROLL_WINDOW})), min({}, {FROZEN_MAX_SHARE}))",
                    width(texts[0].word),
                    width(texts[0].line),
                ),
            ));
            // Every other column fits beside it so its values can be seen
            // whole — wrapping at spaces if they must — but is never
            // narrower than its widest word.
            for (i, text) in texts.iter().enumerate().skip(1) {
                sizing.push((
                    min_var(i),
                    format!(
                        "max({}, min({}, calc(100cqw - var(--chartml-table-frozen-scrolling) - {FADE_WIDTH_PX}px)))",
                        width(text.word),
                        width(text.line),
                    ),
                ));
            }
            // The frozen column takes whatever width the other columns leave
            // it, up to its one-line width: it gives way before the table
            // scrolls, down to its scrolling width.
            let others: Vec<String> = (1..columns.len()).map(|i| format!("var({})", min_var(i))).collect();
            sizing.push((
                min_var(0),
                format!(
                    "min({}, max(var(--chartml-table-frozen-scrolling), calc(100cqw - ({}))))",
                    width(texts[0].line),
                    others.join(" + "),
                ),
            ));
        } else {
            sizing.push((min_var(0), width(texts[0].line)));
        }
        // The fade: `FADE_WIDTH_PX` once the columns at their minimum are
        // wider than the scroll area (the step completes within 0.04px),
        // otherwise 0. Rows are at least the columns plus the fade, so when
        // the table scrolls its right end shows the fade's own empty strip.
        let all_mins: Vec<String> = (0..columns.len()).map(|i| format!("var({})", min_var(i))).collect();
        sizing.push(("--chartml-table-columns-min".into(), format!("calc({})", all_mins.join(" + "))));
        sizing.push((
            "--chartml-table-fade".into(),
            format!("clamp(0px, calc((var(--chartml-table-columns-min) - 100cqw) * 1000), {FADE_WIDTH_PX}px)"),
        ));
        let fade_width = "var(--chartml-table-fade)".to_string();
        let row_min = "calc(var(--chartml-table-columns-min) + var(--chartml-table-fade))".to_string();
        let template = all_mins
            .iter()
            .map(|min| format!("minmax({min}, 1fr)"))
            .chain(std::iter::once(fade_width.clone()))
            .collect::<Vec<_>>()
            .join(" ");
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
        // body's frozen cells; opaque, so rows scrolling under it never show
        // through) ──
        let mut header_children: Vec<ChartElement> = columns
                .iter()
                .enumerate()
                .map(|(i, c)| header_cell(&c.label, &place(i), theme, &fade_width))
                .collect();
        header_children.push(fade_cell(fade_column));
        let header_row = ChartElement::Div {
            class: "chartml-table-header".to_string(),
            style: style(&[
                ("display", "grid"),
                ("grid-template-columns", &template),
                ("min-width", &row_min),
                ("position", "sticky"),
                ("top", "0"),
                ("z-index", "3"),
                ("background", &opaque_bg(&header_bg_var, theme)),
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
            let mut children: Vec<ChartElement> = row_cells
                .iter()
                .enumerate()
                .map(|(i, text)| body_cell(text, &place(i), theme, &fade_width))
                .collect();
            children.push(fade_cell(fade_column));
            body_children.push(ChartElement::Div {
                class: class.to_string(),
                style: style(&[
                    ("display", "grid"),
                    ("grid-template-columns", &template),
                    ("background", &opaque_bg(bg, theme)),
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
        let mut scroll_style = style(&[
            ("flex", "1 1 0"),
            ("min-height", "0"),
            ("overflow", "auto"),
            ("isolation", "isolate"),
            ("container-type", "inline-size"),
        ]);
        scroll_style.extend(sizing);
        let scroll = ChartElement::Div {
            class: "chartml-table-scroll".to_string(),
            style: scroll_style,
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

    /// The table's own cells in a header or body row (not the fade).
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

    #[test]
    fn column_text_follows_label_and_values_up_to_the_cap() {
        let plain = HeaderText { metrics: TextMetrics::default(), weight_scale: 1.0 };
        // Short content gets a short column: "NPS" / "44" needs about 2em,
        // no wider.
        let nps = column_text("NPS", &plain, ["44", "-15"].into_iter()).line;
        let expected = (measure_text("NPS", &plain.metrics) / 12.0 * TEXT_WIDTH_SLACK * 100.0).ceil() / 100.0;
        assert_eq!(nps, expected);
        assert!(nps < 3.0, "{nps}");
        // The widest word: a multi-word value can wrap at its spaces, a
        // number cannot.
        let t = column_text("Region", &plain, ["Northwind Traders International", "$1,234,567"].into_iter());
        let em = |w: &str| (measure_text(w, &plain.metrics) / 12.0 * TEXT_WIDTH_SLACK * 100.0).ceil() / 100.0;
        assert_eq!(t.word, em("International").max(em("$1,234,567")));
        assert!(t.word < t.line);
        // A single word longer than the cap (a URL, an ID) is capped too, so
        // it cannot force sideways scrolling in a wide container; it breaks.
        let url = format!("https://example.com/{}", "a".repeat(120));
        let t = column_text("Link", &plain, [url.as_str()].into_iter());
        assert_eq!((t.line, t.word), (MAX_MIN_TEXT_EM, MAX_MIN_TEXT_EM));
        // Nothing to show: no text width (the column is its padding).
        assert_eq!(column_text("", &plain, ["", ""].into_iter()).line, 0.0);
        // A long value: its estimated width (12px calibration) with slack, in em.
        let v = "Sub-Saharan Africa";
        let expected = measure_text(v, &plain.metrics) / 12.0 * TEXT_WIDTH_SLACK;
        let got = column_text("region", &plain, [v, "Oceania"].into_iter()).line;
        assert!((got - expected).abs() < 0.011, "{got} vs {expected}");
        // A label wider than every value sets the width.
        let label = "Customer lifetime value";
        assert!(column_text(label, &plain, ["1"].into_iter()).line > column_text("x", &plain, ["1"].into_iter()).line);
        // Very long values are capped (beyond it they wrap).
        let long = "x".repeat(200);
        assert_eq!(column_text("n", &plain, [long.as_str()].into_iter()).line, MAX_MIN_TEXT_EM);
        // Bold headers (the default theme's weight 600) measure 10% wider
        // than regular text; weight 500 does not.
        let mut theme = Theme::default();
        let bold = header_text(&theme);
        assert_eq!(bold.weight_scale, BOLD_WIDTH_SCALE);
        assert!(column_text("NPS", &bold, ["44"].into_iter()).line > nps);
        theme.table_header_font_weight = "500".into();
        let regular = header_text(&theme);
        assert_eq!(regular.weight_scale, 1.0);
        theme.table_header_font_weight = "bold".into();
        assert_eq!(header_text(&theme).weight_scale, BOLD_WIDTH_SCALE);
        // Uppercase, letter-spaced headers measure wider.
        theme.table_header_font_weight = "500".into();
        theme.table_header_text_transform = "uppercase".into();
        theme.table_header_letter_spacing = "0.08em".into();
        let kyomi = header_text(&theme);
        assert!(matches!(kyomi.metrics.text_transform, TextTransform::Uppercase));
        assert!((kyomi.metrics.letter_spacing_px - 0.96).abs() < 1e-9);
        assert!(column_text("Customers", &kyomi, [].into_iter()).line > column_text("Customers", &regular, [].into_iter()).line);
        theme.table_header_letter_spacing = "1.3px".into();
        theme.table_font_size = "13px".into();
        assert!((header_text(&theme).metrics.letter_spacing_px - 1.2).abs() < 1e-9);
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
    fn header_and_body_share_one_scroll_area_and_column_grid() {
        // Point 1: header and body sit in one scroll container and every
        // row is a grid with the same column template, so they scroll
        // sideways together and stay aligned; each column sits in the same
        // grid column in the header and in every row.
        let el = wide();
        let scroll = by_class(&el, "chartml-table-scroll");
        assert_eq!(scroll.len(), 1);
        let scroll = scroll[0];
        assert_eq!(get(scroll, "overflow"), Some("auto"));
        assert_eq!(get(scroll, "container-type"), Some("inline-size"));
        let kids: Vec<&str> = children_of(scroll).iter().map(class_of).collect();
        assert_eq!(kids, vec!["chartml-table-header", "chartml-table-body"]);
        let rows = header_and_rows(&el);
        let template = get(rows[0], "grid-template-columns");
        assert!(template.is_some());
        for row in &rows {
            assert_eq!(get(row, "display"), Some("grid"));
            assert_eq!(get(row, "grid-template-columns"), template);
            let cols: Vec<Option<&str>> = real_cells(row).iter().map(|c| get(c, "grid-column")).collect();
            assert_eq!(cols, real_cells(rows[0]).iter().map(|c| get(c, "grid-column")).collect::<Vec<_>>());
        }
        // Header and body are at least as wide as the columns' minimums
        // (the same expression on both), so every row spans the scroll.
        let body = by_class(&el, "chartml-table-body")[0];
        assert!(get(rows[0], "min-width").is_some());
        assert_eq!(get(rows[0], "min-width"), get(body, "min-width"));
    }

    #[test]
    fn measured_widths_reach_every_column_track() {
        // Point 1: each column's track minimum is built from that column's
        // measured one-line width and widest word (never zero), every row
        // uses the same tracks, in column order, ending with the fade.
        let data = wide_data();
        let el = TableRenderer::new().render(&data, &cfg(WIDE_SPEC)).unwrap();
        let scroll = by_class(&el, "chartml-table-scroll")[0];
        let header = header_text(&Theme::default());
        let columns = resolve_columns(&cfg(WIDE_SPEC).visualize, &data);
        let rows = header_and_rows(&el);
        let body_rows = &rows[1..];
        for (i, c) in columns.iter().enumerate() {
            let values: Vec<String> = (0..body_rows.len()).map(|r| format_cell(&data, r, c)).collect();
            let t = column_text(&c.label, &header, values.iter().map(String::as_str));
            assert!(t.line > 0.0, "{}", c.label);
            let min = get(scroll, &format!("--chartml-table-min-{}", i + 1)).unwrap();
            assert!(min.contains(&format!("calc({:.2}em + 12px + 12px)", t.line)), "{}: {min}", c.label);
            // The frozen column's widest word is part of its scrolling width.
            let word_in = if i == 0 { get(scroll, "--chartml-table-frozen-scrolling").unwrap() } else { min };
            assert!(word_in.contains(&format!("calc({:.2}em + 12px + 12px)", t.word)), "{}: {word_in}", c.label);
        }
        let expected: Vec<String> = (1..=columns.len())
            .map(|i| format!("minmax(var(--chartml-table-min-{i}), 1fr)"))
            .chain(std::iter::once("var(--chartml-table-fade)".to_string()))
            .collect();
        for row in &rows {
            assert_eq!(get(row, "grid-template-columns"), Some(expected.join(" ").as_str()));
        }
    }

    #[test]
    fn fade_turns_on_only_when_the_columns_overflow() {
        // Point 4: the fade's width is 0 unless the sum of all column
        // minimums is wider than the scroll area (`100cqw`), when it is
        // `FADE_WIDTH_PX`; rows are at least the columns plus the fade.
        let el = wide();
        let scroll = by_class(&el, "chartml-table-scroll")[0];
        let all: Vec<String> = (1..=12).map(|i| format!("var(--chartml-table-min-{i})")).collect();
        assert_eq!(get(scroll, "--chartml-table-columns-min"), Some(format!("calc({})", all.join(" + ")).as_str()));
        assert_eq!(
            get(scroll, "--chartml-table-fade"),
            Some("clamp(0px, calc((var(--chartml-table-columns-min) - 100cqw) * 1000), 32px)")
        );
        for row in by_class(&el, "chartml-table-header").into_iter().chain(by_class(&el, "chartml-table-body")) {
            assert_eq!(
                get(row, "min-width"),
                Some("calc(var(--chartml-table-columns-min) + var(--chartml-table-fade))")
            );
        }
    }

    #[test]
    fn rows_and_header_are_opaque_over_the_table_surface() {
        // Point 2: header and body rows lay their colour over the table
        // surface, so they are opaque even when a theme makes them
        // transparent; the frozen first cell takes its row's background
        // and is sticky at the left; no other cell is.
        let mut config = cfg(WIDE_SPEC);
        config.theme.table_header_bg = "transparent".into();
        config.theme.table_row_bg = "transparent".into();
        config.theme.table_surface = "#24201E".into();
        let el = TableRenderer::new().render(&wide_data(), &config).unwrap();
        for row in header_and_rows(&el) {
            let bg = get(row, "background").unwrap();
            assert!(bg.starts_with("linear-gradient("), "{bg}");
            assert!(bg.ends_with(", var(--chartml-table-surface, #24201E)"), "{bg}");
            let cells = real_cells(row);
            assert_eq!(get(cells[0], "position"), Some("sticky"));
            assert_eq!(get(cells[0], "left"), Some("0"));
            assert_eq!(get(cells[0], "background"), Some("inherit"));
            for c in &cells[1..] {
                assert_eq!(get(c, "position"), None);
                assert_eq!(get(c, "background"), None);
            }
        }
        let header = by_class(&el, "chartml-table-header")[0];
        assert_eq!(get(header, "position"), Some("sticky"));
        assert_eq!(get(header, "top"), Some("0"));
    }

    #[test]
    fn frozen_cell_covers_a_gutter_and_keeps_its_text_in_place() {
        // The frozen cell reaches over the next cell's left padding (12px in
        // the default theme), fading out, and widens its own right padding
        // by the same amount so its text does not move. A lone column has
        // nothing beside it and no gutter.
        let el = wide();
        for row in header_and_rows(&el) {
            let first = real_cells(row)[0];
            assert_eq!(get(first, "margin-right"), Some("calc(-1 * 12px)"));
            assert_eq!(get(first, "padding-right"), Some("calc(12px + 12px)"));
            assert!(get(first, "mask-image").is_some_and(|m| m.contains("transparent")));
        }
        let one = TableRenderer::new().render(&data(), &cfg("type: table\ncolumns: month\n")).unwrap();
        for row in header_and_rows(&one) {
            assert_eq!(get(real_cells(row)[0], "margin-right"), None);
        }
    }

    #[test]
    fn fade_is_the_last_cell_of_every_row() {
        // Point 4: every header and body row ends with a fade in its own
        // grid track: sticky at the right edge, the row's background masked
        // into a gradient, no border of its own, ignoring the pointer. The
        // last real cell reaches under that track so its border runs to the
        // row's end.
        let el = wide();
        for row in header_and_rows(&el) {
            let fade = children_of(row).last().unwrap();
            assert_eq!(class_of(fade), "chartml-table-fade");
            assert_eq!(get(fade, "grid-column"), Some("13"));
            assert_eq!(get(fade, "position"), Some("sticky"));
            assert_eq!(get(fade, "right"), Some("0"));
            assert_eq!(get(fade, "background"), Some("inherit"));
            assert_eq!(get(fade, "mask-image"), Some("linear-gradient(to right, transparent, #000)"));
            assert_eq!(get(fade, "pointer-events"), Some("none"));
            assert_eq!(get(fade, "border-bottom"), None);
            let last = *real_cells(row).last().unwrap();
            assert_eq!(get(last, "grid-column"), Some("12 / span 2"));
        }
    }

    #[test]
    fn sizes_from_container_not_viewport() {
        // Point 5: no viewport units anywhere; sizes are em of the table
        // font, px, percentages of the table's own box and `cqw` of its
        // scroll area.
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
        let scroll = by_class(&el, "chartml-table-scroll")[0];
        assert!(get(scroll, "--chartml-table-fade").unwrap().contains("100cqw"));
    }

    #[test]
    fn vertical_scroll_and_pager_unchanged() {
        // Point 6: the rows scroll vertically inside a flex-filling area
        // under the sticky header; the pager stays outside it, below.
        let el = wide();
        let kids: Vec<&str> = children_of(&el).iter().map(class_of).collect();
        assert_eq!(kids, vec!["chartml-table-scroll", "chartml-table-pager"]);
        let scroll = &children_of(&el)[0];
        assert_eq!(get(scroll, "flex"), Some("1 1 0"));
        assert_eq!(get(scroll, "min-height"), Some("0"));
        let mut contents = Vec::new();
        find_contents(&el, &mut contents);
        assert!(contents.iter().any(|c| c == "Showing 1–3 of 4 · Page 1 of 2"));
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
