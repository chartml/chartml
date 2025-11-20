/**
 * Metric Card Chart Renderer Plugin for ChartML
 *
 * Self-contained metric card renderer with:
 * - Large metric display
 * - Comparison indicators (up/down arrows)
 * - Trend coloring (good/bad)
 * - Custom formatting
 */

import * as d3 from 'd3';

/**
 * Render a metric card using D3
 * @private
 */
function renderMetricCard(container, data, config) {
  // Clear container
  container.innerHTML = '';

  // Extract config (ChartML mapper already extracted values from data)
  const { value, label, format, comparison, align = 'center', showLabel = true } = config;

  // Calculate responsive font size based on container width
  const containerWidth = container.offsetWidth || 300;
  let valueFontSize = '2.5rem';  // Default for wide containers
  if (containerWidth < 200) {
    valueFontSize = '1.5rem';
  } else if (containerWidth < 300) {
    valueFontSize = '2rem';
  }

  // Create container div
  const card = d3.select(container)
    .append('div')
    .style('display', 'flex')
    .style('flex-direction', 'column')
    .style('justify-content', 'center')
    .style('align-items', align)
    .style('height', '100%')
    .style('padding', '1rem');

  // Add label if enabled
  if (showLabel && label) {
    card.append('div')
      .style('font-size', '0.875rem')
      .style('color', '#6b7280')
      .style('margin-bottom', '0.5rem')
      .text(label);
  }

  // Add value
  const formattedValue = value != null ? formatValue(value, format) : '—';
  card.append('div')
    .style('font-size', valueFontSize)
    .style('font-weight', '700')
    .style('color', '#111827')
    .text(formattedValue);

  // Add comparison if provided
  if (comparison && comparison.direction !== 'neutral') {
    const { percentChange, direction, isGood } = comparison;

    let color = '#6b7280'; // neutral gray
    if (isGood === true) {
      color = '#10b981'; // green
    } else if (isGood === false) {
      color = '#ef4444'; // red
    }

    const arrow = direction === 'up' ? '▲' : '▼';
    const percentText = `${Math.abs(percentChange).toFixed(1)}%`;

    card.append('div')
      .style('font-size', '0.875rem')
      .style('color', color)
      .style('font-weight', '500')
      .style('margin-top', '0.5rem')
      .text(`${arrow} ${percentText}`);
  }
}

/**
 * Format value using D3 format specs
 * @private
 * @see https://github.com/d3/d3-format
 */
function formatValue(value, format) {
  if (!format) {
    return value.toLocaleString();
  }

  try {
    // Use D3's format function which handles all standard specs:
    // .1% - percent with 1 decimal
    // $,.2f - currency with thousands separator
    // .2f - fixed point with 2 decimals
    // .3s - SI-prefix notation (1.23k, 1.23M, etc)
    // d - integer
    const formatter = d3.format(format);
    return formatter(value);
  } catch (error) {
    console.warn(`[MetricRenderer] Invalid format spec "${format}":`, error);
    return value.toLocaleString();
  }
}

/**
 * Create metric card renderer
 *
 * @returns {Function} Renderer function compatible with ChartML
 *
 * @example
 * import { createMetricRenderer } from '@chartml/chart-metric';
 *
 * const chartml = new ChartML();
 * chartml.registerChartRenderer('metric', createMetricRenderer());
 */
export function createMetricRenderer() {
  /**
   * Render a metric card
   *
   * @param {HTMLElement} container - DOM element to render into
   * @param {Array} data - Chart data (passed by ChartML but not used - mapper extracts values)
   * @param {Object} config - Chart configuration from mapToMetricCard
   * @param {*} config.value - The metric value to display (already extracted from data)
   * @param {string} config.label - Metric label
   * @param {string} [config.format] - Format specification (currency, percent, .2f, etc)
   * @param {Object} [config.comparison] - Comparison data (already calculated)
   * @param {number} [config.comparison.percentChange] - Percentage change
   * @param {string} [config.comparison.direction] - Direction: 'up', 'down', or 'neutral'
   * @param {boolean} [config.comparison.isGood] - Whether this change is good (true), bad (false), or neutral (null)
   * @param {string} [config.align] - Alignment (left, center, right)
   * @param {boolean} [config.showLabel] - Show label (default true)
   */
  const renderMetric = function(container, data, config) {
    renderMetricCard(container, data, config);
  };

  // Implement optional ChartML plugin interface for custom default dimensions
  // Metric cards are much shorter than standard charts
  renderMetric.getDefaultDimensions = () => ({ height: 150 });

  return renderMetric;
}

export default createMetricRenderer;

// Auto-register on import
import { globalRegistry } from '@chartml/core';
const metricRenderer = createMetricRenderer();
globalRegistry.registerChartRenderer('metric', metricRenderer);
