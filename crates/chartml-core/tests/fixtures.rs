use std::fs;
use std::path::PathBuf;

use chartml_core::spec::{
    ChartMLSpec, ChartMode, Component, DataRef, FieldRef, MarkEncoding, Orientation,
};
use chartml_core::parse;

/// Returns the path to the fixtures directory at the workspace root.
fn fixtures_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // crates/chartml-core -> workspace root
    manifest_dir.join("../../fixtures").canonicalize().unwrap()
}

/// Helper: read and parse a single fixture file, returning the ChartSpec inside.
fn parse_fixture(name: &str) -> chartml_core::spec::ChartSpec {
    let path = fixtures_dir().join(name);
    let yaml = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", name, e));
    let spec = parse(&yaml)
        .unwrap_or_else(|e| panic!("Failed to parse fixture {}: {}", name, e));
    match spec {
        ChartMLSpec::Single(Component::Chart(chart)) => chart,
        other => panic!("Expected Single(Chart) for {}, got {:?}", name, other),
    }
}

// ---------------------------------------------------------------------------
// Generic: every YAML fixture in the directory must parse successfully
// ---------------------------------------------------------------------------

#[test]
fn all_fixtures_parse_successfully() {
    let dir = fixtures_dir();
    let mut count = 0;
    for entry in fs::read_dir(&dir).expect("Failed to read fixtures directory") {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("yaml") {
            let yaml = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
            let result = parse(&yaml);
            assert!(
                result.is_ok(),
                "Failed to parse {}: {:?}",
                path.display(),
                result.err()
            );
            // Every fixture should be a Single(Chart(...))
            let spec = result.unwrap();
            assert!(
                matches!(&spec, ChartMLSpec::Single(Component::Chart(_))),
                "Expected Single(Chart) for {}, got {:?}",
                path.display(),
                spec
            );
            count += 1;
        }
    }
    assert_eq!(count, 12, "Expected 12 YAML fixtures, found {}", count);
}

// ---------------------------------------------------------------------------
// bar_basic.yaml
// ---------------------------------------------------------------------------

#[test]
fn bar_basic_chart_type() {
    let chart = parse_fixture("bar_basic.yaml");
    assert_eq!(chart.visualize.chart_type, "bar");
}

#[test]
fn bar_basic_title() {
    let chart = parse_fixture("bar_basic.yaml");
    assert_eq!(chart.title.as_deref(), Some("Monthly Revenue"));
}

#[test]
fn bar_basic_inline_data_rows() {
    let chart = parse_fixture("bar_basic.yaml");
    match &chart.data {
        DataRef::Inline(data) => {
            assert_eq!(data.provider, "inline");
            let rows = data.rows.as_ref().expect("Expected rows");
            assert_eq!(rows.len(), 3, "bar_basic should have 3 data rows");
        }
        other => panic!("Expected Inline data, got {:?}", other),
    }
}

#[test]
fn bar_basic_columns_and_rows() {
    let chart = parse_fixture("bar_basic.yaml");
    match &chart.visualize.columns {
        Some(FieldRef::Simple(s)) => assert_eq!(s, "month"),
        other => panic!("Expected Simple columns field ref, got {:?}", other),
    }
    match &chart.visualize.rows {
        Some(FieldRef::Simple(s)) => assert_eq!(s, "revenue"),
        other => panic!("Expected Simple rows field ref, got {:?}", other),
    }
}

#[test]
fn bar_basic_axes() {
    let chart = parse_fixture("bar_basic.yaml");
    let axes = chart.visualize.axes.as_ref().expect("Expected axes");
    let rows_axis = axes.left.as_ref().expect("Expected rows/left axis");
    assert_eq!(rows_axis.label.as_deref(), Some("Revenue ($)"));
    assert_eq!(rows_axis.format.as_deref(), Some("$,.0f"));
}

// ---------------------------------------------------------------------------
// bar_stacked.yaml
// ---------------------------------------------------------------------------

