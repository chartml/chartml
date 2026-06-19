#[cfg(target_arch = "wasm32")]
use send_wrapper::SendWrapper;
use std::cell::Cell;
#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
#[cfg(target_arch = "wasm32")]
use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
use chartml_core::element::ElementData;
use chartml_core::params::ParamValues;
use chartml_core::pipeline::{PreparedChart, RenderOptions};
use chartml_core::resolver::Resolver;
use chartml_core::spec::{
    parse as parse_chartml_spec, ChartMLSpec, ChartSpec, Component, DataRef, InlineData,
};
use chartml_core::theme::Theme;

use crate::{CacheBackendRef, ChartMLRef, HooksRef, ProviderRef};
use crate::tooltip::{provide_tooltip_context, DefaultTooltip};
#[cfg(target_arch = "wasm32")]
use crate::tooltip::TooltipState;

/// Custom tooltip renderer type.
pub type TooltipRenderer = Arc<dyn Fn(&ElementData) -> AnyView + Send + Sync>;

#[cfg(target_arch = "wasm32")]
/// Default TTL applied when an `autoRefresh: true` source omits an explicit
/// `cache.ttl`. Mirrors `chartml_core::resolver::DEFAULT_TTL` (5 minutes) so
/// we never spin the auto-refresh interval faster than the cache itself
/// would.
const DEFAULT_AUTO_REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Inject chart CSS into the document head (idempotent — checks for existing style tag).
/// CSS is embedded at compile time from style/chartml.css.
fn inject_chartml_css() {
    #[cfg(target_arch = "wasm32")]
    {
        const CSS: &str = include_str!("../style/chartml.css");
        const CSS_ID: &str = "chartml-injected-styles";
        let document = web_sys::window()
            .expect("window must be available in WASM")
            .document()
            .expect("window must have a document in WASM");
        if document.get_element_by_id(CSS_ID).is_some() {
            return; // Already injected
        }
        let style = document.create_element("style")
            .expect("create_element must succeed for a valid tag name");
        style.set_attribute("id", CSS_ID)
            .expect("set_attribute must succeed for valid attribute names");
        style.set_text_content(Some(CSS));
        document.head()
            .expect("document must have a head element")
            .append_child(&style)
            .expect("append_child must succeed on a valid element");
    }
}

fn extract_yaml_title(yaml: &str) -> Option<String> {
    // Use the real YAML parser rather than scanning lines. The previous
    // line-scan broke out of the loop on the first `data:`/`visualize:` line,
    // so any spec whose `title:` key was ordered after those blocks (e.g.
    // alphabetically-keyed specs) silently lost its title.
    first_chart_spec(yaml)
        .and_then(|s| s.title)
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

/// Build the inline `style` attribute for the HTML `<div class="chart-title">`
/// from a `Theme`. Threads title typography (family, size, weight, style) out
/// of the theme so users can override the chart title font without touching
/// the Leptos component.
///
/// Sentinel discipline: when the theme's title typography fields equal
/// `Theme::default()`, we emit the *legacy* hardcoded values (16px / 600 /
/// no font-family / no font-style) so the default-rendered DOM is byte-
/// identical to pre-Phase-4 output. Phase 2's `Theme::default()` values for
/// title typography (14px / 700) do not match the pre-Phase-4 legacy, so we
/// must bridge via the sentinel rather than pass the defaults through.
/// Anything else is emitted verbatim.
///
/// The `color` property is intentionally left as the CSS-var fallback
/// `var(--chartml-text-strong, #1f2937)` so browser-side dark mode still
/// works via CSS custom properties.
pub(crate) fn build_title_style(theme: &Theme) -> String {
    let default = Theme::default();

    // Legacy sentinel values (match pre-Phase-4 hardcoded HTML).
    const LEGACY_TITLE_FONT_SIZE: &str = "16px";
    const LEGACY_TITLE_FONT_WEIGHT: &str = "600";

    let font_size = if theme.title_font_size == default.title_font_size {
        LEGACY_TITLE_FONT_SIZE.to_string()
    } else if theme.title_font_size.fract() == 0.0 {
        format!("{}px", theme.title_font_size as i64)
    } else {
        format!("{}px", theme.title_font_size)
    };

    let font_weight = if theme.title_font_weight == default.title_font_weight {
        LEGACY_TITLE_FONT_WEIGHT.to_string()
    } else {
        theme.title_font_weight.to_string()
    };

    let mut style = format!(
        "font-size: {}; font-weight: {}; color: var(--chartml-text-strong, #1f2937); margin-bottom: 8px;",
        font_size, font_weight
    );

    // Legacy HTML had no `font-family` — only emit when the theme overrides it.
    if theme.title_font_family != default.title_font_family {
        style.push_str(&format!(" font-family: {};", theme.title_font_family));
    }

    // Same story for `font-style`.
    if theme.title_font_style != default.title_font_style {
        style.push_str(&format!(" font-style: {};", theme.title_font_style));
    }

    style
}

/// Parsed information from a chart spec needed for the auto-refresh loop.
///
/// Phase 4 component scans the spec's `data:` for any source with
/// `cache.autoRefresh: true`, builds one `AutoRefreshSource` per matching
/// entry, then spawns a single interval keyed off the SHORTEST positive TTL
/// across all sources so multiple per-source intervals can't drift apart.
///
/// `name` is the user-chosen source key (or `"source"` for an unnamed flat
/// `data:` block). It's surfaced both to the visibility-listener / tick
/// callback (where it gets logged via `console.debug` for observability) and
/// to host-app tests that want to assert which sources the parser picked up.
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug)]
struct AutoRefreshSource {
    /// User-chosen source name (or `"source"` for unnamed flat data).
    /// Logged on every refresh tick so consumers tailing the browser console
    /// can see which sources are being invalidated.
    name: String,
    /// Inline spec for the source, used to recompute the resolver key on
    /// each tick. Cloning is cheap (small struct of `Option<String>`).
    inline: InlineData,
    /// Parsed TTL. We parse `cache.ttl` once on collection rather than on
    /// every tick.
    ttl: Duration,
}

#[cfg(target_arch = "wasm32")]
/// Scan a parsed `ChartSpec.data` for sources whose cache config requests
/// auto-refresh. Both flat `Inline { cache: Some(...) }` and per-entry
/// `NamedMap` sources are inspected; `Named` (string ref to a pre-registered
/// source) cannot have a cache config and is skipped.
fn collect_auto_refresh_sources(spec: &ChartSpec) -> Vec<AutoRefreshSource> {
    let mut out = Vec::new();
    match &spec.data {
        DataRef::Inline(inline) => {
            if let Some(src) = auto_refresh_from_inline("source".to_string(), inline) {
                out.push(src);
            }
        }
        DataRef::NamedMap(map) => {
            for (name, inline) in map {
                if let Some(src) = auto_refresh_from_inline(name.clone(), inline) {
                    out.push(src);
                }
            }
        }
        DataRef::Named(_) => {
            // Pre-registered named source — no cache config attached, no
            // auto-refresh applicable. The host app should re-register the
            // table directly when its data changes.
        }
    }
    out
}

