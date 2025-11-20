/**
 * Global Plugin Registry for ChartML
 *
 * Allows plugins to auto-register themselves when imported.
 * This eliminates the need for manual registration.
 */

class GlobalPluginRegistry {
  constructor() {
    this.chartRenderers = new Map();
    this.dataSources = new Map();
    this.aggregateMiddleware = [];
  }

  registerChartRenderer(type, renderer) {
    if (this.chartRenderers.has(type)) {
      console.warn(
        `⚠️  ChartML: Renderer "${type}" is already registered and will be overwritten.\n` +
        `   Consider using a namespaced type (e.g., "@yourorg/${type}") to avoid conflicts.`
      );
    }
    this.chartRenderers.set(type, renderer);
  }

  registerDataSource(name, handler) {
    this.dataSources.set(name, handler);
  }

  registerAggregateMiddleware(middleware) {
    this.aggregateMiddleware.push(middleware);
  }

  getChartRenderer(type) {
    return this.chartRenderers.get(type);
  }

  getDataSource(name) {
    return this.dataSources.get(name);
  }

  getAllChartRenderers() {
    return this.chartRenderers;
  }

  getAllDataSources() {
    return this.dataSources;
  }

  getAllAggregateMiddleware() {
    return this.aggregateMiddleware;
  }
}

// Create singleton instance
const globalRegistry = new GlobalPluginRegistry();

export { globalRegistry };
