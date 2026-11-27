//! Grid line settings resolved from a chart's `style.grid`.

use crate::plugin::ChartConfig;

/// Grid line configuration resolved from the spec.
#[derive(Debug, Clone)]
pub struct GridConfig {
    /// Draw vertical grid lines (`grid.x`).
    pub show_x: bool,
    /// Draw horizontal grid lines (`grid.y`).
    pub show_y: bool,
    /// Grid line colour (`grid.color`, else `theme.grid`).
    pub color: String,
    /// Grid line opacity (`grid.opacity`). `None` draws them opaque.
    pub opacity: Option<f64>,
    /// Grid line dash pattern (`grid.dashArray`).
    pub dash_array: Option<String>,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            show_x: false,
            show_y: true,
            color: "#e0e0e0".into(),
            opacity: None,
            dash_array: None,
        }
    }
}

impl GridConfig {
    /// Build from chart config: the chart type's default visibility
    /// (`show_x`, `show_y`) and the theme's grid colour, overridden field by
    /// field by the chart's resolved `style.grid`.
    pub fn from_config_with_defaults(config: &ChartConfig, show_x: bool, show_y: bool) -> Self {
        let mut grid = Self {
            show_x,
            show_y,
            color: config.theme.grid.clone(),
            opacity: None,
            dash_array: None,
        };
        if let Some(g) = config.visualize.style.as_ref().and_then(|s| s.grid.as_ref()) {
            if let Some(x) = g.x { grid.show_x = x; }
            if let Some(y) = g.y { grid.show_y = y; }
            if let Some(ref c) = g.color { grid.color = c.clone(); }
            if let Some(o) = g.opacity { grid.opacity = Some(o); }
            if let Some(ref d) = g.dash_array { grid.dash_array = Some(d.clone()); }
        }
        grid
    }
}
