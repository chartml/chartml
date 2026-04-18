mod adapters;
mod data_sources;
#[cfg(target_arch = "wasm32")]
mod pipeline;

use chartml_chart_cartesian::CartesianRenderer;
use chartml_chart_metric::MetricRenderer;
use chartml_chart_pie::PieRenderer;
use chartml_chart_scatter::ScatterRenderer;
use chartml_chart_table::TableRenderer;
use chartml_core::ChartML;
use chartml_render::element_to_svg;
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
use std::rc::Rc;

/// Browser-only ergonomics: the chartml 5.0 async pipeline holds the inner
/// `ChartML` for the duration of a `fetch`/`transform`/`renderToSvgAsync`
/// future. We store it as `Rc<ChartML>` so each in-flight async call holds
/// its own cheap clone — the future is `'static` and pinned, no `RefCell`
/// borrow needs to span an `.await`.
///
/// Mutation methods (`registerProvider`, `setHooks`, `setCache`, etc.)
/// require unique access via `Rc::get_mut`. **Contract:** every `register*`
/// / `set*` call must happen on a `WasmChartML` whose inner `Rc` has no
/// outstanding clones — i.e. before any in-flight `fetch` Promise.
/// Attempting to mutate while a Promise is pending panics with a clear
/// "ChartML configuration cannot change while async operations are in
/// flight" message.
#[cfg(target_arch = "wasm32")]
type WasmInner = Rc<ChartML>;

#[wasm_bindgen]
pub struct WasmChartML {
    #[cfg(not(target_arch = "wasm32"))]
    inner: ChartML,
    #[cfg(target_arch = "wasm32")]
    inner: WasmInner,
}

impl Default for WasmChartML {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl WasmChartML {
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmChartML {
        // Install the wasm panic hook on first construction so a Rust panic
        // (which appears to JS as the cryptic `RuntimeError: unreachable`)
        // prints a real message + backtrace via `console.error` instead.
        // Idempotent; safe to call from every constructor.
        #[cfg(target_arch = "wasm32")]
        console_error_panic_hook::set_once();

        let mut chartml = ChartML::new();
        // Register all built-in renderers
        chartml.register_renderer("bar", CartesianRenderer::new());
        chartml.register_renderer("line", CartesianRenderer::new());
        chartml.register_renderer("area", CartesianRenderer::new());
        chartml.register_renderer("pie", PieRenderer::new());
        chartml.register_renderer("doughnut", PieRenderer::new());
        chartml.register_renderer("scatter", ScatterRenderer::new());
        chartml.register_renderer("bubble", ScatterRenderer::new());
        chartml.register_renderer("metric", MetricRenderer::new());
        chartml.register_renderer("table", TableRenderer::new());
        // Register built-in data sources (browser-only) — predates the
        // chartml 5.0 resolver/provider refactor and is preserved verbatim
        // for backwards compatibility with the legacy `DataSource` trait.
        #[cfg(target_arch = "wasm32")]
        chartml.register_data_source("http", data_sources::http::HttpDataSource::new());

        #[cfg(not(target_arch = "wasm32"))]
        {
            WasmChartML { inner: chartml }
        }
        #[cfg(target_arch = "wasm32")]
        {
            WasmChartML {
                inner: Rc::new(chartml),
            }
        }
    }

    /// Render a ChartML YAML spec to an SVG string.
    #[wasm_bindgen(js_name = "renderToSvg")]
    pub fn render_to_svg(&self, yaml: &str, options: JsValue) -> Result<String, JsValue> {
        let (width, height) = parse_render_options(&options);
        with_inner(self, |chartml| {
            let element = chartml
                .render_from_yaml_with_size(yaml, width, height)
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            let w = width.unwrap_or(800.0);
            let h = height.unwrap_or(400.0);
            Ok(element_to_svg(&element, w, h))
        })
    }

