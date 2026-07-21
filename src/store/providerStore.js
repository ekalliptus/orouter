"use client";

import { create } from "zustand";
import { CLIENT_STORE_TTL_MS } from "@/shared/constants/config";

// Module-level in-flight promises so concurrent callers share a single request.
let providersInFlight = null;
let nodesInFlight = null;

// Connections historically use `_id` while some normalized views expose `id`.
const providerId = (p) => (p && (p.id ?? p._id)) ?? p?._id;

const useProviderStore = create((set, get) => ({
  providers: [],
  providerNodes: [],
  loading: false,
  nodesLoading: false,
  error: null,
  nodesError: null,
  lastFetched: 0,
  nodesLastFetched: 0,

  setProviders: (providers) => set({ providers, lastFetched: Date.now() }),

  setProviderNodes: (providerNodes) =>
    set({ providerNodes, nodesLastFetched: Date.now() }),

  addProvider: (provider) =>
    set((state) => ({ providers: [provider, ...state.providers] })),

  updateProvider: (id, updates) =>
    set((state) => ({
      providers: state.providers.map((p) =>
        providerId(p) === id ? { ...p, ...updates } : p
      ),
    })),

  removeProvider: (id) =>
    set((state) => ({
      providers: state.providers.filter((p) => providerId(p) !== id),
    })),

  invalidate: () => set({ lastFetched: 0 }),
  invalidateNodes: () => set({ nodesLastFetched: 0 }),

  setLoading: (loading) => set({ loading }),
  setError: (error) => set({ error }),
  setNodesLoading: (nodesLoading) => set({ nodesLoading }),
  setNodesError: (nodesError) => set({ nodesError }),

  // Skips network when cache is fresh (< CLIENT_STORE_TTL_MS). Pass {force:true} to override.
  // Returns the current/fetched provider list so callers can share a single request.
  fetchProviders: async ({ force = false } = {}) => {
    const { lastFetched, providers } = get();
    if (!force && providers.length > 0 && Date.now() - lastFetched < CLIENT_STORE_TTL_MS) {
      return providers;
    }
    if (providersInFlight) return providersInFlight;

    set({ loading: true, error: null });
    providersInFlight = (async () => {
      try {
        const response = await fetch("/api/providers");
        const data = await response.json();
        const next = data?.connections || data?.providers || [];
        if (response.ok) {
          set({ providers: next, loading: false, lastFetched: Date.now() });
        } else {
          set({ error: data?.error, loading: false });
        }
        return next;
      } catch (error) {
        set({ error: "Failed to fetch providers", loading: false });
        return get().providers;
      } finally {
        providersInFlight = null;
      }
    })();

    return providersInFlight;
  },

  // Same TTL semantics as providers. Returns the current/fetched node list.
  fetchProviderNodes: async ({ force = false } = {}) => {
    const { nodesLastFetched, providerNodes } = get();
    if (
      !force &&
      providerNodes.length > 0 &&
      Date.now() - nodesLastFetched < CLIENT_STORE_TTL_MS
    ) {
      return providerNodes;
    }
    if (nodesInFlight) return nodesInFlight;

    set({ nodesLoading: true, nodesError: null });
    nodesInFlight = (async () => {
      try {
        const response = await fetch("/api/provider-nodes");
        const data = await response.json();
        const next = data?.nodes || data?.providerNodes || data || [];
        if (response.ok) {
          set({ providerNodes: next, nodesLoading: false, nodesLastFetched: Date.now() });
        } else {
          set({ nodesError: data?.error, nodesLoading: false });
        }
        return next;
      } catch (error) {
        set({ nodesError: "Failed to fetch provider nodes", nodesLoading: false });
        return get().providerNodes;
      } finally {
        nodesInFlight = null;
      }
    })();

    return nodesInFlight;
  },
}));

export default useProviderStore;
