/**
 * Unit tests for d3ChartMapper - range mark floor/ceiling
 */

import { describe, it, expect } from 'vitest';
import { mapToCartesianChart } from '../src/d3ChartMapper.js';

describe('d3ChartMapper - Range mark floor/ceiling', () => {
  const baseSpec = {
    type: 'line',
    columns: 'date',
    rows: [
      { field: 'forecast', mark: 'line' },
    ]
  };
  const data = [
    { date: '2025-01', forecast: 100, upper_bound: 120, lower_bound: -10 }
  ];

  it('should pass through floor: 0 on range mark', () => {
    const spec = {
      ...baseSpec,
      rows: [
        ...baseSpec.rows,
        { mark: 'range', upper: 'upper_bound', lower: 'lower_bound', floor: 0 }
      ]
    };
    const result = mapToCartesianChart(spec, data);
    const rangeRow = result.rows.find(r => r.mark === 'range');
    expect(rangeRow.floor).toBe(0);
  });

  it('should pass through ceiling on range mark', () => {
    const spec = {
      ...baseSpec,
      rows: [
        ...baseSpec.rows,
        { mark: 'range', upper: 'upper_bound', lower: 'lower_bound', ceiling: 100 }
      ]
    };
    const result = mapToCartesianChart(spec, data);
    const rangeRow = result.rows.find(r => r.mark === 'range');
    expect(rangeRow.ceiling).toBe(100);
  });

  it('should set floor and ceiling to null when not provided', () => {
    const spec = {
      ...baseSpec,
      rows: [
        ...baseSpec.rows,
        { mark: 'range', upper: 'upper_bound', lower: 'lower_bound' }
      ]
    };
    const result = mapToCartesianChart(spec, data);
    const rangeRow = result.rows.find(r => r.mark === 'range');
    expect(rangeRow.floor).toBeNull();
    expect(rangeRow.ceiling).toBeNull();
  });
});
