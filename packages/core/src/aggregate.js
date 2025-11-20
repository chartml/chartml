/**
 * Built-in aggregation using d3-array
 *
 * Lightweight aggregation for typical dashboard datasets (<10k rows).
 * Supports: GROUP BY, aggregations (sum/avg/count/min/max), filters, sort, limit, calculated fields.
 *
 * For larger datasets or complex SQL operations, users can install aggregate middleware plugins.
 *
 * CACHING:
 * - In-memory cache to prevent redundant BigQuery calls on re-renders
 * - Cache key based on data source spec + aggregation spec
 * - TTL of 5 minutes (balances freshness vs. performance)
 */

import * as d3 from 'd3-array';

// In-memory cache: Map<cacheKey, { result, timestamp }>
const aggregateCache = new Map();
const CACHE_TTL_MS = 5 * 60 * 1000; // 5 minutes

// In-flight request tracking: Map<cacheKey, Promise>
// Prevents duplicate fetches when multiple renders happen simultaneously
const inFlightRequests = new Map();

/**
 * Generate cache key from data source and aggregation spec
 */
function generateCacheKey(spec, context) {
  // Include data source spec (query, inline data, etc.)
  const dataSpec = spec.data || {};

  // Include aggregation spec
  const aggregateSpec = spec.aggregate || {};

  // Create a stable string representation
  return JSON.stringify({
    data: dataSpec,
    aggregate: aggregateSpec
  });
}

/**
 * Built-in aggregation middleware using d3-array
 *
 * @param {Array<Object>|null} data - Input data array (null if not yet fetched)
 * @param {Object} spec - Pipeline specification
 * @param {Object} [spec.aggregate] - Aggregation specification
 * @param {Object} context - Plugin context
 * @param {Function} [context.fetchData] - Lazy callback to fetch data from source
 * @param {boolean} [context.bypassCache] - If true, skip cache and fetch fresh data
 * @returns {Promise<Array<Object>>} Transformed data array
 */
