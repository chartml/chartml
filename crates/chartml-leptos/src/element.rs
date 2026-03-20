use leptos::prelude::*;
use chartml_core::element::*;
use crate::tooltip::{TooltipState, use_tooltip};

/// Recursively render a ChartElement tree into Leptos view nodes.
pub fn render_element(element: &ChartElement) -> AnyView {
    match element {
        ChartElement::Svg { viewbox, width: _, height, class, children } => {
            let viewbox_str = viewbox.to_string();
            let class = class.clone();
            // Use viewBox for coordinate system; width=100% to fill container.
            // Height is preserved to maintain aspect ratio.
            let height_str = height.map(|h| h.to_string()).unwrap_or_default();
            let children_views: Vec<AnyView> = children.iter().map(render_element).collect();

            view! {
                <svg
                    viewBox=viewbox_str
                    width="100%"
                    height=height_str
                    class=class
                    style="overflow: visible; display: block;"
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
            // Bar animation: transform-origin at bottom center
            let origin_x = x + width / 2.0;
            let origin_y = y + height;
            let base_style = format!("transform-origin: {}px {}px;", origin_x, origin_y);

            if let Some(data) = data.clone() {
                render_interactive(
                    view! {
                        <rect
                            x=x_str y=y_str width=w_str height=h_str
                            fill=fill stroke=stroke_str class=class
                            style=base_style.clone()
                        />
                    }.into_any(),
                    base_style,
                    data,
                )
            } else {
                view! {
                    <rect
                        x=x_str y=y_str width=w_str height=h_str
                        fill=fill stroke=stroke_str class=class
                        style=base_style
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

            if let Some(data) = data.clone() {
                render_interactive(
                    view! {
                        <path
                            d=d fill=fill_str stroke=stroke_str
                            stroke-width=sw stroke-dasharray=sda class=class
                        />
                    }.into_any(),
                    String::new(),
                    data,
                )
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

            if let Some(data) = data.clone() {
                render_interactive(
                    view! {
                        <circle
                            cx=cx_str cy=cy_str r=r_str
                            fill=fill stroke=stroke_str class=class
                        />
                    }.into_any(),
                    String::new(),
                    data,
                )
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
            let mut pairs: Vec<_> = style.iter().collect();
            pairs.sort_by_key(|(k, _)| (*k).clone());
            let style_str = pairs.iter()
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
            let mut pairs: Vec<_> = style.iter().collect();
            pairs.sort_by_key(|(k, _)| (*k).clone());
            let style_str = pairs.iter()
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

/// Wrap any SVG element with interactive hover behavior.
/// On mouseenter: sets the shared tooltip signal with ElementData + position.
/// On mouseleave: clears the tooltip signal.
/// This keeps tooltip rendering out of the SVG — the ChartMLChart container
/// renders the tooltip as an HTML overlay.
fn render_interactive(
    inner: AnyView,
    base_style: String,
    data: ElementData,
) -> AnyView {
    let tooltip_signal = use_tooltip();
    let hovered = RwSignal::new(false);
    let data_for_enter = data.clone();

    view! {
        <g
            class="chartml-interactive"
            style=move || {
                if hovered.get() {
                    format!("{} opacity: 0.8; cursor: pointer;", base_style)
                } else {
                    base_style.clone()
                }
            }
            on:mouseenter=move |ev| {
                hovered.set(true);
                if let Some(sig) = tooltip_signal {
                    let x = ev.client_x() as f64;
                    let y = ev.client_y() as f64;
                    sig.set(TooltipState::show(data_for_enter.clone(), x, y));
                }
            }
            on:mousemove=move |ev| {
                if let Some(sig) = tooltip_signal {
                    if hovered.get() {
                        sig.update(|s| {
                            s.x = ev.client_x() as f64;
                            s.y = ev.client_y() as f64;
                        });
                    }
                }
            }
            on:mouseleave=move |_| {
                hovered.set(false);
                if let Some(sig) = tooltip_signal {
                    sig.set(TooltipState::hide());
                }
            }
        >
            {inner}
        </g>
    }.into_any()
}
