/**
 * @chartml/datafusion
 *
 * Optional DataFusion SQL/aggregate/forecast transform plugin for ChartML.
 *
 * Usage:
 *   import { ChartML } from '@chartml/core';
 *   import { createDataFusionTransform } from '@chartml/datafusion';
 *
 *   const chartml = await ChartML.create();
 *   const dfTransform = await createDataFusionTransform();
 *   chartml.registerTransform(dfTransform);
 */

let initPromise: Promise<any> | null = null;

/**
 * Load and initialize the DataFusion WASM module.
 * Returns a transform function compatible with ChartML.registerTransform().
 */
export async function createDataFusionTransform(): Promise<
  (rows: Record<string, unknown>[], spec: Record<string, unknown>, context: Record<string, unknown>) => Promise<{ data: Record<string, unknown>[]; metadata: Record<string, unknown> }>
> {
  if (!initPromise) {
    initPromise = import('../pkg/chartml-datafusion.js').then(async (m) => {
      await m.default();
      return m;
    });
  }
  const mod = await initPromise;

  // Return a transform function that calls the WASM module.
  // Context is forwarded for interface consistency. For DataFusion specifically,
  // param substitution happens upstream (in YAML before parsing), so context
  // is typically empty. Custom JS transforms may use it for runtime params.
  return async (rows, spec, context) => {
    const result = await mod.transform(rows, spec, context);
    return result;
  };
}