export async function d3Aggregate(data, spec, context = {}) {
  if (!spec) {
    return {
      data: data,
      metadata: {}
    };
  }

  const aggregateSpec = spec.aggregate || {};

  const { dimensions = [], measures = [], sort, limit, filters } = aggregateSpec;

  // No aggregation needed - but may still need to filter
  if (dimensions.length === 0 && measures.length === 0) {
    if (!data && context.fetchData) {
      data = await context.fetchData();
    }

    // Extract actual data
    const actualData = data?.data !== undefined ? data.data : data;

    // Apply filters if present
    const resultData = filters ? applyFilters(actualData, filters) : actualData;

    return {
      data: resultData,
      metadata: {
        refreshedAt: Date.now(),
        cacheHit: false
      }
    };
  }

  // Generate cache key once for all cache operations
  const cacheKey = !context.bypassCache ? generateCacheKey(spec, context) : null;

  // Check cache (unless bypass requested)
  if (!context.bypassCache && cacheKey) {
    const cached = aggregateCache.get(cacheKey);

    if (cached) {
      const age = Date.now() - cached.timestamp;

      // Return cached result if still fresh
      if (age < CACHE_TTL_MS) {
        console.log('[d3Aggregate] Cache HIT - returning cached result');
        return {
          data: cached.result,
          metadata: {
            refreshedAt: cached.timestamp,
            cacheHit: true
          }
        };
      } else {
        // Cache expired - remove it
        console.log('[d3Aggregate] Cache expired - fetching fresh data');
        aggregateCache.delete(cacheKey);
      }
    }

    // Check if there's an in-flight request for this same query
    const inFlight = inFlightRequests.get(cacheKey);
    if (inFlight) {
      console.log('[d3Aggregate] In-flight request found - waiting for it');
      return await inFlight;
    }
  }

  // Create the aggregation promise and track it as in-flight
  const aggregationPromise = (async () => {
    try {
      // Fetch data if not already provided
      if (!data && context.fetchData) {
        console.log('[d3Aggregate] Fetching data from source');
        data = await context.fetchData();
      }

      // Extract actual data and metadata if it's a Result Object from data source
      const actualData = data?.data !== undefined ? data.data : data;
      const sourceMetadata = data?.metadata || {};

      // Split filters into pre-aggregation (dimensions) and post-aggregation (measures/expressions)
      const { preAggFilters, postAggFilters } = splitFilters(filters, dimensions, measures);

      // Apply pre-aggregation filters (dimensions only - WHERE clause equivalent)
      const filteredData = preAggFilters ? applyFilters(actualData, preAggFilters) : actualData;

      // Group by dimensions and compute measures
      let result;

      if (dimensions.length === 0) {
        // No grouping - compute measures over entire dataset
        const aggregated = {};
        measures.forEach(measure => {
          if (!measure.expression) {
            aggregated[measure.name] = computeMeasure(filteredData, measure);
          }
        });
        result = [aggregated];

      } else if (dimensions.length === 1) {
        // Single dimension grouping
        const grouped = d3.rollup(
          filteredData,
          group => {
            const aggregated = {};
            // Include dimension value
            aggregated[dimensions[0]] = group[0][dimensions[0]];
            // Compute measures
            measures.forEach(measure => {
              if (!measure.expression) {
                aggregated[measure.name] = computeMeasure(group, measure);
              }
            });
            return aggregated;
          },
          d => d[dimensions[0]]
        );

        result = Array.from(grouped.values());

      } else {
        // Multi-dimension grouping
        const grouped = d3.rollup(
          filteredData,
          group => {
            const aggregated = {};
            // Include all dimension values
            dimensions.forEach(dim => {
              aggregated[dim] = group[0][dim];
            });
            // Compute measures
            measures.forEach(measure => {
              if (!measure.expression) {
                aggregated[measure.name] = computeMeasure(group, measure);
              }
            });
            return aggregated;
          },
          ...dimensions.map(dim => d => d[dim])
        );

        result = flattenNestedMap(grouped, dimensions);
      }

      // Calculated fields (expressions)
      const expressionMeasures = measures.filter(m => m.expression);
      if (expressionMeasures.length > 0) {
        result = result.map(row => {
          const extended = { ...row };
          expressionMeasures.forEach(measure => {
            try {
              extended[measure.name] = evaluateExpression(measure.expression, extended);
            } catch (error) {
              console.warn(`Failed to evaluate expression "${measure.expression}":`, error.message);
              extended[measure.name] = null;
            }
          });
          return extended;
        });
      }

      // Apply post-aggregation filters (measures/expressions - HAVING clause equivalent)
      if (postAggFilters) {
        result = applyFilters(result, postAggFilters);
      }

      // Sorting
      if (sort?.length > 0) {
        result.sort((a, b) => {
          for (const s of sort) {
            const aVal = a[s.field];
            const bVal = b[s.field];

            // Handle null/undefined
            if (aVal == null && bVal == null) continue;
            if (aVal == null) return 1;
            if (bVal == null) return -1;

            // Compare values
            if (aVal === bVal) continue;
            const cmp = aVal < bVal ? -1 : 1;
            return s.direction === 'desc' ? -cmp : cmp;
          }
          return 0;
        });
      }

      // Limit
      if (limit && limit > 0) {
        result = result.slice(0, limit);
      }

      // Use refreshedAt from source if available (from DuckDB cache), otherwise current time
      const timestamp = sourceMetadata.refreshedAt || Date.now();

      // Store in cache for future renders (unless bypass was requested)
      if (!context.bypassCache && cacheKey) {
        aggregateCache.set(cacheKey, {
          result: result,
          timestamp: timestamp
        });
        console.log('[d3Aggregate] Cached aggregated result');
      }

      // Return Result Object with metadata
      // Preserve refreshedAt from source (DuckDB cache) if available
      return {
        data: result,
        metadata: {
          refreshedAt: timestamp,
          cacheHit: false,
          sourceWasCached: !!sourceMetadata.refreshedAt  // True if data came from DuckDB cache
        }
      };
    } finally {
      // Remove from in-flight tracking
      if (cacheKey) {
        inFlightRequests.delete(cacheKey);
      }
    }
  })();

  // Track this as an in-flight request
  if (cacheKey) {
    inFlightRequests.set(cacheKey, aggregationPromise);
  }

  return await aggregationPromise;
}