#[cfg(target_arch = "wasm32")]
/// Build an `AutoRefreshSource` from one inline spec, returning `None`
/// unless `cache.autoRefresh == true`. Falls back to
/// [`DEFAULT_AUTO_REFRESH_INTERVAL`] when `cache.ttl` is missing — silently
/// dropping `autoRefresh: true` because the user forgot a TTL would be a
/// nasty surprise. Malformed `cache.ttl` strings ALSO fall back rather than
/// erroring (the spec parses cleanly and the resolver will surface the same
/// error during fetch).
fn auto_refresh_from_inline(name: String, inline: &InlineData) -> Option<AutoRefreshSource> {
    let cache = inline.cache.as_ref()?;
    if cache.auto_refresh != Some(true) {
        return None;
    }
    let ttl = cache
        .ttl
        .as_deref()
        .and_then(|s| humantime::parse_duration(s).ok())
        .unwrap_or(DEFAULT_AUTO_REFRESH_INTERVAL);
    Some(AutoRefreshSource {
        name,
        inline: inline.clone(),
        ttl,
    })
}

#[cfg(target_arch = "wasm32")]
/// Pick the shortest positive TTL across the auto-refresh sources, or
/// `None` when there are no sources or every TTL is zero.
fn shortest_interval(sources: &[AutoRefreshSource]) -> Option<Duration> {
    sources
        .iter()
        .map(|s| s.ttl)
        .filter(|d| !d.is_zero())
        .min()
}

/// Collect every `InlineData` the parsed chart spec declares, so the
/// imperative `refresh_trigger` effect can compute resolver keys for ALL
/// sources (not just auto-refresh ones). Pre-registered named sources
/// (`DataRef::Named`) are skipped because the resolver's
/// `invalidate*` API operates on keys derived from inline shapes — host
/// apps wanting to invalidate a registered named source should call the
/// resolver's bulk APIs (`invalidate_by_slug`, `invalidate_by_namespace`)
/// directly from the parent component.
fn collect_invalidatable_sources(spec: &ChartSpec) -> Vec<InlineData> {
    let mut out = Vec::new();
    match &spec.data {
        DataRef::Inline(inline) => out.push(inline.clone()),
        DataRef::NamedMap(map) => {
            for (_name, inline) in map {
                out.push(inline.clone());
            }
        }
        DataRef::Named(_) => {
            // Pre-registered string-ref source — see fn-level docs for
            // why we don't try to invalidate these here.
        }
    }
    out
}

/// Extract the FIRST chart spec from a YAML string. Returns `None` for
/// invalid YAML, multi-document specs without any chart, or specs that
/// only declare params/sources (the auto-refresh wiring needs a `ChartSpec`
/// to scan `.data` against). Errors during parsing are silently swallowed
/// because the chart's main render path will surface the same error in the
/// view layer with full context — the auto-refresh path doesn't have a UI
/// surface to bubble parse errors into.
fn first_chart_spec(yaml: &str) -> Option<ChartSpec> {
    let parsed = parse_chartml_spec(yaml).ok()?;
    match parsed {
        ChartMLSpec::Single(component) => match *component {
            Component::Chart(chart) => Some(*chart),
            _ => None,
        },
        ChartMLSpec::Array(components) => components.into_iter().find_map(|c| match c {
            Component::Chart(chart) => Some(*chart),
            _ => None,
        }),
    }
}

/// Internal representation of a successful render. The `prepared` chart is
/// cached alongside the rendered SVG so resize-only re-renders skip the
/// fetch + transform stages entirely (the spec calls this out explicitly:
/// "Resize: ... no re-fetch on resize").
#[derive(Clone)]
struct ResolvedChart {
    prepared: PreparedChart,
    svg: String,
    width: f64,
}

