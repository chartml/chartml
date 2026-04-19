//! Built-in gallery server for the chartml test suite.
//!
//! Serves a visual gallery of all rendered chart SVGs, organized by chart type.
//! Auto-discovers specs from tests/charts/ and renders from test-output/all/.
//! Refreshing the page rescans the filesystem — no restart needed when adding specs.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

/// Metadata for a single test spec discovered on disk.
struct SpecInfo {
    id: String,       // e.g. "bar/basic_3_months"
    chart_type: String,
    title: String,
    svg_width: u32,
    svg_height: u32,
}

fn specs_dir() -> PathBuf {
    PathBuf::from("tests/charts")
}

fn output_dir() -> PathBuf {
    PathBuf::from("test-output/all")
}

/// Recursively collect all .yaml files under a directory.
fn collect_yamls(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_yamls(&p, &mut *out);
            } else if p.extension().map(|e| e == "yaml" || e == "yml").unwrap_or(false) {
                out.push(p);
            }
        }
    }
}

/// Discover all specs and match with rendered SVGs.
fn discover_specs() -> Vec<SpecInfo> {
    let specs_root = specs_dir();
    let out_root = output_dir();

    let mut yamls = Vec::new();
    collect_yamls(&specs_root, &mut yamls);
    yamls.sort();

    let mut specs = Vec::new();
    for yaml_path in &yamls {
        let rel = match yaml_path.strip_prefix(&specs_root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let chart_type = rel.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        let name = rel.file_stem().unwrap().to_string_lossy().to_string();
        let id = format!("{}/{}", chart_type, name);

        let svg_path = out_root.join(&chart_type).join(format!("{}.svg", name));
        let has_svg = svg_path.exists();

        // Extract title from YAML (simple line scan, no yaml parser needed)
        let yaml_text = fs::read_to_string(yaml_path).unwrap_or_default();
        let mut title = name.replace('_', " ");
        for line in yaml_text.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("name:") {
                title = rest.trim().trim_matches('"').trim_matches('\'').to_string();
                break;
            }
        }

        // Read SVG dimensions from pre-rendered output (if available)
        let (mut svg_width, mut svg_height) = (800u32, 400u32);
        if has_svg {
            if let Ok(svg_text) = fs::read_to_string(&svg_path) {
                let head: String = svg_text.chars().take(500).collect();
                if let Some(w) = extract_attr(&head, "width") {
                    svg_width = w;
                }
                if let Some(h) = extract_attr(&head, "height") {
                    svg_height = h;
                }
            }
        }

        specs.push(SpecInfo {
            id,
            chart_type,
            title,
            svg_width,
            svg_height,
        });
    }
    specs
}

/// Extract a numeric attribute from an SVG tag header, e.g. width="800" → 800.
fn extract_attr(svg_head: &str, attr: &str) -> Option<u32> {
    let needle = format!("{}=\"", attr);
    if let Some(start) = svg_head.find(&needle) {
        let after = &svg_head[start + needle.len()..];
        if let Some(end) = after.find('"') {
            return after[..end].parse::<f64>().ok().map(|v| v as u32);
        }
    }
    None
}

/// Extract the `chart:` sub-document from a test spec YAML.
/// Test specs wrap the ChartML spec under `chart:` alongside `name:`, `assertions:`, etc.
fn extract_chart_yaml(yaml_text: &str) -> String {
    let spec: serde_yaml::Value = match serde_yaml::from_str(yaml_text) {
        Ok(v) => v,
        Err(_) => return yaml_text.to_string(),
    };
    match spec.get("chart") {
        Some(chart_value) => serde_yaml::to_string(chart_value).unwrap_or_else(|_| yaml_text.to_string()),
        None => yaml_text.to_string(), // no wrapper, return as-is
    }
}

/// Read the chart animation/interaction CSS from the demo stylesheet.
fn chart_css() -> String {
    let css_path = Path::new("demo/style/main.css");
    if let Ok(full_css) = fs::read_to_string(css_path) {
        // Extract just the chart animation section (everything after the CHART ANIMATIONS comment)
        if let Some(start) = full_css.find("CHART ANIMATIONS") {
            // Go back to find the start of the comment block
            let section_start = full_css[..start].rfind("/*").unwrap_or(start);
            return full_css[section_start..].to_string();
        }
        full_css
    } else {
        String::new()
    }
}

