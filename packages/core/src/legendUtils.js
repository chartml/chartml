/**
 * Legend Utilities for ChartML
 *
 * Unified legend rendering with consistent styling across all chart types.
 * Features:
 * - Smart multi-row wrapping based on available width
 * - Overflow handling with "+N more" indicator
 * - Label truncation with tooltip on hover
 * - Hover interaction support (highlight legend item / chart element)
 */

import * as d3 from 'd3';

// Consistent legend styling constants
const LEGEND_FONT_SIZE = '11px';
const LEGEND_FONT_FAMILY = 'system-ui';
const LEGEND_TEXT_COLOR = 'var(--chartml-text)';
const SYMBOL_SIZE = 12;
const SYMBOL_RADIUS = 2;
const SYMBOL_TO_TEXT_GAP = 6;
const ITEM_PADDING = 12;
const ROW_HEIGHT = 20;
const MAX_LABEL_LENGTH = 20;
const MAX_ROWS = 3;

/**
 * Measure text width using a temporary SVG element
 */
function measureTextWidth(text, fontSize = LEGEND_FONT_SIZE, fontFamily = LEGEND_FONT_FAMILY) {
  const svg = d3.select('body')
    .append('svg')
    .style('position', 'absolute')
    .style('visibility', 'hidden');

  const textEl = svg.append('text')
    .style('font-size', fontSize)
    .style('font-family', fontFamily)
    .text(text);

  const width = textEl.node().getComputedTextLength();
  svg.remove();
  return width;
}

/**
 * Truncate label if too long, adding ellipsis
 */
function truncateLabel(label, maxLength = MAX_LABEL_LENGTH) {
  const str = String(label);
  if (str.length <= maxLength) return { text: str, truncated: false };
  return { text: str.substring(0, maxLength - 1) + '…', truncated: true };
}

/**
 * Calculate legend layout with smart wrapping
 *
 * @param {Array} items - Array of { label, color, mark, field }
 * @param {number} availableWidth - Maximum width for legend
 * @param {Object} options - Layout options
 * @returns {Object} Layout info: { rows, totalHeight, overflow, overflowCount }
 */
export function calculateLegendLayout(items, availableWidth, options = {}) {
  const {
    maxRows = MAX_ROWS,
    itemPadding = ITEM_PADDING,
    symbolSize = SYMBOL_SIZE,
    symbolToTextGap = SYMBOL_TO_TEXT_GAP,
    rowHeight = ROW_HEIGHT,
    maxLabelLength = MAX_LABEL_LENGTH
  } = options;

  const rows = [[]];
  let currentRowWidth = 0;
  let currentRowIndex = 0;
  let overflow = false;
  let overflowCount = 0;
  const overflowItems = [];

  for (let i = 0; i < items.length; i++) {
    const item = items[i];
    const { text: displayLabel, truncated } = truncateLabel(item.label, maxLabelLength);
    const textWidth = measureTextWidth(displayLabel);
    const itemWidth = symbolSize + symbolToTextGap + textWidth + itemPadding;

    // Check if item fits in current row
    if (currentRowWidth + itemWidth > availableWidth && rows[currentRowIndex].length > 0) {
      // Need new row
      currentRowIndex++;

      if (currentRowIndex >= maxRows) {
        // Exceeded max rows - rest goes to overflow
        overflow = true;
        overflowCount = items.length - i;
        for (let j = i; j < items.length; j++) {
          overflowItems.push(items[j]);
        }
        break;
      }

      rows.push([]);
      currentRowWidth = 0;
    }

    rows[currentRowIndex].push({
      ...item,
      displayLabel,
      fullLabel: item.label,
      truncated,
      textWidth,
      itemWidth,
      index: i
    });

    currentRowWidth += itemWidth;
  }

  // Calculate total height
  const totalHeight = rows.length * rowHeight;

  return {
    rows,
    totalHeight,
    overflow,
    overflowCount,
    overflowItems
  };
}

// Dash patterns for line styles (keep consistent with d3CartesianChart.js)
const LINE_STYLE_DASH_PATTERNS = { dashed: '8 4', dotted: '2 4' };

/**
 * Render legend symbol based on mark type
 */