/// Phase 4 props for [`ChartMLChart`]. Held in one struct so the docs stay
/// close to the prop attributes and the new optional inputs are easy to
/// spot among the legacy ones.
///
/// Main ChartML component for Leptos.
///
/// Renders a ChartML YAML spec reactively. Responds to:
/// - `spec` signal changes (YAML editing) — re-runs fetch + transform.
/// - `param_values` signal changes (interactive param controls) — re-runs.
/// - `refresh_count` signal increments (manual + auto-refresh) — re-runs.
/// - Container resize (via ResizeObserver) — re-renders synchronously
///   from the cached `PreparedChart`, no re-fetch.
///
/// Phase 4 additions:
/// - `provider`: an optional [`ProviderRef`] registered on the inner
///   `ChartML` instance under the `"datasource"` slug. Falls back to
///   Leptos context when not supplied; props win over context.
/// - `cache_backend`: an optional [`CacheBackendRef`] swapped in as the
///   tier-1 cache. Falls back to context.
/// - `hooks`: an optional [`HooksRef`] installed on the resolver.
///   Falls back to context.
/// - `refresh_trigger`: an optional `Signal<u32>` that, when its value
///   changes, invalidates every spec source's resolver key and forces a
///   re-fetch — the imperative equivalent of the internal `Retry` button.
///   Pair with a `RwSignal<u32>` parent-side and `set.update(|c| *c += 1)`
///   to drive a custom "Refresh" button. Auto-refresh handles the timer
///   case; this is for manual user-driven refresh control.
/// - Auto-refresh: when the parsed spec contains a source with
///   `cache.autoRefresh: true`, a Leptos interval is spawned on mount.
///   The interval is paused when the document's `visibilityState` is
///   `"hidden"` and re-armed when it becomes `"visible"`.
#[component]
pub fn ChartMLChart(
    /// ChartML YAML specification string
    #[prop(into)]
    spec: Signal<String>,
    /// Pre-configured ChartML instance
    chartml: ChartMLRef,
    /// Optional CSS class for the container
    #[prop(optional)]
    class: &'static str,
    /// Optional custom tooltip renderer
    #[prop(optional)]
    tooltip: Option<TooltipRenderer>,
    /// Shared reactive param values — when updated by controls, charts re-render
    #[prop(optional)]
    param_values: Option<RwSignal<ParamValues>>,
    /// Provider for `data: { datasource, query }` shapes. Registered on the
    /// inner `ChartML` instance under the `"datasource"` slug. Falls back
    /// to `use_context::<ProviderRef>()` when not supplied.
    #[prop(optional, into)]
    provider: Option<ProviderRef>,
    /// Optional persistent cache backend. Replaces the tier-1 in-memory
    /// cache (default `MemoryBackend`). Falls back to
    /// `use_context::<CacheBackendRef>()` when not supplied.
    #[prop(optional, into)]
    cache_backend: Option<CacheBackendRef>,
    /// Optional resolver hooks for observability. Falls back to
    /// `use_context::<HooksRef>()` when not supplied.
    #[prop(optional, into)]
    hooks: Option<HooksRef>,
    /// Optional imperative refresh trigger — when the wrapped `u32` value
    /// changes, the chart invalidates every spec source's resolver cache
    /// key (across both tier-1 and tier-2) and re-runs the fetch /
    /// transform / render pipeline against the current YAML.
    ///
    /// Functionally equivalent to clicking the chart's internal `Retry`
    /// button; use this when the parent owns its own `Refresh` UI (e.g. a
    /// dashboard "Refresh now" button shared across many charts) and
    /// doesn't want to round-trip through a YAML mutation. Auto-refresh
    /// (the `cache.autoRefresh: true` path) covers the timer case
    /// independently — this prop is purely for parent-driven manual
    /// refresh.
    ///
    /// Pair with a parent-side `RwSignal<u32>`:
    /// ```ignore
    /// let refresh = RwSignal::new(0_u32);
    /// view! {
    ///     <button on:click=move |_| refresh.update(|c| *c += 1)>"Refresh"</button>
    ///     <ChartMLChart spec chartml refresh_trigger=Some(refresh.into()) />
    /// }
    /// ```
    #[prop(optional, into)]
    refresh_trigger: Option<Signal<u32>>,
    /// Fires after a successful fetch + transform + render with the
    /// `refreshed_at` timestamp (ms since epoch). On cache hits this
    /// reflects when the data was originally fetched from the server,
    /// not the current time.
    #[prop(optional, into)]
    on_refreshed: Option<Callback<f64>>,
) -> impl IntoView {
    let chartml = chartml.clone();
    let tooltip_state = provide_tooltip_context();

    // Inject chart CSS into document head on first mount (idempotent)
    inject_chartml_css();

    // Prop-vs-context resolution: explicit prop wins, then Leptos context.
    // Doing the lookup here once (rather than inside an effect) means a
    // context hand-off after mount is intentionally NOT picked up — props
    // and context are read at component construction time. Re-mounting
    // (e.g. via a `Show` wrapper) re-runs this resolution.
    let provider = provider.or_else(use_context::<ProviderRef>);
    let cache_backend = cache_backend.or_else(use_context::<CacheBackendRef>);
    let hooks = hooks.or_else(use_context::<HooksRef>);

    // Wire the resolver-side configuration. The resolver's `register_provider`
    // / `set_primary_cache` / `set_hooks` methods all use interior mutability
    // so we don't need `&mut chartml` — the `ChartMLRef` (`Arc<ChartML>` /
    // `Rc<ChartML>`) shares fine. Re-running this on every `ChartMLChart`
    // mount is intentional: the host can swap providers between mounts
    // without rebuilding the `ChartML` instance.
    {
        let resolver = chartml.resolver();
        if let Some(p) = provider.as_ref() {
            resolver.register_provider("datasource", p.clone());
        }
        if let Some(b) = cache_backend.as_ref() {
            resolver.set_primary_cache(b.clone());
        }
        if let Some(h) = hooks.as_ref() {
            resolver.set_hooks(h.clone());
        }
    }

    // Track container width — updated by ResizeObserver (wasm32 only).
    // The read half (`container_width`) drives the resize re-render effect
    // on both targets; the write half is only used by the ResizeObserver
    // block which is gated behind `#[cfg(target_arch = "wasm32")]`.
    let (container_width, set_container_width) = signal(0.0_f64);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = set_container_width;
    let container_ref = NodeRef::<leptos::html::Div>::new();

    // Refresh counter — bumped by manual refresh button, auto-refresh
    // interval, and the optional external `refresh_trigger` prop. Feeds
    // the Resource's input tuple so increments trigger a fresh fetch +
    // transform pass (resize alone does NOT bump this).
    let refresh_count = RwSignal::new(0_u32);

    // External refresh trigger wiring. When the parent supplies a
    // `refresh_trigger` signal and its value changes, we invalidate every
    // spec source's resolver cache key (mirroring what the auto-refresh
    // interval does on each tick) and bump `refresh_count` to drive the
    // main fetch effect. The first run is skipped because the initial
    // mount already triggers a fetch via the main effect — re-running for
    // the initial value would double-fetch on first paint.
    if let Some(trigger) = refresh_trigger {
        let chartml_for_trigger = chartml.clone();
        // `Cell<bool>` is fine here — this effect runs strictly on the
        // single-threaded reactive owner. `Cell` keeps the closure
        // `Send`-bound-clean in case Leptos's effect bound tightens.
        let initial_seen = Rc::new(Cell::new(false));
        let initial_seen_for_effect = initial_seen.clone();
        Effect::new(move || {
            // Subscribe to the trigger so future increments re-run this
            // closure. The value itself isn't used — only the side
            // effect of bumping `refresh_count` matters.
            trigger.try_get();

            if !initial_seen_for_effect.get() {
                initial_seen_for_effect.set(true);
                return;
            }

            // Invalidate every source the parsed spec declares. We can't
            // know what `namespace` the inner ChartML was configured with
            // (it's set on the instance, not threaded into the component),
            // so this fires `None` — matching the auto-refresh interval
            // wiring above. Multi-tenant deployments that need namespaced
            // invalidation can call `chartml.resolver().invalidate_by_namespace(...)`
            // directly from the parent and skip this prop.
            let yaml = spec.get_untracked();
            if let Some(parsed) = first_chart_spec(&yaml) {
                let resolver = chartml_for_trigger.resolver();
                for source in collect_invalidatable_sources(&parsed) {
                    let key = Resolver::key_for(&source, None);
                    let resolver = resolver.clone();
                    spawn_local(async move {
                        resolver.invalidate(key).await;
                    });
                }
            }

            refresh_count.update(|c| *c = c.wrapping_add(1));
        });
    }

    // Set up ResizeObserver after mount; disconnect on disposal.
    // Debounce: only update container_width after resize activity stops for 200ms.
    // Matches the markdown-react plugin pattern (250ms debounce + initial grace).
    #[cfg(target_arch = "wasm32")]
    {
        type RoEntry = SendWrapper<Rc<RefCell<Option<(web_sys::ResizeObserver, Closure<dyn Fn(js_sys::Array)>)>>>>;
        let resize_observer: RoEntry = SendWrapper::new(Rc::new(RefCell::new(None)));
        let debounce_handle: SendWrapper<Rc<RefCell<Option<i32>>>> = SendWrapper::new(Rc::new(RefCell::new(None)));

        let ro_clone = resize_observer.clone();
        let debounce_clone = debounce_handle.clone();
        Effect::new(move || {
            if let Some(el) = container_ref.get() {
                // Set initial width immediately (no debounce for first measurement)
                let width = el.client_width() as f64;
                if width > 0.0 {
                    set_container_width.set(width);
                }

                let dh = debounce_clone.clone();
                let cb = Closure::<dyn Fn(js_sys::Array)>::new(move |entries: js_sys::Array| {
                    if let Some(entry) = entries.get(0).dyn_ref::<web_sys::ResizeObserverEntry>() {
                        let rect = entry.content_rect();
                        let w = rect.width();
                        if w > 0.0 {
                            // Cancel any pending debounce
                            if let Some(tid) = dh.borrow_mut().take() {
                                web_sys::window()
                                    .expect("window must be available in WASM")
                                    .clear_timeout_with_handle(tid);
                            }
                            // Set new debounced timeout
                            let cb_js = Closure::once_into_js(move || {
                                set_container_width.set(w);
                            });
                            let tid = web_sys::window()
                                .expect("window must be available in WASM")
                                .set_timeout_with_callback_and_timeout_and_arguments_0(
                                    cb_js.unchecked_ref(), 200,
                                ).unwrap_or(0);
                            *dh.borrow_mut() = Some(tid);
                        }
                    }
                });

                if let Ok(observer) = web_sys::ResizeObserver::new(cb.as_ref().unchecked_ref()) {
                    observer.observe(&el);
                    *ro_clone.borrow_mut() = Some((observer, cb));
                }
            }
        });

        on_cleanup(move || {
            if let Some((observer, _cb)) = resize_observer.borrow_mut().take() {
                observer.disconnect();
            }
            if let Some(tid) = debounce_handle.borrow_mut().take() {
                web_sys::window()
                    .expect("window must be available in WASM")
                    .clear_timeout_with_handle(tid);
            }
        });
    }

    // ── SVG tooltip delegation ───────────────────────────────────────────
    //
    // The SVG is injected as a raw string via `inner_html`, so Leptos never
    // attaches per-element event handlers.  Instead we add a single pair of
    // delegated listeners (pointermove / pointerleave) to the outer container
    // div — already mounted in the DOM via `container_ref` — and walk upward
    // from the event target to find the nearest element that carries
    // `data-label` (written by `chartml_core::svg::write_data_attrs`).
    //
    // Lifecycle: the closures are stored in an `Rc<RefCell<...>>` cell so
    // `on_cleanup` can remove them from the DOM when the component unmounts.
    // Re-mounting a fresh `ChartMLChart` re-runs this `Effect::new` block and
    // reinstalls fresh closures against the new container element.
    #[cfg(target_arch = "wasm32")]
    {
        type TooltipListeners = SendWrapper<Rc<RefCell<Option<(
            web_sys::EventTarget,
            Closure<dyn Fn(web_sys::PointerEvent)>,
            Closure<dyn Fn(web_sys::PointerEvent)>,
        )>>>>;

        let listeners: TooltipListeners =
            SendWrapper::new(Rc::new(RefCell::new(None)));
        let listeners_for_effect = listeners.clone();
        let listeners_for_cleanup = listeners.clone();

        Effect::new(move || {
            let Some(el) = container_ref.get() else { return };

            // If we already installed listeners for this element, skip.
            if listeners_for_effect.borrow().is_some() {
                return;
            }

            let target: web_sys::EventTarget = el.clone().into();

            // pointermove — find the nearest ancestor-or-self with data-label
            // inside the SVG host and populate the tooltip signal.
            let tooltip_state_for_move = tooltip_state;
            let on_move = Closure::<dyn Fn(web_sys::PointerEvent)>::new(
                move |evt: web_sys::PointerEvent| {
                    // Walk up from the event target through the DOM until we
                    // find an element with `data-label`, or we leave the SVG.
                    let mut current: Option<web_sys::Element> =
                        evt.target()
                           .and_then(|t| t.dyn_into::<web_sys::Element>().ok());

                    let mut found_label: Option<String> = None;
                    let mut found_value: Option<String> = None;
                    let mut found_series: Option<String> = None;

                    while let Some(ref el) = current {
                        // Stop at the svg-host boundary — don't pick up labels
                        // from the outer chartml-container itself.
                        let class_name = el.get_attribute("class").unwrap_or_default();
                        if class_name.contains("chartml-svg-host") {
                            break;
                        }

                        let label = el.get_attribute("data-label");
                        if label.is_some() {
                            found_label = label;
                            found_value = el.get_attribute("data-value");
                            found_series = el.get_attribute("data-series");
                            break;
                        }

                        current = el.parent_element();
                    }

                    if let Some(label) = found_label {
                        let value = found_value.unwrap_or_default();
                        let mut data = ElementData::new(label, value);
                        if let Some(series) = found_series {
                            data = data.with_series(series);
                        }
                        // Use clientX/clientY (viewport coords) so the
                        // `position: fixed` tooltip renders in the right spot.
                        tooltip_state_for_move.set(TooltipState::show(
                            data,
                            evt.client_x() as f64,
                            evt.client_y() as f64,
                        ));
                    } else {
                        tooltip_state_for_move.set(TooltipState::hide());
                    }
                },
            );

            // pointerleave — clear the tooltip when the mouse leaves the
            // chart container entirely.
            let tooltip_state_for_leave = tooltip_state;
            let on_leave = Closure::<dyn Fn(web_sys::PointerEvent)>::new(
                move |_evt: web_sys::PointerEvent| {
                    tooltip_state_for_leave.set(TooltipState::hide());
                },
            );

            let _ = target.add_event_listener_with_callback(
                "pointermove",
                on_move.as_ref().unchecked_ref(),
            );
            let _ = target.add_event_listener_with_callback(
                "pointerleave",
                on_leave.as_ref().unchecked_ref(),
            );

            *listeners_for_effect.borrow_mut() = Some((target, on_move, on_leave));
        });

        on_cleanup(move || {
            if let Some((target, on_move, on_leave)) =
                listeners_for_cleanup.borrow_mut().take()
            {
                let _ = target.remove_event_listener_with_callback(
                    "pointermove",
                    on_move.as_ref().unchecked_ref(),
                );
                let _ = target.remove_event_listener_with_callback(
                    "pointerleave",
                    on_leave.as_ref().unchecked_ref(),
                );
            }
        });
    }

    let container_class = if class.is_empty() {
        "chartml-container".to_string()
    } else {
        format!("chartml-container {}", class)
    };

    // Cached `ResolvedChart` — produced asynchronously from the resource
    // pipeline, then re-rendered synchronously on resize. `Option` until
    // the first successful pass completes; cleared back to `None` whenever
    // the resource transitions through loading/error.
    let resolved: RwSignal<Option<ResolvedChart>> = RwSignal::new(None);
    let last_error: RwSignal<Option<String>> = RwSignal::new(None);
    let is_loading: RwSignal<bool> = RwSignal::new(false);
    // Wall-clock timestamp (ms since unix epoch) of the last successful
    // resolve. Surfaced in the demo's "last refreshed at" indicator;
    // exposed as an attribute on the container for tests + telemetry.
    let last_refreshed_ms: RwSignal<f64> = RwSignal::new(0.0);

    // Pull a snapshot of the chart's title from the YAML synchronously so
    // the title can render before the async pipeline completes.
    let title_signal = Memo::new(move |_| extract_yaml_title(&spec.try_get().unwrap_or_default()));

    // Main fetch + transform effect. Reactive on `(spec, params, refresh_count)`.
    // Runs the new chartml-5 pipeline (`fetch` → `transform` →
    // `render_prepared_to_svg`) and writes results to `resolved` /
    // `last_error` / `is_loading`. Width changes do NOT bump this — width
    // is only consumed when synchronously re-rendering from a cached
    // `PreparedChart` below.
    {
        let chartml = chartml.clone();
        let render_gen: Rc<Cell<u32>> = Rc::new(Cell::new(0));
        let render_gen_for_effect = render_gen.clone();
        Effect::new(move || {
            let Some(yaml) = spec.try_get() else { return };
            // Subscribing to refresh_count without using its value — only
            // the side effect (re-running this closure) matters.
            refresh_count.try_get();
            let params = param_values.and_then(|pv| pv.try_get());

            if yaml.trim().is_empty() {
                resolved.set(None);
                last_error.set(Some("Enter a ChartML YAML spec".to_string()));
                is_loading.set(false);
                return;
            }

            // Bump the generation counter so a stale in-flight fetch can't
            // overwrite a fresher one when it eventually resolves.
            let my_gen = render_gen_for_effect.get().wrapping_add(1);
            render_gen_for_effect.set(my_gen);
            let gen_ref = render_gen_for_effect.clone();

            is_loading.set(true);
            last_error.set(None);

            let chartml_async = chartml.clone();
            let yaml_owned = yaml.clone();
            let params_owned = params.clone();
            // Snapshot the current width so the FIRST render after a fetch
            // uses the same dimensions the user is currently looking at.
            // Subsequent resize-only changes flow through the resize effect
            // below (which calls `render_prepared_to_svg` synchronously).
            let initial_width = container_width.get_untracked();

            spawn_local(async move {
                let opts = RenderOptions {
                    width: if initial_width > 0.0 { Some(initial_width) } else { None },
                    height: None,
                    params: params_owned,
                };
                let fetch_result = chartml_async.fetch(&yaml_owned, &opts).await;
                if gen_ref.get() != my_gen { return; }
                let fetched = match fetch_result {
                    Ok(f) => f,
                    Err(err) => {
                        resolved.set(None);
                        last_error.set(Some(format!("{}", err)));
                        is_loading.set(false);
                        return;
                    }
                };
                let transform_result = chartml_async.transform(fetched, &opts).await;
                if gen_ref.get() != my_gen { return; }
                let prepared = match transform_result {
                    Ok(p) => p,
                    Err(err) => {
                        resolved.set(None);
                        last_error.set(Some(format!("{}", err)));
                        is_loading.set(false);
                        return;
                    }
                };
                let refreshed_at_ms = {
                    let age = prepared.metadata.refreshed_at.elapsed()
                        .unwrap_or(std::time::Duration::ZERO);
                    now_ms() - (age.as_millis() as f64)
                };
                let svg = match chartml_async.render_prepared_to_svg(&prepared, &opts) {
                    Ok(s) => s,
                    Err(err) => {
                        resolved.set(None);
                        last_error.set(Some(format!("{}", err)));
                        is_loading.set(false);
                        return;
                    }
                };
                resolved.set(Some(ResolvedChart {
                    prepared,
                    svg,
                    width: initial_width,
                }));
                last_error.set(None);
                is_loading.set(false);
                last_refreshed_ms.set(refreshed_at_ms);
                if let Some(cb) = on_refreshed {
                    cb.try_run(refreshed_at_ms);
                }
            });
        });
    }

    // Resize-only re-render effect. Reactive on `container_width` AND
    // `resolved`. When width changes (or a fresh prepared chart arrives),
    // re-runs `render_prepared_to_svg` synchronously to produce a new SVG
    // string sized to the current container width. Skips the work when
    // the cached width already matches the current container width.
    {
        let chartml = chartml.clone();
        Effect::new(move || {
            let Some(width) = container_width.try_get() else { return };
            if width <= 0.0 { return; }
            let Some(current) = resolved.try_get().flatten() else { return; };
            if (current.width - width).abs() < 0.5 {
                // Already rendered at this width (within sub-pixel tolerance).
                return;
            }
            let opts = RenderOptions::with_size(Some(width), None);
            match chartml.render_prepared_to_svg(&current.prepared, &opts) {
                Ok(svg) => {
                    resolved.set(Some(ResolvedChart {
                        prepared: current.prepared,
                        svg,
                        width,
                    }));
                }
                Err(err) => {
                    last_error.set(Some(format!("{}", err)));
                }
            }
        });
    }

    // Auto-refresh wiring. Reactive on `spec` only so that editing the
    // YAML re-evaluates which sources need auto-refresh. Manual refresh
    // (the "Retry" / "Refresh now" buttons that bump `refresh_count`)
    // does NOT reset the interval phase — see the explanatory comment
    // inside the effect for why.
    //
    // The interval handle and visibility-listener closure are stored in
    // `Rc<RefCell<...>>` cells so the `on_cleanup` hook can drop them when
    // the component unmounts. We also reset them when the spec changes
    // (the inner effect clears the previous interval before installing
    // a new one).
    #[cfg(target_arch = "wasm32")]
    {
        let chartml_for_refresh = chartml.clone();
        // Holders for the active interval handle and visibility listener.
        // Both are `SendWrapper<Rc<RefCell<...>>>` so they satisfy Leptos's
        // `Send + 'static` reactive bound on wasm32-unknown-unknown
        // (single-threaded; the wrapper is a noop guard).
        type Holders = SendWrapper<Rc<RefCell<AutoRefreshState>>>;
        let holders: Holders = SendWrapper::new(Rc::new(RefCell::new(AutoRefreshState {
            interval: None,
            visibility_listener: None,
            sources: Vec::new(),
            tick_period: Duration::ZERO,
        })));

        let holders_for_effect = holders.clone();
        Effect::new(move || {
            let Some(yaml) = spec.try_get() else { return };
            // Auto-refresh setup is driven by spec changes only — we
            // intentionally do NOT subscribe to `refresh_count` here.
            //
            // The interval tick callback (see `install_auto_refresh_interval`)
            // bumps `refresh_count` to drive the main fetch effect. If this
            // effect also subscribed to `refresh_count`, every tick would
            // tear the interval down and re-create it (re-parsing YAML and
            // re-registering the visibility listener each time), so the
            // interval would never be stable.
            //
            // Trade-off: manual "Retry" / "Refresh now" no longer resets the
            // auto-refresh interval phase. This matches the behavior most
            // users expect — the cadence is set by the spec's `cache.ttl`
            // and a manual refresh just slots in alongside it rather than
            // restarting the clock.

            // Reset previous timer + listener — every spec edit re-arms.
            clear_auto_refresh(&holders_for_effect);

            let Some(parsed_spec) = first_chart_spec(&yaml) else {
                return;
            };
            let sources = collect_auto_refresh_sources(&parsed_spec);
            if sources.is_empty() {
                return;
            }
            let Some(period) = shortest_interval(&sources) else {
                return;
            };

            // Cache the snapshot inside the holders so the interval
            // closure (plus the visibility listener that re-arms it) can
            // both read it. `tick_period` is what the listener uses when
            // re-arming after a "hidden → visible" transition.
            holders_for_effect.borrow_mut().sources = sources.clone();
            holders_for_effect.borrow_mut().tick_period = period;

            // Helper builds and installs the interval. Called both
            // initially (here) and from the visibility listener when the
            // tab becomes visible again. The closure invalidates every
            // tracked source's resolver key, then bumps `refresh_count`
            // which triggers the main fetch effect.
            install_auto_refresh_interval(&chartml_for_refresh, &holders_for_effect, refresh_count);

            // Install the visibility listener once. If `document` is
            // unavailable (SSR), skip — auto-refresh is browser-only.
            install_visibility_listener(
                &chartml_for_refresh,
                &holders_for_effect,
                refresh_count,
            );
        });

        // Drop both the interval and the visibility listener on unmount.
        // SendWrapper drop on the wrong thread would panic, but
        // wasm32-unknown-unknown is single-threaded, so we're safe.
        let holders_for_cleanup = holders.clone();
        on_cleanup(move || {
            clear_auto_refresh(&holders_for_cleanup);
        });
    }

    // Build the title style string once from the theme. The theme is set on
    // the ChartML instance before it's handed to this component and does not
    // change reactively, so a one-shot computation is correct here.
    let title_style = build_title_style(chartml.theme());

    view! {
        <div
            class=container_class
            style="position: relative;"
            node_ref=container_ref
            data-last-refreshed-ms=move || {
                let ts = last_refreshed_ms.try_get().unwrap_or(0.0);
                if ts > 0.0 { Some(format!("{}", ts)) } else { None }
            }
        >
            // Chart title (extracted synchronously from the YAML so it
            // shows immediately, even before the async pipeline finishes).
            {
                let title_style = title_style.clone();
                move || {
                    let title_style = title_style.clone();
                    title_signal.try_get().flatten().map(|t| view! {
                        <div class="chart-title" style=title_style>
                            {t}
                        </div>
                    })
                }
            }

            // Chart content — the SVG string from `render_prepared_to_svg`,
            // injected via `inner_html` so the renderer's typography /
            // animation attributes survive. Re-runs synchronously when
            // width changes (resize) and after every successful resource
            // resolution.
            {move || {
                resolved.try_get().flatten().map(|r| view! {
                    <div class="chartml-svg-host" inner_html=r.svg></div>
                })
            }}

            // Error display + retry button. The retry button bumps
            // `refresh_count`, which the main fetch effect subscribes to
            // and re-runs the pipeline against the current YAML.
            {move || {
                last_error.try_get().flatten().map(|msg| view! {
                    <div class="chartml-error" role="alert">
                        <p style="color: #dc3545; font-family: monospace; padding: 12px; background: #fff5f5; border: 1px solid #dc3545; border-radius: 4px;">
                            {msg}
                        </p>
                        <button
                            class="chartml-retry-button"
                            type="button"
                            on:click=move |_| { refresh_count.update(|c| *c = c.wrapping_add(1)); }
                        >
                            "Retry"
                        </button>
                    </div>
                })
            }}

            // Loading state — shown whenever the async pipeline is in
            // flight, including during a re-fetch triggered by manual /
            // auto refresh (so the user gets feedback even when an old
            // SVG is still on screen).
            {move || {
                is_loading.try_get().unwrap_or(false).then(|| view! {
                    <div class="chartml-loading">
                        <div class="chartml-spinner" />
                    </div>
                })
            }}

            // Tooltip overlay — populated by the delegated pointermove/
            // pointerleave listeners installed on the container (see the
            // "SVG tooltip delegation" block above).  `tooltip_state` is
            // written with viewport-relative clientX/clientY so the
            // `position: fixed` tooltip renders at the correct screen
            // position regardless of scroll or container transforms.
            {
                let tooltip = tooltip.clone();
                move || {
                    let state = tooltip_state.try_get().unwrap_or_default();
                    if !state.visible() {
                        return view! { <div style="display:none;" /> }.into_any();
                    }

                    let style = format!(
                        "position: fixed; left: {}px; top: {}px; pointer-events: none; z-index: 1000;",
                        state.x + 12.0,
                        state.y - 12.0,
                    );

                    let content = if let Some(ref renderer) = tooltip {
                        renderer(state.data.as_ref().expect("tooltip data is Some when state is visible"))
                    } else {
                        view! { <DefaultTooltip state=state.clone() /> }.into_any()
                    };

                    view! {
                        <div class="chartml-tooltip" style=style>
                            {content}
                        </div>
                    }.into_any()
                }
            }
        </div>
    }
}

