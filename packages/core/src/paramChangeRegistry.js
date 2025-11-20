/**
 * Parameter Change Registry
 *
 * Coordinates parameter change notifications across charts sharing the same parameter scope.
 * When a parameter value changes, ALL charts using that scope are notified and re-rendered.
 *
 * Flow:
 * 1. Chart constructor calls extractParamReferences(spec) to find param dependencies
 * 2. Chart subscribes to each param scope it depends on (e.g., 'dashboard_filters')
 * 3. User changes param value in UI → DefaultParamsRenderer calls registry.setParamValue()
 * 4. Registry compares old vs new value → only notifies if value actually changed
 * 5. Registry notifies ALL charts subscribed to that scope → they call rerender()
 *
 * Loop Prevention:
 * - Registry only notifies if value ACTUALLY changed (old !== new)
 * - Applications should compare URL params before updating URL
 * - This prevents: param change → URL update → URL read → param change → loop
 */

export class ParamChangeRegistry {
  constructor() {
    // Map of scope name -> Set<Chart>
    // e.g., 'dashboard_filters' -> Set([chart1, chart2, chart3])
    this.scopes = new Map();
  }

  /**
   * Subscribe a Chart instance to parameter change notifications for a scope
   * @param {string} scopeName - Name of the params block (e.g., 'dashboard_filters')
   * @param {Chart} chart - Chart instance to subscribe
   */
  subscribe(scopeName, chart) {
    if (!scopeName || typeof scopeName !== 'string') {
      console.warn('[ParamChangeRegistry] Invalid scope name:', scopeName);
      return;
    }

    if (!this.scopes.has(scopeName)) {
      this.scopes.set(scopeName, new Set());
    }

    const subscribers = this.scopes.get(scopeName);
    subscribers.add(chart);

    console.log(`[ParamChangeRegistry] Chart subscribed to scope "${scopeName}". Total subscribers: ${subscribers.size}`);
  }

  /**
   * Unsubscribe a Chart instance from parameter change notifications
   * @param {string} scopeName - Name of the params block
   * @param {Chart} chart - Chart instance to unsubscribe
   */
  unsubscribe(scopeName, chart) {
    const subscribers = this.scopes.get(scopeName);
    if (subscribers) {
      subscribers.delete(chart);
      console.log(`[ParamChangeRegistry] Chart unsubscribed from scope "${scopeName}". Remaining subscribers: ${subscribers.size}`);

      // Clean up if no subscribers left
      if (subscribers.size === 0) {
        this.scopes.delete(scopeName);
        console.log(`[ParamChangeRegistry] Scope "${scopeName}" removed (no subscribers)`);
      }
    }
  }

  /**
   * Notify all charts subscribed to a scope that a parameter changed
   * Called by registry.setParamValue() after value comparison
   *
   * @param {string} scopeName - Name of the params block
   * @param {string} paramId - Parameter ID that changed
   * @param {*} newValue - New parameter value
   */
  notifyChange(scopeName, paramId, newValue) {
    const subscribers = this.scopes.get(scopeName);

    if (!subscribers || subscribers.size === 0) {
      console.log(`[ParamChangeRegistry] No subscribers for scope "${scopeName}", skipping notification`);
      return;
    }

    console.log(`[ParamChangeRegistry] Param "${paramId}" changed in scope "${scopeName}". Notifying ${subscribers.size} subscribers.`);

    // Re-render all charts that depend on this scope
    for (const chart of subscribers) {
      if (chart.rerender) {
        console.log(`[ParamChangeRegistry] Triggering rerender for chart`);
        chart.rerender().catch(error => {
          console.error('[ParamChangeRegistry] Chart rerender failed:', error);
        });
      }
    }
  }

  /**
   * Get subscriber count for a scope (useful for debugging)
   * @param {string} scopeName - Name of the params block
   * @returns {number} Number of subscribed charts
   */
  getSubscriberCount(scopeName) {
    const subscribers = this.scopes.get(scopeName);
    return subscribers ? subscribers.size : 0;
  }

  /**
   * Get all registered scopes (useful for debugging)
   * @returns {string[]} Array of scope names
   */
  getScopes() {
    return Array.from(this.scopes.keys());
  }
}
