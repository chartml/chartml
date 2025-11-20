/**
 * Shared chart rendering utilities for ChartML markdown plugins
 */

import { createErrorElement } from './errorDisplay.js';

/**
 * Render a single chart into a container
 * Shared logic for rendering ChartML specs
 *
 * @param {HTMLElement} container - DOM element to render into
 * @param {string|object} spec - ChartML YAML string or parsed spec
 * @param {ChartML} chartmlInstance - ChartML instance with plugins registered
 * @returns {Promise<Chart>} Chart instance
 *
 * @example
 * const chartInstance = await renderChart(container, yamlSpec, chartml);
 */
export async function renderChart(container, spec, chartmlInstance) {
  try {
    // Clear container
    container.innerHTML = '';

    // Render chart and return Chart instance
    const chartInstance = await chartmlInstance.render(spec, container);
    return chartInstance;

  } catch (error) {
    console.error('[ChartML markdown] Render error:', error);

    // Show error in container
    container.innerHTML = '';
    const errorElement = createErrorElement(error);
    container.appendChild(errorElement);

    throw error; // Re-throw so caller can handle
  }
}

/**
 * Get expected dimensions for a chart before rendering
 * Useful for preventing layout shift
 *
 * @param {object} spec - Parsed ChartML spec
 * @param {ChartML} chartmlInstance - ChartML instance
 * @returns {Object} { width, height }
 */
export function getExpectedDimensions(spec, chartmlInstance) {
  try {
    return chartmlInstance.getExpectedDimensions(spec);
  } catch (error) {
    console.warn('[ChartML markdown] Failed to calculate dimensions:', error);
    return { width: null, height: 400 }; // Fallback to default height
  }
}

/**
 * Map colSpan (1-12) to responsive grid column classes
 * Mobile = full width, desktop = specified span
 *
 * @param {number} colSpan - Column span from 1-12
 * @returns {string} Tailwind CSS classes
 */
export function getColSpanClass(colSpan) {
  const classMap = {
    1: 'col-span-12 md:col-span-1',
    2: 'col-span-12 md:col-span-2',
    3: 'col-span-12 md:col-span-3',
    4: 'col-span-12 md:col-span-4',
    5: 'col-span-12 md:col-span-5',
    6: 'col-span-12 md:col-span-6',
    7: 'col-span-12 md:col-span-7',
    8: 'col-span-12 md:col-span-8',
    9: 'col-span-12 md:col-span-9',
    10: 'col-span-12 md:col-span-10',
    11: 'col-span-12 md:col-span-11',
    12: 'col-span-12'
  };
  return classMap[colSpan] || 'col-span-12';
}
