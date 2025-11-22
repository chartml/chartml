/**
 * Unified D3 Cartesian Chart Renderer
 *
 * Handles bar, line, area, and combo charts with dual-axis support.
 * Chart type is just the default mark - individual rows can override.
 */

import * as d3 from 'd3';
import { createFormatter } from './formatters.js';
import { createChartTooltip, positionTooltip } from './tooltipUtils.js';

// Axis label styling constants
const AXIS_LABEL_FONT_SIZE = '12px';
const AXIS_LABEL_FONT_FAMILY = 'system-ui';

/**
 * Setup SVG container with proper dimensions and margins
 */
function setupSvgContainer(container, width, height, marginTop, marginRight, marginBottom, marginLeft) {
  container.innerHTML = '';

  // Ensure container has position: relative for tooltip positioning
  d3.select(container).style('position', 'relative');

  const chartWidth = width - marginLeft - marginRight;
  const chartHeight = height - marginTop - marginBottom;

  const svg = d3.select(container)
    .append('svg')
    .attr('width', '100%')
    .attr('height', height)
    .attr('viewBox', [0, 0, width, height])
    .style('max-width', '100%')
    .style('display', 'block')
    .style('overflow', 'hidden');

  const g = svg.append('g')
    .attr('transform', `translate(${marginLeft},${marginTop})`);

  return { svg, g, chartWidth, chartHeight };
}

/**
 * Sanitize string for use as CSS class name
 * Replaces invalid characters with underscores
 */
function sanitizeClassName(str) {
  if (!str || typeof str !== 'string') return 'unknown';
  // Replace spaces and special chars with underscores, remove leading numbers
  return str.replace(/[^a-zA-Z0-9_-]/g, '_').replace(/^[0-9]/, '_$&');
}

/**
 * Determine if X-axis should use date scale
 */
function determineScaleTypes(data, xField) {
  const isDateScale = data.length > 0 && data[0][xField] instanceof Date;
  return { isDateScale };
}

/**
 * Calculate adaptive bar width for date scales
 * Returns the bar width that fits without overlap
 */
function calculateDateScaleBarWidth(data, xField, chartWidth) {
  if (data.length <= 1) return 20; // Single bar, use default

  // Get unique dates sorted
  const uniqueDates = Array.from(new Set(data.map(d => d[xField].getTime())))
    .map(time => new Date(time))
    .sort((a, b) => a - b);

  const dateCount = uniqueDates.length;

  // Calculate average spacing between bars
  // Available width / number of bars = space per bar
  const spacePerBar = chartWidth / dateCount;

  // Bar should be 80% of available space (20% for gap)
  const calculatedWidth = spacePerBar * 0.8;

  // Cap between 2px (minimum visible) and 20% of chart width (responsive maximum)
  // This prevents bars from being too thin or too fat, while adapting to chart size
  const maxBarWidth = chartWidth * 0.2;
  const barWidth = Math.max(2, Math.min(calculatedWidth, maxBarWidth));

  return barWidth;
}

/**
 * Create X and Y scales
 */
function createScales(data, rows, xField, chartWidth, chartHeight, isDateScale, mode, axes = {}) {
  // Create X scale
  let x;

  if (isDateScale) {
    // For time scales, we use the actual data points to determine inset
    // This prevents bars from overlapping the Y-axis at the edges

    // Count unique dates in the data (actual data points, not D3's generated ticks)
    const uniqueDates = new Set(data.map(d => d[xField].getTime()));
    const dataPointCount = uniqueDates.size;

    // Calculate inset based on actual data points
    // Inset = chartWidth / (2 * numberOfDataPoints)
    let inset = 30; // default fallback
    if (dataPointCount >= 2) {
      inset = chartWidth / (2 * dataPointCount);
    }

    // Create final scale with inset range
    x = d3.scaleUtc()
      .domain(d3.extent(data, d => d[xField]))
      .range([inset, chartWidth - inset]);
  } else {
    // Adaptive padding based on number of bars
    // More bars = less padding to prevent overlap
    const barCount = data.length;
    let padding;
    if (barCount <= 10) {
      padding = 0.2; // 20% padding for few bars (comfortable spacing)
    } else if (barCount <= 20) {
      padding = 0.15; // 15% padding for moderate number
    } else if (barCount <= 40) {
      padding = 0.1; // 10% padding for many bars
    } else {
      padding = 0.05; // 5% padding for very many bars (maximize bar visibility)
    }

    x = d3.scaleBand()
      .domain(data.map(d => d[xField]))
      .range([0, chartWidth])
      .padding(padding);
  }

  // Separate rows by axis
  const leftRows = rows.filter(r => !r.axis || r.axis === 'left');
  const rightRows = rows.filter(r => r.axis === 'right');

  // Calculate Y domain for left axis
  let yLeftMin, yLeftMax;
  const leftFields = leftRows.map(r => r.field);
  const leftMarks = leftRows.map(r => r.mark);

  // Check if left axis has bars or areas in stacked mode
  // Only stack if there are multiple series with the SAME stackable mark type
  const barCount = leftMarks.filter(m => m === 'bar').length;
  const areaCount = leftMarks.filter(m => m === 'area').length;
  const hasStackedBars = barCount > 1 && mode === 'stacked';
  const hasStackedAreas = areaCount > 1 && mode === 'stacked';
  const hasNormalizedAreas = areaCount > 1 && mode === 'normalized';

  if (hasNormalizedAreas) {
    // For normalized (100% stacked) areas, scale is always 0-1
    yLeftMin = 0;
    yLeftMax = 1;
  } else if (hasStackedBars || hasStackedAreas) {
    // For stacked bars/areas, sum all values at each x point
    yLeftMin = 0;
    yLeftMax = d3.max(data, d => d3.sum(leftFields, field => d[field] || 0));
  } else {
    // Otherwise use min/max of all individual values
    const allLeftValues = data.flatMap(d => leftFields.map(field => d[field] || 0));
    yLeftMin = Math.min(0, d3.min(allLeftValues) || 0);
    yLeftMax = d3.max(allLeftValues) || 1;
  }

  // Override with custom min/max if specified
  if (axes.left?.min !== undefined) yLeftMin = axes.left.min;
  if (axes.left?.max !== undefined) yLeftMax = axes.left.max;

  const yLeft = d3.scaleLinear()
    .domain([yLeftMin, yLeftMax])
    .range([chartHeight, 0]);

  // Apply nice() rounding unless explicitly disabled
  if (axes.left?.nice !== false) {
    yLeft.nice();
  }

  // Calculate Y domain for right axis (if needed)
  let yRight = null;
  if (rightRows.length > 0) {
    const rightFields = rightRows.map(r => r.field);
    const allRightValues = data.flatMap(d => rightFields.map(field => d[field] || 0));
    let yRightMin = Math.min(0, d3.min(allRightValues) || 0);
    let yRightMax = d3.max(allRightValues) || 1;

    // Override with custom min/max if specified
    if (axes.right?.min !== undefined) yRightMin = axes.right.min;
    if (axes.right?.max !== undefined) yRightMax = axes.right.max;

    yRight = d3.scaleLinear()
      .domain([yRightMin, yRightMax])
      .range([chartHeight, 0]);

    // Apply nice() rounding unless explicitly disabled
    if (axes.right?.nice !== false) {
      yRight.nice();
    }
  }

  return { x, yLeft, yRight };
}

/**
 * Label Strategy System - Intelligent X-Axis Label Management
 */

/**
 * Measure the width of text labels before rendering
 * Creates a temporary SVG element to measure actual rendered widths
 */
function measureLabelWidths(labels, fontSize = AXIS_LABEL_FONT_SIZE, fontFamily = AXIS_LABEL_FONT_FAMILY) {
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
 * Returns: { strategy: 'horizontal' | 'rotated' | 'truncated' | 'sampled', metadata: {...} }
 */
function determineLabelStrategy(labels, chartWidth, config = {}) {
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
      metadata: { avgWidth, availableSpacePerLabel }
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
        requiredMargin
      }
    };
  }

  // Strategy 3: Try truncation
  // If we can fit truncated labels horizontally
  if (maxLabelWidth + minLabelSpacing <= availableSpacePerLabel && labelCount <= 50) {
    return {
      strategy: 'truncated',
      metadata: { maxLabelWidth, originalMaxWidth: maxWidth }
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
    metadata: { rotationAngle, maxWidth, fallback: true }
  };
}

/**
 * Apply horizontal label strategy (default, no transformation)
 */
function applyHorizontalLabels(xAxis) {
  xAxis.selectAll('text')
    .style('font-size', AXIS_LABEL_FONT_SIZE)
    .style('font-family', AXIS_LABEL_FONT_FAMILY)
    .style('text-anchor', 'middle');

  return 0; // No additional margin needed
}

/**
 * Apply rotated label strategy
 * Returns additional margin needed at bottom
 */
