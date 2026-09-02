// Providers — Node providers/page.js layout:
// grouped sections (Custom Compatible / OAuth / Free Tier / API Key),
// provider-type cards with connection stats linking to a detail page,
// per-group Test All + results modal, hover toggle-all, header search.
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { api } from "@/lib/api";
import { searchQuery, toast } from "@/lib/state";
import Badge from "@/components/Badge.vue";
import Toggle from "@/components/Toggle.vue";
import Modal from "@/components/Modal.vue";

interface Connection {
  id: string;
  provider: string;
  name?: string;
  email?: string;
  authType?: string;
  priority?: number;
  isActive?: boolean;
  testStatus?: string;
  lastError?: string;
  [key: string]: unknown;
}
interface CatalogProvider {
  provider: string;
  hasNativeTransport: boolean;
  models: { id: string }[];
}

const OAUTH_PROVIDERS = new Set([
  "claude", "codex", "antigravity", "glm", "glm-cn", "kiro", "cursor",
  "iflow", "qwen", "gemini-cli", "xai", "kimchi", "kiroc", "trae",
]);
const FREE_PROVIDERS = new Set(["ollama-local", "llm7", "api-airforce", "chutes"]);

const connections = ref<Connection[]>([]);
const catalog = ref<CatalogProvider[]>([]);
const loading = ref(true);
const showAllApikey = ref(false);
const togglingProvider = ref<string | null>(null);

// test-all runner state
const testingGroup = ref<string | null>(null);
const testResults = ref<{ name: string; provider: string; ok: boolean; ms: number }[] | null>(null);
const stopRequested = ref(false);

async function load() {
  loading.value = true;
  try {
    const [conns, models] = await Promise.all([
      api.get<{ connections: Connection[] }>("/api/providers"),
      fetch("/api/models", { credentials: "include" }).then((r) => r.json() as Promise<{ providers: CatalogProvider[] }>),
    ]);
    connections.value = conns.connections ?? [];
    catalog.value = models.providers ?? [];
  } catch {
    toast.error("Failed to load providers");
  } finally {
    loading.value = false;
  }
}
onMounted(load);

interface ProviderCardData {
  id: string;
  conns: Connection[];
  hasNativeTransport: boolean;
}

const compatibleProviders = computed<ProviderCardData[]>(() => {
  const map = new Map<string, Connection[]>();
  for (const c of connections.value) {
    if (c.provider.startsWith("openai-compatible") || c.provider.startsWith("anthropic-compatible")) {
      const list = map.get(c.provider) ?? [];
      list.push(c);
      map.set(c.provider, list);
    }
  }
  return [...map.entries()].map(([id, conns]) => ({ id, conns, hasNativeTransport: true }));
});

const oauthProviders = computed<ProviderCardData[]>(() => groupProviders((id) => OAUTH_PROVIDERS.has(id)));
const freeProviders = computed<ProviderCardData[]>(() => groupProviders((id) => FREE_PROVIDERS.has(id)));
const apikeyProviders = computed<ProviderCardData[]>(() => {
  const known = new Set([...OAUTH_PROVIDERS, ...FREE_PROVIDERS]);
  return groupProviders((id) => !known.has(id) && !id.startsWith("openai-compatible") && !id.startsWith("anthropic-compatible"));
});

function groupProviders(pred: (id: string) => boolean): ProviderCardData[] {
  const map = new Map<string, Connection[]>();
  for (const c of connections.value) {
    if (pred(c.provider)) {
      const list = map.get(c.provider) ?? [];
      list.push(c);
      map.set(c.provider, list);
    }
  }
  // Also surface catalog providers with zero connections (Node shows them).
  for (const cp of catalog.value) {
    if (pred(cp.provider) && !map.has(cp.provider)) {
      map.set(cp.provider, []);
    }
  }
  return [...map.entries()]
    .map(([id, conns]) => ({
      id,
      conns,
      hasNativeTransport: catalog.value.find((c) => c.provider === id)?.hasNativeTransport ?? false,
    }))
    .sort((a, b) => a.id.localeCompare(b.id));
}

const q = computed(() => searchQuery.value.trim().toLowerCase());
function matchesSearch(id: string) {
  return !q.value || id.toLowerCase().includes(q.value);
}

const anyMatch = computed(
  () =>
    compatibleProviders.value.some((p) => matchesSearch(p.id)) ||
    oauthProviders.value.some((p) => matchesSearch(p.id)) ||
    freeProviders.value.some((p) => matchesSearch(p.id)) ||
    apikeyProviders.value.some((p) => matchesSearch(p.id)),
);

