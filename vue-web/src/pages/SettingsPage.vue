// Settings: engine info + shutdown, password change, security, routing,
// token savers, network, observability — all persisted via /api/settings.
<script setup lang="ts">
import { onMounted, ref } from "vue";
import { api } from "@/lib/api";
import { toast } from "@/lib/state";
import Toggle from "@/components/Toggle.vue";

interface VersionInfo { version: string; engine: string; uptimeSecs: number }

const settings = ref<Record<string, unknown> | null>(null);
const version = ref<VersionInfo | null>(null);

const currentPassword = ref("");
const newPassword = ref("");
const savingPw = ref(false);

const routingLimit = ref("");
const comboLimit = ref("");
const headroomUrl = ref("");
const proxyUrl = ref("");
const noProxy = ref("");
const retentionDays = ref("");

function str(key: string): string {
  return String(settings.value?.[key] ?? "");
}
function bool(key: string): boolean {
  return !!settings.value?.[key];
}

onMounted(async () => {
  try {
    settings.value = await api.get<Record<string, unknown>>("/api/settings");
    routingLimit.value = String(settings.value?.stickyRoundRobinLimit ?? 3);
    comboLimit.value = String(settings.value?.comboStickyRoundRobinLimit ?? 1);
    headroomUrl.value = String(settings.value?.headroomUrl ?? "");
    proxyUrl.value = String(settings.value?.outboundProxyUrl ?? "");
    noProxy.value = String(settings.value?.outboundNoProxy ?? "");
    retentionDays.value = String(settings.value?.usageHistoryRetentionDays ?? 30);
  } catch {
    toast.error("Failed to load settings");
  }
  try {
    version.value = await api.get<VersionInfo>("/api/version");
  } catch { /* card shows placeholder */ }
});

async function patch(patchObj: Record<string, unknown>, okMsg: string) {
  try {
    settings.value = { ...(settings.value ?? {}), ...patchObj };
    await api.patch("/api/settings", patchObj);
    toast.success(okMsg);
  } catch {
    toast.error("Failed to save setting");
  }
}

async function changePassword() {
  if (!newPassword.value.trim()) return;
  savingPw.value = true;
  try {
    await api.patch("/api/settings", { currentPassword: currentPassword.value, newPassword: newPassword.value });
    toast.success("Password updated successfully!");
    currentPassword.value = "";
    newPassword.value = "";
  } catch {
    toast.error("Failed to update password. Check current password.");
  } finally {
    savingPw.value = false;
  }
}

async function shutdown() {
  if (!confirm("Shut down the ORouter backend now?")) return;
  try {
    await api.post("/api/version/shutdown");
    toast.success("Shutdown requested — the server is stopping.");
  } catch {
    toast.success("Shutdown requested — the server is stopping.");
  }
}

function fmtUptime(secs: number) {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}
</script>

