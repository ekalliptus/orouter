// Auth state + route guard. The backend tells us auth status via
// GET /api/auth/status (reads the httpOnly cookie we can't see in JS).
// useAuth() caches the result in module state so multiple components don't
// each fire a status probe.
import { useEffect, useState, useCallback } from "react";

let cached: boolean | null = null;
let inflight: Promise<boolean> | null = null;

export async function fetchAuthStatus(force = false): Promise<boolean> {
  if (cached !== null && !force) return cached;
  if (inflight && !force) return inflight;
  inflight = (async () => {
    try {
      const res = await fetch("/api/auth/status", { credentials: "include" });
      const data = (await res.json()) as { authenticated?: boolean };
      cached = !!data.authenticated;
    } catch {
      cached = false;
    }
    inflight = null;
    return cached;
  })();
  return inflight;
}

export function logout(): Promise<void> {
  cached = false;
  return fetch("/api/auth/logout", { method: "POST", credentials: "include" }).then(() => undefined).catch(() => undefined);
}

export function useAuth() {
  const [authed, setAuthed] = useState<boolean | null>(cached);
  useEffect(() => {
    let active = true;
    fetchAuthStatus().then((a) => {
      if (active) setAuthed(a);
    });
    return () => {
      active = false;
    };
  }, []);
  const refresh = useCallback(() => fetchAuthStatus(true).then((a) => setAuthed(a)), []);
  return { authed, refresh };
}
