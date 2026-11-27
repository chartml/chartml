//! SVG paint policy for chart specs.
//!
//! Chart specs come from untrusted authors, and every colour in a spec is
//! copied into an SVG `fill` / `stroke` attribute. [`validate_chart_spec`]
//! is the single boundary: it checks every colour-valued field of a
//! [`ChartSpec`] and fails with [`ChartError::InvalidSpec`] when a value is
//! not an allowed paint. It never substitutes a default.
//!
//! Allowed paints:
//! - hex colours: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`
//! - `rgb()`, `rgba()`, `hsl()`, `hsla()` with numeric arguments
//! - CSS named colours, plus `transparent`, `currentColor` and `none`
//! - a same-document fragment reference, `url(#id)`, optionally followed by
//!   a fallback colour (internal gradients and patterns)
//!
//! Everything else is rejected, in particular `url()` to anything other than
//! a `#fragment`, `javascript:` and `data:` values, and `var()`.

use crate::error::ChartError;
use crate::spec::{
    AnnotationSpec, ChartSpec, ChartStyleSpec, DataLabelsSpec, FieldRef, FieldRefItem, FieldSpec,
    FontSpec, StyleRefOrInline, VisualizeSpec,
};

/// CSS named colours (CSS Color Level 4) plus the SVG paint keywords.
const NAMED_PAINTS: &[&str] = &[
    "aliceblue", "antiquewhite", "aqua", "aquamarine", "azure", "beige", "bisque", "black",
    "blanchedalmond", "blue", "blueviolet", "brown", "burlywood", "cadetblue", "chartreuse",
    "chocolate", "coral", "cornflowerblue", "cornsilk", "crimson", "cyan", "darkblue", "darkcyan",
    "darkgoldenrod", "darkgray", "darkgreen", "darkgrey", "darkkhaki", "darkmagenta",
    "darkolivegreen", "darkorange", "darkorchid", "darkred", "darksalmon", "darkseagreen",
    "darkslateblue", "darkslategray", "darkslategrey", "darkturquoise", "darkviolet", "deeppink",
    "deepskyblue", "dimgray", "dimgrey", "dodgerblue", "firebrick", "floralwhite", "forestgreen",
    "fuchsia", "gainsboro", "ghostwhite", "gold", "goldenrod", "gray", "green", "greenyellow",
    "grey", "honeydew", "hotpink", "indianred", "indigo", "ivory", "khaki", "lavender",
    "lavenderblush", "lawngreen", "lemonchiffon", "lightblue", "lightcoral", "lightcyan",
    "lightgoldenrodyellow", "lightgray", "lightgreen", "lightgrey", "lightpink", "lightsalmon",
    "lightseagreen", "lightskyblue", "lightslategray", "lightslategrey", "lightsteelblue",
    "lightyellow", "lime", "limegreen", "linen", "magenta", "maroon", "mediumaquamarine",
    "mediumblue", "mediumorchid", "mediumpurple", "mediumseagreen", "mediumslateblue",
    "mediumspringgreen", "mediumturquoise", "mediumvioletred", "midnightblue", "mintcream",
    "mistyrose", "moccasin", "navajowhite", "navy", "oldlace", "olive", "olivedrab", "orange",
    "orangered", "orchid", "palegoldenrod", "palegreen", "paleturquoise", "palevioletred",
    "papayawhip", "peachpuff", "peru", "pink", "plum", "powderblue", "purple", "rebeccapurple",
    "red", "rosybrown", "royalblue", "saddlebrown", "salmon", "sandybrown", "seagreen",
    "seashell", "sienna", "silver", "skyblue", "slateblue", "slategray", "slategrey", "snow",
    "springgreen", "steelblue", "tan", "teal", "thistle", "tomato", "turquoise", "violet",
    "wheat", "white", "whitesmoke", "yellow", "yellowgreen", "transparent", "currentcolor",
    "none",
];

/// Returns `true` when `value` is an allowed SVG paint (see the module docs).
pub fn is_safe_paint(value: &str) -> bool {
    let v = value.trim();
    if v.is_empty() {
        return false;
    }
    if let Some(rest) = v.strip_prefix("url(") {
        return is_safe_fragment_paint(rest);
    }
    is_safe_colour(v)
}

/// `rest` is the text after `url(`: `#id)` optionally followed by a fallback colour.
fn is_safe_fragment_paint(rest: &str) -> bool {
    let Some(rest) = rest.strip_prefix('#') else {
        return false;
    };
    let Some(close) = rest.find(')') else {
        return false;
    };
    let (id, after) = (&rest[..close], rest[close + 1..].trim());
    let id_ok = !id.is_empty()
        && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':'));
    id_ok && (after.is_empty() || is_safe_colour(after))
}

fn is_safe_colour(v: &str) -> bool {
    if let Some(hex) = v.strip_prefix('#') {
        return matches!(hex.len(), 3 | 4 | 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit());
    }
    if let Some(open) = v.find('(') {
        let name = v[..open].to_ascii_lowercase();
        let Some(args) = v[open + 1..].strip_suffix(')') else {
            return false;
        };
        return matches!(name.as_str(), "rgb" | "rgba" | "hsl" | "hsla")
            && !args.trim().is_empty()
            && args.chars().all(|c| {
                c.is_ascii_alphanumeric() || c.is_ascii_whitespace() || matches!(c, '.' | '%' | ',' | '/' | '+' | '-')
            });
    }
    NAMED_PAINTS.contains(&v.to_ascii_lowercase().as_str())
}

