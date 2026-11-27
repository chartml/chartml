//! Style resolution: named `type: style` components, the `type: config`
//! default, a chart's own `style:` and its `visualize.style`, merged into the
//! one style a chart renders with (CHA-48).
//!
//! Resolution order, least specific first (spec.md "Resolution Order"):
//!
//! 1. the host's base: [`Theme`](crate::theme::Theme), the default palette and
//!    the renderer defaults;
//! 2. every `type: config` style (a named style or an inline object), in the
//!    order the configs were registered (a host registers its system,
//!    workspace and user defaults in that order), then a config in the
//!    chart's own document (the dashboard's);
//! 3. the chart's top-level `style:` (a named style or an inline object);
//! 4. the chart's `visualize.style`.
//!
//! Each level is deep-merged over the one before it: a level overrides only
//! the fields it sets, and `grid`, `fonts.<role>` and `legend` merge field by
//! field. A list (`colors`) is replaced whole.
//!
//! A style name that is not registered fails the render; it never falls back
//! to the defaults. Every level is validated before it is merged, so an error
//! names the level the bad value came from.

use std::collections::HashMap;

use crate::error::ChartError;
use crate::layout::legend::LegendPlacement;
use crate::paint;
use crate::spec::{
    ChartSpec, ChartStyleSpec, ConfigSpec, FontSpec, FontWeight, FontsSpec, GridSpec, LegendSpec,
    StyleRef, StyleRefOrInline, StyleSpec,
};
use crate::theme::Theme;

/// The registered named styles and config defaults.
#[derive(Debug, Clone, Default)]
pub(crate) struct StyleScope {
    styles: HashMap<String, StyleSpec>,
    /// In registration order; each merges over the ones before it.
    configs: Vec<ConfigSpec>,
}

impl StyleScope {
    /// Register a named style. A style registered under a name already in use
    /// replaces the earlier one.
    ///
    /// The style's values are validated here, so a bad value fails where the
    /// style is defined; a config's style name is looked up at render time,
    /// so a config may be registered before the style it names.
    pub(crate) fn register_style(&mut self, style: StyleSpec) -> Result<(), ChartError> {
        validate_style(&format!("style '{}'", style.name), &from_named(&style))?;
        self.styles.insert(style.name.clone(), style);
        Ok(())
    }

    /// Register a config default. Configs cascade: each one's style
    /// deep-merges over the styles of the configs registered before it, so
    /// a later config overrides only the fields it sets.
    pub(crate) fn register_config(&mut self, config: ConfigSpec) -> Result<(), ChartError> {
        if let StyleRef::Inline(style) = &config.style {
            validate_style("config.style", style)?;
        }
        self.configs.push(config);
        Ok(())
    }

    /// Remove every registered config. Named styles stay registered.
    pub(crate) fn clear_configs(&mut self) {
        self.configs.clear();
    }

    /// Return `chart` with its effective style resolved into
    /// `visualize.style` and its top-level `style` cleared. Rendering the
    /// result reads every style setting from `visualize.style` alone.
    ///
    /// Resolve a chart once, against the scope its document renders in:
    /// resolving the result again in another scope would merge that scope's
    /// config under it.
    pub(crate) fn resolve(&self, chart: &ChartSpec) -> Result<ChartSpec, ChartError> {
        let mut merged: Option<ChartStyleSpec> = None;

        // Registered styles and config styles were validated when they were
        // registered; the chart's own levels are validated here.
        for config in &self.configs {
            let layer = match &config.style {
                StyleRef::Named(name) => self.named(name, "a registered config")?,
                StyleRef::Inline(style) => style.as_ref().clone(),
            };
            merged = Some(merge(merged, &layer));
        }

        match &chart.style {
            Some(StyleRefOrInline::Named(name)) => {
                let layer = self.named(name, "the chart's `style`")?;
                merged = Some(merge(merged, &layer));
            }
            Some(StyleRefOrInline::Inline(style)) => {
                validate_style("style", style)?;
                merged = Some(merge(merged, style));
            }
            None => {}
        }

        if let Some(style) = &chart.visualize.style {
            validate_style("visualize.style", style)?;
            merged = Some(merge(merged, style));
        }

        let mut resolved = chart.clone();
        resolved.style = None;
        resolved.visualize.style = merged;
        Ok(resolved)
    }

    /// Look up a registered (already validated) style by name, as a style
    /// layer.
    fn named(&self, name: &str, referenced_by: &str) -> Result<ChartStyleSpec, ChartError> {
        let style = self.styles.get(name).ok_or_else(|| {
            ChartError::InvalidSpec(format!(
                "style '{name}' is not registered (referenced by {referenced_by}); \
                 register a `type: style` component named '{name}' first"
            ))
        })?;
        Ok(from_named(style))
    }
}

