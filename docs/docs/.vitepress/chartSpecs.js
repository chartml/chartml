// Chart specifications for live previews
export const barChartSpec = `data:
  - month: "Jan"
    revenue: 125000
  - month: "Feb"
    revenue: 138000
  - month: "Mar"
    revenue: 152000

visualize:
  type: bar
  columns: month
  rows: revenue
  style:
    title: Monthly Revenue
    width: 600
    height: 400`;

export const lineChartSpec = `data:
  - month: "Jan"
    customers: 450
  - month: "Feb"
    customers: 485
  - month: "Mar"
    customers: 520

visualize:
  type: line
  columns: month
  rows: customers
  style:
    title: New Customers
    width: 600
    height: 400`;

export const pieChartSpec = `data:
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
    title: Regional Breakdown
    width: 600
    height: 400`;

export const scatterPlotSpec = `data:
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
    color: category
  style:
    title: Marketing Budget vs Sales
    width: 600
    height: 400`;
