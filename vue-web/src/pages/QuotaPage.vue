// Quota Tracker — Node ProviderLimits layout: per-connection cards with an
// animated remaining-% progress bar, {used}/{total} + {remaining}% detail,
// color state (🟢 >70% / 🟡 ≥30% / 🔴 below), auto-refresh with a visible
// countdown, refresh-all, and bulk Turn off Empty / Turn on Available.
// Native quota probing: OpenRouter (credits endpoint); OAuth providers
// honestly report the hybrid requirement.
<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref } from "vue";
import { api } from "@/lib/api";
import { toast } from "@/lib/state";
import Badge from "@/components/Badge.vue";
import Toggle from "@/components/Toggle.vue";

interface Connection {
  id: string;
  provider: string;
  name?: string;
  email?: string;
  isActive?: boolean;
  testStatus?: string;
  lastError?: string;
}
interface QuotaResult {
  available?: boolean;
  provider?: string;
  plan?: string | null;
  quotas?: { name: string; displayName?: string | null; remainingPct: number; usedPct: number; total?: number; resetAt?: string | null }[];
  dollars?: { label?: string | null; limit?: number | null; usage?: number | null; limitRemaining?: number | null; isFreeTier?: boolean | null } | null;
  message?: string | null;
  error?: string | null;
  reason?: string | null;
  testStatus?: string | null;
}

const AUTO_KEY = "orouter-quota-autorefresh";
const connections = ref<Connection[]>([]);
const results = ref<Record<string, QuotaResult>>({});
const loading = ref(true);
const checkingId = ref<string | null>(null);
const bulkBusy = ref(false);

const autoRefresh = ref(localStorage.getItem(AUTO_KEY) !== "off");
const countdown = ref(60);
let tickTimer: ReturnType<typeof setInterval> | null = null;

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

async function checkAll(force = false) {
  const targets = connections.value.filter(
    (c) => c.isActive !== false && (force || results.value[c.id] === undefined),
  );
  for (const c of targets) {
    await check(c);
  }
}

function setAuto(on: boolean) {
  autoRefresh.value = on;
  localStorage.setItem(AUTO_KEY, on ? "on" : "off");
  countdown.value = 60;
}

onMounted(async () => {
  await load();
  if (autoRefresh.value) {
    await checkAll(false);
  }
  tickTimer = setInterval(() => {
    if (!autoRefresh.value || document.hidden) return;
    countdown.value -= 1;
    if (countdown.value <= 0) {
      countdown.value = 60;
      checkAll(true);
    }
  }, 1000);
});
onBeforeUnmount(() => {
  if (tickTimer) clearInterval(tickTimer);
});

// ---- quota math ----

interface QuotaRow {
  name: string;
  emoji: string;
  pctRemaining: number | null;
  used: number | null;
  total: number | null;
  remainingAbs: number | null;
  detail: string;
}

function emojiFor(pct: number | null): string {
  return pct === null ? "⚪" : pct > 70 ? "🟢" : pct >= 30 ? "🟡" : "🔴";
}

