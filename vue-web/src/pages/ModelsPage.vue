// Models catalog: /api/models snapshot — search, kind filter, native badge,
// pricing, copy routable "provider/model" id.
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { toast } from "@/lib/state";
import Badge from "@/components/Badge.vue";

interface CatalogModel {
  id: string;
  name?: string;
  kind: string;
  nativeChat?: boolean | null;
  upstreamId?: string;
  inputPrice?: number | null;
  outputPrice?: number | null;
}
interface CatalogProvider {
  provider: string;
  hasNativeTransport: boolean;
  models: CatalogModel[];
}

const KINDS = ["llm", "embedding", "image", "tts", "stt", "video"];

const providers = ref<CatalogProvider[]>([]);
const loading = ref(true);
const provider = ref("");
const search = ref("");
const kind = ref("llm");

onMounted(async () => {
  try {
    const data = await fetch("/api/models", { credentials: "include" }).then((r) => r.json()) as { providers?: CatalogProvider[] };
    providers.value = data.providers ?? [];
    const first = providers.value.find((p) => p.models.length > 0) ?? providers.value[0];
    if (first) provider.value = first.provider;
  } catch {
    toast.error("Failed to load model catalog");
  } finally {
    loading.value = false;
  }
});

const current = computed(() => providers.value.find((p) => p.provider === provider.value));

const models = computed(() => {
  let list = current.value?.models ?? [];
  if (kind.value !== "all") list = list.filter((m) => m.kind === kind.value);
  const q = search.value.trim().toLowerCase();
  if (q) {
    list = list.filter(
      (m) => m.id.toLowerCase().includes(q) || (m.name ?? "").toLowerCase().includes(q) || (m.upstreamId ?? "").toLowerCase().includes(q),
    );
  }
  return list;
});

function fmtPrice(p?: number | null) {
  return p === null || p === undefined ? "—" : `$${p}`;
}

function copyText(text: string) {
  navigator.clipboard?.writeText(text).then(
    () => toast.success(`Copied ${text}`),
    () => toast.error("Failed to copy"),
  );
}
</script>

<template>
  <div class="fade-in flex flex-col gap-4" style="max-width: 1100px">
    <!-- Filters -->
    <div class="kid-card" style="display: flex; flex-wrap: wrap; gap: 0.6rem; align-items: center; padding: 0.8rem 1rem">
      <select v-model="provider" class="kid-input" style="width: auto; min-width: 220px">
        <option v-for="p in providers" :key="p.provider" :value="p.provider">
          {{ p.provider }} ({{ p.models.length }}){{ p.hasNativeTransport ? " ⚡" : "" }}
        </option>
      </select>
      <input v-model="search" class="kid-input" style="flex: 1; min-width: 200px" placeholder="Search model id or name…" />
      <select v-model="kind" class="kid-input" style="width: auto">
        <option value="all">All kinds</option>
        <option v-for="k in KINDS" :key="k" :value="k">{{ k }}</option>
      </select>
    </div>

    <p v-if="loading" style="font-family: var(--font-body)">Loading catalog…</p>

    <div v-if="!loading" style="display: flex; gap: 0.6rem; flex-wrap: wrap; align-items: center; font-family: var(--font-body); font-size: 0.9rem; color: var(--color-text-muted)">
      <Badge variant="success" size="sm" dot>native (Rust direct)</Badge>
      <Badge variant="neutral" size="sm" dot>Node translator</Badge>
      <span>· {{ models.length }} model(s) shown</span>
    </div>

    <div v-if="!loading && models.length > 0" class="kid-card" style="padding: 0; overflow-x: auto">
      <table style="width: 100%; border-collapse: collapse; text-align: left; font-family: var(--font-body)">
        <thead>
          <tr style="border-bottom: 3px solid var(--nb-border); font-size: 0.7rem; text-transform: uppercase; letter-spacing: 0.05em; color: var(--color-text-muted)">
            <th style="padding: 0.6rem 0.8rem">Model</th>
            <th style="padding: 0.6rem 0.8rem">Kind</th>
            <th style="padding: 0.6rem 0.8rem">Route</th>
            <th style="padding: 0.6rem 0.8rem">Upstream</th>
            <th style="padding: 0.6rem 0.8rem; text-align: right">In / Out $/Mtok</th>
            <th style="padding: 0.6rem 0.8rem"></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="m in models" :key="m.id" style="border-bottom: 2px solid var(--color-surface-3)">
            <td style="padding: 0.55rem 0.8rem">
              <code style="font-size: 0.9rem; background: var(--color-bg-alt); padding: 0.15rem 0.35rem; border: 1px solid var(--nb-border)">{{ m.id }}</code>
              <div v-if="m.name && m.name !== m.id" style="font-size: 0.85rem; color: var(--color-text-muted)">{{ m.name }}</div>
            </td>
            <td style="padding: 0.55rem 0.8rem">
              <Badge :variant="m.kind === 'llm' ? 'info' : 'neutral'" size="sm">{{ m.kind }}</Badge>
            </td>
            <td style="padding: 0.55rem 0.8rem">
              <Badge v-if="m.nativeChat !== false" variant="success" size="sm" dot>native</Badge>
              <Badge v-else variant="neutral" size="sm" dot>Node</Badge>
            </td>
            <td style="padding: 0.55rem 0.8rem; font-size: 0.85rem; color: var(--color-text-muted)">
              <code v-if="m.upstreamId && m.upstreamId !== m.id">{{ m.upstreamId }}</code>
              <template v-else>—</template>
            </td>
            <td style="padding: 0.55rem 0.8rem; text-align: right; white-space: nowrap">
              {{ fmtPrice(m.inputPrice) }} / {{ fmtPrice(m.outputPrice) }}
            </td>
            <td style="padding: 0.55rem 0.8rem; text-align: right">
              <button class="kid-btn" style="padding: 0.2rem 0.5rem; font-size: 0.85rem" @click="copyText(`${provider}/${m.id}`)">
                <span class="material-symbols-outlined" style="font-size: 14px">content_copy</span> {{ provider }}/{{ m.id }}
              </button>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <div v-if="!loading && models.length === 0" class="kid-card" style="text-align: center">
      <p style="font-family: var(--font-body)">No models match this filter.</p>
    </div>
  </div>
</template>
