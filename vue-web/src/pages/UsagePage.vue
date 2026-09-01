// Usage: totals, per-day bar chart, by-provider, top models, recent logs.
// Backed by /api/usage/stats, /api/usage/chart, /api/usage/logs.
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { toast } from "@/lib/state";

interface BucketStat {
  requests: number;
  promptTokens: number;
  completionTokens: number;
  cost: number;
}
interface UsageStats {
  totalRequests: number;
  totalPromptTokens: number;
  totalCompletionTokens: number;
  totalCachedTokens: number;
  totalCost: number;
  byProvider: Record<string, BucketStat>;
  byModel: Record<string, BucketStat>;
}
interface ChartPoint { date: string; requests: number; cost: number }

const PERIODS = ["today", "24h", "7d", "30d", "60d", "all"];
const period = ref("7d");
const stats = ref<UsageStats | null>(null);
const chart = ref<ChartPoint[]>([]);
const logs = ref<string[]>([]);
const loading = ref(true);

async function load() {
  loading.value = true;
  try {
    const [s, c, l] = await Promise.all([
      fetch(`/api/usage/stats?period=${period.value}`, { credentials: "include" }).then((r) => r.json() as Promise<UsageStats>),
      fetch(`/api/usage/chart?period=${period.value}`, { credentials: "include" }).then((r) => r.json() as Promise<{ series?: ChartPoint[] }>),
      fetch("/api/usage/logs", { credentials: "include" }).then((r) => r.json() as Promise<string[]>),
    ]);
    stats.value = s;
    chart.value = c.series ?? [];
    logs.value = Array.isArray(l) ? l : [];
  } catch {
    toast.error("Failed to load usage data");
  } finally {
    loading.value = false;
  }
}
onMounted(load);

function fmt(n: number) {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + "M";
  if (n >= 1_000) return (n / 1_000).toFixed(1) + "k";
  return String(n);
}
function fmtCost(c: number) {
  if (c === 0) return "$0";
  if (c < 1) return `$${c.toFixed(4)}`;
  return `$${c.toFixed(2)}`;
}

const providers = computed(() =>
  stats.value ? Object.entries(stats.value.byProvider).sort((a, b) => b[1].requests - a[1].requests) : [],
);
const models = computed(() =>
  stats.value ? Object.entries(stats.value.byModel).sort((a, b) => b[1].requests - a[1].requests).slice(0, 12) : [],
);
const maxRequests = computed(() => chart.value.reduce((mx, p) => Math.max(mx, p.requests), 0));
</script>