const visibleApikey = computed(() => {
  const list = apikeyProviders.value.filter((p) => matchesSearch(p.id));
  return showAllApikey.value || q.value ? list : list.slice(0, 20);
});

function statusOf(p: ProviderCardData) {
  const active = p.conns.filter((c) => c.isActive !== false);
  if (p.conns.length > 0 && active.length === 0) return { kind: "disabled" as const };
  const connected = active.filter((c) => c.testStatus === "active").length;
  const errors = active.filter((c) => c.testStatus === "error").length;
  if (connected > 0) return { kind: "connected" as const, n: connected };
  if (errors > 0) return { kind: "error" as const, n: errors };
  if (p.conns.length === 0) return { kind: "none" as const };
  return { kind: "noconn" as const };
}

function iconSrc(id: string) {
  return `/providers/${id}.png`;
}

async function toggleAll(p: ProviderCardData) {
  if (p.conns.length === 0) return;
  togglingProvider.value = p.id;
  const next = !p.conns.every((c) => c.isActive !== false);
  const results = await Promise.allSettled(
    p.conns.map((c) => api.put(`/api/providers/${c.id}`, { isActive: next })),
  );
  togglingProvider.value = null;
  const failed = results.filter((r) => r.status === "rejected").length;
  if (failed > 0) toast.error(`${failed} connection(s) failed to update`);
  await load();
}

async function testGroup(group: string, list: ProviderCardData[]) {
  const conns = list.flatMap((p) => p.conns.filter((c) => c.isActive !== false));
  if (conns.length === 0) {
    testResults.value = [];
    return;
  }
  testingGroup.value = group;
  stopRequested.value = false;
  const results: { name: string; provider: string; ok: boolean; ms: number }[] = [];
  for (const c of conns) {
    if (stopRequested.value) break;
    const t0 = performance.now();
    try {
      await api.post(`/api/providers/${c.id}/test`);
      results.push({ name: c.name ?? c.provider, provider: c.provider, ok: true, ms: Math.round(performance.now() - t0) });
    } catch {
      results.push({ name: c.name ?? c.provider, provider: c.provider, ok: false, ms: Math.round(performance.now() - t0) });
    }
  }
  testingGroup.value = null;
  testResults.value = results;
  await load();
}
</script>

