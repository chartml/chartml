use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum ChartError {
    #[error("YAML parse error: {0}")]
    YamlParse(String),

    #[error("Invalid spec: {0}")]
    InvalidSpec(String),

    #[error("Unknown chart type: {0}")]
    UnknownChartType(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Plugin error: {0}")]
    PluginError(String),

    #[error("Data error: {0}")]
    DataError(String),

    #[error("Render error: {0}")]
    RenderError(String),
}

impl From<serde_yaml::Error> for ChartError {
    fn from(err: serde_yaml::Error) -> Self {
        ChartError::YamlParse(err.to_string())
    }
}
