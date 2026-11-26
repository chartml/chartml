use serde::{Deserialize, Serialize};

use crate::data::DataTable;
use crate::element::{ChartElement, Dimensions};
use crate::error::ChartError;
use crate::spec::VisualizeSpec;
use crate::theme::Theme;

/// Configuration passed to chart renderers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartConfig {
    /// The visualize spec from the parsed YAML.
    pub visualize: VisualizeSpec,
    /// Chart title (if any).
    pub title: Option<String>,
    /// Available width in pixels.
    pub width: f64,
    /// Available height in pixels.
    pub height: f64,
    /// Color palette to use.
    pub colors: Vec<String>,
    /// Theme colors for chart chrome (axes, grid, text).
    /// Not serialized — set at render time by the consuming application.
    #[serde(skip, default)]
    pub theme: Theme,
    /// Which page of a paginated chart to render, from 0, when the host can
    /// drive the pager. `None` is a static render: page 0, and the pager's
    /// buttons have no page to go to, so they stay disabled. Only the `table`
    /// renderer pages; every other renderer ignores it. A page past the last
    /// one is an error, never clamped (CHA-30).
    #[serde(default)]
    pub table_page: Option<usize>,
}

/// Chart renderer plugin — converts data + config into a ChartElement tree.
pub trait ChartRenderer: Send + Sync {
    /// Render data with the given config into a ChartElement tree.
    fn render(&self, data: &DataTable, config: &ChartConfig) -> Result<ChartElement, ChartError>;

    /// Optional: provide default dimensions for this chart type.
    fn default_dimensions(&self, _spec: &VisualizeSpec) -> Option<Dimensions> {
        None
    }
}
