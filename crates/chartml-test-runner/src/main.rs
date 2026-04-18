//! chartml-test: Renders a ChartML YAML spec and dumps the element tree + SVG.
//!
//! Usage:
//!   chartml-test <spec.yaml> [--output-dir <dir>]
//!   chartml-test --batch <dir-of-specs> [--output-dir <dir>]
//!   chartml-test --gallery [--port <port>]
//!   chartml-test --accept                         # save current output as golden baseline
//!   chartml-test --diff                            # compare current output against golden

mod diff;
mod gallery;
mod tree;

use chartml_core::ChartML;
use chartml_chart_cartesian::CartesianRenderer;
use chartml_chart_pie::PieRenderer;
use chartml_chart_scatter::ScatterRenderer;
use chartml_chart_metric::MetricRenderer;
use chartml_chart_table::TableRenderer;
use chartml_datafusion::DataFusionTransform;
use chartml_render::element_to_svg;
use std::path::{Path, PathBuf};
use std::fs;

fn create_chartml() -> ChartML {
    let mut c = ChartML::new();
    c.register_renderer("bar", CartesianRenderer::new());
    c.register_renderer("line", CartesianRenderer::new());
    c.register_renderer("area", CartesianRenderer::new());
    c.register_renderer("pie", PieRenderer::new());
    c.register_renderer("doughnut", PieRenderer::new());
    c.register_renderer("scatter", ScatterRenderer::new());
    c.register_renderer("bubble", ScatterRenderer::new());
    c.register_renderer("metric", MetricRenderer::new());
    c.register_renderer("table", TableRenderer::new());
    c.register_transform(DataFusionTransform);
    c
}

/// Compute the output prefix for a spec file.
/// In batch mode, uses `<parent_dir>/<stem>` relative to the batch root to avoid collisions.
/// In single mode, uses just the stem.
fn output_prefix(yaml_path: &Path, batch_root: Option<&Path>) -> String {
    let stem = yaml_path.file_stem().unwrap().to_string_lossy();
    if let Some(root) = batch_root {
        // Get the relative path from batch root, then use parent dir + stem
        if let Ok(relative) = yaml_path.strip_prefix(root) {
            if let Some(parent) = relative.parent() {
                let parent_str = parent.to_string_lossy();
                if !parent_str.is_empty() {
                    return format!("{}/{}", parent_str, stem);
                }
            }
        }
    }
    stem.to_string()
}

