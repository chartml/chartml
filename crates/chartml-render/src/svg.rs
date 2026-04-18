//! ChartElement tree → SVG string serialization.
//!
//! As of chartml 5.0 (phase 2), the implementation lives in
//! `chartml_core::svg` so the new pipeline's `render_prepared_to_svg`
//! method can produce SVG strings without circular dependency on
//! `chartml-render`. This module re-exports the canonical
//! `element_to_svg` for back-compat with existing callers
//! (`chartml-test-runner`, `chartml-wasm`, downstream apps).

pub use chartml_core::svg::element_to_svg;
