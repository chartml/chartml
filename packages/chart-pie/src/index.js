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
import { globalRegistry, createChartTooltip, positionTooltip, createLegend } from '@chartml/core';

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
    const { categoryField, valueField, height, colors, type, width, animation = true } = config;

    // Helper to get animation duration (0 if animations disabled)
    const getAnimationDuration = (baseMs) => animation ? baseMs : 0;

    // Clear container
    container.innerHTML = '';

    // Colors MUST be present from style resolution
    if (!colors || !Array.isArray(colors)) {
      throw new Error('Pie chart config missing colors array. Ensure style resolution includes palette colors.');
    }

    const pieColors = colors;

    // Legend is always at bottom - reserve space for it
    // Legend needs ~60-80px depending on number of rows
    const LEGEND_HEIGHT = 70;
    const TOP_MARGIN = 20;
    const SIDE_MARGIN = 20;

    // Available space for pie (above legend)
    const availableHeight = height - LEGEND_HEIGHT - TOP_MARGIN;
    const availableWidth = width - (SIDE_MARGIN * 2);

    // Radius is constrained by both dimensions
    const radius = Math.max(40, Math.min(availableWidth / 2, availableHeight / 2));

    // Center pie in available space (above legend area)
    const cx = width / 2;
    const cy = TOP_MARGIN + availableHeight / 2;

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

    // Will be set after legend creation for bidirectional hover
    let legendResult = null;

    // Add slices with hover effects
    const slices = g.selectAll('path')
      .data(arcs)
      .join('path')
      .attr('class', 'pie-slice')
      .attr('data-index', (d, i) => i)
      .attr('d', arc)
      .attr('fill', (d, i) => pieColors[i % pieColors.length])
      .attr('stroke', 'white')
      .attr('stroke-width', 2)
      .style('opacity', 0.9)
      .style('cursor', 'pointer')
      .on('mouseenter', function(event, d) {
        const i = parseInt(d3.select(this).attr('data-index'));
        const category = d.data[categoryField];
        const value = d.data[valueField];
        const percentage = ((value / total) * 100).toFixed(1);

        // Enlarge this slice and dim others
        slices.each(function(_, idx) {
          if (idx === i) {
            d3.select(this)
              .transition()
              .duration(getAnimationDuration(200))
              .attr('d', arcHover)
              .style('opacity', 1);
          } else {
            d3.select(this)
              .transition()
              .duration(getAnimationDuration(200))
              .style('opacity', 0.7);
          }
        });

        // Highlight corresponding legend item
        if (legendResult) {
          legendResult.highlight(i);
        }

        // Show tooltip
        tooltip
          .style('opacity', 1)
          .html(`<strong>${category}</strong><br/>${value.toLocaleString()} (${percentage}%)`);
      })
      .on('mousemove', function(event) {
        positionTooltip(tooltip, event);
      })
      .on('mouseleave', function() {
        // Reset all slices
        slices
          .transition()
          .duration(getAnimationDuration(200))
          .attr('d', arc)
          .style('opacity', 0.9);

        // Reset legend
        if (legendResult) {
          legendResult.reset();
        }

        // Hide tooltip
        tooltip.style('opacity', 0);
      });

    // Create legend using unified utility
    // Position at bottom with smart wrapping and overflow handling
    const legendItems = arcs.map((d, i) => ({
      label: d.data[categoryField],
      color: pieColors[i % pieColors.length],
      mark: 'pie',
      field: categoryField,
      index: i
    }));

    const legendY = height - 60; // Position near bottom
    legendResult = createLegend(svg, legendItems, {
      x: 0,
      y: legendY,
      width: width,
      align: 'center',
      maxRows: 3,
      onItemHover: (item) => {
        // Highlight corresponding slice, dim others
        slices.each(function(_, idx) {
          const el = d3.select(this);
          if (idx === item.index) {
            el.transition()
              .duration(getAnimationDuration(200))
              .attr('d', arcHover)
              .style('opacity', 1);
          } else {
            // Only save original opacity if not already saved (prevents cumulative dimming)
            if (!el.attr('data-original-opacity')) {
              const currentOpacity = parseFloat(el.attr('opacity') || el.style('opacity')) || 0.9;
              el.attr('data-original-opacity', currentOpacity);
            }
            el.transition()
              .duration(getAnimationDuration(200))
              .style('opacity', 0.3);
          }
        });
      },
      onItemLeave: () => {
        // Reset all slices to their original opacity (keep data-original-opacity for fast item-to-item hover)
        slices.each(function() {
          const el = d3.select(this);
          const originalOpacity = parseFloat(el.attr('data-original-opacity')) || 0.9;
          el.transition()
            .duration(getAnimationDuration(200))
            .attr('d', arc)
            .style('opacity', originalOpacity);
        });
      }
    });

    // Append SVG to container
    container.appendChild(svg.node());
  };
}

export default createPieChartRenderer;

// Auto-register on import
const pieRenderer = createPieChartRenderer();
globalRegistry.registerChartRenderer('pie', pieRenderer);
globalRegistry.registerChartRenderer('doughnut', pieRenderer);
