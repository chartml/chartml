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
    has_svg: bool,
    svg_content: Option<String>,
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

        // Read SVG and extract dimensions
        let (mut svg_width, mut svg_height) = (800u32, 400u32);
        let svg_content = if has_svg {
            fs::read_to_string(&svg_path).ok().inspect(|svg_text| {
                let head: String = svg_text.chars().take(500).collect();
                if let Some(w) = extract_attr(&head, "width") {
                    svg_width = w;
                }
                if let Some(h) = extract_attr(&head, "height") {
                    svg_height = h;
                }
            })
        } else {
            None
        };

        specs.push(SpecInfo {
            id,
            chart_type,
            title,
            svg_width,
            svg_height,
            has_svg,
            svg_content,
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

/// Build the full gallery HTML.
fn build_html(specs: &[SpecInfo]) -> String {
    // Group by type, preserving order
    let mut groups: BTreeMap<String, Vec<&SpecInfo>> = BTreeMap::new();
    for s in specs {
        groups.entry(s.chart_type.clone()).or_default().push(s);
    }

    let type_order = [
        "bar", "line", "area", "pie", "doughnut", "scatter", "bubble", "metric",
        "edge_cases", "sizing",
    ];

    let total = specs.len();
    let rendered = specs.iter().filter(|s| s.has_svg).count();

    let mut nav_html = String::new();
    let mut sections_html = String::new();

    // Ordered types first, then any extras
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
        // Title-case
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
            t = t,
            label = label,
            count = group.len(),
        ));

        let mut cards_html = String::new();
        for s in group {
            let chart_embed = if let Some(ref svg) = s.svg_content {
                // Inline the SVG so CSS animations and hover/tooltips work
                format!(
                    r#"<div class="chart-frame">{}</div>"#,
                    svg,
                )
            } else {
                r#"<div class="chart-frame no-render">Not rendered</div>"#.to_string()
            };

            let size_label = format!("{}x{}", s.svg_width, s.svg_height);
            // Escape HTML in title
            let safe_title = s.title.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");

            cards_html.push_str(&format!(
                r#"<div class="card" data-id="{id}">
                    <div class="card-header">
                        <span class="card-title">{title}</span>
                        <span class="card-actions"><button class="info-btn" onclick="showYaml('{id}');event.stopPropagation()">YAML</button><span class="card-size">{size}</span></span>
                    </div>
                    {chart}
                    <div class="card-footer"><code>{id}</code></div>
                </div>"#,
                id = s.id, title = safe_title, size = size_label, chart = chart_embed,
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
.chart-frame svg{{max-width:100%;height:auto}}
.chart-frame img{{max-width:100%;height:auto}}
.chart-frame.no-render{{color:#86868b;font-size:13px}}
.card-footer{{padding:8px 16px 12px}}
.card-footer code{{font-size:11px;color:#86868b}}
.card-actions{{display:flex;align-items:center;gap:8px}}
.info-btn{{padding:2px 8px;border:1px solid #d2d2d7;border-radius:4px;background:#fff;cursor:pointer;font-size:10px;font-weight:600;color:#0071e3;transition:all .15s}}
.info-btn:hover{{background:#0071e3;color:#fff}}
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
.lightbox-content{{background:#fff;border-radius:16px;max-width:95vw;max-height:95vh;overflow:auto;padding:24px}}
.lightbox-content h3{{margin-bottom:12px}}
.lightbox-content img{{max-width:100%;height:auto}}
.lightbox-close{{position:fixed;top:20px;right:24px;color:#fff;font-size:28px;cursor:pointer;z-index:1001;background:rgba(0,0,0,0.5);border-radius:50%;width:40px;height:40px;display:flex;align-items:center;justify-content:center}}
.lightbox-close:hover{{background:rgba(0,0,0,0.8)}}
@media(max-width:600px){{.grid{{grid-template-columns:1fr}}main{{padding:16px}}}}
</style>
</head>
<body>
<header>
<h1>ChartML Test Gallery</h1>
<div class="stats"><strong>{rendered}</strong> / {total} rendered</div>
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
<script>
// Search
const search=document.getElementById('search');
search.addEventListener('input',()=>{{
  const q=search.value.toLowerCase();
  document.querySelectorAll('.card').forEach(c=>{{
    const id=c.dataset.id.toLowerCase();
    const t=c.querySelector('.card-title').textContent.toLowerCase();
    c.classList.toggle('hidden',!id.includes(q)&&!t.includes(q));
  }});
  document.querySelectorAll('section').forEach(s=>{{
    s.classList.toggle('hidden',s.querySelectorAll('.card:not(.hidden)').length===0);
  }});
}});

// Lightbox — clone the card's inline SVG so animations replay
document.querySelectorAll('.card').forEach(c=>{{
  c.addEventListener('click',()=>{{
    const id=c.dataset.id;
    const t=c.querySelector('.card-title').textContent;
    const s=c.querySelector('.card-size').textContent;
    const svg=c.querySelector('svg');
    const lbc=document.getElementById('lightbox-content');
    lbc.innerHTML=`<h3>${{t}} <span style="color:#86868b;font-size:14px">${{s}}</span></h3><div id="lb-chart"></div><p style="margin-top:12px;color:#86868b;font-size:13px"><code>${{id}}</code></p>`;
    if(svg){{
      const clone=svg.cloneNode(true);
      clone.style.width='100%';
      clone.style.maxWidth=clone.getAttribute('width')+'px';
      clone.style.height='auto';
      document.getElementById('lb-chart').appendChild(clone);
    }}
    document.getElementById('lightbox').classList.add('active');
  }});
}});
function closeLightbox(){{document.getElementById('lightbox').classList.remove('active')}}
document.getElementById('lightbox').addEventListener('click',e=>{{if(e.target.id==='lightbox')closeLightbox()}});
document.addEventListener('keydown',e=>{{if(e.key==='Escape'){{closeLightbox();closeYaml()}}}});

// YAML modal — fetches and shows the spec YAML
async function showYaml(id){{
  const res=await fetch('/yaml/'+id);
  const yaml=await res.text();
  const m=document.getElementById('yaml-modal');
  document.getElementById('yaml-title').textContent=id;
  document.getElementById('yaml-pre').textContent=yaml;
  m.classList.add('active');
}}
function closeYaml(){{document.getElementById('yaml-modal').classList.remove('active')}}

// Tooltip — reads data-label/data-value/data-series from SVG elements
const tt=document.getElementById('chart-tooltip');
const ttLabel=document.getElementById('tt-label');
const ttValue=document.getElementById('tt-value');
document.addEventListener('mouseover',e=>{{
  const el=e.target.closest('[data-label]');
  if(el){{
    const series=el.getAttribute('data-series');
    const label=el.getAttribute('data-label');
    const value=el.getAttribute('data-value');
    ttLabel.textContent=(series?series+': ':'')+label;
    ttValue.textContent=value||'';
    tt.style.display='block';
  }}
}});
document.addEventListener('mouseout',e=>{{
  const el=e.target.closest('[data-label]');
  if(el)tt.style.display='none';
}});
document.addEventListener('mousemove',e=>{{
  if(tt.style.display==='block'){{
    tt.style.left=(e.clientX+12)+'px';
    tt.style.top=(e.clientY-12)+'px';
  }}
}});
</script>
</body>
</html>"##,
        chart_styles = chart_styles,
        total = total,
        rendered = rendered,
        nav = nav_html,
        sections = sections_html,
    )
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
        let html = build_html(&specs);
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
    let svg_count = specs.iter().filter(|s| s.has_svg).count();

    // Get local IP for display
    let local_ip = get_local_ip().unwrap_or_else(|| "localhost".to_string());

    println!("ChartML Test Gallery");
    println!("  Specs: {}  |  SVGs: {}", specs.len(), svg_count);
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