/// A named style as a style layer. A Style component carries a subset of the
/// inline style fields.
fn from_named(style: &StyleSpec) -> ChartStyleSpec {
    ChartStyleSpec {
        colors: style.colors.clone(),
        grid: style.grid.clone(),
        height: style.height,
        show_dots: style.show_dots,
        stroke_width: style.stroke_width,
        fonts: style.fonts.clone(),
        legend: style.legend.clone(),
        ..ChartStyleSpec::default()
    }
}

/// Deep-merge `over` onto `base`: every field `over` sets wins.
fn merge(base: Option<ChartStyleSpec>, over: &ChartStyleSpec) -> ChartStyleSpec {
    let base = base.unwrap_or_default();
    ChartStyleSpec {
        height: over.height.or(base.height),
        width: over.width.or(base.width),
        colors: over.colors.clone().or(base.colors),
        grid: merge_opt(base.grid, &over.grid, merge_grid),
        show_dots: over.show_dots.or(base.show_dots),
        stroke_width: over.stroke_width.or(base.stroke_width),
        curve_type: over.curve_type.clone().or(base.curve_type),
        fill_opacity: over.fill_opacity.or(base.fill_opacity),
        fonts: merge_opt(base.fonts, &over.fonts, merge_fonts),
        legend: merge_opt(base.legend, &over.legend, merge_legend),
        page_size: over.page_size.or(base.page_size),
    }
}

fn merge_opt<T: Clone>(base: Option<T>, over: &Option<T>, f: fn(T, &T) -> T) -> Option<T> {
    match (base, over) {
        (Some(b), Some(o)) => Some(f(b, o)),
        (None, Some(o)) => Some(o.clone()),
        (b, None) => b,
    }
}

fn merge_grid(base: GridSpec, over: &GridSpec) -> GridSpec {
    GridSpec {
        x: over.x.or(base.x),
        y: over.y.or(base.y),
        color: over.color.clone().or(base.color),
        opacity: over.opacity.or(base.opacity),
        dash_array: over.dash_array.clone().or(base.dash_array),
    }
}

fn merge_fonts(base: FontsSpec, over: &FontsSpec) -> FontsSpec {
    FontsSpec {
        title: merge_opt(base.title, &over.title, merge_font),
        axis: merge_opt(base.axis, &over.axis, merge_font),
        data_label: merge_opt(base.data_label, &over.data_label, merge_font),
    }
}

fn merge_font(base: FontSpec, over: &FontSpec) -> FontSpec {
    FontSpec {
        family: over.family.clone().or(base.family),
        size: over.size.or(base.size),
        weight: over.weight.clone().or(base.weight),
        color: over.color.clone().or(base.color),
    }
}

fn merge_legend(base: LegendSpec, over: &LegendSpec) -> LegendSpec {
    LegendSpec {
        position: over.position.clone().or(base.position),
        orientation: over.orientation.clone().or(base.orientation),
    }
}

/// Validate one style level: its paints, font sizes and weights, line width
/// and legend placement. `path` names the level in the error.
pub(crate) fn validate_style(path: &str, style: &ChartStyleSpec) -> Result<(), ChartError> {
    paint::validate_style(path, style)?;
    if let Some(fonts) = &style.fonts {
        for (role, font) in [
            ("title", &fonts.title),
            ("axis", &fonts.axis),
            ("dataLabel", &fonts.data_label),
        ] {
            if let Some(font) = font {
                validate_font(&format!("{path}.fonts.{role}"), font)?;
            }
        }
    }
    if let Some(width) = style.stroke_width {
        if !(width.is_finite() && width >= 0.0) {
            return Err(ChartError::InvalidSpec(format!(
                "{path}.strokeWidth must be a number of 0 or more, got {width}"
            )));
        }
    }
    for (field, opacity) in [
        ("fillOpacity", style.fill_opacity),
        ("grid.opacity", style.grid.as_ref().and_then(|g| g.opacity)),
    ] {
        if let Some(opacity) = opacity {
            if !(0.0..=1.0).contains(&opacity) {
                return Err(ChartError::InvalidSpec(format!(
                    "{path}.{field} must be a number from 0 to 1, got {opacity}"
                )));
            }
        }
    }
    LegendPlacement::from_spec(style.legend.as_ref())
        .map_err(|msg| ChartError::InvalidSpec(format!("{path}.legend.{msg}")))?;
    Ok(())
}

