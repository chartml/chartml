use leptos::prelude::*;

/// Tooltip display data
#[derive(Clone, Default)]
pub struct TooltipData {
    pub label: String,
    pub value: String,
    pub series: Option<String>,
    pub x: f64,
    pub y: f64,
    pub visible: bool,
}

/// A tooltip context that can be shared across chart elements.
/// Uses a signal to track the current tooltip state.
pub fn create_tooltip_signal() -> RwSignal<TooltipData> {
    RwSignal::new(TooltipData::default())
}
