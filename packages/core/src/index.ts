import type {
  RenderOptions,
  RendererCallback,
  DataSourceCallback,
  TransformCallback,
  ResolverCallback,
  ChartElement,
} from './types.js';

// Re-export types
export type {
  RenderOptions,
  RendererCallback,
  DataSourceCallback,
  TransformCallback,
  ResolverCallback,
  ChartElement,
  ViewBox,
  ElementData,
  Transform,
  TextAnchor,
  SvgElement,
  GroupElement,
  RectElement,
  PathElement,
  CircleElement,
  LineElement,
  TextElement,
  DivElement,
  SpanElement,
} from './types.js';

// Single init promise — ensures WASM is initialized exactly once,
// even under concurrent create() calls.
let initPromise: Promise<any> | null = null;

export class ChartML {
  private inner: any; // WasmChartML from pkg

  private constructor(inner: any) {
    this.inner = inner;
  }

  /**
   * Create a new ChartML instance. Initializes WASM on first call.
   * Safe to call concurrently — all callers share the same init promise.
   */
  static async create(): Promise<ChartML> {
    if (!initPromise) {
      initPromise = import('../pkg/chartml.js').then(async (m) => {
        await m.default();
        return m;
      });
    }
    const mod = await initPromise;
    const inner = new mod.WasmChartML();
    return new ChartML(inner);
  }

  /** Render YAML spec to SVG string. */
  renderToSvg(yaml: string, options?: RenderOptions): string {
    return this.inner.renderToSvg(yaml, options ?? {});
  }

  /** Render YAML spec to ChartElement JSON tree. */
  renderToElement(yaml: string, options?: RenderOptions): ChartElement {
    return this.inner.renderToElement(yaml, options ?? {});
  }

  /** Register a custom chart type renderer. */
  registerRenderer(chartType: string, renderFn: RendererCallback): void {
    this.inner.registerRenderer(chartType, renderFn);
  }

  /** Register a custom data source. */
  registerDataSource(name: string, fetchFn: DataSourceCallback): void {
    this.inner.registerDataSource(name, fetchFn);
  }

  /** Register a transform middleware. */
  registerTransform(transformFn: TransformCallback): void {
    this.inner.registerTransform(transformFn);
  }

  /** Set the datasource resolver. */
  setDatasourceResolver(resolverFn: ResolverCallback): void {
    this.inner.setDatasourceResolver(resolverFn);
  }

  /** Register a YAML component (source, style, config, params). */
  registerComponent(yaml: string): void {
    this.inner.registerComponent(yaml);
  }
}
