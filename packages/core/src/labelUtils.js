/**
 * Label Utilities for ChartML
 *
 * Collision detection and adaptive label strategies for chart axes and legends.
 * Automatically handles label overlap with strategies: horizontal, rotated, truncated, sampled.
 *
 * External chart plugins can use these utilities for consistent label behavior across all chart types.
 *
 * @example
 * import { determineLabelStrategy, applyLabelStrategy } from '@chartml/core';
 *
 * const labels = data.map(d => d.category);
 * const strategy = determineLabelStrategy(labels, chartWidth);
 * applyLabelStrategy(xAxis, strategy.strategy, strategy.metadata, { tooltip, container });
 */

import * as d3 from 'd3';
import { positionTooltip } from './tooltipUtils.js';

// Default axis label styling - can be overridden via config
export const DEFAULT_LABEL_FONT_SIZE = '12px';
export const DEFAULT_LABEL_FONT_FAMILY = 'system-ui';

/**
 * Measure the width of text labels before rendering
 * Creates a temporary SVG element to measure actual rendered widths
 *
 * @param {Array<string>} labels - Array of label strings to measure
 * @param {string} fontSize - Font size (default: '12px')
 * @param {string} fontFamily - Font family (default: 'system-ui')
 * @returns {Array<number>} Array of label widths in pixels
 *
 * @example
 * const widths = measureLabelWidths(['Jan', 'Feb', 'Mar']);
 * // Returns: [25, 28, 30] (approximate pixel widths)
 */
export function measureLabelWidths(labels, fontSize = DEFAULT_LABEL_FONT_SIZE, fontFamily = DEFAULT_LABEL_FONT_FAMILY) {
  // Create temporary SVG for measurement
  const svg = d3.select('body')
    .append('svg')
    .style('position', 'absolute')
    .style('visibility', 'hidden')
    .style('width', '0')
    .style('height', '0');

  const measurements = labels.map(label => {
    const text = svg.append('text')
      .style('font-size', fontSize)
      .style('font-family', fontFamily)
      .text(label);

    const width = text.node().getComputedTextLength();
    text.remove();
    return width;
  });

  svg.remove();
  return measurements;
}

/**
 * Determine the best label strategy based on available space and label widths
 *
 * Analyzes labels and returns optimal strategy with metadata for application.
 * Strategies (in priority order):
 * 1. horizontal - Labels fit comfortably with spacing
 * 2. rotated - Rotate -45° for moderate label counts
 * 3. truncated - Truncate long labels with ellipsis
 * 4. sampled - Show subset of labels for very many categories
 *
 * @param {Array<string>} labels - Array of label strings
 * @param {number} chartWidth - Available width for labels in pixels
 * @param {Object} config - Configuration options
 * @param {string} config.labelStrategy - Force strategy ('auto', 'horizontal', 'rotated', 'truncated', 'sampled')
 * @param {number} config.maxLabelWidth - Max width for truncated labels (default: 120px)
 * @param {number} config.rotationAngle - Rotation angle in degrees (default: -45)
 * @param {number} config.minLabelSpacing - Minimum space between labels (default: 10px)
 * @param {number} config.maxLabelsBeforeSampling - Threshold for sampling (default: 50)
 * @returns {Object} Strategy result { strategy: string, metadata: {...} }
 *
 * @example
 * const result = determineLabelStrategy(['Category A', 'B', 'C'], 300);
 * // Returns: { strategy: 'horizontal', metadata: { avgWidth: 50, ... } }
 */
