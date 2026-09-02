// Profile/Settings — Node profile/page.js section order:
// Local Mode (theme + db location), Security (require login + password form
// with confirm), Routing Strategy (RR + sticky limits), Network (outbound
// proxy + test), Observability, then Shutdown/Logout actions + app footer.
// SSO card shows an honest "requires Node engine" note.
<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { api } from "@/lib/api";
import { fetchAuthStatus, toast } from "@/lib/state";
import Toggle from "@/components/Toggle.vue";

interface VersionInfo { version: string; engine: string; uptimeSecs: number }

const router = useRouter();
const settings = ref<Record<string, unknown> | null>(null);
const version = ref<VersionInfo | null>(null);

const dark = ref(document.documentElement.classList.contains("dark"));
function setTheme(mode: "light" | "dark") {
  dark.value = mode === "dark";
  document.documentElement.classList.toggle("dark", dark.value);
  try { localStorage.setItem("orouter-theme", mode); } catch { /* ok */ }
}

const currentPassword = ref("");
const newPassword = ref("");
const confirmPassword = ref("");
const savingPw = ref(false);
const pwStatus = ref<{ ok: boolean; msg: string } | null>(null);

const routingLimit = ref("");
const comboLimit = ref("");
const comboRR = ref(false);
const proxyUrl = ref("");
const noProxy = ref("");
const proxyTestStatus = ref<{ ok: boolean; msg: string } | null>(null);
const retentionDays = ref("");

onMounted(async () => {
  try {
    settings.value = await api.get<Record<string, unknown>>("/api/settings");
    routingLimit.value = String(settings.value?.stickyRoundRobinLimit ?? 3);
    comboLimit.value = String(settings.value?.comboStickyRoundRobinLimit ?? 1);
    comboRR.value = String(settings.value?.comboStrategy ?? "") === "round-robin";
    proxyUrl.value = String(settings.value?.outboundProxyUrl ?? "");
    noProxy.value = String(settings.value?.outboundNoProxy ?? "");
    retentionDays.value = String(settings.value?.usageHistoryRetentionDays ?? 30);
  } catch {
    toast.error("Failed to load settings");
  }
  try {
    version.value = await api.get<VersionInfo>("/api/version");
  } catch { /* footer shows placeholder */ }
});

function bool(key: string): boolean {
  return !!settings.value?.[key];
}
function str(key: string): string {
  return String(settings.value?.[key] ?? "");
}

async function patch(patchObj: Record<string, unknown>, okMsg: string) {
  try {
    settings.value = { ...(settings.value ?? {}), ...patchObj };
    await api.patch("/api/settings", patchObj);
    toast.success(okMsg);
    return true;
  } catch {
    toast.error("Failed to save");
    return false;
  }
}

async function changePassword() {
  if (newPassword.value !== confirmPassword.value) {
    pwStatus.value = { ok: false, msg: "Passwords do not match" };
    return;
  }
  savingPw.value = true;
  try {
    await api.patch("/api/settings", { currentPassword: currentPassword.value, newPassword: newPassword.value });
    pwStatus.value = { ok: true, msg: "Password updated" };
    toast.success("Password updated successfully!");
    currentPassword.value = "";
    newPassword.value = "";
    confirmPassword.value = "";
  } catch {
    pwStatus.value = { ok: false, msg: "Update failed — check current password" };
  } finally {
    savingPw.value = false;
  }
}

async function testProxy() {
  proxyTestStatus.value = null;
  try {
    const r = await api.post<{ ok: boolean; ip?: string; error?: string }>("/api/settings/proxy-test", { proxyUrl: proxyUrl.value.trim() });
    proxyTestStatus.value = r.ok
      ? { ok: true, msg: `Proxy OK — exit IP ${r.ip ?? "?"}` }
      : { ok: false, msg: r.error ?? "Proxy unreachable" };
  } catch {
    proxyTestStatus.value = { ok: false, msg: "Test request failed" };
  }
}

async function saveLimits() {
  const rl = parseInt(routingLimit.value, 10);
  const cl = parseInt(comboLimit.value, 10);
  const p: Record<string, unknown> = {};
  if (!Number.isNaN(rl)) p.stickyRoundRobinLimit = rl;
  if (!Number.isNaN(cl)) p.comboStickyRoundRobinLimit = cl;
  if (Object.keys(p).length) patch(p, "Routing limits saved");
}

async function saveProxy() {
  patch({ outboundProxyUrl: proxyUrl.value.trim(), outboundNoProxy: noProxy.value.trim() }, "Proxy settings saved");
}

async function saveRetention() {
  const d = parseInt(retentionDays.value, 10);
  if (!Number.isNaN(d)) patch({ usageHistoryRetentionDays: d }, "Retention saved");
}