    /// Render a ChartML YAML spec to a ChartElement JSON object.
    #[wasm_bindgen(js_name = "renderToElement")]
    pub fn render_to_element(&self, yaml: &str, options: JsValue) -> Result<JsValue, JsValue> {
        let (width, height) = parse_render_options(&options);
        with_inner(self, |chartml| {
            let element = chartml
                .render_from_yaml_with_size(yaml, width, height)
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            serde_wasm_bindgen::to_value(&element).map_err(|e| JsValue::from_str(&e.to_string()))
        })
    }

    /// Register a custom JS function as a chart renderer for a given chart type.
    ///
    /// The function receives `(rows: object[], config: object)` and must return
    /// a `ChartElement`-shaped JSON object.
    #[wasm_bindgen(js_name = "registerRenderer")]
    pub fn register_renderer(&mut self, chart_type: &str, render_fn: js_sys::Function) {
        let renderer = adapters::renderer::JsChartRenderer::new(render_fn);
        with_inner_mut(self, |chartml| {
            chartml.register_renderer(chart_type, renderer);
        });
    }

    /// Register a named JS data source.
    ///
    /// **Legacy chartml 4.x API.** New code should use [`Self::register_provider`]
    /// instead — providers integrate with the resolver / cache / dedup
    /// pipeline that powers the chartml 5.0 async API. This method is kept
    /// for back-compat only.
    ///
    /// The function receives `(spec: object)` and must return a `Promise<object[]>`.
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = "registerDataSource")]
    pub fn register_data_source(&mut self, name: &str, fetch_fn: js_sys::Function) {
        let source = adapters::data_source::JsDataSource::new(fetch_fn);
        with_inner_mut(self, |chartml| {
            chartml.register_data_source(name, source);
        });
    }

    /// Register a JS transform middleware.
    ///
    /// The function receives `(sources: Record<string, object[]>, spec: object, context: object)`
    /// and must return a `Promise<{data: object[], metadata: object}>`.
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = "registerTransform")]
    pub fn register_transform(&mut self, transform_fn: js_sys::Function) {
        let middleware = adapters::transform::JsTransformMiddleware::new(transform_fn);
        with_inner_mut(self, |chartml| {
            chartml.register_transform(middleware);
        });
    }

    /// Set a JS datasource resolver.
    ///
    /// The function receives `(slug: string)` and must return a
    /// `Promise<{provider: string, connectionString?: string, config: object}>`.
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = "setDatasourceResolver")]
    pub fn set_datasource_resolver(&mut self, resolver_fn: js_sys::Function) {
        let resolver = adapters::resolver::JsDatasourceResolver::new(resolver_fn);
        with_inner_mut(self, |chartml| {
            chartml.set_datasource_resolver(resolver);
        });
    }

    /// Register a named YAML component (source, style, config, params).
    #[wasm_bindgen(js_name = "registerComponent")]
    pub fn register_component(&mut self, yaml: &str) -> Result<(), JsValue> {
        with_inner_mut(self, |chartml| {
            chartml
                .register_component(yaml)
                .map_err(|e| JsValue::from_str(&e.to_string()))
        })
    }

    // ── chartml 5.0 — DataSourceProvider bridge ──────────────────────────

    /// Register a `DataSourceProvider` callback under a dispatch key.
    ///
    /// Built-in kinds (`"inline"`, `"http"`) are pre-registered on
    /// construction. The `"datasource"` slot is intentionally empty —
    /// consumers whose YAML uses `data: { datasource: "...", query: "..." }`
    /// MUST register their own provider under that key.
    ///
    /// The callback signature is:
    /// ```ts
    /// (request: FetchRequest) => Promise<{
    ///   data: Array<Record<string, unknown>> | Uint8Array,
    ///   metadata?: Record<string, unknown>,
    /// }>
    /// ```
    /// Where `data` is either an array of row objects (canonical chartml shape)
    /// or a `Uint8Array` of Arrow IPC bytes (decoded server-side).
    /// Re-registration replaces the previous provider for that kind.
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = "registerProvider")]
    pub fn register_provider(&mut self, kind: String, callback: js_sys::Function) {
        let provider = adapters::provider::JsCallbackProvider::new(kind.clone(), callback);
        with_inner_mut(self, |chartml| {
            chartml.register_provider(&kind, provider);
        });
    }

    /// Set the tenant / workspace namespace folded into every resolver
    /// cache key. Multi-tenant deployments MUST set this so two tenants
    /// sharing a slug name cannot collide in the cache.
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = "setNamespace")]
    pub fn set_namespace(&mut self, namespace: String) {
        with_inner_mut(self, |chartml| {
            chartml.set_namespace(namespace);
        });
    }

    /// Replace the tier-1 cache backend.
    ///
    /// `backend` may be:
    /// - the string `"memory"` — the default in-process cache (no-op replacement);
    /// - the string `"indexeddb"` — promotes the in-memory backend into a
    ///   tier-2 IndexedDB-backed cache (only available when the `wasm-indexeddb`
    ///   feature is enabled at WASM build time);
    /// - a JS object exposing `get / put / invalidate / invalidateByTag /
    ///   clear / shutdown?` — wired through [`adapters::cache_backend::JsCallbackCacheBackend`].
    ///
    /// The replacement starts empty — entries in the old backend are not
    /// migrated.
    ///
    /// IndexedDB construction is async (the database has to open). When
    /// `"indexeddb"` is passed this method returns a Promise that resolves
    /// once the database is open.
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = "setCache")]
    pub fn set_cache(&mut self, backend: JsValue) -> Result<js_sys::Promise, JsValue> {
        if let Some(name) = backend.as_string() {
            return self.set_cache_named(&name);
        }
        let js_backend = adapters::cache_backend::JsCallbackCacheBackend::from_js(backend)?;
        with_inner_mut(self, |chartml| {
            chartml.set_cache(js_backend);
        });
        Ok(immediate_void_promise())
    }

    /// Internal: dispatch a string `setCache` argument. Memory is sync —
    /// returns an already-resolved Promise. IndexedDB is async — returns
    /// the open-database future.
    #[cfg(target_arch = "wasm32")]
    fn set_cache_named(&mut self, name: &str) -> Result<js_sys::Promise, JsValue> {
        match name {
            "memory" => {
                use chartml_core::resolver::MemoryBackend;
                with_inner_mut(self, |chartml| {
                    chartml.set_cache(MemoryBackend::new());
                });
                Ok(immediate_void_promise())
            }
            "indexeddb" => {
                #[cfg(feature = "wasm-indexeddb")]
                {
                    // Capture an `Rc<ChartML>` clone for the async block. The
                    // `IndexedDbBackend::new` future is the entire async work
                    // here — once it resolves we install the backend on the
                    // tier-2 slot, leaving tier-1 (`MemoryBackend`) intact so
                    // memory hits short-circuit before the IndexedDB read.
                    let inner = self.inner.clone();
                    let database_name = "chartml-cache".to_string();
                    // Default namespace for the bare `setCache("indexeddb")`
                    // form. Multi-tenant deployments should use the JS-object
                    // backend overload (or call `setNamespace` first and then
                    // pass the explicit `IndexedDbBackend` instance) so the
                    // namespace varies per workspace/user.
                    let namespace = "default".to_string();
                    Ok(wasm_bindgen_futures::future_to_promise(async move {
                        use chartml_core::resolver::backends::indexeddb::IndexedDbBackend;
                        let backend =
                            IndexedDbBackend::new(&database_name, &namespace)
                                .await
                                .map_err(|e| JsValue::from_str(&e.to_string()))?;
                        inner
                            .resolver()
                            .set_persistent_cache(chartml_core::resolver::SharedRef::new(backend));
                        Ok(JsValue::UNDEFINED)
                    }))
                }
                #[cfg(not(feature = "wasm-indexeddb"))]
                {
                    Err(JsValue::from_str(
                        "setCache(\"indexeddb\"): chartml-wasm was built without the `wasm-indexeddb` feature",
                    ))
                }
            }
            other => Err(JsValue::from_str(&format!(
                "setCache: unknown built-in backend '{other}' (expected 'memory' or 'indexeddb')"
            ))),
        }
    }

    /// Register a `ResolverHooks` impl. `hooks` is a plain JS object whose
    /// optional `onProgress` / `onCacheHit` / `onCacheMiss` / `onError`
    /// properties are invoked at the matching pipeline events. Missing
    /// properties are silent no-ops.
    ///
    /// Each handler receives the matching event in camelCase form (see the
    /// TypeScript `ResolverHooksInterface` type for the field shape).
    ///
    /// Takes `&self` (not `&mut self`) — `ChartML::set_hooks` only needs a
    /// shared reference because the resolver wraps the hook slot in its
    /// own interior mutability. That means hooks can be installed AFTER
    /// `fetch`/`renderToSvgAsync` Promises are already in flight without
    /// hitting the `Rc::get_mut` panic path.
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = "setHooks")]
    pub fn set_hooks(&self, hooks: JsValue) {
        let bridge = adapters::hooks::JsCallbackHooks::from_js(&hooks);
        with_inner(self, |chartml| {
            chartml.set_hooks(bridge);
        });
    }

    // ── chartml 5.0 — three-stage pipeline ───────────────────────────────

    /// Stage 1: parse YAML, resolve params, dispatch every named source
    /// through the registered providers. Returns an opaque `FetchedChart`
    /// handle that subsequent `transform` / `renderPreparedToSvg` calls
    /// consume.
    ///
    /// `opts` is `{ width?: number, height?: number, params?: object }` and
    /// is forwarded down the pipeline so the same options apply at each
    /// stage.
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = "fetch")]
    pub fn fetch(&self, yaml: String, opts: JsValue) -> js_sys::Promise {
        let inner = self.inner.clone();
        let render_opts = match parse_full_render_options(&opts) {
            Ok(o) => o,
            Err(e) => return immediate_rejected_promise(&e),
        };
        wasm_bindgen_futures::future_to_promise(async move {
            let fetched = inner
                .fetch(&yaml, &render_opts)
                .await
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            Ok(pipeline::FetchedChart::wrap(fetched, render_opts).into_js())
        })
    }

    /// Stage 2: collapse the fetched sources into a single `DataTable`.
    /// Consumes the `FetchedChart` produced by `fetch`.
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = "transform")]
    pub fn transform(&self, fetched: JsValue) -> js_sys::Promise {
        let inner = self.inner.clone();
        let unwrapped = match pipeline::FetchedChart::unwrap(fetched) {
            Ok(v) => v,
            Err(e) => return immediate_rejected_promise(&e.as_string().unwrap_or_default()),
        };
        wasm_bindgen_futures::future_to_promise(async move {
            // `inner` is `Rc<ChartML>`; deref directly — `transform` is
            // `&self` so the Rc handle stays valid across the `.await`.
            let prepared = inner
                .transform(unwrapped.fetched, &unwrapped.opts)
                .await
                .map_err(|e: chartml_core::ChartError| JsValue::from_str(&e.to_string()))?;
            Ok(pipeline::PreparedChart::wrap(prepared, unwrapped.opts).into_js())
        })
    }

    /// Stage 3 (sync): render an already-prepared chart to an SVG string.
    /// `opts` overrides the width/height carried in the `PreparedChart` if
    /// supplied — useful for resize-without-refetch flows.
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = "renderPreparedToSvg")]
    pub fn render_prepared_to_svg(
        &self,
        prepared: JsValue,
        opts: JsValue,
    ) -> Result<String, JsValue> {
        let unwrapped = pipeline::PreparedChart::unwrap(prepared)?;
        let mut render_opts = unwrapped.opts;
        let (width, height) = parse_render_options(&opts);
        if width.is_some() {
            render_opts.width = width;
        }
        if height.is_some() {
            render_opts.height = height;
        }
        with_inner(self, |chartml| {
            chartml
                .render_prepared_to_svg(&unwrapped.prepared, &render_opts)
                .map_err(|e| JsValue::from_str(&e.to_string()))
        })
    }

    /// Convenience: run the full async pipeline (`fetch` → `transform` →
    /// `renderPreparedToSvg`) in one Promise. Most JS consumers will use
    /// only this method.
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = "renderToSvgAsync")]
    pub fn render_to_svg_async(&self, yaml: String, opts: JsValue) -> js_sys::Promise {
        let inner = self.inner.clone();
        let render_opts = match parse_full_render_options(&opts) {
            Ok(o) => o,
            Err(e) => return immediate_rejected_promise(&e),
        };
        wasm_bindgen_futures::future_to_promise(async move {
            let svg = inner
                .render_to_svg_async(&yaml, &render_opts)
                .await
                .map_err(|e: chartml_core::ChartError| JsValue::from_str(&e.to_string()))?;
            Ok(JsValue::from_str(&svg))
        })
    }

    /// Await graceful shutdown on every registered provider AND cache
    /// backend. Call at SSR request end / browser tab close.
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = "shutdown")]
    pub fn shutdown(&self) -> js_sys::Promise {
        let inner = self.inner.clone();
        wasm_bindgen_futures::future_to_promise(async move {
            inner.shutdown().await;
            Ok(JsValue::UNDEFINED)
        })
    }

    // ── chartml 5.0 — bulk invalidation ──────────────────────────────────

    /// Drop a single resolver entry by its hashed cache key. The key is the
    /// `bigint` returned by `resolverKeyFor(spec, namespace)`.
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = "resolverInvalidate")]
    pub fn resolver_invalidate(&self, key: js_sys::BigInt) -> js_sys::Promise {
        let inner = self.inner.clone();
        let key_u64 = match bigint_to_u64(&key) {
            Ok(k) => k,
            Err(e) => return immediate_rejected_promise(&e),
        };
        wasm_bindgen_futures::future_to_promise(async move {
            inner.resolver().invalidate(key_u64).await;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Drop every cached entry across every tier.
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = "resolverInvalidateAll")]
    pub fn resolver_invalidate_all(&self) -> js_sys::Promise {
        let inner = self.inner.clone();
        wasm_bindgen_futures::future_to_promise(async move {
            inner.resolver().invalidate_all().await;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Drop every entry whose source spec carried the given `datasource`
    /// slug. Useful for "datasource X was edited; refresh all queries
    /// against it" workflows.
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = "resolverInvalidateBySlug")]
    pub fn resolver_invalidate_by_slug(&self, slug: String) -> js_sys::Promise {
        let inner = self.inner.clone();
        wasm_bindgen_futures::future_to_promise(async move {
            inner.resolver().invalidate_by_slug(&slug).await;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Drop every entry tagged with the given namespace. Used for tenant
    /// isolation flows ("user logged out; clear their cached data").
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = "resolverInvalidateByNamespace")]
    pub fn resolver_invalidate_by_namespace(&self, namespace: String) -> js_sys::Promise {
        let inner = self.inner.clone();
        wasm_bindgen_futures::future_to_promise(async move {
            inner.resolver().invalidate_by_namespace(&namespace).await;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Compute the cache key the resolver would use for a given inline-data
    /// spec. Returned as a `BigInt` (JS `Number` can't represent every
    /// `u64`). Pass the result to `resolverInvalidate` to drop a single
    /// entry without re-implementing the hash JS-side.
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = "resolverKeyFor")]
    pub fn resolver_key_for(
        &self,
        spec: JsValue,
        namespace: Option<String>,
    ) -> Result<js_sys::BigInt, JsValue> {
        use chartml_core::resolver::Resolver;
        use chartml_core::spec::InlineData;
        let inline: InlineData = serde_wasm_bindgen::from_value(spec)
            .map_err(|e| JsValue::from_str(&format!("invalid spec: {e}")))?;
        let key = Resolver::key_for(&inline, namespace.as_deref());
        Ok(u64_to_bigint(key))
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────

/// Run a closure with a borrowed `&ChartML`. On native we borrow the field
/// directly; on wasm32 the field is `Rc<ChartML>` and `Deref` coercion takes
/// `&Rc<ChartML>` to `&ChartML` automatically — same body either way.
#[inline]
fn with_inner<R>(this: &WasmChartML, f: impl FnOnce(&ChartML) -> R) -> R {
    f(&this.inner)
}

/// Run a closure with a `&mut ChartML`. wasm32 acquires unique access via
/// `Rc::get_mut`; if any in-flight async operation still holds a clone,
/// this panics with a clear configuration-vs-fetch ordering error.
#[inline]
#[cfg(not(target_arch = "wasm32"))]
fn with_inner_mut<R>(this: &mut WasmChartML, f: impl FnOnce(&mut ChartML) -> R) -> R {
    f(&mut this.inner)
}

#[inline]
#[cfg(target_arch = "wasm32")]
fn with_inner_mut<R>(this: &mut WasmChartML, f: impl FnOnce(&mut ChartML) -> R) -> R {
    let inner = Rc::get_mut(&mut this.inner).expect(
        "ChartML configuration cannot change while async operations are in flight \
         (call register*/set* methods only after every previous Promise has settled)",
    );
    f(inner)
}

fn parse_render_options(options: &JsValue) -> (Option<f64>, Option<f64>) {
    if options.is_undefined() || options.is_null() {
        return (None, None);
    }
    let width = js_sys::Reflect::get(options, &"width".into())
        .ok()
        .and_then(|v| v.as_f64());
    let height = js_sys::Reflect::get(options, &"height".into())
        .ok()
        .and_then(|v| v.as_f64());
    (width, height)
}

/// Parse the full chartml 5.0 `RenderOptions` shape from a JS object:
/// `{ width?: number, height?: number, params?: Record<string, unknown> }`.
#[cfg(target_arch = "wasm32")]
fn parse_full_render_options(options: &JsValue) -> Result<chartml_core::RenderOptions, String> {
    use chartml_core::params::ParamValues;
    let (width, height) = parse_render_options(options);
    let mut render_opts = chartml_core::RenderOptions {
        width,
        height,
        params: None,
    };
    if !options.is_undefined() && !options.is_null() && options.is_object() {
        let params_js = js_sys::Reflect::get(options, &JsValue::from_str("params"))
            .map_err(|e| format!("failed to read `params`: {e:?}"))?;
        if !params_js.is_undefined() && !params_js.is_null() {
            let params: ParamValues = serde_wasm_bindgen::from_value(params_js)
                .map_err(|e| format!("invalid `params` object: {e}"))?;
            render_opts.params = Some(params);
        }
    }
    Ok(render_opts)
}

/// Build an already-resolved `Promise<undefined>`. Used by sync code paths
/// that still need to return a Promise to satisfy the public TS contract
/// (e.g. `setCache("memory")`).
#[cfg(target_arch = "wasm32")]
fn immediate_void_promise() -> js_sys::Promise {
    js_sys::Promise::resolve(&JsValue::UNDEFINED)
}

/// Build an already-rejected Promise carrying the given error string.
#[cfg(target_arch = "wasm32")]
fn immediate_rejected_promise(message: &str) -> js_sys::Promise {
    js_sys::Promise::reject(&JsValue::from_str(message))
}

/// `BigInt` → `u64` conversion that preserves wraparound semantics for the
/// bottom 64 bits. JS `BigInt`s are arbitrary precision; the resolver only
/// uses the low 64 bits because that's what `Resolver::key_for` returns.
#[cfg(target_arch = "wasm32")]
fn bigint_to_u64(bigint: &js_sys::BigInt) -> Result<u64, String> {
    let s: String = bigint
        .to_string(16)
        .map_err(|_| "BigInt → string conversion failed".to_string())?
        .into();
    // `BigInt::toString(16)` returns the unsigned hex form; negative BigInts
    // get a leading '-'. Cache keys are unsigned so we reject negatives.
    if let Some(rest) = s.strip_prefix('-') {
        return Err(format!("cache key cannot be negative (got -{rest})"));
    }
    u64::from_str_radix(&s, 16).map_err(|e| format!("BigInt → u64: {e}"))
}

#[cfg(target_arch = "wasm32")]
fn u64_to_bigint(value: u64) -> js_sys::BigInt {
    // `js_sys::BigInt::from(u64)` accepts an unsigned conversion — this
    // matches the design doc's "keys are unsigned 64-bit" contract.
    js_sys::BigInt::from(value)
}
