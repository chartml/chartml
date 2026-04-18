// Render options
export interface RenderOptions {
  width?: number;
  height?: number;
  /**
   * chartml 5.0 — per-render parameter values keyed by `{scope}.{name}`
   * (e.g. `{ "filters.year": 2024 }`). Substituted into `$param.*` references
   * inside the YAML before fetch dispatch.
   */
  params?: Record<string, unknown>;
}

// Plugin callback types
export type RendererCallback = (rows: Record<string, unknown>[], config: Record<string, unknown>) => ChartElement;
export type DataSourceCallback = (spec: Record<string, unknown>) => Promise<Record<string, unknown>[]>;
export type TransformCallback = (rows: Record<string, unknown>[], spec: Record<string, unknown>, context: Record<string, unknown>) => Promise<{ data: Record<string, unknown>[]; metadata: Record<string, unknown> }>;
export type ResolverCallback = (slug: string) => Promise<{ provider: string; connectionString?: string; config: Record<string, unknown> }>;

// ---------------------------------------------------------------------------
// chartml 5.0 — DataSourceProvider / hooks / cache backend interfaces
// ---------------------------------------------------------------------------

/**
 * Resolved spec passed to a `DataSourceProvider` callback. Mirrors the Rust
 * `chartml_core::resolver::FetchRequest` with camelCase keys for JS ergonomics.
 *
 * `$param.foo` references in the original YAML are already substituted by the
 * time the callback runs — providers see literal values, never `$param`.
 */
export interface FetchRequest {
  /** User-chosen name within the chart spec (`null` for unnamed flat data). */
  sourceName: string | null;
  /** Fully resolved flat-form data spec. */
  spec: InlineDataSpec;
  /** Per-request HTTP headers (for HTTP-style providers). */
  headers: Record<string, string>;
  /** Tenant / workspace namespace (`null` for single-tenant deployments). */
  namespace: string | null;
}

/**
 * Resolved inline-data spec shape. Always one of `rows` (literal data),
 * `url`/`endpoint` (HTTP), or `datasource`+`query` (provider lookup).
 */
export interface InlineDataSpec {
  /** Provider dispatch key override (`"inline"`, `"http"`, custom). */
  provider?: string;
  /** Literal row data (set when the YAML used `data: { rows: [...] }`). */
  rows?: Record<string, unknown>[];
  /** HTTP URL — set when the YAML used `data: { url: "..." }`. */
  url?: string;
  /** Alternate HTTP endpoint key — synonym for `url`. */
  endpoint?: string;
  /** Datasource slug — set when the YAML used `data: { datasource: "..." }`. */
  datasource?: string;
  /** SQL/DSL query — paired with `datasource`. */
  query?: string;
  /** Cache TTL hint as a humantime string (`"5m"`, `"30s"`, ...). */
  cacheTtl?: string;
  /** Component-layer `auto_refresh` hint. */
  cacheAutoRefresh?: boolean;
}

/**
 * Provider callback signature. Receive the resolved spec, return the rows
 * (canonical chartml shape) OR an Arrow IPC `Uint8Array` blob.
 *
 * Errors (rejected promise OR thrown exception) surface as `FetchError::Other`
 * inside the resolver and bubble up to the caller as a `ChartError`.
 */
export type ProviderCallback = (request: FetchRequest) => Promise<FetchResult>;

/**
 * Provider callback return shape. `data` is either an array of row objects
 * or a `Uint8Array` of Arrow IPC bytes (decoded server-side); `metadata` is
 * preserved on the cached entry so cache-hits return identical metadata.
 */
export interface FetchResult {
  /** Rows (object array) OR an Arrow IPC blob. */
  data: Record<string, unknown>[] | Uint8Array;
  /** Free-form provider metadata (e.g. `bytesBilled`, `rowsReturned`). */
  metadata?: Record<string, unknown>;
}

/**
 * Pipeline-phase progress event handed to `ResolverHooksInterface.onProgress`.
 */
export interface ProgressEventDto {
  phase: 'fetch' | 'transform' | 'render';
  sourceName: string | null;
  /** Bytes/rows loaded so far (provider-supplied; usually `null`). */
  loaded: number | null;
  /** Total expected (provider-supplied; usually `null`). */
  total: number | null;
  message: string;
}

/**
 * Cache-hit event. `key` is the hex-encoded `u64` cache key (e.g.
 * `"0xdeadbeef..."`) — re-parse via `BigInt(event.key)` if you need the
 * numeric form for `resolverInvalidate(...)`.
 */
export interface CacheHitEventDto {
  key: string;
  sourceName: string | null;
  tier: 'memory' | 'persistent';
  ageMs: number;
}

export interface CacheMissEventDto {
  key: string;
  sourceName: string | null;
  /**
   * Camel-case to match the JS-idiomatic serialization the WASM bridge emits
   * (Rust `MissReason::NotFound` → `"notFound"`, etc.). Matching exactly is
   * load-bearing — JS code doing `if (event.reason === 'NotFound')` would be
   * silently broken.
   */
  reason: 'notFound' | 'expired' | 'invalidated';
}