function renderSymbol(group, mark, color, size = SYMBOL_SIZE, lineStyle = null, opacity = null) {

  if (mark === 'line') {
    const line = group.append('line')
      .attr('x1', 0)
      .attr('y1', size / 2)
      .attr('x2', size)
      .attr('y2', size / 2)
      .attr('stroke', color)
      .attr('stroke-width', 2.5)
      .attr('stroke-linecap', 'round');

    if (lineStyle && LINE_STYLE_DASH_PATTERNS[lineStyle]) {
      line.attr('stroke-dasharray', LINE_STYLE_DASH_PATTERNS[lineStyle]);
    }
  } else if (mark === 'range') {
    // Filled rectangle with opacity matching the actual range area rendering
    group.append('rect')
      .attr('width', size)
      .attr('height', size)
      .attr('rx', SYMBOL_RADIUS)
      .attr('fill', color)
      .attr('opacity', opacity || 0.15);
  } else if (mark === 'scatter' || mark === 'point') {
    group.append('circle')
      .attr('cx', size / 2)
      .attr('cy', size / 2)
      .attr('r', size / 2 - 1)
      .attr('fill', color);
  } else {
    // Default: bar, area, pie - use rectangle
    group.append('rect')
      .attr('width', size)
      .attr('height', size)
      .attr('rx', SYMBOL_RADIUS)
      .attr('fill', color);
  }
}

/**
 * Create and render a unified legend
 *
 * @param {d3.Selection} container - D3 selection of container (SVG or group)
 * @param {Array} items - Array of { label, color, mark?, field? }
 * @param {Object} config - Configuration options
 * @returns {Object} { group, height, onHover, onLeave }
 */
