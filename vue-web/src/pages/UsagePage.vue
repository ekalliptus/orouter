// Usage — Node layout: SegmentedControl Overview|Details; periods
// Today|24h|7D|30D|60D; 5 overview cards; chart with Tokens/Cost toggle;
// usage tables (by Model/Account/API Key/Endpoint × Costs/Tokens);
// recent requests with In/Out arrows. Backed by native Rust endpoints.
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { toast } from "@/lib/state";
import SegmentedControl from "@/components/SegmentedControl.vue";

interface BucketStat {
  requests: number;
  promptTokens: number;
  completionTokens: number;
  cachedTokens?: number;
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
  byAccount: Record<string, BucketStat>;
  byApiKey: Record<string, BucketStat>;
  byEndpoint: Record<string, BucketStat>;
  recentRequests?: { timestamp: string; model: string; provider: string; promptTokens: number; completionTokens: number; status: string }[];
}
interface ChartPoint { date: string; requests: number; promptTokens: number; completionTokens: number; cost: number }

const tab = ref("overview");
const period = ref("today");
const stats = ref<UsageStats | null>(null);
const chart = ref<ChartPoint[]>([]);
const loading = ref(true);

const PERIODS = [
  { value: "today", label: "Today" },
  { value: "24h", label: "24h" },
  { value: "7d", label: "7D" },
  { value: "30d", label: "30D" },
  { value: "60d", label: "60D" },
];

const chartMetric = ref<"tokens" | "cost">("tokens");
const tableSel = ref("byModel");
const valueMode = ref<"costs" | "tokens">("tokens");
const sortKey = ref("requests");
const sortDesc = ref(true);

