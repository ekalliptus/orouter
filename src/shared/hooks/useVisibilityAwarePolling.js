"use client";

import { useEffect, useRef } from "react";

/**
 * Runs `callback` on an interval, but pauses while the document is hidden
 * (background tab / minimized window) and resumes immediately when it becomes
 * visible again. This is the single biggest client-side perf win on a
 * dashboard with several always-on pollers: while the user is in another tab,
 * NO polling fires — no fetches, no re-renders, no main-thread contention.
 *
 * On becoming visible again it ticks once immediately so stale data refreshes
 * right away, then resumes the interval.
 *
 * @param {() => void} callback - function to call on each tick
 * @param {number} intervalMs - time between ticks while visible
 * @param {boolean} [enabled=true] - additional gate (e.g. an autoRefresh toggle)
 */
export function useVisibilityAwarePolling(callback, intervalMs, enabled = true) {
  const savedCallback = useRef(callback);
  useEffect(() => {
    savedCallback.current = callback;
  }, [callback]);

  useEffect(() => {
    if (!enabled || typeof window === "undefined") return undefined;

    let intervalId = null;
    const tick = () => savedCallback.current();
    const start = () => {
      if (intervalId == null) {
        // Immediate tick on (re)start so a freshly-focused tab refreshes at once
        // instead of waiting a full interval for its first update.
        tick();
        intervalId = setInterval(tick, intervalMs);
      }
    };
    const stop = () => {
      if (intervalId != null) {
        clearInterval(intervalId);
        intervalId = null;
      }
    };
    const onVisibility = () => {
      if (document.hidden) stop();
      else start();
    };

    if (!document.hidden) start();
    document.addEventListener("visibilitychange", onVisibility);

    return () => {
      document.removeEventListener("visibilitychange", onVisibility);
      stop();
    };
  }, [intervalMs, enabled]);
}
