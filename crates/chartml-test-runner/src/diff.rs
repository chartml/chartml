//! Golden baseline diffing for regression detection.
//!
//! `--accept`: copies test-output/all/ SVGs to test-output/golden/
//! `--diff`:   compares test-output/all/ SVGs against test-output/golden/

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const CURRENT_DIR: &str = "test-output/all";
const GOLDEN_DIR: &str = "test-output/golden";

/// Recursively collect all .svg files relative to a root directory.
fn collect_svgs(dir: &Path, root: &Path, out: &mut BTreeMap<String, PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_svgs(&path, root, out);
            } else if path.extension().map(|e| e == "svg").unwrap_or(false) {
                if let Ok(rel) = path.strip_prefix(root) {
                    let key = rel.to_string_lossy().to_string();
                    out.insert(key, path);
                }
            }
        }
    }
}

/// Copy current SVGs to golden directory.
/// Preserves .sig files for SVGs whose content hasn't changed.
pub fn accept() {
    let current = Path::new(CURRENT_DIR);
    let golden = Path::new(GOLDEN_DIR);

    if !current.exists() {
        eprintln!("No current output at {}. Run --batch first.", CURRENT_DIR);
        std::process::exit(1);
    }

    let mut current_svgs = BTreeMap::new();
    collect_svgs(current, current, &mut current_svgs);

    let mut accepted = 0u32;
    let mut unchanged = 0u32;
    let mut sigs_preserved = 0u32;
    let mut sigs_invalidated = 0u32;

    // Collect existing golden SVGs before modifying anything
    let mut old_golden_svgs = BTreeMap::new();
    if golden.exists() {
        collect_svgs(golden, golden, &mut old_golden_svgs);
    }

    // Remove golden SVGs (and their sigs) that no longer exist in current output
    for (rel_path, golden_path) in &old_golden_svgs {
        if !current_svgs.contains_key(rel_path) {
            let _ = fs::remove_file(golden_path);
            let sig_path = PathBuf::from(format!("{}.sig", golden_path.display()));
            let _ = fs::remove_file(&sig_path);
        }
    }

    for (rel_path, src_path) in &current_svgs {
        let dest = golden.join(rel_path);
        let sig_path = PathBuf::from(format!("{}.sig", dest.display()));

        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).expect("Failed to create golden subdir");
        }

        // Check if golden SVG already exists with identical content
        if dest.exists() {
            let current_bytes = fs::read(src_path).unwrap_or_default();
            let golden_bytes = fs::read(&dest).unwrap_or_default();

            if current_bytes == golden_bytes {
                unchanged += 1;
                if sig_path.exists() {
                    sigs_preserved += 1;
                }
                continue;
            }

            // Content changed — remove stale .sig file
            if sig_path.exists() {
                fs::remove_file(&sig_path).ok();
                sigs_invalidated += 1;
            }
        }

        fs::copy(src_path, &dest).expect("Failed to copy SVG to golden");
        accepted += 1;
    }

    println!("Accepted golden baseline in {}", GOLDEN_DIR);
    println!(
        "  Updated: {}, Unchanged: {}, Sigs preserved: {}, Sigs invalidated: {}",
        accepted, unchanged, sigs_preserved, sigs_invalidated
    );
}

