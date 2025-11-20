/**
 * @chartml/markdown-common
 *
 * Shared utilities and styles for ChartML markdown plugins
 * Used by both @chartml/markdown-react and @chartml/markdown-it
 */

export { createErrorElement, createErrorHTML } from './errorDisplay.js';
export { renderChart, getExpectedDimensions, getColSpanClass } from './chartRenderer.js';

// Re-export for convenience
export { ChartML } from '@chartml/core';
