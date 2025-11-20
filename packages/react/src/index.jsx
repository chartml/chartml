/**
 * @chartml/react
 *
 * React wrapper component for ChartML
 * Provides a convenient React API for rendering ChartML specifications
 *
 * @example
 * import { ChartMLChart } from '@chartml/react';
 * import { ChartML } from '@chartml/core';
 *
 * function MyComponent() {
 *   const spec = `
 *     data:
 *       - month: Jan
 *         revenue: 45000
 *       - month: Feb
 *         revenue: 52000
 *
 *     visualize:
 *       type: bar
 *       columns: month
 *       rows: revenue
 *       style:
 *         title: "Monthly Revenue"
 *   `;
 *
 *   return <ChartMLChart spec={spec} />;
 * }
 */

import { useEffect, useRef } from 'react';
import { ChartML } from '@chartml/core';

/**
 * ChartMLChart Component
 *
 * @param {Object} props
 * @param {string|object} props.spec - ChartML specification (YAML string or object)
 * @param {ChartML} [props.chartml] - Optional ChartML instance (for custom plugins)
 * @param {Object} [props.options] - Options for ChartML instance (if not providing instance)
 * @param {Function} [props.options.onProgress] - Progress callback
 * @param {Function} [props.options.onCacheHit] - Cache hit callback
 * @param {Function} [props.options.onCacheMiss] - Cache miss callback
 * @param {Function} [props.options.onError] - Error callback
 * @param {Object} [props.options.palettes] - Custom color palettes
 * @param {string} [props.className] - CSS class for container
 * @param {Object} [props.style] - Inline styles for container
 *
 * @example
 * // Basic usage
 * <ChartMLChart spec={yamlSpec} />
 *
 * @example
 * // With custom ChartML instance (for plugins)
 * const chartml = new ChartML();
 * chartml.registerChartRenderer('pie', pieRenderer);
 * <ChartMLChart spec={spec} chartml={chartml} />
 *
 * @example
 * // With hooks
 * <ChartMLChart
 *   spec={spec}
 *   options={{
 *     onProgress: (event) => console.log(`Progress: ${event.percent}%`),
 *     onCacheHit: (event) => console.log('Cache hit!'),
 *     palettes: { myPalette: ['#ff0000', '#00ff00', '#0000ff'] }
 *   }}
 * />
 */
export function ChartMLChart({ spec, chartml, options = {}, className = '', style = {} }) {
  const containerRef = useRef(null);
  const chartmlRef = useRef(null);

  useEffect(() => {
    // Create or use provided ChartML instance
    if (!chartmlRef.current) {
      chartmlRef.current = chartml || new ChartML(options);
    }

    // Render chart
    const renderChart = async () => {
      if (containerRef.current && spec) {
        try {
          await chartmlRef.current.render(spec, containerRef.current);
        } catch (error) {
          console.error('[ChartML React] Render failed:', error);
          if (options.onError) {
            options.onError(error);
          }
        }
      }
    };

    renderChart();

    // Cleanup function
    return () => {
      // Clear container on unmount
      if (containerRef.current) {
        containerRef.current.innerHTML = '';
      }
    };
  }, [spec, chartml, options]);

  return <div ref={containerRef} className={className} style={style} />;
}

/**
 * useChartML Hook
 *
 * Creates and manages a ChartML instance with plugins
 *
 * @param {Object} options - ChartML options
 * @returns {ChartML} ChartML instance
 *
 * @example
 * import { useChartML } from '@chartml/react';
 * import { createPieChartRenderer } from '@chartml/chart-pie';
 *
 * function MyComponent() {
 *   const chartml = useChartML({
 *     onProgress: (e) => console.log(`Progress: ${e.percent}%`)
 *   });
 *
 *   useEffect(() => {
 *     chartml.registerChartRenderer('pie', createPieChartRenderer());
 *   }, [chartml]);
 *
 *   return <ChartMLChart spec={spec} chartml={chartml} />;
 * }
 */
export function useChartML(options = {}) {
  const chartmlRef = useRef(null);

  if (!chartmlRef.current) {
    chartmlRef.current = new ChartML(options);
  }

  return chartmlRef.current;
}

export default ChartMLChart;
