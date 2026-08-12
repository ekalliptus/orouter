"use client";

import { create } from "zustand";
import { CLIENT_STORE_TTL_MS } from "@/shared/constants/config";

const useProviderStore = create((set, get) => ({
  providers: [],
  providerNodes: [],
  loading: false,
  error: null,
  lastFetched: 0,
  lastFetchedNodes: 0,

  setProviders: (providers) => set({ providers, lastFetched: Date.now() }),

  addProvider: (provider) =>
    set((state) => ({ providers: [provider, ...state.providers] })),

  updateProvider: (id, updates) =>
    set((state) => ({
      providers: state.providers.map((p) =>
        p._id === id ? { ...p, ...updates } : p
      ),
    })),

  removeProvider: (id) =>
    set((state) => ({
      providers: state.providers.filter((p) => p._id !== id),
    })),

  invalidate: () => set({ lastFetched: 0 }),

  setLoading: (loading) => set({ loading }),

  setError: (error) => set({ error }),

  // Skips network when cache is fresh (< CLIENT_STORE_TTL_MS). Pass {force:true} to override.
  // Returns the connections array so callers (e.g. UsageStats topology) can use it directly.
  fetchProviders: async ({ force = false } = {}) => {
    const { lastFetched, providers } = get();
    if (!force && providers.length > 0 && Date.now() - lastFetched < CLIENT_STORE_TTL_MS) return providers;
    set({ loading: true, error: null });
    try {
      const response = await fetch("/api/providers");
      const data = await response.json();
      if (response.ok) {
        const connections = data.connections || data.providers || [];
        set({ providers: connections, loading: false, lastFetched: Date.now() });
        return connections;
      } else {
        set({ error: data.error, loading: false });
        return providers;
      }
    } catch (error) {
      set({ error: "Failed to fetch providers", loading: false });
      return providers;
    }
  },

  // Fetch OpenAI/Anthropic-compatible provider nodes (used by Usage + ModelSelect).
  fetchProviderNodes: async ({ force = false } = {}) => {
    const { lastFetchedNodes, providerNodes } = get();
    if (!force && providerNodes && providerNodes.length > 0 && Date.now() - (lastFetchedNodes || 0) < CLIENT_STORE_TTL_MS) return providerNodes;
    try {
      const response = await fetch("/api/provider-nodes");
      const data = await response.json();
      const nodes = data.nodes || data || [];
      set({ providerNodes: nodes, lastFetchedNodes: Date.now() });
      return nodes;
    } catch (error) {
      return [];
    }
  },
}));

export default useProviderStore;