async function load() {
  loading.value = true;
  try {
    const [s, c] = await Promise.all([
      fetch(`/api/usage/stats?period=${period.value}`, { credentials: "include" }).then((r) => r.json() as Promise<UsageStats>),
      fetch(`/api/usage/chart?period=${period.value}`, { credentials: "include" }).then((r) => r.json() as Promise<{ series?: ChartPoint[] }>),
    ]);
    stats.value = s;
    chart.value = c.series ?? [];
  } catch {
    toast.error("Failed to load usage");
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
function relTime(iso: string) {
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return iso;
  const diff = Math.max(0, Date.now() - then);
  const s = Math.floor(diff / 1000);
  if (s < 60) return "Just now";
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
  return `${Math.floor(s / 86400)}d ago`;
}

const maxChart = computed(() =>
  Math.max(1, ...chart.value.map((p) => (chartMetric.value === "tokens" ? p.promptTokens + p.completionTokens : p.cost))),
);

interface Row {
  key: string;
  label: string;
  secondary?: string;
  requests: number;
  prompt: number;
  cached: number;
  completion: number;
  cost: number;
}

const tableRows = computed<Row[]>(() => {
  const s = stats.value;
  if (!s) return [];
  const toRow = ([key, v]: [string, BucketStat], label?: string, secondary?: string): Row => ({
    key,
    label: label ?? key,
    secondary,
    requests: v.requests,
    prompt: v.promptTokens,
    cached: v.cachedTokens ?? 0,
    completion: v.completionTokens,
    cost: v.cost,
  });
  if (tableSel.value === "byModel") {
    return Object.entries(s.byModel).map(([k, v]) => {
      const [model, provider] = k.split("|");
      return toRow([k, v], model, provider ?? "");
    });
  }
  if (tableSel.value === "byAccount") return Object.entries(s.byAccount).map(([k, v]) => toRow([k, v], k));
  if (tableSel.value === "byApiKey") return Object.entries(s.byApiKey).map(([k, v]) => toRow([k, v], k.split("|")[0]));
  return Object.entries(s.byEndpoint).map(([k, v]) => toRow([k, v], k));
});

const sortedRows = computed(() => {
  const k = sortKey.value as keyof Row;
  const rows = [...tableRows.value];
  rows.sort((a, b) => {
    const av = a[k] as number;
    const bv = b[k] as number;
    return sortDesc.value ? bv - av : av - bv;
  });
  return rows;
});

function setSort(k: string) {
  if (sortKey.value === k) sortDesc.value = !sortDesc.value;
  else {
    sortKey.value = k;
    sortDesc.value = true;
  }
}

const recent = computed(() => stats.value?.recentRequests ?? []);
</script>

<template>
  <div class="fade-in flex flex-col gap-4">
    <!-- Tabs row -->
    <div style="display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 0.6rem">
      <SegmentedControl
        :options="[
          { value: 'overview', label: 'Overview' },
          { value: 'details', label: 'Details' },
        ]"
        :model-value="tab"
        @update:model-value="tab = $event"
      />
      <SegmentedControl
        v-if="tab === 'overview'"
        small
        :options="PERIODS"
        :model-value="period"
        @update:model-value="period = $event; load()"
      />
    </div>

    <p v-if="loading" style="font-family: var(--font-body)">Loading…</p>

    <!-- ============ OVERVIEW ============ -->
    <template v-if="tab === 'overview' && !loading">
      <!-- 5 stat cards -->
      <div style="display: grid; gap: 1rem; grid-template-columns: repeat(auto-fill, minmax(170px, 1fr))">
        <div v-for="c in [
          { label: 'Total Requests', value: fmt(stats?.totalRequests ?? 0) },
          { label: 'Total Input Tokens', value: fmt(stats?.totalPromptTokens ?? 0) },
          { label: 'Cached Tokens', value: fmt(stats?.totalCachedTokens ?? 0) },
          { label: 'Output Tokens', value: fmt(stats?.totalCompletionTokens ?? 0) },
          { label: 'Est. Cost', value: '~' + fmtCost(stats?.totalCost ?? 0) },
        ]" :key="c.label" class="kid-card" style="padding: 0.75rem 1rem; display: flex; flex-direction: column; gap: 0.25rem">
          <div style="font-family: var(--font-body); font-size: 0.75rem; text-transform: uppercase; font-weight: 600; letter-spacing: 0.05em; color: var(--color-text-muted)">{{ c.label }}</div>
          <div style="font-size: 1.5rem; font-weight: 700; line-height: 1.15">{{ c.value }}</div>
          <div v-if="c.label === 'Est. Cost'" style="font-family: var(--font-body); font-size: 0.7rem; color: var(--color-text-subtle)">Estimated, not actual billing</div>
        </div>
      </div>

      <!-- Chart + Recent -->
      <div style="display: grid; gap: 1rem; grid-template-columns: 2fr minmax(280px, 1fr)">
        <div class="kid-card">
          <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.75rem">
            <h2 style="font-size: 1.1rem; margin: 0">Usage Chart</h2>
            <SegmentedControl
              small
              :options="[{ value: 'tokens', label: 'Tokens' }, { value: 'cost', label: 'Cost' }]"
              :model-value="chartMetric"
              @update:model-value="chartMetric = $event as any"
            />
          </div>
          <div v-if="chart.length === 0" style="font-family: var(--font-body); color: var(--color-text-muted); padding: 2rem 0; text-align: center">
            No data for this period
          </div>
          <div v-else style="display: flex; align-items: flex-end; gap: 0.4rem; height: 150px; overflow-x: auto">
            <div v-for="p in chart" :key="p.date" style="display: flex; flex-direction: column; align-items: center; gap: 0.3rem; min-width: 32px" :title="`${p.date}: ${fmtCost(p.cost)}`">
              <div
                :style="{
                  width: '24px',
                  height: Math.max(4, Math.round(((chartMetric === 'tokens' ? p.promptTokens + p.completionTokens : p.cost) / maxChart) * 100)) + 'px',
                  background: chartMetric === 'tokens' ? 'var(--color-brand-500)' : 'var(--color-warning)',
                  borderRadius: '6px 6px 2px 2px',
                }"
              />
              <span style="font-family: var(--font-body); font-size: 0.68rem; color: var(--color-text-muted); white-space: nowrap">{{ p.date.slice(5) }}</span>
            </div>
          </div>
        </div>

        <div class="kid-card" style="height: 480px; display: flex; flex-direction: column">
          <div class="console-label" style="margin-bottom: 0.5rem">Recent Requests</div>
          <div v-if="recent.length === 0" style="font-family: var(--font-body); color: var(--color-text-muted)">No requests yet.</div>
          <div v-else style="overflow-y: auto; flex: 1">
            <div v-for="(r, i) in recent" :key="i" style="display: flex; align-items: center; gap: 0.5rem; padding: 0.4rem 0; border-bottom: 1px solid var(--color-border-subtle)">
              <span
                style="width: 6px; height: 6px; border-radius: 9999px; flex-shrink: 0"
                :style="{ background: r.status === 'ok' ? 'var(--color-success)' : 'var(--color-danger)' }"
              />
              <code style="font-size: 0.75rem; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap">{{ r.model }}</code>
              <span style="font-family: var(--font-body); font-size: 0.75rem; color: var(--color-text-muted); white-space: nowrap">
                {{ r.promptTokens }}↑ {{ r.completionTokens }}↓
              </span>
              <span style="font-family: var(--font-body); font-size: 0.72rem; color: var(--color-text-subtle); white-space: nowrap">{{ relTime(r.timestamp) }}</span>
            </div>
          </div>
        </div>
      </div>

      <!-- Usage tables -->
      <div class="kid-card">
        <div style="display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 0.6rem; margin-bottom: 0.75rem">
          <select v-model="tableSel" class="kid-input" style="width: auto">
            <option value="byModel">Usage by Model</option>
            <option value="byAccount">Usage by Account</option>
            <option value="byApiKey">Usage by API Key</option>
            <option value="byEndpoint">Usage by Endpoint</option>
          </select>
          <SegmentedControl
            small
            :options="[{ value: 'costs', label: 'Costs' }, { value: 'tokens', label: 'Tokens' }]"
            :model-value="valueMode"
            @update:model-value="valueMode = $event as any"
          />
        </div>

        <div v-if="sortedRows.length === 0" style="font-family: var(--font-body); color: var(--color-text-muted); padding: 1rem 0">
          No usage recorded yet.
        </div>
        <div v-else style="overflow-x: auto">
          <table style="width: 100%; border-collapse: collapse; text-align: left; font-family: var(--font-body); font-size: 0.88rem">
            <thead>
              <tr style="border-bottom: 3px solid var(--nb-border); font-size: 0.7rem; text-transform: uppercase; letter-spacing: 0.05em; color: var(--color-text-muted)">
                <th style="padding: 0.5rem 0.6rem; cursor: pointer" @click="setSort('label')">{{ tableSel === 'byEndpoint' ? 'Endpoint' : tableSel === 'byApiKey' ? 'API Key Name' : 'Model' }} ↕</th>
                <th v-if="tableSel !== 'byEndpoint' && tableSel !== 'byApiKey'" style="padding: 0.5rem 0.6rem">Provider</th>
                <th style="padding: 0.5rem 0.6rem; text-align: right; cursor: pointer" @click="setSort('requests')">Requests ↕</th>
                <th style="padding: 0.5rem 0.6rem; text-align: right">Input</th>
                <th style="padding: 0.5rem 0.6rem; text-align: right">Cached</th>
                <th style="padding: 0.5rem 0.6rem; text-align: right">Output</th>
                <th style="padding: 0.5rem 0.6rem; text-align: right">{{ valueMode === 'costs' ? 'Total Cost' : 'Total Tokens' }}</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="r in sortedRows" :key="r.key" style="border-bottom: 2px solid var(--color-surface-3)">
                <td style="padding: 0.45rem 0.6rem"><code style="font-size: 0.82rem">{{ r.label }}</code></td>
                <td v-if="tableSel !== 'byEndpoint' && tableSel !== 'byApiKey'" style="padding: 0.45rem 0.6rem; color: var(--color-text-muted)">{{ r.secondary || "—" }}</td>
                <td style="padding: 0.45rem 0.6rem; text-align: right">{{ fmt(r.requests) }}</td>
                <td style="padding: 0.45rem 0.6rem; text-align: right; color: var(--color-text-subtle)">{{ valueMode === 'costs' ? '—' : fmt(r.prompt) }}</td>
                <td style="padding: 0.45rem 0.6rem; text-align: right; color: var(--color-text-subtle)">{{ valueMode === 'costs' ? '—' : fmt(r.cached) }}</td>
                <td style="padding: 0.45rem 0.6rem; text-align: right; color: var(--color-text-subtle)">{{ valueMode === 'costs' ? '—' : fmt(r.completion) }}</td>
                <td style="padding: 0.45rem 0.6rem; text-align: right; font-weight: 600">{{ valueMode === 'costs' ? fmtCost(r.cost) : fmt(r.prompt + r.completion) }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </template>

    <!-- ============ DETAILS ============ -->
    <template v-if="tab === 'details' && !loading">
      <div class="kid-card" style="padding: 0">
        <div class="console-label" style="padding: 0.6rem 0.8rem; border-bottom: 2px solid var(--color-surface-3)">
          Recent request-level records
        </div>
        <div v-if="recent.length === 0" style="padding: 2rem; text-align: center; font-family: var(--font-body); color: var(--color-text-muted)">
          No request details found
        </div>
        <div v-else style="overflow-x: auto">
          <table style="width: 100%; border-collapse: collapse; text-align: left; font-family: var(--font-body); font-size: 0.85rem">
            <thead>
              <tr style="border-bottom: 3px solid var(--nb-border); font-size: 0.7rem; text-transform: uppercase; letter-spacing: 0.05em; color: var(--color-text-muted)">
                <th style="padding: 0.5rem 0.7rem">Timestamp</th>
                <th style="padding: 0.5rem 0.7rem">Model</th>
                <th style="padding: 0.5rem 0.7rem">Provider</th>
                <th style="padding: 0.5rem 0.7rem; text-align: right">Input</th>
                <th style="padding: 0.5rem 0.7rem; text-align: right">Output</th>
                <th style="padding: 0.5rem 0.7rem">Status</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(r, i) in recent" :key="i" style="border-bottom: 2px solid var(--color-surface-3)">
                <td style="padding: 0.4rem 0.7rem; white-space: nowrap">{{ relTime(r.timestamp) }}</td>
                <td style="padding: 0.4rem 0.7rem"><code style="font-size: 0.8rem">{{ r.model }}</code></td>
                <td style="padding: 0.4rem 0.7rem">{{ r.provider || "—" }}</td>
                <td style="padding: 0.4rem 0.7rem; text-align: right">{{ fmt(r.promptTokens) }}</td>
                <td style="padding: 0.4rem 0.7rem; text-align: right">{{ fmt(r.completionTokens) }}</td>
                <td style="padding: 0.4rem 0.7rem">
                  <Badge :variant="r.status === 'ok' ? 'success' : 'danger'" size="sm" dot>{{ r.status }}</Badge>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
      <div class="kid-card" style="background: color-mix(in srgb, var(--color-info) 8%, var(--color-surface))">
        <div style="font-family: var(--font-body); font-size: 0.88rem; color: var(--color-text-muted)">
          ℹ️ Full request-level details (latency, cache creation, PXPIPE, raw payloads) require the
          Node engine's observability — start hybrid mode with observability enabled.
        </div>
      </div>
    </template>
  </div>
</template>
