import { useEffect, useRef, useCallback } from 'react';
import type { BrowserScopeKey } from '../../../shared/types';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown>; on: (channel: string, cb: (...args: any[]) => void) => () => void } }).mergenApi;

export function useBrowserSync(scope: BrowserScopeKey, containerRef: React.RefObject<HTMLElement>) {
  const syncBounds = useCallback(() => {
    if (!containerRef.current) return;
    const rect = containerRef.current.getBoundingClientRect();
    api.invoke('browser:syncBounds', {
      scope,
      x: Math.round(rect.x),
      y: Math.round(rect.y),
      width: Math.round(rect.width),
      height: Math.round(rect.height),
    });
  }, [scope, containerRef]);

  useEffect(() => {
    if (!containerRef.current) return;
    syncBounds();
    api.invoke('browser:show', scope);
    const ro = new ResizeObserver(() => syncBounds());
    ro.observe(containerRef.current);
    return () => {
      ro.disconnect();
      api.invoke('browser:hide', scope);
    };
  }, [scope, syncBounds, containerRef]);

  return { syncBounds };
}