#[cfg(target_arch = "wasm32")]
/// Tear down the active interval + visibility listener. Safe to call
/// multiple times — every field is `Option<...>` and `take()`-cleared.
fn clear_auto_refresh(holders: &SendWrapper<Rc<RefCell<AutoRefreshState>>>) {
    let mut state = holders.borrow_mut();
    if let Some(handle) = state.interval.take() {
        handle.clear();
    }
    if let Some((target, listener)) = state.visibility_listener.take() {
        let _ = target.remove_event_listener_with_callback(
            "visibilitychange",
            listener.as_ref().unchecked_ref(),
        );
        // `listener` drops here, freeing the JS closure.
    }
}

#[cfg(target_arch = "wasm32")]
/// Install the interval that runs auto-refresh ticks. Called from the
/// auto-refresh effect on initial setup, and from the visibility listener
/// when the document goes hidden → visible. Replaces any previously
/// installed interval (the caller must have cleared it first; we check via
/// `is_some`).
fn install_auto_refresh_interval(
    chartml: &ChartMLRef,
    holders: &SendWrapper<Rc<RefCell<AutoRefreshState>>>,
    refresh_count: RwSignal<u32>,
) {
    let (period, sources) = {
        let state = holders.borrow();
        if state.interval.is_some() {
            return;
        }
        (state.tick_period, state.sources.clone())
    };
    if period.is_zero() || sources.is_empty() {
        return;
    }

    let chartml_for_tick = chartml.clone();
    let cb = move || {
        // Hidden tabs are an extra safety net: even if the visibility
        // listener races the very-first tick we still skip refreshing
        // when the document isn't visible.
        if document_is_hidden() {
            return;
        }
        let resolver = chartml_for_tick.resolver();
        let namespace = None::<String>;
        for src in &sources {
            // Log per-source invalidation to the browser console so devs
            // tailing the inspector can confirm the auto-refresh wiring
            // fires on the cadence they expect. Surfaces the user-chosen
            // source name rather than the opaque numeric resolver key.
            log_auto_refresh_tick(&src.name);
            let key = Resolver::key_for(&src.inline, namespace.as_deref());
            let resolver = resolver.clone();
            spawn_local(async move {
                resolver.invalidate(key).await;
            });
        }
        refresh_count.update(|c| *c = c.wrapping_add(1));
    };

    if let Ok(handle) = leptos::prelude::set_interval_with_handle(cb, period) {
        holders.borrow_mut().interval = Some(handle);
    }
}