function applyRotatedLabels(xAxis, angle = -45, labelWidths = []) {
  const angleRad = angle * Math.PI / 180;
  const maxWidth = labelWidths.length > 0 ? Math.max(...labelWidths) : 100;

  // Calculate additional margin needed for rotated labels
  // Height = width * |sin(angle)|
  const additionalMargin = maxWidth * Math.abs(Math.sin(angleRad));

  xAxis.selectAll('text')
    .style('font-size', AXIS_LABEL_FONT_SIZE)
    .style('font-family', AXIS_LABEL_FONT_FAMILY)
    .style('text-anchor', 'end')
    .attr('dx', '-0.8em')
    .attr('dy', '0.15em')
    .attr('transform', `rotate(${angle})`);

  return Math.ceil(additionalMargin);
}

/**
 * Apply truncated label strategy with ellipsis
 * Returns additional margin needed (usually 0)
 */
function applyTruncatedLabels(xAxis, maxWidth, tooltip, container) {
  xAxis.selectAll('text')
    .style('font-size', AXIS_LABEL_FONT_SIZE)
    .style('font-family', AXIS_LABEL_FONT_FAMILY)
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
    })
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

  return 0; // No additional margin needed
}

/**
 * Get strategic indices for intelligent sampling
 * Always includes first, last, and evenly distributed middle points
 */