/// Compare current SVGs against golden baseline. Returns true if there are changes.
pub fn diff() -> bool {
    let current_path = Path::new(CURRENT_DIR);
    let golden_path = Path::new(GOLDEN_DIR);

    if !golden_path.exists() {
        eprintln!("No golden baseline at {}. Run --accept first.", GOLDEN_DIR);
        std::process::exit(1);
    }

    if !current_path.exists() {
        eprintln!("No current output at {}. Run --batch first.", CURRENT_DIR);
        std::process::exit(1);
    }

    let mut current_svgs = BTreeMap::new();
    let mut golden_svgs = BTreeMap::new();
    collect_svgs(current_path, current_path, &mut current_svgs);
    collect_svgs(golden_path, golden_path, &mut golden_svgs);

    let mut unchanged = 0u32;
    let mut changed: Vec<(String, Vec<String>)> = Vec::new();
    let mut new_specs: Vec<String> = Vec::new();
    let mut removed: Vec<String> = Vec::new();

    // Check all current SVGs against golden
    for (rel, current_file) in &current_svgs {
        if let Some(golden_file) = golden_svgs.get(rel) {
            let current_bytes = fs::read(current_file).unwrap_or_default();
            let golden_bytes = fs::read(golden_file).unwrap_or_default();

            if current_bytes == golden_bytes {
                unchanged += 1;
            } else {
                let current_str = String::from_utf8_lossy(&current_bytes);
                let golden_str = String::from_utf8_lossy(&golden_bytes);
                let diffs = describe_svg_changes(&golden_str, &current_str);
                changed.push((rel.clone(), diffs));
            }
        } else {
            new_specs.push(rel.clone());
        }
    }

    // Check for removed specs (in golden but not current)
    for rel in golden_svgs.keys() {
        if !current_svgs.contains_key(rel) {
            removed.push(rel.clone());
        }
    }

    // Report
    let total_changes = changed.len() + new_specs.len() + removed.len();

    println!("=== Regression Check ===\n");
    println!("Unchanged: {}", unchanged);
    println!("Changed:   {}", changed.len());
    println!("New:       {}", new_specs.len());
    println!("Removed:   {}", removed.len());

    if !changed.is_empty() {
        println!("\nChanged specs:");
        for (spec, diffs) in &changed {
            let spec_name = spec.replace(".svg", "");
            if diffs.is_empty() {
                println!("  {} — content changed (no specific attributes identified)", spec_name);
            } else {
                println!("  {} — {}", spec_name, diffs.join(", "));
            }
        }
    }

    if !new_specs.is_empty() {
        println!("\nNew specs (no golden baseline):");
        for spec in &new_specs {
            println!("  {}", spec.replace(".svg", ""));
        }
    }

    if !removed.is_empty() {
        println!("\nRemoved specs (in golden but not current):");
        for spec in &removed {
            println!("  {}", spec.replace(".svg", ""));
        }
    }

    if total_changes == 0 {
        println!("\nNo regressions detected.");
    } else {
        println!("\n{} spec(s) differ from golden baseline.", total_changes);
        println!("Review changes, then run --accept to update the baseline.");
    }

    total_changes > 0
}

/// Compare two SVG strings and describe what changed in human-readable terms.
fn describe_svg_changes(golden: &str, current: &str) -> Vec<String> {
    let mut diffs = Vec::new();

    // Extract and compare key SVG attributes
    let golden_transforms = extract_all_attr(golden, "transform");
    let current_transforms = extract_all_attr(current, "transform");
    if golden_transforms != current_transforms {
        // Find specific transform changes
        let g_translates = extract_translates(golden);
        let c_translates = extract_translates(current);
        if g_translates != c_translates {
            // Find the first difference
            for (g, c) in g_translates.iter().zip(c_translates.iter()) {
                if g != c {
                    diffs.push(format!("layout shift (translate {} → {})", g, c));
                    break;
                }
            }
            if diffs.is_empty() && g_translates.len() != c_translates.len() {
                diffs.push(format!("transform count changed ({} → {})", g_translates.len(), c_translates.len()));
            }
        }
    }

    // Compare tick label content
    let golden_ticks = extract_text_content(golden, "tick-label");
    let current_ticks = extract_text_content(current, "tick-label");
    if golden_ticks != current_ticks {
        let g_set: std::collections::HashSet<_> = golden_ticks.iter().collect();
        let c_set: std::collections::HashSet<_> = current_ticks.iter().collect();
        let added: Vec<_> = c_set.difference(&g_set).collect();
        let removed: Vec<_> = g_set.difference(&c_set).collect();
        if !added.is_empty() || !removed.is_empty() {
            diffs.push(format!("tick labels changed ({} added, {} removed)", added.len(), removed.len()));
        }
    }

    // Compare axis label positions
    let golden_axis_labels = extract_positioned_elements(golden, "axis-label");
    let current_axis_labels = extract_positioned_elements(current, "axis-label");
    if golden_axis_labels != current_axis_labels {
        for (i, (g, c)) in golden_axis_labels.iter().zip(current_axis_labels.iter()).enumerate() {
            if g.0 != c.0 || g.1 != c.1 {
                diffs.push(format!("axis-label[{}] moved (x:{} → {}, y:{} → {})", i, g.0, c.0, g.1, c.1));
            }
        }
    }

    // Compare viewBox / dimensions
    let g_vb = extract_attr_value(golden, "viewBox");
    let c_vb = extract_attr_value(current, "viewBox");
    if g_vb != c_vb {
        diffs.push(format!("viewBox changed ({:?} → {:?})", g_vb, c_vb));
    }

    // Compare element counts
    let g_rects = golden.matches("<rect ").count();
    let c_rects = current.matches("<rect ").count();
    if g_rects != c_rects {
        diffs.push(format!("rect count ({} → {})", g_rects, c_rects));
    }

    let g_paths = golden.matches("<path ").count();
    let c_paths = current.matches("<path ").count();
    if g_paths != c_paths {
        diffs.push(format!("path count ({} → {})", g_paths, c_paths));
    }

    let g_circles = golden.matches("<circle ").count();
    let c_circles = current.matches("<circle ").count();
    if g_circles != c_circles {
        diffs.push(format!("circle count ({} → {})", g_circles, c_circles));
    }

    let g_texts = golden.matches("<text ").count();
    let c_texts = current.matches("<text ").count();
    if g_texts != c_texts {
        diffs.push(format!("text count ({} → {})", g_texts, c_texts));
    }

    // Byte-level size change
    let size_diff = current.len() as i64 - golden.len() as i64;
    if size_diff.abs() > 50 && diffs.is_empty() {
        diffs.push(format!("size changed by {} bytes", size_diff));
    }

    diffs
}

