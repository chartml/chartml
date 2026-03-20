use leptos::prelude::*;

pub const DEFAULT_SPEC: &str = r#"type: chart
version: 1
title: "Monthly Revenue"
data:
  provider: inline
  rows:
    - month: "Jan"
      revenue: 125000
    - month: "Feb"
      revenue: 138000
    - month: "Mar"
      revenue: 152000
    - month: "Apr"
      revenue: 145000
    - month: "May"
      revenue: 168000
visualize:
  type: bar
  columns: month
  rows: revenue
  axes:
    rows:
      label: "Revenue ($)"
      format: "$,.0f""#;

const LINE_SPEC: &str = r#"type: chart
version: 1
title: "New Customers Over Time"
data:
  provider: inline
  rows:
    - date: "2024-01-01"
      customers: 450
    - date: "2024-01-08"
      customers: 485
    - date: "2024-01-15"
      customers: 520
    - date: "2024-01-22"
      customers: 562
    - date: "2024-01-29"
      customers: 598
visualize:
  type: line
  columns: date
  rows: customers"#;

const AREA_SPEC: &str = r#"type: chart
version: 1
title: "Regional Revenue Composition"
data:
  provider: inline
  rows:
    - week: "Week 1"
      region: "North"
      revenue: 42000
    - week: "Week 1"
      region: "South"
      revenue: 38000
    - week: "Week 1"
      region: "East"
      revenue: 35000
    - week: "Week 2"
      region: "North"
      revenue: 45000
    - week: "Week 2"
      region: "South"
      revenue: 40000
    - week: "Week 2"
      region: "East"
      revenue: 37000
    - week: "Week 3"
      region: "North"
      revenue: 48000
    - week: "Week 3"
      region: "South"
      revenue: 42000
    - week: "Week 3"
      region: "East"
      revenue: 40000
    - week: "Week 4"
      region: "North"
      revenue: 52000
    - week: "Week 4"
      region: "South"
      revenue: 45000
    - week: "Week 4"
      region: "East"
      revenue: 43000
transform:
  aggregate:
    dimensions: [week, region]
    measures:
      - column: revenue
        aggregation: sum
        name: total_revenue
visualize:
  type: area
  mode: stacked
  columns: week
  rows: total_revenue
  marks:
    color: region
  axes:
    rows:
      label: "Revenue ($)"
  style:
    height: 300"#;

const PIE_SPEC: &str = r#"type: chart
version: 1
title: "Revenue by Region"
data:
  provider: inline
  rows:
    - region: "North America"
      revenue: 520000
    - region: "Europe"
      revenue: 380000
    - region: "Asia Pacific"
      revenue: 450000
    - region: "Latin America"
      revenue: 125000
visualize:
  type: pie
  columns: region
  rows: revenue
  style:
    height: 300"#;

const SCATTER_SPEC: &str = r#"type: chart
version: 1
title: "Price vs Sales Volume"
data:
  provider: inline
  rows:
    - price: 29.99
      units: 450
      category: "Electronics"
    - price: 39.99
      units: 380
      category: "Electronics"
    - price: 49.99
      units: 320
      category: "Electronics"
    - price: 19.99
      units: 520
      category: "Clothing"
    - price: 24.99
      units: 480
      category: "Clothing"
    - price: 34.99
      units: 410
      category: "Clothing"
visualize:
  type: scatter
  columns: price
  rows: units
  marks:
    color: category"#;

const METRIC_SPEC: &str = r#"type: chart
version: 1
title: "Total Revenue"
data:
  provider: inline
  rows:
    - current: 1234567
      previous: 1100000
visualize:
  type: metric
  value: current
  label: "Current Revenue"
  format: "$,.0f"
  compareWith: previous
  invertTrend: false"#;

const STACKED_BAR_SPEC: &str = r#"type: chart
version: 1
title: "Revenue Composition by Month"
data:
  provider: inline
  rows:
    - month: "Jan"
      product_line: "Hardware"
      revenue: 65000
    - month: "Jan"
      product_line: "Software"
      revenue: 40000
    - month: "Jan"
      product_line: "Services"
      revenue: 20000
    - month: "Feb"
      product_line: "Hardware"
      revenue: 72000
    - month: "Feb"
      product_line: "Software"
      revenue: 45000
    - month: "Feb"
      product_line: "Services"
      revenue: 21000
    - month: "Mar"
      product_line: "Hardware"
      revenue: 78000
    - month: "Mar"
      product_line: "Software"
      revenue: 52000
    - month: "Mar"
      product_line: "Services"
      revenue: 22000
transform:
  aggregate:
    dimensions: [month, product_line]
    measures:
      - column: revenue
        aggregation: sum
        name: total_revenue
visualize:
  type: bar
  mode: stacked
  columns: month
  rows: total_revenue
  marks:
    color: product_line
  style:
    height: 350"#;

const MULTI_LINE_SPEC: &str = r#"type: chart
version: 1
title: "Regional Revenue Trends"
data:
  provider: inline
  rows:
    - week: "Week 1"
      region: "North"
      revenue: 42000
    - week: "Week 1"
      region: "South"
      revenue: 38000
    - week: "Week 1"
      region: "East"
      revenue: 35000
    - week: "Week 2"
      region: "North"
      revenue: 45000
    - week: "Week 2"
      region: "South"
      revenue: 40000
    - week: "Week 2"
      region: "East"
      revenue: 37000
    - week: "Week 3"
      region: "North"
      revenue: 48000
    - week: "Week 3"
      region: "South"
      revenue: 42000
    - week: "Week 3"
      region: "East"
      revenue: 40000
    - week: "Week 4"
      region: "North"
      revenue: 52000
    - week: "Week 4"
      region: "South"
      revenue: 45000
    - week: "Week 4"
      region: "East"
      revenue: 43000
transform:
  aggregate:
    dimensions: [week, region]
    measures:
      - column: revenue
        aggregation: sum
        name: total_revenue
visualize:
  type: line
  columns: week
  rows: total_revenue
  marks:
    color: region
  style:
    height: 300"#;

struct GalleryItem {
    name: &'static str,
    spec: &'static str,
}

const GALLERY_ITEMS: &[GalleryItem] = &[
    GalleryItem { name: "Bar", spec: DEFAULT_SPEC },
    GalleryItem { name: "Line", spec: LINE_SPEC },
    GalleryItem { name: "Area", spec: AREA_SPEC },
    GalleryItem { name: "Pie", spec: PIE_SPEC },
    GalleryItem { name: "Scatter", spec: SCATTER_SPEC },
    GalleryItem { name: "Metric", spec: METRIC_SPEC },
    GalleryItem { name: "Stacked Bar", spec: STACKED_BAR_SPEC },
    GalleryItem { name: "Multi-Line", spec: MULTI_LINE_SPEC },
];

#[component]
pub fn Gallery(
    on_select: impl Fn(String) + 'static + Send + Sync + Clone,
) -> impl IntoView {
    view! {
        <div class="gallery">
            {GALLERY_ITEMS.iter().map(|item| {
                let spec = item.spec.to_string();
                let on_select = on_select.clone();
                let name = item.name;
                view! {
                    <button
                        class="gallery-button"
                        on:click=move |_| on_select(spec.clone())
                    >
                        {name}
                    </button>
                }
            }).collect::<Vec<_>>()}
        </div>
    }
}
