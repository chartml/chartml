//! Render-time options for the three-stage pipeline.
//!
//! Consolidates the scattered `container_width`, `container_height`, and
//! `param_overrides` arguments that the legacy `render_from_yaml_*` family
//! used to take individually. A single `&RenderOptions` parameter threads
//! through every stage (`fetch`, `transform`, `render_prepared_to_svg`,
//! `render_to_svg_async`) so future fields (e.g. `theme`, `density`,
//! `cancel_token`) can be added without breaking signatures.

use crate::params::ParamValues;

/// Render-time knobs for the chartml 5.0 pipeline.
///
/// Defaults preserve every legacy behavior: `width` / `height` of `None`
/// fall through to the spec's `style.width` / `style.height`, then to the
/// renderer's default dimensions (e.g. 800×400 for cartesian charts);
/// `params` of `None` matches the legacy "no param overrides" path. Cloning
/// is cheap — fields are scalar plus a small `HashMap`.
#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    /// Container width override in CSS pixels. `None` means "use the spec
    /// or the renderer's default".
    pub width: Option<f64>,
    /// Container height override in CSS pixels. `None` means "use the spec
    /// or the renderer's default".
    pub height: Option<f64>,
    /// Interactive parameter values (e.g. dropdown selections) layered on
    /// top of `params:` defaults declared in the YAML. `None` skips param
    /// overrides entirely.
    pub params: Option<ParamValues>,
    /// Which page of a paginated `table` to render, from 0, for a host that
    /// drives the pager. `None` (the default) is a static render: page 0, with
    /// the pager's buttons disabled. Only that page's rows are rendered; column
    /// widths are still measured over every row. A page past the last is an
    /// error, not clamped. Other chart types ignore it (CHA-30).
    pub table_page: Option<usize>,
    /// The host draws the chart's `title` itself, as a heading above the
    /// rendered SVG (the Leptos component does). `false` (the default) means
    /// the SVG is shown alone, so a chart that can only show its title inside
    /// itself must do so. Only the `metric` renderer reads it (CHA-49).
    pub title_heading: bool,
}

impl RenderOptions {
    /// Build options that only override the canvas size — no param values.
    /// Sugar for the common "I just want a different width/height" call site.
    pub fn with_size(width: Option<f64>, height: Option<f64>) -> Self {
        Self {
            width,
            height,
            params: None,
            table_page: None,
            title_heading: false,
        }
    }

    /// Borrow the parameter overrides (if any). Matches the
    /// `Option<&ParamValues>` shape every internal callsite already uses.
    pub fn params_ref(&self) -> Option<&ParamValues> {
        self.params.as_ref()
    }
}