export function determineLabelStrategy(labels, chartWidth, config = {}) {
  const {
    labelStrategy = 'auto',
    maxLabelWidth = 120,
    rotationAngle = -45, // Fixed at -45° for consistency
    minLabelSpacing = 10,
    maxLabelsBeforeSampling = 50
  } = config;

  // If user forces a strategy, honor it
  if (labelStrategy !== 'auto') {
    return {
      strategy: labelStrategy,
      metadata: { forced: true, rotationAngle, maxLabelWidth }
    };
  }

  const labelCount = labels.length;
  const availableSpacePerLabel = chartWidth / labelCount;

  // Measure actual label widths
  const labelWidths = measureLabelWidths(labels);
  const maxWidth = Math.max(...labelWidths);
  const avgWidth = labelWidths.reduce((a, b) => a + b, 0) / labelWidths.length;

  // Strategy 1: Try horizontal (default)
  // All labels fit with comfortable spacing horizontally
  if (avgWidth + minLabelSpacing <= availableSpacePerLabel) {
    return {
      strategy: 'horizontal',
      metadata: { avgWidth, availableSpacePerLabel, labelWidths }
    };
  }

  // Strategy 2: Rotation at -45° (industry standard)
  // Calculate required margin based on max label width
  if (labelCount <= 40) {
    const radians = 45 * Math.PI / 180;
    const requiredVerticalSpace = maxWidth * Math.sin(radians);

    // Cap at 150px to prevent ridiculously long labels from breaking layout
    const requiredMargin = Math.min(Math.ceil(requiredVerticalSpace) + 15, 150);

    return {
      strategy: 'rotated',
      metadata: {
        rotationAngle,
        avgWidth,
        maxWidth,
        requiredMargin,
        labelWidths
      }
    };
  }

  // Strategy 3: Try truncation
  // If we can fit truncated labels horizontally
  if (maxLabelWidth + minLabelSpacing <= availableSpacePerLabel && labelCount <= 50) {
    return {
      strategy: 'truncated',
      metadata: { maxLabelWidth, originalMaxWidth: maxWidth, labelWidths }
    };
  }

  // Strategy 4: Intelligent sampling
  // For VERY many categories (30+) where even rotation won't help
  if (labelCount >= 30) {
    return {
      strategy: 'sampled',
      metadata: {
        totalLabels: labelCount,
        maxLabelsToShow: Math.max(Math.floor(chartWidth / 120), 5),
        reason: 'too_many_categories'
      }
    };
  }

  // Fallback: If we have fewer than 30 categories but tight space, use rotation anyway
  return {
    strategy: 'rotated',
    metadata: { rotationAngle, maxWidth, fallback: true, labelWidths }
  };
}

/**
 * Apply horizontal label strategy (default, no transformation)
 *
 * @param {d3.Selection} axisSelection - D3 selection of axis group
 * @param {string} fontSize - Font size (default: DEFAULT_LABEL_FONT_SIZE)
 * @param {string} fontFamily - Font family (default: DEFAULT_LABEL_FONT_FAMILY)
 * @returns {number} Additional margin needed (always 0 for horizontal)
 */
export function applyHorizontalLabels(axisSelection, fontSize = DEFAULT_LABEL_FONT_SIZE, fontFamily = DEFAULT_LABEL_FONT_FAMILY) {
  axisSelection.selectAll('text')
    .style('font-size', fontSize)
    .style('font-family', fontFamily)
    .style('text-anchor', 'middle');

  return 0; // No additional margin needed
}

/**
 * Apply rotated label strategy
 *
 * @param {d3.Selection} axisSelection - D3 selection of axis group
 * @param {number} angle - Rotation angle in degrees (default: -45)
 * @param {Array<number>} labelWidths - Pre-measured label widths for margin calculation
 * @param {string} fontSize - Font size (default: DEFAULT_LABEL_FONT_SIZE)
 * @param {string} fontFamily - Font family (default: DEFAULT_LABEL_FONT_FAMILY)
 * @returns {number} Additional margin needed at bottom in pixels
 */
export function applyRotatedLabels(axisSelection, angle = -45, labelWidths = [], fontSize = DEFAULT_LABEL_FONT_SIZE, fontFamily = DEFAULT_LABEL_FONT_FAMILY) {
  const angleRad = angle * Math.PI / 180;
  const maxWidth = labelWidths.length > 0 ? Math.max(...labelWidths) : 100;

  // Calculate additional margin needed for rotated labels
  // Height = width * |sin(angle)|
  const additionalMargin = maxWidth * Math.abs(Math.sin(angleRad));

  axisSelection.selectAll('text')
    .style('font-size', fontSize)
    .style('font-family', fontFamily)
    .style('text-anchor', 'end')
    .attr('dx', '-0.8em')
    .attr('dy', '0.15em')
    .attr('transform', `rotate(${angle})`);

  return Math.ceil(additionalMargin);
}

/**
 * Apply truncated label strategy with ellipsis
 *
 * @param {d3.Selection} axisSelection - D3 selection of axis group
 * @param {number} maxWidth - Maximum label width in pixels
 * @param {Object} options - Additional options
 * @param {d3.Selection} options.tooltip - Optional tooltip for showing full text
 * @param {HTMLElement} options.container - Container for tooltip positioning
 * @param {string} options.fontSize - Font size (default: DEFAULT_LABEL_FONT_SIZE)
 * @param {string} options.fontFamily - Font family (default: DEFAULT_LABEL_FONT_FAMILY)
 * @returns {number} Additional margin needed (always 0 for truncated)
 */
