// Single init promise — ensures WASM is initialized exactly once,
// even under concurrent create() calls.
let initPromise = null;
export class ChartML {
    constructor(inner) {
        this.inner = inner;
    }
    /**
     * Create a new ChartML instance. Initializes WASM on first call.
     * Safe to call concurrently — all callers share the same init promise.
     */
    static async create() {
        if (!initPromise) {
            initPromise = import('../pkg/web/chartml_wasm.js').then(async (m) => {
                await m.default();
                return m;
            });
        }
        const mod = await initPromise;
        const inner = new mod.WasmChartML();
        return new ChartML(inner);
    }
    /** Render YAML spec to SVG string (sync — inline data only). */
    renderToSvg(yaml, options) {
        return this.inner.renderToSvg(yaml, options ?? {});
    }
    /** Render YAML spec to ChartElement JSON tree (sync — inline data only). */
    renderToElement(yaml, options) {
        return this.inner.renderToElement(yaml, options ?? {});
    }
    /** Register a custom chart type renderer. */
    registerRenderer(chartType, renderFn) {
        this.inner.registerRenderer(chartType, renderFn);
    }
    /** Register a custom data source (legacy chartml 4.x API). New code should use `registerProvider`. */
    registerDataSource(name, fetchFn) {
        this.inner.registerDataSource(name, fetchFn);
    }
    /** Register a transform middleware. */
    registerTransform(transformFn) {
        this.inner.registerTransform(transformFn);
    }
    /** Set the datasource resolver. */
    setDatasourceResolver(resolverFn) {
        this.inner.setDatasourceResolver(resolverFn);
    }
    /** Register a YAML component (source, style, config, params). */
    registerComponent(yaml) {
        this.inner.registerComponent(yaml);
    }
    // -----------------------------------------------------------------------
    // chartml 5.0 — DataSourceProvider + cache + hooks + async pipeline
    // -----------------------------------------------------------------------
    /**
     * Register a `DataSourceProvider` callback under a dispatch key. The key
     * matches `data: { provider: <kind>, ... }` in the YAML; built-in kinds
     * (`"inline"`, `"http"`) are pre-registered. The `"datasource"` slot is
     * intentionally empty — consumers using `data: { datasource: ..., query:
     * ... }` MUST register their own provider under that key.
     *
     * Re-registration replaces the previous provider for the kind.
     */
    registerProvider(kind, callback) {
        this.inner.registerProvider(kind, callback);
    }
    /**
     * Set the tenant / workspace namespace folded into every cache key.
     * Multi-tenant deployments MUST set this so two tenants sharing a slug
     * name cannot collide in the cache.
     */
    setNamespace(namespace) {
        this.inner.setNamespace(namespace);
    }
    /**
     * Replace the tier-1 cache backend. Accepts:
     * - `"memory"` — default in-process backend (resets the cache).
     * - `"indexeddb"` — promotes a tier-2 IndexedDB-backed cache for browsers
     *   (only available when chartml-wasm is built with `wasm-indexeddb`).
     * - A JS object implementing `CacheBackendInterface`.
     *
     * Returns a Promise so `"indexeddb"` (which has to open the DB
     * asynchronously) can be awaited uniformly.
     */
    setCache(backend) {
        return this.inner.setCache(backend);
    }
    /**
     * Enable IndexedDB-backed persistent caching with the given database
     * name and namespace. The database opens lazily on the first fetch.
     *
     * This is a simpler alternative to `setCache("indexeddb")` — it lets
     * callers specify the database name and namespace explicitly.
     */
    enableIndexedDbCache(dbName, namespace) {
        this.inner.enableIndexedDbCache(dbName, namespace);
    }
    /**
     * Register a `ResolverHooksInterface` impl. Missing handlers are silent
     * no-ops. Handlers may return Promises — the resolver fires them
     * fire-and-forget so a slow telemetry sink cannot stall the pipeline.
     */
    setHooks(hooks) {
        this.inner.setHooks(hooks);
    }
    /**
     * Stage 1: parse YAML, dispatch every named source through the registered
     * providers. Returns an opaque handle the next stages consume.
     *
     * Most consumers will use `renderToSvgAsync` instead, which runs the full
     * fetch → transform → renderPreparedToSvg pipeline in one call.
     */
    fetch(yaml, options) {
        return this.inner.fetch(yaml, options ?? {});
    }
    /**
     * Stage 2: collapse the fetched sources into a single prepared table.
     * Consumes the `FetchedChart` produced by `fetch` (single-use handle).
     */
    transform(fetched) {
        return this.inner.transform(fetched);
    }
    /**
     * Stage 3 (sync): render an already-prepared chart to an SVG string.
     * `options.width` / `options.height` override the dimensions carried in
     * the `PreparedChart` if supplied — useful for resize-without-refetch.
     */
    renderPreparedToSvg(prepared, options) {
        return this.inner.renderPreparedToSvg(prepared, options ?? {});
    }
    /**
     * Convenience: run the full async pipeline (`fetch` → `transform` →
     * `renderPreparedToSvg`) in one Promise. Most JS consumers use only this.
     */
    renderToSvgAsync(yaml, options) {
        return this.inner.renderToSvgAsync(yaml, options ?? {});
    }
    /**
     * Await graceful shutdown on every registered provider AND cache backend.
     * Call at SSR request end / browser tab close.
     */
    shutdown() {
        return this.inner.shutdown();
    }
    // -----------------------------------------------------------------------
    // chartml 5.0 — bulk invalidation API
    // -----------------------------------------------------------------------
    /** Drop a single resolver entry by its hashed cache key. */
    resolverInvalidate(key) {
        return this.inner.resolverInvalidate(key);
    }
    /** Drop every cached entry across every tier. */
    resolverInvalidateAll() {
        return this.inner.resolverInvalidateAll();
    }
    /**
     * Drop every entry whose source spec carried the given `datasource` slug.
     * Useful for "datasource X was edited; refresh all queries against it".
     */
    resolverInvalidateBySlug(slug) {
        return this.inner.resolverInvalidateBySlug(slug);
    }
    /** Drop every entry tagged with the given namespace. */
    resolverInvalidateByNamespace(namespace) {
        return this.inner.resolverInvalidateByNamespace(namespace);
    }
    /**
     * Compute the cache key the resolver would use for a given inline-data
     * spec. Returned as a `bigint` (JS `number` can't represent every `u64`).
     * Pass the result to `resolverInvalidate` to drop a single entry without
     * re-implementing the hash JS-side.
     */
    resolverKeyFor(spec, namespace) {
        return this.inner.resolverKeyFor(spec, namespace ?? null);
    }
}