#[test]
fn bar_stacked_chart_type_and_mode() {
    let chart = parse_fixture("bar_stacked.yaml");
    assert_eq!(chart.visualize.chart_type, "bar");
    assert!(
        matches!(chart.visualize.mode, Some(ChartMode::Stacked)),
        "Expected mode stacked, got {:?}",
        chart.visualize.mode
    );
}

#[test]
fn bar_stacked_marks_color() {
    let chart = parse_fixture("bar_stacked.yaml");
    let marks = chart.visualize.marks.as_ref().expect("Expected marks");
    match &marks.color {
        Some(MarkEncoding::Simple(s)) => assert_eq!(s, "product_line"),
        other => panic!("Expected Simple mark color, got {:?}", other),
    }
}

#[test]
fn bar_stacked_inline_data_rows() {
    let chart = parse_fixture("bar_stacked.yaml");
    match &chart.data {
        DataRef::Inline(data) => {
            let rows = data.rows.as_ref().expect("Expected rows");
            assert_eq!(rows.len(), 9, "bar_stacked should have 9 data rows (3 months x 3 products)");
        }
        other => panic!("Expected Inline data, got {:?}", other),
    }
}

#[test]
fn bar_stacked_transform() {
    let chart = parse_fixture("bar_stacked.yaml");
    let transform = chart.transform.as_ref().expect("Expected transform");
    let agg = transform.aggregate.as_ref().expect("Expected aggregate");
    assert_eq!(agg.dimensions.len(), 2);
    assert_eq!(agg.measures.len(), 1);
    assert_eq!(agg.measures[0].name, "total_revenue");
    assert_eq!(agg.measures[0].aggregation.as_deref(), Some("sum"));
}

#[test]
fn bar_stacked_style_height() {
    let chart = parse_fixture("bar_stacked.yaml");
    let style = chart.visualize.style.as_ref().expect("Expected visualize style");
    assert_eq!(style.height, Some(350.0));
}

// ---------------------------------------------------------------------------
// bar_grouped.yaml
// ---------------------------------------------------------------------------

#[test]
fn bar_grouped_mode() {
    let chart = parse_fixture("bar_grouped.yaml");
    assert_eq!(chart.visualize.chart_type, "bar");
    assert!(
        matches!(chart.visualize.mode, Some(ChartMode::Grouped)),
        "Expected mode grouped, got {:?}",
        chart.visualize.mode
    );
}

#[test]
fn bar_grouped_marks_color() {
    let chart = parse_fixture("bar_grouped.yaml");
    let marks = chart.visualize.marks.as_ref().expect("Expected marks");
    match &marks.color {
        Some(MarkEncoding::Simple(s)) => assert_eq!(s, "region"),
        other => panic!("Expected Simple mark color, got {:?}", other),
    }
}

#[test]
fn bar_grouped_inline_data() {
    let chart = parse_fixture("bar_grouped.yaml");
    match &chart.data {
        DataRef::Inline(data) => {
            let rows = data.rows.as_ref().expect("Expected rows");
            assert_eq!(rows.len(), 6, "bar_grouped should have 6 data rows (2 quarters x 3 regions)");
        }
        other => panic!("Expected Inline data, got {:?}", other),
    }
}

#[test]
fn bar_grouped_custom_colors() {
    let chart = parse_fixture("bar_grouped.yaml");
    let style = chart.visualize.style.as_ref().expect("Expected visualize style");
    let colors = style.colors.as_ref().expect("Expected colors");
    assert_eq!(colors.len(), 3);
    assert_eq!(colors[0], "#4285f4");
    assert_eq!(colors[1], "#ea4335");
    assert_eq!(colors[2], "#34a853");
}

#[test]
fn bar_grouped_transform() {
    let chart = parse_fixture("bar_grouped.yaml");
    let transform = chart.transform.as_ref().expect("Expected transform");
    let agg = transform.aggregate.as_ref().expect("Expected aggregate");
    assert_eq!(agg.dimensions.len(), 2);
    assert_eq!(agg.measures.len(), 1);
}

