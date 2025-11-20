# @chartml/markdown-common

Shared utilities and styles for ChartML markdown plugins. Internal package used by `@chartml/markdown-react` and `@chartml/markdown-it`.

## Overview

This package provides common functionality shared between ChartML's markdown integrations:
- Chart rendering utilities
- Error display components
- Grid layout helpers
- Shared CSS styles

**Note:** This is typically a dependency of other packages and not directly used by end users. See `@chartml/markdown-react` or `@chartml/markdown-it` for user-facing packages.

## Installation

```bash
npm install @chartml/markdown-common
```

Or install a higher-level package that includes this:

```bash
npm install @chartml/markdown-react  # For React
npm install @chartml/markdown-it     # For static sites
```

## API Reference

### Chart Rendering

#### `renderChart(container, spec, chartmlInstance)`

Render a single chart into a container with error handling.

**Parameters:**
- `container` (HTMLElement) - DOM element to render into
- `spec` (string | object) - ChartML YAML string or parsed spec
- `chartmlInstance` (ChartML) - ChartML instance with plugins registered

**Returns:** `Promise<Chart>` - Chart instance

**Example:**

```javascript
import { renderChart } from '@chartml/markdown-common';
import { ChartML } from '@chartml/core';

const chartml = new ChartML();
const spec = `
  type: chart
  version: 1
  data: [...]
  visualize: { type: bar }
`;

const chartInstance = await renderChart(container, spec, chartml);
```

#### `getExpectedDimensions(spec, chartmlInstance)`

Get expected chart dimensions before rendering to prevent layout shift.

**Parameters:**
- `spec` (object) - Parsed ChartML spec
- `chartmlInstance` (ChartML) - ChartML instance

**Returns:** `{ width: number|null, height: number }`

**Example:**

```javascript
import { getExpectedDimensions } from '@chartml/markdown-common';

const dimensions = getExpectedDimensions(parsedSpec, chartml);
container.style.minHeight = `${dimensions.height}px`;

// Then render the chart
await renderChart(container, parsedSpec, chartml);
```

#### `getColSpanClass(colSpan)`

Map colSpan value (1-12) to responsive Tailwind CSS grid classes.

**Parameters:**
- `colSpan` (number) - Column span from 1 to 12

**Returns:** `string` - Tailwind CSS classes

**Responsive Behavior:**
- Mobile: Always full width (`col-span-12`)
- Desktop: Uses specified span (`md:col-span-{n}`)

**Example:**

```javascript
import { getColSpanClass } from '@chartml/markdown-common';

const className = getColSpanClass(6);  // 'col-span-12 md:col-span-6'
chartContainer.className = className;
```

**Class Mappings:**

| colSpan | CSS Classes |
|---------|-------------|
| 1 | `col-span-12 md:col-span-1` |
| 6 | `col-span-12 md:col-span-6` |
| 12 | `col-span-12` |

### Error Display

#### `createErrorElement(error)`

Create a styled error DOM element for client-side rendering.

**Parameters:**
- `error` (Error | string) - Error object or error message

**Returns:** `HTMLElement` - DOM element with `.chartml-error` class

**Example:**

```javascript
import { createErrorElement } from '@chartml/markdown-common';

try {
  await renderChart(container, spec, chartml);
} catch (error) {
  const errorEl = createErrorElement(error);
  container.appendChild(errorEl);
}
```

#### `createErrorHTML(error)`

Create error HTML string for server-side rendering.

**Parameters:**
- `error` (Error | string) - Error object or error message

**Returns:** `string` - HTML string with proper escaping

**Example:**

```javascript
import { createErrorHTML } from '@chartml/markdown-common';

try {
  // Server-side rendering
  html = generateChartHTML(spec);
} catch (error) {
  html = createErrorHTML(error);
}
```

**Security:** Automatically escapes HTML to prevent XSS attacks.

## Shared Styles

The package includes `styles.css` with common styles for chart errors and containers.

### Using Styles

#### In JavaScript/React

```javascript
import '@chartml/markdown-common/styles.css';
```

#### In CSS

```css
@import '@chartml/markdown-common/styles.css';
```

### Available Classes

**`.chartml-error`** - Error display styling
```css
.chartml-error {
  background: #fee2e2;
  border: 1px solid #f87171;
  color: #991b1b;
  padding: 1rem;
  border-radius: 0.375rem;
  margin: 1rem 0;
}
```

## Usage in Plugin Development

### Creating a Custom Markdown Plugin

```javascript
import {
  renderChart,
  getExpectedDimensions,
  getColSpanClass,
  createErrorElement
} from '@chartml/markdown-common';
import { ChartML } from '@chartml/core';

export function MyMarkdownPlugin(markdownProcessor, chartml) {
  markdownProcessor.registerCodeBlock('chartml', async (code, container) => {
    try {
      // Parse the code
      const spec = parseChartML(code);

      // Prevent layout shift
      const { height } = getExpectedDimensions(spec, chartml);
      container.style.minHeight = `${height}px`;

      // Handle grid layout
      const colSpan = spec?.layout?.colSpan || 12;
      container.className = getColSpanClass(colSpan);

      // Render chart
      await renderChart(container, spec, chartml);

    } catch (error) {
      const errorEl = createErrorElement(error);
      container.appendChild(errorEl);
    }
  });
}
```

## TypeScript Support

Type definitions are included:

```typescript
import type {
  renderChart,
  getExpectedDimensions,
  getColSpanClass,
  createErrorElement,
  createErrorHTML
} from '@chartml/markdown-common';
```

## Packages Using This Library

- **@chartml/markdown-react** - React-markdown integration
- **@chartml/markdown-it** - Markdown-it plugin for static sites

## Browser Support

- Modern browsers with ES6 support
- IE11+ (with polyfills)

## Documentation

- **ChartML Specification**: https://chartml.org/spec
- **API Reference**: https://chartml.org/api
- **Plugin Development**: https://chartml.org/plugin-architecture

## Contributing

This is an internal shared package. If you're building a new markdown integration for ChartML, use these utilities to maintain consistency across plugins.

### Guidelines

1. **Error Handling** - Always use `createErrorElement` or `createErrorHTML` for consistent error display
2. **Layout Shift Prevention** - Use `getExpectedDimensions` before rendering
3. **Responsive Grids** - Use `getColSpanClass` for consistent grid behavior
4. **Rendering** - Use `renderChart` for consistent error handling

## License

MIT © 2025 Alytic Pty Ltd

## Powered By

ChartML is maintained by the team at [Kyomi](https://kyomi.ai) and is the visualization engine that powers the platform.
