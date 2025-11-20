/**
 * ChartML Component Registry
 *
 * Manages registration and resolution of reusable ChartML components:
 * - Sources (data definitions)
 * - Styles (theme definitions)
 * - Configs (scope-level defaults)
 *
 * Supports page-level and global scopes for markdown plugin integration.
 */

import { deepMerge } from './deepMerge.js';

/**
 * Registry for storing ChartML component definitions
 */
class ComponentRegistry {
  constructor(paramChangeRegistry = null) {
    this.sources = new Map();
    this.styles = new Map();
    this.configs = new Map();
    this.params = new Map();  // Store params definitions and values
    this.paramChangeRegistry = paramChangeRegistry;  // Optional registry for param change notifications
  }

  /**
   * Register a source component
   * @param {string} name - Unique identifier
   * @param {Object} definition - Source definition
   * @throws {Error} If name already exists
   */
  registerSource(name, definition) {
    if (!name || typeof name !== 'string') {
      throw new Error('Source name must be a non-empty string');
    }

    if (this.sources.has(name)) {
      throw new Error(`Source "${name}" is already registered. Use a unique name.`);
    }

    // Validate required fields
    if (!definition.provider) {
      throw new Error(`Source "${name}" must specify a provider (e.g., inline, http, or custom plugin provider)`);
    }

    this.sources.set(name, { ...definition });
  }

  /**
   * Register a style component
   * @param {string} name - Unique identifier
   * @param {Object} definition - Style definition
   * @throws {Error} If name already exists
   */
  registerStyle(name, definition) {
    if (!name || typeof name !== 'string') {
      throw new Error('Style name must be a non-empty string');
    }

    if (this.styles.has(name)) {
      throw new Error(`Style "${name}" is already registered. Use a unique name.`);
    }

    this.styles.set(name, { ...definition });
  }

  /**
   * Register a config component
   * @param {Object} definition - Config definition
   */
  registerConfig(definition) {
    // Configs don't have names - they're scope-level
    // Store by incremental ID to support multiple configs
    const id = this.configs.size;
    this.configs.set(id, { ...definition });
  }

  /**
   * Register a params component
   * @param {string} name - Unique identifier (required for named params blocks)
   * @param {Object} definition - Params definition
   * @throws {Error} If name already exists
   */
  registerParams(name, definition) {
    if (!name || typeof name !== 'string') {
      throw new Error('Params name must be a non-empty string');
    }

    if (this.params.has(name)) {
      throw new Error(`Params "${name}" is already registered. Use a unique name.`);
    }

    // Validate that params array exists
    if (!definition.params || !Array.isArray(definition.params)) {
      throw new Error(`Params "${name}" must have a params array`);
    }

    // Initialize with default values (keyed by param.id)
    const values = {};
    definition.params.forEach(param => {
      if (!param.id) {
        throw new Error(`Param in "${name}" must have an id field`);
      }
      values[param.id] = param.default;
    });

    this.params.set(name, {
      definition: { ...definition },
      values
    });
  }

  /**
   * Resolve a source reference
   * @param {string} name - Source name to resolve
   * @returns {Object|null} Source definition or null if not found
   */
  resolveSource(name) {
    if (!name || typeof name !== 'string') {
      return null;
    }
    return this.sources.get(name) || null;
  }

  /**
   * Resolve a style reference
   * @param {string} name - Style name to resolve
   * @returns {Object|null} Style definition or null if not found
   */
  resolveStyle(name) {
    if (!name || typeof name !== 'string') {
      return null;
    }
    return this.styles.get(name) || null;
  }

  /**
   * Resolve a params reference
   * @param {string} name - Params name to resolve
   * @returns {Object|null} Params definition or null if not found
   */
  resolveParams(name) {
    if (!name || typeof name !== 'string') {
      return null;
    }
    const params = this.params.get(name);
    return params ? params.definition : null;
  }

  /**
   * Get current parameter values for a named params block
   * @param {string} name - Params block name
   * @returns {Object} Current parameter values { param_id: value }
   */
  getParamValues(name) {
    if (!name || typeof name !== 'string') {
      return {};
    }
    const params = this.params.get(name);
    return params ? { ...params.values } : {};
  }

  /**
   * Set a parameter value
   * @param {string} name - Params block name
   * @param {string} paramId - Parameter id
   * @param {*} value - New value
   */
  setParamValue(name, paramId, value) {
    if (!name || typeof name !== 'string') {
      throw new Error('Params name must be a non-empty string');
    }

    const params = this.params.get(name);

    if (!params) {
      throw new Error(`Params "${name}" not found. Register it first.`);
    }

    // Validate that param exists in definition
    const paramDef = params.definition.params?.find(p => p.id === paramId);
    if (!paramDef) {
      throw new Error(`Parameter "${paramId}" not found in params "${name}"`);
    }

    // Compare old vs new value to prevent unnecessary notifications
    const oldValue = params.values[paramId];
    const valueChanged = oldValue !== value;

    // Update value
    params.values[paramId] = value;

    // Notify subscribers ONLY if value actually changed (loop prevention)
    if (valueChanged && this.paramChangeRegistry) {
      this.paramChangeRegistry.notifyChange(name, paramId, value);
    }
  }

  /**
   * Get merged config
   * Merges all registered configs in registration order
   * @returns {Object} Merged configuration
   */
  getMergedConfig() {
    let merged = {};

    // Merge all configs in order
    for (const [, config] of this.configs) {
      merged = deepMerge(merged, config);
    }

    return merged;
  }

  /**
   * Clear all registered components
   * Useful for testing or page navigation
   */
  clear() {
    this.sources.clear();
    this.styles.clear();
    this.configs.clear();
    this.params.clear();
  }

  /**
   * Get registry statistics
   * @returns {Object} Count of each component type
   */
  getStats() {
    return {
      sources: this.sources.size,
      styles: this.styles.size,
      configs: this.configs.size,
      params: this.params.size
    };
  }

  /**
   * Check if a source exists
   * @param {string} name - Source name
   * @returns {boolean}
   */
  hasSource(name) {
    return this.sources.has(name);
  }

  /**
   * Check if a style exists
   * @param {string} name - Style name
   * @returns {boolean}
   */
  hasStyle(name) {
    return this.styles.has(name);
  }

  /**
   * Check if a params block exists
   * @param {string} name - Params name
   * @returns {boolean}
   */
  hasParams(name) {
    if (!name || typeof name !== 'string') {
      return false;
    }
    return this.params.has(name);
  }
}

/**
 * Global registry instance
 */
let globalRegistry = new ComponentRegistry();

/**
 * Get the global registry
 * @returns {ComponentRegistry}
 */
export function getGlobalRegistry() {
  return globalRegistry;
}

/**
 * Create a new isolated registry
 * Useful for page-level scoping in markdown plugins
 * @param {ParamChangeRegistry} paramChangeRegistry - Optional param change registry for notifications
 * @returns {ComponentRegistry}
 */
export function createRegistry(paramChangeRegistry = null) {
  return new ComponentRegistry(paramChangeRegistry);
}

/**
 * Reset the global registry
 * Useful for testing or page transitions
 */
export function resetGlobalRegistry() {
  globalRegistry = new ComponentRegistry();
}

export { ComponentRegistry };