async function shutdown() {
  if (!confirm("Are you sure you want to close the proxy server?")) return;
  try {
    await api.post("/api/version/shutdown");
  } catch { /* server goes away mid-response */ }
}

async function logout() {
  try {
    await fetch("/api/auth/logout", { method: "POST", credentials: "include" });
  } catch { /* best effort */ }
  await fetchAuthStatus(true);
  router.push("/login");
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
  <div class="fade-in flex flex-col gap-4" style="max-width: 680px; margin: 0 auto">
    <!-- 1. Local Mode -->
    <div class="kid-card">
      <div style="display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 0.6rem">
        <div style="display: flex; align-items: center; gap: 0.6rem">
          <span class="material-symbols-outlined" style="color: var(--color-success); font-size: 22px">computer</span>
          <div>
            <h2 style="font-size: 1.2rem; margin: 0">Local Mode</h2>
            <span style="font-family: var(--font-body); font-size: 0.85rem; color: var(--color-text-muted)">Running on your machine</span>
          </div>
        </div>
        <div style="display: inline-flex; border: 1px solid var(--nb-border)">
          <button class="kid-btn" style="border: none" :class="''" @click="setTheme('light')"><span class="material-symbols-outlined" style="font-size: 16px">light_mode</span></button>
          <button class="kid-btn" style="border: none" @click="setTheme('dark')"><span class="material-symbols-outlined" style="font-size: 16px">dark_mode</span></button>
        </div>
      </div>
      <div style="margin-top: 0.9rem; padding: 0.6rem 0.8rem; background: var(--color-bg-alt); border: 1px solid var(--nb-border)">
        <div class="console-label" style="margin-bottom: 0.25rem">Database Location</div>
        <code style="font-size: 0.85rem">%APPDATA%\9router\db\data.sqlite</code>
        <div style="font-family: var(--font-body); font-size: 0.8rem; color: var(--color-text-muted); margin-top: 0.4rem">
          Backup via the Node dashboard (hybrid mode) — settings/database export.
        </div>
      </div>
    </div>

    <!-- 2. Security -->
    <div class="kid-card">
      <div style="display: flex; align-items: center; gap: 0.6rem; margin-bottom: 0.75rem">
        <span class="material-symbols-outlined" style="font-size: 20px; color: var(--color-primary)">shield</span>
        <h2 style="font-size: 1.2rem; margin: 0">Security</h2>
      </div>
      <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem">
        <div>
          <strong>Require login</strong>
          <div style="font-family: var(--font-body); font-size: 0.88rem; color: var(--color-text-muted)">Non-loopback clients must sign in</div>
        </div>
        <Toggle :checked="bool('requireLogin')" @change="(v) => patch({ requireLogin: v }, 'requireLogin updated')" />
      </div>
      <form v-if="bool('requireLogin')" style="margin-top: 0.9rem; display: grid; gap: 0.6rem" @submit.prevent="changePassword">
        <div v-if="bool('hasPassword')">
          <label style="font-family: var(--font-body); font-size: 0.9rem">Current Password</label>
          <input v-model="currentPassword" type="password" class="kid-input" :disabled="savingPw" />
        </div>
        <div style="display: grid; gap: 0.6rem; grid-template-columns: 1fr 1fr">
          <div>
            <label style="font-family: var(--font-body); font-size: 0.9rem">New Password</label>
            <input v-model="newPassword" type="password" class="kid-input" :disabled="savingPw" />
          </div>
          <div>
            <label style="font-family: var(--font-body); font-size: 0.9rem">Confirm New Password</label>
            <input v-model="confirmPassword" type="password" class="kid-input" :disabled="savingPw" />
          </div>
        </div>
        <p v-if="pwStatus" :style="{ fontFamily: 'var(--font-body)', fontSize: '0.88rem', margin: 0, color: pwStatus.ok ? 'var(--color-success)' : 'var(--color-danger)' }">
          {{ pwStatus.msg }}
        </p>
        <button type="submit" class="kid-btn kid-btn--primary" style="justify-self: start" :disabled="savingPw || !newPassword.trim()">
          {{ bool('hasPassword') ? "Update Password" : "Set Password" }}
        </button>
      </form>
    </div>

    <!-- 3. Routing Strategy -->
    <div class="kid-card">
      <div style="display: flex; align-items: center; gap: 0.6rem; margin-bottom: 0.75rem">
        <span class="material-symbols-outlined" style="font-size: 20px; color: var(--color-primary)">route</span>
        <h2 style="font-size: 1.2rem; margin: 0">Routing Strategy</h2>
      </div>
      <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem">
        <div><strong>Round Robin</strong><div style="font-family: var(--font-body); font-size: 0.88rem; color: var(--color-text-muted)">Rotate accounts instead of fill-first</div></div>
        <Toggle :checked="str('fallbackStrategy') === 'round-robin'" @change="(v) => patch({ fallbackStrategy: v ? 'round-robin' : 'fill-first' }, 'fallbackStrategy updated')" />
      </div>
      <div v-if="str('fallbackStrategy') === 'round-robin'" style="display: flex; justify-content: space-between; align-items: center; gap: 1rem; margin-top: 0.75rem">
        <div><strong>Sticky Limit</strong></div>
        <input v-model="routingLimit" type="number" min="1" max="10" class="kid-input" style="width: 90px" />
      </div>
      <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem; margin-top: 0.75rem">
        <div><strong>Combo Round Robin</strong></div>
        <Toggle :checked="comboRR" @change="(v) => { comboRR = v; patch({ comboStrategy: v ? 'round-robin' : 'fallback' }, 'comboStrategy updated'); }" />
      </div>
      <div v-if="comboRR" style="display: flex; justify-content: space-between; align-items: center; gap: 1rem; margin-top: 0.75rem">
        <div><strong>Combo Sticky Limit</strong></div>
        <input v-model="comboLimit" type="number" min="1" max="100" class="kid-input" style="width: 90px" />
      </div>
      <button class="kid-btn" style="margin-top: 0.9rem" @click="saveLimits">Save limits</button>
    </div>

    <!-- 5. Network -->
    <div class="kid-card">
      <div style="display: flex; align-items: center; gap: 0.6rem; margin-bottom: 0.75rem">
        <span class="material-symbols-outlined" style="font-size: 20px; color: var(--color-primary)">wifi</span>
        <h2 style="font-size: 1.2rem; margin: 0">Network</h2>
      </div>
      <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem">
        <div><strong>Outbound Proxy</strong></div>
        <Toggle :checked="bool('outboundProxyEnabled')" @change="(v) => patch({ outboundProxyEnabled: v }, 'outboundProxyEnabled updated')" />
      </div>
      <div v-if="bool('outboundProxyEnabled')" style="margin-top: 0.75rem; display: grid; gap: 0.6rem">
        <div>
          <label style="font-family: var(--font-body); font-size: 0.9rem">Proxy URL</label>
          <input v-model="proxyUrl" class="kid-input" placeholder="http://127.0.0.1:7890" />
        </div>
        <div>
          <label style="font-family: var(--font-body); font-size: 0.9rem">No Proxy (comma-separated)</label>
          <input v-model="noProxy" class="kid-input" placeholder="localhost,127.0.0.1" />
        </div>
        <div style="display: flex; gap: 0.5rem; align-items: center">
          <button class="kid-btn" @click="testProxy">Test proxy URL</button>
          <button class="kid-btn kid-btn--primary" @click="saveProxy">Apply</button>
          <span v-if="proxyTestStatus" :style="{ fontFamily: 'var(--font-body)', fontSize: '0.85rem', color: proxyTestStatus.ok ? 'var(--color-success)' : 'var(--color-danger)' }">
            {{ proxyTestStatus.msg }}
          </span>
        </div>
      </div>
    </div>

    <!-- 6. Observability -->
    <div class="kid-card">
      <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem">
        <div style="display: flex; align-items: center; gap: 0.6rem">
          <span class="material-symbols-outlined" style="font-size: 20px; color: var(--color-primary)">monitoring</span>
          <div>
            <strong>Enable Observability</strong>
            <div style="font-family: var(--font-body); font-size: 0.88rem; color: var(--color-text-muted)">Record request details for inspection in the logs view</div>
          </div>
        </div>
        <Toggle :checked="bool('enableObservability')" @change="(v) => patch({ enableObservability: v }, 'enableObservability updated')" />
      </div>
      <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem; margin-top: 0.9rem">
        <div><strong>Usage history retention (days)</strong></div>
        <div style="display: flex; gap: 0.4rem; align-items: center">
          <input v-model="retentionDays" type="number" class="kid-input" style="width: 90px" />
          <button class="kid-btn" @click="saveRetention">Save</button>
        </div>
      </div>
    </div>

    <!-- 7. Account actions -->
    <div style="display: flex; gap: 0.6rem; justify-content: flex-end">
      <button class="kid-btn" style="border-color: var(--color-danger); color: var(--color-danger)" @click="shutdown">
        <span class="material-symbols-outlined" style="font-size: 16px">power_settings_new</span> Shutdown
      </button>
      <button class="kid-btn" @click="logout">
        <span class="material-symbols-outlined" style="font-size: 16px">logout</span> Logout
      </button>
    </div>

    <!-- 8. Footer -->
    <div style="text-align: center; font-family: var(--font-body); font-size: 0.85rem; color: var(--color-text-muted); padding: 0.5rem 0 1rem">
      ORouter v{{ version?.version ?? "…" }} · engine {{ version?.engine ?? "rust" }} · up {{ version ? fmtUptime(version.uptimeSecs) : "…" }}<br />
      Local Mode — All data stored on your machine
    </div>
  </div>
</template>
