/**
 * Tooltip Utilities for ChartML
 *
 * Centralized tooltip creation to ensure consistent styling across all chart types.
 * Chart plugins can use this utility for standard ChartML tooltip behavior,
 * or create custom tooltips if needed for specialized use cases.
 */

import * as d3 from 'd3';

/**
 * Create a standard ChartML tooltip
 *
 * This creates a tooltip div with the `.chart-tooltip` class that applications
 * can style via CSS. The tooltip is positioned absolutely within the container
 * and includes smooth fade in/out transitions.
 *
 * @param {HTMLElement} container - Container element for tooltip
 * @returns {d3.Selection} D3 selection of tooltip div
 *
 * @example
 * import { createChartTooltip } from '@chartml/core';
 *
 * function myChartRenderer(container, data, config) {
 *   const tooltip = createChartTooltip(container);
 *
 *   // Use tooltip in event handlers
 *   bars.on('mouseenter', (event, d) => {
 *     tooltip
 *       .style('opacity', 1)
 *       .html(`<strong>${d.label}</strong><br/>${d.value}`);
 *   });
 * }
 */
export function createChartTooltip(container) {
  return d3.select(container)
    .append('div')
    .attr('class', 'chart-tooltip')
    .style('transition', 'opacity 0.2s'); // Smooth fade in/out
}

/**
 * Position a tooltip near the mouse cursor
 *
 * Uses fixed positioning with viewport coordinates to prevent container
 * expansion and scrollbar issues. All chart types should use this helper
 * for consistent tooltip positioning.
 *
 * @param {d3.Selection} tooltip - D3 selection of tooltip element
 * @param {MouseEvent} event - Mouse event from event handler
 * @param {Object} options - Positioning options
 * @param {number} options.offsetX - Horizontal offset from cursor (default: 10)
 * @param {number} options.offsetY - Vertical offset from cursor (default: -10)
 *
 * @example
 * import { createChartTooltip, positionTooltip } from '@chartml/core';
 *
 * const tooltip = createChartTooltip(container);
 *
 * bars.on('mousemove', (event) => {
 *   positionTooltip(tooltip, event);
 * });
 */
export function positionTooltip(tooltip, event, options = {}) {
  const { offsetX = 10, offsetY = -10 } = options;

  tooltip
    .style('left', (event.clientX + offsetX) + 'px')
    .style('top', (event.clientY + offsetY) + 'px');
}