fn check(path: &str, value: &str) -> Result<(), ChartError> {
    if is_safe_paint(value) {
        return Ok(());
    }
    let shown: String = value.chars().take(80).collect();
    Err(ChartError::InvalidSpec(format!(
        "{path}: {shown:?} is not an allowed colour. Use a hex colour, rgb()/hsl(), a CSS colour \
         name, or a same-document url(#id) reference; external url(), javascript: and data: \
         paints are rejected"
    )))
}

fn check_opt(path: &str, value: &Option<String>) -> Result<(), ChartError> {
    value.as_deref().map_or(Ok(()), |v| check(path, v))
}

fn check_data_labels(path: &str, dl: &Option<DataLabelsSpec>) -> Result<(), ChartError> {
    match dl {
        Some(dl) => check_opt(&format!("{path}.dataLabels.color"), &dl.color),
        None => Ok(()),
    }
}

fn check_field(path: &str, f: &FieldSpec) -> Result<(), ChartError> {
    check_opt(&format!("{path}.color"), &f.color)?;
    check_data_labels(path, &f.data_labels)
}

fn check_field_ref(path: &str, r: &Option<FieldRef>) -> Result<(), ChartError> {
    match r {
        None | Some(FieldRef::Simple(_)) => Ok(()),
        Some(FieldRef::Detailed(f)) => check_field(path, f),
        Some(FieldRef::Multiple(items)) => {
            for (i, item) in items.iter().enumerate() {
                if let FieldRefItem::Detailed(f) = item {
                    check_field(&format!("{path}[{i}]"), f)?;
                }
            }
            Ok(())
        }
    }
}

fn check_font(path: &str, f: &Option<FontSpec>) -> Result<(), ChartError> {
    match f {
        Some(f) => check_opt(&format!("{path}.color"), &f.color),
        None => Ok(()),
    }
}

pub(crate) fn validate_style(path: &str, s: &ChartStyleSpec) -> Result<(), ChartError> {
    for (i, c) in s.colors.iter().flatten().enumerate() {
        check(&format!("{path}.colors[{i}]"), c)?;
    }
    if let Some(grid) = &s.grid {
        check_opt(&format!("{path}.grid.color"), &grid.color)?;
    }
    if let Some(fonts) = &s.fonts {
        check_font(&format!("{path}.fonts.title"), &fonts.title)?;
        check_font(&format!("{path}.fonts.axis"), &fonts.axis)?;
        check_font(&format!("{path}.fonts.dataLabel"), &fonts.data_label)?;
    }
    Ok(())
}

fn check_annotation(path: &str, a: &AnnotationSpec) -> Result<(), ChartError> {
    check_opt(&format!("{path}.color"), &a.color)?;
    check_opt(&format!("{path}.strokeColor"), &a.stroke_color)
}

fn check_visualize(v: &VisualizeSpec) -> Result<(), ChartError> {
    check_field_ref("visualize.columns", &v.columns)?;
    check_field_ref("visualize.rows", &v.rows)?;
    check_data_labels("visualize", &v.data_labels)?;
    for (i, a) in v.annotations.iter().flatten().enumerate() {
        check_annotation(&format!("visualize.annotations[{i}]"), a)?;
    }
    if let Some(style) = &v.style {
        validate_style("visualize.style", style)?;
    }
    Ok(())
}

/// Validate every colour-valued field of `spec`. Returns the first unsafe
/// value as [`ChartError::InvalidSpec`] naming the field.
pub fn validate_chart_spec(spec: &ChartSpec) -> Result<(), ChartError> {
    check_visualize(&spec.visualize)?;
    if let Some(StyleRefOrInline::Inline(style)) = &spec.style {
        validate_style("style", style)?;
    }
    Ok(())
}

/// Validate a resolved palette (the colours actually handed to renderers,
/// which may come from the host's default palette rather than the spec).
pub fn validate_palette(colors: &[String]) -> Result<(), ChartError> {
    for (i, c) in colors.iter().enumerate() {
        check(&format!("palette[{i}]"), c)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ordinary_paints() {
        for v in [
            "#fff", "#FFFF", "#2E7D9A", "#2E7D9Aff", "red", "RebeccaPurple", "transparent",
            "currentColor", "none", "rgb(1,2,3)", "rgba(1, 2, 3, 0.5)", "rgb(1 2 3 / 50%)",
            "hsl(120, 50%, 50%)", "hsla(120deg 50% 50% / .3)", "url(#grad)",
            "url(#grad) #fff", "url(#a.b-c_d:e)", "  #fff  ",
        ] {
            assert!(is_safe_paint(v), "{v} should be allowed");
        }
    }

    #[test]
    fn rejects_unsafe_paints() {
        for v in [
            "", "   ", "url(https://example.com/p.svg#g)", "url(//example.com/p.svg#g)",
            "url(/p.svg#g)", "url(p.svg#g)", "url(#)", "url(#a b)", "url(#g", "url('#g')",
            "url(#g) url(https://x/y#z)", "url(#g) javascript:alert(1)", "javascript:alert(1)",
            "data:image/svg+xml;base64,AAAA", "var(--x)", "rgb(1,2,3", "rgb(url(x))",
            "rgb(1,2,3)x", "#12", "#12345", "#gggggg", "notacolour", "red;stroke:url(//x)",
            "calc(1)", "expression(1)",
        ] {
            assert!(!is_safe_paint(v), "{v:?} should be rejected");
        }
    }

    #[test]
    fn error_names_the_field_and_value() {
        let err = check("visualize.style.colors[2]", "url(https://e.com/p#g)").expect_err("unsafe paint must fail");
        let msg = err.to_string();
        assert!(msg.contains("visualize.style.colors[2]"), "{msg}");
        assert!(msg.contains("https://e.com/p#g"), "{msg}");
    }
}