#[cfg(target_arch = "wasm32")]
/// Install the `visibilitychange` listener. Pauses the interval when the
/// document is hidden and re-arms it when it becomes visible. Skipped if
/// `web_sys::window().document()` is unavailable (SSR or detached doc).
fn install_visibility_listener(
    chartml: &ChartMLRef,
    holders: &SendWrapper<Rc<RefCell<AutoRefreshState>>>,
    refresh_count: RwSignal<u32>,
) {
    if holders.borrow().visibility_listener.is_some() {
        return;
    }
    let Some(window) = web_sys::window() else { return };
    let Some(document) = window.document() else { return };
    let target: web_sys::EventTarget = document.clone().into();

    let chartml_for_listener = chartml.clone();
    let holders_for_listener = holders.clone();
    let listener = Closure::<dyn FnMut()>::new(move || {
        if document_is_hidden() {
            // Pause: drop the interval handle, keep sources/period for
            // the next "visible" transition to re-install from.
            let mut state = holders_for_listener.borrow_mut();
            if let Some(handle) = state.interval.take() {
                handle.clear();
            }
        } else {
            // Re-arm. If the interval is already installed (e.g. multiple
            // visibility events fire in quick succession) `install` will
            // no-op via the `is_some` guard.
            install_auto_refresh_interval(
                &chartml_for_listener,
                &holders_for_listener,
                refresh_count,
            );
        }
    });

    let _ = target.add_event_listener_with_callback(
        "visibilitychange",
        listener.as_ref().unchecked_ref(),
    );
    holders.borrow_mut().visibility_listener = Some((target, listener));
}

