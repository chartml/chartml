use std::sync::Arc;
use leptos::prelude::*;
use chartml_core::ChartML;
use chartml_leptos::ChartMLChart;

/// A single example chart with title and description.
#[component]
fn ExampleChart(
    title: &'static str,
    description: &'static str,
    spec: &'static str,
    chartml: Arc<ChartML>,
) -> impl IntoView {
    let yaml = signal(spec.to_string());
    view! {
        <div class="example-chart">
            <h3 class="example-title">{title}</h3>
            <p class="example-desc">{description}</p>
            <div class="example-render">
                <ChartMLChart spec=yaml.0 chartml=chartml />
            </div>
        </div>
    }
}

/// A row of metric cards.
#[component]
fn MetricRow(
    specs: Vec<&'static str>,
    chartml: Arc<ChartML>,
) -> impl IntoView {
    view! {
        <div class="metric-row">
            {specs.into_iter().map(|spec| {
                let yaml = signal(spec.to_string());
                let chartml = chartml.clone();
                view! {
                    <div class="metric-cell">
                        <ChartMLChart spec=yaml.0 chartml=chartml />
                    </div>
                }
            }).collect::<Vec<_>>()}
        </div>
    }
}

/// Full examples page — mirrors the JS chartml docs/examples.md layout.
#[component]
pub fn ExamplesPage(chartml: Arc<ChartML>) -> impl IntoView {
    view! {
        <div class="examples-page">

            // ─── Section: Reusable Styles ───
            <section class="examples-section">
                <h2>"Reusable Styles and Configuration"</h2>
                <p class="section-desc">"Charts inherit themes automatically. These charts use the corporate_theme style."</p>

                <div class="examples-grid two-col">
                    <ExampleChart
                        title="Monthly Revenue"
                        description="Bar chart using default theme"
                        spec=MONTHLY_REVENUE
                        chartml=chartml.clone()
                    />
                    <ExampleChart
                        title="New Customers"
                        description="Line chart sharing the same theme"
                        spec=NEW_CUSTOMERS
                        chartml=chartml.clone()
                    />
                </div>

                <ExampleChart
                    title="Regional Breakdown"
                    description="Pie chart with selective style overrides"
                    spec=REGIONAL_PIE
                    chartml=chartml.clone()
                />
            </section>

            // ─── Section: KPI Overview ───
            <section class="examples-section">
                <h2>"KPI Overview: Executive Metrics Dashboard"</h2>
                <p class="section-desc">"Metric cards showing key performance indicators with trend comparisons."</p>

                <MetricRow
                    specs=vec![METRIC_REVENUE, METRIC_USERS, METRIC_CONVERSION, METRIC_AOV]
                    chartml=chartml.clone()
                />

                <ExampleChart
                    title="Error Rate (Inverted Trend)"
                    description="A decrease in error rate is shown as positive (green) because invertTrend is true."
                    spec=METRIC_ERROR_RATE
                    chartml=chartml.clone()
                />
            </section>

            // ─── Section: Reference Lines ───
            <section class="examples-section">
                <h2>"Reference Lines & Bands"</h2>
                <p class="section-desc">"Revenue tracking with goal markers. (Annotations are v0.2 — shown as plain charts for now.)"</p>

                <ExampleChart
                    title="Monthly Revenue vs Goal"
                    description="6-month revenue trend"
                    spec=REVENUE_GOAL
                    chartml=chartml.clone()
                />
            </section>

            // ─── Section: Advanced Analytics ───
            <section class="examples-section">
                <h2>"Dashboard: Advanced Analytics"</h2>
                <p class="section-desc">"Combo charts, multi-line trends, area charts, and scatter plots."</p>

                <div class="examples-grid two-col">
                    <ExampleChart
                        title="Actual Revenue vs Target"
                        description="Combo chart: bars for actuals, line for targets"
                        spec=COMBO_ACTUAL_TARGET
                        chartml=chartml.clone()
                    />
                    <ExampleChart
                        title="Regional Revenue Trends"
                        description="Multi-line chart showing weekly revenue per region"
                        spec=MULTI_LINE_REGIONAL
                        chartml=chartml.clone()
                    />
                </div>

                <div class="examples-grid two-col">
                    <ExampleChart
                        title="Cumulative Revenue Growth"
                        description="Area chart showing total revenue over time"
                        spec=AREA_CUMULATIVE
                        chartml=chartml.clone()
                    />
                    <ExampleChart
                        title="Revenue Composition by Month"
                        description="Stacked bar showing product line breakdown"
                        spec=STACKED_BAR_COMPOSITION
                        chartml=chartml.clone()
                    />
                </div>
            </section>

            // ─── Section: Scatter & Bubble ───
            <section class="examples-section">
                <h2>"Scatter & Bubble Charts"</h2>

                <div class="examples-grid two-col">
                    <ExampleChart
                        title="Marketing Budget vs Sales"
                        description="Scatter plot with color encoding by category"
                        spec=SCATTER_BUDGET
                        chartml=chartml.clone()
                    />
                    <ExampleChart
                        title="Revenue Efficiency Analysis"
                        description="Bubble chart with color and size encoding"
                        spec=BUBBLE_EFFICIENCY
                        chartml=chartml.clone()
                    />
                </div>
            </section>

            // ─── Section: Pie & Doughnut ───
            <section class="examples-section">
                <h2>"Pie & Doughnut Charts"</h2>

                <div class="examples-grid two-col">
                    <ExampleChart
                        title="Regional Breakdown"
                        description="Standard pie chart"
                        spec=REGIONAL_PIE
                        chartml=chartml.clone()
                    />
                    <ExampleChart
                        title="Revenue Distribution"
                        description="Doughnut chart with custom color palette"
                        spec=DOUGHNUT_DISTRIBUTION
                        chartml=chartml.clone()
                    />
                </div>
            </section>

        </div>
    }
}

