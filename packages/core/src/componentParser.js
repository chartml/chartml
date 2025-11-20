/**
 * ChartML Component Parser and Validator
 *
 * Parses and validates ChartML YAML blocks to determine component type
 * and ensure all required fields are present.
 *
 * Supports ChartML v1.0 component types:
 * - source: Reusable data definitions
 * - style: Reusable style/theme definitions
 * - config: Page or scope-level configuration defaults
 * - chart: Visualization definitions (can reference sources/styles)
 */

import yaml from 'js-yaml';

/**
 * Supported ChartML component types
 */
export const COMPONENT_TYPES = {
  SOURCE: 'source',
  STYLE: 'style',
  CONFIG: 'config',
  PARAMS: 'params',
  CHART: 'chart'
};

/**
 * Supported ChartML versions
 */
const SUPPORTED_VERSIONS = ['1', '1.0'];

/**
 * Parse ChartML YAML string into component object
 *
 * @param {string} yamlString - ChartML YAML string
 * @returns {Object} Parsed component with type and properties
 * @throws {Error} If YAML is invalid or component type is unsupported
 */
export function parseComponent(yamlString) {
  if (!yamlString || typeof yamlString !== 'string') {
    throw new Error('ChartML component must be a non-empty string');
  }

  // Parse YAML
  let parsed;
  try {
    parsed = yaml.load(yamlString);
  } catch (error) {
    throw new Error(`Failed to parse ChartML YAML: ${error.message}`);
  }

  if (!parsed || typeof parsed !== 'object') {
    throw new Error('ChartML component must be a valid YAML object');
  }

  // Determine component type
  const componentType = determineComponentType(parsed);

  // Validate version if present
  if (parsed.version) {
    validateVersion(parsed.version);
  }

  // Validate component based on type
  validateComponent(componentType, parsed);

  return {
    type: componentType,
    spec: parsed,
    raw: yamlString
  };
}

/**
 * Determine the component type from parsed YAML
 *
 * @param {Object} spec - Parsed YAML object
 * @returns {string} Component type (source, style, config, or chart)
 * @throws {Error} If type field is missing or invalid
 */
function determineComponentType(spec) {
  if (!spec.type || typeof spec.type !== 'string') {
    throw new Error('ChartML component must specify a "type" field (source, style, config, or chart)');
  }

  const type = spec.type.toLowerCase();

  if (!Object.values(COMPONENT_TYPES).includes(type)) {
    throw new Error(
      `Invalid component type: "${spec.type}". Must be one of: ${Object.values(COMPONENT_TYPES).join(', ')}`
    );
  }

  return type;
}

/**
 * Validate ChartML version
 *
 * @param {string|number} version - Version from YAML
 * @throws {Error} If version is not supported
 */
function validateVersion(version) {
  const versionStr = String(version);

  if (!SUPPORTED_VERSIONS.includes(versionStr)) {
    throw new Error(
      `Unsupported ChartML version: ${version}. Supported versions: ${SUPPORTED_VERSIONS.join(', ')}`
    );
  }
}

/**
 * Validate component based on its type
 *
 * @param {string} type - Component type
 * @param {Object} spec - Parsed component specification
 * @throws {Error} If required fields are missing or invalid
 */
function validateComponent(type, spec) {
  switch (type) {
    case COMPONENT_TYPES.SOURCE:
      validateSourceComponent(spec);
      break;

    case COMPONENT_TYPES.STYLE:
      validateStyleComponent(spec);
      break;

    case COMPONENT_TYPES.CONFIG:
      validateConfigComponent(spec);
      break;

    case COMPONENT_TYPES.PARAMS:
      validateParamsComponent(spec);
      break;

    case COMPONENT_TYPES.CHART:
      validateChartComponent(spec);
      break;

    default:
      throw new Error(`Unknown component type: ${type}`);
  }
}

/**
 * Validate source component
 *
 * Required fields:
 * - name: Unique identifier for this source
 * - provider: Data source type (inline, http, or custom plugin)
 * - Additional fields depend on provider type
 */
function validateSourceComponent(spec) {
  if (!spec.name || typeof spec.name !== 'string') {
    throw new Error('Source component must have a "name" field (string)');
  }

  if (!spec.provider || typeof spec.provider !== 'string') {
    throw new Error(
      `Source "${spec.name}" must specify a "provider" field (e.g., inline, http, or a custom plugin provider)`
    );
  }

  // Validate provider-specific fields for built-in providers only
  // Custom plugin providers are validated by the plugin itself
  switch (spec.provider.toLowerCase()) {
    case 'inline':
      if (!spec.rows || !Array.isArray(spec.rows)) {
        throw new Error(`Source "${spec.name}" with provider "inline" must have a "rows" field (array)`);
      }
      break;

    case 'http':
      if (!spec.url || typeof spec.url !== 'string') {
        throw new Error(`Source "${spec.name}" with provider "http" must have a "url" field (string)`);
      }
      break;

    case 'api':
      if (!spec.endpoint || typeof spec.endpoint !== 'string') {
        throw new Error(`Source "${spec.name}" with provider "api" must have an "endpoint" field (string)`);
      }
      break;
  }
}

/**
 * Validate style component
 *
 * Required fields:
 * - name: Unique identifier for this style
 * - At least one style property (colors, background, fonts, etc.)
 */
