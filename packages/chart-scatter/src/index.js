/**
 * @chartml/chart-scatter
 *
 * Scatter plot and bubble chart renderer plugin for ChartML
 * Renders interactive scatter plots with optional size and color dimensions
 *
 * @example
 * import { createScatterPlotRenderer } from '@chartml/chart-scatter';
 * import { ChartML } from '@chartml/core';
 *
 * const chartml = new ChartML();
 * chartml.registerChartRenderer('scatter', createScatterPlotRenderer());
 */

import * as d3 from 'd3';
import { globalRegistry, createChartTooltip, positionTooltip, determineLabelStrategy, applyLabelStrategy, createLegend } from '@chartml/core';

/**
 * Create a scatter plot renderer
 *
 * @returns {Function} Renderer function compatible with ChartML
 *
 * @example
 * const renderer = createScatterPlotRenderer();
 * chartml.registerChartRenderer('scatter', renderer);
 */
export function createScatterPlotRenderer() {
  /**
   * Render a scatter plot
   *
   * @param {HTMLElement} container - DOM element to render into
   * @param {Array} data - Chart data
   * @param {Object} config - Chart configuration
   * @param {string} config.xField - Field name for x-axis
   * @param {string} config.yField - Field name for y-axis
   * @param {string} [config.sizeField] - Optional field for bubble size
   * @param {string} [config.colorField] - Optional field for color grouping
   * @param {number} config.width - Chart width
   * @param {number} config.height - Chart height
   * @param {string} [config.xAxisLabel] - X-axis label
   * @param {string} [config.yAxisLabel] - Y-axis label
   * @param {Array} config.colors - Color palette array
   * @param {Array} [config.radiusRange] - Min/max radius for bubble sizes
   */
  return function renderScatterPlot(container, data, config) {
    const {
      xField = 'x',
      yField = 'y',
      sizeField = null,
      colorField = null,
      width = 600,
      height = 400,
      marginTop = 20,
      marginRight = 30,
      marginLeft = 70,
      xAxisLabel = '',
      yAxisLabel = '',
      colors,
      defaultRadius = 5,
      radiusRange = [3, 20],
      animation = true  // Enable animations by default
    } = config;

    // Helper to get animation duration (0 if animations disabled)
    const getAnimationDuration = (baseMs) => animation ? baseMs : 0;

    // Validate that colors are provided
    if (!colors || !Array.isArray(colors)) {
      throw new Error('Scatter plot config missing colors array. Ensure style resolution includes palette colors.');
    }

    // Clear previous content
    container.innerHTML = '';

    // Pre-calculate marginBottom based on label strategy (before creating SVG)
    // This ensures X-axis labels and legend don't collide
    const estimatedChartWidth = width - marginLeft - marginRight;

    // For continuous scales, get the tick values that D3 will generate
    const xScale = d3.scaleLinear()
      .domain([Math.min(0, d3.min(data, d => d[xField]) || 0), d3.max(data, d => d[xField])])
      .nice()
      .range([0, estimatedChartWidth]);

    const xTickValues = xScale.ticks(5);
    const xLabels = xTickValues.map(d => String(xScale.tickFormat(5)(d)));

    // Determine optimal label strategy
    const labelStrategy = determineLabelStrategy(xLabels, estimatedChartWidth);

    // Calculate marginBottom based on strategy
    let marginBottom;
    if (config.marginBottom !== undefined) {
      // User explicitly set margin, honor it
      marginBottom = config.marginBottom;
    } else {
      const baseTickLabelSpace = 20; // Base space for tick labels (always present)
      const legendSpace = colorField ? 40 : 0;
      const axisLabelSpace = xAxisLabel ? 25 : 0;
      const labelExtraSpace = labelStrategy.metadata.requiredMargin || 0;
      const legendPadding = colorField ? 10 : 0;

      marginBottom = baseTickLabelSpace + labelExtraSpace + axisLabelSpace + legendPadding + legendSpace;
    }

    // Calculate final chart dimensions
    const chartWidth = width - marginLeft - marginRight;
    const chartHeight = height - marginTop - marginBottom;

    // Create SVG with calculated height
    const svg = d3.select(container)
      .append('svg')
      .attr('width', '100%')
      .attr('height', height)
      .attr('viewBox', [0, 0, width, height])
      .style('max-width', '100%')
      .style('display', 'block');

    // Create chart group with margins
    const g = svg.append('g')
      .attr('transform', `translate(${marginLeft},${marginTop})`);

    // Create scales (reuse the xScale we already created for label strategy)
    const x = xScale.range([0, chartWidth]); // Update range to final chartWidth

    const y = d3.scaleLinear()
      .domain([Math.min(0, d3.min(data, d => d[yField]) || 0), d3.max(data, d => d[yField])])
      .nice()
      .range([chartHeight, 0]);

    // Size scale if sizeField is provided
    const sizeScale = sizeField
      ? d3.scaleSqrt()
          .domain([0, d3.max(data, d => d[sizeField])])
          .range(radiusRange)
      : () => defaultRadius;

    // Color scale if colorField is provided
    const colorScale = colorField
      ? d3.scaleOrdinal()
          .domain([...new Set(data.map(d => d[colorField]))])
          .range(colors)
      : () => colors[0];

    // Create tooltip early for use in label strategy
    const tooltip = createChartTooltip(container);

    // Add X axis with collision detection
    const xAxisGenerator = d3.axisBottom(x).ticks(5);

    const xAxis = g.append('g')
      .attr('transform', `translate(0,${chartHeight})`)
      .style('color', 'var(--chartml-axis-line)')
      .call(xAxisGenerator);

    // Apply label strategy (handles rotation, truncation, etc.)
    applyLabelStrategy(xAxis, labelStrategy.strategy, labelStrategy.metadata, {
      tooltip,
      container
    });

    // Add X axis label - positioned after axis tick labels using calculated margin
    if (xAxisLabel) {
      const labelExtraSpace = labelStrategy.metadata.requiredMargin || 0;
      g.append('text')
        .attr('x', chartWidth / 2)
        .attr('y', chartHeight + labelExtraSpace + 20)
        .attr('text-anchor', 'middle')
        .style('font-size', '14px')
        .style('font-family', 'system-ui')
        .style('fill', 'var(--chartml-text)')
        .text(xAxisLabel);
    }

    // Add Y axis
    const yAxis = g.append('g')
      .style('color', 'var(--chartml-axis-line)')
      .call(d3.axisLeft(y).ticks(5));

    yAxis.selectAll('text')
      .style('font-size', '12px')
      .style('font-family', 'system-ui');

    // Add Y axis label
    if (yAxisLabel) {
      g.append('text')
        .attr('transform', 'rotate(-90)')
        .attr('x', -chartHeight / 2)
        .attr('y', -marginLeft + 15)
        .attr('text-anchor', 'middle')
        .style('font-size', '14px')
        .style('font-family', 'system-ui')
        .style('fill', 'var(--chartml-text)')
        .text(yAxisLabel);
    }

    // Add grid lines
    const gridX = g.append('g')
      .attr('class', 'grid-x')
      .call(d3.axisBottom(x)
        .tickSize(chartHeight)
        .tickFormat('')
      );

    gridX.selectAll('line')
      .style('stroke', 'var(--chartml-grid)')
      .style('stroke-opacity', 0.5);

    gridX.select('.domain').style('opacity', 0);

    const gridY = g.append('g')
      .attr('class', 'grid-y')
      .call(d3.axisLeft(y)
        .tickSize(-chartWidth)
        .tickFormat('')
      );

    gridY.selectAll('line')
      .style('stroke', 'var(--chartml-grid)')
      .style('stroke-opacity', 0.5);

    gridY.select('.domain').style('opacity', 0);

    // Add dots
    g.selectAll('.dot')
      .data(data)
      .join('circle')
      .attr('class', 'dot')
      .attr('data-category', d => colorField ? d[colorField] : '')
      .attr('cx', d => x(d[xField]))
      .attr('cy', d => y(d[yField]))
      .attr('r', d => sizeField ? sizeScale(d[sizeField]) : defaultRadius)
      .attr('fill', d => colorField ? colorScale(d[colorField]) : colors[0])
      .style('stroke', 'var(--chartml-separator)')
      .attr('stroke-width', 1.5)
      .attr('opacity', 0.8)
      .style('cursor', 'pointer')
      .on('mouseenter', function(event, d) {
        // Highlight dot
        d3.select(this)
          .transition()
          .duration(getAnimationDuration(200))
          .attr('r', (sizeField ? sizeScale(d[sizeField]) : defaultRadius) * 1.3)
          .attr('opacity', 1)
          .attr('stroke-width', 2);

        // Build tooltip content
        let tooltipHtml = `<strong>${xField}: ${d[xField].toLocaleString()}</strong>`;
        tooltipHtml += `<br/>${yField}: ${d[yField].toLocaleString()}`;
        if (sizeField) {
          tooltipHtml += `<br/>${sizeField}: ${d[sizeField].toLocaleString()}`;
        }
        if (colorField) {
          tooltipHtml += `<br/>${colorField}: ${d[colorField]}`;
        }

        // Show tooltip
        tooltip
          .style('opacity', 1)
          .html(tooltipHtml);
      })
      .on('mousemove', function(event) {
        positionTooltip(tooltip, event);
      })
      .on('mouseleave', function(event, d) {
        // Remove highlight
        d3.select(this)
          .transition()
          .duration(getAnimationDuration(200))
          .attr('r', sizeField ? sizeScale(d[sizeField]) : defaultRadius)
          .attr('opacity', 0.8)
          .attr('stroke-width', 1.5);

        // Hide tooltip
        tooltip.style('opacity', 0);
      });

    // Add entrance animation
    g.selectAll('.dot')
      .attr('r', 0)
      .transition()
      .delay((d, i) => i * 20)
      .duration(getAnimationDuration(600))
      .attr('r', d => sizeField ? sizeScale(d[sizeField]) : defaultRadius);

    // Add legend if colorField is provided - using unified utility with hover
    if (colorField) {
      const categories = [...new Set(data.map(d => d[colorField]))];

      // Position legend using the pre-calculated margin values
      const baseTickLabelSpace = 20;
      const labelExtraSpace = labelStrategy.metadata.requiredMargin || 0;
      const axisLabelSpace = xAxisLabel ? 25 : 0;
      const legendPadding = 10;
      const legendY = marginTop + chartHeight + baseTickLabelSpace + labelExtraSpace + axisLabelSpace + legendPadding;

      const legendItems = categories.map((category, idx) => ({
        label: String(category),
        color: colorScale(category),
        mark: 'scatter',
        field: colorField,
        category: category,
        index: idx
      }));

      // Helper to get dots by category
      const getDotsByCategory = (category) => {
        return svg.selectAll('.dot').filter(function() {
          return d3.select(this).attr('data-category') === String(category);
        });
      };

      createLegend(svg, legendItems, {
        x: marginLeft,
        y: legendY,
        width: chartWidth,
        align: 'center',
        maxRows: 2,
        onItemHover: (item) => {
          // Highlight dots in this category, dim others
          svg.selectAll('.dot').each(function() {
            const dot = d3.select(this);
            const dotCategory = dot.attr('data-category');
            if (dotCategory === String(item.category)) {
              dot.transition().duration(getAnimationDuration(200)).attr('opacity', 1);
            } else {
              // Only save original opacity if not already saved (prevents cumulative dimming)
              if (!dot.attr('data-original-opacity')) {
                const currentOpacity = parseFloat(dot.attr('opacity') || dot.style('opacity')) || 0.8;
                dot.attr('data-original-opacity', currentOpacity);
              }
              dot.transition().duration(getAnimationDuration(200)).attr('opacity', 0.3);
            }
          });
        },
        onItemLeave: () => {
          // Reset all dots to their original opacity (keep data-original-opacity for fast item-to-item hover)
          svg.selectAll('.dot').each(function() {
            const dot = d3.select(this);
            const originalOpacity = parseFloat(dot.attr('data-original-opacity')) || 0.8;
            dot.transition().duration(getAnimationDuration(200)).attr('opacity', originalOpacity);
          });
        }
      });
    }
  };
}

export default createScatterPlotRenderer;

// Auto-register on import
globalRegistry.registerChartRenderer('scatter', createScatterPlotRenderer());
