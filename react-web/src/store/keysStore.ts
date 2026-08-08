// API keys store. Mirrors the shape of providerStore: TTL-cached list + local
// optimistic helpers. The backend serves GET/POST/DELETE /api/keys.
import { create } from "zustand";

const CLIENT_STORE_TTL_MS = 60_000;

export interface ApiKey {
  id: string;
  key: string;
  name: string;
  machineId?: string;
  isActive?: boolean;
  createdAt: string;
}

interface KeysState {
  keys: ApiKey[];
  loading: boolean;
  error: string | null;
  lastFetched: number;
  fetchKeys: (opts?: { force?: boolean }) => Promise<void>;
  createKey: (name: string) => Promise<ApiKey | null>;
  deleteKey: (id: string) => Promise<boolean>;
}

export const useKeysStore = create<KeysState>((set, get) => ({
  keys: [],
  loading: false,
  error: null,
  lastFetched: 0,

  fetchKeys: async ({ force = false } = {}) => {
    const { lastFetched, keys } = get();
    if (!force && keys.length > 0 && Date.now() - lastFetched < CLIENT_STORE_TTL_MS) return;
    set({ loading: true, error: null });
    try {
      const res = await fetch("/api/keys", { credentials: "include" });
      const data = (await res.json()) as { keys?: ApiKey[]; error?: string };
      if (res.ok) {
        set({ keys: data.keys ?? [], loading: false, lastFetched: Date.now() });
      } else {
        set({ error: data.error ?? "Failed to fetch keys", loading: false });
      }
    } catch {
      set({ error: "Failed to fetch keys", loading: false });
    }
  },

  createKey: async (name) => {
    try {
      const res = await fetch("/api/keys", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({ name }),
      });
      const data = (await res.json()) as ApiKey & { error?: string };
      if (!res.ok) return null;
      set((s) => ({ keys: [data, ...s.keys] }));
      return data;
    } catch {
      return null;
    }
  },

  deleteKey: async (id) => {
    try {
      const res = await fetch(`/api/keys/${id}`, { method: "DELETE", credentials: "include" });
      if (!res.ok) return false;
      set((s) => ({ keys: s.keys.filter((k) => k.id !== id) }));
      return true;
    } catch {
      return false;
    }
  },
}));
