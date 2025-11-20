/**
 * Color utilities for chart palette management
 *
 * Provides 12-color base palettes with automatic fallback generation
 * for charts with 13+ categories.
 */

/**
 * Generate fallback colors using desaturation algorithm
 *
 * For charts with >12 categories, generate muted variants:
 * - Reduce saturation by 40%
 * - Shift luminosity toward mid-range (70% of original + 15%)
 * - Maintain hue relationships for consistency
 *
 * @param {string} hex - Base color in hex format (#RRGGBB)
 * @returns {string} Desaturated color in hex format
 */
export function generateFallbackColor(hex) {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);

  // Convert to HSL
  const rNorm = r / 255;
  const gNorm = g / 255;
  const bNorm = b / 255;

  const max = Math.max(rNorm, gNorm, bNorm);
  const min = Math.min(rNorm, gNorm, bNorm);
  let h, s, l = (max + min) / 2;

  if (max === min) {
    h = s = 0;
  } else {
    const d = max - min;
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min);

    switch (max) {
      case rNorm: h = ((gNorm - bNorm) / d + (gNorm < bNorm ? 6 : 0)) / 6; break;
      case gNorm: h = ((bNorm - rNorm) / d + 2) / 6; break;
      case bNorm: h = ((rNorm - gNorm) / d + 4) / 6; break;
    }
  }

  // Reduce saturation by 40% and shift luminosity toward mid-range
  s = s * 0.6;
  l = l * 0.7 + 0.15; // Shift toward 50% luminosity

  // Convert back to RGB
  const hue2rgb = (p, q, t) => {
    if (t < 0) t += 1;
    if (t > 1) t -= 1;
    if (t < 1/6) return p + (q - p) * 6 * t;
    if (t < 1/2) return q;
    if (t < 2/3) return p + (q - p) * (2/3 - t) * 6;
    return p;
  };

  let newR, newG, newB;
  if (s === 0) {
    newR = newG = newB = l;
  } else {
    const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
    const p = 2 * l - q;
    newR = hue2rgb(p, q, h + 1/3);
    newG = hue2rgb(p, q, h);
    newB = hue2rgb(p, q, h - 1/3);
  }

  const toHex = (n) => {
    const hex = Math.round(n * 255).toString(16);
    return hex.length === 1 ? '0' + hex : hex;
  };

  return `#${toHex(newR)}${toHex(newG)}${toHex(newB)}`;
}

/**
 * Generate fallback palette from base colors
 *
 * Creates 12 additional desaturated variants for colors 13-24
 *
 * @param {string[]} baseColors - Array of 12 base colors
 * @returns {string[]} Array of 12 fallback colors
 */
export function generateFallbackColors(baseColors) {
  if (!baseColors || baseColors.length === 0) {
    console.warn('[colorUtils] No base colors provided for fallback generation');
    return [];
  }

  return baseColors.map(color => generateFallbackColor(color));
}

/**
 * Get colors for a chart based on series count
 *
 * Recommended usage:
 * - 1-5 categories: First 5 colors for maximum contrast
 * - 6-12 categories: Full 12-color palette
 * - 13+ categories: Automatic fallback with desaturated variants
 *
 * @param {number} seriesCount - Number of series/categories in chart
 * @param {string[]} basePalette - Base 12-color palette
 * @returns {string[]} Array of colors for the chart
 */
export function getChartColors(seriesCount, basePalette) {
  if (!basePalette || basePalette.length === 0) {
    console.warn('[colorUtils] No base palette provided');
    return [];
  }

  // For 1-12 series, use base palette
  if (seriesCount <= 12) {
    return basePalette.slice(0, seriesCount);
  }

  // For 13-24 series, add fallback colors
  if (seriesCount <= 24) {
    const fallbacks = generateFallbackColors(basePalette);
    return [...basePalette, ...fallbacks].slice(0, seriesCount);
  }

  // For >24 series, repeat the pattern
  // This is not ideal, but provides graceful degradation
  const fallbacks = generateFallbackColors(basePalette);
  const fullPalette = [...basePalette, ...fallbacks];

  const colors = [];
  for (let i = 0; i < seriesCount; i++) {
    colors.push(fullPalette[i % fullPalette.length]);
  }

  console.warn(`[colorUtils] Chart has ${seriesCount} series. Consider filtering data, using small multiples, or grouping smaller categories.`);

  return colors;
}

/**
 * Validate hex color format
 *
 * @param {string} hex - Color string to validate
 * @returns {boolean} True if valid hex color
 */
export function isValidHexColor(hex) {
  return /^#[0-9A-F]{6}$/i.test(hex);
}

/**
 * Get color by index from palette with fallback support
 *
 * @param {number} index - Color index
 * @param {string[]} basePalette - Base palette
 * @returns {string} Color at index (with fallback if needed)
 */
export function getColorAtIndex(index, basePalette) {
  if (index < 12) {
    return basePalette[index];
  }

  // Generate fallback colors if needed
  const fallbacks = generateFallbackColors(basePalette);
  const fallbackIndex = index - 12;

  if (fallbackIndex < 12) {
    return fallbacks[fallbackIndex];
  }

  // For >24, cycle through the combined palette
  const fullPalette = [...basePalette, ...fallbacks];
  return fullPalette[index % fullPalette.length];
}