function rowsFor(r: QuotaResult): QuotaRow[] {
  if (!r.available) return [];

  // Percent-window providers (claude / codex / antigravity / glm): native
  // `quotas[]` with remainingPct per window.
  const rows: QuotaRow[] = (r.quotas ?? []).map((q) => {
    const pct = Math.max(0, Math.min(100, Math.round(q.remainingPct)));
    const reset = q.resetAt
      ? new Date(q.resetAt).toLocaleString(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" })
      : null;
    return {
      name: (q.displayName ?? q.name) + (q.total === 1000 ? "" : ""),
      emoji: emojiFor(pct),
      pctRemaining: pct,
      used: q.usedPct !== undefined ? Math.round(q.usedPct) : null,
      total: 100,
      remainingAbs: null,
      detail: `${Math.round(q.usedPct)}% used${reset ? ` · resets ${reset}` : ""}`,
    };
  });

  // Dollar-based providers (openrouter credits).
  if (r.dollars) {
    const d = r.dollars;
    const used = typeof d.usage === "number" ? d.usage : null;
    const total = typeof d.limit === "number" ? d.limit : null;
    const remaining = typeof d.limitRemaining === "number"
      ? d.limitRemaining
      : total !== null && used !== null
        ? total - used
        : null;
    const pct = total !== null && total > 0 && remaining !== null
      ? Math.max(0, Math.min(100, Math.round((remaining / total) * 100)))
      : null;
    rows.push({
      name: d.label ?? "Credits",
      emoji: emojiFor(pct),
      pctRemaining: pct,
      used,
      total,
      remainingAbs: remaining,
      detail: total !== null
        ? `$${used?.toFixed(2) ?? "?"} / $${total.toFixed(2)} · ${pct}% left`
        : `used $${used?.toFixed(2) ?? "?"} · no hard limit`,
    });
  }
  return rows;
}

const eligible = computed(() => connections.value.filter((c) => c.isActive !== false));

const depleted = computed(() =>
  eligible.value.filter((c) => {
    const pct = rowsFor(results.value[c.id] ?? {})[0]?.pctRemaining;
    return pct !== null && pct !== undefined && pct <= 5;
  }),
);
const restorable = computed(() =>
  connections.value.filter((c) => c.isActive === false && c.provider !== "antigravity"),
);

async function bulkSetActive(targets: Connection[], active: boolean, label: string) {
  if (targets.length === 0) {
    toast.info("Nothing to change");
    return;
  }
  bulkBusy.value = true;
  let ok = 0;
  for (const c of targets) {
    try {
      await api.put(`/api/providers/${c.id}`, { isActive: active });
      ok += 1;
    } catch { /* keep going */ }
  }
  bulkBusy.value = false;
  toast.success(`${label}: ${ok} connection(s) updated`);
  await load();
}

function iconSrc(c: Connection) {
  return `/providers/${c.provider}.png`;
}
</script>

<template>
  <div class="fade-in flex flex-col gap-4" style="max-width: 1000px">
    <!-- Header controls row (Node parity) -->
    <div style="display: flex; justify-content: flex-end; align-items: center; flex-wrap: wrap; gap: 0.4rem">
      <button
        v-if="depleted.length > 0"
        class="kid-btn"
        style="padding: 0.25rem 0.6rem; font-size: 0.8rem; background: var(--color-danger); color: #fff"
        :disabled="bulkBusy"
        @click="bulkSetActive(depleted, false, 'Turn off Empty')"
      >
        <span class="material-symbols-outlined" style="font-size: 15px">block</span>
        Turn off Empty ({{ depleted.length }})
      </button>
      <button
        v-if="restorable.length > 0"
        class="kid-btn"
        style="padding: 0.25rem 0.6rem; font-size: 0.8rem; background: var(--color-success); color: #fff"
        :disabled="bulkBusy"
        @click="bulkSetActive(restorable, true, 'Turn on Available')"
      >
        <span class="material-symbols-outlined" style="font-size: 15px">check_circle</span>
        Turn on Available ({{ restorable.length }})
      </button>
      <button
        class="kid-btn"
        style="padding: 0.25rem 0.6rem; font-size: 0.8rem"
        :style="autoRefresh ? { background: 'var(--color-brand-500)', color: '#fff' } : {}"
        @click="setAuto(!autoRefresh)"
      >
        <span class="material-symbols-outlined" style="font-size: 15px">{{ autoRefresh ? "toggle_on" : "toggle_off" }}</span>
        Auto-refresh{{ autoRefresh ? ` (${countdown}s)` : "" }}
      </button>
      <button
        class="kid-btn"
        style="padding: 0.25rem 0.6rem; font-size: 0.8rem"
        :disabled="bulkBusy"
        @click="checkAll(true)"
      >
        <span class="material-symbols-outlined" style="font-size: 15px">refresh</span>
        Refresh all
      </button>
    </div>

    <p v-if="loading" style="font-family: var(--font-body)">Loading connections…</p>

    <div v-if="!loading && connections.length === 0" class="kid-card" style="text-align: center; padding: 2.5rem 1rem">
      <span class="material-symbols-outlined" style="font-size: 36px; color: var(--color-text-muted)">cloud_off</span>
      <h3 style="margin: 0.4rem 0 0.3rem">No Providers Connected</h3>
      <p style="font-family: var(--font-body); color: var(--color-text-muted); margin: 0">
        Connect to providers to track your API quota limits and usage.
      </p>
    </div>

    <!-- Cards -->
    <div style="display: grid; gap: 1rem; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr))">
      <div v-for="c in connections" :key="c.id" class="kid-card" :style="c.isActive === false ? { opacity: 0.6 } : undefined">
        <!-- Header strip -->
        <div style="display: flex; justify-content: space-between; align-items: flex-start; gap: 0.5rem">
          <div style="display: flex; gap: 0.7rem; align-items: center; min-width: 0">
            <img :src="iconSrc(c)" alt="" style="width: 32px; height: 32px; object-fit: contain" />
            <div style="min-width: 0">
              <h3 style="margin: 0; font-size: 1.02rem; text-transform: capitalize; overflow: hidden; text-overflow: ellipsis; white-space: nowrap">{{ c.provider }}</h3>
              <span style="font-family: var(--font-body); font-size: 0.82rem; color: var(--color-text-muted); display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap">
                {{ c.name ?? c.email ?? c.id.slice(0, 8) }}
              </span>
            </div>
          </div>
          <Badge :variant="c.isActive !== false ? (c.testStatus === 'active' ? 'success' : c.testStatus === 'error' ? 'danger' : 'neutral') : 'neutral'" size="sm" dot>
            {{ c.isActive === false ? "off" : c.testStatus ?? "unknown" }}
          </Badge>
        </div>

        <!-- Body -->
        <div style="margin-top: 0.75rem">
          <!-- Not probed yet -->
          <div v-if="!results[c.id]" style="font-family: var(--font-body); font-size: 0.9rem; color: var(--color-text-muted)">
            Not checked yet — hit refresh or wait for auto-refresh.
          </div>

          <!-- Probed: quota rows -->
          <template v-else>
            <div v-if="results[c.id]?.plan" style="font-family: var(--font-body); font-size: 0.82rem; color: var(--color-text-muted); margin-bottom: 0.3rem">
              Plan: <strong style="color: var(--color-text-main)">{{ results[c.id].plan }}</strong>
            </div>
            <div v-if="rowsFor(results[c.id]).length > 0" style="font-family: var(--font-body); font-size: 0.82rem; color: var(--color-text-muted); margin-bottom: 0.4rem">
              {{ rowsFor(results[c.id]).length }} quota(s)
            </div>
            <div
              v-for="(row, i) in rowsFor(results[c.id])"
              :key="i"
              style="padding: 0.35rem 0"
            >
              <div style="display: flex; align-items: center; gap: 0.45rem">
                <span style="font-size: 0.95rem">{{ row.emoji }}</span>
                <strong style="font-size: 0.88rem; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap">{{ row.name }}</strong>
                <span v-if="row.pctRemaining !== null" style="font-family: var(--font-body); font-size: 0.82rem; font-weight: 700" :style="{ color: row.pctRemaining > 70 ? 'var(--color-success)' : row.pctRemaining >= 30 ? '#d97706' : 'var(--color-danger)' }">
                  {{ row.pctRemaining }}%
                </span>
              </div>
              <!-- Animated remaining-% bar -->
              <div style="height: 9px; margin-top: 0.3rem; background: var(--color-surface-3); border: 1px solid var(--nb-border); overflow: hidden">
                <div
                  class="quota-fill"
                  :style="{
                    width: (row.pctRemaining ?? 100) + '%',
                    background: row.pctRemaining === null
                      ? 'var(--color-brand-500)'
                      : row.pctRemaining > 70
                        ? 'var(--color-success)'
                        : row.pctRemaining >= 30
                          ? '#d97706'
                          : 'var(--color-danger)',
                  }"
                />
              </div>
              <div style="font-family: var(--font-body); font-size: 0.8rem; color: var(--color-text-muted); margin-top: 0.25rem; display: flex; justify-content: space-between; gap: 0.5rem">
                <span>{{ row.detail }}</span>
                <span v-if="row.remainingAbs !== null" style="white-space: nowrap">${{ row.remainingAbs.toFixed(2) }} left</span>
              </div>
            </div>

            <!-- Unavailable / error / hybrid note -->
            <div v-if="rowsFor(results[c.id]).length === 0" style="font-family: var(--font-body); font-size: 0.88rem; color: var(--color-text-muted)">
              <template v-if="results[c.id].error">{{ results[c.id].error }}</template>
              <template v-else>{{ results[c.id].reason }}</template>
            </div>
          </template>
        </div>

        <!-- Card actions -->
        <div style="display: flex; justify-content: space-between; align-items: center; margin-top: 0.85rem">
          <Toggle :checked="c.isActive !== false" @change="() => bulkSetActive([c], c.isActive === false, c.isActive === false ? 'Enable' : 'Disable')" />
          <button
            class="kid-btn kid-btn--accent"
            style="padding: 0.25rem 0.6rem; font-size: 0.82rem"
            :disabled="checkingId === c.id"
            @click="check(c)"
          >
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

<style scoped>
/* Animated remaining-% fill: grows from 0 on mount, eases on updates. */
.quota-fill {
  height: 100%;
  transition: width 1s cubic-bezier(0.22, 1, 0.36, 1);
  animation: quota-grow 1s cubic-bezier(0.22, 1, 0.36, 1);
  position: relative;
}
@keyframes quota-grow {
  from {
    width: 0 !important;
  }
}
/* Subtle shimmer so live bars read as "fresh data". */
.quota-fill::after {
  content: "";
  position: absolute;
  inset: 0;
  background: linear-gradient(
    110deg,
    transparent 25%,
    rgba(255, 255, 255, 0.35) 50%,
    transparent 75%
  );
  background-size: 200% 100%;
  animation: quota-shimmer 2.2s linear infinite;
}
@keyframes quota-shimmer {
  from {
    background-position: 200% 0;
  }
  to {
    background-position: -200% 0;
  }
}
</style>