// ---------------------------------------------------------------------------
// bar_horizontal.yaml
// ---------------------------------------------------------------------------

#[test]
fn bar_horizontal_orientation() {
    let chart = parse_fixture("bar_horizontal.yaml");
    assert_eq!(chart.visualize.chart_type, "bar");
    assert!(
        matches!(chart.visualize.orientation, Some(Orientation::Horizontal)),
        "Expected orientation horizontal, got {:?}",
        chart.visualize.orientation
    );
}

#[test]
fn bar_horizontal_sort_desc() {
    let chart = parse_fixture("bar_horizontal.yaml");
    let transform = chart.transform.as_ref().expect("Expected transform");
    let agg = transform.aggregate.as_ref().expect("Expected aggregate");
    let sort = agg.sort.as_ref().expect("Expected sort");
    assert_eq!(sort.len(), 1);
    assert_eq!(sort[0].field, "total_revenue");
    assert_eq!(sort[0].direction.as_deref(), Some("desc"));
}

#[test]
fn bar_horizontal_inline_data() {
    let chart = parse_fixture("bar_horizontal.yaml");
    match &chart.data {
        DataRef::Inline(data) => {
            let rows = data.rows.as_ref().expect("Expected rows");
            assert_eq!(rows.len(), 4, "bar_horizontal should have 4 data rows");
        }
        other => panic!("Expected Inline data, got {:?}", other),
    }
}

#[test]
fn bar_horizontal_style_height() {
    let chart = parse_fixture("bar_horizontal.yaml");
    let style = chart.visualize.style.as_ref().expect("Expected visualize style");
    assert_eq!(style.height, Some(250.0));
}

// ---------------------------------------------------------------------------
// line_basic.yaml
// ---------------------------------------------------------------------------

#[test]
fn line_basic_chart_type() {
    let chart = parse_fixture("line_basic.yaml");
    assert_eq!(chart.visualize.chart_type, "line");
}

#[test]
fn line_basic_title() {
    let chart = parse_fixture("line_basic.yaml");
    assert_eq!(chart.title.as_deref(), Some("New Customers Over Time"));
}

#[test]
fn line_basic_date_columns() {
    let chart = parse_fixture("line_basic.yaml");
    match &chart.visualize.columns {
        Some(FieldRef::Simple(s)) => assert_eq!(s, "date"),
        other => panic!("Expected Simple columns 'date', got {:?}", other),
    }
}

#[test]
fn line_basic_inline_data_rows() {
    let chart = parse_fixture("line_basic.yaml");
    match &chart.data {
        DataRef::Inline(data) => {
            let rows = data.rows.as_ref().expect("Expected rows");
            assert_eq!(rows.len(), 5, "line_basic should have 5 data rows");
        }
        other => panic!("Expected Inline data, got {:?}", other),
    }
}

#[test]
fn line_basic_no_transform() {
    let chart = parse_fixture("line_basic.yaml");
    assert!(chart.transform.is_none(), "line_basic should have no transform");
}

// ---------------------------------------------------------------------------
// line_multi_series.yaml
// ---------------------------------------------------------------------------

#[test]
fn line_multi_series_chart_type() {
    let chart = parse_fixture("line_multi_series.yaml");
    assert_eq!(chart.visualize.chart_type, "line");
}

#[test]
fn line_multi_series_marks_color() {
    let chart = parse_fixture("line_multi_series.yaml");
    let marks = chart.visualize.marks.as_ref().expect("Expected marks");
    match &marks.color {
        Some(MarkEncoding::Simple(s)) => assert_eq!(s, "region"),
        other => panic!("Expected Simple mark color 'region', got {:?}", other),
    }
}

