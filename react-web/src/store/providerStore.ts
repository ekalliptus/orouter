// Ported from src/store/providerStore.js. A provider connection row from the
// shared SQLite store. TTL-cached list (60s) + local CRUD helpers so the UI
// can optimistic-update before the refetch lands.
import { create } from "zustand";

const CLIENT_STORE_TTL_MS = 60_000;

export interface ProviderConnection {
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
        set({ providers: data.connections ?? data.providers ?? [], loading: false, lastFetched: Date.now() });
      } else {
        set({ error: data.error ?? "Failed to fetch providers", loading: false });
      }
    } catch {
      set({ error: "Failed to fetch providers", loading: false });
    }
  },
}));