export interface ErrorEventDto {
  phase: 'fetch' | 'transform' | 'render';
  sourceName: string | null;
  error: string;
}

/**
 * Optional handlers passed to `ChartML.setHooks(...)`. Every property is
 * optional; missing handlers are silent no-ops. Handlers may return
 * `Promise<void>` and the resolver will await them, but failures inside a
 * handler are logged via `tracing` and dropped (hooks are documented as
 * fire-and-forget — a slow telemetry sink cannot stall the pipeline).
 */
export interface ResolverHooksInterface {
  onProgress?: (event: ProgressEventDto) => void | Promise<void>;
  onCacheHit?: (event: CacheHitEventDto) => void | Promise<void>;
  onCacheMiss?: (event: CacheMissEventDto) => void | Promise<void>;
  onError?: (event: ErrorEventDto) => void | Promise<void>;
}

/**
 * Cached entry shape exchanged with custom JS-side cache backends.
 *
 * Keys are passed as hex strings (`"0xdeadbeef..."`) so they survive the JS
 * `Number` 53-bit precision cap; backends that want the numeric form can
 * `BigInt(key)`.
 */
export interface CachedEntryDto {
  /** Row data, normalized to plain objects. */
  rows: Record<string, unknown>[];
  /** Epoch milliseconds when the entry was first inserted. */
  fetchedAtMs: number;
  ttlMs: number;
  /** Bulk-invalidation tags (`"slug:foo"`, `"namespace:bar"`). */
  tags: string[];
  /** Provider-supplied metadata preserved across cache hits. */
  metadata: Record<string, unknown>;
}

/**
 * Optional handlers passed to `ChartML.setCache(...)` when supplying a JS
 * object backend instead of one of the built-in `"memory"` / `"indexeddb"`
 * shorthand strings. Every method may be `async` (return a Promise).
 */
export interface CacheBackendInterface {
  get(keyHex: string): Promise<CachedEntryDto | null>;
  put(keyHex: string, entry: CachedEntryDto): Promise<void>;
  invalidate(keyHex: string): Promise<void>;
  invalidateByTag(tag: string): Promise<void>;
  clear(): Promise<void>;
  /** Optional graceful-shutdown hook. */
  shutdown?(): Promise<void>;
}

/**
 * Opaque handle returned by `ChartML.fetch(...)` / consumed by
 * `ChartML.transform(...)`. Field shape is intentionally undocumented — the
 * handle is a slab id plus a kind tag and is meaningful only to the WASM
 * binding. Handles are single-use: `transform`/`renderPreparedToSvg` consume
 * them, so calling `transform` twice on the same handle errors.
 */
export type FetchedChart = unknown;

/** Opaque handle returned by `ChartML.transform(...)`. See `FetchedChart`. */
export type PreparedChart = unknown;

// Supporting types (matches Rust serde output)

export interface ViewBox {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface ElementData {
  label: string;
  value: string;
  series?: string;
  raw: Record<string, unknown>;
}

// Transform is a serde enum — serializes as { "Translate": [x, y] }, etc.
export type Transform =
  | { Translate: [number, number] }
  | { Rotate: [number, number, number] }
  | { Multiple: Transform[] };

// TextAnchor serializes as a plain string: "Start", "Middle", "End"
export type TextAnchor = 'Start' | 'Middle' | 'End';

// ChartElement discriminated union (matches Rust serde output with tag="type", rename_all="camelCase")
export type ChartElement =
  | SvgElement
  | GroupElement
  | RectElement
  | PathElement
  | CircleElement
  | LineElement
  | TextElement
  | DivElement
  | SpanElement;

export interface SvgElement {
  type: 'svg';
  viewbox: ViewBox;
  width?: number;
  height?: number;
  class: string;
  children: ChartElement[];
}

export interface GroupElement {
  type: 'group';
  class: string;
  transform?: Transform;
  children: ChartElement[];
}

export interface RectElement {
  type: 'rect';
  x: number;
  y: number;
  width: number;
  height: number;
  fill: string;
  stroke?: string;
  class: string;
  data?: ElementData;
}

export interface PathElement {
  type: 'path';
  d: string;
  fill?: string;
  stroke?: string;
  strokeWidth?: number;
  strokeDasharray?: string;
  opacity?: number;
  class: string;
  data?: ElementData;
}

export interface CircleElement {
  type: 'circle';
  cx: number;
  cy: number;
  r: number;
  fill: string;
  stroke?: string;
  class: string;
  data?: ElementData;
}

export interface LineElement {
  type: 'line';
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  stroke: string;
  strokeWidth?: number;
  strokeDasharray?: string;
  class: string;
}

export interface TextElement {
  type: 'text';
  x: number;
  y: number;
  content: string;
  anchor: TextAnchor;
  dominantBaseline?: string;
  transform?: Transform;
  fontSize?: string;
  fontWeight?: string;
  fill?: string;
  class: string;
  data?: ElementData;
}

export interface DivElement {
  type: 'div';
  class: string;
  style: Record<string, string>;
  children: ChartElement[];
}

export interface SpanElement {
  type: 'span';
  class: string;
  style: Record<string, string>;
  content: string;
}