#[cfg(target_arch = "wasm32")]
/// `true` when `document.visibilityState == "hidden"`. Used both by the
/// per-tick guard and the visibility listener so the two can never
/// disagree.
fn document_is_hidden() -> bool {
    let Some(window) = web_sys::window() else { return false };
    let Some(document) = window.document() else { return false };
    document.visibility_state() == web_sys::VisibilityState::Hidden
}

/// Wall-clock now in milliseconds since the unix epoch. On wasm32, uses
/// `Date.now()` because `SystemTime::now()` panics on `wasm32-unknown-unknown`.
/// On native targets, uses `SystemTime::now()`.
fn now_ms() -> f64 {
    #[cfg(target_arch = "wasm32")]
    { js_sys::Date::now() }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as f64)
            .unwrap_or(0.0)
    }
}

#[cfg(target_arch = "wasm32")]
/// Log an auto-refresh tick for a single source to the browser console at
/// `debug` level. Browser-only AND debug-only — gated on
/// `target_arch = "wasm32"` so native tests don't touch `web_sys::console`,
/// and on `debug_assertions` so release builds don't pay the
/// `format!` allocation + JS interop call on every tick. Release callers
/// (and all native callers) get a no-op.
fn log_auto_refresh_tick(source_name: &str) {
    #[cfg(all(target_arch = "wasm32", debug_assertions))]
    {
        let msg = format!("[chartml-leptos] auto-refresh tick: source='{source_name}'");
        web_sys::console::debug_1(&wasm_bindgen::JsValue::from_str(&msg));
    }
    #[cfg(not(all(target_arch = "wasm32", debug_assertions)))]
    {
        // Suppress the unused-variable lint on native targets and on
        // release-mode wasm — `source_name` is meaningful but only
        // consumed by the debug-mode wasm32 branch above.
        let _ = source_name;
    }
}

