/**
 * Source Refresh Registry
 *
 * Coordinates refresh notifications across charts sharing the same data source.
 * When one chart refreshes a source, ALL charts using that source are notified.
 *
 * Flow:
 * 1. Chart calls refresh() with sourceName
 * 2. Registry notifies ALL subscribers: "refresh starting" (they show spinners)
 * 3. Registry executes the refresh callback (only the initiating chart fetches)
 * 4. Registry notifies ALL subscribers: "refresh complete" (they stop spinners, can re-render)
 * 5. Middleware deduplicates if multiple charts happen to call at same time
 */

export class SourceRefreshRegistry {
  constructor() {
    // Map of source name -> { subscribers: Set<Chart>, isRefreshing: boolean, lastFetched: number }
    this.sources = new Map();
  }

  /**
   * Subscribe a Chart instance to source refresh notifications
   * @param {string} sourceName - Name of the data source (e.g., 'search_trends')
   * @param {Chart} chart - Chart instance to subscribe
   */
  subscribe(sourceName, chart) {
    if (!this.sources.has(sourceName)) {
      this.sources.set(sourceName, {
        subscribers: new Set(),
        isRefreshing: false,
        lastFetched: null
      });
    }

    const source = this.sources.get(sourceName);
    source.subscribers.add(chart);
  }

  /**
   * Unsubscribe a Chart instance from source refresh notifications
   * @param {string} sourceName - Name of the data source
   * @param {Chart} chart - Chart instance to unsubscribe
   */
  unsubscribe(sourceName, chart) {
    const source = this.sources.get(sourceName);
    if (source) {
      source.subscribers.delete(chart);

      // Clean up if no subscribers left
      if (source.subscribers.size === 0) {
        this.sources.delete(sourceName);
      }
    }
  }

  /**
   * Refresh a source - coordinates notifications to all subscribers
   * @param {string} sourceName - Name of the data source
   * @param {Function} refreshCallback - Async function that does the actual refresh (from initiating chart)
   * @param {Chart} initiatingChart - The chart that triggered the refresh (to skip re-rendering it)
   * @returns {Promise<void>}
   */
  async refreshSource(sourceName, refreshCallback, initiatingChart = null) {
    const source = this.sources.get(sourceName);
    if (!source) {
      // No subscribers - just execute the callback
      await refreshCallback();
      return;
    }

    try {
      // Mark source as refreshing
      source.isRefreshing = true;

      // STEP 1: Notify ALL subscribers that refresh is starting
      for (const chart of source.subscribers) {
        if (chart.onRefreshStateChange) {
          chart.onRefreshStateChange(true);
        }
      }

      // STEP 2: Execute the refresh callback (only from the initiating chart)
      // Middleware will deduplicate if multiple sources happen to refresh simultaneously
      await refreshCallback();

      // STEP 3: Update shared timestamp
      source.lastFetched = Date.now();

    } finally {
      // STEP 4: Notify ALL subscribers that refresh is complete
      source.isRefreshing = false;

      for (const chart of source.subscribers) {
        // Update timestamp in metadata
        if (chart.metadata) {
          chart.metadata.last_updated = source.lastFetched;
        }

        // Stop spinner
        if (chart.onRefreshStateChange) {
          chart.onRefreshStateChange(false);
        }

        // Re-render OTHER charts to show fresh data (skip initiating chart - it already rendered)
        if (chart !== initiatingChart && chart.rerender) {
          chart.rerender().catch(error => {
            console.error('[SourceRefreshRegistry] Chart rerender failed:', error);
          });
        }
      }
    }
  }

  /**
   * Get the last fetched timestamp for a source
   * @param {string} sourceName - Name of the data source
   * @returns {number|null} Timestamp in milliseconds, or null if never fetched
   */
  getLastFetched(sourceName) {
    const source = this.sources.get(sourceName);
    return source?.lastFetched || null;
  }

  /**
   * Check if a source is currently refreshing
   * @param {string} sourceName - Name of the data source
   * @returns {boolean}
   */
  isRefreshing(sourceName) {
    const source = this.sources.get(sourceName);
    return source?.isRefreshing || false;
  }
}