fn validate_font(path: &str, font: &FontSpec) -> Result<(), ChartError> {
    if let Some(family) = &font.family {
        validate_font_family(&format!("{path}.family"), family)?;
    }
    if let Some(size) = font.size {
        if !(size.is_finite() && size > 0.0) {
            return Err(ChartError::InvalidSpec(format!(
                "{path}.size must be a number greater than 0, got {size}"
            )));
        }
    }
    if let Some(weight) = &font.weight {
        // The inherited weight only matters for `bolder` / `lighter`; any
        // valid weight passes here.
        font_weight(weight, 400).map_err(|msg| ChartError::InvalidSpec(format!("{path}.weight {msg}")))?;
    }
    Ok(())
}

/// A font family from a chart spec is copied into SVG `font-family`
/// attributes and into HTML inline `style` strings (the Leptos title heading,
/// a metric card's title), so, like a paint, it is checked before use: a font
/// family list may hold only letters, digits, spaces, commas, hyphens,
/// underscores, and single or double quotes (in pairs) around a family name.
/// Anything else (`;`, `{`, `}`, `(`, `)`, `\`, `<`, `>`, `:`, `/`, a line
/// break) could end the declaration and inject CSS, such as a
/// `background-image: url(...)` that makes the browser fetch a URL, so it
/// fails with [`ChartError::InvalidSpec`] naming the field.
fn validate_font_family(path: &str, family: &str) -> Result<(), ChartError> {
    let allowed = |c: char| c.is_alphanumeric() || matches!(c, ' ' | ',' | '-' | '_' | '\'' | '"');
    let balanced =
        family.matches('\'').count().is_multiple_of(2) && family.matches('"').count().is_multiple_of(2);
    if family.trim().is_empty() || !family.chars().all(allowed) || !balanced {
        return Err(ChartError::InvalidSpec(format!(
            "{path} is not an allowed font family: {family:?} (use family names separated by \
             commas, with letters, digits, spaces, hyphens, underscores and quotes only)"
        )));
    }
    Ok(())
}

/// The numeric weight a font weight resolves to. `inherited` is the weight
/// `bolder` and `lighter` are relative to, as in CSS (Fonts Level 4,
/// "Meaning of Relative Weights").
///
/// Accepted: a whole number from 1 to 1000, or `normal` (400), `bold` (700),
/// `bolder` and `lighter`. A number written as a string (`"600"`, which
/// stored specs from before numeric weights parsed use) is read as that
/// number. Anything else is an error message.
fn font_weight(weight: &FontWeight, inherited: u16) -> Result<u16, String> {
    const EXPECTED: &str = "must be a whole number from 1 to 1000, or normal, bold, bolder or lighter";
    let numeric = |n: f64| {
        if n.is_finite() && n.fract() == 0.0 && (1.0..=1000.0).contains(&n) {
            // In range 1..=1000 and whole, so the cast is exact.
            Ok(n as u16)
        } else {
            Err(format!("{EXPECTED}; got {n}"))
        }
    };
    match weight {
        FontWeight::Number(n) => numeric(*n),
        // Keywords are lowercase, as the schema's `FontWeight` lists them.
        FontWeight::Text(s) => match s.as_str() {
            "normal" => Ok(400),
            "bold" => Ok(700),
            "bolder" => Ok(match inherited {
                0..=349 => 400,
                350..=549 => 700,
                550..=899 => 900,
                w => w,
            }),
            "lighter" => Ok(match inherited {
                0..=99 => inherited,
                100..=549 => 100,
                550..=749 => 400,
                _ => 700,
            }),
            other => match other.parse::<f64>() {
                Ok(n) => numeric(n),
                Err(_) => Err(format!("{EXPECTED}; got '{s}'")),
            },
        },
    }
}