fn render_spec(yaml_path: &Path, output_dir: &Path, batch_root: Option<&Path>) -> Result<RenderResult, String> {
    let yaml = fs::read_to_string(yaml_path)
        .map_err(|e| format!("Failed to read {}: {}", yaml_path.display(), e))?;

    // Parse the test spec to extract just the chart YAML
    let spec: serde_yaml::Value = serde_yaml::from_str(&yaml)
        .map_err(|e| format!("Failed to parse YAML {}: {}", yaml_path.display(), e))?;

    // The chart YAML is under the "chart" key in the test spec
    let chart_value = spec.get("chart")
        .ok_or_else(|| format!("{}: missing 'chart' key in test spec", yaml_path.display()))?;

    let chart_yaml = serde_yaml::to_string(chart_value)
        .map_err(|e| format!("Failed to re-serialize chart YAML: {}", e))?;

    // Check if this spec expects an error
    let expect_error = spec.get("assertions")
        .and_then(|a| a.get("expect_error"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let chartml = create_chartml();
    let render_result = chartml.render_from_yaml(&chart_yaml);

    let prefix = output_prefix(yaml_path, batch_root);

    if expect_error {
        // Ensure output subdirectory exists
        let output_subdir = output_dir.join(&prefix).parent().map(|p| p.to_path_buf());
        if let Some(ref subdir) = output_subdir {
            let _ = fs::create_dir_all(subdir);
        }
        let result_path = output_dir.join(format!("{}.result.json", prefix));
        return match render_result {
            Err(e) => {
                let result_meta = serde_json::json!({
                    "spec": prefix,
                    "render_status": "expected_error",
                    "render_error": e.to_string(),
                    "eval_status": "pass",
                    "classification": null,
                    "failures": [],
                    "fix_required": null,
                    "last_evaluated": null
                });
                let _ = fs::write(&result_path, serde_json::to_string_pretty(&result_meta).unwrap());
                Ok(RenderResult {
                    spec_name: prefix,
                    tree_path: PathBuf::new(),
                    svg_path: PathBuf::new(),
                    assertions_path: None,
                    expected_error: Some(e.to_string()),
                })
            }
            Ok(_) => Err(format!("{}: expected error but render succeeded", yaml_path.display())),
        };
    }

    let element = render_result
        .map_err(|e| format!("Render failed for {}: {}", yaml_path.display(), e))?;

    // Ensure output subdirectory exists (for batch mode with type/ prefixes)
    let output_subdir = output_dir.join(&prefix).parent().map(|p| p.to_path_buf());
    if let Some(ref subdir) = output_subdir {
        fs::create_dir_all(subdir)
            .map_err(|e| format!("Failed to create output subdir: {}", e))?;
    }

    // Write element tree as JSON
    let tree_json = tree::element_to_json(&element);
    let tree_path = output_dir.join(format!("{}.tree.json", prefix));
    fs::write(&tree_path, serde_json::to_string_pretty(&tree_json).unwrap())
        .map_err(|e| format!("Failed to write tree: {}", e))?;

    // Write SVG
    let (width, height) = tree::extract_dimensions(&element);
    let svg = element_to_svg(&element, width, height);
    let svg_path = output_dir.join(format!("{}.svg", prefix));
    fs::write(&svg_path, &svg)
        .map_err(|e| format!("Failed to write SVG: {}", e))?;

    // Extract assertions from the spec (pass through to evaluator)
    let assertions = spec.get("assertions").cloned();
    let assertions_path = output_dir.join(format!("{}.assertions.json", prefix));
    if let Some(ref a) = assertions {
        let assertions_json: serde_json::Value = serde_yaml::from_value(a.clone())
            .unwrap_or(serde_json::Value::Null);
        fs::write(&assertions_path, serde_json::to_string_pretty(&assertions_json).unwrap())
            .map_err(|e| format!("Failed to write assertions: {}", e))?;
    }

    // Write render result metadata (evaluator will update this with verdict)
    let result_path = output_dir.join(format!("{}.result.json", prefix));
    let result_meta = serde_json::json!({
        "spec": prefix,
        "render_status": "ok",
        "tree_path": tree_path.to_string_lossy(),
        "svg_path": svg_path.to_string_lossy(),
        "eval_status": null,
        "classification": null,
        "failures": [],
        "fix_required": null,
        "last_evaluated": null
    });
    fs::write(&result_path, serde_json::to_string_pretty(&result_meta).unwrap())
        .map_err(|e| format!("Failed to write result: {}", e))?;

    Ok(RenderResult {
        spec_name: prefix,
        tree_path,
        svg_path,
        assertions_path: if assertions.is_some() { Some(assertions_path) } else { None },
        expected_error: None,
    })
}

struct RenderResult {
    spec_name: String,
    tree_path: PathBuf,
    svg_path: PathBuf,
    assertions_path: Option<PathBuf>,
    expected_error: Option<String>,
}

fn main() {
    // DataFusionTransform is async and internally drives DataFusion futures
    // that require a Tokio reactor (e.g. for `JoinSet::spawn`). The sync
    // render path inside chartml-core uses `pollster::block_on` to await the
    // middleware, which parks the current thread but does NOT drive Tokio
    // tasks. Entering a multi-threaded Tokio runtime here gives DataFusion's
    // spawned work somewhere to run on background worker threads while
    // pollster blocks the main thread waiting for the top-level future.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to build Tokio runtime");
    let _guard = runtime.enter();

    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage:");
        eprintln!("  chartml-test <spec.yaml> [--output-dir <dir>]");
        eprintln!("  chartml-test --batch <dir-of-specs> [--output-dir <dir>]");
        eprintln!("  chartml-test --gallery [--port <port>]");
        eprintln!("  chartml-test --accept                  # save current as golden baseline");
        eprintln!("  chartml-test --diff                     # compare current vs golden");
        std::process::exit(1);
    }

    let mut output_dir = PathBuf::from("test-output");
    let mut batch_mode = false;
    let mut gallery_mode = false;
    let mut accept_mode = false;
    let mut diff_mode = false;
    let mut gallery_port: u16 = 8642;
    let mut input_path = PathBuf::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--output-dir" => {
                i += 1;
                output_dir = PathBuf::from(&args[i]);
            }
            "--batch" => {
                batch_mode = true;
                i += 1;
                input_path = PathBuf::from(&args[i]);
            }
            "--gallery" => {
                gallery_mode = true;
            }
            "--port" => {
                i += 1;
                gallery_port = args[i].parse().expect("Invalid port number");
            }
            "--accept" => {
                accept_mode = true;
            }
            "--diff" => {
                diff_mode = true;
            }
            arg => {
                input_path = PathBuf::from(arg);
            }
        }
        i += 1;
    }

    if gallery_mode {
        gallery::serve(gallery_port);
        return;
    }

    if accept_mode {
        diff::accept();
        return;
    }

    if diff_mode {
        let has_changes = diff::diff();
        if has_changes {
            std::process::exit(1);
        }
        return;
    }

    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    if batch_mode {
        run_batch(&input_path, &output_dir);
    } else {
        run_single(&input_path, &output_dir);
    }
}