<template>
  <div class="fade-in flex flex-col gap-4" style="max-width: 720px">
    <!-- Engine -->
    <div class="kid-card">
      <div style="font-weight: 700; font-size: 1.15rem; margin-bottom: 0.9rem">Engine</div>
      <div style="display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 0.8rem">
        <div style="font-family: var(--font-body); color: var(--color-text-muted); font-size: 0.95rem">
          <div>ORouter backend <strong style="color: var(--color-text-main)">v{{ version?.version ?? "…" }}</strong> · engine: <strong style="color: var(--color-text-main)">rust</strong></div>
          <div>Uptime: {{ version ? fmtUptime(version.uptimeSecs) : "…" }}</div>
        </div>
        <button class="kid-btn" style="background: var(--color-danger); color: #fff" @click="shutdown">Shut down server</button>
      </div>
    </div>

    <!-- Password -->
    <form class="kid-card" @submit.prevent="changePassword">
      <div style="font-weight: 700; font-size: 1.15rem; margin-bottom: 0.75rem">Change Dashboard Password</div>
      <div style="display: grid; gap: 0.6rem">
        <div>
          <label style="font-family: var(--font-body); font-size: 0.9rem">Current password</label>
          <input v-model="currentPassword" type="password" class="kid-input" :disabled="savingPw" />
        </div>
        <div>
          <label style="font-family: var(--font-body); font-size: 0.9rem">New password</label>
          <input v-model="newPassword" type="password" class="kid-input" :disabled="savingPw" />
        </div>
      </div>
      <button type="submit" class="kid-btn kid-btn--primary" style="margin-top: 0.75rem" :disabled="savingPw || !newPassword.trim()">
        {{ savingPw ? "Saving…" : "Update Password" }}
      </button>
    </form>

    <!-- Security -->
    <div v-if="settings" class="kid-card">
      <div style="font-weight: 700; font-size: 1.15rem; margin-bottom: 0.9rem">Security</div>
      <div style="display: flex; flex-direction: column; gap: 1rem">
        <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem">
          <div><strong>Require API Key for LLM Requests</strong></div>
          <Toggle :checked="bool('requireApiKey')" @change="(v) => patch({ requireApiKey: v }, 'requireApiKey updated')" />
        </div>
        <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem">
          <div><strong>Require Dashboard Login</strong></div>
          <Toggle :checked="bool('requireLogin')" @change="(v) => patch({ requireLogin: v }, 'requireLogin updated')" />
        </div>
        <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem">
          <div><strong>Allow Dashboard via Tunnel</strong></div>
          <Toggle :checked="bool('tunnelDashboardAccess')" @change="(v) => patch({ tunnelDashboardAccess: v }, 'tunnelDashboardAccess updated')" />
        </div>
      </div>
    </div>

    <!-- Routing -->
    <div v-if="settings" class="kid-card">
      <div style="font-weight: 700; font-size: 1.15rem; margin-bottom: 0.9rem">Routing Strategy</div>
      <div style="display: grid; gap: 0.8rem; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr))">
        <div>
          <label style="font-family: var(--font-body); font-size: 0.9rem">Combo strategy</label>
          <select class="kid-input" :value="str('comboStrategy') || 'fallback'" @change="patch({ comboStrategy: ($event.target as HTMLSelectElement).value }, 'comboStrategy updated')">
            <option value="fallback">fallback</option>
            <option value="round-robin">round-robin</option>
            <option value="sticky-round-robin">sticky-round-robin</option>
          </select>
        </div>
        <div>
          <label style="font-family: var(--font-body); font-size: 0.9rem">Sticky RR limit (per account)</label>
          <input v-model="routingLimit" type="number" min="1" class="kid-input" />
        </div>
        <div>
          <label style="font-family: var(--font-body); font-size: 0.9rem">Sticky RR limit (combo)</label>
          <input v-model="comboLimit" type="number" min="1" class="kid-input" />
        </div>
      </div>
      <button
        class="kid-btn kid-btn--primary"
        style="margin-top: 0.75rem"
        @click="() => {
          const rl = parseInt(routingLimit, 10); const cl = parseInt(comboLimit, 10);
          const p: Record<string, unknown> = {};
          if (!Number.isNaN(rl)) p.stickyRoundRobinLimit = rl;
          if (!Number.isNaN(cl)) p.comboStickyRoundRobinLimit = cl;
          if (Object.keys(p).length) patch(p, 'Routing limits saved');
        }"
      >
        Save limits
      </button>
    </div>

    <!-- Token savers -->
    <div v-if="settings" class="kid-card">
      <div style="font-weight: 700; font-size: 1.15rem; margin-bottom: 0.9rem">Token Savers</div>
      <div style="display: flex; flex-direction: column; gap: 1rem">
        <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem">
          <div><strong>RTK compression</strong><div style="font-family: var(--font-body); color: var(--color-text-muted); font-size: 0.9rem">Compress tool_result content in-flight.</div></div>
          <Toggle :checked="bool('rtkEnabled')" @change="(v) => patch({ rtkEnabled: v }, 'rtkEnabled updated')" />
        </div>
        <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem">
          <div><strong>Caveman mode</strong></div>
          <Toggle :checked="bool('cavemanEnabled')" @change="(v) => patch({ cavemanEnabled: v }, 'cavemanEnabled updated')" />
        </div>
        <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem">
          <div><strong>Ponytail mode</strong></div>
          <Toggle :checked="bool('ponytailEnabled')" @change="(v) => patch({ ponytailEnabled: v }, 'ponytailEnabled updated')" />
        </div>
        <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem">
          <div><strong>Headroom integration</strong></div>
          <Toggle :checked="bool('headroomEnabled')" @change="(v) => patch({ headroomEnabled: v }, 'headroomEnabled updated')" />
        </div>
        <div v-if="bool('headroomEnabled')" style="display: grid; gap: 0.6rem">
          <input v-model="headroomUrl" class="kid-input" placeholder="http://localhost:8787" />
          <button class="kid-btn" style="justify-self: start" @click="patch({ headroomUrl: headroomUrl.trim() }, 'Headroom URL saved')">Save headroom URL</button>
        </div>
        <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem">
          <div><strong>PXPIPE</strong></div>
          <Toggle :checked="bool('pxpipeEnabled')" @change="(v) => patch({ pxpipeEnabled: v }, 'pxpipeEnabled updated')" />
        </div>
      </div>
    </div>

    <!-- Network -->
    <div v-if="settings" class="kid-card">
      <div style="font-weight: 700; font-size: 1.15rem; margin-bottom: 0.9rem">Network</div>
      <div style="display: flex; flex-direction: column; gap: 1rem">
        <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem">
          <div><strong>Outbound proxy</strong></div>
          <Toggle :checked="bool('outboundProxyEnabled')" @change="(v) => patch({ outboundProxyEnabled: v }, 'outboundProxyEnabled updated')" />
        </div>
        <div style="display: grid; gap: 0.6rem">
          <input v-model="proxyUrl" class="kid-input" placeholder="http://127.0.0.1:7890" />
          <input v-model="noProxy" class="kid-input" placeholder="localhost,127.0.0.1" />
          <button class="kid-btn" style="justify-self: start" @click="patch({ outboundProxyUrl: proxyUrl.trim(), outboundNoProxy: noProxy.trim() }, 'Proxy settings saved')">Save proxy settings</button>
        </div>
      </div>
    </div>

    <!-- Observability -->
    <div v-if="settings" class="kid-card">
      <div style="font-weight: 700; font-size: 1.15rem; margin-bottom: 0.9rem">Observability & Retention</div>
      <div style="display: flex; flex-direction: column; gap: 1rem">
        <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem">
          <div><strong>Enable observability</strong></div>
          <Toggle :checked="bool('enableObservability')" @change="(v) => patch({ enableObservability: v }, 'enableObservability updated')" />
        </div>
        <div style="display: grid; gap: 0.6rem; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr))">
          <div>
            <label style="font-family: var(--font-body); font-size: 0.9rem">Max records</label>
            <input class="kid-input" type="number" :value="str('observabilityMaxRecords')" @change="patch({ observabilityMaxRecords: parseInt(($event.target as HTMLInputElement).value, 10) || 0 }, 'observabilityMaxRecords updated')" />
          </div>
          <div>
            <label style="font-family: var(--font-body); font-size: 0.9rem">Usage retention (days)</label>
            <input v-model="retentionDays" class="kid-input" type="number" />
          </div>
        </div>
        <button
          class="kid-btn"
          style="justify-self: start"
          @click="() => { const d = parseInt(retentionDays, 10); if (!Number.isNaN(d)) patch({ usageHistoryRetentionDays: d }, 'Retention saved'); }"
        >
          Save retention
        </button>
      </div>
    </div>
  </div>
</template>
