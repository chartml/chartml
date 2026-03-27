use serde::{Deserialize, Serialize};

use crate::data::DataTable;
use crate::element::{ChartElement, Dimensions};
use crate::error::ChartError;
use crate::spec::VisualizeSpec;

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
}

/// Chart renderer plugin — converts data + config into a ChartElement tree.
pub trait ChartRenderer: super::MaybeSend {
    /// Render data with the given config into a ChartElement tree.
    fn render(&self, data: &DataTable, config: &ChartConfig) -> Result<ChartElement, ChartError>;

    /// Optional: provide default dimensions for this chart type.
    fn default_dimensions(&self, _spec: &VisualizeSpec) -> Option<Dimensions> {
        None
    }
}
