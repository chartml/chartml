use leptos::prelude::*;
use chartml_core::element::ElementData;

/// Active tooltip state — set by interactive elements, read by the container.
#[derive(Clone, Default)]
pub struct TooltipState {
    /// The data from the hovered element.
    pub data: Option<ElementData>,
    /// Mouse X relative to the chart container (pixels).
    pub x: f64,
    /// Mouse Y relative to the chart container (pixels).
    pub y: f64,
}

impl TooltipState {
    pub fn visible(&self) -> bool {
        self.data.is_some()
    }

    pub fn show(data: ElementData, x: f64, y: f64) -> Self {
        Self { data: Some(data), x, y }
    }

    pub fn hide() -> Self {
        Self::default()
    }
}

/// Shared tooltip signal provided via Leptos context.
/// Interactive elements write to this; the ChartMLChart container reads it.
pub fn provide_tooltip_context() -> RwSignal<TooltipState> {
    let signal = RwSignal::new(TooltipState::default());
    provide_context(signal);
    signal
}

/// Read the tooltip context provided by an ancestor ChartMLChart.
pub fn use_tooltip() -> Option<RwSignal<TooltipState>> {
    use_context::<RwSignal<TooltipState>>()
}

/// Default tooltip renderer — an HTML div with label and value.
/// Users can override by providing a custom `tooltip` prop to ChartMLChart.
#[component]
pub fn DefaultTooltip(state: TooltipState) -> impl IntoView {
    let data = state.data.expect("DefaultTooltip is only rendered when state.visible()");
    let series_text = data.series.as_ref().map(|s| format!("{}: ", s)).unwrap_or_default();

    view! {
        <div class="chartml-tooltip-label">{series_text}{data.label.clone()}</div>
        <div class="chartml-tooltip-value">{data.value.clone()}</div>
    }
}
