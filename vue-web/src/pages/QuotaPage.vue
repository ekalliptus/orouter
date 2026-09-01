// Quota: per-connection live quota. OpenRouter is probed natively (credits
// API); OAuth-based providers report why native quota is unavailable.
<script setup lang="ts">
import { onMounted, ref } from "vue";
import { api } from "@/lib/api";
import { toast } from "@/lib/state";
import Badge from "@/components/Badge.vue";

interface Connection {
  id: string;
  provider: string;
  name?: string;
  isActive?: boolean;
  testStatus?: string;
  lastError?: string;
}
interface QuotaResult {
  available?: boolean;
  provider?: string;
  label?: string | null;
  limit?: number | null;
  usage?: number | null;
  limitRemaining?: number | null;
  isFreeTier?: boolean | null;
  error?: string | null;
  reason?: string | null;
  testStatus?: string | null;
}

const connections = ref<Connection[]>([]);
const results = ref<Record<string, QuotaResult>>({});
const loading = ref(true);
const checkingId = ref<string | null>(null);

async function load() {
  loading.value = true;
  try {
    const data = await api.get<{ connections: Connection[] }>("/api/providers");
    connections.value = data.connections ?? [];
  } catch {
    toast.error("Failed to load connections");
  } finally {
    loading.value = false;
  }
}
onMounted(load);

async function check(c: Connection) {
  checkingId.value = c.id;
  try {
    results.value = { ...results.value, [c.id]: await api.get<QuotaResult>(`/api/usage/${c.id}`) };
  } catch (e) {
    toast.error(e instanceof Error && e.message ? e.message : "Quota check failed");
  } finally {
    checkingId.value = null;
  }
}

function pct(r: QuotaResult): number | null {
  if (typeof r.limit !== "number" || typeof r.usage !== "number" || r.limit <= 0) return null;
  return Math.min(100, Math.round((r.usage / r.limit) * 100));
}

function iconSrc(c: Connection) {
  return `/providers/${c.provider}.png`;
}
</script>

<template>
  <div class="fade-in flex flex-col gap-4" style="max-width: 900px">
    <div style="display: flex; justify-content: flex-end">
      <button class="kid-btn" style="padding: 0.3rem 0.75rem" @click="load">
        <span class="material-symbols-outlined" style="font-size: 16px">refresh</span> Refresh
      </button>
    </div>

    <p v-if="loading" style="font-family: var(--font-body)">Loading connections…</p>

    <div v-if="!loading && connections.length === 0" class="kid-card" style="text-align: center">
      <p style="font-family: var(--font-body)">No connections configured.</p>
    </div>

    <div style="display: grid; gap: 1rem; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr))">
      <div v-for="c in connections" :key="c.id" class="kid-card" :style="c.isActive === false ? { opacity: 0.55 } : undefined">
        <div style="display: flex; justify-content: space-between; align-items: flex-start">
          <div style="display: flex; gap: 0.75rem; align-items: center">
            <img :src="iconSrc(c)" alt="" style="width: 32px; height: 32px; object-fit: contain" />
            <div>
              <strong style="display: block">{{ c.name ?? c.provider }}</strong>
              <span style="font-family: var(--font-body); color: var(--color-text-muted); font-size: 0.85rem">{{ c.provider }}</span>
            </div>
          </div>
          <Badge :variant="c.testStatus === 'active' ? 'success' : c.testStatus === 'error' ? 'danger' : 'neutral'" size="sm" dot>
            {{ c.testStatus ?? "unknown" }}
          </Badge>
        </div>

        <template v-if="results[c.id]">
          <div v-if="results[c.id].available" style="margin-top: 0.75rem">
            <div v-if="results[c.id].label" style="font-family: var(--font-body); font-size: 0.85rem; color: var(--color-text-muted)">
              {{ results[c.id].label }}<span v-if="results[c.id].isFreeTier"> · free tier</span>
            </div>
            <div v-if="pct(results[c.id]) !== null" style="margin-top: 0.4rem">
              <div style="height: 8px; background: var(--color-surface-3); border: 1px solid var(--nb-border)">
                <div
                  :style="{
                    height: '100%',
                    width: pct(results[c.id]) + '%',
                    background: pct(results[c.id])! > 85 ? 'var(--color-danger)' : 'var(--color-brand-500)',
                  }"
                />
              </div>
              <div style="font-family: var(--font-body); font-size: 0.8rem; color: var(--color-text-muted); margin-top: 0.25rem">
                used ${{ results[c.id].usage }} / ${{ results[c.id].limit }}
                <template v-if="typeof results[c.id].limitRemaining === 'number'">
                  · ${{ results[c.id].limitRemaining }} left
                </template>
              </div>
            </div>
            <div v-else style="font-family: var(--font-body); font-size: 0.85rem; color: var(--color-text-muted); margin-top: 0.4rem">
              usage: ${{ results[c.id].usage ?? "?" }}<template v-if="results[c.id].limit"> / limit {{ results[c.id].limit }}</template>
            </div>
          </div>
          <div v-else style="margin-top: 0.6rem; font-family: var(--font-body); font-size: 0.85rem; color: var(--color-text-muted)">
            <template v-if="results[c.id].error">{{ results[c.id].error }}</template>
            <template v-else>{{ results[c.id].reason }}</template>
          </div>
        </template>

        <div style="display: flex; justify-content: flex-end; margin-top: 0.85rem">
          <button class="kid-btn kid-btn--accent" style="padding: 0.25rem 0.6rem; font-size: 0.82rem" :disabled="checkingId === c.id" @click="check(c)">
            <span class="material-symbols-outlined" :class="{ 'animate-spin': checkingId === c.id }" style="font-size: 15px">
              {{ checkingId === c.id ? "progress_activity" : "data_usage" }}
            </span>
            {{ checkingId === c.id ? "Checking…" : "Check quota" }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
