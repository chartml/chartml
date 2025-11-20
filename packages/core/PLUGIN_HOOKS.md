# ChartML Plugin Hook System

## Overview

ChartML Core v1.0+ includes a powerful hook system that allows plugins to emit events for:
- Progress tracking during data loading
- Cache hit/miss notifications
- Error handling
- Custom coordination between tabs

This enables advanced implementations like:
- **Real-time progress bars** during large dataset streaming
- **Cache analytics** and optimization
- **Multi-tab coordination** for shared resources
- **Custom error recovery** strategies

## Hook API

### Available Hooks

```javascript
const chartml = new ChartML({
  onProgress: (event) => {
    // Called during data loading/streaming
    console.log(`Progress: ${event.loaded}/${event.total} (${event.percent}%)`);
  },

  onCacheHit: (event) => {
    // Called when data is served from cache
    console.log(`Cache hit: ${event.cacheKey}, age: ${event.age}ms`);
  },

  onCacheMiss: (event) => {
    // Called when cache miss requires fresh data fetch
    console.log(`Cache miss: ${event.cacheKey}, fetching fresh data...`);
  },

  onError: (error) => {
    // Called when errors occur during rendering
    console.error(`ChartML error:`, error);
  }
});
```

### Hook Event Objects

#### onProgress Event
```javascript
{
  type: 'progress',
  phase: 'data' | 'aggregate' | 'render',
  loaded: number,      // Bytes or rows loaded
  total: number,       // Total bytes or rows
  percent: number,     // 0-100
  message: string      // Human-readable message
}
```

#### onCacheHit Event
```javascript
{
  type: 'cache_hit',
  cacheKey: string,    // Cache identifier
  age: number,         // Cache age in milliseconds
  size: number,        // Cached data size
  source: string       // Cache source (e.g., 'opfs', 'memory')
}
```

#### onCacheMiss Event
```javascript
{
  type: 'cache_miss',
  cacheKey: string,    // Cache identifier
  reason: string       // Why cache missed ('not_found', 'expired', 'invalidated')
}
```

## Plugin Implementation

### Data Source Plugins

Plugins receive hooks via the second parameter:

```javascript
export function createStreamingAPIPlugin(options) {
  return async function (spec, { hooks }) {
    const { onProgress, onCacheHit, onCacheMiss } = hooks;

    // Check cache
    const cached = await checkCache(spec.url);
    if (cached) {
      onCacheHit?.({
        type: 'cache_hit',
        cacheKey: spec.url,
        age: Date.now() - cached.timestamp,
        size: cached.data.length
      });
      return cached.data;
    }

    // Cache miss
    onCacheMiss?.({
      type: 'cache_miss',
      cacheKey: spec.url,
      reason: 'not_found'
    });

    // Fetch with progress
    const response = await fetch(spec.url, {
      method: 'GET',
      headers: options.headers || {}
    });

    const reader = response.body.getReader();
    const contentLength = +response.headers.get('Content-Length');

    let loaded = 0;
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      loaded += value.length;
      onProgress?.({
        type: 'progress',
        phase: 'data',
        loaded,
        total: contentLength,
        percent: (loaded / contentLength) * 100,
        message: `Downloading data: ${formatBytes(loaded)}`
      });
    }

    return parseData(chunks);
  };
}
```

### Aggregation Middleware Plugins

Middleware also receives hooks:

```javascript
export function createAdvancedDuckDBMiddleware(options) {
  return async function (data, aggregateSpec, { hooks }) {
    const { onProgress } = hooks;

    // Report progress during aggregation
    onProgress?.({
      type: 'progress',
      phase: 'aggregate',
      loaded: 0,
      total: data.length,
      percent: 0,
      message: 'Starting aggregation...'
    });

    // ... perform aggregation

    onProgress?.({
      type: 'progress',
      phase: 'aggregate',
      loaded: data.length,
      total: data.length,
      percent: 100,
      message: 'Aggregation complete'
    });

    return result;
  };
}
```