<template>
  <div class="fade-in flex flex-col gap-4">
    <!-- Period tabs -->
    <div style="display: flex; justify-content: flex-end; flex-wrap: wrap; gap: 0.4rem">
      <button
        v-for="p in PERIODS"
        :key="p"
        class="kid-btn"
        :style="{ padding: '0.3rem 0.7rem', fontSize: '0.8rem', background: period === p ? 'var(--color-accent)' : 'var(--color-surface)' }"
        @click="period = p; load()"
      >
        {{ p }}
      </button>
    </div>

    <p v-if="loading" style="font-family: var(--font-body)">Loading usage…</p>

    <!-- Totals -->
    <div v-if="stats && !loading" style="display: grid; gap: 1rem; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr))">
      <div v-for="c in [
        { label: 'Requests', value: fmt(stats.totalRequests) },
        { label: 'Prompt tokens', value: fmt(stats.totalPromptTokens) },
        { label: 'Completion tokens', value: fmt(stats.totalCompletionTokens) },
        { label: 'Cached tokens', value: fmt(stats.totalCachedTokens) },
        { label: 'Cost', value: fmtCost(stats.totalCost) },
      ]" :key="c.label" class="kid-card" style="padding: 0.75rem 1rem; display: flex; flex-direction: column; gap: 0.25rem">
        <div style="font-family: var(--font-body); font-size: 0.8rem; text-transform: uppercase; font-weight: 600; letter-spacing: 0.05em; color: var(--color-text-muted)">{{ c.label }}</div>
        <div style="font-size: 1.6rem; font-weight: 700; line-height: 1.15">{{ c.value }}</div>
      </div>
    </div>

    <!-- Daily chart -->
    <div v-if="chart.length > 0 && !loading">
      <h2 style="font-size: 1.25rem; margin: 0 0 0.5rem">Daily requests</h2>
      <div class="kid-card" style="display: flex; align-items: flex-end; gap: 0.45rem; height: 160px; padding: 1rem; overflow-x: auto">
        <div v-for="p in chart" :key="p.date" style="display: flex; flex-direction: column; align-items: center; gap: 0.3rem; min-width: 34px" :title="`${p.date}: ${p.requests} req, ${fmtCost(p.cost)}`">
          <span style="font-family: var(--font-body); font-size: 0.75rem; color: var(--color-text-muted)">{{ p.requests }}</span>
          <div :style="{ width: '26px', height: Math.max(4, Math.round((p.requests / Math.max(maxRequests, 1)) * 100)) + 'px', background: 'var(--color-brand-500)', borderRadius: '6px 6px 2px 2px' }" />
          <span style="font-family: var(--font-body); font-size: 0.7rem; color: var(--color-text-muted); white-space: nowrap">{{ p.date.slice(5) }}</span>
        </div>
      </div>
    </div>

    <!-- By provider -->
    <div v-if="providers.length > 0">
      <h2 style="font-size: 1.25rem; margin: 0 0 0.5rem">By provider</h2>
      <div style="display: grid; gap: 0.6rem">
        <div v-for="[name, s] in providers" :key="name" class="kid-card" style="display: flex; justify-content: space-between; align-items: center; padding: 0.7rem 1rem; flex-wrap: wrap; gap: 0.5rem">
          <strong>{{ name }}</strong>
          <div style="font-family: var(--font-body); color: var(--color-text-muted); display: flex; gap: 1.2rem; flex-wrap: wrap">
            <span>{{ fmt(s.requests) }} req</span>
            <span>{{ fmt(s.promptTokens + s.completionTokens) }} tok</span>
            <span>{{ fmtCost(s.cost) }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- Top models -->
    <div v-if="models.length > 0">
      <h2 style="font-size: 1.25rem; margin: 0 0 0.5rem">Top models</h2>
      <div class="kid-card" style="padding: 0; overflow-x: auto">
        <table style="width: 100%; border-collapse: collapse; text-align: left; font-family: var(--font-body)">
          <thead>
            <tr style="border-bottom: 3px solid var(--nb-border); font-size: 0.7rem; text-transform: uppercase; letter-spacing: 0.05em; color: var(--color-text-muted)">
              <th style="padding: 0.5rem 0.8rem">Model</th>
              <th style="padding: 0.5rem 0.8rem; text-align: right">Req</th>
              <th style="padding: 0.5rem 0.8rem; text-align: right">Tokens</th>
              <th style="padding: 0.5rem 0.8rem; text-align: right">Cost</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="[name, s] in models" :key="name" style="border-bottom: 2px solid var(--color-surface-3)">
              <td style="padding: 0.5rem 0.8rem"><code style="font-size: 0.85rem">{{ name }}</code></td>
              <td style="padding: 0.5rem 0.8rem; text-align: right">{{ fmt(s.requests) }}</td>
              <td style="padding: 0.5rem 0.8rem; text-align: right">{{ fmt(s.promptTokens + s.completionTokens) }}</td>
              <td style="padding: 0.5rem 0.8rem; text-align: right">{{ fmtCost(s.cost) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- Recent logs -->
    <div>
      <h2 style="font-size: 1.25rem; margin: 0 0 0.5rem">Recent requests</h2>
      <div v-if="logs.length === 0" class="kid-card" style="text-align: center">
        <p style="font-family: var(--font-body)">No requests logged yet. Send a chat to see usage here!</p>
      </div>
      <div v-else class="kid-card" style="padding: 0; overflow: hidden">
        <div
          v-for="(line, i) in logs.slice(0, 50)"
          :key="i"
          style="font-family: var(--font-body); font-size: 0.85rem; padding: 0.45rem 0.8rem; white-space: pre-wrap; word-break: break-word; border-bottom: 1px solid var(--color-bg-alt)"
        >
          {{ line }}
        </div>
      </div>
    </div>
  </div>
</template>
