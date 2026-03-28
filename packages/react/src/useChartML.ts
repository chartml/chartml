import { useEffect, useRef, useState } from 'react';
import { ChartML } from '@chartml/core';

type ConfigureFn = (chartml: ChartML) => void;

/**
 * Hook to create and configure a ChartML instance.
 * Initializes WASM on first render. Returns null while loading.
 *
 * @param configure - Optional callback to register plugins on the instance
 * @param onError - Called if WASM initialization fails
 */
export function useChartML(configure?: ConfigureFn, onError?: (error: Error) => void): ChartML | null {
  const [instance, setInstance] = useState<ChartML | null>(null);
  // Stable refs for callbacks so they don't trigger re-init on every render
  const configureRef = useRef(configure);
  configureRef.current = configure;
  const onErrorRef = useRef(onError);
  onErrorRef.current = onError;

  useEffect(() => {
    let cancelled = false;
    ChartML.create()
      .then((chartml) => {
        if (!cancelled) {
          configureRef.current?.(chartml);
          setInstance(chartml);
        }
      })
      .catch((e) => {
        if (!cancelled) {
          onErrorRef.current?.(e instanceof Error ? e : new Error(String(e)));
        }
      });
    return () => { cancelled = true; };
  }, []);

  return instance;
}
