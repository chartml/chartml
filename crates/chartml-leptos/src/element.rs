use leptos::prelude::*;
use chartml_core::element::*;

/// Recursively render a ChartElement tree into Leptos view nodes.
/// This is the main rendering function that the ChartMLChart component calls.
pub fn render_element(element: &ChartElement) -> AnyView {
    match element {
        ChartElement::Svg { viewbox, width, height, class, children } => {
            let viewbox_str = viewbox.to_string();
            let class = class.clone();
            let width_str = width.map(|w| w.to_string()).unwrap_or_default();
            let height_str = height.map(|h| h.to_string()).unwrap_or_default();
            let children_views: Vec<AnyView> = children.iter().map(render_element).collect();

            view! {
                <svg
                    viewBox=viewbox_str
                    width=width_str
                    height=height_str
                    class=class
                    style="overflow: visible;"
                >
                    {children_views}
                </svg>
            }.into_any()
        }

        ChartElement::Group { class, transform, children } => {
            let class = class.clone();
            let transform_str = transform.as_ref().map(|t| t.to_svg_string()).unwrap_or_default();
            let children_views: Vec<AnyView> = children.iter().map(render_element).collect();

            view! {
                <g class=class transform=transform_str>
                    {children_views}
                </g>
            }.into_any()
        }

        ChartElement::Rect { x, y, width, height, fill, stroke, class, data } => {
            let x_str = x.to_string();
            let y_str = y.to_string();
            let w_str = width.to_string();
            let h_str = height.to_string();
            let fill = fill.clone();
            let stroke_str = stroke.clone().unwrap_or_default();
            let class = class.clone();
            let tooltip_data = data.clone();
            // For bar animation: transform-origin at bottom center of each rect
            let origin_x = x + width / 2.0;
            let origin_y = y + height;
            let style = format!("transform-origin: {}px {}px;", origin_x, origin_y);

            if let Some(data) = tooltip_data {
                render_interactive_rect(x_str, y_str, w_str, h_str, fill, stroke_str, class, style, data)
            } else {
                view! {
                    <rect
                        x=x_str y=y_str width=w_str height=h_str
                        fill=fill stroke=stroke_str class=class
                        style=style
                    />
                }.into_any()
            }
        }

        ChartElement::Path { d, fill, stroke, stroke_width, stroke_dasharray, class, data } => {
            let d = d.clone();
            let fill_str = fill.clone().unwrap_or_else(|| "none".to_string());
            let stroke_str = stroke.clone().unwrap_or_else(|| "none".to_string());
            let sw = stroke_width.map(|w| w.to_string()).unwrap_or_default();
            let sda = stroke_dasharray.clone().unwrap_or_default();
            let class = class.clone();
            let tooltip_data = data.clone();

            if let Some(data) = tooltip_data {
                render_interactive_path(d, fill_str, stroke_str, sw, sda, class, data)
            } else {
                view! {
                    <path
                        d=d fill=fill_str stroke=stroke_str
                        stroke-width=sw stroke-dasharray=sda class=class
                    />
                }.into_any()
            }
        }

        ChartElement::Circle { cx, cy, r, fill, stroke, class, data } => {
            let cx_str = cx.to_string();
            let cy_str = cy.to_string();
            let r_str = r.to_string();
            let fill = fill.clone();
            let stroke_str = stroke.clone().unwrap_or_default();
            let class = class.clone();
            let tooltip_data = data.clone();

            if let Some(data) = tooltip_data {
                render_interactive_circle(cx_str, cy_str, r_str, fill, stroke_str, class, data)
            } else {
                view! {
                    <circle
                        cx=cx_str cy=cy_str r=r_str
                        fill=fill stroke=stroke_str class=class
                    />
                }.into_any()
            }
        }

        ChartElement::Line { x1, y1, x2, y2, stroke, stroke_width, stroke_dasharray, class } => {
            let x1 = x1.to_string();
            let y1 = y1.to_string();
            let x2 = x2.to_string();
            let y2 = y2.to_string();
            let stroke = stroke.clone();
            let sw = stroke_width.map(|w| w.to_string()).unwrap_or_default();
            let sda = stroke_dasharray.clone().unwrap_or_default();
            let class = class.clone();

            view! {
                <line
                    x1=x1 y1=y1 x2=x2 y2=y2
                    stroke=stroke stroke-width=sw stroke-dasharray=sda class=class
                />
            }.into_any()
        }

        ChartElement::Text { x, y, content, anchor, dominant_baseline, transform, font_size, fill, class } => {
            let x = x.to_string();
            let y = y.to_string();
            let content = content.clone();
            let anchor = anchor.to_string();
            let db = dominant_baseline.clone().unwrap_or_default();
            let transform_str = transform.as_ref().map(|t| t.to_svg_string()).unwrap_or_default();
            let fs = font_size.clone().unwrap_or_default();
            let fill = fill.clone().unwrap_or_default();
            let class = class.clone();

            view! {
                <text
                    x=x y=y
                    text-anchor=anchor
                    dominant-baseline=db
                    transform=transform_str
                    font-size=fs
                    fill=fill
                    class=class
                >
                    {content}
                </text>
            }.into_any()
        }

        ChartElement::Div { class, style, children } => {
            let class = class.clone();
            let style_str = style.iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<_>>()
                .join("; ");
            let children_views: Vec<AnyView> = children.iter().map(render_element).collect();

            view! {
                <div class=class style=style_str>
                    {children_views}
                </div>
            }.into_any()
        }

        ChartElement::Span { class, style, content } => {
            let class = class.clone();
            let style_str = style.iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<_>>()
                .join("; ");
            let content = content.clone();

            view! {
                <span class=class style=style_str>{content}</span>
            }.into_any()
        }
    }
}

