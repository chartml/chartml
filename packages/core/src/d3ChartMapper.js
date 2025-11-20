/**
 * ChartML v2 to D3 Cartesian Chart Configuration Mapper
 *
 * Converts ChartML visualize specifications to unified D3 cartesian chart config.
 * Chart type becomes the default mark - individual rows can override.
 */

import { getChartColors } from './colorUtils';

/**
 * Normalize field specification to consistent format with mark and axis
 */
function normalizeField(field, defaultMark) {
  if (typeof field === 'string') {
    return {
      field,
      mark: defaultMark,
      axis: 'left',
      color: null,
      label: field,
      dataLabels: null
    };
  }
  return {
    field: field.field,
    mark: field.mark || defaultMark,
    axis: field.axis || 'left',
    color: field.color || null,
    label: field.label || field.field,
    dataLabels: field.dataLabels || null
  };
}

/**
 * Parse date strings if they appear to be dates
 */
function parseDataDates(data, xField) {
  if (!data || data.length === 0) return data;

  const sampleValue = data[0][xField];

  // Check if it's an ISO date string
  if (typeof sampleValue === 'string' && /^\d{4}-\d{2}-\d{2}/.test(sampleValue)) {
    return data.map(d => ({
      ...d,
      [xField]: new Date(d[xField])
    }));
  }

  return data;
}

/**
 * Pivot data from long to wide format when marks.color is used for grouping
 */
function pivotDataByColor(data, xField, yField, colorField) {
  const xValues = [...new Set(data.map(d => d[xField]))];
  const colorValues = [...new Set(data.map(d => d[colorField]))];

  const pivotedData = xValues.map(xValue => {
    const row = { [xField]: xValue };
    colorValues.forEach(colorValue => {
      const match = data.find(d => d[xField] === xValue && d[colorField] === colorValue);
      row[colorValue] = match ? match[yField] : 0;
    });
    return row;
  });

  return { pivotedData, colorValues };
}

/**
 * Map ChartML visualize spec to unified D3 cartesian chart config
 */
export function mapToCartesianChart(visualizeSpec, data, instanceConfig = {}) {
  const { type, mode = 'stacked', orientation = 'vertical', rows, columns, marks = {}, axes = {}, style = {}, annotations = [] } = visualizeSpec;

  // Parse dates if needed
  const columnField = Array.isArray(columns) ? columns[0] : columns;
  const xField = (typeof columnField === 'string' ? columnField : columnField?.field) || 'x';
  let processedData = parseDataDates(data, xField);

  // Normalize row fields
  const rowFields = Array.isArray(rows) ? rows : [rows];

  // Handle marks.color pivoting for single row with color grouping
  let normalizedRows;
  if (marks.color && rowFields.length === 1) {
    const yField = typeof rowFields[0] === 'string' ? rowFields[0] : rowFields[0].field;
    const { pivotedData, colorValues } = pivotDataByColor(processedData, xField, yField, marks.color);
    processedData = pivotedData;

    // Create rows from color values
    normalizedRows = colorValues.map((colorValue, idx) =>
      normalizeField({ field: colorValue, label: colorValue }, type, idx)
    );
  } else {
    // Standard row normalization
    normalizedRows = rowFields.map((row, idx) => normalizeField(row, type, idx));
  }

  // Determine colors - check spec first, then fall back to instance default
  const basePalette = style.colors || instanceConfig.defaultPalette || [
    '#2E7D9A', '#D4A445', '#4A7C59', '#D66B5B', '#8B6BA8', '#9BB85A',
    '#A85A6B', '#5A6BA8', '#B87D5A', '#5A9B9B', '#759B75', '#A8758B'
  ];

  const seriesCount = normalizedRows.length;
  const chartColors = getChartColors(seriesCount, basePalette);

  return {
    xField,
    rows: normalizedRows,
    type, // Default mark type
    mode, // 'stacked' or 'grouped'
    orientation, // 'vertical' or 'horizontal' - only applies to bar charts
    width: style.width || instanceConfig.dimensions?.width || 600,
    height: style.height || instanceConfig.dimensions?.height || 400,
    axes: {
      x: {
        label: axes.x?.label || '',
        format: axes.x?.format
      },
      left: {
        label: axes.left?.label || '',
        format: axes.left?.format,
        min: axes.left?.min,
        max: axes.left?.max,
        nice: axes.left?.nice !== false  // Default true
      },
      right: {
        label: axes.right?.label || '',
        format: axes.right?.format,
        min: axes.right?.min,
        max: axes.right?.max,
        nice: axes.right?.nice !== false  // Default true
      }
    },
    colors: chartColors,
    curveType: style.curveType || 'curveMonotoneX',
    showDots: style.showDots !== false, // Default true
    fillOpacity: style.fillOpacity || 0.6,
    style: {
      grid: style.grid  // Pass through grid configuration
    },
    annotations: annotations || [],  // Pass through annotations (reference lines/bands)
    data: processedData
  };
}

/**
 * Map ChartML visualize spec to D3 scatter plot config
 */
