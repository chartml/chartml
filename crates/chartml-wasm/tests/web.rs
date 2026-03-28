use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

use chartml_wasm::WasmChartML;

#[wasm_bindgen_test]
fn test_render_bar_chart() {
    let chartml = WasmChartML::new();
    let yaml = r#"
type: chart
version: 1
data:
  provider: inline
  rows:
    - month: Jan
      revenue: 100
    - month: Feb
      revenue: 200
visualize:
  type: bar
  columns: month
  rows: revenue
"#;
    let result = chartml.render_to_svg(yaml, JsValue::UNDEFINED);
    assert!(result.is_ok(), "render_to_svg failed: {:?}", result.err());
    let svg = result.unwrap();
    assert!(svg.contains("<svg"), "Expected SVG root element");
    assert!(svg.contains(r#"class="bar""#), "Expected bar rect elements");
}

#[wasm_bindgen_test]
fn test_render_line_chart() {
    let chartml = WasmChartML::new();
    let yaml = r#"
type: chart
version: 1
data:
  provider: inline
  rows:
    - week: W1
      users: 50
    - week: W2
      users: 80
    - week: W3
      users: 120
visualize:
  type: line
  columns: week
  rows: users
"#;
    let result = chartml.render_to_svg(yaml, JsValue::UNDEFINED);
    assert!(result.is_ok(), "Line chart render failed: {:?}", result.err());
    let svg = result.unwrap();
    assert!(svg.contains("<svg"), "Expected SVG root element");
    assert!(svg.contains(r#"class="line""#), "Expected line path element");
}

#[wasm_bindgen_test]
fn test_render_area_chart() {
    let chartml = WasmChartML::new();
    let yaml = r#"
type: chart
version: 1
data:
  provider: inline
  rows:
    - month: Jan
      sales: 100
    - month: Feb
      sales: 200
    - month: Mar
      sales: 150
visualize:
  type: area
  columns: month
  rows: sales
"#;
    let result = chartml.render_to_svg(yaml, JsValue::UNDEFINED);
    assert!(result.is_ok(), "Area chart render failed: {:?}", result.err());
    let svg = result.unwrap();
    assert!(svg.contains("<svg"), "Expected SVG root element");
    assert!(svg.contains(r#"class="area""#), "Expected area path element");
}

#[wasm_bindgen_test]
fn test_render_pie_chart() {
    let chartml = WasmChartML::new();
    let yaml = r#"
type: chart
version: 1
data:
  provider: inline
  rows:
    - category: A
      value: 30
    - category: B
      value: 70
visualize:
  type: pie
  columns: category
  rows: value
"#;
    let result = chartml.render_to_svg(yaml, JsValue::UNDEFINED);
    assert!(result.is_ok(), "Pie chart render failed: {:?}", result.err());
    let svg = result.unwrap();
    assert!(svg.contains("<svg"), "Expected SVG root element");
    // Pie slices are rendered as path elements with arc commands
    assert!(svg.contains("<path"), "Expected path elements for pie slices");
    assert!(svg.contains(" A"), "Expected arc (A) commands in pie paths");
}

#[wasm_bindgen_test]
fn test_render_doughnut_chart() {
    let chartml = WasmChartML::new();
    let yaml = r#"
type: chart
version: 1
data:
  provider: inline
  rows:
    - category: X
      value: 40
    - category: Y
      value: 60
visualize:
  type: doughnut
  columns: category
  rows: value
"#;
    let result = chartml.render_to_svg(yaml, JsValue::UNDEFINED);
    assert!(result.is_ok(), "Doughnut chart render failed: {:?}", result.err());
    let svg = result.unwrap();
    assert!(svg.contains("<svg"), "Expected SVG root element");
    assert!(svg.contains("<path"), "Expected path elements for doughnut slices");
}

#[wasm_bindgen_test]
fn test_render_scatter_chart() {
    let chartml = WasmChartML::new();
    let yaml = r#"
type: chart
version: 1
data:
  provider: inline
  rows:
    - x: 10
      y: 20
    - x: 30
      y: 40
visualize:
  type: scatter
  columns: x
  rows: y
"#;
    let result = chartml.render_to_svg(yaml, JsValue::UNDEFINED);
    assert!(result.is_ok(), "Scatter chart render failed: {:?}", result.err());
    let svg = result.unwrap();
    assert!(svg.contains("<circle"), "Expected circle elements for scatter points");
}

#[wasm_bindgen_test]
fn test_render_bubble_chart() {
    let chartml = WasmChartML::new();
    let yaml = r#"
type: chart
version: 1
data:
  provider: inline
  rows:
    - x: 10
      y: 20
      size: 5
    - x: 30
      y: 40
      size: 15
visualize:
  type: bubble
  columns: x
  rows: y
  marks:
    size: size
"#;
    let result = chartml.render_to_svg(yaml, JsValue::UNDEFINED);
    assert!(result.is_ok(), "Bubble chart render failed: {:?}", result.err());
    let svg = result.unwrap();
    assert!(svg.contains("<circle"), "Expected circle elements for bubble chart");
}

#[wasm_bindgen_test]
fn test_render_metric_card() {
    let chartml = WasmChartML::new();
    let yaml = r#"
type: chart
version: 1
data:
  provider: inline
  rows:
    - label: Revenue
      value: 1234567
visualize:
  type: metric
  columns: label
  value: value
"#;
    let result = chartml.render_to_svg(yaml, JsValue::UNDEFINED);
    assert!(result.is_ok(), "Metric card render failed: {:?}", result.err());
    let svg = result.unwrap();
    // Metric cards render as HTML (div/span) wrapped in SVG foreignObject
    assert!(svg.contains("1,234,567"),
        "Expected formatted metric value '1,234,567' in output");
}

#[wasm_bindgen_test]
fn test_render_to_element_returns_object() {
    let chartml = WasmChartML::new();
    let yaml = r#"
type: chart
version: 1
data:
  provider: inline
  rows:
    - x: A
      y: 10
visualize:
  type: bar
  columns: x
  rows: y
"#;
    let result = chartml.render_to_element(yaml, JsValue::UNDEFINED);
    assert!(result.is_ok(), "render_to_element failed: {:?}", result.err());
    let element = result.unwrap();
    assert!(!element.is_undefined(), "Element should not be undefined");
    assert!(!element.is_null(), "Element should not be null");
    // ChartElement is serialized as a JSON object with a "type" field
    assert!(element.is_object(), "Element should be a JS object");
}

#[wasm_bindgen_test]
fn test_render_with_size_options() {
    let chartml = WasmChartML::new();
    let yaml = r#"
type: chart
version: 1
data:
  provider: inline
  rows:
    - x: A
      y: 10
visualize:
  type: bar
  columns: x
  rows: y
"#;
    let options = js_sys::Object::new();
    js_sys::Reflect::set(&options, &"width".into(), &600.0.into()).unwrap();
    js_sys::Reflect::set(&options, &"height".into(), &300.0.into()).unwrap();

    let result = chartml.render_to_svg(yaml, options.into());
    assert!(result.is_ok(), "Render with options failed: {:?}", result.err());
    let svg = result.unwrap();
    // element_to_svg serializes f64 dimensions as integers when they have no fractional part
    assert!(svg.contains(r#"width="600""#), "Expected width=600 in SVG");
    assert!(svg.contains(r#"height="300""#), "Expected height=300 in SVG");
}

#[wasm_bindgen_test]
fn test_invalid_yaml_returns_error() {
    let chartml = WasmChartML::new();
    let result = chartml.render_to_svg("not: valid: chartml: spec", JsValue::UNDEFINED);
    assert!(result.is_err(), "Expected error for invalid YAML");
}

#[wasm_bindgen_test]
fn test_register_component() {
    let mut chartml = WasmChartML::new();
    let component_yaml = r#"
type: source
version: 1
name: test_data
provider: inline
rows:
  - x: A
    y: 10
  - x: B
    y: 20
"#;
    let result = chartml.register_component(component_yaml);
    assert!(result.is_ok(), "register_component failed: {:?}", result.err());
}
