/**
 * Client-side script to render ChartML blocks
 * This should be imported in your app to activate chart rendering
 */

import { ChartML } from '@chartml/core';
import { createErrorElement, getColSpanClass } from '@chartml/markdown-common';
import yaml from 'js-yaml';

/**
 * Find and render all ChartML containers on the page
 * All blocks share a single ChartML instance for parameter coordination
 *
 * Handles both single components and arrays (multi-component blocks)
 * Matches the behavior of the React markdown plugin
 */
export async function renderAllCharts() {
  const containers = document.querySelectorAll('.chartml-container[data-chartml-spec]');

  // Create a single shared ChartML instance for all blocks
  // This ensures params blocks and chart blocks share the same registry
  const chartml = new ChartML();

  for (const container of containers) {
    try {
      const spec = container.getAttribute('data-chartml-spec');
      if (!spec) continue;

      // Parse YAML - could be single component or array
      const parsed = yaml.load(spec);
      const components = Array.isArray(parsed) ? parsed : [parsed];

      // Separate by component type (matches React plugin behavior)
      const sourceComponents = components.filter(c => c?.type?.toLowerCase() === 'source');
      const styleComponents = components.filter(c => c?.type?.toLowerCase() === 'style');
      const configComponents = components.filter(c => c?.type?.toLowerCase() === 'config');
      const paramsComponents = components.filter(c => c?.type?.toLowerCase() === 'params');
      const chartComponents = components.filter(c => !c.type || c?.type?.toLowerCase() === 'chart');

      // Register source/style/config components (no visual output)
      for (const comp of [...sourceComponents, ...styleComponents, ...configComponents]) {
        await chartml.render(comp, container);
      }

      // Render params components (have UI)
      for (const comp of paramsComponents) {
        await chartml.render(comp, container);
      }

      // Hide source/style/config/params containers
      if (sourceComponents.length > 0 || styleComponents.length > 0 || configComponents.length > 0 || paramsComponents.length > 0) {
        if (chartComponents.length === 0) {
          // No charts to render, hide the entire block
          container.style.display = 'none';
          const parent = container.closest('.chartml-block');
          if (parent) parent.style.display = 'none';
        }
      }

      // Render chart components with grid layout
      if (chartComponents.length > 0) {
        container.innerHTML = ''; // Clear for charts

        // Create grid wrapper for charts (matches React plugin)
        const gridWrapper = document.createElement('div');
        gridWrapper.className = 'grid grid-cols-12 gap-4';
        gridWrapper.style.margin = '1rem 0';

        // Append grid to DOM first so child elements can measure their width
        container.appendChild(gridWrapper);

        for (const chartSpec of chartComponents) {
          // Get colSpan from layout (default to 12 for full width)
          const colSpan = chartSpec?.layout?.colSpan || 12;
          const colSpanClass = getColSpanClass(colSpan);

          // Create grid item
          const gridItem = document.createElement('div');
          gridItem.className = colSpanClass;

          // Append to DOM BEFORE rendering so offsetWidth is available for responsive sizing
          gridWrapper.appendChild(gridItem);

          // Render chart into grid item (now has correct offsetWidth)
          await chartml.render(chartSpec, gridItem);
        }
      }
    } catch (error) {
      console.error('Error rendering ChartML:', error);

      // Show error in container
      container.innerHTML = '';
      const errorElement = createErrorElement(error);
      container.appendChild(errorElement);
    }
  }
}

/**
 * Auto-render charts when DOM is ready
 */
if (typeof window !== 'undefined') {
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', renderAllCharts);
  } else {
    // DOM is already ready
    renderAllCharts();
  }
}
