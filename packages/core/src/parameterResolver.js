/**
 * Parameter Resolution for ChartML
 *
 * Provides generic parameter substitution with two syntaxes:
 * 1. Named params: $blockname.param_id (e.g., $dashboard_filters.region)
 * 2. Chart-level params: $param_id (e.g., $top_n)
 *
 * This allows flexible parameter references anywhere in the spec:
 * - In data queries: WHERE region = '$dashboard_filters.region'
 * - In aggregate filters: value: "$dashboard_filters.selected_regions"
 * - In aggregate limit: limit: "$top_n"
 * - In visualize config: title: "Sales in $dashboard_filters.region"
 *
 * Design is framework-agnostic and doesn't depend on Kyomi-specific logic.
 */

/**
 * Resolve parameter references in a ChartML specification
 *
 * Supports two syntaxes:
 * 1. Named params: "$blockname.param_id" (e.g., "$dashboard_filters.region")
 * 2. Chart-level params: "$param_id" (e.g., "$top_n")
 *
 * Uses string substitution on the entire spec.
 *
 * @param {Object} spec - ChartML specification object
 * @param {Object} paramValues - Current parameter values
 *   For named params: { "blockname.param_id": value }
 *   For chart-level: { "param_id": value }
 * @param {Object} chartParams - Optional chart-level inline params definitions for fallback
 * @returns {Object} Spec with all parameter references resolved
 *
 * @example
 * // Named params
 * const spec = {
 *   data: "SELECT * FROM sales WHERE region = '$dashboard_filters.region'",
 *   aggregate: {
 *     filter: [{ field: 'status', value: '$dashboard_filters.status' }]
 *   }
 * };
 *
 * const resolved = resolveParamReferences(spec, {
 *   "dashboard_filters.region": 'US',
 *   "dashboard_filters.status": 'active'
 * });
 *
 * @example
 * // Chart-level params
 * const spec = {
 *   aggregate: { limit: "$top_n" }
 * };
 *
 * const resolved = resolveParamReferences(spec, { "top_n": 10 });
 */
export function resolveParamReferences(spec, paramValues = {}, chartParams = null) {
  if (!spec) return spec;

  // Convert spec to JSON string for easy replacement
  let specString = JSON.stringify(spec);

  // Pattern matches: "$identifier" or "$identifier.path"
  // Where identifier can be a block name or param name
  const paramReferenceRegex = /"\$([a-zA-Z0-9_]+(?:\.[a-zA-Z0-9_]+)*)"/g;

  // Replace each reference with its value
  specString = specString.replace(paramReferenceRegex, (match, path) => {
    // Check if this is a named param (has dot) or chart-level param (no dot)
    const hasDot = path.includes('.');

    let value;

    if (hasDot) {
      // Named param: $blockname.param_id
      // Direct lookup using full key (paramValues uses flat dot-notation keys)
      value = paramValues[path];

      if (value === undefined) {
        console.warn(`[ChartML] Named parameter reference not found: $${path}`);
        return match;
      }
    } else {
      // Chart-level param: $param_id
      // First check paramValues (for when chart params are in global state)
      value = paramValues[path];

      // If not found and we have chart params, check their defaults
      if (value === undefined && chartParams && Array.isArray(chartParams)) {
        const paramDef = chartParams.find(p => p.id === path);
        if (paramDef) {
          value = paramDef.default;
        }
      }

      if (value === undefined) {
        console.warn(`[ChartML] Chart-level parameter reference not found: $${path}`);
        return match;
      }
    }

    // Return JSON-encoded value (handles strings, numbers, booleans, arrays, objects)
    return JSON.stringify(value);
  });

  return JSON.parse(specString);
}

/**
 * Get nested value from object using dot notation
 *
 * @param {Object} obj - Object to read from
 * @param {string} path - Dot-separated path (e.g., 'region' or 'filters.region')
 * @returns {*} Value at path, or undefined if not found
 *
 * @example
 * getNestedValue({ region: 'US', filters: { status: 'active' } }, 'filters.status')
 * // Returns: 'active'
 */
function getNestedValue(obj, path) {
  const parts = path.split('.');
  let current = obj;

  for (const part of parts) {
    if (current === null || current === undefined) {
      return undefined;
    }
    current = current[part];
  }

  return current;
}

/**
 * Extract parameter references from a spec
 *
 * Useful for determining which params a chart depends on.
 *
 * @param {Object} spec - ChartML specification
 * @returns {Set<string>} Set of parameter paths referenced
 *   Named params: 'dashboard_filters.region'
 *   Chart params: 'top_n'
 *
 * @example
 * const refs = extractParamReferences({
 *   data: "SELECT * FROM sales WHERE region = '$dashboard_filters.region'",
 *   aggregate: { limit: "$top_n" }
 * });
 * // Returns: Set(['dashboard_filters.region', 'top_n'])
 */
export function extractParamReferences(spec) {
  if (!spec) return new Set();

  const specString = JSON.stringify(spec);
  const paramReferenceRegex = /"\$([a-zA-Z0-9_]+(?:\.[a-zA-Z0-9_]+)*)"/g;
  const references = new Set();

  let match;
  while ((match = paramReferenceRegex.exec(specString)) !== null) {
    // Add the full path (e.g., 'dashboard_filters.region' or 'top_n')
    references.add(match[1]);
  }

  return references;
}

/**
 * Validate that all parameter references can be resolved
 *
 * Checks if all parameter references in the spec have corresponding values.
 *
 * @param {Object} spec - ChartML specification
 * @param {Object} paramValues - Available parameter values
 * @param {Array} chartParams - Optional chart-level params definitions
 * @returns {Object} Validation result { valid: boolean, missing: string[] }
 *
 * @example
 * const result = validateParamReferences(
 *   { data: "WHERE region = '$dashboard_filters.region'" },
 *   { "dashboard_filters.status": 'active' }
 * );
 * // Returns: { valid: false, missing: ['dashboard_filters.region'] }
 */
export function validateParamReferences(spec, paramValues = {}, chartParams = null) {
  const references = extractParamReferences(spec);
  const missing = [];

  for (const ref of references) {
    const hasDot = ref.includes('.');

    if (hasDot) {
      // Named param - check in paramValues
      if (getNestedValue(paramValues, ref) === undefined) {
        missing.push(ref);
      }
    } else {
      // Chart-level param - check paramValues first, then chart params defaults
      if (!(ref in paramValues)) {
        // Check if it's defined in chart params with a default
        const paramDef = chartParams?.find(p => p.id === ref);
        if (!paramDef || paramDef.default === undefined) {
          missing.push(ref);
        }
      }
    }
  }

  return {
    valid: missing.length === 0,
    missing
  };
}
