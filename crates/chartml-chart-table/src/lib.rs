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
use chartml_core::plugin::{ChartConfig, ChartRenderer};
use chartml_core::spec::{FieldRef, FieldRefItem, FieldSpec, VisualizeSpec};
use chartml_core::theme::Theme;

const DEFAULT_PAGE_SIZE: usize = 50;
const DEFAULT_HEIGHT: f64 = 400.0;

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

/// Format a cell value. Numeric values go through `NumberFormatter` when a
/// format string is present; otherwise both numeric and string values are
/// rendered as plain text. A true null (both `get_f64` and `get_string` return
/// `None`) renders as "—" (U+2014) so the table always has visible content.
/// An actual empty string (`Some("")`) is preserved as-is.
fn format_cell(data: &DataTable, row: usize, col: &Column) -> String {
    if let Some(n) = data.get_f64(row, &col.field) {
        if let Some(fmt) = &col.format {
            return NumberFormatter::new(fmt).format(n);
        }
        return format_number_plain(n);
    }
    match data.get_string(row, &col.field) {
        Some(s) => s,
        None => "\u{2014}".to_string(),
    }
}

fn format_number_plain(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e16 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

fn style(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

/// Build a header <span> cell (header cells are flex children of the header row div).
fn header_cell(label: &str, theme: &Theme) -> ChartElement {
    ChartElement::Span {
        class: "chartml-table-header-cell".to_string(),
        style: style(&[
            ("flex", "1 1 0"),
            ("padding", &theme.table_cell_padding),
            ("font-weight", "600"),
            ("color", &theme.table_header_text),
            ("text-align", "left"),
            ("border-bottom", &format!("1px solid {}", theme.table_border)),
        ]),
        content: label.to_string(),
    }
}

fn body_cell(content: &str, theme: &Theme) -> ChartElement {
    ChartElement::Span {
        class: "chartml-table-cell".to_string(),
        style: style(&[
            ("flex", "1 1 0"),
            ("padding", &theme.table_cell_padding),
            ("color", &theme.table_text),
            ("text-align", "left"),
            ("border-bottom", &format!("1px solid {}", theme.table_border)),
            ("overflow", "hidden"),
            ("text-overflow", "ellipsis"),
            ("white-space", "nowrap"),
        ]),
        content: content.to_string(),
    }
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

        // ── Header row ──
        let header_row = ChartElement::Div {
            class: "chartml-table-header".to_string(),
            style: style(&[
                ("display", "flex"),
                ("background", &theme.table_header_bg),
                ("font-size", &theme.table_font_size),
                ("font-family", &theme.label_font_family),
            ]),
            children: columns
                .iter()
                .map(|c| header_cell(&c.label, theme))
                .collect(),
        };

        // ── Body rows ──
        let mut body_children: Vec<ChartElement> = Vec::with_capacity(end);
        for row_idx in 0..end {
            let is_alt = row_idx % 2 == 1;
            let bg = if is_alt {
                &theme.table_row_bg_alt
            } else {
                &theme.table_row_bg
            };
            let class = if is_alt {
                "chartml-table-row chartml-table-row-alt"
            } else {
                "chartml-table-row"
            };
            let cells: Vec<ChartElement> = columns
                .iter()
                .map(|col| body_cell(&format_cell(data, row_idx, col), theme))
                .collect();
            body_children.push(ChartElement::Div {
                class: class.to_string(),
                style: style(&[
                    ("display", "flex"),
                    ("background", bg),
                    ("font-size", &theme.table_font_size),
                    ("font-family", &theme.label_font_family),
                ]),
                children: cells,
            });
        }
        let body = ChartElement::Div {
            class: "chartml-table-body".to_string(),
            style: style(&[("display", "flex"), ("flex-direction", "column")]),
            children: body_children,
        };

        // ── Pager footer (shown when there are more rows than one page) ──
        let mut root_children: Vec<ChartElement> = vec![header_row, body];
        if total_rows > page_size {
            let page_count = total_rows.div_ceil(page_size);
            let info = ChartElement::Span {
                class: "chartml-table-pager-info".to_string(),
                style: style(&[
                    ("color", &theme.text_secondary),
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
                    ("border-top", &format!("1px solid {}", theme.table_border)),
                    ("background", &theme.table_header_bg),
                ]),
                children: vec![info],
            });
        }

        Ok(ChartElement::Div {
            class: "chartml-table".to_string(),
            style: style(&[
                ("display", "flex"),
                ("flex-direction", "column"),
                ("width", "100%"),
                ("background", &theme.table_row_bg),
                ("color", &theme.table_text),
                ("border", &format!("1px solid {}", theme.table_border)),
                ("border-radius", "4px"),
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
