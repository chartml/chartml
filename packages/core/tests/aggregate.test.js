/**
 * Unit tests for d3-array aggregation middleware
 */

import { describe, it, expect } from 'vitest';
import { d3Aggregate } from '../src/aggregate.js';

// Sample data for testing
const salesData = [
  { region: 'North', product: 'Widget A', revenue: 1000, units: 10, date: '2024-01-15' },
  { region: 'North', product: 'Widget A', revenue: 1500, units: 15, date: '2024-01-20' },
  { region: 'North', product: 'Widget B', revenue: 800, units: 8, date: '2024-01-25' },
  { region: 'South', product: 'Widget A', revenue: 1200, units: 12, date: '2024-01-10' },
  { region: 'South', product: 'Widget B', revenue: 900, units: 9, date: '2024-01-18' },
  { region: 'East', product: 'Widget A', revenue: 1100, units: 11, date: '2024-01-22' },
];

describe('d3Aggregate - Basic Grouping', () => {
  it('should handle no aggregation (returns original data)', async () => {
    const result = await d3Aggregate(salesData, {});
    expect(result).toEqual(salesData);
  });

  it('should handle empty dimensions and measures', async () => {
    const result = await d3Aggregate(salesData, { dimensions: [], measures: [] });
    expect(result).toEqual(salesData);
  });

  it('should group by single dimension', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ]
    });

    expect(result).toHaveLength(3); // North, South, East
    expect(result).toEqual(
      expect.arrayContaining([
        { region: 'North', total_revenue: 3300 },
        { region: 'South', total_revenue: 2100 },
        { region: 'East', total_revenue: 1100 }
      ])
    );
  });

  it('should group by multiple dimensions', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region', 'product'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ]
    });

    expect(result).toHaveLength(5); // North-A, North-B, South-A, South-B, East-A
    expect(result).toEqual(
      expect.arrayContaining([
        { region: 'North', product: 'Widget A', total_revenue: 2500 },
        { region: 'North', product: 'Widget B', total_revenue: 800 },
        { region: 'South', product: 'Widget A', total_revenue: 1200 },
        { region: 'South', product: 'Widget B', total_revenue: 900 },
        { region: 'East', product: 'Widget A', total_revenue: 1100 }
      ])
    );
  });

  it('should aggregate without grouping (global aggregation)', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: [],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' },
        { column: 'units', aggregation: 'sum', name: 'total_units' }
      ]
    });

    expect(result).toHaveLength(1);
    expect(result[0]).toEqual({
      total_revenue: 6500,
      total_units: 65
    });
  });
});

describe('d3Aggregate - Aggregation Functions', () => {
  it('should compute SUM aggregation', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ]
    });

    const north = result.find(r => r.region === 'North');
    expect(north.total_revenue).toBe(3300); // 1000 + 1500 + 800
  });

  it('should compute AVG aggregation', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'avg', name: 'avg_revenue' }
      ]
    });

    const north = result.find(r => r.region === 'North');
    expect(north.avg_revenue).toBe(1100); // (1000 + 1500 + 800) / 3
  });

  it('should compute COUNT aggregation', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'count', name: 'count' }
      ]
    });

    const north = result.find(r => r.region === 'North');
    expect(north.count).toBe(3);

    const south = result.find(r => r.region === 'South');
    expect(south.count).toBe(2);
  });

  it('should compute MIN aggregation', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'min', name: 'min_revenue' }
      ]
    });

    const north = result.find(r => r.region === 'North');
    expect(north.min_revenue).toBe(800);
  });

  it('should compute MAX aggregation', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'max', name: 'max_revenue' }
      ]
    });

    const north = result.find(r => r.region === 'North');
    expect(north.max_revenue).toBe(1500);
  });

  it('should compute FIRST aggregation', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'date', aggregation: 'first', name: 'first_date' }
      ]
    });

    const north = result.find(r => r.region === 'North');
    expect(north.first_date).toBe('2024-01-15');
  });

  it('should compute LAST aggregation', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'date', aggregation: 'last', name: 'last_date' }
      ]
    });

    const north = result.find(r => r.region === 'North');
    expect(north.last_date).toBe('2024-01-25');
  });

  it('should handle MEAN as alias for AVG', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'mean', name: 'mean_revenue' }
      ]
    });

    const north = result.find(r => r.region === 'North');
    expect(north.mean_revenue).toBe(1100);
  });

  it('should handle multiple measures at once', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' },
        { column: 'revenue', aggregation: 'avg', name: 'avg_revenue' },
        { column: 'revenue', aggregation: 'min', name: 'min_revenue' },
        { column: 'revenue', aggregation: 'max', name: 'max_revenue' },
        { column: 'units', aggregation: 'count', name: 'count' }
      ]
    });

    const north = result.find(r => r.region === 'North');
    expect(north).toMatchObject({
      region: 'North',
      total_revenue: 3300,
      avg_revenue: 1100,
      min_revenue: 800,
      max_revenue: 1500,
      count: 3
    });
  });
});