/// Extract all values of a given attribute from SVG text.
fn extract_all_attr(svg: &str, attr: &str) -> Vec<String> {
    let needle = format!("{}=\"", attr);
    let mut results = Vec::new();
    let mut search_from = 0;
    while let Some(start) = svg[search_from..].find(&needle) {
        let abs_start = search_from + start + needle.len();
        if let Some(end) = svg[abs_start..].find('"') {
            results.push(svg[abs_start..abs_start + end].to_string());
        }
        search_from = abs_start + 1;
    }
    results
}

/// Extract all translate(x,y) values.
fn extract_translates(svg: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut search_from = 0;
    while let Some(start) = svg[search_from..].find("translate(") {
        let abs_start = search_from + start;
        if let Some(end) = svg[abs_start..].find(')') {
            results.push(svg[abs_start..abs_start + end + 1].to_string());
        }
        search_from = abs_start + 10;
    }
    results
}

/// Extract text content for elements with a given class.
fn extract_text_content(svg: &str, class: &str) -> Vec<String> {
    let needle = format!("class=\"{}\"", class);
    let mut results = Vec::new();
    let mut search_from = 0;
    while let Some(pos) = svg[search_from..].find(&needle) {
        let abs_pos = search_from + pos;
        // Find the > after the class attribute, then content until </text>
        if let Some(tag_end) = svg[abs_pos..].find('>') {
            let content_start = abs_pos + tag_end + 1;
            if let Some(close) = svg[content_start..].find("</text>") {
                let content = &svg[content_start..content_start + close];
                results.push(content.to_string());
            }
        }
        search_from = abs_pos + needle.len();
    }
    results
}

/// Extract (x, y) positions for elements with a given class.
fn extract_positioned_elements(svg: &str, class: &str) -> Vec<(String, String)> {
    let needle = format!("class=\"{}\"", class);
    let mut results = Vec::new();
    let mut search_from = 0;
    while let Some(pos) = svg[search_from..].find(&needle) {
        let abs_pos = search_from + pos;
        // Look backward from the class attribute to find x= and y= in the same tag
        let tag_start = svg[..abs_pos].rfind('<').unwrap_or(0);
        let tag_region = &svg[tag_start..abs_pos + needle.len()];

        let x = extract_attr_value(tag_region, "x").unwrap_or_default();
        let y = extract_attr_value(tag_region, "y").unwrap_or_default();
        results.push((x, y));

        search_from = abs_pos + needle.len();
    }
    results
}

/// Extract the first value of an attribute from a string.
fn extract_attr_value(s: &str, attr: &str) -> Option<String> {
    let needle = format!("{}=\"", attr);
    if let Some(start) = s.find(&needle) {
        let after = &s[start + needle.len()..];
        if let Some(end) = after.find('"') {
            return Some(after[..end].to_string());
        }
    }
    None
}