/// The theme a chart renders with: `base` with the chart's resolved style
/// applied to it. `style` is the resolved `visualize.style`
/// ([`StyleScope::resolve`]).
///
/// - `fonts.title` sets the title typography (`title_font_*`, `title_color`);
/// - `fonts.axis` sets axis and tick label typography (`label_font_*`,
///   `numeric_font_family` / `numeric_font_size`, `axis_label_color`),
///   except on a `table`, which has no axes and keeps its own typography
///   (it draws its cells in the label font);
/// - `fonts.dataLabel` sets data label typography (`data_label_font_*`,
///   `data_label_color`);
/// - `strokeWidth` sets `series_line_weight`.
pub(crate) fn chart_theme(
    base: &Theme,
    style: Option<&ChartStyleSpec>,
    chart_type: &str,
) -> Result<Theme, ChartError> {
    let mut theme = base.clone();
    let Some(style) = style else {
        return Ok(theme);
    };
    if let Some(width) = style.stroke_width {
        theme.series_line_weight = width as f32;
    }
    let Some(fonts) = &style.fonts else {
        return Ok(theme);
    };
    let weight = |path: &str, w: &FontWeight, inherited: u16| {
        font_weight(w, inherited).map_err(|msg| ChartError::InvalidSpec(format!("{path} {msg}")))
    };
    if let Some(f) = &fonts.title {
        if let Some(family) = &f.family {
            theme.title_font_family = family.clone();
        }
        if let Some(size) = f.size {
            theme.title_font_size = size as f32;
        }
        if let Some(w) = &f.weight {
            theme.title_font_weight = weight("fonts.title.weight", w, theme.title_font_weight)?;
        }
        if let Some(color) = &f.color {
            theme.title_color = Some(color.clone());
        }
    }
    if let Some(f) = fonts.axis.as_ref().filter(|_| chart_type != "table") {
        if let Some(family) = &f.family {
            theme.label_font_family = family.clone();
            theme.numeric_font_family = family.clone();
        }
        if let Some(size) = f.size {
            theme.label_font_size = size as f32;
            theme.numeric_font_size = size as f32;
        }
        if let Some(w) = &f.weight {
            theme.label_font_weight = weight("fonts.axis.weight", w, theme.label_font_weight)?;
        }
        if let Some(color) = &f.color {
            theme.axis_label_color = Some(color.clone());
        }
    }
    if let Some(f) = &fonts.data_label {
        if let Some(family) = &f.family {
            theme.data_label_font_family = family.clone();
        }
        if let Some(size) = f.size {
            theme.data_label_font_size = size as f32;
        }
        if let Some(w) = &f.weight {
            theme.data_label_font_weight = weight("fonts.dataLabel.weight", w, theme.data_label_font_weight)?;
        }
        if let Some(color) = &f.color {
            theme.data_label_color = Some(color.clone());
        }
    }
    Ok(theme)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_weights_from_1_to_1000_are_accepted_whole() {
        for (n, expected) in [(1.0, 1), (600.0, 600), (1000.0, 1000)] {
            assert_eq!(font_weight(&FontWeight::Number(n), 400), Ok(expected));
        }
    }

    #[test]
    fn out_of_range_or_fractional_weights_fail() {
        for n in [0.0, -1.0, 1000.5, 1001.0, 550.5, f64::INFINITY, f64::NAN] {
            let err = font_weight(&FontWeight::Number(n), 400).expect_err("must fail");
            assert!(err.contains("1 to 1000"), "{n}: {err}");
        }
    }

    #[test]
    fn font_families_allow_ordinary_stacks_only() {
        for ok in [
            "Inter",
            "Inter, 'Helvetica Neue', sans-serif",
            "\"Geist Mono\", monospace",
            "system-ui, -apple-system, Segoe_UI",
            "Noto Sans JP",
            "ヒラギノ角ゴ, sans-serif",
        ] {
            assert!(validate_font_family("f", ok).is_ok(), "{ok}");
        }
        for bad in [
            "Inter; background-image: url(https://tracker.example/x)",
            "Inter}body{color:red",
            "a(b)",
            "x\\y",
            "<script>",
            "Inter:hover",
            "url//x",
            "Inter\ncolor: red",
            "'Inter",
            "",
            "   ",
        ] {
            let err = validate_font_family("fonts.title.family", bad).expect_err(bad);
            assert!(err.to_string().contains("fonts.title.family"), "{err}");
        }
    }

    #[test]
    fn keyword_weights_follow_css() {
        let kw = |s: &str, inherited| font_weight(&FontWeight::Text(s.into()), inherited);
        assert_eq!(kw("normal", 900), Ok(400));
        assert_eq!(kw("bold", 100), Ok(700));
        assert!(kw("Bold", 100).is_err(), "keywords are lowercase, as in the schema");
        // CSS Fonts 4 relative weights.
        assert_eq!(kw("bolder", 300), Ok(400));
        assert_eq!(kw("bolder", 400), Ok(700));
        assert_eq!(kw("bolder", 700), Ok(900));
        assert_eq!(kw("bolder", 950), Ok(950));
        assert_eq!(kw("lighter", 50), Ok(50));
        assert_eq!(kw("lighter", 400), Ok(100));
        assert_eq!(kw("lighter", 600), Ok(400));
        assert_eq!(kw("lighter", 800), Ok(700));
        assert!(kw("heavy", 400).is_err());
        assert!(kw("", 400).is_err());
        // Stored specs from before numeric weights wrote numbers as strings.
        assert_eq!(kw("600", 400), Ok(600));
        assert!(kw("0", 400).is_err());
        assert!(kw("550.5", 400).is_err());
    }
}