#[cfg(target_arch = "wasm32")]
/// Reactive state shared between the auto-refresh effect, its interval
/// callback, and its visibility-listener callback. Lives in a single
/// `RefCell` so each callback can update or clear the others without
/// cross-cell borrow conflicts.
struct AutoRefreshState {
    interval: Option<leptos::prelude::IntervalHandle>,
    visibility_listener: Option<(web_sys::EventTarget, Closure<dyn FnMut()>)>,
    sources: Vec<AutoRefreshSource>,
    tick_period: Duration,
}

#[cfg(test)]
mod title_style_tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use chartml_core::theme::Theme;

    #[test]
    fn default_theme_emits_legacy_hardcoded_values() {
        // Default theme must produce the exact pre-Phase-4 style string so
        // existing browser DOM is unchanged.
        let style = build_title_style(&Theme::default());
        assert!(style.contains("font-size: 16px"), "style: {}", style);
        assert!(style.contains("font-weight: 600"), "style: {}", style);
        assert!(style.contains("color: var(--chartml-text-strong, #1f2937)"));
        assert!(style.contains("margin-bottom: 8px"));
        // Legacy had no font-family / font-style on the title.
        assert!(!style.contains("font-family:"), "style: {}", style);
        assert!(!style.contains("font-style:"), "style: {}", style);
    }

    #[test]
    fn custom_title_typography_is_threaded_through() {
        let mut theme = Theme::default();
        theme.title_font_family = "Georgia, serif".into();
        theme.title_font_size = 22.0;
        theme.title_font_weight = 800;
        theme.title_font_style = "italic".into();
        let style = build_title_style(&theme);
        assert!(style.contains("font-size: 22px"), "style: {}", style);
        assert!(style.contains("font-weight: 800"), "style: {}", style);
        assert!(style.contains("font-family: Georgia, serif"), "style: {}", style);
        assert!(style.contains("font-style: italic"), "style: {}", style);
    }

    #[test]
    fn fractional_font_size_preserves_decimal() {
        let mut theme = Theme::default();
        theme.title_font_size = 18.5;
        let style = build_title_style(&theme);
        assert!(style.contains("font-size: 18.5px"), "style: {}", style);
    }

    /// Phase 10 — Kyomi sanity check #1 (serif chart title). The full Kyomi
    /// theme targets an `Instrument Serif, Georgia, serif` title face. The
    /// integration-level `phase10_kyomi_sanity.rs` test cannot reach this
    /// helper (it's `pub(crate)`), so this unit test pins the serif family
    /// in the emitted title style string.
    #[test]
    fn phase10_kyomi_title_uses_serif_family() {
        let mut theme = Theme::default();
        theme.title_font_family = "Instrument Serif, Georgia, serif".into();
        theme.title_font_size = 22.0;
        theme.title_font_weight = 400;
        theme.title_font_style = "normal".into();
        let style = build_title_style(&theme);
        assert!(
            style.contains("font-family: Instrument Serif, Georgia, serif"),
            "kyomi title must use Instrument Serif family: {}",
            style,
        );
        assert!(style.contains("font-size: 22px"), "style: {}", style);
        assert!(style.contains("font-weight: 400"), "style: {}", style);
    }

    #[test]
    fn only_size_override_keeps_legacy_weight_and_omits_family() {
        let mut theme = Theme::default();
        theme.title_font_size = 20.0;
        let style = build_title_style(&theme);
        assert!(style.contains("font-size: 20px"));
        // Weight still matches default sentinel -> legacy 600.
        assert!(style.contains("font-weight: 600"));
        assert!(!style.contains("font-family:"));
        assert!(!style.contains("font-style:"));
    }
}

#[cfg(test)]
mod extract_title_tests {
    use super::*;