<template>
  <div class="fade-in flex flex-col gap-6" style="max-width: 1200px">
    <p v-if="loading" style="font-family: var(--font-body)">Loading providers…</p>

    <div
      v-if="!loading && !anyMatch"
      class="kid-card"
      style="text-align: center; padding: 3rem 1rem; border-style: dashed"
    >
      <span class="material-symbols-outlined" style="font-size: 40px; color: var(--color-text-muted)">search_off</span>
      <p style="font-family: var(--font-body); margin: 0.5rem 0 0">No providers match your search</p>
    </div>

    <template v-if="!loading">
      <!-- Custom Compatible -->
      <section v-if="compatibleProviders.filter((p) => matchesSearch(p.id)).length > 0">
        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.75rem; flex-wrap: wrap; gap: 0.5rem">
          <h2 style="font-size: 1.2rem; margin: 0">Custom Providers (OpenAI/Anthropic Compatible)</h2>
        </div>
        <div style="display: grid; gap: 1rem; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr))">
          <router-link
            v-for="p in compatibleProviders.filter((p) => matchesSearch(p.id))"
            :key="p.id"
            :to="`/dashboard/providers/${p.id}`"
            class="kid-card"
            style="display: flex; align-items: center; gap: 0.75rem; text-decoration: none; color: inherit"
          >
            <div style="width: 32px; height: 32px; display: flex; align-items: center; justify-content: center; background: color-mix(in srgb, var(--color-brand-500) 15%, transparent)">
              <img :src="iconSrc(p.id)" alt="" style="width: 24px; height: 24px; object-fit: contain" />
            </div>
            <strong style="flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap">{{ p.id }}</strong>
            <span style="font-family: var(--font-body); font-size: 0.8rem; color: var(--color-text-muted)">{{ p.conns.length }}</span>
          </router-link>
        </div>
      </section>

      <!-- OAuth -->
      <section v-if="oauthProviders.filter((p) => matchesSearch(p.id)).length > 0">
        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.75rem; flex-wrap: wrap; gap: 0.5rem">
          <h2 style="font-size: 1.2rem; margin: 0">OAuth Providers</h2>
          <button
            class="kid-btn kid-btn--accent"
            style="padding: 0.3rem 0.7rem; font-size: 0.82rem"
            :disabled="testingGroup === 'oauth'"
            @click="testGroup('oauth', oauthProviders)"
          >
            <span class="material-symbols-outlined" style="font-size: 15px">science</span>
            {{ testingGroup === "oauth" ? "Testing..." : "Test All" }}
          </button>
        </div>
        <div style="display: grid; gap: 1rem; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr))">
          <router-link
            v-for="p in oauthProviders.filter((p) => matchesSearch(p.id))"
            :key="p.id"
            :to="`/dashboard/providers/${p.id}`"
            class="kid-card prov-card"
            :style="statusOf(p).kind === 'disabled' ? { opacity: 0.5 } : undefined"
          >
            <div style="display: flex; align-items: center; gap: 0.75rem">
              <div style="width: 32px; height: 32px; display: flex; align-items: center; justify-content: center; background: color-mix(in srgb, var(--color-brand-500) 15%, transparent)">
                <img :src="iconSrc(p.id)" alt="" style="width: 24px; height: 24px; object-fit: contain" />
              </div>
              <strong style="flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap">{{ p.id }}</strong>
              <div @click.prevent>
                <Toggle
                  :checked="p.conns.length > 0 && p.conns.some((c) => c.isActive !== false)"
                  :disabled="p.conns.length === 0 || togglingProvider === p.id"
                  @change="() => toggleAll(p)"
                />
              </div>
            </div>
            <div style="margin-top: 0.5rem; display: flex; align-items: center; gap: 0.4rem; flex-wrap: wrap">
              <Badge v-if="statusOf(p).kind === 'disabled'" variant="neutral" size="sm">Disabled</Badge>
              <Badge v-else-if="statusOf(p).kind === 'connected'" variant="success" size="sm" dot>{{ statusOf(p).n }} Connected</Badge>
              <Badge v-else-if="statusOf(p).kind === 'error'" variant="danger" size="sm" dot>{{ statusOf(p).n }} Error</Badge>
              <span v-else style="font-family: var(--font-body); font-size: 0.82rem; color: var(--color-text-muted)">No connections</span>
              <Badge v-if="p.hasNativeTransport" variant="info" size="sm">native</Badge>
            </div>
          </router-link>
        </div>
      </section>

      <!-- Free Tier -->
      <section v-if="freeProviders.filter((p) => matchesSearch(p.id)).length > 0">
        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.75rem">
          <h2 style="font-size: 1.2rem; margin: 0">Free Tier Providers</h2>
        </div>
        <div style="display: grid; gap: 1rem; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr))">
          <router-link
            v-for="p in freeProviders.filter((p) => matchesSearch(p.id))"
            :key="p.id"
            :to="`/dashboard/providers/${p.id}`"
            class="kid-card prov-card"
            style="display: flex; align-items: center; gap: 0.75rem; text-decoration: none; color: inherit"
          >
            <div style="width: 32px; height: 32px; display: flex; align-items: center; justify-content: center; background: color-mix(in srgb, var(--color-success) 15%, transparent)">
              <img :src="iconSrc(p.id)" alt="" style="width: 24px; height: 24px; object-fit: contain" />
            </div>
            <strong style="flex: 1">{{ p.id }}</strong>
            <span style="font-family: var(--font-body); font-size: 0.8rem; color: var(--color-text-muted)">{{ p.conns.length }} conn</span>
          </router-link>
        </div>
      </section>

      <!-- API Key -->
      <section v-if="apikeyProviders.filter((p) => matchesSearch(p.id)).length > 0">
        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.75rem; flex-wrap: wrap; gap: 0.5rem">
          <h2 style="font-size: 1.2rem; margin: 0">API Key Providers</h2>
          <button
            class="kid-btn kid-btn--accent"
            style="padding: 0.3rem 0.7rem; font-size: 0.82rem"
            :disabled="testingGroup === 'apikey'"
            @click="testGroup('apikey', apikeyProviders)"
          >
            <span class="material-symbols-outlined" style="font-size: 15px">science</span>
            {{ testingGroup === "apikey" ? "Testing..." : "Test All" }}
          </button>
        </div>
        <div style="display: grid; gap: 1rem; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr))">
          <router-link
            v-for="p in visibleApikey"
            :key="p.id"
            :to="`/dashboard/providers/${p.id}`"
            class="kid-card prov-card"
            :style="statusOf(p).kind === 'disabled' ? { opacity: 0.5 } : undefined"
          >
            <div style="display: flex; align-items: center; gap: 0.75rem">
              <div style="width: 32px; height: 32px; display: flex; align-items: center; justify-content: center; background: color-mix(in srgb, var(--color-brand-500) 15%, transparent)">
                <img :src="iconSrc(p.id)" alt="" style="width: 24px; height: 24px; object-fit: contain" />
              </div>
              <strong style="flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap">{{ p.id }}</strong>
              <div @click.prevent>
                <Toggle
                  :checked="p.conns.length > 0 && p.conns.some((c) => c.isActive !== false)"
                  :disabled="p.conns.length === 0 || togglingProvider === p.id"
                  @change="() => toggleAll(p)"
                />
              </div>
            </div>
            <div style="margin-top: 0.5rem; display: flex; align-items: center; gap: 0.4rem; flex-wrap: wrap">
              <Badge v-if="statusOf(p).kind === 'disabled'" variant="neutral" size="sm">Disabled</Badge>
              <Badge v-else-if="statusOf(p).kind === 'connected'" variant="success" size="sm" dot>{{ statusOf(p).n }} Connected</Badge>
              <Badge v-else-if="statusOf(p).kind === 'error'" variant="danger" size="sm" dot>{{ statusOf(p).n }} Error</Badge>
              <span v-else style="font-family: var(--font-body); font-size: 0.82rem; color: var(--color-text-muted)">No connections</span>
              <Badge v-if="p.hasNativeTransport" variant="info" size="sm">native</Badge>
            </div>
          </router-link>
        </div>
        <button
          v-if="!q && apikeyProviders.length > 20"
          class="kid-btn"
          style="width: 100%; margin-top: 1rem; border-style: dashed"
          @click="showAllApikey = !showAllApikey"
        >
          {{ showAllApikey ? "Show fewer" : `Show all ${apikeyProviders.length} providers` }}
          <span class="material-symbols-outlined" style="font-size: 16px">{{ showAllApikey ? "expand_less" : "expand_more" }}</span>
        </button>
      </section>
    </template>

    <!-- Test results modal -->
    <Modal v-if="testResults" width="480px" @close="testResults = null">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.75rem">
        <h3 style="font-size: 1.2rem; margin: 0">Test Results</h3>
        <button class="kid-btn" style="padding: 0.2rem 0.4rem" @click="testResults = null">
          <span class="material-symbols-outlined" style="font-size: 16px">close</span>
        </button>
      </div>
      <div style="display: flex; gap: 0.5rem; margin-bottom: 0.75rem; flex-wrap: wrap">
        <Badge variant="success" size="sm">{{ testResults.filter((r) => r.ok).length }} passed</Badge>
        <Badge v-if="testResults.some((r) => !r.ok)" variant="danger" size="sm">{{ testResults.filter((r) => !r.ok).length }} failed</Badge>
        <Badge variant="neutral" size="sm">{{ testResults.length }} tested</Badge>
      </div>
      <div style="max-height: 50vh; overflow-y: auto">
        <div v-for="(r, i) in testResults" :key="i" style="display: flex; align-items: center; gap: 0.6rem; padding: 0.4rem 0; border-top: 1px solid var(--color-border-subtle)">
          <span class="material-symbols-outlined" :style="{ fontSize: 16, color: r.ok ? 'var(--color-success)' : 'var(--color-danger)' }">
            {{ r.ok ? "check_circle" : "error" }}
          </span>
          <strong style="flex: 1; font-size: 0.9rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap">{{ r.name }}</strong>
          <span style="font-family: var(--font-body); font-size: 0.8rem; color: var(--color-text-muted)">{{ r.ms }}ms</span>
          <Badge :variant="r.ok ? 'success' : 'danger'" size="sm">{{ r.ok ? "OK" : "FAIL" }}</Badge>
        </div>
      </div>
    </Modal>
  </div>
</template>

<style scoped>
.prov-card {
  text-decoration: none;
  color: inherit;
  transition: box-shadow 0.15s, border-color 0.15s;
}
.prov-card:hover {
  box-shadow: var(--nb-shadow);
  border-color: color-mix(in srgb, var(--color-brand-500) 30%, transparent);
}
</style>