#[test]
fn line_multi_series_inline_data() {
    let chart = parse_fixture("line_multi_series.yaml");
    match &chart.data {
        DataRef::Inline(data) => {
            let rows = data.rows.as_ref().expect("Expected rows");
            assert_eq!(rows.len(), 12, "line_multi_series should have 12 data rows (4 weeks x 3 regions)");
        }
        other => panic!("Expected Inline data, got {:?}", other),
    }
}

#[test]
fn line_multi_series_transform() {
    let chart = parse_fixture("line_multi_series.yaml");
    let transform = chart.transform.as_ref().expect("Expected transform");
    let agg = transform.aggregate.as_ref().expect("Expected aggregate");
    assert_eq!(agg.dimensions.len(), 2);
    assert_eq!(agg.measures[0].name, "total_revenue");
}

// ---------------------------------------------------------------------------
// area_stacked.yaml
// ---------------------------------------------------------------------------

#[test]
fn area_stacked_chart_type_and_mode() {
    let chart = parse_fixture("area_stacked.yaml");
    assert_eq!(chart.visualize.chart_type, "area");
    assert!(
        matches!(chart.visualize.mode, Some(ChartMode::Stacked)),
        "Expected mode stacked, got {:?}",
        chart.visualize.mode
    );
}

#[test]
fn area_stacked_marks_color() {
    let chart = parse_fixture("area_stacked.yaml");
    let marks = chart.visualize.marks.as_ref().expect("Expected marks");
    match &marks.color {
        Some(MarkEncoding::Simple(s)) => assert_eq!(s, "region"),
        other => panic!("Expected Simple mark color 'region', got {:?}", other),
    }
}

#[test]
fn area_stacked_axes() {
    let chart = parse_fixture("area_stacked.yaml");
    let axes = chart.visualize.axes.as_ref().expect("Expected axes");
    let rows_axis = axes.left.as_ref().expect("Expected rows/left axis");
    assert_eq!(rows_axis.label.as_deref(), Some("Revenue ($)"));
}