    #[test]
    fn title_before_data_is_extracted() {
        let yaml = "\
title: My Chart
type: chart
version: 1
data:
  datasource: ds
  query: \"SELECT 1\"
visualize:
  type: line
  columns: day
";
        assert_eq!(extract_yaml_title(yaml).as_deref(), Some("My Chart"));
    }

    #[test]
    fn title_after_data_block_is_still_extracted() {
        // Regression: the previous line-scan broke on the first `data:` line and
        // never reached an alphabetically-later `title:` key, dropping the title.
        let yaml = "\
data:
  datasource: ds
  query: \"SELECT 1\"
layout:
  colSpan: 8
title: Daily Visitors & Sessions
type: chart
version: 1
visualize:
  type: line
  columns: day
";
        assert_eq!(
            extract_yaml_title(yaml).as_deref(),
            Some("Daily Visitors & Sessions"),
        );
    }

    #[test]
    fn missing_title_returns_none() {
        let yaml = "\
type: chart
version: 1
data:
  datasource: ds
  query: \"SELECT 1\"
visualize:
  type: line
  columns: day
";
        assert_eq!(extract_yaml_title(yaml), None);
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod auto_refresh_tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use chartml_core::spec::source::CacheConfig as SpecCacheConfig;

    fn empty_inline() -> InlineData {
        InlineData {
            provider: None,
            rows: None,
            url: None,
            endpoint: None,
            cache: None,
            datasource: None,
            query: None,
        }
    }

    fn cache(ttl: Option<&str>, auto: Option<bool>) -> SpecCacheConfig {
        SpecCacheConfig {
            ttl: ttl.map(String::from),
            auto_refresh: auto,
        }
    }

    #[test]
    fn auto_refresh_skips_sources_without_flag() {
        let inline = InlineData {
            cache: Some(cache(Some("30s"), None)),
            ..empty_inline()
        };
        assert!(auto_refresh_from_inline("a".into(), &inline).is_none());

        let inline = InlineData {
            cache: Some(cache(Some("30s"), Some(false))),
            ..empty_inline()
        };
        assert!(auto_refresh_from_inline("a".into(), &inline).is_none());
    }

    #[test]
    fn auto_refresh_parses_explicit_ttl() {
        let inline = InlineData {
            cache: Some(cache(Some("45s"), Some(true))),
            ..empty_inline()
        };
        let src = auto_refresh_from_inline("metric".into(), &inline).expect("auto-refresh");
        assert_eq!(src.ttl, Duration::from_secs(45));
        assert_eq!(src.name, "metric");
    }

    #[test]
    fn auto_refresh_falls_back_to_default_ttl_when_missing() {
        // A user that says "auto-refresh me" but forgets the TTL gets
        // `DEFAULT_AUTO_REFRESH_INTERVAL` rather than a silent disable.
        let inline = InlineData {
            cache: Some(cache(None, Some(true))),
            ..empty_inline()
        };
        let src = auto_refresh_from_inline("m".into(), &inline).expect("auto-refresh");
        assert_eq!(src.ttl, DEFAULT_AUTO_REFRESH_INTERVAL);
    }

    #[test]
    fn auto_refresh_falls_back_when_ttl_unparseable() {
        // Same fallback for malformed TTL — the resolver will surface the
        // parse error during fetch.
        let inline = InlineData {
            cache: Some(cache(Some("five seconds"), Some(true))),
            ..empty_inline()
        };
        let src = auto_refresh_from_inline("m".into(), &inline).expect("auto-refresh");
        assert_eq!(src.ttl, DEFAULT_AUTO_REFRESH_INTERVAL);
    }

    #[test]
    fn shortest_interval_picks_smallest_positive_ttl() {
        let sources = vec![
            AutoRefreshSource {
                name: "a".into(),
                inline: empty_inline(),
                ttl: Duration::from_secs(60),
            },
            AutoRefreshSource {
                name: "b".into(),
                inline: empty_inline(),
                ttl: Duration::from_secs(15),
            },
            AutoRefreshSource {
                name: "c".into(),
                inline: empty_inline(),
                ttl: Duration::from_secs(120),
            },
        ];
        assert_eq!(shortest_interval(&sources), Some(Duration::from_secs(15)));
    }

    #[test]
    fn shortest_interval_skips_zero_ttls() {
        let sources = vec![
            AutoRefreshSource {
                name: "zero".into(),
                inline: empty_inline(),
                ttl: Duration::ZERO,
            },
            AutoRefreshSource {
                name: "real".into(),
                inline: empty_inline(),
                ttl: Duration::from_secs(10),
            },
        ];
        assert_eq!(shortest_interval(&sources), Some(Duration::from_secs(10)));
    }

    #[test]
    fn shortest_interval_returns_none_when_all_zero_or_empty() {
        assert_eq!(shortest_interval(&[]), None);
        let sources = vec![AutoRefreshSource {
            name: "z".into(),
            inline: empty_inline(),
            ttl: Duration::ZERO,
        }];
        assert_eq!(shortest_interval(&sources), None);
    }

    #[test]
    fn collect_auto_refresh_sources_handles_named_map() {
        let yaml = r#"
type: chart
version: 1
data:
  metric_a:
    datasource: warehouse
    query: SELECT 1
    cache:
      ttl: 30s
      autoRefresh: true
  metric_b:
    datasource: warehouse
    query: SELECT 2
    cache:
      ttl: 60s
      autoRefresh: true
  metric_c:
    datasource: warehouse
    query: SELECT 3
transform:
  sql: SELECT * FROM metric_a
visualize:
  type: bar
  columns: a
  rows: b
"#;
        let spec = first_chart_spec(yaml).expect("parse");
        let sources = collect_auto_refresh_sources(&spec);
        // metric_a + metric_b should be picked up; metric_c (no autoRefresh) skipped.
        let names: Vec<String> = sources.iter().map(|s| s.name.clone()).collect();
        assert!(names.contains(&"metric_a".to_string()));
        assert!(names.contains(&"metric_b".to_string()));
        assert!(!names.contains(&"metric_c".to_string()));
        // Shortest TTL across the auto-refreshing entries.
        assert_eq!(shortest_interval(&sources), Some(Duration::from_secs(30)));
    }

    #[test]
    fn collect_auto_refresh_sources_handles_flat_inline() {
        let yaml = r#"
type: chart
version: 1
data:
  datasource: warehouse
  query: SELECT 1
  cache:
    ttl: 10s
    autoRefresh: true
visualize:
  type: bar
  columns: a
  rows: b
"#;
        let spec = first_chart_spec(yaml).expect("parse");
        let sources = collect_auto_refresh_sources(&spec);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].name, "source");
        assert_eq!(sources[0].ttl, Duration::from_secs(10));
    }

    #[test]
    fn collect_auto_refresh_sources_skips_named_string_ref() {
        let yaml = r#"
type: chart
version: 1
data: registered_source
visualize:
  type: bar
  columns: a
  rows: b
"#;
        let spec = first_chart_spec(yaml).expect("parse");
        assert!(collect_auto_refresh_sources(&spec).is_empty());
    }
}
