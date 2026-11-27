/**
 * Unit tests for d3ChartMapper - range mark floor/ceiling
 */

import { describe, it, expect } from 'vitest';
import { mapToCartesianChart, mapChartMLToD3Config } from '../src/d3ChartMapper.js';

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

describe('d3ChartMapper - metric card title', () => {
  // CHA-49: the heading above the card (index.js) carries the title, so the
  // mapped card config must not turn a title into an in-card label too.
  const spec = { type: 'metric', value: 'revenue', format: '$,.0f' };
  const data = [{ revenue: 45000 }];

  it('does not use the title as the in-card label', () => {
    const { config } = mapChartMLToD3Config(spec, data, 'Total Revenue');
    expect(config.title).toBe('Total Revenue');
    expect(config.label).toBeNull();
    expect(config.showLabel).toBe(false);
  });

  it('shows an explicit label inside the card', () => {
    const { config } = mapChartMLToD3Config({ ...spec, label: 'Revenue' }, data, 'Total Revenue');
    expect(config.label).toBe('Revenue');
    expect(config.showLabel).toBe(true);
  });
});
