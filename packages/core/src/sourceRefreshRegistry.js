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

    console.log(`[SourceRefreshRegistry] Chart subscribed to source "${sourceName}". Total subscribers: ${source.subscribers.size}`);
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
      console.log(`[SourceRefreshRegistry] Chart unsubscribed from source "${sourceName}". Remaining subscribers: ${source.subscribers.size}`);

      // Clean up if no subscribers left
      if (source.subscribers.size === 0) {
        this.sources.delete(sourceName);
        console.log(`[SourceRefreshRegistry] Source "${sourceName}" removed (no subscribers)`);
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
      console.log(`[SourceRefreshRegistry] No subscribers for source "${sourceName}", executing refresh directly`);
      await refreshCallback();
      return;
    }

    console.log(`[SourceRefreshRegistry] Refreshing source "${sourceName}". Notifying ${source.subscribers.size} subscribers.`);

    try {
      // Mark source as refreshing
      source.isRefreshing = true;

      // STEP 1: Notify ALL subscribers that refresh is starting
      console.log(`[SourceRefreshRegistry] Broadcasting REFRESH_START to ${source.subscribers.size} subscribers`);
      for (const chart of source.subscribers) {
        if (chart.onRefreshStateChange) {
          chart.onRefreshStateChange(true);
        }
      }

      // STEP 2: Execute the refresh callback (only from the initiating chart)
      // Middleware will deduplicate if multiple sources happen to refresh simultaneously
      console.log(`[SourceRefreshRegistry] Executing refresh callback`);
      await refreshCallback();

      // STEP 3: Update shared timestamp
      source.lastFetched = Date.now();
      console.log(`[SourceRefreshRegistry] Refresh complete. Timestamp: ${source.lastFetched}`);

    } finally {
      // STEP 4: Notify ALL subscribers that refresh is complete
      source.isRefreshing = false;

      console.log(`[SourceRefreshRegistry] Broadcasting REFRESH_COMPLETE to ${source.subscribers.size} subscribers`);
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
          console.log(`[SourceRefreshRegistry] Triggering rerender for non-initiating chart`);
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