function getStrategicIndices(totalCount, targetCount) {
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
 */
function applySampledLabels(xAxisGenerator, domain, maxLabels) {
  const indices = getStrategicIndices(domain.length, maxLabels);
  const tickValues = indices.map(i => domain[i]);

  return xAxisGenerator.tickValues(tickValues);
}

/**
 * Add axes and labels
 */
function addAxesAndLabels(g, svg, scales, axes, chartWidth, chartHeight, marginLeft, marginRight, marginBottom, isDateScale, mode, container, data, xField, width) {
  const { x, yLeft, yRight } = scales;

  // Create X axis generator
  let xAxisGenerator = d3.axisBottom(x);

  // Determine label strategy for categorical x-axis
  let labelStrategy = null;
  let labelWidths = [];

  if (isDateScale) {
    // For time scales, use hybrid approach:
    // - Few data points: show only actual data points (avoids duplicate labels)
    // - Many data points: use D3's intelligent tick reduction (avoids overlap)

    // Get unique dates from actual data
    const uniqueDates = Array.from(new Set(data.map(d => d[xField].getTime())))
      .map(time => new Date(time))
      .sort((a, b) => a - b);

    const dataPointCount = uniqueDates.length;

    // Estimate how many ticks can fit without overlap (~50px per label minimum)
    const maxFittableTicks = Math.floor(chartWidth / 50);

    if (dataPointCount <= maxFittableTicks) {
      // Few data points - show all actual data points to avoid duplicate labels
      xAxisGenerator = xAxisGenerator.tickValues(uniqueDates);
    } else {
      // Many data points - use D3's smart tick reduction
      const suggestedTicks = Math.max(4, Math.min(maxFittableTicks, 10));
      xAxisGenerator = xAxisGenerator.ticks(suggestedTicks);
    }

    // Apply date formatting if specified, otherwise use UTC default
    if (axes.x?.format) {
      const formatter = createFormatter(axes.x.format, 'date');
      xAxisGenerator = xAxisGenerator.tickFormat(formatter);
    } else {
      // Force UTC formatting to avoid timezone issues
      xAxisGenerator = xAxisGenerator.tickFormat(d3.utcFormat('%b %d'));
    }
  } else {
    // For categorical axes, use intelligent label strategy
    const domain = x.domain();
    const labels = domain.map(d => String(d));

    // Determine best strategy based on label widths and available space
    labelStrategy = determineLabelStrategy(labels, chartWidth, axes.x || {});
    labelWidths = measureLabelWidths(labels);

    // Apply sampling strategy if needed (must be done before rendering)
    if (labelStrategy.strategy === 'sampled') {
      xAxisGenerator = applySampledLabels(
        xAxisGenerator,
        domain,
        labelStrategy.metadata.maxLabelsToShow
      );
    }

    // Apply formatting for categorical axes if specified
    if (axes.x?.format) {
      const formatter = createFormatter(axes.x.format, 'auto');
      xAxisGenerator = xAxisGenerator.tickFormat(formatter);
    }
  }

  // Render X axis
  const xAxis = g.append('g')
    .attr('transform', `translate(0,${chartHeight})`)
    .call(xAxisGenerator);

  // Apply label strategy for categorical axes
  let additionalMargin = 0;
  if (!isDateScale && labelStrategy) {
    // Create tooltip for truncated labels (reuse existing container tooltip)
    const tooltip = d3.select(container).select('.chart-tooltip');

    switch (labelStrategy.strategy) {
      case 'horizontal':
        additionalMargin = applyHorizontalLabels(xAxis);
        break;
      case 'rotated':
        additionalMargin = applyRotatedLabels(
          xAxis,
          labelStrategy.metadata.rotationAngle,
          labelWidths
        );
        break;
      case 'truncated':
        additionalMargin = applyTruncatedLabels(
          xAxis,
          labelStrategy.metadata.maxLabelWidth,
          tooltip,
          container
        );
        break;
      case 'sampled':
        // Sampled strategy already applied to xAxisGenerator
        additionalMargin = applyHorizontalLabels(xAxis);
        break;
    }
  } else if (isDateScale) {
    // For date scales, apply standard styling
    xAxis.selectAll('text')
      .style('font-size', AXIS_LABEL_FONT_SIZE)
      .style('font-family', AXIS_LABEL_FONT_FAMILY);
  }

  // For date scales with inset range, extend the axis line to full chart width
  if (isDateScale) {
    // Hide the default domain line (it only spans from first to last tick)
    xAxis.select('.domain').remove();

    // Draw a custom line that spans the full chart width
    g.append('line')
      .attr('x1', 0)
      .attr('x2', chartWidth)
      .attr('y1', chartHeight)
      .attr('y2', chartHeight)
      .attr('stroke', 'currentColor')
      .attr('stroke-width', 1);
  }

  // Add X axis label
  if (axes.x?.label) {
    g.append('text')
      .attr('x', chartWidth / 2)
      .attr('y', chartHeight + marginBottom - 5)
      .attr('text-anchor', 'middle')
      .style('font-size', '14px')
      .style('font-family', AXIS_LABEL_FONT_FAMILY)
      .style('fill', '#374151')
      .text(axes.x.label);
  }

  // Add left Y axis - format as percentage for normalized mode or use custom format
  let yAxisLeftGenerator = d3.axisLeft(yLeft).ticks(5);

  if (mode === 'normalized') {
    yAxisLeftGenerator = yAxisLeftGenerator.tickFormat(d3.format('.0%'));
  } else if (axes.left?.format) {
    const formatter = createFormatter(axes.left.format, 'number');
    yAxisLeftGenerator = yAxisLeftGenerator.tickFormat(formatter);
  }

  const yAxisLeft = g.append('g').call(yAxisLeftGenerator);

  yAxisLeft.selectAll('text')
    .style('font-size', AXIS_LABEL_FONT_SIZE)
    .style('font-family', AXIS_LABEL_FONT_FAMILY);

  // Measure tick label widths and adjust margins if needed to prevent overlap
  let maxTickWidth = 0;
  yAxisLeft.selectAll('text').each(function() {
    const bbox = this.getBBox();
    maxTickWidth = Math.max(maxTickWidth, bbox.width);
  });

  // Calculate space needed based on whether we have an axis label
  const hasLeftLabel = !!axes.left?.label;
  let requiredSpace;

  if (hasLeftLabel) {
    // With label: tick labels + gap + axis label space
    const gap = 10;
    const axisLabelSpace = 30;
    requiredSpace = maxTickWidth + gap + axisLabelSpace;
  } else {
    // Without label: just tick labels + small buffer
    const buffer = 15;
    requiredSpace = maxTickWidth + buffer;
  }

  const availableSpace = marginLeft;

  // Track the effective margin (may be increased if overlap detected)
  let effectiveMarginLeft = marginLeft;

  // If we need more space, shift the entire chart to the right
  if (requiredSpace > availableSpace) {
    const additionalMargin = requiredSpace - availableSpace;
    effectiveMarginLeft = marginLeft + additionalMargin;
    const currentTransform = g.attr('transform');
    const newTransform = currentTransform.replace(
      `translate(${marginLeft},`,
      `translate(${effectiveMarginLeft},`
    );
    g.attr('transform', newTransform);
  } else if (requiredSpace < availableSpace && !hasLeftLabel) {
    // If we have extra space and no label, reduce margin to maximize chart area
    effectiveMarginLeft = requiredSpace;
    const currentTransform = g.attr('transform');
    const newTransform = currentTransform.replace(
      `translate(${marginLeft},`,
      `translate(${effectiveMarginLeft},`
    );
    g.attr('transform', newTransform);
  }

  // Add left Y axis label using the effective margin
  if (axes.left?.label) {
    g.append('text')
      .attr('transform', 'rotate(-90)')
      .attr('x', -chartHeight / 2)
      .attr('y', -effectiveMarginLeft + 15)
      .attr('text-anchor', 'middle')
      .style('font-size', '14px')
      .style('font-family', AXIS_LABEL_FONT_FAMILY)
      .style('fill', '#374151')
      .text(axes.left.label);
  }

  // Add right Y axis (if needed)
  if (yRight) {
    let yAxisRightGenerator = d3.axisRight(yRight).ticks(5);

    if (axes.right?.format) {
      const formatter = createFormatter(axes.right.format, 'number');
      yAxisRightGenerator = yAxisRightGenerator.tickFormat(formatter);
    }

    const yAxisRight = g.append('g')
      .attr('transform', `translate(${chartWidth}, 0)`)
      .call(yAxisRightGenerator);

    yAxisRight.selectAll('text')
      .style('font-size', AXIS_LABEL_FONT_SIZE)
      .style('font-family', AXIS_LABEL_FONT_FAMILY);

    // Measure tick label widths for right axis label positioning
    let maxRightTickWidth = 0;
    yAxisRight.selectAll('text').each(function() {
      const bbox = this.getBBox();
      maxRightTickWidth = Math.max(maxRightTickWidth, bbox.width);
    });

    // Add right Y axis label with dynamic positioning
    if (axes.right?.label) {
      // Position label far enough to avoid overlap with tick labels
      const gap = 10;
      const labelOffset = chartWidth + maxRightTickWidth + gap + 15;

      g.append('text')
        .attr('transform', 'rotate(-90)')
        .attr('x', -chartHeight / 2)
        .attr('y', labelOffset)
        .attr('text-anchor', 'middle')
        .style('font-size', '14px')
        .style('font-family', AXIS_LABEL_FONT_FAMILY)
        .style('fill', '#374151')
        .text(axes.right.label);
    }
  }
}

/**
 * Add grid lines (horizontal and/or vertical)
 */
function addGridLines(g, scales, chartWidth, chartHeight, gridConfig = {}) {
  const { x, yLeft } = scales;

  // Default grid config
  const config = {
    x: gridConfig.x !== undefined ? gridConfig.x : false,  // Vertical grid lines off by default
    y: gridConfig.y !== undefined ? gridConfig.y : true,   // Horizontal grid lines on by default
    color: gridConfig.color || '#e0e0e0',
    opacity: gridConfig.opacity !== undefined ? gridConfig.opacity : 0.5,
    dashArray: gridConfig.dashArray || null
  };

  // Add horizontal grid lines (from y-axis)
  if (config.y && yLeft) {
    const gridY = g.append('g')
      .attr('class', 'grid grid-y')
      .call(d3.axisLeft(yLeft)
        .tickSize(-chartWidth)
        .tickFormat('')
      );

    gridY.selectAll('line')
      .style('stroke', config.color)
      .style('stroke-opacity', config.opacity);

    if (config.dashArray) {
      gridY.selectAll('line')
        .style('stroke-dasharray', config.dashArray);
    }

    // Hide axis path
    gridY.select('.domain').style('opacity', 0);
  }

  // Add vertical grid lines (from x-axis)
  if (config.x && x) {
    const gridX = g.append('g')
      .attr('class', 'grid grid-x')
      .attr('transform', `translate(0,${chartHeight})`)
      .call(d3.axisBottom(x)
        .tickSize(-chartHeight)
        .tickFormat('')
      );

    gridX.selectAll('line')
      .style('stroke', config.color)
      .style('stroke-opacity', config.opacity);

    if (config.dashArray) {
      gridX.selectAll('line')
        .style('stroke-dasharray', config.dashArray);
    }

    // Hide axis path
    gridX.select('.domain').style('opacity', 0);
  }
}

// Tooltip creation moved to centralized tooltipUtils.js
// Import createChartTooltip from './tooltipUtils.js' for consistent styling

/**
 * Add data labels to marks
 */
function addDataLabels(g, data, row, x, yScale, chartHeight, xField, isDateScale) {
  if (!row.dataLabels || !row.dataLabels.show) {
    return; // Data labels not enabled
  }

  const config = row.dataLabels;
  const position = config.position || 'top';
  const format = config.format || null;
  const fontSize = config.fontSize || 12;
  const color = config.color || '#374151';

  // Create formatter if format is specified
  const formatter = format ? createFormatter(format, 'number') : (v => v.toLocaleString());

  // Add text labels
  const labels = g.selectAll(`.label-${row.field}`)
    .data(data)
    .join('text')
    .attr('class', `label label-${row.field}`)
    .attr('x', d => {
      if (isDateScale) {
        return x(d[xField]);
      } else {
        return x(d[xField]) + x.bandwidth() / 2;
      }
    })
    .attr('y', d => {
      const value = d[row.field] || 0;
      const yPos = yScale(value);

      // Calculate position based on config
      if (row.mark === 'bar') {
        if (position === 'top') {
          return yPos - 5; // Above bar
        } else if (position === 'center') {
          return yPos + (chartHeight - yPos) / 2; // Center of bar
        } else if (position === 'end') {
          return chartHeight - 5; // Bottom of chart
        }
      } else if (row.mark === 'line' || row.mark === 'area') {
        if (position === 'top') {
          return yPos - 5; // Above point
        } else if (position === 'center') {
          return yPos; // On point
        }
      }

      return yPos - 5; // Default: above
    })
    .attr('text-anchor', 'middle')
    .attr('font-size', fontSize)
    .attr('font-family', 'system-ui')
    .attr('fill', color)
    .attr('font-weight', '500')
    .text(d => {
      const value = d[row.field];
      return value != null ? formatter(value) : '';
    });

  // Fade in labels after marks animate
  labels
    .attr('opacity', 0)
    .transition()
    .delay(200)
    .duration(200)
    .attr('opacity', 0.9);
}

/**
 * Render bar mark for a single row
 */
function renderBarMark(g, data, row, x, yScale, chartHeight, color, tooltip, container, xField, isDateScale, chartWidth) {
  // Calculate bar width with a responsive maximum cap to prevent overlap with many bars
  const maxBarWidth = chartWidth * 0.2; // Maximum width: 20% of chart width (responsive)
  let barWidth, xOffset;

  if (isDateScale) {
    // For date scales, calculate adaptive bar width based on available space
    barWidth = calculateDateScaleBarWidth(data, xField, chartWidth);
    xOffset = 0;
  } else {
    const bandwidth = x.bandwidth();
    barWidth = Math.min(bandwidth, maxBarWidth);
    // Calculate x offset to center bar if it's smaller than bandwidth
    xOffset = (bandwidth - barWidth) / 2;
  }

  const bars = g.selectAll(`.bar-${row.field}`)
    .data(data)
    .join('rect')
    .attr('class', `bar bar-${row.field}`)
    .attr('x', d => {
      if (isDateScale) {
        // Center the bar on the date point
        return x(d[xField]) - (barWidth / 2);
      } else {
        return x(d[xField]) + xOffset;
      }
    })
    .attr('width', barWidth)
    .attr('fill', color)
    .attr('opacity', 0.9)
    .style('cursor', 'pointer')
    .on('mouseenter', function(event, d) {
      const barColor = d3.select(this).attr('fill');
      d3.select(this)
        .transition()
        .duration(200)
        .attr('opacity', 1)
        .attr('stroke', d3.color(barColor).darker(0.5))
        .attr('stroke-width', 2);

      const xValue = isDateScale
        ? d3.utcFormat('%b %d, %Y')(d[xField])
        : d[xField];

      tooltip
        .style('opacity', 1)
        .html(`<strong>${xValue}</strong><br/>${row.label || row.field}: ${d[row.field].toLocaleString()}`);
    })
    .on('mousemove', function(event) {
      positionTooltip(tooltip, event);
    })
    .on('mouseleave', function() {
      d3.select(this)
        .transition()
        .duration(200)
        .attr('opacity', 0.9)
        .attr('stroke', 'none');

      tooltip.style('opacity', 0);
    });

  // Animate bars from bottom
  // Disable pointer events during animation to prevent transition conflicts
  bars
    .style('pointer-events', 'none')
    .attr('y', chartHeight)
    .attr('height', 0)
    .transition()
    .duration(400)
    .attr('y', d => yScale(d[row.field] || 0))
    .attr('height', d => chartHeight - yScale(d[row.field] || 0))
    .on('end', function() {
      // Re-enable pointer events after animation completes
      d3.select(this).style('pointer-events', 'auto');
    });

  // Add data labels if configured
  addDataLabels(g, data, row, x, yScale, chartHeight, xField, isDateScale);
}

/**
 * Render stacked bars for multiple bar rows
 */
function renderStackedBars(g, data, barRows, x, yScale, chartHeight, colors, tooltip, container, xField, isDateScale, mode, chartWidth) {
  const fields = barRows.map(r => r.field);

  // Calculate bar width with a responsive maximum cap (20% of chart width)
  const maxBarWidth = chartWidth * 0.2;
  let barWidth, xOffset;

  if (isDateScale) {
    // For date scales, calculate adaptive bar width based on available space
    barWidth = calculateDateScaleBarWidth(data, xField, chartWidth);
    xOffset = 0;
  } else {
    const bandwidth = x.bandwidth();
    barWidth = Math.min(bandwidth, maxBarWidth);
    xOffset = (bandwidth - barWidth) / 2;
  }

  if (mode === 'stacked') {
    // Stacked mode
    const stack = d3.stack()
      .keys(fields)
      .order(d3.stackOrderNone)
      .offset(d3.stackOffsetNone);

    const series = stack(data);

    const groups = g.selectAll('g.stack')
      .data(series)
      .join('g')
      .attr('class', 'stack')
      .attr('fill', (d, i) => barRows[i].color || colors[i]);

    groups.selectAll('rect')
      .data(d => d)
      .join('rect')
      .attr('x', d => {
        if (isDateScale) {
          // Center the bar on the date point
          return x(d.data[xField]) - (barWidth / 2);
        } else {
          return x(d.data[xField]) + xOffset;
        }
      })
      .attr('width', barWidth)
      .attr('opacity', 0.9)
      .style('cursor', 'pointer')
      .on('mouseenter', function(event, d) {
        const key = d3.select(this.parentNode).datum().key;
        const value = d.data[key];
        const barColor = d3.select(this.parentNode).attr('fill');
        const row = barRows.find(r => r.field === key);

        d3.select(this)
          .transition()
          .duration(200)
          .attr('opacity', 1)
          .attr('stroke', d3.color(barColor).darker(0.5))
          .attr('stroke-width', 2);

        tooltip
          .style('opacity', 1)
          .html(`<strong>${d.data[xField]}</strong><br/>${row.label || key}: ${value.toLocaleString()}`);
      })
      .on('mousemove', function(event) {
        positionTooltip(tooltip, event);
      })
      .on('mouseleave', function() {
        d3.select(this)
          .transition()
          .duration(200)
          .attr('opacity', 0.9)
          .attr('stroke', 'none');

        tooltip.style('opacity', 0);
      });

    // Animate stacked bars
    // Disable pointer events during animation to prevent transition conflicts
    groups.selectAll('rect')
      .style('pointer-events', 'none')
      .attr('y', chartHeight)
      .attr('height', 0)
      .transition()
      .delay((d, i) => i * 10)
      .duration(400)
      .attr('y', d => yScale(d[1]))
      .attr('height', d => yScale(d[0]) - yScale(d[1]))
      .on('end', function() {
        // Re-enable pointer events after animation completes
        d3.select(this).style('pointer-events', 'auto');
      });

  } else {
    // Grouped mode
    // Use less padding for grouped bars to maximize bar width
    const x1 = d3.scaleBand()
      .domain(fields)
      .rangeRound([0, barWidth])
      .padding(0.05);

    const groups = g.selectAll('g.group')
      .data(data)
      .join('g')
      .attr('class', 'group')
      .attr('transform', d => {
        if (isDateScale) {
          // Center the grouped bars on the date point
          return `translate(${x(d[xField]) - (barWidth / 2)}, 0)`;
        } else {
          return `translate(${x(d[xField]) + xOffset}, 0)`;
        }
      });

    groups.selectAll('rect')
      .data(d => fields.map((key, i) => ({ key, value: d[key], xValue: d[xField], row: barRows[i] })))
      .join('rect')
      .attr('x', d => x1(d.key))
      .attr('width', x1.bandwidth())
      .attr('fill', d => d.row.color || colors[fields.indexOf(d.key)])
      .attr('opacity', 0.9)
      .style('cursor', 'pointer')
      .on('mouseenter', function(event, d) {
        const barColor = d3.select(this).attr('fill');
        d3.select(this)
          .transition()
          .duration(200)
          .attr('opacity', 1)
          .attr('stroke', d3.color(barColor).darker(0.5))
          .attr('stroke-width', 2);

        tooltip
          .style('opacity', 1)
          .html(`<strong>${d.xValue}</strong><br/>${d.row.label || d.key}: ${d.value.toLocaleString()}`);
      })
      .on('mousemove', function(event) {
        positionTooltip(tooltip, event);
      })
      .on('mouseleave', function() {
        d3.select(this)
          .transition()
          .duration(200)
          .attr('opacity', 0.9)
          .attr('stroke', 'none');

        tooltip.style('opacity', 0);
      });

    // Animate grouped bars
    // Disable pointer events during animation to prevent transition conflicts
    groups.selectAll('rect')
      .style('pointer-events', 'none')
      .attr('y', chartHeight)
      .attr('height', 0)
      .transition()
      .delay((d, i) => i * 10)
      .duration(400)
      .attr('y', d => yScale(d.value))
      .attr('height', d => chartHeight - yScale(d.value))
      .on('end', function() {
        // Re-enable pointer events after animation completes
        d3.select(this).style('pointer-events', 'auto');
      });
  }

  // Add data labels for each row if configured
  // Note: For stacked bars, labels would overlap - may need special handling in future
  barRows.forEach(row => {
    if (row.dataLabels?.show) {
      addDataLabels(g, data, row, x, yScale, chartHeight, xField, isDateScale);
    }
  });
}

/**
 * Render line mark for a single row
 */
function renderLineMark(g, data, row, x, yScale, chartHeight, color, tooltip, container, xField, isDateScale, curveType, showDots) {
  // Filter out null/undefined/NaN values
  const validData = data.filter(d => {
    const value = d[row.field];
    return value != null && !isNaN(value) && isFinite(value);
  });

  if (validData.length === 0) {
    console.warn(`[renderLineMark] No valid data for field '${row.field}'`);
    return;
  }

  const curve = d3[curveType] || d3.curveLinear;
  const line = d3.line()
    .curve(curve)
    .x(d => isDateScale ? x(d[xField]) : (x(d[xField]) + x.bandwidth() / 2))
    .y(d => yScale(d[row.field] || 0));

  // Draw line
  const path = g.append('path')
    .datum(validData)
    .attr('class', `line line-${row.field}`)
    .attr('fill', 'none')
    .attr('stroke', color)
    .attr('stroke-width', 3)
    .attr('d', line);

  // Animate line drawing
  const totalLength = path.node().getTotalLength();
  path
    .attr('stroke-dasharray', totalLength + ' ' + totalLength)
    .attr('stroke-dashoffset', totalLength)
    .transition()
    .duration(500)
    .ease(d3.easeLinear)
    .attr('stroke-dashoffset', 0);

  // Add hover targets for tooltip (always present, even when dots are hidden)
  const sanitizedField = sanitizeClassName(row.field);

  // Create invisible hover circles that are always there to capture mouse events
  const hoverTargets = g.selectAll(`.hover-target-${sanitizedField}`)
    .data(validData)
    .join('circle')
    .attr('class', `hover-target hover-target-${sanitizedField}`)
    .attr('cx', d => isDateScale ? x(d[xField]) : (x(d[xField]) + x.bandwidth() / 2))
    .attr('cy', d => yScale(d[row.field]))
    .attr('r', 8)  // Larger radius for easier hovering
    .attr('fill', 'transparent')  // Invisible by default
    .attr('opacity', 0)
    .style('cursor', 'pointer')
    .on('mouseenter', function(event, d) {
      // Show visual feedback if dots are visible
      if (showDots) {
        const correspondingDot = g.select(`.dot-${sanitizedField}[data-index="${d3.select(this).attr('data-index')}"]`);
        correspondingDot
          .transition()
          .duration(200)
          .attr('r', 7)
          .attr('opacity', 1);
      } else {
        // If no dots, show a temporary highlight dot matching visible dot style
        d3.select(this)
          .transition()
          .duration(200)
          .attr('r', 5)
          .attr('fill', color)
          .attr('stroke', 'white')
          .attr('stroke-width', 2)
          .attr('opacity', 1);
      }

      const xValue = isDateScale
        ? d3.utcFormat('%b %d, %Y')(d[xField])
        : d[xField];

      tooltip
        .style('opacity', 1)
        .html(`<strong>${xValue}</strong><br/>${row.label || row.field}: ${d[row.field].toLocaleString()}`);
    })
    .on('mousemove', function(event) {
      positionTooltip(tooltip, event);
    })
    .on('mouseleave', function() {
      // Hide visual feedback if dots are visible
      if (showDots) {
        const correspondingDot = g.select(`.dot-${sanitizedField}[data-index="${d3.select(this).attr('data-index')}"]`);
        correspondingDot
          .transition()
          .duration(200)
          .attr('r', 5)
          .attr('opacity', 0.9);
      } else {
        // Hide temporary highlight dot (reset to invisible hover target)
        d3.select(this)
          .transition()
          .duration(200)
          .attr('r', 8)
          .attr('fill', 'transparent')
          .attr('stroke', 'none')
          .attr('opacity', 0);
      }

      tooltip.style('opacity', 0);
    });

  // Add index attribute for coordination between hover targets and dots
  hoverTargets.each(function(d, i) {
    d3.select(this).attr('data-index', i);
  });

  // Add visible dots if enabled
  if (showDots) {
    g.selectAll(`.dot-${sanitizedField}`)
      .data(validData)
      .join('circle')
      .attr('class', `dot dot-${sanitizedField}`)
      .attr('cx', d => isDateScale ? x(d[xField]) : (x(d[xField]) + x.bandwidth() / 2))
      .attr('cy', d => yScale(d[row.field]))
      .attr('r', 5)
      .attr('fill', color)
      .attr('stroke', 'white')
      .attr('stroke-width', 2)
      .attr('opacity', 0.9)
      .style('pointer-events', 'none')  // Let hover targets handle mouse events
      .each(function(d, i) {
        d3.select(this).attr('data-index', i);
      });

    // Animate dots entrance - scale delay based on data size
    // For large datasets (>50 points), show all dots at once after line animation
    // For small datasets, stagger the entrance for visual effect
    const dotDelay = validData.length > 50
      ? (() => 500)  // All dots appear together after line finishes
      : ((_, i) => 500 + i * 20);  // Stagger small datasets

    g.selectAll(`.dot-${sanitizedField}`)
      .attr('r', 0)
      .transition()
      .delay(dotDelay)
      .duration(200)
      .attr('r', 5);
  }

  // Add data labels if configured
  addDataLabels(g, validData, row, x, yScale, chartHeight, xField, isDateScale);
}

/**
 * Render area mark for rows
 */
function renderAreaMarks(g, data, areaRows, x, yScale, chartHeight, colors, xField, isDateScale, curveType, fillOpacity, mode) {
  const curve = d3[curveType] || d3.curveLinear;
  const fields = areaRows.map(r => r.field);

  if (mode === 'stacked' && fields.length > 1) {
    // Stacked areas - use stackOffsetNone for absolute values
    const stack = d3.stack()
      .keys(fields)
      .order(d3.stackOrderNone)
      .offset(d3.stackOffsetNone);

    const series = stack(data);

    const area = d3.area()
      .curve(curve)
      .x(d => isDateScale ? x(d.data[xField]) : (x(d.data[xField]) + x.bandwidth() / 2))
      .y0(d => yScale(d[0]))
      .y1(d => yScale(d[1]));

    series.forEach((s, index) => {
      const path = g.append('path')
        .datum(s)
        .attr('fill', areaRows[index].color || colors[index])
        .attr('opacity', fillOpacity)
        .attr('d', area)
        .on('mouseenter', function() {
          d3.select(this)
            .transition()
            .duration(200)
            .attr('opacity', fillOpacity + 0.2);
        })
        .on('mouseleave', function() {
          d3.select(this)
            .transition()
            .duration(200)
            .attr('opacity', fillOpacity);
        });

      // Animate areas
      path
        .attr('opacity', 0)
        .transition()
        .delay(index * 100)
        .duration(400)
        .attr('opacity', fillOpacity);
    });
  } else if (mode === 'normalized' && fields.length > 1) {
    // Normalized stacked areas (100% stacked) - use stackOffsetExpand
    const stack = d3.stack()
      .keys(fields)
      .order(d3.stackOrderNone)
      .offset(d3.stackOffsetExpand);

    const series = stack(data);

    const area = d3.area()
      .curve(curve)
      .x(d => isDateScale ? x(d.data[xField]) : (x(d.data[xField]) + x.bandwidth() / 2))
      .y0(d => yScale(d[0]))
      .y1(d => yScale(d[1]));

    series.forEach((s, index) => {
      const path = g.append('path')
        .datum(s)
        .attr('fill', areaRows[index].color || colors[index])
        .attr('opacity', fillOpacity)
        .attr('d', area)
        .on('mouseenter', function() {
          d3.select(this)
            .transition()
            .duration(200)
            .attr('opacity', fillOpacity + 0.2);
        })
        .on('mouseleave', function() {
          d3.select(this)
            .transition()
            .duration(200)
            .attr('opacity', fillOpacity);
        });

      // Animate areas
      path
        .attr('opacity', 0)
        .transition()
        .delay(index * 100)
        .duration(400)
        .attr('opacity', fillOpacity);
    });
  } else {
    // Overlapping areas
    areaRows.forEach((row, index) => {
      const area = d3.area()
        .curve(curve)
        .x(d => isDateScale ? x(d[xField]) : (x(d[xField]) + x.bandwidth() / 2))
        .y0(chartHeight)
        .y1(d => yScale(d[row.field] || 0));

      const path = g.append('path')
        .datum(data)
        .attr('fill', row.color || colors[index])
        .attr('opacity', fillOpacity)
        .attr('d', area)
        .on('mouseenter', function() {
          d3.select(this)
            .transition()
            .duration(200)
            .attr('opacity', fillOpacity + 0.2);
        })
        .on('mouseleave', function() {
          d3.select(this)
            .transition()
            .duration(200)
            .attr('opacity', fillOpacity);
        });

      // Animate areas
      path
        .attr('opacity', 0)
        .transition()
        .delay(index * 100)
        .duration(400)
        .attr('opacity', fillOpacity);
    });
  }
}

/**
 * Add legend below chart
 */
function addLegend(svg, rows, colors, marginLeft, height, marginBottom, chartWidth, legendSpace, axes) {
  if (rows.length <= 1) return;

  // Position legend in the space reserved for it, accounting for x-axis label if present
  // X-axis label is at y = height - 5, with ~14px font size, so occupies roughly (height - 20) to (height - 5)
  // Legend text is 11px at y=11 within group, so extends from legendY to (legendY + 11)
  const hasXAxisLabel = axes?.x?.label;
  const xAxisLabelHeight = hasXAxisLabel ? 20 : 0; // Space occupied by x-axis label
  const gap = 5; // Small gap between legend and x-axis label (or bottom)

  const legendY = height - legendSpace - xAxisLabelHeight - gap;

  const legend = svg.append('g')
    .attr('transform', `translate(${marginLeft}, ${legendY})`);

  // First pass: measure actual text widths to calculate spacing
  const legendItems = [];
  const tempGroup = legend.append('g').style('opacity', 0); // Hidden group for measurement

  rows.forEach((row, idx) => {
    const labelText = (row.label || row.field);
    const tempText = tempGroup.append('text')
      .style('font-size', '11px')
      .style('font-family', AXIS_LABEL_FONT_FAMILY)
      .text(labelText);

    const textWidth = tempText.node().getComputedTextLength();
    const itemWidth = 18 + textWidth + 20; // symbol (18px) + text + padding (20px)

    legendItems.push({
      row,
      labelText,
      textWidth,
      itemWidth,
      idx
    });
  });

  tempGroup.remove(); // Clean up measurement group

  // Calculate total width needed and center the legend group
  const totalLegendWidth = legendItems.reduce((sum, item) => sum + item.itemWidth, 0);
  const legendStartX = Math.max(0, (chartWidth - totalLegendWidth) / 2);

  // Second pass: render with proper spacing
  let currentX = legendStartX;
  legendItems.forEach((item) => {
    const legendRow = legend.append('g')
      .attr('transform', `translate(${currentX}, 0)`);

    // Use appropriate symbol for mark type
    const mark = item.row.mark || 'bar';
    if (mark === 'bar' || mark === 'area') {
      legendRow.append('rect')
        .attr('width', 12)
        .attr('height', 12)
        .attr('rx', 2)
        .attr('fill', item.row.color || colors[item.idx]);
    } else if (mark === 'line') {
      legendRow.append('line')
        .attr('x1', 0)
        .attr('y1', 6)
        .attr('x2', 12)
        .attr('y2', 6)
        .attr('stroke', item.row.color || colors[item.idx])
        .attr('stroke-width', 3);
    }

    // Add text label (already measured, no truncation needed since we sized to fit)
    legendRow.append('text')
      .attr('x', 18)
      .attr('y', 11)
      .style('font-size', '11px')
      .style('font-family', AXIS_LABEL_FONT_FAMILY)
      .style('fill', '#374151')
      .text(item.labelText);

    // Move to next position
    currentX += item.itemWidth;
  });
}

/**
 * Render reference line annotation
 */
function renderAnnotationLine(g, annotation, scales, chartWidth, chartHeight, marginLeft, marginTop, isDateScale) {
  const { axis, value, label, labelPosition = 'end', color = '#666', strokeWidth = 1, dashArray, opacity = 1.0 } = annotation;

  // Determine which scale to use and coordinates
  let x1, y1, x2, y2;
  let scale;

  if (axis === 'x') {
    scale = scales.x;

    // Convert value to Date if x-axis is a date scale and value is a string
    let scaledValue = value;
    if (isDateScale && typeof value === 'string') {
      scaledValue = new Date(value);
    }

    const xPos = scale(scaledValue);
    if (xPos === undefined) return; // Value not in domain

    x1 = x2 = xPos;
    y1 = 0;
    y2 = chartHeight;
  } else if (axis === 'left' || axis === 'right') {
    scale = axis === 'left' ? scales.yLeft : scales.yRight;
    if (!scale) return; // Scale doesn't exist

    const yPos = scale(value);
    if (yPos === undefined) return; // Value not in domain

    x1 = 0;
    x2 = chartWidth;
    y1 = y2 = yPos;
  } else {
    return; // Invalid axis
  }

  // Create line
  const line = g.append('line')
    .attr('x1', x1)
    .attr('y1', y1)
    .attr('x2', x2)
    .attr('y2', y2)
    .attr('stroke', color)
    .attr('stroke-width', strokeWidth)
    .attr('opacity', opacity)
    .style('pointer-events', 'none');

  if (dashArray) {
    line.attr('stroke-dasharray', dashArray);
  }

  // Add label if specified
  if (label) {
    let textX, textY, textAnchor = 'middle';

    if (axis === 'x') {
      // Vertical line - label at top or bottom
      textX = x1;
      if (labelPosition === 'start') {
        textY = 0 - 5;
        textAnchor = 'middle';
      } else if (labelPosition === 'center') {
        textY = chartHeight / 2;
        textAnchor = 'middle';
      } else { // end
        textY = chartHeight + 15;
        textAnchor = 'middle';
      }
    } else {
      // Horizontal line - label at left, center, or right
      textY = y1 - 5;
      if (labelPosition === 'start') {
        textX = 5;
        textAnchor = 'start';
      } else if (labelPosition === 'center') {
        textX = chartWidth / 2;
        textAnchor = 'middle';
      } else { // end
        textX = chartWidth - 5;
        textAnchor = 'end';
      }
    }

    g.append('text')
      .attr('x', textX)
      .attr('y', textY)
      .attr('text-anchor', textAnchor)
      .attr('font-size', AXIS_LABEL_FONT_SIZE)
      .attr('font-family', 'system-ui, -apple-system, sans-serif')
      .attr('fill', color)
      .style('pointer-events', 'none')
      .text(label);
  }
}

/**
 * Render reference band annotation
 */
function renderAnnotationBand(g, annotation, scales, chartWidth, chartHeight, marginLeft, marginTop, isDateScale) {
  const { axis, from, to, label, color = '#666', opacity = 0.2, strokeColor, strokeWidth = 0 } = annotation;

  // Determine which scale to use and coordinates
  let x, y, width, height;
  let scale;

  if (axis === 'x') {
    scale = scales.x;

    // Convert values to Date if x-axis is a date scale and values are strings
    let scaledFrom = from;
    let scaledTo = to;
    if (isDateScale) {
      if (typeof from === 'string') scaledFrom = new Date(from);
      if (typeof to === 'string') scaledTo = new Date(to);
    }

    const xPosFrom = scale(scaledFrom);
    const xPosTo = scale(scaledTo);
    if (xPosFrom === undefined || xPosTo === undefined) return;

    x = Math.min(xPosFrom, xPosTo);
    width = Math.abs(xPosTo - xPosFrom);
    y = 0;
    height = chartHeight;
  } else if (axis === 'left' || axis === 'right') {
    scale = axis === 'left' ? scales.yLeft : scales.yRight;
    if (!scale) return;

    const yPosFrom = scale(from);
    const yPosTo = scale(to);
    if (yPosFrom === undefined || yPosTo === undefined) return;

    x = 0;
    width = chartWidth;
    y = Math.min(yPosFrom, yPosTo);
    height = Math.abs(yPosTo - yPosFrom);
  } else {
    return;
  }

  // Create band rectangle
  const rect = g.append('rect')
    .attr('x', x)
    .attr('y', y)
    .attr('width', width)
    .attr('height', height)
    .attr('fill', color)
    .attr('opacity', opacity)
    .style('pointer-events', 'none');

  // Add border if specified
  if (strokeColor && strokeWidth > 0) {
    rect.attr('stroke', strokeColor)
      .attr('stroke-width', strokeWidth);
  }

  // Add label if specified
  if (label) {
    let textX, textY;

    if (axis === 'x') {
      textX = x + width / 2;
      textY = 15;
    } else {
      textX = 10;
      textY = y + height / 2;
    }

    g.append('text')
      .attr('x', textX)
      .attr('y', textY)
      .attr('text-anchor', axis === 'x' ? 'middle' : 'start')
      .attr('font-size', AXIS_LABEL_FONT_SIZE)
      .attr('font-family', 'system-ui, -apple-system, sans-serif')
      .attr('fill', color)
      .attr('opacity', Math.min(opacity * 3, 1.0)) // Make label more visible than band
      .style('pointer-events', 'none')
      .text(label);
  }
}

/**
 * Render horizontal bar chart
 *
 * Supports stacked, grouped, and single bar modes with proper legend and tooltips
 */
function renderHorizontalBarChart(container, data, config) {
  const {
    xField = 'x',
    rows = [],
    mode = 'stacked',
    width = 600,
    height = 400,
    marginTop = 20,
    marginRight = 30,
    axes = {},
    colors // REQUIRED - no default
  } = config;

  // Validate that colors are provided
  if (!colors || !Array.isArray(colors)) {
    throw new Error('Horizontal bar chart config missing colors array. Ensure style resolution includes palette colors.');
  }

  // Calculate dynamic marginLeft based on longest y-axis label
  let marginLeft;
  if (config.marginLeft !== undefined) {
    // User explicitly set margin, honor it
    marginLeft = config.marginLeft;
  } else {
    // Measure y-axis labels (category names on left side)
    const labels = data.map(d => String(d[xField]));
    const labelWidths = measureLabelWidths(labels, '12px', 'system-ui');
    const maxLabelWidth = Math.max(...labelWidths);

    // Add padding for tick marks and spacing (10px tick + 5px padding)
    // Cap at 250px to prevent ridiculously long labels from breaking layout
    marginLeft = Math.min(Math.ceil(maxLabelWidth) + 15, 250);
  }

  // Calculate dynamic marginBottom for x-axis label and legend
  const hasLegend = rows.length > 1;
  let marginBottom;
  if (config.marginBottom !== undefined) {
    marginBottom = config.marginBottom;
  } else {
    // Check if x-axis label exists (can be in axes.x.label or axes.left.label for horizontal charts)
    const hasXAxisLabel = axes.x?.label || axes.left?.label;
    // Need more space for axis label (14px font) + tick labels (12px) + spacing + legend (if present)
    // Base: 30px (tick labels) or 50px (tick labels + axis label)
    // Add 30px for legend if present
    const baseMargin = hasXAxisLabel ? 50 : 30;
    marginBottom = hasLegend ? baseMargin + 30 : baseMargin;
  }

  container.innerHTML = '';

  const chartWidth = width - marginLeft - marginRight;
  const chartHeight = height - marginTop - marginBottom;

  const svg = d3.select(container)
    .append('svg')
    .attr('width', '100%')
    .attr('height', height)
    .attr('viewBox', [0, 0, width, height])
    .style('max-width', '100%')
    .style('display', 'block');

  const g = svg.append('g')
    .attr('transform', `translate(${marginLeft},${marginTop})`);

  const normalizedRows = rows.map((row, idx) => ({
    ...row,
    color: row.color || colors[idx % colors.length],
    label: row.label || row.field
  }));

  const leftRows = normalizedRows.filter(r => !r.axis || r.axis === 'left');
  const leftFields = leftRows.map(r => r.field);

  let xMin, xMax;
  if (mode === 'stacked' && leftRows.length > 1) {
    xMin = 0;
    xMax = d3.max(data, d => d3.sum(leftFields, field => d[field] || 0));
  } else {
    const allValues = data.flatMap(d => leftFields.map(field => d[field] || 0));
    xMin = Math.min(0, d3.min(allValues) || 0);
    xMax = d3.max(allValues) || 1;
  }

  const x = d3.scaleLinear()
    .domain([xMin, xMax])
    .nice()
    .range([0, chartWidth]);

  // Adaptive padding based on number of bars (same logic as vertical charts)
  const barCount = data.length;
  let padding;
  if (barCount <= 10) {
    padding = 0.2; // 20% padding for few bars
  } else if (barCount <= 20) {
    padding = 0.15; // 15% padding for moderate number
  } else if (barCount <= 40) {
    padding = 0.1; // 10% padding for many bars
  } else {
    padding = 0.05; // 5% padding for very many bars
  }

  const y = d3.scaleBand()
    .domain(data.map(d => d[xField]))
    .range([0, chartHeight])
    .padding(padding);

  const xAxisFormat = axes.left?.format || axes.x?.format;
  const xFormatter = xAxisFormat ? createFormatter(xAxisFormat, 'auto') : null;

  g.append('g')
    .attr('transform', `translate(0,${chartHeight})`)
    .call(d3.axisBottom(x).ticks(5).tickFormat(xFormatter || (d => d)))
    .selectAll('text')
    .style('font-size', AXIS_LABEL_FONT_SIZE)
    .style('font-family', AXIS_LABEL_FONT_FAMILY);

  const xLabel = axes.left?.label || axes.x?.label;
  if (xLabel) {
    g.append('text')
      .attr('x', chartWidth / 2)
      .attr('y', chartHeight + marginBottom - 5)
      .attr('text-anchor', 'middle')
      .style('font-size', '14px')
      .style('font-family', AXIS_LABEL_FONT_FAMILY)
      .style('fill', '#374151')
      .text(xLabel);
  }

  g.append('g')
    .call(d3.axisLeft(y))
    .selectAll('text')
    .style('font-size', AXIS_LABEL_FONT_SIZE)
    .style('font-family', AXIS_LABEL_FONT_FAMILY);

  g.append('g')
    .attr('class', 'grid')
    .attr('opacity', 0.1)
    .call(d3.axisBottom(x).tickSize(chartHeight).tickFormat(''));

  // Create tooltip using centralized utility (same as vertical charts)
  const tooltip = createChartTooltip(container);

  // Calculate bar height with a maximum cap (horizontal bars)
  const bandwidth = y.bandwidth();
  const maxBarHeight = 40;
  const barHeight = Math.min(bandwidth, maxBarHeight);
  const yOffset = (bandwidth - barHeight) / 2;

  if (leftRows.length > 1 && mode === 'stacked') {
    const series = d3.stack().keys(leftFields).value((d, key) => d[key] || 0)(data);

    series.forEach((serie, i) => {
      const row = leftRows[i];
      g.selectAll(`.bar-${sanitizeClassName(row.field)}`)
        .data(serie)
        .join('rect')
        .attr('y', d => y(d.data[xField]) + yOffset)
        .attr('x', d => x(d[0]))
        .attr('width', d => x(d[1]) - x(d[0]))
        .attr('height', barHeight)
        .attr('fill', row.color)
        .style('opacity', 0.9)
        .style('cursor', 'pointer')
        .on('mouseenter', function(event, d) {
          d3.select(this).style('opacity', 1);
          tooltip.style('opacity', 1)
            .html(`<strong>${row.label}</strong><br/>${d.data[row.field].toLocaleString()}`);
        })
        .on('mousemove', function(event) {
          positionTooltip(tooltip, event);
        })
        .on('mouseleave', function() {
          d3.select(this).style('opacity', 0.9);
          tooltip.style('opacity', 0);
        });
    });
  } else if (leftRows.length > 1 && mode === 'grouped') {
    const ySubgroup = d3.scaleBand()
      .domain(leftFields)
      .range([0, barHeight])
      .padding(0.05);

    leftRows.forEach(row => {
      g.selectAll(`.bar-${sanitizeClassName(row.field)}`)
        .data(data)
        .join('rect')
        .attr('y', d => y(d[xField]) + yOffset + ySubgroup(row.field))
        .attr('x', 0)
        .attr('width', d => x(d[row.field] || 0))
        .attr('height', ySubgroup.bandwidth())
        .attr('fill', row.color)
        .style('opacity', 0.9)
        .style('cursor', 'pointer')
        .on('mouseenter', function(event, d) {
          d3.select(this).style('opacity', 1);
          tooltip.style('opacity', 1)
            .html(`<strong>${row.label}</strong><br/>${d[row.field].toLocaleString()}`);
        })
        .on('mousemove', function(event) {
          positionTooltip(tooltip, event);
        })
        .on('mouseleave', function() {
          d3.select(this).style('opacity', 0.9);
          tooltip.style('opacity', 0);
        });
    });
  } else if (leftRows.length === 1) {
    const row = leftRows[0];
    g.selectAll('.bar')
      .data(data)
      .join('rect')
      .attr('y', d => y(d[xField]) + yOffset)
      .attr('x', 0)
      .attr('width', d => x(d[row.field] || 0))
      .attr('height', barHeight)
      .attr('fill', row.color)
      .style('opacity', 0.9)
      .style('cursor', 'pointer')
      .on('mouseenter', function(event, d) {
        d3.select(this).style('opacity', 1);
        tooltip.style('opacity', 1)
          .html(`<strong>${d[xField]}</strong><br/>${d[row.field].toLocaleString()}`);
      })
      .on('mousemove', function(event) {
        positionTooltip(tooltip, event);
      })
      .on('mouseleave', function() {
        d3.select(this).style('opacity', 0.9);
        tooltip.style('opacity', 0);
      });
  }

  if (normalizedRows.length > 1) {
    const legendX = marginLeft;
    // Position legend in the bottom margin area, centered vertically
    const legendY = height - marginBottom / 2;
    const legendItemWidth = Math.min(150, chartWidth / normalizedRows.length);

    const legend = svg.append('g')
      .attr('transform', `translate(${legendX}, ${legendY})`);

    normalizedRows.forEach((row, i) => {
      const itemX = i * legendItemWidth;
      legend.append('rect')
        .attr('x', itemX)
        .attr('y', 0)
        .attr('width', 12)
        .attr('height', 12)
        .attr('fill', row.color);
      legend.append('text')
        .attr('x', itemX + 18)
        .attr('y', 11)
        .style('font-size', AXIS_LABEL_FONT_SIZE)
        .style('font-family', AXIS_LABEL_FONT_FAMILY)
        .style('fill', '#374151')
        .text(row.label);
    });
  }
}

/**
 * Main render function for cartesian charts
 *
 * @param {HTMLElement} container - DOM element to render into
 * @param {Array} data - Array of data objects
 * @param {Object} config - Chart configuration
 * @param {string} config.xField - Field for X-axis
 * @param {Array} config.rows - Array of { field, mark, axis, color, label }
 * @param {string} config.type - Default mark type (bar, line, area)
 * @param {string} config.mode - Stacking mode ('stacked' or 'grouped')
 * @param {Object} config.axes - Axis labels { x, left, right }
 * @param {number} config.width - Chart width
 * @param {number} config.height - Chart height
 * @param {Array} config.colors - Color palette
 * @param {string} config.curveType - Line curve type
 * @param {boolean} config.showDots - Show dots on lines
 * @param {number} config.fillOpacity - Fill opacity for areas
 */
export function renderD3CartesianChart(container, data, config) {
  const orientation = config.orientation || 'vertical';
  const type = config.type || 'bar';

  // For horizontal bar charts, delegate to specialized horizontal renderer
  if (orientation === 'horizontal' && type === 'bar') {
    return renderHorizontalBarChart(container, data, config);
  }

  // Continue with vertical/standard rendering
  const {
    xField = 'x',
    rows = [],
    mode = 'stacked', // 'stacked' or 'grouped' for multiple bars/areas
    width = 600,
    height = 400,
    marginTop = 20,
    marginRight = 30,
    marginLeft: configMarginLeft,
    axes = {},
    colors, // REQUIRED - no default
    curveType = 'curveMonotoneX',
    showDots = true,
    fillOpacity = 0.6,
    annotations = []
  } = config;

  // Validate that colors are provided
  if (!colors || !Array.isArray(colors)) {
    throw new Error('Cartesian chart config missing colors array. Ensure style resolution includes palette colors.');
  }

  // Use user-provided marginLeft or a sensible default
  // (Final calculation happens later based on actual tick labels)
  const marginLeft = configMarginLeft !== undefined ? configMarginLeft : 70;

  // Pre-calculate label strategy to determine required margin
  // We need this BEFORE setting up SVG to ensure labels aren't cut off
  // NOTE: Only for categorical axes - date scales handle their own formatting
  const categoryCount = data.length;
  const hasLegend = rows.length > 1;
  const legendSpace = hasLegend ? 30 : 0;

  // Check if this is a date scale (do this inline to avoid duplicate declaration)
  const hasDateValues = data.length > 0 && data[0][xField] instanceof Date;

  let marginBottom;
  if (config.marginBottom !== undefined) {
    // User explicitly set margin, honor it
    marginBottom = config.marginBottom;
  } else if (hasDateValues) {
    // Date scales use D3's built-in formatting and tick reduction
    // No rotation needed, so use standard margin
    // Add extra space if there's an x-axis title
    const hasXAxisLabel = axes.x?.label;
    const axisLabelSpace = hasXAxisLabel ? 20 : 0;
    marginBottom = 30 + legendSpace + axisLabelSpace;
  } else {
    // Categorical axes - estimate chart width to measure labels
    const estimatedChartWidth = width - marginLeft - marginRight;
    const domain = data.map(d => d[xField]);
    const labels = domain.map(d => String(d));

    // Determine strategy to get required margin
    const prelimStrategy = determineLabelStrategy(labels, estimatedChartWidth, axes.x || {});

    if (prelimStrategy.strategy === 'rotated' && prelimStrategy.metadata.requiredMargin) {
      // Use calculated margin based on actual label widths
      // Add extra space for x-axis label if present (to avoid collision with rotated category labels)
      const hasXAxisLabel = axes.x?.label;
      const axisLabelSpace = hasXAxisLabel ? 25 : 0;
      marginBottom = prelimStrategy.metadata.requiredMargin + legendSpace + axisLabelSpace;
    } else if (prelimStrategy.strategy === 'horizontal') {
      // Add extra space for x-axis label if present
      const hasXAxisLabel = axes.x?.label;
      const axisLabelSpace = hasXAxisLabel ? 20 : 0;
      marginBottom = 30 + legendSpace + axisLabelSpace;
    } else {
      // Fallback for other strategies
      const hasXAxisLabel = axes.x?.label;
      const axisLabelSpace = hasXAxisLabel ? 20 : 0;
      marginBottom = 50 + legendSpace + axisLabelSpace;
    }
  }

  // Normalize rows to include mark type
  const normalizedRows = rows.map((row, idx) => ({
    ...row,
    mark: row.mark || type, // Default to chart type
    axis: row.axis || 'left', // Default to left axis
    color: row.color || colors[idx % colors.length],
    label: row.label || row.field
  }));

  // Separate rows by axis for margin calculation and rendering
  const leftRows = normalizedRows.filter(r => r.axis === 'left');
  const rightRows = normalizedRows.filter(r => r.axis === 'right');

  // Pre-calculate left Y-axis margin based on numeric tick labels
  let finalMarginLeft = marginLeft;

  if (leftRows.length > 0) {
    const tempChartHeight = height - marginTop - marginBottom;

    // Calculate extent for left axis
    const leftFields = leftRows.map(r => r.field);
    const allLeftValues = data.flatMap(d => leftFields.map(field => d[field] || 0));
    let yLeftMin = Math.min(0, d3.min(allLeftValues) || 0);
    let yLeftMax = d3.max(allLeftValues) || 1;

    if (axes.left?.min !== undefined) yLeftMin = axes.left.min;
    if (axes.left?.max !== undefined) yLeftMax = axes.left.max;

    const tempYLeft = d3.scaleLinear()
      .domain([yLeftMin, yLeftMax])
      .range([tempChartHeight, 0]);

    if (axes.left?.nice !== false) {
      tempYLeft.nice();
    }

    const ticks = tempYLeft.ticks(5);
    const formatter = axes.left?.format
      ? createFormatter(axes.left.format)
      : d3.format(',');

    const tickLabels = ticks.map(t => formatter(t));
    const labelWidths = measureLabelWidths(tickLabels, AXIS_LABEL_FONT_SIZE, AXIS_LABEL_FONT_FAMILY);
    const maxLabelWidth = Math.max(...labelWidths);

    // Calculate required space
    const hasLeftLabel = !!axes.left?.label;
    if (hasLeftLabel) {
      const gap = 10;
      const axisLabelSpace = 30;
      finalMarginLeft = Math.min(maxLabelWidth + gap + axisLabelSpace, 250);
    } else {
      // D3 tick size (6px) + gap between tick and label (3px) + safety padding (6px) = 15px
      // But labels are right-aligned to the axis, so we need the full label width + tick/gap
      const buffer = 20; // Increased from 15 to give more breathing room
      finalMarginLeft = Math.min(maxLabelWidth + buffer, 250);
    }

  }

  // Detect if right axis is needed and calculate margin dynamically
  const hasRightAxis = normalizedRows.some(r => r.axis === 'right');
  let finalMarginRight = marginRight;

  if (hasRightAxis) {
    // Calculate right margin based on tick label widths (same approach as left axis)
    // rightRows already declared above

    // Create temporary scale to get tick values
    const tempChartHeight = height - marginTop - marginBottom;

    // Calculate extent for right axis (same logic as createScales)
    const rightFields = rightRows.map(r => r.field);
    const allRightValues = data.flatMap(d => rightFields.map(field => d[field] || 0));
    let yRightMin = 0;
    let yRightMax = d3.max(allRightValues) || 1;

    // Override with custom min/max if specified
    if (axes.right?.min !== undefined) yRightMin = axes.right.min;
    if (axes.right?.max !== undefined) yRightMax = axes.right.max;

    const tempYRight = d3.scaleLinear()
      .domain([yRightMin, yRightMax])
      .range([tempChartHeight, 0]);

    // Apply nice() rounding unless explicitly disabled
    if (axes.right?.nice !== false) {
      tempYRight.nice();
    }

    // Get tick values and format them (use same tick count as actual rendering)
    const ticks = tempYRight.ticks(5);
    const formatter = axes.right?.format
      ? createFormatter(axes.right.format)
      : d3.format(',');

    // Same approach as left axis: measure label widths
    const tickLabels = ticks.map(t => formatter(t));
    const labelWidths = measureLabelWidths(tickLabels, AXIS_LABEL_FONT_SIZE, AXIS_LABEL_FONT_FAMILY);
    const maxLabelWidth = Math.max(...labelWidths);


    // Right axis needs: measured width + D3's 9px offset + 15px safety buffer
    const hasRightLabel = axes.right?.label;
    const axisLabelSpace = hasRightLabel ? 30 : 0;
    finalMarginRight = Math.min(Math.ceil(maxLabelWidth) + 9 + 15 + axisLabelSpace, 250);

  }

  // Setup SVG
  const { svg, g, chartWidth, chartHeight } = setupSvgContainer(
    container, width, height, marginTop, finalMarginRight, marginBottom, finalMarginLeft
  );

  // Determine scale types
  const { isDateScale } = determineScaleTypes(data, xField);

  // Create scales
  const scales = createScales(data, normalizedRows, xField, chartWidth, chartHeight, isDateScale, mode, axes);

  // Create tooltip (before axes so it can be used for label tooltips)
  const tooltip = createChartTooltip(container);

  // Add axes and labels
  addAxesAndLabels(g, svg, scales, axes, chartWidth, chartHeight, finalMarginLeft, finalMarginRight, marginBottom, isDateScale, mode, container, data, xField, width);

  // Add grid lines
  addGridLines(g, scales, chartWidth, chartHeight, config.style?.grid);

  // Group rows by mark and axis for rendering
  // leftRows and rightRows already declared above for margin calculation

  // Render left axis marks
  const leftBars = leftRows.filter(r => r.mark === 'bar');
  const leftLines = leftRows.filter(r => r.mark === 'line');
  const leftAreas = leftRows.filter(r => r.mark === 'area');

  // Render bars on left axis
  if (leftBars.length > 1 && mode) {
    // Multiple bars - use stacked or grouped rendering
    renderStackedBars(g, data, leftBars, scales.x, scales.yLeft, chartHeight, colors, tooltip, container, xField, isDateScale, mode, chartWidth);
  } else if (leftBars.length === 1) {
    // Single bar
    renderBarMark(g, data, leftBars[0], scales.x, scales.yLeft, chartHeight, leftBars[0].color, tooltip, container, xField, isDateScale, chartWidth);
  }

  // Render areas on left axis
  if (leftAreas.length > 0) {
    renderAreaMarks(g, data, leftAreas, scales.x, scales.yLeft, chartHeight, colors, xField, isDateScale, curveType, fillOpacity, mode);
  }

  // Render lines on left axis
  leftLines.forEach(row => {
    renderLineMark(g, data, row, scales.x, scales.yLeft, chartHeight, row.color, tooltip, container, xField, isDateScale, curveType, showDots);
  });

  // Render right axis marks
  if (scales.yRight) {
    const rightBars = rightRows.filter(r => r.mark === 'bar');
    const rightLines = rightRows.filter(r => r.mark === 'line');
    const rightAreas = rightRows.filter(r => r.mark === 'area');

    // Render bars on right axis
    rightBars.forEach(row => {
      renderBarMark(g, data, row, scales.x, scales.yRight, chartHeight, row.color, tooltip, container, xField, isDateScale, chartWidth);
    });

    // Render areas on right axis
    if (rightAreas.length > 0) {
      renderAreaMarks(g, data, rightAreas, scales.x, scales.yRight, chartHeight, colors, xField, isDateScale, curveType, fillOpacity, mode);
    }

    // Render lines on right axis
    rightLines.forEach(row => {
      renderLineMark(g, data, row, scales.x, scales.yRight, chartHeight, row.color, tooltip, container, xField, isDateScale, curveType, showDots);
    });
  }

  // Render annotations (reference lines and bands)
  if (annotations && annotations.length > 0) {
    // Create a group for annotations that renders behind data marks
    const annotationGroup = g.append('g').attr('class', 'annotations');

    annotations.forEach(annotation => {
      if (annotation.type === 'line') {
        renderAnnotationLine(annotationGroup, annotation, scales, chartWidth, chartHeight, marginLeft, marginTop, isDateScale);
      } else if (annotation.type === 'band') {
        renderAnnotationBand(annotationGroup, annotation, scales, chartWidth, chartHeight, marginLeft, marginTop, isDateScale);
      }
    });

    // Move annotations to back (behind data marks)
    annotationGroup.lower();
  }

  // Add legend (pass legendSpace and axes so it can position correctly relative to rotated labels and axis title)
  addLegend(svg, normalizedRows, colors, marginLeft, height, marginBottom, chartWidth, legendSpace, axes);
}