fn render_interactive_rect(
    x: String, y: String, w: String, h: String,
    fill: String, stroke: String, class: String,
    base_style: String,
    data: ElementData,
) -> AnyView {
    let hovered = RwSignal::new(false);
    let tooltip_text = format!("{}: {}", data.label, data.value);
    let tooltip_width = (tooltip_text.len() as f64 * 7.0 + 16.0).to_string();

    view! {
        <g class="chartml-interactive">
            <rect
                x=x.clone() y=y.clone() width=w height=h
                fill=fill stroke=stroke class=class
                style=move || {
                    if hovered.get() {
                        format!("{} opacity: 0.8; cursor: pointer;", base_style)
                    } else {
                        base_style.clone()
                    }
                }
                on:mouseenter=move |_| hovered.set(true)
                on:mouseleave=move |_| hovered.set(false)
            />
            {move || {
                let tooltip_text = tooltip_text.clone();
                let tooltip_width = tooltip_width.clone();
                if hovered.get() {
                    let tx: f64 = x.parse().unwrap_or(0.0);
                    let ty: f64 = y.parse().unwrap_or(0.0) - 10.0;
                    view! {
                        <g class="chartml-tooltip">
                            <rect
                                x=(tx - 5.0).to_string()
                                y=(ty - 20.0).to_string()
                                width=tooltip_width
                                height="20"
                                rx="3"
                                fill="#333"
                                opacity="0.9"
                            />
                            <text
                                x=tx.to_string()
                                y=(ty - 5.0).to_string()
                                fill="white"
                                font-size="12px"
                                class="chartml-tooltip-text"
                            >
                                {tooltip_text}
                            </text>
                        </g>
                    }.into_any()
                } else {
                    view! { <g /> }.into_any()
                }
            }}
        </g>
    }.into_any()
}

fn render_interactive_path(
    d: String, fill: String, stroke: String,
    stroke_width: String, stroke_dasharray: String, class: String,
    data: ElementData,
) -> AnyView {
    let hovered = RwSignal::new(false);
    let tooltip_text = format!("{}: {}", data.label, data.value);
    let tooltip_width = (tooltip_text.len() as f64 * 7.0 + 16.0).to_string();

    view! {
        <g class="chartml-interactive">
            <path
                d=d fill=fill stroke=stroke
                stroke-width=stroke_width stroke-dasharray=stroke_dasharray class=class
                style=move || if hovered.get() { "opacity: 0.8; cursor: pointer;" } else { "" }
                on:mouseenter=move |_| hovered.set(true)
                on:mouseleave=move |_| hovered.set(false)
            />
            {move || {
                let tooltip_text = tooltip_text.clone();
                let tooltip_width = tooltip_width.clone();
                if hovered.get() {
                    view! {
                        <g class="chartml-tooltip">
                            <rect
                                x="-5"
                                y="-30"
                                width=tooltip_width
                                height="20"
                                rx="3"
                                fill="#333"
                                opacity="0.9"
                            />
                            <text
                                x="0"
                                y="-15"
                                fill="white"
                                font-size="12px"
                                class="chartml-tooltip-text"
                            >
                                {tooltip_text}
                            </text>
                        </g>
                    }.into_any()
                } else {
                    view! { <g /> }.into_any()
                }
            }}
        </g>
    }.into_any()
}

fn render_interactive_circle(
    cx: String, cy: String, r: String,
    fill: String, stroke: String, class: String,
    data: ElementData,
) -> AnyView {
    let hovered = RwSignal::new(false);
    let tooltip_text = format!("{}: {}", data.label, data.value);
    let tooltip_width = (tooltip_text.len() as f64 * 7.0 + 16.0).to_string();

    view! {
        <g class="chartml-interactive">
            <circle
                cx=cx.clone() cy=cy.clone() r=r
                fill=fill stroke=stroke class=class
                style=move || if hovered.get() { "opacity: 0.8; cursor: pointer;" } else { "" }
                on:mouseenter=move |_| hovered.set(true)
                on:mouseleave=move |_| hovered.set(false)
            />
            {move || {
                let tooltip_text = tooltip_text.clone();
                let tooltip_width = tooltip_width.clone();
                if hovered.get() {
                    let tx: f64 = cx.parse().unwrap_or(0.0);
                    let ty: f64 = cy.parse().unwrap_or(0.0) - 15.0;
                    view! {
                        <g class="chartml-tooltip">
                            <rect
                                x=(tx - 5.0).to_string()
                                y=(ty - 20.0).to_string()
                                width=tooltip_width
                                height="20"
                                rx="3"
                                fill="#333"
                                opacity="0.9"
                            />
                            <text
                                x=tx.to_string()
                                y=(ty - 5.0).to_string()
                                fill="white"
                                font-size="12px"
                                class="chartml-tooltip-text"
                            >
                                {tooltip_text}
                            </text>
                        </g>
                    }.into_any()
                } else {
                    view! { <g /> }.into_any()
                }
            }}
        </g>
    }.into_any()
}