#[test]
fn area_stacked_inline_data() {
    let chart = parse_fixture("area_stacked.yaml");
    match &chart.data {
        DataRef::Inline(data) => {
            let rows = data.rows.as_ref().expect("Expected rows");
            assert_eq!(rows.len(), 12, "area_stacked should have 12 data rows");
        }
        other => panic!("Expected Inline data, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// scatter_basic.yaml
// ---------------------------------------------------------------------------

#[test]
fn scatter_basic_chart_type() {
    let chart = parse_fixture("scatter_basic.yaml");
    assert_eq!(chart.visualize.chart_type, "scatter");
}

#[test]
fn scatter_basic_columns_and_rows() {
    let chart = parse_fixture("scatter_basic.yaml");
    match &chart.visualize.columns {
        Some(FieldRef::Simple(s)) => assert_eq!(s, "price"),
        other => panic!("Expected Simple columns 'price', got {:?}", other),
    }
    match &chart.visualize.rows {
        Some(FieldRef::Simple(s)) => assert_eq!(s, "units"),
        other => panic!("Expected Simple rows 'units', got {:?}", other),
    }
}

#[test]
fn scatter_basic_marks_color() {
    let chart = parse_fixture("scatter_basic.yaml");
    let marks = chart.visualize.marks.as_ref().expect("Expected marks");
    match &marks.color {
        Some(MarkEncoding::Simple(s)) => assert_eq!(s, "category"),
        other => panic!("Expected Simple mark color 'category', got {:?}", other),
    }
}

#[test]
fn scatter_basic_inline_data() {
    let chart = parse_fixture("scatter_basic.yaml");
    match &chart.data {
        DataRef::Inline(data) => {
            let rows = data.rows.as_ref().expect("Expected rows");
            assert_eq!(rows.len(), 6, "scatter_basic should have 6 data rows");
        }
        other => panic!("Expected Inline data, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// scatter_bubble.yaml
// ---------------------------------------------------------------------------

#[test]
fn scatter_bubble_chart_type() {
    let chart = parse_fixture("scatter_bubble.yaml");
    assert_eq!(chart.visualize.chart_type, "scatter");
}

#[test]
fn scatter_bubble_marks_size() {
    let chart = parse_fixture("scatter_bubble.yaml");
    let marks = chart.visualize.marks.as_ref().expect("Expected marks");
    match &marks.size {
        Some(MarkEncoding::Simple(s)) => assert_eq!(s, "units"),
        other => panic!("Expected Simple mark size 'units', got {:?}", other),
    }
}

#[test]
fn scatter_bubble_marks_color() {
    let chart = parse_fixture("scatter_bubble.yaml");
    let marks = chart.visualize.marks.as_ref().expect("Expected marks");
    match &marks.color {
        Some(MarkEncoding::Simple(s)) => assert_eq!(s, "category"),
        other => panic!("Expected Simple mark color 'category', got {:?}", other),
    }
}

#[test]
fn scatter_bubble_inline_data() {
    let chart = parse_fixture("scatter_bubble.yaml");
    match &chart.data {
        DataRef::Inline(data) => {
            let rows = data.rows.as_ref().expect("Expected rows");
            assert_eq!(rows.len(), 6, "scatter_bubble should have 6 data rows");
        }
        other => panic!("Expected Inline data, got {:?}", other),
    }
}

#[test]
fn scatter_bubble_style_height() {
    let chart = parse_fixture("scatter_bubble.yaml");
    let style = chart.visualize.style.as_ref().expect("Expected visualize style");
    assert_eq!(style.height, Some(400.0));
}

// ---------------------------------------------------------------------------
// pie_basic.yaml
// ---------------------------------------------------------------------------

#[test]
fn pie_basic_chart_type() {
    let chart = parse_fixture("pie_basic.yaml");
    assert_eq!(chart.visualize.chart_type, "pie");
}

#[test]
fn pie_basic_title() {
    let chart = parse_fixture("pie_basic.yaml");
    assert_eq!(chart.title.as_deref(), Some("Revenue by Region"));
}

#[test]
fn pie_basic_inline_data() {
    let chart = parse_fixture("pie_basic.yaml");
    match &chart.data {
        DataRef::Inline(data) => {
            let rows = data.rows.as_ref().expect("Expected rows");
            assert_eq!(rows.len(), 4, "pie_basic should have 4 data rows");
        }
        other => panic!("Expected Inline data, got {:?}", other),
    }
}

#[test]
fn pie_basic_columns_and_rows() {
    let chart = parse_fixture("pie_basic.yaml");
    match &chart.visualize.columns {
        Some(FieldRef::Simple(s)) => assert_eq!(s, "region"),
        other => panic!("Expected Simple columns 'region', got {:?}", other),
    }
    match &chart.visualize.rows {
        Some(FieldRef::Simple(s)) => assert_eq!(s, "revenue"),
        other => panic!("Expected Simple rows 'revenue', got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// doughnut_basic.yaml
// ---------------------------------------------------------------------------

#[test]
fn doughnut_basic_chart_type() {
    let chart = parse_fixture("doughnut_basic.yaml");
    assert_eq!(chart.visualize.chart_type, "doughnut");
}

#[test]
fn doughnut_basic_title() {
    let chart = parse_fixture("doughnut_basic.yaml");
    assert_eq!(chart.title.as_deref(), Some("Revenue Distribution"));
}

#[test]
fn doughnut_basic_custom_colors() {
    let chart = parse_fixture("doughnut_basic.yaml");
    let style = chart.visualize.style.as_ref().expect("Expected visualize style");
    let colors = style.colors.as_ref().expect("Expected colors");
    assert_eq!(colors.len(), 4);
    assert_eq!(colors[0], "#4285f4");
    assert_eq!(colors[1], "#ea4335");
    assert_eq!(colors[2], "#fbbc04");
    assert_eq!(colors[3], "#34a853");
}

#[test]
fn doughnut_basic_inline_data() {
    let chart = parse_fixture("doughnut_basic.yaml");
    match &chart.data {
        DataRef::Inline(data) => {
            let rows = data.rows.as_ref().expect("Expected rows");
            assert_eq!(rows.len(), 4, "doughnut_basic should have 4 data rows");
        }
        other => panic!("Expected Inline data, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// metric_basic.yaml
// ---------------------------------------------------------------------------

#[test]
fn metric_basic_chart_type() {
    let chart = parse_fixture("metric_basic.yaml");
    assert_eq!(chart.visualize.chart_type, "metric");
}

#[test]
fn metric_basic_value() {
    let chart = parse_fixture("metric_basic.yaml");
    assert_eq!(chart.visualize.value.as_deref(), Some("current"));
}

#[test]
fn metric_basic_label() {
    let chart = parse_fixture("metric_basic.yaml");
    assert_eq!(chart.visualize.label.as_deref(), Some("Current Revenue"));
}

#[test]
fn metric_basic_format() {
    let chart = parse_fixture("metric_basic.yaml");
    assert_eq!(chart.visualize.format.as_deref(), Some("$,.0f"));
}

#[test]
fn metric_basic_compare_with() {
    let chart = parse_fixture("metric_basic.yaml");
    assert_eq!(chart.visualize.compare_with.as_deref(), Some("previous"));
}

#[test]
fn metric_basic_invert_trend() {
    let chart = parse_fixture("metric_basic.yaml");
    assert_eq!(chart.visualize.invert_trend, Some(false));
}

#[test]
fn metric_basic_layout() {
    let chart = parse_fixture("metric_basic.yaml");
    let layout = chart.layout.as_ref().expect("Expected layout");
    assert_eq!(layout.col_span, Some(3));
}

#[test]
fn metric_basic_inline_data() {
    let chart = parse_fixture("metric_basic.yaml");
    match &chart.data {
        DataRef::Inline(data) => {
            let rows = data.rows.as_ref().expect("Expected rows");
            assert_eq!(rows.len(), 1, "metric_basic should have 1 data row");
        }
        other => panic!("Expected Inline data, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Cross-cutting: version field
// ---------------------------------------------------------------------------

#[test]
fn all_fixtures_have_version_1() {
    let fixtures = [
        "bar_basic.yaml",
        "bar_stacked.yaml",
        "bar_grouped.yaml",
        "bar_horizontal.yaml",
        "line_basic.yaml",
        "line_multi_series.yaml",
        "area_stacked.yaml",
        "scatter_basic.yaml",
        "scatter_bubble.yaml",
        "pie_basic.yaml",
        "doughnut_basic.yaml",
        "metric_basic.yaml",
    ];
    for name in &fixtures {
        let chart = parse_fixture(name);
        assert_eq!(chart.version, 1, "Fixture {} should have version 1", name);
    }
}

// ---------------------------------------------------------------------------
// Cross-cutting: all fixtures with inline data have provider "inline"
// ---------------------------------------------------------------------------

#[test]
fn all_fixtures_have_inline_provider() {
    let fixtures = [
        "bar_basic.yaml",
        "bar_stacked.yaml",
        "bar_grouped.yaml",
        "bar_horizontal.yaml",
        "line_basic.yaml",
        "line_multi_series.yaml",
        "area_stacked.yaml",
        "scatter_basic.yaml",
        "scatter_bubble.yaml",
        "pie_basic.yaml",
        "doughnut_basic.yaml",
        "metric_basic.yaml",
    ];
    for name in &fixtures {
        let chart = parse_fixture(name);
        match &chart.data {
            DataRef::Inline(data) => {
                assert_eq!(
                    data.provider, "inline",
                    "Fixture {} should have provider 'inline'",
                    name
                );
            }
            other => panic!("Expected Inline data for {}, got {:?}", name, other),
        }
    }
}
