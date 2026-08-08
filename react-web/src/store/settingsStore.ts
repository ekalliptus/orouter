// Ported from src/store/settingsStore.js. TTL cache (60s) + PATCH merge.
// `settings` is loosely typed (Record<string, unknown>) for now; M3 will
// narrow it once the Rust backend's /api/settings shape is finalized.
import { create } from "zustand";

const CLIENT_STORE_TTL_MS = 60_000;
export type Settings = Record<string, unknown>;

interface SettingsState {
  settings: Settings | null;
  loading: boolean;
  error: string | null;
  lastFetched: number;
  invalidate: () => void;
  fetchSettings: (opts?: { force?: boolean }) => Promise<Settings | null>;
  patchSettings: (patch: Partial<Settings>) => Promise<Settings | null>;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: null,
  loading: false,
  error: null,
  lastFetched: 0,

  invalidate: () => set({ lastFetched: 0 }),

  fetchSettings: async ({ force = false } = {}) => {
    const { lastFetched, settings } = get();
    if (!force && settings && Date.now() - lastFetched < CLIENT_STORE_TTL_MS) return settings;
    set({ loading: true, error: null });
    try {
      const res = await fetch("/api/settings", { credentials: "include" });
      const data = (await res.json()) as Settings | { error?: string };
      if (res.ok) {
        set({ settings: data as Settings, loading: false, lastFetched: Date.now() });
        return data as Settings;
      }
      set({ error: (data as { error?: string }).error ?? "Failed to fetch settings", loading: false });
    } catch {
      set({ error: "Failed to fetch settings", loading: false });
    }
    return null;
  },

  patchSettings: async (patch) => {
    try {
      const res = await fetch("/api/settings", {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify(patch),
      });
      if (!res.ok) return null;
      const updated = (await res.json()) as Settings;
      set({ settings: updated, lastFetched: Date.now() });
      return updated;
    } catch {
      return null;
    }
  },
}));
