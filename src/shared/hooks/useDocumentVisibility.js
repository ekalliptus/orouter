"use client";

import { useEffect, useState } from "react";

/**
 * Tracks whether the document (browser tab/window) is currently visible.
 *
 * Components that poll or run periodic work can read this to pause while the
 * dashboard is in a background tab — avoiding wasted network + main-thread
 * contention when nobody is looking. React does not pause setInterval/timers
 * on hidden tabs, so this guard is what stops N pollers from running while the
 * user is in another tab.
 *
 * @returns {boolean} true when the document is visible (tab focused & in foreground)
 */
export function useDocumentVisibility() {
  const [visible, setVisible] = useState(() =>
    typeof document === "undefined" ? true : !document.hidden
  );

  useEffect(() => {
    if (typeof document === "undefined") return undefined;
    const onChange = () => setVisible(!document.hidden);
    document.addEventListener("visibilitychange", onChange);
    return () => document.removeEventListener("visibilitychange", onChange);
  }, []);

  return visible;
}
