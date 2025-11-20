/**
 * Format Utilities for ChartML
 *
 * Maps ChartML format strings to D3 format functions.
 * Supports both number formatting (d3.format) and date formatting (d3.timeFormat).
 */

import * as d3 from 'd3';

/**
 * Create a formatter function from a ChartML format string
 *
 * @param {string} formatString - ChartML format string (e.g., "$,.0f", "%b %d, %Y")
 * @param {string} type - Value type hint: 'number', 'date', or 'auto'
 * @returns {Function} Formatter function that takes a value and returns formatted string
 */
export function createFormatter(formatString, type = 'auto') {
  if (!formatString) {
    return (value) => String(value);
  }

  // Auto-detect format type if not specified
  if (type === 'auto') {
    // Date formats use % followed by a letter (like %Y, %m, %d)
    // Number percentage formats end with % (like .1%, .2%)
    const isDateFormat = /%[A-Za-z]/.test(formatString);
    type = isDateFormat ? 'date' : 'number';
  }

  if (type === 'date') {
    return createDateFormatter(formatString);
  } else {
    return createNumberFormatter(formatString);
  }
}

/**
 * Create a number formatter function
 *
 * @param {string} formatString - D3 number format string
 * @returns {Function} Number formatter
 */
function createNumberFormatter(formatString) {
  try {
    // Handle special format types
    if (formatString === '~s') {
      // SI prefix format (1.2k, 3.4M, etc.)
      return d3.format('~s');
    } else if (formatString.includes('%')) {
      // Percentage format
      return d3.format(formatString);
    } else {
      // Standard number format
      return d3.format(formatString);
    }
  } catch (error) {
    console.warn(`[Formatter] Invalid number format: ${formatString}`, error);
    return (value) => String(value);
  }
}

/**
 * Create a date formatter function
 *
 * @param {string} formatString - D3 time format string (strftime pattern)
 * @returns {Function} Date formatter
 */
function createDateFormatter(formatString) {
  try {
    const formatter = d3.timeFormat(formatString);
    return (value) => {
      if (value instanceof Date) {
        return formatter(value);
      } else if (typeof value === 'string' || typeof value === 'number') {
        return formatter(new Date(value));
      }
      return String(value);
    };
  } catch (error) {
    console.warn(`[Formatter] Invalid date format: ${formatString}`, error);
    return (value) => String(value);
  }
}

/**
 * Format a value using ChartML format string
 *
 * @param {*} value - Value to format
 * @param {string} formatString - ChartML format string
 * @param {string} type - Value type hint: 'number', 'date', or 'auto'
 * @returns {string} Formatted string
 */
export function formatValue(value, formatString, type = 'auto') {
  if (value == null || value === undefined) {
    return '';
  }

  const formatter = createFormatter(formatString, type);
  return formatter(value);
}

/**
 * Common format presets for convenience
 */
export const COMMON_FORMATS = {
  // Numbers
  INTEGER: ',.0f',                    // 1,234
  DECIMAL_1: ',.1f',                  // 1,234.5
  DECIMAL_2: ',.2f',                  // 1,234.56

  // Currency
  CURRENCY: '$,.0f',                  // $1,234
  CURRENCY_2: '$,.2f',                // $1,234.56

  // Percentage
  PERCENT: '.0%',                     // 45%
  PERCENT_1: '.1%',                   // 45.2%
  PERCENT_2: '.2%',                   // 45.23%

  // Abbreviated
  SI_PREFIX: '~s',                    // 1.2k, 3.4M, 2.1G

  // Dates
  DATE_ISO: '%Y-%m-%d',               // 2025-01-15
  DATE_SHORT: '%b %d, %Y',            // Jan 15, 2025
  DATE_LONG: '%B %d, %Y',             // January 15, 2025
  DATE_US: '%m/%d/%Y',                // 01/15/2025
  DATETIME_SHORT: '%b %d, %I:%M %p',  // Jan 15, 2:30 PM
  YEAR_MONTH: '%Y-%m',                // 2025-01
  QUARTER: '%Y Q%q',                  // 2025 Q1

  // Time
  TIME_12: '%I:%M %p',                // 2:30 PM
  TIME_24: '%H:%M',                   // 14:30
  TIME_FULL: '%I:%M:%S %p',           // 2:30:45 PM
};

/**
 * Detect appropriate format for a value
 *
 * @param {*} value - Value to analyze
 * @returns {string} Suggested format string
 */
export function detectFormat(value) {
  if (value instanceof Date) {
    return COMMON_FORMATS.DATE_SHORT;
  } else if (typeof value === 'number') {
    if (Number.isInteger(value)) {
      return COMMON_FORMATS.INTEGER;
    } else {
      return COMMON_FORMATS.DECIMAL_2;
    }
  }
  return null;
}
