use leptos::prelude::*;

// ─── Specs copied verbatim from chartml JS examples page ───
// Source: /home/jason/repos/chartml/docs/docs/examples.md
// Only charts with inline data (no named sources / transforms in v0.1)

pub const DEFAULT_SPEC: &str = r##"type: chart
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
visualize:
  type: bar
  columns: month
  rows: revenue"##;

const LINE_SPEC: &str = r##"type: chart
version: 1
title: "New Customers"
data:
  provider: inline
  rows:
    - month: "Jan"
      customers: 450
    - month: "Feb"
      customers: 485
    - month: "Mar"
      customers: 520
visualize:
  type: line
  columns: month
  rows: customers"##;

const PIE_SPEC: &str = r##"type: chart
version: 1
title: "Regional Breakdown"
data:
  provider: inline
  rows:
    - region: "US"
      revenue: 85000
    - region: "EU"
      revenue: 67000
    - region: "Asia"
      revenue: 52000
    - region: "LatAm"
      revenue: 31000
visualize:
  type: pie
  columns: region
  rows: revenue"##;

const SCATTER_SPEC: &str = r##"type: chart
version: 1
title: "Marketing Budget vs Sales"
data:
  provider: inline
  rows:
    - budget: 50000
      sales: 125000
      category: "Electronics"
    - budget: 35000
      sales: 88000
      category: "Clothing"
    - budget: 65000
      sales: 165000
      category: "Electronics"
    - budget: 40000
      sales: 95000
      category: "Clothing"
    - budget: 55000
      sales: 140000
      category: "Home"
    - budget: 30000
      sales: 75000
      category: "Home"
visualize:
  type: scatter
  columns: budget
  rows: sales
  marks:
    color: category"##;

// ─── KPI Metrics (from examples.md "KPI Overview" section) ───

const METRIC_REVENUE_SPEC: &str = r##"type: chart
version: 1
title: "Total Revenue"
data:
  provider: inline
  rows:
    - { current: 1234567, previous: 1100000 }
visualize:
  type: metric
  value: current
  format: "$,.0f"
  compareWith: previous"##;

const METRIC_ERROR_RATE_SPEC: &str = r##"type: chart
version: 1
data:
  provider: inline
  rows:
    - { current: 0.023, previous: 0.031 }
visualize:
  type: metric
  value: current
  label: "Error Rate"
  format: ".2%"
  compareWith: previous
  invertTrend: true"##;

// ─── Revenue vs Goal (from examples.md "Reference Lines" section, without annotations) ───

const BAR_REVENUE_GOAL_SPEC: &str = r##"type: chart
version: 1
title: "Monthly Revenue vs Goal"
data:
  provider: inline
  rows:
    - { month: "Jan", revenue: 120000 }
    - { month: "Feb", revenue: 135000 }
    - { month: "Mar", revenue: 148000 }
    - { month: "Apr", revenue: 162000 }
    - { month: "May", revenue: 145000 }
    - { month: "Jun", revenue: 158000 }
visualize:
  type: bar
  columns: month
  rows: revenue
  axes:
    rows:
      label: "Revenue ($)"
      format: "$,.0f""##;

// ─── Combo chart: Actual vs Target (from examples.md "Advanced Analytics") ───

const COMBO_SPEC: &str = r##"type: chart
version: 1
title: "Actual Revenue vs Target"
data:
  provider: inline
  rows:
    - month: "Jan"
      actual: 125000
      target: 120000
    - month: "Feb"
      actual: 138000
      target: 130000
    - month: "Mar"
      actual: 152000
      target: 145000
visualize:
  type: bar
  columns: month
  rows:
    - field: actual
      mark: bar
      color: "#4285f4"
      label: "Actual Revenue"
    - field: target
      mark: line
      color: "#ea4335"
      label: "Target"
  style:
    height: 300"##;

// ─── Stacked bar (from examples.md "Revenue Composition") ───

const STACKED_BAR_SPEC: &str = r##"type: chart
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
visualize:
  type: bar
  mode: stacked
  columns: month
  rows: revenue
  marks:
    color: product_line
  style:
    height: 350"##;

// ─── Multi-line (from examples.md "Regional Revenue Trends") ───

const MULTI_LINE_SPEC: &str = r##"type: chart
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
visualize:
  type: line
  columns: week
  rows: revenue
  marks:
    color: region
  style:
    height: 300"##;

// ─── Area chart (from examples.md "Cumulative Revenue Growth") ───

const AREA_SPEC: &str = r##"type: chart
version: 1
title: "Cumulative Revenue Growth"
data:
  provider: inline
  rows:
    - week: "Week 1"
      revenue: 115000
    - week: "Week 2"
      revenue: 122000
    - week: "Week 3"
      revenue: 130000
    - week: "Week 4"
      revenue: 140000
    - week: "Week 5"
      revenue: 145000
    - week: "Week 6"
      revenue: 158000
visualize:
  type: area
  columns: week
  rows: revenue
  axes:
    rows:
      label: "Revenue ($)"
  style:
    height: 300"##;

// ─── Doughnut (from examples.md pattern) ───