export function applyTruncatedLabels(axisSelection, maxWidth, options = {}) {
  const {
    tooltip = null,
    container = null,
    fontSize = DEFAULT_LABEL_FONT_SIZE,
    fontFamily = DEFAULT_LABEL_FONT_FAMILY
  } = options;

  axisSelection.selectAll('text')
    .style('font-size', fontSize)
    .style('font-family', fontFamily)
    .style('text-anchor', 'middle')
    .each(function(d) {
      const text = d3.select(this);
      const fullText = String(d);

      // Store full text for tooltip
      text.attr('data-full-text', fullText);

      // Truncate text to fit maxWidth
      let truncated = fullText;
      text.text(truncated);

      while (text.node().getComputedTextLength() > maxWidth && truncated.length > 0) {
        truncated = truncated.slice(0, -1);
        text.text(truncated + '…');
      }
    });

  // Add tooltip interactions if tooltip and container provided
  if (tooltip && container) {
    axisSelection.selectAll('text')
      .style('cursor', 'help')
      .on('mouseenter', function(event) {
        const fullText = d3.select(this).attr('data-full-text');
        const currentText = d3.select(this).text();

        // Only show tooltip if text is truncated
        if (currentText.endsWith('…')) {
          tooltip
            .style('opacity', 1)
            .html(fullText);
        }
      })
      .on('mousemove', function(event) {
        positionTooltip(tooltip, event);
      })
      .on('mouseleave', function() {
        tooltip.style('opacity', 0);
      });
  }

  return 0; // No additional margin needed
}

/**
 * Get strategic indices for intelligent sampling
 * Always includes first, last, and evenly distributed middle points
 *
 * @param {number} totalCount - Total number of labels
 * @param {number} targetCount - Target number of labels to show
 * @returns {Array<number>} Array of indices to display
 *
 * @example
 * getStrategicIndices(10, 5);
 * // Returns: [0, 2, 5, 7, 9] (first, last, evenly spaced)
 */
export function getStrategicIndices(totalCount, targetCount) {
  if (totalCount <= targetCount) {
    return Array.from({ length: totalCount }, (_, i) => i);
  }

  const indices = new Set();

  // Always include first and last
  indices.add(0);
  indices.add(totalCount - 1);

  // Calculate step for even distribution
  const step = totalCount / (targetCount - 1);

  for (let i = 1; i < targetCount - 1; i++) {
    indices.add(Math.round(i * step));
  }

  return Array.from(indices).sort((a, b) => a - b);
}

/**
 * Apply intelligent sampling strategy
 * Shows strategic subset of labels (first, last, evenly spaced)
 *
 * @param {Function} axisGenerator - D3 axis generator (d3.axisBottom() or d3.axisLeft())
 * @param {Array} domain - Full domain array
 * @param {number} maxLabels - Maximum number of labels to show
 * @returns {Function} Modified axis generator with sampled tickValues
 *
 * @example
 * const sampled = applySampledLabels(d3.axisBottom(x), allCategories, 10);
 * g.call(sampled);
 */
export function applySampledLabels(axisGenerator, domain, maxLabels) {
  const indices = getStrategicIndices(domain.length, maxLabels);
  const tickValues = indices.map(i => domain[i]);

  return axisGenerator.tickValues(tickValues);
}

/**
 * High-level function to apply label strategy
 * Convenience wrapper that calls the appropriate strategy function
 *
 * @param {d3.Selection} axisSelection - D3 selection of axis group
 * @param {string} strategy - Strategy name ('horizontal', 'rotated', 'truncated', 'sampled')
 * @param {Object} metadata - Metadata from determineLabelStrategy()
 * @param {Object} options - Additional options (tooltip, container, fontSize, fontFamily)
 * @returns {number} Additional margin needed in pixels
 *
 * @example
 * const result = determineLabelStrategy(labels, chartWidth);
 * const additionalMargin = applyLabelStrategy(xAxis, result.strategy, result.metadata);
 */
export function applyLabelStrategy(axisSelection, strategy, metadata = {}, options = {}) {
  const {
    fontSize = DEFAULT_LABEL_FONT_SIZE,
    fontFamily = DEFAULT_LABEL_FONT_FAMILY,
    tooltip = null,
    container = null
  } = options;

  switch (strategy) {
    case 'horizontal':
      return applyHorizontalLabels(axisSelection, fontSize, fontFamily);

    case 'rotated':
      return applyRotatedLabels(
        axisSelection,
        metadata.rotationAngle || -45,
        metadata.labelWidths || [],
        fontSize,
        fontFamily
      );

    case 'truncated':
      return applyTruncatedLabels(
        axisSelection,
        metadata.maxLabelWidth || 120,
        { tooltip, container, fontSize, fontFamily }
      );

    case 'sampled':
      // Note: Sampled strategy must be applied to axis generator before rendering
      // This case is here for completeness but typically handled differently
      console.warn('[labelUtils] Sampled strategy should be applied via applySampledLabels() before axis rendering');
      return 0;

    default:
      console.warn(`[labelUtils] Unknown strategy: ${strategy}, using horizontal`);
      return applyHorizontalLabels(axisSelection, fontSize, fontFamily);
  }
}
