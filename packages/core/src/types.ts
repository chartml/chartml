// Render options
export interface RenderOptions {
  width?: number;
  height?: number;
}

// Plugin callback types
export type RendererCallback = (rows: Record<string, unknown>[], config: Record<string, unknown>) => ChartElement;
export type DataSourceCallback = (spec: Record<string, unknown>) => Promise<Record<string, unknown>[]>;
export type TransformCallback = (rows: Record<string, unknown>[], spec: Record<string, unknown>, context: Record<string, unknown>) => Promise<{ data: Record<string, unknown>[]; metadata: Record<string, unknown> }>;
export type ResolverCallback = (slug: string) => Promise<{ provider: string; connectionString?: string; config: Record<string, unknown> }>;

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
