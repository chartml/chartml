// SPDX-License-Identifier: MIT

//! Reusable chart header bar component.
//!
//! Displays a title, optional type selector, modifier chips (orientation/mode),
//! action buttons, and an action overflow menu above a chart. Used by both the
//! Kyomi dashboard viewer and the MCP chart app.

use leptos::prelude::*;
use send_wrapper::SendWrapper;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Chart types available in the type selector dropdown.
const CHART_TYPES: &[(&str, &str)] = &[
    ("bar", "Bar"),
    ("line", "Line"),
    ("area", "Area"),
    ("scatter", "Scatter"),
    ("pie", "Pie"),
    ("doughnut", "Doughnut"),
    ("table", "Table"),
    ("metric", "Metric"),
];

// ---------------------------------------------------------------------------
// Icons (inline SVG, Heroicons 24/outline subset)
// ---------------------------------------------------------------------------

mod icons {
    use leptos::prelude::*;

    #[component]
    pub fn RefreshIcon() -> impl IntoView {
        view! {
            <svg class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.992 0 3.181 3.183a8.25 8.25 0 0 0 13.803-3.7M4.031 9.865a8.25 8.25 0 0 1 13.803-3.7l3.181 3.182M21.015 4.356v4.992" />
            </svg>
        }
    }

    #[component]
    pub fn DotsVerticalIcon() -> impl IntoView {
        view! {
            <svg class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 5v.01M12 12v.01M12 19v.01M12 6a1 1 0 110-2 1 1 0 010 2zm0 7a1 1 0 110-2 1 1 0 010 2zm0 7a1 1 0 110-2 1 1 0 010 2z" />
            </svg>
        }
    }

    #[component]
    pub fn ChevronDownIcon() -> impl IntoView {
        view! {
            <svg class="h-3 w-3" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" d="m19.5 8.25-7.5 7.5-7.5-7.5" />
            </svg>
        }
    }