function validateStyleComponent(spec) {
  if (!spec.name || typeof spec.name !== 'string') {
    throw new Error('Style component must have a "name" field (string)');
  }

  // Check that at least one style property exists
  const styleProperties = ['colors', 'background', 'fonts', 'grid', 'padding', 'width', 'height'];
  const hasStyleProperty = styleProperties.some(prop => spec.hasOwnProperty(prop));

  if (!hasStyleProperty) {
    throw new Error(
      `Style "${spec.name}" must define at least one style property: ${styleProperties.join(', ')}`
    );
  }

  // Validate colors if present
  if (spec.colors && !Array.isArray(spec.colors)) {
    throw new Error(`Style "${spec.name}" colors must be an array of color strings`);
  }
}

/**
 * Validate config component
 *
 * Config components don't require a name - they apply to the scope (page/global)
 * They should have a theme object with configuration properties
 */
function validateConfigComponent(spec) {
  // Config doesn't require specific fields, but should have meaningful content
  if (!spec.theme || typeof spec.theme !== 'object') {
    console.warn('Config component should typically have a "theme" object with configuration properties');
  }
}

/**
 * Validate params component
 *
 * Required fields:
 * - name: Unique identifier for this params group (optional but recommended)
 * - params: Array of parameter definitions
 */
function validateParamsComponent(spec) {
  // Name is optional but recommended
  if (spec.name && typeof spec.name !== 'string') {
    throw new Error('Params component "name" must be a string if provided');
  }

  // Must have params array
  if (!spec.params || !Array.isArray(spec.params)) {
    throw new Error('Params component must have a "params" field (array of parameter definitions)');
  }

  if (spec.params.length === 0) {
    throw new Error('Params component must have at least one parameter definition');
  }

  // Validate each parameter definition
  spec.params.forEach((param, index) => {
    if (!param.id || typeof param.id !== 'string') {
      throw new Error(`Parameter at index ${index} must have an "id" property (string)`);
    }

    if (!param.type || typeof param.type !== 'string') {
      throw new Error(`Parameter "${param.id}" must have a "type" property (select, multiselect, number, date, etc.)`);
    }

    // Validate type-specific requirements
    const validTypes = ['select', 'multiselect', 'number', 'number_range', 'date', 'daterange', 'text'];
    if (!validTypes.includes(param.type)) {
      throw new Error(
        `Parameter "${param.id}" has invalid type "${param.type}". Must be one of: ${validTypes.join(', ')}`
      );
    }

    // Select types require options
    if ((param.type === 'select' || param.type === 'multiselect') && !Array.isArray(param.options)) {
      throw new Error(`Parameter "${param.id}" with type "${param.type}" must have "options" array`);
    }
  });
}

/**
 * Validate chart component
 *
 * Required fields:
 * - Either data (inline/url) OR dataSource (reference to named source)
 * - visualize: Chart visualization specification
 */
function validateChartComponent(spec) {
  // Must have either inline data or a data source reference
  const hasInlineData = spec.data !== undefined;
  const hasDataSource = spec.dataSource && typeof spec.dataSource === 'string';

  if (!hasInlineData && !hasDataSource) {
    throw new Error(
      'Chart component must have either "data" (inline data/URL) or "dataSource" (reference to named source)'
    );
  }

  if (hasInlineData && hasDataSource) {
    throw new Error(
      'Chart component cannot have both "data" and "dataSource". Use one or the other.'
    );
  }

  // Must have visualize block
  if (!spec.visualize || typeof spec.visualize !== 'object') {
    throw new Error('Chart component must have a "visualize" block with chart configuration');
  }

  // Visualize must have a type
  if (!spec.visualize.type || typeof spec.visualize.type !== 'string') {
    throw new Error('Chart "visualize" block must specify a chart "type" (bar, line, pie, etc.)');
  }
}

/**
 * Parse multiple ChartML components from markdown content
 *
 * Useful for two-pass rendering where we need to collect all
 * source/style/config definitions before rendering charts
 *
 * @param {Array<string>} yamlBlocks - Array of ChartML YAML strings
 * @returns {Object} Object with arrays of each component type
 */
export function parseMultipleComponents(yamlBlocks) {
  const components = {
    sources: [],
    styles: [],
    configs: [],
    params: [],
    charts: []
  };

  const errors = [];

  for (let i = 0; i < yamlBlocks.length; i++) {
    try {
      const component = parseComponent(yamlBlocks[i]);

      switch (component.type) {
        case COMPONENT_TYPES.SOURCE:
          components.sources.push(component);
          break;

        case COMPONENT_TYPES.STYLE:
          components.styles.push(component);
          break;

        case COMPONENT_TYPES.CONFIG:
          components.configs.push(component);
          break;

        case COMPONENT_TYPES.PARAMS:
          components.params.push(component);
          break;

        case COMPONENT_TYPES.CHART:
          components.charts.push(component);
          break;
      }
    } catch (error) {
      errors.push({
        blockIndex: i,
        error: error.message,
        content: yamlBlocks[i]
      });
    }
  }

  return {
    components,
    errors
  };
}

/**
 * Check if a component has a reference to another component
 *
 * @param {Object} spec - Parsed component specification
 * @returns {Object} Object with source, style, and params references
 */
export function extractReferences(spec) {
  const references = {
    dataSource: null,
    style: null,
    params: null
  };

  // Check for data source reference
  if (spec.dataSource && typeof spec.dataSource === 'string') {
    references.dataSource = spec.dataSource;
  }

  // Check for style reference
  if (spec.visualize?.style && typeof spec.visualize.style === 'string') {
    references.style = spec.visualize.style;
  }

  // Check for params reference
  if (spec.params && typeof spec.params === 'string') {
    references.params = spec.params;
  }

  return references;
}