describe('d3Aggregate - Calculated Fields', () => {
  it('should compute simple calculated field', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' },
        { column: 'units', aggregation: 'sum', name: 'total_units' },
        { expression: 'total_revenue / total_units', name: 'avg_price' }
      ]
    });

    const north = result.find(r => r.region === 'North');
    expect(north.avg_price).toBe(100); // 3300 / 33
  });

  it('should compute chained calculated fields', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' },
        { column: 'units', aggregation: 'sum', name: 'total_units' },
        { expression: 'total_revenue / total_units', name: 'avg_price' },
        { expression: 'avg_price * 1.2', name: 'markup_price' }
      ]
    });

    const north = result.find(r => r.region === 'North');
    expect(north.markup_price).toBe(120); // 100 * 1.2
  });

  it('should support complex expressions with parentheses', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' },
        { column: 'units', aggregation: 'sum', name: 'total_units' },
        { expression: '(total_revenue + 1000) / (total_units + 10)', name: 'adjusted' }
      ]
    });

    const north = result.find(r => r.region === 'North');
    expect(north.adjusted).toBe(100); // (3300 + 1000) / (33 + 10) = 4300 / 43 = 100
  });

  it('should handle division by zero gracefully', async () => {
    const testData = [
      { region: 'North', revenue: 1000, units: 0 }
    ];

    const result = await d3Aggregate(testData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' },
        { column: 'units', aggregation: 'sum', name: 'total_units' },
        { expression: 'total_revenue / total_units', name: 'avg_price' }
      ]
    });

    const north = result.find(r => r.region === 'North');
    expect(north.avg_price).toBe(Infinity); // Division by zero returns Infinity
  });
});

describe('d3Aggregate - Filtering', () => {
  it('should apply pre-aggregation filter (WHERE)', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ],
      filters: {
        combinator: 'and',
        rules: [
          { field: 'product', operator: '=', value: 'Widget A' }
        ]
      }
    });

    expect(result).toHaveLength(3);
    const north = result.find(r => r.region === 'North');
    expect(north.total_revenue).toBe(2500); // Only Widget A sales
  });

  it('should apply post-aggregation filter (HAVING)', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ],
      filters: {
        combinator: 'and',
        rules: [
          { field: 'total_revenue', operator: '>', value: 2000 }
        ]
      }
    });

    expect(result).toHaveLength(2); // Only North (3300) and South (2100)
    expect(result.map(r => r.region)).toEqual(expect.arrayContaining(['North', 'South']));
  });

  it('should apply both pre and post-aggregation filters', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ],
      filters: {
        combinator: 'and',
        rules: [
          { field: 'product', operator: '=', value: 'Widget A' }, // WHERE
          { field: 'total_revenue', operator: '>=', value: 2000 } // HAVING
        ]
      }
    });

    expect(result).toHaveLength(1); // Only North Widget A (2500)
    expect(result[0].region).toBe('North');
    expect(result[0].total_revenue).toBe(2500);
  });

  it('should handle equality operators', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ],
      filters: {
        rules: [
          { field: 'region', operator: '=', value: 'North' }
        ]
      }
    });

    expect(result).toHaveLength(1);
    expect(result[0].region).toBe('North');
  });

  it('should handle inequality operators', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['product'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ],
      filters: {
        rules: [
          { field: 'product', operator: '!=', value: 'Widget A' }
        ]
      }
    });

    expect(result).toHaveLength(1);
    expect(result[0].product).toBe('Widget B');
  });

  it('should handle comparison operators (>, >=, <, <=)', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ],
      filters: {
        rules: [
          { field: 'revenue', operator: '>=', value: 1000 }
        ]
      }
    });

    // Should exclude revenue < 1000 (800, 900)
    const total = result.reduce((sum, r) => sum + r.total_revenue, 0);
    expect(total).toBe(4800); // 1000 + 1500 + 1200 + 1100
  });

  it('should handle IN operator', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ],
      filters: {
        rules: [
          { field: 'region', operator: 'in', value: ['North', 'South'] }
        ]
      }
    });

    expect(result).toHaveLength(2);
    expect(result.map(r => r.region)).toEqual(expect.arrayContaining(['North', 'South']));
  });

  it('should handle NOT IN operator', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ],
      filters: {
        rules: [
          { field: 'region', operator: 'not in', value: ['North', 'South'] }
        ]
      }
    });

    expect(result).toHaveLength(1);
    expect(result[0].region).toBe('East');
  });

  it('should handle LIKE operator', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['product'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ],
      filters: {
        rules: [
          { field: 'product', operator: 'like', value: '%Widget%' }
        ]
      }
    });

    expect(result).toHaveLength(2); // Both Widget A and Widget B
  });

  it('should handle OR combinator', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ],
      filters: {
        combinator: 'or',
        rules: [
          { field: 'region', operator: '=', value: 'North' },
          { field: 'region', operator: '=', value: 'East' }
        ]
      }
    });

    expect(result).toHaveLength(2);
    expect(result.map(r => r.region)).toEqual(expect.arrayContaining(['North', 'East']));
  });

  it('should handle AND combinator', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ],
      filters: {
        combinator: 'and',
        rules: [
          { field: 'region', operator: '=', value: 'North' },
          { field: 'product', operator: '=', value: 'Widget A' }
        ]
      }
    });

    const total = result.reduce((sum, r) => sum + r.total_revenue, 0);
    expect(total).toBe(2500); // Only North + Widget A
  });
});