/**
 * Compute a single measure on a group of rows
 */
function computeMeasure(group, measure) {
  if (measure.expression) {
    // Calculated fields are computed after aggregation
    return null;
  }

  const { column, aggregation } = measure;

  switch (aggregation?.toLowerCase()) {
    case 'sum':
      return d3.sum(group, d => Number(d[column]) || 0);

    case 'avg':
    case 'mean':
      return d3.mean(group, d => Number(d[column]) || 0) || 0;

    case 'count':
      return group.length;

    case 'min':
      return d3.min(group, d => d[column]);

    case 'max':
      return d3.max(group, d => d[column]);

    case 'first':
      return group[0]?.[column];

    case 'last':
      return group[group.length - 1]?.[column];

    default:
      throw new Error(`Unknown aggregation: ${aggregation}`);
  }
}

/**
 * Flatten nested Map from multi-dimension rollup
 */
function flattenNestedMap(map, dimensions, depth = 0) {
  const result = [];

  if (depth >= dimensions.length) {
    return [map];
  }

  for (const [key, value] of map.entries()) {
    if (value instanceof Map) {
      const nested = flattenNestedMap(value, dimensions, depth + 1);
      result.push(...nested);
    } else {
      result.push(value);
    }
  }

  return result;
}

/**
 * Simple expression evaluator for calculated fields
 * Supports: +, -, *, /, field references, parentheses
 *
 * Security: Uses Function constructor with restricted scope (safer than eval)
 * Additional safety: Validates operators, limits complexity, has timeout
 */
function evaluateExpression(expression, context) {
  try {
    // Safety limit: Max expression length to prevent DoS
    if (expression.length > 500) {
      throw new Error('Expression too long (max 500 characters)');
    }

    // Safety check: Whitelist allowed operators and characters
    // Allow: numbers, operators (+, -, *, /), parens, spaces, and identifiers
    const allowedPattern = /^[a-zA-Z0-9_+\-*/().\s]+$/;
    if (!allowedPattern.test(expression)) {
      throw new Error('Expression contains disallowed characters');
    }

    // Safety check: Prevent dangerous patterns
    const dangerousPatterns = [
      /\.\./,           // Prototype pollution attempts
      /__proto__/,      // Prototype pollution
      /constructor/,    // Constructor access
      /eval/i,          // Eval attempts
      /function/i,      // Function declarations
      /import/i,        // Import statements
      /require/i,       // Require statements
    ];

    for (const pattern of dangerousPatterns) {
      if (pattern.test(expression)) {
        throw new Error('Expression contains disallowed pattern');
      }
    }

    // Create a safe context with only the data fields (no global access)
    const safeContext = { ...context };

    // Replace field names with context lookups
    let code = expression;

    // Extract all identifiers (field names)
    const identifiers = expression.match(/\b[a-zA-Z_][a-zA-Z0-9_]*\b/g) || [];

    // Replace each identifier with context lookup
    identifiers.forEach(identifier => {
      // Skip JavaScript keywords
      const keywords = ['true', 'false', 'null', 'undefined', 'NaN', 'Infinity'];
      if (keywords.includes(identifier)) return;

      // Replace field name with context access
      const regex = new RegExp(`\\b${identifier}\\b`, 'g');
      code = code.replace(regex, `context.${identifier}`);
    });

    // Create function with restricted scope
    const fn = new Function('context', `'use strict'; return (${code});`);

    // Execute with timeout protection (prevents infinite loops)
    // Note: This is a basic timeout - for production, consider using a worker thread
    const timeout = 100; // 100ms max
    const startTime = Date.now();

    const result = fn(safeContext);

    if (Date.now() - startTime > timeout) {
      throw new Error('Expression execution timeout');
    }

    // Ensure result is a number
    return Number(result) || 0;

  } catch (error) {
    throw new Error(`Expression evaluation failed: ${expression} - ${error.message}`);
  }
}

