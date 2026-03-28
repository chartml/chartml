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
    /** Render YAML spec to SVG string. */
    renderToSvg(yaml, options) {
        return this.inner.renderToSvg(yaml, options ?? {});
    }
    /** Render YAML spec to ChartElement JSON tree. */
    renderToElement(yaml, options) {
        return this.inner.renderToElement(yaml, options ?? {});
    }
    /** Register a custom chart type renderer. */
    registerRenderer(chartType, renderFn) {
        this.inner.registerRenderer(chartType, renderFn);
    }
    /** Register a custom data source. */
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
}