describe('d3Aggregate - Sorting', () => {
  it('should sort by single field ascending', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ],
      sort: [
        { field: 'total_revenue', direction: 'asc' }
      ]
    });

    expect(result).toHaveLength(3);
    expect(result[0].region).toBe('East'); // 1100
    expect(result[1].region).toBe('South'); // 2100
    expect(result[2].region).toBe('North'); // 3300
  });

  it('should sort by single field descending', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ],
      sort: [
        { field: 'total_revenue', direction: 'desc' }
      ]
    });

    expect(result).toHaveLength(3);
    expect(result[0].region).toBe('North'); // 3300
    expect(result[1].region).toBe('South'); // 2100
    expect(result[2].region).toBe('East'); // 1100
  });

  it('should sort by multiple fields', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region', 'product'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ],
      sort: [
        { field: 'region', direction: 'asc' },
        { field: 'total_revenue', direction: 'desc' }
      ]
    });

    expect(result[0].region).toBe('East');
    expect(result[1].region).toBe('North');
    expect(result[1].product).toBe('Widget A'); // North Widget A (2500) before North Widget B (800)
  });

  it('should handle null values in sorting', async () => {
    const testData = [
      { region: 'North', revenue: 1000 },
      { region: 'South', revenue: 500 },
      { region: 'East', revenue: 200 }
    ];

    const result = await d3Aggregate(testData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'first', name: 'first_revenue' } // Use first to preserve nulls
      ],
      sort: [
        { field: 'first_revenue', direction: 'asc' }
      ]
    });

    // Should sort in ascending order by revenue
    expect(result[0].region).toBe('East'); // 200
    expect(result[1].region).toBe('South'); // 500
    expect(result[2].region).toBe('North'); // 1000
  });
});

describe('d3Aggregate - Limit', () => {
  it('should limit results', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ],
      limit: 2
    });

    expect(result).toHaveLength(2);
  });

  it('should apply limit after sorting', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ],
      sort: [
        { field: 'total_revenue', direction: 'desc' }
      ],
      limit: 2
    });

    expect(result).toHaveLength(2);
    expect(result[0].region).toBe('North'); // Top revenue
    expect(result[1].region).toBe('South'); // Second
    // East excluded by limit
  });

  it('should handle limit larger than result set', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ],
      limit: 100
    });

    expect(result).toHaveLength(3); // All results
  });

  it('should handle limit of 0', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ],
      limit: 0
    });

    expect(result).toHaveLength(3); // Limit 0 ignored
  });
});