/**
 * Split filters into pre-aggregation (dimensions) and post-aggregation (measures/expressions)
 *
 * Pre-aggregation filters (WHERE clause): Filter on dimension fields before grouping
 * Post-aggregation filters (HAVING clause): Filter on aggregated measures/expressions after grouping
 *
 * @param {Object} filters - Filter specification
 * @param {Array} dimensions - Array of dimension field names
 * @param {Array} measures - Array of measure definitions
 * @returns {Object} { preAggFilters, postAggFilters }
 */
function splitFilters(filters, dimensions, measures) {
  if (!filters || !filters.rules || filters.rules.length === 0) {
    return { preAggFilters: null, postAggFilters: null };
  }

  // Get all measure field names (both direct aggregations and computed expressions)
  const measureFields = new Set(measures.map(m => m.name));

  // Separate rules based on whether they reference dimensions or measures
  const preAggRules = [];
  const postAggRules = [];

  filters.rules.forEach(rule => {
    if (measureFields.has(rule.field)) {
      // Filter references a measure/expression → post-aggregation (HAVING)
      postAggRules.push(rule);
    } else if (dimensions.includes(rule.field)) {
      // Filter references a dimension → pre-aggregation (WHERE)
      preAggRules.push(rule);
    } else {
      // Unknown field - apply pre-aggregation to be safe
      console.warn(`[d3Aggregate] Filter field "${rule.field}" not found in dimensions or measures, applying pre-aggregation`);
      preAggRules.push(rule);
    }
  });

  return {
    preAggFilters: preAggRules.length > 0 ? { ...filters, rules: preAggRules } : null,
    postAggFilters: postAggRules.length > 0 ? { ...filters, rules: postAggRules } : null
  };
}

/**
 * Apply filters to data
 * Supports combinator logic (and/or) with multiple rules
 *
 * @param {Array} data - Array of data objects
 * @param {Object} filters - Filter specification
 * @param {string} filters.combinator - 'and' or 'or'
 * @param {Array} filters.rules - Array of filter rules
 * @returns {Array} Filtered data
 */
function applyFilters(data, filters) {
  if (!filters || !filters.rules || filters.rules.length === 0) {
    return data;
  }

  const { combinator = 'and', rules } = filters;

  return data.filter(row => {
    const results = rules.map(rule => applyRule(row, rule));

    if (combinator === 'or') {
      return results.some(r => r);
    } else {
      // Default to 'and'
      return results.every(r => r);
    }
  });
}

/**
 * Apply a single filter rule to a row
 *
 * @param {Object} row - Data row
 * @param {Object} rule - Filter rule {field, operator, value}
 * @returns {boolean} True if row passes the rule
 */
function applyRule(row, rule) {
  const { field, operator, value } = rule;
  const fieldValue = row[field];

  switch (operator) {
    case '=':
    case '==':
      return fieldValue == value;

    case '!=':
      return fieldValue != value;

    case '>':
      return fieldValue > value;

    case '>=':
      return fieldValue >= value;

    case '<':
      return fieldValue < value;

    case '<=':
      return fieldValue <= value;

    case 'in':
      return Array.isArray(value) && value.includes(fieldValue);

    case 'not in':
      return Array.isArray(value) && !value.includes(fieldValue);

    case 'contains':
      return String(fieldValue).includes(String(value));

    case 'not contains':
      return !String(fieldValue).includes(String(value));

    case 'starts with':
      return String(fieldValue).startsWith(String(value));

    case 'ends with':
      return String(fieldValue).endsWith(String(value));

    case 'is null':
      return fieldValue === null || fieldValue === undefined;

    case 'is not null':
      return fieldValue !== null && fieldValue !== undefined;

    default:
      console.warn(`[d3Aggregate] Unknown operator: ${operator}`);
      return true;  // Don't filter out on unknown operator
  }
}