export function mapToScatterPlot(visualizeSpec, instanceConfig = {}) {
  const { rows, columns, marks = {}, axes = {}, style = {} } = visualizeSpec;

  const rowField = Array.isArray(rows) ? rows[0] : rows;
  const columnField = Array.isArray(columns) ? columns[0] : columns;

  const yField = (typeof rowField === 'string' ? rowField : rowField?.field) || 'y';
  const xField = (typeof columnField === 'string' ? columnField : columnField?.field) || 'x';

  // Determine colors - check spec first, then fall back to instance default
  const colors = style.colors || instanceConfig.defaultPalette || [
    '#2E7D9A', '#D4A445', '#4A7C59', '#D66B5B', '#8B6BA8', '#9BB85A',
    '#A85A6B', '#5A6BA8', '#B87D5A', '#5A9B9B', '#759B75', '#A8758B'
  ];

  return {
    xField,
    yField,
    sizeField: marks.size || null,
    colorField: marks.color || null,
    width: style.width || instanceConfig.dimensions?.width || 600,
    height: style.height || instanceConfig.dimensions?.height || 400,
    xAxisLabel: axes.x?.label || '',
    yAxisLabel: axes.left?.label || '',
    colors,
    radiusRange: [5, 20]
  };
}

/**
 * Map ChartML visualize spec to D3 pie/doughnut chart config
 */
export function mapToPieChart(visualizeSpec, data, chartType = 'pie', instanceConfig = {}) {
  const { rows, columns, style = {} } = visualizeSpec;

  const rowField = Array.isArray(rows) ? rows[0] : rows;
  const columnField = Array.isArray(columns) ? columns[0] : columns;

  const valueField = (typeof rowField === 'string' ? rowField : rowField?.field) || 'value';
  const categoryField = (typeof columnField === 'string' ? columnField : columnField?.field) || 'category';

  // Determine colors - check spec first, then fall back to instance default
  const basePalette = style.colors || instanceConfig.defaultPalette || [
    '#2E7D9A', '#D4A445', '#4A7C59', '#D66B5B', '#8B6BA8', '#9BB85A',
    '#A85A6B', '#5A6BA8', '#B87D5A', '#5A9B9B', '#759B75', '#A8758B'
  ];

  const sliceCount = data ? data.length : 12;
  const chartColors = getChartColors(sliceCount, basePalette);

  return {
    categoryField,
    valueField,
    type: chartType,
    width: style.width || instanceConfig.dimensions?.width || 600,
    height: style.height || instanceConfig.dimensions?.height || 400,
    colors: chartColors
  };
}

/**
 * Map ChartML visualize spec to metric card config
 */
export function mapToMetricCard(visualizeSpec, data) {
  const { value, label, format, compareWith, invertTrend = false, style = {} } = visualizeSpec;

  // Get the data row (should be single row for metrics)
  const dataRow = data[0] || {};

  const currentValue = dataRow[value];
  const previousValue = compareWith ? dataRow[compareWith] : null;

  // Calculate comparison if we have both values
  let comparison = null;
  if (previousValue != null && currentValue != null) {
    const change = currentValue - previousValue;
    const percentChange = (change / previousValue) * 100;

    // Determine actual direction
    let direction = 'neutral';
    if (percentChange > 0) {
      direction = 'up';
    } else if (percentChange < 0) {
      direction = 'down';
    }

    // Determine if this is a good or bad change
    let isGood = null;
    if (direction !== 'neutral') {
      if (invertTrend) {
        // For inverted metrics (like error rate), down is good, up is bad
        isGood = direction === 'down';
      } else {
        // For normal metrics, up is good, down is bad
        isGood = direction === 'up';
      }
    }

    comparison = {
      change,
      percentChange,
      direction,  // Actual direction: 'up', 'down', or 'neutral'
      isGood      // Whether this change is good (true), bad (false), or neutral (null)
    };
  }

  return {
    value: currentValue,
    label: label || value,
    format: format || null,
    comparison,
    align: style.align || 'center',  // Default to center alignment
    showLabel: style.showLabel !== false  // Default to true, can be disabled
  };
}

/**
 * Main mapping function
 */
export function mapChartMLToD3Config(visualizeSpec, data, title = null, instanceConfig = {}) {
  const { type } = visualizeSpec;

  switch (type) {
    case 'bar':
    case 'line':
    case 'area': {
      // All cartesian charts use the unified renderer
      const mapped = mapToCartesianChart(visualizeSpec, data, instanceConfig);
      return {
        chartType: 'cartesian',
        config: { ...mapped, title },
        data: mapped.data
      };
    }

    case 'scatter':
      return {
        chartType: 'scatter',
        config: { ...mapToScatterPlot(visualizeSpec, instanceConfig), title },
        data
      };

    case 'pie':
    case 'doughnut':
      return {
        chartType: type,
        config: { ...mapToPieChart(visualizeSpec, data, type, instanceConfig), title },
        data
      };

    case 'table':
      return {
        chartType: 'table',
        config: { spec: visualizeSpec, title },
        data
      };

    case 'metric':
      return {
        chartType: 'metric',
        config: { ...mapToMetricCard(visualizeSpec, data), title },
        data
      };

    default:
      throw new Error(`Unknown chart type: ${type}`);
  }
}