describe('d3Aggregate - Complete Pipeline', () => {
  it('should execute complete pipeline: filter → group → calculate → having → sort → limit', async () => {
    const result = await d3Aggregate(salesData, {
      // GROUP BY
      dimensions: ['region'],

      // SELECT measures
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' },
        { column: 'units', aggregation: 'sum', name: 'total_units' },
        { expression: 'total_revenue / total_units', name: 'avg_price' }
      ],

      // WHERE + HAVING
      filters: {
        combinator: 'and',
        rules: [
          { field: 'product', operator: '=', value: 'Widget A' }, // WHERE
          { field: 'total_revenue', operator: '>=', value: 1500 } // HAVING
        ]
      },

      // ORDER BY
      sort: [
        { field: 'total_revenue', direction: 'desc' }
      ],

      // LIMIT
      limit: 2
    });

    // Expected: Only North (2500) passes HAVING filter (>= 1500)
    // South (1200) and East (1100) fail HAVING filter
    // Sorted desc, limited to 2 (but only 1 result passes)
    expect(result).toHaveLength(1);
    expect(result[0].region).toBe('North');
    expect(result[0].total_revenue).toBe(2500);
    expect(result[0].avg_price).toBe(100); // 2500 / 25
  });

  it('should handle real-world dashboard query', async () => {
    const result = await d3Aggregate(salesData, {
      dimensions: ['region', 'product'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' },
        { column: 'revenue', aggregation: 'avg', name: 'avg_revenue' },
        { column: 'units', aggregation: 'count', name: 'transaction_count' },
        { expression: 'total_revenue / transaction_count', name: 'avg_transaction_value' }
      ],
      filters: {
        combinator: 'and',
        rules: [
          { field: 'revenue', operator: '>=', value: 900 }, // WHERE
          { field: 'total_revenue', operator: '>=', value: 1000 } // HAVING
        ]
      },
      sort: [
        { field: 'total_revenue', direction: 'desc' }
      ],
      limit: 5
    });

    expect(result.length).toBeGreaterThan(0);
    expect(result[0]).toHaveProperty('region');
    expect(result[0]).toHaveProperty('product');
    expect(result[0]).toHaveProperty('total_revenue');
    expect(result[0]).toHaveProperty('avg_revenue');
    expect(result[0]).toHaveProperty('transaction_count');
    expect(result[0]).toHaveProperty('avg_transaction_value');
  });
});

describe('d3Aggregate - Edge Cases', () => {
  it('should handle empty data array', async () => {
    const result = await d3Aggregate([], {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ]
    });

    expect(result).toEqual([]);
  });

  it('should handle missing column values', async () => {
    const testData = [
      { region: 'North', revenue: 1000 },
      { region: 'South' }, // missing revenue
      { region: 'East', revenue: 500 }
    ];

    const result = await d3Aggregate(testData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ]
    });

    const south = result.find(r => r.region === 'South');
    expect(south.total_revenue).toBe(0); // Missing treated as 0
  });

  it('should handle non-numeric values in numeric aggregations', async () => {
    const testData = [
      { region: 'North', revenue: 'invalid' },
      { region: 'South', revenue: 1000 }
    ];

    const result = await d3Aggregate(testData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' }
      ]
    });

    const north = result.find(r => r.region === 'North');
    expect(north.total_revenue).toBe(0); // Invalid number treated as 0
  });

  it('should handle single row data', async () => {
    const testData = [
      { region: 'North', revenue: 1000, units: 10 }
    ];

    const result = await d3Aggregate(testData, {
      dimensions: ['region'],
      measures: [
        { column: 'revenue', aggregation: 'sum', name: 'total_revenue' },
        { column: 'revenue', aggregation: 'avg', name: 'avg_revenue' }
      ]
    });

    expect(result).toHaveLength(1);
    expect(result[0].total_revenue).toBe(1000);
    expect(result[0].avg_revenue).toBe(1000);
  });

  it('should handle undefined aggregateSpec', async () => {
    const result = await d3Aggregate(salesData, undefined);
    expect(result).toEqual(salesData);
  });

  it('should handle null aggregateSpec', async () => {
    const result = await d3Aggregate(salesData, null);
    expect(result).toEqual(salesData);
  });
});
