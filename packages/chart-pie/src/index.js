/**
 * @chartml/chart-pie
 *
 * Pie and doughnut chart renderer plugin for ChartML
 * Renders interactive pie and doughnut charts with tooltips and legends
 *
 * @example
 * import { createPieChartRenderer } from '@chartml/chart-pie';
 * import { ChartML } from '@chartml/core';
 *
 * const chartml = new ChartML();
 * chartml.registerChartRenderer('pie', createPieChartRenderer());
 * chartml.registerChartRenderer('doughnut', createPieChartRenderer());
 */

import * as d3 from 'd3';
import { globalRegistry, createChartTooltip, positionTooltip } from '@chartml/core';

/**
 * Create a pie/doughnut chart renderer
 *
 * @returns {Function} Renderer function compatible with ChartML
 *
 * @example
 * const renderer = createPieChartRenderer();
 * chartml.registerChartRenderer('pie', renderer);
 */
export function createPieChartRenderer() {
  /**
   * Render a pie or doughnut chart
   *
   * @param {HTMLElement} container - DOM element to render into
   * @param {Array} data - Chart data
   * @param {Object} config - Chart configuration
   * @param {string} config.categoryField - Field name for categories
   * @param {string} config.valueField - Field name for values
   * @param {string} config.type - Chart type ('pie' or 'doughnut')
   * @param {number} config.width - Chart width
   * @param {number} config.height - Chart height
   * @param {Array} config.colors - Color palette array
   */
  return function renderPieChart(container, data, config) {
    const { categoryField, valueField, height, colors, type, width } = config;

    // Clear container
    container.innerHTML = '';

    // Colors MUST be present from style resolution
    if (!colors || !Array.isArray(colors)) {
      throw new Error('Pie chart config missing colors array. Ensure style resolution includes palette colors.');
    }

    const pieColors = colors;

    // Determine legend position based on available space
    // Legend on right needs ~150px width. Pie chart needs reasonable diameter.
    const LEGEND_WIDTH = 150;
    const MIN_PIE_DIAMETER = 180; // Minimum readable pie chart diameter

    // Calculate max radius if legend is on right
    // Pie is centered at width/2, so radius can extend at most to (width - LEGEND_WIDTH - 20px padding)
    const maxRightEdge = width - LEGEND_WIDTH - 20; // Right edge where legend starts (with padding)
    const maxRadiusForRightLegend = maxRightEdge - (width / 2); // Distance from center to legend
    const maxPieDiameterForRightLegend = maxRadiusForRightLegend * 2;

    // Also constrain by height
    const maxPieDiameterByHeight = height - 80; // 80px for top/bottom margins

    // Final diameter if using right legend
    const rightLegendPieDiameter = Math.min(maxPieDiameterForRightLegend, maxPieDiameterByHeight);

    // Use right legend only if:
    // 1. We have enough width for legend (width >= 400), AND
    // 2. The resulting pie diameter is reasonable (>= MIN_PIE_DIAMETER)
    const legendOnRight = width >= 400 && rightLegendPieDiameter >= MIN_PIE_DIAMETER;

    // Calculate dimensions - adjust based on legend position
    let radius, cx, cy;
    if (legendOnRight) {
      // Legend on right - pie centered, radius constrained to not overlap legend
      radius = rightLegendPieDiameter / 2;
      cx = width / 2; // Always center horizontally
      cy = height / 2;
    } else {
      // Legend at bottom - pie uses full width but less height
      const bottomMargin = 80; // Space for bottom legend
      radius = Math.min(width / 2 - 40, (height - bottomMargin) / 2 - 40);
      cx = width / 2; // Always center horizontally
      cy = (height - bottomMargin) / 2 + 20; // Center in available space above legend
    }

    const innerRadius = type === 'doughnut' ? radius * 0.6 : 0;

    // Use d3.pie to calculate angles
    const pie = d3.pie()
      .value(d => d[valueField])
      .sort(null); // Maintain data order

    // Use d3.arc to generate path strings
    const arc = d3.arc()
      .innerRadius(innerRadius)
      .outerRadius(radius);

    // Arc for hover effect (slightly larger)
    const arcHover = d3.arc()
      .innerRadius(innerRadius)
      .outerRadius(radius + 5);

    // Generate pie slices
    const arcs = pie(data);
    const total = d3.sum(data, d => d[valueField]);

    // Create SVG using d3 for proper event handling
    const svg = d3.create('svg')
      .attr('width', '100%')
      .attr('height', height)
      .attr('viewBox', [0, 0, width, height])
      .attr('preserveAspectRatio', 'xMidYMid meet')
      .style('font-family', 'system-ui')
      .style('max-width', '100%');

    // Create tooltip using centralized utility for consistent styling
    const tooltip = createChartTooltip(container);

    // Create pie slices group
    const g = svg.append('g')
      .attr('transform', `translate(${cx}, ${cy})`);

    // Add slices with hover effects
    g.selectAll('path')
      .data(arcs)
      .join('path')
      .attr('d', arc)
      .attr('fill', (d, i) => pieColors[i % pieColors.length])
      .attr('stroke', 'white')
      .attr('stroke-width', 2)
      .style('opacity', 0.9)
      .style('cursor', 'pointer')
      .on('mouseenter', function(event, d) {
        const category = d.data[categoryField];
        const value = d.data[valueField];
        const percentage = ((value / total) * 100).toFixed(1);

        // Enlarge slice
        d3.select(this)
          .transition()
          .duration(200)
          .attr('d', arcHover)
          .style('opacity', 1);

        // Show tooltip
        tooltip
          .style('opacity', 1)
          .html(`<strong>${category}</strong><br/>${value.toLocaleString()} (${percentage}%)`);
      })
      .on('mousemove', function(event) {
        positionTooltip(tooltip, event);
      })
      .on('mouseleave', function() {
        // Reset slice
        d3.select(this)
          .transition()
          .duration(200)
          .attr('d', arc)
          .style('opacity', 0.9);

        // Hide tooltip
        tooltip.style('opacity', 0);
      });

    // Create legend with responsive positioning
    // Use right-side legend for wide charts (>= 400px), bottom legend for narrow charts
    // (legendOnRight already calculated above in dimensions section)
    let legend;
    if (legendOnRight) {
      // Legend on right side
      legend = svg.append('g')
        .attr('transform', `translate(${width - 150}, 20)`);

      legend.selectAll('g')
        .data(arcs)
        .join('g')
        .attr('transform', (d, i) => `translate(0, ${i * 25})`)
        .each(function(d, i) {
          const g = d3.select(this);
          const category = d.data[categoryField];
          const percentage = ((d.data[valueField] / total) * 100).toFixed(1);
          const color = pieColors[i % pieColors.length];

          g.append('rect')
            .attr('width', 18)
            .attr('height', 18)
            .attr('rx', 3)
            .attr('fill', color);

          g.append('text')
            .attr('x', 25)
            .attr('y', 13)
            .attr('font-size', 12)
            .attr('fill', '#374151')
            .text(category);

          g.append('text')
            .attr('x', 25)
            .attr('y', 25)
            .attr('font-size', 10)
            .attr('fill', '#6b7280')
            .text(`${percentage}%`);
        });
    } else {
      // Legend at bottom (horizontal layout)
      // Calculate horizontal spacing first to determine how many rows we need
      const itemWidth = 120; // Approximate width per legend item
      const itemsPerRow = Math.max(1, Math.floor((width - 40) / itemWidth));
      const numRows = Math.ceil(arcs.length / itemsPerRow);

      // Calculate actual legend width (for centering)
      const actualItemsInLastRow = arcs.length % itemsPerRow || itemsPerRow;
      const legendWidth = Math.min(arcs.length, itemsPerRow) * itemWidth;

      // Each legend item needs about 30px of vertical space (18px rect + 25px for bottom text)
      const rowHeight = 30;
      const legendHeight = numRows * rowHeight;

      // Center legend horizontally and position at bottom with padding
      const legendX = (width - legendWidth) / 2;
      const legendY = height - legendHeight - 10; // 10px bottom padding

      legend = svg.append('g')
        .attr('transform', `translate(${legendX}, ${legendY})`);

      legend.selectAll('g')
        .data(arcs)
        .join('g')
        .attr('transform', (d, i) => {
          const col = i % itemsPerRow;
          const row = Math.floor(i / itemsPerRow);
          return `translate(${col * itemWidth}, ${row * rowHeight})`;
        })
        .each(function(d, i) {
          const g = d3.select(this);
          const category = d.data[categoryField];
          const percentage = ((d.data[valueField] / total) * 100).toFixed(1);
          const color = pieColors[i % pieColors.length];

          g.append('rect')
            .attr('width', 18)
            .attr('height', 18)
            .attr('rx', 3)
            .attr('fill', color);

          g.append('text')
            .attr('x', 25)
            .attr('y', 13)
            .attr('font-size', 12)
            .attr('fill', '#374151')
            .text(category);

          g.append('text')
            .attr('x', 25)
            .attr('y', 25)
            .attr('font-size', 10)
            .attr('fill', '#6b7280')
            .text(`${percentage}%`);
        });
    }

    // Append SVG to container
    container.appendChild(svg.node());
  };
}

export default createPieChartRenderer;

// Auto-register on import
const pieRenderer = createPieChartRenderer();
globalRegistry.registerChartRenderer('pie', pieRenderer);
globalRegistry.registerChartRenderer('doughnut', pieRenderer);