## UI Integration Examples

### React Progress Bar

```jsx
import { ChartML } from '@chartml/core';
import { useState } from 'react';

function ChartWithProgress({ spec }) {
  const [progress, setProgress] = useState(0);
  const [message, setMessage] = useState('');

  const chartml = new ChartML({
    onProgress: (event) => {
      setProgress(event.percent);
      setMessage(event.message);
    }
  });

  return (
    <div>
      {progress > 0 && progress < 100 && (
        <div className="progress-bar">
          <div style={{ width: `${progress}%` }} />
          <span>{message}</span>
        </div>
      )}
      <div ref={ref => chartml.render(spec, ref)} />
    </div>
  );
}
```

### Cache Analytics Dashboard

```javascript
const cacheStats = {
  hits: 0,
  misses: 0,
  totalAge: 0
};

const chartml = new ChartML({
  onCacheHit: (event) => {
    cacheStats.hits++;
    cacheStats.totalAge += event.age;
    updateDashboard();
  },

  onCacheMiss: (event) => {
    cacheStats.misses++;
    updateDashboard();
  }
});

function getCacheHitRate() {
  const total = cacheStats.hits + cacheStats.misses;
  return total > 0 ? (cacheStats.hits / total) * 100 : 0;
}
```

## Advanced: Multi-Tab Coordination

Example of using hooks for cross-tab coordination:

```javascript
// In your advanced plugin
export function createCoordinatedDuckDBPlugin(options) {
  const coordinator = new SharedWorker('/coordinator.js');

  return async function (data, aggregateSpec, { hooks }) {
    const { onCacheHit, onCacheMiss } = hooks;

    // Check if another tab is already processing this query
    const lockAcquired = await coordinator.tryAcquireLock(queryId);

    if (!lockAcquired) {
      // Another tab is processing, wait for their result
      onCacheHit?.({
        type: 'cache_hit',
        cacheKey: queryId,
        source: 'coordinator',
        message: 'Using result from another tab'
      });

      return await coordinator.waitForResult(queryId);
    }

    // We got the lock, process the query
    onCacheMiss?.({
      type: 'cache_miss',
      cacheKey: queryId,
      reason: 'not_processed'
    });

    const result = await processWith DuckDB(data, aggregateSpec);

    // Broadcast result to other tabs
    coordinator.broadcastResult(queryId, result);

    return result;
  };
}
```

## Best Practices

### 1. Always Check Hook Existence

Hooks are optional, always check before calling:

```javascript
// ✅ CORRECT
onProgress?.({ ... });

// ❌ WRONG - will throw if hook not registered
onProgress({ ... });
```

### 2. Don't Block on Hooks

Hooks should be fire-and-forget:

```javascript
// ✅ CORRECT - async but don't await
onProgress?.({ type: 'progress', ... });

// ❌ WRONG - blocking
await onProgress?.({ type: 'progress', ... });
```

### 3. Provide Meaningful Messages

Help users understand what's happening:

```javascript
// ✅ CORRECT
onProgress?.({
  phase: 'data',
  percent: 45,
  message: 'Downloading data from API (2.3 MB / 5.1 MB)'
});

// ❌ WRONG - not helpful
onProgress?.({
  phase: 'data',
  percent: 45,
  message: 'Loading...'
});
```

### 4. Handle Hook Errors Gracefully

Core handles hook errors, but don't let them break your plugin:

```javascript
try {
  const data = await fetchData();
  onProgress?.({ ... });
  return data;
} catch (error) {
  // Plugin error, not hook error
  throw error;
}
```

## Conclusion

The hook system makes ChartML extensible for advanced use cases while keeping the core simple. Basic plugins can ignore hooks entirely, while advanced implementations can leverage them for:

- Better UX (progress bars, status messages)
- Performance optimization (cache analytics)
- Resource coordination (multi-tab sync)
- Custom error handling

The system is **opt-in** - plugins work without hooks, but become more powerful with them.