/// Build the gallery HTML — renders all charts client-side using WasmChartML.
fn build_wasm_html(specs: &[SpecInfo]) -> String {
    // Group by type, preserving order
    let mut groups: BTreeMap<String, Vec<&SpecInfo>> = BTreeMap::new();
    for s in specs {
        groups.entry(s.chart_type.clone()).or_default().push(s);
    }

    let type_order = [
        "bar", "line", "area", "pie", "doughnut", "scatter", "bubble", "metric",
        "table", "edge_cases", "sizing",
    ];

    let total = specs.len();

    let mut nav_html = String::new();
    let mut sections_html = String::new();

    let mut seen = std::collections::HashSet::new();
    let ordered: Vec<String> = type_order
        .iter()
        .map(|s| s.to_string())
        .chain(groups.keys().cloned())
        .filter(|t| seen.insert(t.clone()))
        .collect();

    for t in &ordered {
        let group = match groups.get(t.as_str()) {
            Some(g) => g,
            None => continue,
        };
        let label = t.replace('_', " ");
        let label: String = label
            .split_whitespace()
            .map(|w| {
                let mut c = w.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().to_string() + c.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        nav_html.push_str(&format!(
            "<a href=\"#{t}\" class=\"nav-item\">{label} <span class=\"count\">{count}</span></a>",
            t = t, label = label, count = group.len(),
        ));

        let mut cards_html = String::new();
        for s in group {
            let safe_title = s.title.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
            let size_label = format!("{}x{}", s.svg_width, s.svg_height);

            cards_html.push_str(&format!(
                r#"<div class="card" data-id="{id}" data-width="{w}" data-height="{h}">
                    <div class="card-header">
                        <span class="card-title">{title}</span>
                        <span class="card-actions"><button class="info-btn" onclick="showYaml('{id}');event.stopPropagation()">YAML</button><span class="card-size">{size}</span></span>
                    </div>
                    <div class="chart-frame" id="chart-{escaped_id}">
                        <div class="loading-spinner"></div>
                    </div>
                    <div class="card-footer"><code>{id}</code></div>
                </div>"#,
                id = s.id,
                escaped_id = s.id.replace('/', "-"),
                title = safe_title,
                size = size_label,
                w = s.svg_width,
                h = s.svg_height,
            ));
        }

        sections_html.push_str(&format!(
            r#"<section id="{t}">
                <h2>{label} <span class="section-count">({count})</span></h2>
                <div class="grid">{cards}</div>
            </section>"#,
            t = t, label = label, count = group.len(), cards = cards_html,
        ));
    }

    let chart_styles = chart_css();

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>ChartML Test Gallery — {total} specs</title>
<style>
/* ── Chart animations & interactions ── */
{chart_styles}
</style>
<style>
*{{margin:0;padding:0;box-sizing:border-box}}
body{{font-family:Inter,-apple-system,system-ui,sans-serif;background:#f5f5f7;color:#1d1d1f}}
header{{background:#1d1d1f;color:#f5f5f7;padding:20px 32px;display:flex;align-items:center;justify-content:space-between;position:sticky;top:0;z-index:100}}
header h1{{font-size:18px;font-weight:600}}
header .stats{{font-size:13px;color:#86868b}}
header .stats strong{{color:#30d158}}
.toolbar{{background:#fff;border-bottom:1px solid #d2d2d7;padding:12px 32px;display:flex;gap:12px;align-items:center;position:sticky;top:58px;z-index:99}}
.toolbar input{{flex:1;max-width:400px;padding:8px 12px;border:1px solid #d2d2d7;border-radius:8px;font-size:14px;outline:none}}
.toolbar input:focus{{border-color:#0071e3;box-shadow:0 0 0 3px rgba(0,113,227,0.15)}}
nav{{display:flex;gap:4px;padding:12px 32px;background:#fff;border-bottom:1px solid #e5e5e5;flex-wrap:wrap}}
.nav-item{{padding:6px 12px;border-radius:6px;text-decoration:none;color:#1d1d1f;font-size:13px;font-weight:500;transition:background .15s}}
.nav-item:hover{{background:#e8e8ed}}
.nav-item .count{{color:#86868b;font-weight:400}}
main{{padding:24px 32px;max-width:1600px;margin:0 auto}}
section{{margin-bottom:40px}}
section h2{{font-size:20px;font-weight:600;margin-bottom:16px;padding-top:8px}}
section h2 .section-count{{color:#86868b;font-weight:400;font-size:16px}}
.grid{{display:grid;grid-template-columns:repeat(auto-fill,minmax(400px,1fr));gap:16px}}
.card{{background:#fff;border-radius:12px;overflow:hidden;box-shadow:0 1px 3px rgba(0,0,0,0.08);transition:box-shadow .2s,transform .2s;cursor:pointer}}
.card:hover{{box-shadow:0 4px 12px rgba(0,0,0,0.12);transform:translateY(-2px)}}
.card-header{{padding:12px 16px 8px;display:flex;justify-content:space-between;align-items:baseline}}
.card-title{{font-size:13px;font-weight:600}}
.card-size{{font-size:11px;color:#86868b;font-family:monospace}}
.chart-frame{{display:flex;align-items:center;justify-content:center;padding:8px 12px;background:#fafafa;min-height:80px;overflow:hidden}}
.chart-frame svg{{width:100%;height:auto}}
.chart-frame.no-render{{color:#86868b;font-size:13px}}
.chart-frame.error{{color:#ff3b30;font-size:12px;font-family:monospace;white-space:pre-wrap;word-break:break-word;padding:12px;text-align:left}}
.card-footer{{padding:8px 16px 12px}}
.card-footer code{{font-size:11px;color:#86868b}}
.card-actions{{display:flex;align-items:center;gap:8px}}
.info-btn{{padding:2px 8px;border:1px solid #d2d2d7;border-radius:4px;background:#fff;cursor:pointer;font-size:10px;font-weight:600;color:#0071e3;transition:all .15s}}
.info-btn:hover{{background:#0071e3;color:#fff}}
.loading-spinner{{width:24px;height:24px;border:3px solid #e5e5e5;border-top-color:#0071e3;border-radius:50%;animation:spin 0.8s linear infinite}}
@keyframes spin{{to{{transform:rotate(360deg)}}}}
.yaml-modal{{display:none;position:fixed;inset:0;background:rgba(0,0,0,0.85);z-index:2000;align-items:center;justify-content:center;padding:40px}}
.yaml-modal.active{{display:flex}}
.yaml-modal-content{{background:#1e1e1e;border-radius:12px;max-width:800px;width:90vw;max-height:90vh;overflow:auto;padding:0}}
.yaml-modal-header{{display:flex;justify-content:space-between;align-items:center;padding:16px 20px;border-bottom:1px solid #333;position:sticky;top:0;background:#1e1e1e;z-index:1}}
.yaml-modal-header h3{{color:#e5e5e5;font-size:14px;margin:0}}
.yaml-modal-close{{color:#999;font-size:24px;cursor:pointer;background:none;border:none;padding:0 4px}}
.yaml-modal-close:hover{{color:#fff}}
.yaml-modal pre{{margin:0;padding:20px;color:#d4d4d4;font-family:'SF Mono',Menlo,Consolas,monospace;font-size:12px;line-height:1.5;white-space:pre;overflow-x:auto}}
.hidden{{display:none!important}}
.lightbox{{display:none;position:fixed;inset:0;background:rgba(0,0,0,0.85);z-index:1000;align-items:center;justify-content:center;padding:40px}}
.lightbox.active{{display:flex}}
.lightbox-content{{background:#fff;border-radius:16px;width:90vw;max-width:1400px;max-height:95vh;overflow:auto;padding:24px}}
.lightbox-content h3{{margin-bottom:12px}}
.lightbox-close{{position:fixed;top:20px;right:24px;color:#fff;font-size:28px;cursor:pointer;z-index:1001;background:rgba(0,0,0,0.5);border-radius:50%;width:40px;height:40px;display:flex;align-items:center;justify-content:center}}
.lightbox-close:hover{{background:rgba(0,0,0,0.8)}}
@media(max-width:600px){{.grid{{grid-template-columns:1fr}}main{{padding:16px}}}}
</style>
</head>
<body>
<header>
<h1>ChartML Test Gallery</h1>
<div class="stats"><strong id="rendered-count">0</strong> / {total} rendered</div>
</header>
<div class="toolbar">
<input type="text" id="search" placeholder="Search specs..." autofocus />
</div>
<nav>{nav}</nav>
<main>{sections}</main>
<div class="lightbox" id="lightbox">
<div class="lightbox-close" onclick="closeLightbox()">&times;</div>
<div class="lightbox-content" id="lightbox-content"></div>
</div>
<div class="yaml-modal" id="yaml-modal" onclick="if(event.target===this)closeYaml()">
<div class="yaml-modal-content">
<div class="yaml-modal-header"><h3 id="yaml-title"></h3><button class="yaml-modal-close" onclick="closeYaml()">&times;</button></div>
<pre id="yaml-pre"></pre>
</div>
</div>
<div id="chart-tooltip" class="chartml-tooltip" style="display:none;position:fixed;z-index:2000;">
  <div class="chartml-tooltip-label" id="tt-label"></div>
  <div class="chartml-tooltip-value" id="tt-value"></div>
</div>
<script type="module">
import init, {{ WasmChartML }} from '/pkg/web/chartml_wasm.js';
import initDf, {{ transform as dfTransform }} from '/pkg-datafusion/web/chartml_wasm_datafusion.js';

let renderedCount = 0;
const totalCount = {total};

function updateCount() {{
  document.getElementById('rendered-count').textContent = renderedCount;
}}

// Cache fetched YAML so re-renders on resize don't re-fetch
const yamlCache = new Map();

async function getYaml(id) {{
  if (yamlCache.has(id)) return yamlCache.get(id);
  const res = await fetch('/chart-yaml/' + id);
  if (!res.ok) throw new Error('YAML fetch failed: ' + res.status);
  const yaml = await res.text();
  yamlCache.set(id, yaml);
  return yaml;
}}

// Async render path — required so the registered TransformMiddleware (DataFusion)
// drives `transform.sql` / `transform.forecast` specs. The sync `renderToSvg`
// errors on WASM whenever middleware is needed.
async function renderChartAtSize(frame, yaml, width, aspectRatio, chartml) {{
  const w = Math.round(width);
  const h = Math.round(w * aspectRatio);
  if (w <= 0) return;
  const svg = await chartml.renderToSvgAsync(yaml, {{ width: w, height: h }});
  frame.innerHTML = svg;
}}

async function renderChart(card, chartml) {{
  const id = card.dataset.id;
  const specW = parseInt(card.dataset.width) || 800;
  const specH = parseInt(card.dataset.height) || 400;
  const aspectRatio = specH / specW;
  const frameId = 'chart-' + id.replace('/', '-');
  const frame = document.getElementById(frameId);
  if (!frame) return;

  try {{
    const yaml = await getYaml(id);

    // Initial render at container width
    const containerWidth = frame.clientWidth - 24; // subtract padding
    const w = containerWidth > 0 ? containerWidth : specW;
    await renderChartAtSize(frame, yaml, w, aspectRatio, chartml);
    renderedCount++;
    updateCount();

    // Store render info for ResizeObserver
    frame._chartYaml = yaml;
    frame._aspectRatio = aspectRatio;
    frame._lastWidth = w;
  }} catch (err) {{
    frame.classList.add('error');
    frame.textContent = err.message || String(err);
  }}
}}

async function main() {{
  // Init both WASM modules in parallel — the .wasm fetches are independent.
  await Promise.all([
    init('/pkg/wasm/chartml_wasm_bg.wasm'),
    initDf('/pkg-datafusion/wasm/chartml_wasm_datafusion_bg.wasm'),
  ]);
  const chartml = new WasmChartML();
  // Wire the DataFusion transform BEFORE any render call — the WasmChartML
  // `register*` methods require unique access via `Rc::get_mut`, so they
  // must run before any in-flight async pipeline holds an `Rc` clone.
  chartml.registerTransform(dfTransform);

  // Render charts in batches to avoid blocking the UI
  const cards = Array.from(document.querySelectorAll('.card[data-id]'));
  const BATCH = 8;
  for (let i = 0; i < cards.length; i += BATCH) {{
    const batch = cards.slice(i, i + BATCH);
    await Promise.all(batch.map(c => renderChart(c, chartml)));
  }}

  // Re-render charts on container resize
  let resizeTimeout;
  const observer = new ResizeObserver(entries => {{
    clearTimeout(resizeTimeout);
    resizeTimeout = setTimeout(() => {{
      for (const entry of entries) {{
        const frame = entry.target;
        if (!frame._chartYaml) continue;
        const newWidth = Math.round(entry.contentRect.width - 24);
        if (newWidth > 0 && Math.abs(newWidth - frame._lastWidth) > 5) {{
          renderChartAtSize(frame, frame._chartYaml, newWidth, frame._aspectRatio, chartml)
            .then(() => {{ frame._lastWidth = newWidth; }})
            .catch((err) => {{ console.warn('Chart resize render failed:', err); }});
        }}
      }}
    }}, 200);
  }});

  document.querySelectorAll('.chart-frame').forEach(f => observer.observe(f));

  // Expose chartml for lightbox re-rendering
  window._chartml = chartml;
}}

main().catch(err => console.error('WASM init failed:', err));

// Search
const search = document.getElementById('search');
search.addEventListener('input', () => {{
  const q = search.value.toLowerCase();
  document.querySelectorAll('.card').forEach(c => {{
    const id = c.dataset.id.toLowerCase();
    const t = c.querySelector('.card-title').textContent.toLowerCase();
    c.classList.toggle('hidden', !id.includes(q) && !t.includes(q));
  }});
  document.querySelectorAll('section').forEach(s => {{
    s.classList.toggle('hidden', s.querySelectorAll('.card:not(.hidden)').length === 0);
  }});
}});

// Lightbox — re-render chart at full lightbox width
document.querySelectorAll('.card').forEach(c => {{
  c.addEventListener('click', async () => {{
    const id = c.dataset.id;
    const t = c.querySelector('.card-title').textContent;
    const s = c.querySelector('.card-size').textContent;
    const specW = parseInt(c.dataset.width) || 800;
    const specH = parseInt(c.dataset.height) || 400;
    const aspectRatio = specH / specW;
    const lbc = document.getElementById('lightbox-content');
    lbc.innerHTML = `<h3>${{t}} <span style="color:#86868b;font-size:14px">${{s}}</span></h3><div id="lb-chart"></div><p style="margin-top:12px;color:#86868b;font-size:13px"><code>${{id}}</code></p>`;

    const chartml = window._chartml;
    if (chartml) {{
      try {{
        const yaml = await getYaml(id);
        const lbChart = document.getElementById('lb-chart');
        // Use the lightbox content width (90vw, max 800px from CSS)
        const lbWidth = lbc.clientWidth - 48; // subtract padding
        const w = Math.max(lbWidth, specW);
        const h = Math.round(w * aspectRatio);
        const svg = await chartml.renderToSvgAsync(yaml, {{ width: w, height: h }});
        lbChart.innerHTML = svg;
      }} catch (err) {{
        document.getElementById('lb-chart').textContent = 'Render error: ' + err.message;
      }}
    }}
    document.getElementById('lightbox').classList.add('active');
  }});
}});
function closeLightbox() {{ document.getElementById('lightbox').classList.remove('active') }}
document.getElementById('lightbox').addEventListener('click', e => {{ if (e.target.id === 'lightbox') closeLightbox() }});
document.addEventListener('keydown', e => {{ if (e.key === 'Escape') {{ closeLightbox(); closeYaml() }} }});

// YAML modal
async function showYaml(id) {{
  const res = await fetch('/yaml/' + id);
  const yaml = await res.text();
  document.getElementById('yaml-title').textContent = id;
  document.getElementById('yaml-pre').textContent = yaml;
  document.getElementById('yaml-modal').classList.add('active');
}}
window.showYaml = showYaml;
function closeYaml() {{ document.getElementById('yaml-modal').classList.remove('active') }}
window.closeYaml = closeYaml;

// Tooltip
const tt = document.getElementById('chart-tooltip');
const ttLabel = document.getElementById('tt-label');
const ttValue = document.getElementById('tt-value');
document.addEventListener('mouseover', e => {{
  const el = e.target.closest('[data-label]');
  if (el) {{
    const series = el.getAttribute('data-series');
    const label = el.getAttribute('data-label');
    const value = el.getAttribute('data-value');
    ttLabel.textContent = (series ? series + ': ' : '') + label;
    ttValue.textContent = value || '';
    tt.style.display = 'block';
  }}
}});
document.addEventListener('mouseout', e => {{
  const el = e.target.closest('[data-label]');
  if (el) tt.style.display = 'none';
}});
document.addEventListener('mousemove', e => {{
  if (tt.style.display === 'block') {{
    tt.style.left = (e.clientX + 12) + 'px';
    tt.style.top = (e.clientY - 12) + 'px';
  }}
}});
</script>
</body>
</html>"##,
        chart_styles = chart_styles,
        total = total,
        nav = nav_html,
        sections = sections_html,
    )
}

/// Serve a static file from a wasm-pack output directory (e.g. `packages/core/pkg`
/// or `packages/datafusion/pkg`). Rejects path traversal, picks the JS/WASM/JSON
/// content type from the extension, and 404s on missing files.
fn serve_pkg_file(stream: &mut TcpStream, pkg_dir: &Path, file_name: &str) {
    if file_name.contains("..") {
        let _ = stream.write_all(b"HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n");
        return;
    }
    let file_path = pkg_dir.join(file_name);
    let Ok(data) = fs::read(&file_path) else {
        let _ = stream
            .write_all(b"HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\nPkg file not found");
        return;
    };
    let content_type = match file_path.extension().and_then(|e| e.to_str()) {
        Some("js") => "application/javascript",
        Some("wasm") => "application/wasm",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    };
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
        content_type,
        data.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.write_all(&data);
}

/// Handle a single HTTP request.
fn handle_request(mut stream: TcpStream) {
    let mut buf = [0u8; 4096];
    let n = match stream.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return,
    };
    let request = String::from_utf8_lossy(&buf[..n]);

    // Parse the request line: "GET /path HTTP/1.1"
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");

    // Strip query string
    let path = path.split('?').next().unwrap_or(path);

    if path == "/" || path == "/index.html" {
        let specs = discover_specs();
        let html = build_wasm_html(&specs);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
            html.len()
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.write_all(html.as_bytes());
    } else if let Some(spec_id) = path.strip_prefix("/yaml/") {
        let yaml_path = specs_dir().join(format!("{}.yaml", spec_id));
        if let Ok(yaml_data) = fs::read(&yaml_path) {
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
                yaml_data.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(&yaml_data);
        } else {
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\nYAML not found");
        }
    } else if let Some(spec_id) = path.strip_prefix("/svg/") {
        let svg_path = output_dir().join(format!("{}.svg", spec_id));
        if let Ok(svg_data) = fs::read(&svg_path) {
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: image/svg+xml\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
                svg_data.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(&svg_data);
        } else {
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\nSVG not found");
        }
    } else if let Some(spec_id) = path.strip_prefix("/chart-yaml/") {
        if spec_id.contains("..") {
            let _ = stream.write_all(b"HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n");
            return;
        }
        // Return just the chart YAML (extracted from the test spec wrapper)
        let yaml_path = specs_dir().join(format!("{}.yaml", spec_id));
        match fs::read_to_string(&yaml_path) {
            Ok(yaml_text) => {
                let chart_yaml = extract_chart_yaml(&yaml_text);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
                    chart_yaml.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.write_all(chart_yaml.as_bytes());
            }
            Err(_) => {
                let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\nYAML not found");
            }
        }
    } else if path == "/specs" {
        let specs = discover_specs();
        let ids: Vec<&str> = specs.iter().map(|s| s.id.as_str()).collect();
        let json = serde_json::to_string(&ids).unwrap_or_else(|_| "[]".to_string());
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
            json.len()
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.write_all(json.as_bytes());
    } else if let Some(file_name) = path.strip_prefix("/pkg/") {
        serve_pkg_file(&mut stream, &PathBuf::from("packages/core/pkg"), file_name);
    } else if let Some(file_name) = path.strip_prefix("/pkg-datafusion/") {
        // Mirrors the `/pkg/` handler but serves the DataFusion WASM bundle so
        // gallery JS can `import` it and register the SQL transform middleware.
        serve_pkg_file(&mut stream, &PathBuf::from("packages/datafusion/pkg"), file_name);
    } else {
        let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\nNot found");
    }
}

/// Start the gallery HTTP server, binding on all interfaces.
pub fn serve(port: u16) {
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).unwrap_or_else(|e| {
        eprintln!("Failed to bind to {}: {}", addr, e);
        std::process::exit(1);
    });

    let specs = discover_specs();

    // Get local IP for display
    let local_ip = get_local_ip().unwrap_or_else(|| "localhost".to_string());

    println!("ChartML Test Gallery (WASM browser rendering)");
    println!("  Specs: {}", specs.len());
    println!();
    println!("  Local:   http://localhost:{}", port);
    println!("  Network: http://{}:{}", local_ip, port);
    println!();
    println!("  Press Ctrl+C to stop");

    for stream in listener.incoming().flatten() {
        handle_request(stream);
    }
}

/// Best-effort local IP detection for display.
fn get_local_ip() -> Option<String> {
    use std::net::UdpSocket;
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|a| a.ip().to_string())
}
