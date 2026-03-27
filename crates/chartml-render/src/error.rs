/// Errors that can occur during chart rendering.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("Chart rendering failed: {0}")]
    Chart(#[from] chartml_core::ChartError),

    #[cfg(feature = "rasterize")]
    #[error("SVG parsing failed: {0}")]
    SvgParse(String),

    #[cfg(feature = "rasterize")]
    #[error("PNG encoding failed: {0}")]
    PngEncode(String),

    #[cfg(feature = "rasterize")]
    #[error("Rasterization failed: {0}")]
    Rasterize(String),
}