export function createLegend(container, items, config = {}) {
  const {
    x = 0,
    y = 0,
    width = 600,
    align = 'center', // 'left', 'center', 'right'
    maxRows = MAX_ROWS,
    rowHeight = ROW_HEIGHT,
    onItemHover = null,
    onItemLeave = null
  } = config;

  // Skip if only one item (no legend needed)
  if (items.length <= 1) {
    return { group: null, height: 0 };
  }

  // Calculate layout
  const layout = calculateLegendLayout(items, width, { maxRows, rowHeight });

  // Create legend group
  const legendGroup = container.append('g')
    .attr('class', 'chart-legend')
    .attr('transform', `translate(${x}, ${y})`);

  // Create tooltip for truncated labels and overflow
  const tooltip = d3.select('body')
    .append('div')
    .attr('class', 'legend-tooltip')
    .style('position', 'fixed')
    .style('background', 'var(--chartml-surface)')
    .style('color', 'var(--chartml-text-strong)')
    .style('padding', '6px 10px')
    .style('border-radius', '4px')
    .style('font-size', '11px')
    .style('font-family', LEGEND_FONT_FAMILY)
    .style('pointer-events', 'none')
    .style('opacity', 0)
    .style('z-index', 10000)
    .style('box-shadow', 'var(--chartml-shadow)')
    .style('border', '1px solid var(--chartml-border)')
    .style('max-width', '300px')
    .style('white-space', 'pre-wrap');

  // Render each row
  layout.rows.forEach((row, rowIndex) => {
    // Calculate row width for alignment
    const rowWidth = row.reduce((sum, item) => sum + item.itemWidth, 0);
    let startX = 0;

    if (align === 'center') {
      startX = (width - rowWidth) / 2;
    } else if (align === 'right') {
      startX = width - rowWidth;
    }

    let currentX = startX;

    row.forEach((item) => {
      const itemGroup = legendGroup.append('g')
        .attr('class', 'legend-item')
        .attr('transform', `translate(${currentX}, ${rowIndex * rowHeight})`)
        .attr('data-index', item.index)
        .attr('data-field', item.field || '')
        .style('cursor', 'pointer');

      // Render symbol
      renderSymbol(itemGroup, item.mark || 'bar', item.color, SYMBOL_SIZE, item.lineStyle || null, item.opacity || null);

      // Render label
      itemGroup.append('text')
        .attr('x', SYMBOL_SIZE + SYMBOL_TO_TEXT_GAP)
        .attr('y', SYMBOL_SIZE - 1)
        .style('font-size', LEGEND_FONT_SIZE)
        .style('font-family', LEGEND_FONT_FAMILY)
        .style('fill', LEGEND_TEXT_COLOR)
        .text(item.displayLabel);

      // Add interactions
      itemGroup
        .on('mouseenter', function(event) {
          // Show tooltip for truncated labels
          if (item.truncated) {
            tooltip
              .style('opacity', 1)
              .text(item.fullLabel);
          }

          // Highlight this item
          d3.select(this).style('opacity', 1);

          // Dim other legend items
          legendGroup.selectAll('.legend-item')
            .filter(function() { return this !== event.currentTarget; })
            .style('opacity', 0.3);

          // Call external hover handler
          if (onItemHover) {
            onItemHover(item, event);
          }
        })
        .on('mousemove', function(event) {
          if (item.truncated) {
            tooltip
              .style('left', (event.clientX + 10) + 'px')
              .style('top', (event.clientY - 10) + 'px');
          }
        })
        .on('mouseleave', function(event) {
          tooltip.style('opacity', 0);

          // Restore all legend items
          legendGroup.selectAll('.legend-item')
            .style('opacity', 1);

          // Call external leave handler
          if (onItemLeave) {
            onItemLeave(item, event);
          }
        });

      currentX += item.itemWidth;
    });
  });

  // Render overflow indicator if needed
  if (layout.overflow && layout.overflowCount > 0) {
    const lastRowIndex = layout.rows.length - 1;
    const lastRow = layout.rows[lastRowIndex];
    const lastRowWidth = lastRow.reduce((sum, item) => sum + item.itemWidth, 0);

    let overflowX = 0;
    if (align === 'center') {
      overflowX = (width + lastRowWidth) / 2 + 8;
    } else if (align === 'right') {
      overflowX = width + 8;
    } else {
      overflowX = lastRowWidth + 8;
    }

    const overflowText = `+${layout.overflowCount} more`;
    const overflowGroup = legendGroup.append('g')
      .attr('class', 'legend-overflow')
      .attr('transform', `translate(${overflowX}, ${lastRowIndex * rowHeight})`)
      .style('cursor', 'pointer');

    overflowGroup.append('text')
      .attr('y', SYMBOL_SIZE - 1)
      .style('font-size', LEGEND_FONT_SIZE)
      .style('font-family', LEGEND_FONT_FAMILY)
      .style('fill', 'var(--chartml-text-secondary)')
      .style('font-style', 'italic')
      .text(overflowText);

    // Tooltip showing all overflow items
    const overflowTooltipContent = layout.overflowItems
      .map(item => item.label)
      .join('\n');

    overflowGroup
      .on('mouseenter', function(event) {
        tooltip
          .style('opacity', 1)
          .text(overflowTooltipContent);
      })
      .on('mousemove', function(event) {
        tooltip
          .style('left', (event.clientX + 10) + 'px')
          .style('top', (event.clientY - 10) + 'px');
      })
      .on('mouseleave', function() {
        tooltip.style('opacity', 0);
      });
  }

  // Return control object
  return {
    group: legendGroup,
    height: layout.totalHeight,
    layout,
    /**
     * Highlight a specific legend item by index or field
     */
    highlight: (indexOrField) => {
      legendGroup.selectAll('.legend-item').each(function() {
        const el = d3.select(this);
        const idx = parseInt(el.attr('data-index'));
        const field = el.attr('data-field');
        const match = indexOrField === idx || indexOrField === field;
        el.style('opacity', match ? 1 : 0.3);
      });
    },
    /**
     * Reset all legend items to normal opacity
     */
    reset: () => {
      legendGroup.selectAll('.legend-item').style('opacity', 1);
    },
    /**
     * Clean up tooltip on destroy
     */
    destroy: () => {
      tooltip.remove();
    }
  };
}

/**
 * Calculate space needed for legend
 * Useful for margin calculations before rendering
 *
 * @param {Array} items - Array of { label }
 * @param {number} availableWidth - Maximum width
 * @param {Object} options - Layout options
 * @returns {number} Height needed for legend
 */
export function calculateLegendHeight(items, availableWidth, options = {}) {
  if (items.length <= 1) return 0;

  const layout = calculateLegendLayout(items, availableWidth, options);
  return layout.totalHeight + 10; // Add padding
}
