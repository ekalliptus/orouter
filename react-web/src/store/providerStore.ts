// Ported from src/store/providerStore.js. A provider connection row from the
// shared SQLite store. TTL-cached list (60s) + local CRUD helpers so the UI
// can optimistic-update before the refetch lands.
import { create } from "zustand";

const CLIENT_STORE_TTL_MS = 60_000;

export interface ProviderConnection {
  id?: string;
  _id: string;
  provider: string;
  name?: string;
  authType?: string;
  isActive?: boolean;
  [key: string]: unknown;
}

interface ProviderState {
  providers: ProviderConnection[];
  loading: boolean;
  error: string | null;
  lastFetched: number;
  setProviders: (providers: ProviderConnection[]) => void;
  addProvider: (provider: ProviderConnection) => void;
  updateProvider: (id: string, updates: Partial<ProviderConnection>) => void;
  removeProvider: (id: string) => void;
  invalidate: () => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  fetchProviders: (opts?: { force?: boolean }) => Promise<void>;
  createProvider: (input: { provider: string; apiKey: string; name: string }) => Promise<ProviderConnection | null>;
  deleteProvider: (id: string) => Promise<boolean>;
  testProvider: (id: string) => Promise<{ valid: boolean; error: string | null } | null>;
}

export const useProviderStore = create<ProviderState>((set, get) => ({
  providers: [],
  loading: false,
  error: null,
  lastFetched: 0,

  setProviders: (providers) => set({ providers, lastFetched: Date.now() }),
  addProvider: (provider) => set((s) => ({ providers: [provider, ...s.providers] })),
  updateProvider: (id, updates) =>
    set((s) => ({ providers: s.providers.map((p) => (p._id === id ? { ...p, ...updates } : p)) })),
  removeProvider: (id) => set((s) => ({ providers: s.providers.filter((p) => p._id !== id) })),
  invalidate: () => set({ lastFetched: 0 }),
  setLoading: (loading) => set({ loading }),
  setError: (error) => set({ error }),

  fetchProviders: async ({ force = false } = {}) => {
    const { lastFetched, providers } = get();
    if (!force && providers.length > 0 && Date.now() - lastFetched < CLIENT_STORE_TTL_MS) return;
    set({ loading: true, error: null });
    try {
      const res = await fetch("/api/providers", { credentials: "include" });
      const data = (await res.json()) as { connections?: ProviderConnection[]; providers?: ProviderConnection[]; error?: string };
      if (res.ok) {
        const rows = data.connections ?? data.providers ?? [];
        const providers = rows.map((p) => ({ ...p, _id: p.id ?? p._id }));
        set({ providers, loading: false, lastFetched: Date.now() });
      } else {
        set({ error: data.error ?? "Failed to fetch providers", loading: false });
      }
    } catch {
      set({ error: "Failed to fetch providers", loading: false });
    }
  },

  createProvider: async (input) => {
    try {
      const res = await fetch("/api/providers", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify(input),
      });
      const data = (await res.json()) as { connection?: ProviderConnection; error?: string };
      if (!res.ok) return null;
      const conn = data.connection!;
      // The backend returns id/provider but the store keys on _id — normalize.
      const normalized = { ...conn, _id: (conn as ProviderConnection & { id?: string }).id ?? conn._id };
      set((s) => ({ providers: [normalized, ...s.providers] }));
      return normalized;
    } catch {
      return null;
    }
  },

  deleteProvider: async (id) => {
    try {
      const res = await fetch(`/api/providers/${id}`, { method: "DELETE", credentials: "include" });
      if (!res.ok) return false;
      set((s) => ({ providers: s.providers.filter((p) => p._id !== id) }));
      return true;
    } catch {
      return false;
    }
  },

  testProvider: async (id) => {
    try {
      const res = await fetch(`/api/providers/${id}/test`, { method: "POST", credentials: "include" });
      const data = (await res.json()) as { valid?: boolean; error?: string | null };
      if (!res.ok) return null;
      // Refresh provider list to pick up the written-back testStatus.
      get().fetchProviders({ force: true });
      return { valid: !!data.valid, error: data.error ?? null };
    } catch {
      return null;
    }
  },
}));