// ─── All specs from chartml JS examples.md ───

const MONTHLY_REVENUE: &str = r##"type: chart
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

const NEW_CUSTOMERS: &str = r##"type: chart
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

const REGIONAL_PIE: &str = r##"type: chart
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
  rows: revenue
  style:
    height: 300"##;

const METRIC_REVENUE: &str = r##"type: chart
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

const METRIC_USERS: &str = r##"type: chart
version: 1
title: "Active Users"
data:
  provider: inline
  rows:
    - { current: 8432, previous: 8100 }
visualize:
  type: metric
  value: current
  format: ",.0f"
  compareWith: previous"##;

const METRIC_CONVERSION: &str = r##"type: chart
version: 1
title: "Conversion Rate"
data:
  provider: inline
  rows:
    - { current: 0.042, previous: 0.038 }
visualize:
  type: metric
  value: current
  format: ".1%"
  compareWith: previous"##;

const METRIC_AOV: &str = r##"type: chart
version: 1
title: "Avg Order Value"
data:
  provider: inline
  rows:
    - { current: 156.78, previous: 142.50 }
visualize:
  type: metric
  value: current
  format: "$,.2f"
  compareWith: previous"##;

const METRIC_ERROR_RATE: &str = r##"type: chart
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

const REVENUE_GOAL: &str = r##"type: chart
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

const COMBO_ACTUAL_TARGET: &str = r##"type: chart
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

const MULTI_LINE_REGIONAL: &str = r##"type: chart
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

const AREA_CUMULATIVE: &str = r##"type: chart
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

const STACKED_BAR_COMPOSITION: &str = r##"type: chart
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

const SCATTER_BUDGET: &str = r##"type: chart
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

const BUBBLE_EFFICIENCY: &str = r##"type: chart
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

const DOUGHNUT_DISTRIBUTION: &str = r##"type: chart
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