    /// Return the SVG view for a chart type icon.
    pub fn chart_type_icon(chart_type: &str) -> impl IntoView {
        match chart_type {
            "bar" => view! {
                <svg class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M3 13.125C3 12.504 3.504 12 4.125 12h2.25c.621 0 1.125.504 1.125 1.125v6.75C7.5 20.496 6.996 21 6.375 21h-2.25A1.125 1.125 0 0 1 3 19.875v-6.75ZM9.75 8.625c0-.621.504-1.125 1.125-1.125h2.25c.621 0 1.125.504 1.125 1.125v11.25c0 .621-.504 1.125-1.125 1.125h-2.25a1.125 1.125 0 0 1-1.125-1.125V8.625ZM16.5 4.125c0-.621.504-1.125 1.125-1.125h2.25C20.496 3 21 3.504 21 4.125v15.75c0 .621-.504 1.125-1.125 1.125h-2.25a1.125 1.125 0 0 1-1.125-1.125V4.125Z" />
                </svg>
            }.into_any(),
            "line" => view! {
                <svg class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M2.25 18 9 11.25l4.306 4.306a11.95 11.95 0 0 1 5.814-5.518l2.74-1.22m0 0-5.94-2.281m5.94 2.28-2.28 5.941" />
                </svg>
            }.into_any(),
            "area" => view! {
                <svg class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M3 20l4-8 4 4 4-10 4 6v8H3Z" />
                    <path stroke-linecap="round" stroke-linejoin="round" d="M3 20l4-8 4 4 4-10 4 6" />
                </svg>
            }.into_any(),
            "scatter" => view! {
                <svg class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
                    <circle cx="5" cy="17" r="1.5" /><circle cx="8" cy="10" r="1.5" />
                    <circle cx="12" cy="14" r="1.5" /><circle cx="14" cy="7" r="1.5" />
                    <circle cx="17" cy="12" r="1.5" /><circle cx="20" cy="5" r="1.5" />
                </svg>
            }.into_any(),
            "pie" => view! {
                <svg class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M12 3a9 9 0 1 0 9 9h-9V3Z" />
                    <path stroke-linecap="round" stroke-linejoin="round" d="M14 2.05A9 9 0 0 1 21.95 10H14V2.05Z" />
                </svg>
            }.into_any(),
            "doughnut" => view! {
                <svg class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M12 3a9 9 0 1 0 9 9h-9V3Z" />
                    <path stroke-linecap="round" stroke-linejoin="round" d="M14 2.05A9 9 0 0 1 21.95 10H14V2.05Z" />
                    <circle cx="12" cy="12" r="4" fill="var(--chartml-bg, #f4f4f5)" stroke="currentColor" stroke-width="1.5" />
                </svg>
            }.into_any(),
            "table" => view! {
                <svg class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M3.375 19.5h17.25m-17.25 0a1.125 1.125 0 0 1-1.125-1.125M3.375 19.5h7.5c.621 0 1.125-.504 1.125-1.125m-9.75 0V5.625m0 12.75v-1.5c0-.621.504-1.125 1.125-1.125m18.375 2.625V5.625m0 12.75c0 .621-.504 1.125-1.125 1.125m1.125-1.125v-1.5c0-.621-.504-1.125-1.125-1.125m0 3.75h-7.5A1.125 1.125 0 0 1 12 18.375m9.75-12.75c0-.621-.504-1.125-1.125-1.125H3.375c-.621 0-1.125.504-1.125 1.125m19.5 0v1.5c0 .621-.504 1.125-1.125 1.125M2.25 5.625v1.5c0 .621.504 1.125 1.125 1.125m0 0h17.25m-17.25 0h7.5c.621 0 1.125.504 1.125 1.125M3.375 8.25c-.621 0-1.125.504-1.125 1.125v1.5c0 .621.504 1.125 1.125 1.125m17.25-3.75h-7.5c-.621 0-1.125.504-1.125 1.125m8.625-1.125c.621 0 1.125.504 1.125 1.125v1.5c0 .621-.504 1.125-1.125 1.125m-17.25 0h7.5m-7.5 0c-.621 0-1.125.504-1.125 1.125v1.5c0 .621.504 1.125 1.125 1.125M12 10.875v-1.5m0 1.5c0 .621-.504 1.125-1.125 1.125M12 10.875c0 .621.504 1.125 1.125 1.125m-2.25 0c.621 0 1.125.504 1.125 1.125M10.875 12h-7.5m8.625 0h7.5m-7.5 0c-.621 0-1.125.504-1.125 1.125M20.625 12c.621 0 1.125.504 1.125 1.125v1.5c0 .621-.504 1.125-1.125 1.125m-17.25 0h7.5m-7.5 0c-.621 0-1.125.504-1.125 1.125v1.5c0 .621.504 1.125 1.125 1.125M12 13.875v-1.5m0 1.5c0 .621-.504 1.125-1.125 1.125M12 13.875c0 .621.504 1.125 1.125 1.125m-2.25 0c.621 0 1.125.504 1.125 1.125M10.875 15h-7.5" />
                </svg>
            }.into_any(),
            "metric" => view! {
                <svg class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M5.25 8.25h15m-16.5 7.5h15m-1.8-13.5-3.9 19.5m-2.1-19.5-3.9 19.5" />
                </svg>
            }.into_any(),
            _ => view! { <span /> }.into_any(),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format millisecond timestamp as relative time string.
fn format_relative_time(timestamp_ms: f64) -> String {
    let now = js_sys::Date::now();
    let diff_ms = now - timestamp_ms;
    let diff_secs = (diff_ms / 1000.0) as i64;

    if diff_secs < 5 { return "just now".to_string(); }
    if diff_secs < 60 { return format!("{diff_secs}s ago"); }
    let mins = diff_secs / 60;
    if mins < 60 { return format!("{mins}m ago"); }
    let hours = mins / 60;
    if hours < 24 { return format!("{hours}h ago"); }
    let days = hours / 24;
    format!("{days}d ago")
}

/// Set a timeout via web_sys. Returns the timeout ID for cancellation.
fn set_timeout(ms: i32, f: impl FnOnce() + 'static) -> i32 {
    let cb = Closure::once_into_js(f);
    web_sys::window()
        .unwrap()
        .set_timeout_with_callback_and_timeout_and_arguments_0(
            cb.unchecked_ref(),
            ms,
        )
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

/// Chart type selector dropdown.
#[component]
fn ChartTypeSelector(
    current_type: String,
    on_select: Callback<String>,
) -> impl IntoView {
    let (open, set_open) = signal(false);
    let menu_ref = NodeRef::<leptos::html::Div>::new();
    let current = StoredValue::new(current_type);

    // Close on Escape / click-outside — listeners added when open, removed when closed or disposed.
    // SendWrapper is safe: WASM is single-threaded, these closures never cross threads.
    type KeyHandler = SendWrapper<Rc<RefCell<Option<(Closure<dyn Fn(web_sys::KeyboardEvent)>, web_sys::Window)>>>>;
    type ClickHandler = SendWrapper<Rc<RefCell<Option<(Closure<dyn Fn(web_sys::MouseEvent)>, web_sys::Window)>>>>;
    let esc_handler: KeyHandler = SendWrapper::new(Rc::new(RefCell::new(None)));
    let click_handler: ClickHandler = SendWrapper::new(Rc::new(RefCell::new(None)));
    let click_timeout: SendWrapper<Rc<RefCell<Option<i32>>>> = SendWrapper::new(Rc::new(RefCell::new(None)));

    let remove_esc = {
        let h = esc_handler.clone();
        move || { if let Some((cb, win)) = h.borrow_mut().take() {
            let _ = win.remove_event_listener_with_callback("keydown", cb.as_ref().unchecked_ref());
        }}
    };
    let remove_click = {
        let h = click_handler.clone();
        let t = click_timeout.clone();
        move || {
            if let Some(tid) = t.borrow_mut().take() {
                web_sys::window().unwrap().clear_timeout_with_handle(tid);
            }
            if let Some((cb, win)) = h.borrow_mut().take() {
                let _ = win.remove_event_listener_with_callback("click", cb.as_ref().unchecked_ref());
            }
        }
    };

    let remove_esc_e = remove_esc.clone();
    let remove_click_e = remove_click.clone();
    let esc_h = esc_handler.clone();
    let click_h = click_handler.clone();
    let click_t = click_timeout.clone();
    Effect::new(move || {
        if !open.get() { remove_esc_e(); remove_click_e(); return; }
        let window = web_sys::window().unwrap();

        remove_esc_e();
        let esc_cb = Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(move |e: web_sys::KeyboardEvent| {
            if e.key() == "Escape" { set_open.set(false); }
        });
        let _ = window.add_event_listener_with_callback("keydown", esc_cb.as_ref().unchecked_ref());
        *esc_h.borrow_mut() = Some((esc_cb, window.clone()));

        remove_click_e();
        let click_h2 = click_h.clone();
        let win2 = window.clone();
        let tid = set_timeout(0, move || {
            let click_cb = Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
                if let Some(menu) = menu_ref.get() {
                    let target = e.target().and_then(|t| t.dyn_into::<web_sys::Node>().ok());
                    if let Some(t) = target { if !menu.contains(Some(&t)) { set_open.set(false); } }
                }
            });
            let _ = win2.add_event_listener_with_callback("click", click_cb.as_ref().unchecked_ref());
            *click_h2.borrow_mut() = Some((click_cb, win2));
        });
        *click_t.borrow_mut() = Some(tid);
    });

    on_cleanup(move || { remove_esc(); remove_click(); });

    let current_label = CHART_TYPES.iter()
        .find(|(t, _)| *t == current.get_value())
        .map(|(_, l)| *l)
        .unwrap_or("Chart");

    view! {
        <div class="relative" node_ref=menu_ref>
            <button
                class="flex items-center gap-1 px-2 py-1 text-xs font-medium rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
                on:click=move |_| set_open.update(|v| *v = !*v)
            >
                {icons::chart_type_icon(&current.get_value())}
                <span>{current_label}</span>
                <icons::ChevronDownIcon />
            </button>

            <Show when=move || open.get()>
                <div class="absolute left-0 top-full mt-1 w-40 bg-popover border border-border rounded-md shadow-lg z-50 py-1">
                    {CHART_TYPES.iter().map(|(t, label)| {
                        let chart_type = t.to_string();
                        let ct_for_click = chart_type.clone();
                        let ct_for_class = chart_type.clone();
                        let is_active = current.get_value() == chart_type;
                        view! {
                            <button
                                class=if is_active {
                                    "w-full flex items-center gap-2 px-3 py-1.5 text-sm text-foreground bg-accent"
                                } else {
                                    "w-full flex items-center gap-2 px-3 py-1.5 text-sm text-popover-foreground hover:bg-accent"
                                }
                                on:click=move |_| {
                                    on_select.run(ct_for_click.clone());
                                    set_open.set(false);
                                }
                            >
                                {icons::chart_type_icon(&ct_for_class)}
                                <span>{*label}</span>
                            </button>
                        }
                    }).collect_view()}
                </div>
            </Show>
        </div>
    }
}

/// Toggle chip for orientation/mode modifiers.
#[component]
fn ModifierChip(
    label: &'static str,
    active: bool,
    on_toggle: Callback<()>,
) -> impl IntoView {
    let class = if active {
        "px-2 py-0.5 text-xs font-medium rounded-full bg-accent text-foreground border border-border cursor-pointer transition-colors"
    } else {
        "px-2 py-0.5 text-xs font-medium rounded-full text-muted-foreground hover:bg-accent/50 border border-transparent cursor-pointer transition-colors"
    };

    view! {
        <button class=class on:click=move |_| on_toggle.run(())>
            {label}
        </button>
    }
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

/// Chart header bar with title, type selector, action buttons, and overflow menu.
///
/// Used by:
/// - Dashboard viewer (`ChartBlock` in `markdown_renderer.rs`)
/// - MCP chart app (`app.rs`)
#[component]
pub fn ChartHeaderBar(
    /// Chart title (displayed left-aligned).
    #[prop(optional, into)]
    title: Option<String>,

    /// Timestamp (ms since epoch) of last data refresh.
    #[prop(optional, into)]
    last_updated: Option<Signal<Option<f64>>>,

    /// Whether a refresh is in progress (spins the refresh icon).
    #[prop(optional, into)]
    is_refreshing: Option<Signal<bool>>,

    /// Current chart type (bar, line, area, etc.).
    #[prop(optional, into)]
    chart_type: Option<String>,

    /// Current chart orientation (Some("horizontal") or None for vertical).
    #[prop(optional, into)]
    chart_orientation: Option<String>,

    /// Current chart mode (Some("stacked"|"grouped"|"normalized") or None).
    #[prop(optional, into)]
    chart_mode: Option<String>,

    /// Show the chart type selector dropdown.
    #[prop(optional)]
    show_type_selector: bool,

    /// Show the refresh button.
    #[prop(optional)]
    show_refresh: bool,

    // -- Feature flags for menu items --
    #[prop(optional)] show_edit: bool,
    #[prop(optional)] show_delete: bool,
    #[prop(optional)] show_save_to_dashboard: bool,
    #[prop(optional)] show_info: bool,
    #[prop(optional)] show_ask_about: bool,

    // -- Callbacks --
    #[prop(optional)] on_refresh: Option<Callback<()>>,
    #[prop(optional)] on_type_change: Option<Callback<String>>,
    #[prop(optional)] on_orientation_change: Option<Callback<Option<String>>>,
    #[prop(optional)] on_mode_change: Option<Callback<Option<String>>>,
    #[prop(optional)] on_edit: Option<Callback<()>>,
    #[prop(optional)] on_delete: Option<Callback<()>>,
    #[prop(optional)] on_save_to_dashboard: Option<Callback<()>>,
    #[prop(optional)] on_info: Option<Callback<()>>,
    #[prop(optional)] on_ask_about: Option<Callback<()>>,

    /// Slot: content rendered before the title (e.g., logo button, drag handle).
    #[prop(optional)]
    before: Option<Children>,
) -> impl IntoView {
    let (menu_open, set_menu_open) = signal(false);
    let menu_ref = NodeRef::<leptos::html::Div>::new();

    // Store optional values for use in closures
    let ct = StoredValue::new(chart_type.clone());
    let co = StoredValue::new(chart_orientation.clone());
    let cm = StoredValue::new(chart_mode.clone());

    // Determine which modifier chips to show based on chart type
    let show_orientation_chip = chart_type.as_deref() == Some("bar") && show_type_selector;
    let show_mode_chip = matches!(chart_type.as_deref(), Some("bar") | Some("area")) && show_type_selector;

    // Any menu items to show?
    let has_menu_items = show_edit || show_delete || show_save_to_dashboard || show_info || show_ask_about;

    // Close menu on Escape / click-outside
    type MKeyHandler = SendWrapper<Rc<RefCell<Option<(Closure<dyn Fn(web_sys::KeyboardEvent)>, web_sys::Window)>>>>;
    type MClickHandler = SendWrapper<Rc<RefCell<Option<(Closure<dyn Fn(web_sys::MouseEvent)>, web_sys::Window)>>>>;
    let menu_esc_handler: MKeyHandler = SendWrapper::new(Rc::new(RefCell::new(None)));
    let menu_click_handler: MClickHandler = SendWrapper::new(Rc::new(RefCell::new(None)));

    let remove_menu_esc = {
        let h = menu_esc_handler.clone();
        move || { if let Some((cb, win)) = h.borrow_mut().take() {
            let _ = win.remove_event_listener_with_callback("keydown", cb.as_ref().unchecked_ref());
        }}
    };
    let menu_click_timeout: SendWrapper<Rc<RefCell<Option<i32>>>> = SendWrapper::new(Rc::new(RefCell::new(None)));
    let remove_menu_click = {
        let h = menu_click_handler.clone();
        let t = menu_click_timeout.clone();
        move || {
            if let Some(tid) = t.borrow_mut().take() {
                web_sys::window().unwrap().clear_timeout_with_handle(tid);
            }
            if let Some((cb, win)) = h.borrow_mut().take() {
                let _ = win.remove_event_listener_with_callback("click", cb.as_ref().unchecked_ref());
            }
        }
    };

    let remove_menu_esc_e = remove_menu_esc.clone();
    let remove_menu_click_e = remove_menu_click.clone();
    let menu_esc_h = menu_esc_handler.clone();
    let menu_click_h = menu_click_handler.clone();
    let menu_click_t = menu_click_timeout.clone();
    Effect::new(move || {
        if !menu_open.get() { remove_menu_esc_e(); remove_menu_click_e(); return; }
        let window = web_sys::window().unwrap();

        remove_menu_esc_e();
        let esc_cb = Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(move |e: web_sys::KeyboardEvent| {
            if e.key() == "Escape" { set_menu_open.set(false); }
        });
        let _ = window.add_event_listener_with_callback("keydown", esc_cb.as_ref().unchecked_ref());
        *menu_esc_h.borrow_mut() = Some((esc_cb, window.clone()));

        remove_menu_click_e();
        let click_h2 = menu_click_h.clone();
        let win2 = window.clone();
        let tid = set_timeout(0, move || {
            let click_cb = Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
                if let Some(menu) = menu_ref.get() {
                    let target = e.target().and_then(|t| t.dyn_into::<web_sys::Node>().ok());
                    if let Some(t) = target { if !menu.contains(Some(&t)) { set_menu_open.set(false); } }
                }
            });
            let _ = win2.add_event_listener_with_callback("click", click_cb.as_ref().unchecked_ref());
            *click_h2.borrow_mut() = Some((click_cb, win2));
        });
        *menu_click_t.borrow_mut() = Some(tid);
    });

    on_cleanup(move || { remove_menu_esc(); remove_menu_click(); });

    view! {
        <div class="flex items-center justify-between px-4 py-2 bg-muted/50 border-b border-border">
            // ── Left side: before slot + title + type selector + modifier chips ──
            <div class="flex items-center gap-2 min-w-0">
                {before.map(|children| children())}

                {title.map(|t| view! {
                    <span class="text-sm font-medium text-foreground truncate">{t}</span>
                })}

                // Type selector
                {(show_type_selector && ct.get_value().is_some()).then(|| {
                    let current = ct.get_value().unwrap();
                    let on_type = on_type_change.unwrap_or(Callback::new(|_| {}));
                    view! { <ChartTypeSelector current_type=current on_select=on_type /> }
                })}

                // Orientation chip (bar charts)
                {show_orientation_chip.then(|| {
                    let is_horizontal = co.get_value().as_deref() == Some("horizontal");
                    let on_orient = on_orientation_change.unwrap_or(Callback::new(|_| {}));
                    view! {
                        <ModifierChip
                            label="Horizontal"
                            active=is_horizontal
                            on_toggle=Callback::new(move |()| {
                                if is_horizontal {
                                    on_orient.run(None);
                                } else {
                                    on_orient.run(Some("horizontal".to_string()));
                                }
                            })
                        />
                    }
                })}

                // Mode chip (bar: grouped, area: normalized)
                {show_mode_chip.then(|| {
                    let chart_type_val = ct.get_value().unwrap_or_default();
                    let current_mode = cm.get_value();
                    let on_m = on_mode_change.unwrap_or(Callback::new(|_| {}));
                    let (label, mode_value) = if chart_type_val == "area" {
                        ("Normalized", "normalized")
                    } else {
                        ("Grouped", "grouped")
                    };
                    let is_active = current_mode.as_deref() == Some(mode_value);
                    let mode_str = mode_value.to_string();
                    view! {
                        <ModifierChip
                            label=label
                            active=is_active
                            on_toggle=Callback::new(move |()| {
                                if is_active {
                                    on_m.run(None);
                                } else {
                                    on_m.run(Some(mode_str.clone()));
                                }
                            })
                        />
                    }
                })}
            </div>

            // ── Right side: timestamp + refresh + action menu ──
            <div class="flex items-center gap-1 flex-shrink-0">
                // Relative timestamp
                {last_updated.map(|sig| view! {
                    <span class="text-xs text-muted-foreground hidden sm:inline">
                        {move || sig.get().map(format_relative_time).unwrap_or_default()}
                    </span>
                })}

                // Refresh button
                {(show_refresh && on_refresh.is_some()).then(|| {
                    let on_ref = on_refresh.unwrap();
                    let spin_class = move || {
                        let spinning = is_refreshing
                            .map(|s| s.get())
                            .unwrap_or(false);
                        if spinning {
                            "p-1.5 rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors animate-spin"
                        } else {
                            "p-1.5 rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
                        }
                    };
                    view! {
                        <button
                            class=spin_class
                            title="Refresh"
                            on:click=move |_| on_ref.run(())
                        >
                            <icons::RefreshIcon />
                        </button>
                    }
                })}

                // Action overflow menu
                {has_menu_items.then(|| {
                    let edit_cb = StoredValue::new(on_edit);
                    let delete_cb = StoredValue::new(on_delete);
                    let save_cb = StoredValue::new(on_save_to_dashboard);
                    let info_cb = StoredValue::new(on_info);
                    let ask_cb = StoredValue::new(on_ask_about);

                    view! {
                        <div class="relative" node_ref=menu_ref>
                            <button
                                class="p-1.5 rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
                                title="Actions"
                                on:click=move |_| set_menu_open.update(|v| *v = !*v)
                            >
                                <icons::DotsVerticalIcon />
                            </button>

                            <Show when=move || menu_open.get()>
                                <div class="absolute right-0 top-full mt-1 w-48 bg-popover border border-border rounded-md shadow-lg z-50 py-1">
                                    {show_edit.then(|| {
                                        let cb = edit_cb.get_value();
                                        cb.map(|cb| view! {
                                            <button
                                                class="w-full text-left px-3 py-2 text-sm text-popover-foreground hover:bg-accent"
                                                on:click=move |_| { cb.run(()); set_menu_open.set(false); }
                                            >"Edit"</button>
                                        })
                                    })}
                                    {show_save_to_dashboard.then(|| {
                                        let cb = save_cb.get_value();
                                        cb.map(|cb| view! {
                                            <button
                                                class="w-full text-left px-3 py-2 text-sm text-popover-foreground hover:bg-accent"
                                                on:click=move |_| { cb.run(()); set_menu_open.set(false); }
                                            >"Save to Dashboard"</button>
                                        })
                                    })}
                                    {show_ask_about.then(|| {
                                        let cb = ask_cb.get_value();
                                        cb.map(|cb| view! {
                                            <button
                                                class="w-full text-left px-3 py-2 text-sm text-popover-foreground hover:bg-accent"
                                                on:click=move |_| { cb.run(()); set_menu_open.set(false); }
                                            >"Ask About Chart"</button>
                                        })
                                    })}
                                    {show_info.then(|| {
                                        let cb = info_cb.get_value();
                                        cb.map(|cb| view! {
                                            <button
                                                class="w-full text-left px-3 py-2 text-sm text-popover-foreground hover:bg-accent"
                                                on:click=move |_| { cb.run(()); set_menu_open.set(false); }
                                            >"Chart Info"</button>
                                        })
                                    })}
                                    {show_delete.then(|| {
                                        let cb = delete_cb.get_value();
                                        cb.map(|cb| view! {
                                            <button
                                                class="w-full text-left px-3 py-2 text-sm text-destructive hover:bg-destructive/10"
                                                on:click=move |_| { cb.run(()); set_menu_open.set(false); }
                                            >"Delete"</button>
                                        })
                                    })}
                                </div>
                            </Show>
                        </div>
                    }
                })}
            </div>
        </div>
    }
}