const DOUGHNUT_SPEC: &str = r##"type: chart
version: 1
title: "Revenue Distribution"
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
  type: doughnut
  columns: region
  rows: revenue
  style:
    height: 400
    colors: ["#4285f4", "#ea4335", "#fbbc04", "#34a853"]"##;

// ─── Bubble chart (from examples.md pattern) ───

const BUBBLE_SPEC: &str = r##"type: chart
version: 1
title: "Revenue Efficiency Analysis"
data:
  provider: inline
  rows:
    - product: "Widget A"
      revenue: 125000
      profit: 45000
      units: 2400
      category: "Hardware"
    - product: "Widget B"
      revenue: 98000
      profit: 38000
      units: 1800
      category: "Hardware"
    - product: "Software X"
      revenue: 156000
      profit: 92000
      units: 450
      category: "Software"
    - product: "Software Y"
      revenue: 134000
      profit: 78000
      units: 380
      category: "Software"
    - product: "Service A"
      revenue: 67000
      profit: 28000
      units: 120
      category: "Services"
    - product: "Service B"
      revenue: 89000
      profit: 35000
      units: 150
      category: "Services"
visualize:
  type: scatter
  columns: revenue
  rows: profit
  marks:
    color: category
    size: units
  style:
    height: 400"##;

// ─── Label Strategy Demo specs ───

const TWELVE_MONTHS_SPEC: &str = r##"type: chart
version: 1
title: "Monthly Revenue 2024"
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
    - month: "Jun"
      revenue: 158000
    - month: "Jul"
      revenue: 172000
    - month: "Aug"
      revenue: 165000
    - month: "Sep"
      revenue: 180000
    - month: "Oct"
      revenue: 175000
    - month: "Nov"
      revenue: 190000
    - month: "Dec"
      revenue: 195000
visualize:
  type: bar
  columns: month
  rows: revenue
  axes:
    rows:
      label: "Revenue"
      format: "$,.0f""##;

const DAILY_SALES_DATES_SPEC: &str = r##"type: chart
version: 1
title: "Daily Sales - March 2025"
data:
  provider: inline
  rows:
    - date: "2025-03-10"
      sales: 4200
    - date: "2025-03-11"
      sales: 4350
    - date: "2025-03-12"
      sales: 4100
    - date: "2025-03-13"
      sales: 4500
    - date: "2025-03-14"
      sales: 4400
    - date: "2025-03-15"
      sales: 6200
    - date: "2025-03-16"
      sales: 7100
    - date: "2025-03-17"
      sales: 6800
    - date: "2025-03-18"
      sales: 7500
visualize:
  type: line
  columns: date
  rows: sales
  axes:
    rows:
      label: "Daily Sales"
      format: ",.0f""##;

const LONG_CATEGORIES_SPEC: &str = r##"type: chart
version: 1
title: "Revenue by Product Line"
data:
  provider: inline
  rows:
    - product: "Enterprise Cloud Platform"
      revenue: 450000
    - product: "Developer Tools Suite"
      revenue: 380000
    - product: "Data Analytics Engine"
      revenue: 520000
    - product: "Security & Compliance"
      revenue: 290000
    - product: "Mobile SDK Framework"
      revenue: 340000
    - product: "AI/ML Infrastructure"
      revenue: 610000
    - product: "Customer Success Platform"
      revenue: 270000
    - product: "Integration Middleware"
      revenue: 310000
visualize:
  type: bar
  columns: product
  rows: revenue
  axes:
    rows:
      format: "$,.0f""##;

struct GalleryItem {
    name: &'static str,
    spec: &'static str,
}

const GALLERY_ITEMS: &[GalleryItem] = &[
    GalleryItem { name: "Bar", spec: DEFAULT_SPEC },
    GalleryItem { name: "Bar (6 months)", spec: BAR_REVENUE_GOAL_SPEC },
    GalleryItem { name: "Stacked Bar", spec: STACKED_BAR_SPEC },
    GalleryItem { name: "Combo", spec: COMBO_SPEC },
    GalleryItem { name: "Line", spec: LINE_SPEC },
    GalleryItem { name: "Multi-Line", spec: MULTI_LINE_SPEC },
    GalleryItem { name: "Area", spec: AREA_SPEC },
    GalleryItem { name: "Pie", spec: PIE_SPEC },
    GalleryItem { name: "Doughnut", spec: DOUGHNUT_SPEC },
    GalleryItem { name: "Scatter", spec: SCATTER_SPEC },
    GalleryItem { name: "Bubble", spec: BUBBLE_SPEC },
    GalleryItem { name: "Metric", spec: METRIC_REVENUE_SPEC },
    GalleryItem { name: "Metric (Error Rate)", spec: METRIC_ERROR_RATE_SPEC },
    GalleryItem { name: "Bar (12 months)", spec: TWELVE_MONTHS_SPEC },
    GalleryItem { name: "Line (Daily Dates)", spec: DAILY_SALES_DATES_SPEC },
    GalleryItem { name: "Bar (Long Labels)", spec: LONG_CATEGORIES_SPEC },
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
