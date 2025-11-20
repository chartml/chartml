/**
 * Global configuration system for ChartML
 *
 * Supports configuration hierarchy:
 * 1. System defaults (built-in)
 * 2. Developer defaults (set via configure())
 * 3. Chart-level overrides (in ChartML spec)
 *
 * Uses deep merge for configuration precedence.
 * Accepts both JavaScript objects and YAML strings.
 */

import { deepMerge } from './deepMerge.js';
import yaml from 'js-yaml';

/**
 * System-level defaults (built into the library)
 */
const SYSTEM_DEFAULTS = {
  theme: {
    colors: [
      '#E67E22', // Orange
      '#3498DB', // Blue
      '#2ECC71', // Green
      '#9B59B6', // Purple
      '#E74C3C', // Red
      '#1ABC9C', // Turquoise
      '#F39C12', // Yellow
      '#34495E'  // Dark gray
    ],
    background: '#FFFFFF',
    fonts: {
      title: {
        size: 16,
        weight: 600,
        family: 'system-ui, -apple-system, sans-serif',
        color: '#374151'
      },
      axis: {
        size: 12,
        family: 'system-ui, -apple-system, sans-serif',
        color: '#6B7280'
      },
      legend: {
        size: 12,
        family: 'system-ui, -apple-system, sans-serif',
        color: '#374151'
      }
    },
    grid: {
      color: '#E5E7EB',
      opacity: 0.5
    },
    padding: {
      top: 20,
      right: 20,
      bottom: 60,
      left: 60
    }
  }
};

/**
 * Developer-configured defaults (set via configure())
 * Starts as empty, merged with system defaults
 */
let developerConfig = {};

/**
 * Set developer-level configuration
 * This merges with system defaults
 *
 * @param {Object|string} config - Configuration object or YAML string
 *
 * @example JavaScript object
 * configure({
 *   theme: {
 *     colors: ['#FF6B6B', '#4ECDC4', '#45B7D1'],
 *     fonts: {
 *       title: { family: 'Inter, sans-serif' }
 *     }
 *   }
 * });
 *
 * @example YAML string
 * configure(`
 *   theme:
 *     colors: ['#FF6B6B', '#4ECDC4']
 *     fonts:
 *       title:
 *         family: Inter, sans-serif
 * `);
 */
export function configure(config) {
  if (!config) {
    developerConfig = {};
    return;
  }

  // If config is a string, parse as YAML
  if (typeof config === 'string') {
    try {
      developerConfig = yaml.load(config);
    } catch (error) {
      console.error('Failed to parse YAML configuration:', error);
      developerConfig = {};
    }
  } else {
    developerConfig = config;
  }
}

/**
 * Get current developer configuration
 * @returns {Object} Developer config
 */
export function getDeveloperConfig() {
  return developerConfig;
}

/**
 * Reset configuration to system defaults
 */
export function resetConfig() {
  developerConfig = {};
}

/**
 * Get merged configuration for a chart
 * Applies precedence: system → developer → chart spec
 *
 * @param {Object} chartSpec - Chart specification (optional overrides)
 * @returns {Object} Merged configuration
 */
export function getMergedConfig(chartSpec = {}) {
  // Start with system defaults
  let merged = { ...SYSTEM_DEFAULTS };

  // Merge developer config
  if (Object.keys(developerConfig).length > 0) {
    merged = deepMerge(merged, developerConfig);
  }

  // Merge chart-level spec (style section)
  if (chartSpec.style) {
    // Map ChartML style properties to config structure
    const chartConfig = {
      theme: {}
    };

    if (chartSpec.style.colors) {
      chartConfig.theme.colors = chartSpec.style.colors;
    }

    if (chartSpec.style.background) {
      chartConfig.theme.background = chartSpec.style.background;
    }

    if (chartSpec.style.fonts) {
      chartConfig.theme.fonts = chartSpec.style.fonts;
    }

    if (chartSpec.style.grid) {
      chartConfig.theme.grid = chartSpec.style.grid;
    }

    merged = deepMerge(merged, chartConfig);
  }

  return merged;
}

/**
 * Get system defaults (for reference/documentation)
 * @returns {Object} System default configuration
 */
export function getSystemDefaults() {
  return { ...SYSTEM_DEFAULTS };
}
