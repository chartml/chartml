/**
 * Deep merge utility for configuration objects
 * Used for merging theme/config hierarchies
 */

/**
 * Deep merge two objects
 * Arrays are replaced, not merged
 * Nested objects are recursively merged
 *
 * @param {Object} target - Base object
 * @param {Object} source - Object to merge in
 * @returns {Object} Merged object
 */
export function deepMerge(target, source) {
  if (!source || typeof source !== 'object') {
    return target;
  }

  const result = { ...target };

  for (const key in source) {
    if (source.hasOwnProperty(key)) {
      const sourceValue = source[key];
      const targetValue = result[key];

      // If source value is an array, replace target value
      if (Array.isArray(sourceValue)) {
        result[key] = [...sourceValue];
      }
      // If both are objects, recursively merge
      else if (
        sourceValue !== null &&
        typeof sourceValue === 'object' &&
        !Array.isArray(sourceValue) &&
        targetValue !== null &&
        typeof targetValue === 'object' &&
        !Array.isArray(targetValue)
      ) {
        result[key] = deepMerge(targetValue, sourceValue);
      }
      // Otherwise, source overwrites target
      else {
        result[key] = sourceValue;
      }
    }
  }

  return result;
}