fn run_single(spec_path: &Path, output_dir: &Path) {
    match render_spec(spec_path, output_dir, None) {
        Ok(result) => {
            if let Some(ref err) = result.expected_error {
                println!("OK   {} (expected error: {})", result.spec_name, err);
            } else {
                println!("OK {}", result.spec_name);
                println!("  tree: {}", result.tree_path.display());
                println!("  svg:  {}", result.svg_path.display());
                if let Some(ref a) = result.assertions_path {
                    println!("  assertions: {}", a.display());
                }
            }
        }
        Err(e) => {
            eprintln!("FAIL {}", e);
            std::process::exit(1);
        }
    }
}

fn run_batch(specs_dir: &Path, output_dir: &Path) {
    let mut specs: Vec<PathBuf> = Vec::new();
    collect_specs(specs_dir, &mut specs);
    specs.sort();

    let total = specs.len();
    let mut passed = 0;
    let mut failed = 0;
    let mut failures: Vec<(String, String)> = Vec::new();

    for spec_path in &specs {
        match render_spec(spec_path, output_dir, Some(specs_dir)) {
            Ok(result) => {
                if let Some(ref err) = result.expected_error {
                    println!("OK   {} (expected error: {})", result.spec_name, err);
                } else {
                    println!("OK   {}", result.spec_name);
                }
                passed += 1;
            }
            Err(e) => {
                let name = spec_path.file_stem().unwrap().to_string_lossy().to_string();
                eprintln!("FAIL {}: {}", name, e);
                failures.push((name, e));
                failed += 1;
            }
        }
    }

    // Write batch summary manifest
    let manifest = serde_json::json!({
        "total": total,
        "rendered": passed,
        "render_failed": failed,
        "render_failures": failures.iter().map(|(n, e)| serde_json::json!({"spec": n, "error": e})).collect::<Vec<_>>(),
    });
    let manifest_path = output_dir.join("manifest.json");
    let _ = fs::write(&manifest_path, serde_json::to_string_pretty(&manifest).unwrap());

    println!("\n--- Results ---");
    println!("Total: {}  Passed: {}  Failed: {}", total, passed, failed);
    println!("Manifest: {}", manifest_path.display());
    if !failures.is_empty() {
        println!("\nFailures:");
        for (name, err) in &failures {
            println!("  {} — {}", name, err);
        }
        std::process::exit(1);
    }
}

fn collect_specs(dir: &Path, specs: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_specs(&path, specs);
            } else if path.extension().map(|e| e == "yaml" || e == "yml").unwrap_or(false) {
                specs.push(path);
            }
        }
    }
}
