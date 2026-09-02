// Provider detail — Node providers/[id] layout:
// back link + icon header; Connections card (priority arrows, auth badges,
// proxy assign, one-by-one test runner, bulk select/delete, add via key or
// OAuth); Available Models card (chips with copy/test).
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { api } from "@/lib/api";
import { toast } from "@/lib/state";
import Badge from "@/components/Badge.vue";
import Toggle from "@/components/Toggle.vue";
import Modal from "@/components/Modal.vue";
import ConfirmModal from "@/components/ConfirmModal.vue";

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
  refreshToken?: string;
  expiresAt?: string;
  modelLock_glm?: unknown;
  providerSpecificData?: Record<string, unknown>;
  [key: string]: unknown;
}
interface Pool { id: string; name?: string; proxyUrl?: string; isActive?: boolean }
interface CatalogModel { id: string; name?: string; kind?: string }

const route = useRoute();
const router = useRouter();
const providerId = computed(() => route.params.id as string);

const conns = ref<Connection[]>([]);
const pools = ref<Pool[]>([]);
const models = ref<CatalogModel[]>([]);
const loading = ref(true);

const editing = ref<Connection | null>(null);
const editName = ref("");
const editApiKey = ref("");
const savingEdit = ref(false);

const showAddKey = ref(false);
const addKeyName = ref("");
const addKeyApi = ref("");
const creatingKey = ref(false);

const showOAuth = ref(false);
const oauthProvider = ref("claude");
const oauthState = ref("");
const oauthAuthUrl = ref("");
const oauthCode = ref("");
const oauthBusy = ref(false);

const confirmDelete = ref<Connection | null>(null);
const selected = ref(new Set<string>());
const testingId = ref<string | null>(null);
const oneByOneRunning = ref(false);
const oneByOneSummary = ref<{ done: number; pass: number; fail: number } | null>(null);
const stopOneByOne = ref(false);

const proxyMenuFor = ref<string | null>(null);

async function load() {
  loading.value = true;
  try {
    const [connsRes, poolsRes, modelsRes] = await Promise.all([
      api.get<{ connections: Connection[] }>("/api/providers"),
      api.get<{ pools: Pool[] }>("/api/proxy-pools"),
      fetch("/api/models", { credentials: "include" }).then((r) => r.json() as Promise<{ providers: { provider: string; models: CatalogModel[] }[] }>),
    ]);
    conns.value = (connsRes.connections ?? []).filter((c) => c.provider === providerId.value);
    pools.value = poolsRes.pools ?? [];
    models.value = modelsRes.providers.find((p) => p.provider === providerId.value)?.models
      ?.filter((m) => m.kind === "llm") ?? [];
  } catch {
    toast.error("Failed to load provider detail");
  } finally {
    loading.value = false;
  }
}
onMounted(load);

const providerName = computed(() => providerId.value);
const iconSrc = (id: string) => `/providers/${id}.png`;

const hasCooldown = (c: Connection) =>
  Object.entries(c).some(([k, v]) => k.startsWith("modelLock_") && typeof v === "string" && v && new Date(v).getTime() > Date.now());

const proxyOf = (c: Connection): string | null => {
  const psd = c.providerSpecificData ?? {};
  const poolId = psd["connectionProxyPoolId"] as string | undefined;
  if (poolId && poolId !== "none") {
    const pool = pools.value.find((x) => x.id === poolId);
    if (pool) return pool.name ?? "pool";
  }
  if (psd["connectionProxyEnabled"] && psd["connectionProxyUrl"]) return "custom URL";
  return null;
};

async function move(c: Connection, dir: -1 | 1) {
  const sorted = [...conns.value].sort((a, b) => (a.priority ?? 99) - (b.priority ?? 99));
  const idx = sorted.findIndex((x) => x.id === c.id);
  const swap = sorted[idx + dir];
  if (!swap) return;
  try {
    await api.put(`/api/providers/${c.id}`, { priority: swap.priority ?? 99 });
    await load();
  } catch {
    toast.error("Failed to reorder");
  }
}

async function toggleActive(c: Connection) {
  const next = !(c.isActive !== false);
  try {
    await api.put(`/api/providers/${c.id}`, { isActive: next });
    await load();
  } catch {
    toast.error("Failed to update connection");
  }
}

function openEdit(c: Connection) {
  editing.value = c;
  editName.value = c.name ?? "";
  editApiKey.value = "";
}

async function saveEdit() {
  if (!editing.value) return;
  savingEdit.value = true;
  const patch: Record<string, unknown> = { name: editName.value.trim() };
  if (editApiKey.value.trim()) patch.apiKey = editApiKey.value.trim();
  try {
    await api.put(`/api/providers/${editing.value.id}`, patch);
    toast.success("Connection updated");
    editing.value = null;
    await load();
  } catch {
    toast.error("Failed to update connection");
  } finally {
    savingEdit.value = false;
  }
}

async function remove(c: Connection) {
  confirmDelete.value = c;
}

async function doRemove() {
  const c = confirmDelete.value;
  confirmDelete.value = null;
  if (!c) return;
  try {
    await api.del(`/api/providers/${c.id}`);
    toast.success("Connection deleted");
    if (selected.value.has(c.id)) {
      const s = new Set(selected.value);
      s.delete(c.id);
      selected.value = s;
    }
    await load();
  } catch {
    toast.error("Failed to delete connection");
  }
}

async function deleteSelected() {
  const ids = [...selected.value];
  for (const id of ids) {
    try {
      await api.del(`/api/providers/${id}`);
    } catch { /* continue */ }
  }
  selected.value = new Set();
  toast.success(`${ids.length} connection(s) deleted`);
  await load();
}

function toggleSelect(id: string) {
  const s = new Set(selected.value);
  if (s.has(id)) s.delete(id);
  else s.add(id);
  selected.value = s;
}

async function testOne(c: Connection): Promise<boolean> {
  testingId.value = c.id;
  try {
    const r = await api.post<{ valid: boolean }>(`/api/providers/${c.id}/test`);
    return !!r.valid;
  } catch {
    return false;
  } finally {
    testingId.value = null;
  }
}

async function testSingle(c: Connection) {
  const ok = await testOne(c);
  await load();
  if (ok) toast.success(`"${c.name ?? providerId.value}" is working!`);
  else toast.error(`"${c.name ?? providerId.value}" failed`, "Test Connection");
}

async function testOneByOne() {
  oneByOneRunning.value = true;
  stopOneByOne.value = false;
  const summary = { done: 0, pass: 0, fail: 0 };
  oneByOneSummary.value = { ...summary };
  for (const c of conns.value.filter((x) => x.isActive !== false)) {
    if (stopOneByOne.value) break;
    if (await testOne(c)) summary.pass += 1;
    else summary.fail += 1;
    summary.done += 1;
    oneByOneSummary.value = { ...summary };
    await new Promise((r) => setTimeout(r, 1000));
  }
  oneByOneRunning.value = false;
  await load();
}

async function assignProxy(c: Connection, poolId: string | null) {
  const psd: Record<string, unknown> = { ...(c.providerSpecificData ?? {}) };
  psd["connectionProxyPoolId"] = poolId ?? "none";
  try {
    await api.put(`/api/providers/${c.id}`, { providerSpecificData: psd });
    toast.success(poolId ? "Proxy pool assigned" : "Proxy unbound");
    proxyMenuFor.value = null;
    await load();
  } catch {
    toast.error("Failed to assign proxy");
  }
}

async function createKeyConn() {
  if (!addKeyName.value.trim() || !addKeyApi.value.trim()) return;
  creatingKey.value = true;
  try {
    await api.post("/api/providers", { provider: providerId.value, name: addKeyName.value.trim(), apiKey: addKeyApi.value.trim() });
    toast.success("Connection added");
    showAddKey.value = false;
    addKeyName.value = "";
    addKeyApi.value = "";
    await load();
  } catch {
    toast.error("Failed to add connection");
  } finally {
    creatingKey.value = false;
  }
}

async function startOAuth() {
  oauthBusy.value = true;
  try {
    const r = await api.post<{ authUrl: string; state: string }>(`/api/oauth/${providerId.value}/start`);
    oauthState.value = r.state;
    oauthAuthUrl.value = r.authUrl;
    window.open(r.authUrl, "_blank");
  } catch {
    toast.error("This provider has no native OAuth here");
  } finally {
    oauthBusy.value = false;
  }
}

async function exchangeOAuth() {
  if (!oauthCode.value.trim() || !oauthState.value) return;
  oauthBusy.value = true;
  try {
    await api.post(`/api/oauth/${providerId.value}/exchange`, { state: oauthState.value, code: oauthCode.value.trim() });
    toast.success(`${providerId.value} account connected!`);
    showOAuth.value = false;
    oauthAuthUrl.value = "";
    oauthCode.value = "";
    await load();
  } catch (e) {
    toast.error(e instanceof Error && e.message ? e.message : "Exchange failed");
  } finally {
    oauthBusy.value = false;
  }
}

function copyText(text: string) {
  navigator.clipboard?.writeText(text).then(() => toast.success(`Copied ${text}`), () => toast.error("Failed to copy"));
}

async function refreshToken(c: Connection) {
  toast.info(`Refreshing ${c.name ?? providerId.value} token…`);
  try {
    const r = await api.post<{ expiresAt: string }>(`/api/oauth/${providerId.value}/refresh`, { connectionId: c.id });
    toast.success(`Token refreshed — valid until ${new Date(r.expiresAt).toLocaleString()}`);
    await load();
  } catch (e) {
    toast.error(e instanceof Error && e.message ? e.message : "Refresh failed", "Token Refresh");
  }
}

const isOAuthProvider = computed(() =>
  ["claude", "codex", "antigravity", "glm", "kiro", "cursor", "iflow", "qwen", "gemini-cli", "xai"].includes(providerId.value),
);
</script>

<template>
  <div class="fade-in flex flex-col gap-6" style="max-width: 1100px">
    <!-- Header -->
    <div>
      <router-link to="/dashboard/providers" style="font-family: var(--font-body); font-size: 0.9rem; color: var(--color-text-muted); text-decoration: none">
        ← Back to Providers
      </router-link>
      <div style="display: flex; align-items: center; gap: 1rem; margin-top: 0.5rem">
        <img :src="iconSrc(providerId)" alt="" style="width: 48px; height: 48px; object-fit: contain" />
        <div>
          <h1 style="font-size: 1.6rem; margin: 0">{{ providerName }}</h1>
          <span style="font-family: var(--font-body); color: var(--color-text-muted); font-size: 0.95rem">
            {{ conns.length }} connection(s)
          </span>
        </div>
      </div>
    </div>

    <p v-if="loading" style="font-family: var(--font-body)">Loading…</p>

    <template v-if="!loading">
      <!-- Connections card -->
      <div class="kid-card">
        <div style="display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 0.5rem; margin-bottom: 0.75rem">
          <h2 style="font-size: 1.25rem; margin: 0">Connections</h2>
          <div style="display: flex; gap: 0.4rem; flex-wrap: wrap">
            <button
              v-if="selected.size > 0"
              class="kid-btn"
              style="padding: 0.25rem 0.6rem; font-size: 0.82rem; background: var(--color-danger); color: #fff"
              @click="deleteSelected"
            >
              Delete Selected ({{ selected.size }})
            </button>
            <button
              v-if="conns.length > 0"
              class="kid-btn kid-btn--accent"
              style="padding: 0.25rem 0.6rem; font-size: 0.82rem"
              :disabled="oneByOneRunning"
              @click="oneByOneRunning ? (stopOneByOne = true) : testOneByOne()"
            >
              {{ oneByOneRunning ? "Stop" : "Test One-by-One" }}
            </button>
            <button v-if="isOAuthProvider" class="kid-btn kid-btn--primary" style="padding: 0.25rem 0.6rem; font-size: 0.82rem" @click="showOAuth = true">
              <span class="material-symbols-outlined" style="font-size: 15px">key</span> OAuth
            </button>
            <button class="kid-btn kid-btn--primary" style="padding: 0.25rem 0.6rem; font-size: 0.82rem" @click="showAddKey = true">
              <span class="material-symbols-outlined" style="font-size: 15px">vpn_key</span> API Key
            </button>
          </div>
        </div>

        <div v-if="oneByOneSummary" style="font-family: var(--font-body); font-size: 0.85rem; color: var(--color-text-muted); margin-bottom: 0.6rem">
          One-by-one: {{ oneByOneSummary.done }} done · {{ oneByOneSummary.pass }} passed · {{ oneByOneSummary.fail }} failed
        </div>

        <div v-if="conns.length === 0" style="text-align: center; padding: 2rem 1rem">
          <span class="material-symbols-outlined" style="font-size: 36px; color: var(--color-text-muted)">{{ isOAuthProvider ? "lock" : "vpn_key" }}</span>
          <p style="font-family: var(--font-body); margin: 0.4rem 0 0.8rem">No connections yet</p>
        </div>

        <div v-for="c in [...conns].sort((a, b) => (a.priority ?? 99) - (b.priority ?? 99))" :key="c.id">
          <div style="display: flex; align-items: center; gap: 0.4rem; padding: 0.15rem 0">
            <input
              type="checkbox"
              :checked="selected.has(c.id)"
              @change="toggleSelect(c.id)"
            />
          </div>
          <div
            style="display: flex; align-items: center; gap: 0.6rem; padding: 0.5rem 0.2rem 0.65rem; border-top: 1px solid var(--color-border-subtle); flex-wrap: wrap"
            :style="c.isActive === false ? { opacity: 0.55 } : {}"
          >
            <div style="display: flex; flex-direction: column">
              <button class="kid-btn" style="padding: 0 0.25rem; border: none; background: none; font-size: 12px" @click="move(c, -1)">▲</button>
              <button class="kid-btn" style="padding: 0 0.25rem; border: none; background: none; font-size: 12px" @click="move(c, 1)">▼</button>
            </div>
            <span class="material-symbols-outlined" style="font-size: 16px; color: var(--color-text-muted)">
              {{ c.authType === "oauth" ? "lock" : "vpn_key" }}
            </span>
            <div style="min-width: 160px">
              <strong style="display: block; font-size: 0.98rem">{{ c.name ?? c.email ?? c.id.slice(0, 8) }}</strong>
              <span v-if="c.email" style="font-family: var(--font-body); font-size: 0.8rem; color: var(--color-text-muted)">{{ c.email }}</span>
            </div>
            <Badge :variant="c.isActive !== false ? (c.testStatus === 'active' ? 'success' : c.testStatus === 'error' ? 'danger' : 'neutral') : 'neutral'" size="sm" dot>
              {{ c.isActive === false ? "disabled" : c.testStatus ?? "unknown" }}
            </Badge>
            <Badge v-if="hasCooldown(c)" variant="warning" size="sm">cooldown</Badge>
            <Badge v-if="proxyOf(c)" variant="success" size="sm">Proxy: {{ proxyOf(c) }}</Badge>
            <span style="font-family: var(--font-body); font-size: 0.8rem; color: var(--color-text-muted)">#{{ c.priority ?? 99 }}</span>
            <span v-if="c.lastError" style="font-family: var(--font-body); font-size: 0.82rem; color: var(--color-danger); flex: 1; min-width: 140px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap" :title="String(c.lastError)">
              {{ c.lastError }}
            </span>
            <div style="margin-left: auto; display: flex; align-items: center; gap: 0.4rem; flex-wrap: wrap">
              <div v-if="pools.length > 0" style="position: relative">
                <button class="kid-btn" style="padding: 0.2rem 0.5rem; font-size: 0.78rem" @click="proxyMenuFor = proxyMenuFor === c.id ? null : c.id">
                  <span class="material-symbols-outlined" style="font-size: 14px">hub</span> Proxy
                </button>
                <div
                  v-if="proxyMenuFor === c.id"
                  class="kid-card"
                  style="position: absolute; right: 0; top: 110%; z-index: 50; min-width: 200px; padding: 0.4rem"
                >
                  <button class="kid-btn" style="width: 100%; border: none; text-align: left" @click="assignProxy(c, null)">None</button>
                  <button
                    v-for="pool in pools"
                    :key="pool.id"
                    class="kid-btn"
                    style="width: 100%; border: none; text-align: left"
                    :disabled="pool.isActive === false"
                    @click="assignProxy(c, pool.id)"
                  >
                    {{ pool.name ?? pool.id }}{{ pool.isActive === false ? " (inactive)" : "" }}
                  </button>
                </div>
              </div>
              <button v-if="c.authType === 'oauth'" class="kid-btn" style="padding: 0.2rem 0.4rem" title="Refresh token" @click="refreshToken(c)">
                <span class="material-symbols-outlined" style="font-size: 15px">autorenew</span>
              </button>
              <button class="kid-btn kid-btn--accent" style="padding: 0.2rem 0.4rem" :disabled="testingId === c.id" @click="testSingle(c)">
                <span class="material-symbols-outlined" :class="{ 'animate-spin': testingId === c.id }" style="font-size: 15px">
                  {{ testingId === c.id ? "progress_activity" : "science" }}
                </span>
              </button>
              <button class="kid-btn" style="padding: 0.2rem 0.4rem" @click="openEdit(c)">
                <span class="material-symbols-outlined" style="font-size: 15px">edit</span>
              </button>
              <button class="kid-btn" style="padding: 0.2rem 0.4rem; background: var(--color-danger); color: #fff" @click="remove(c)">
                <span class="material-symbols-outlined" style="font-size: 15px">delete</span>
              </button>
              <Toggle :checked="c.isActive !== false" @change="() => toggleActive(c)" />
            </div>
          </div>
        </div>
      </div>

      <!-- Available Models card -->
      <div class="kid-card">
        <h2 style="font-size: 1.25rem; margin: 0 0 0.75rem">Available Models ({{ models.length }})</h2>
        <div v-if="models.length === 0" style="font-family: var(--font-body); color: var(--color-text-muted); font-size: 0.92rem">
          No models listed in the catalog for this provider.
        </div>
        <div v-else style="display: flex; flex-wrap: wrap; gap: 0.5rem">
          <span
            v-for="m in models"
            :key="m.id"
            class="kid-card"
            style="display: inline-flex; align-items: center; gap: 0.4rem; padding: 0.25rem 0.55rem; box-shadow: none"
          >
            <code style="font-size: 0.8rem">{{ providerId }}/{{ m.id }}</code>
            <span v-if="m.name && m.name !== m.id" style="font-family: var(--font-body); font-size: 0.75rem; color: var(--color-text-muted)">{{ m.name }}</span>
            <button class="kid-btn" style="padding: 0 0.2rem; border: none; background: none" title="Copy" @click="copyText(`${providerId}/${m.id}`)">
              <span class="material-symbols-outlined" style="font-size: 13px">content_copy</span>
            </button>
          </span>
        </div>
      </div>
    </template>

    <!-- Edit modal -->
    <Modal v-if="editing" @close="editing = null">
      <form @submit.prevent="saveEdit">
        <h3 style="font-size: 1.25rem; margin: 0 0 0.75rem">Edit Connection</h3>
        <div style="display: grid; gap: 0.6rem">
          <div>
            <label style="font-family: var(--font-body); font-size: 0.9rem">Name</label>
            <input v-model="editName" class="kid-input" :disabled="savingEdit" />
          </div>
          <div>
            <label style="font-family: var(--font-body); font-size: 0.9rem">New API Key (leave blank to keep)</label>
            <input v-model="editApiKey" type="password" class="kid-input" :disabled="savingEdit" />
          </div>
        </div>
        <div style="display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 1.25rem">
          <button type="button" class="kid-btn" :disabled="savingEdit" @click="editing = null">Cancel</button>
          <button type="submit" class="kid-btn kid-btn--primary" :disabled="savingEdit || !editName.trim()">
            {{ savingEdit ? "Saving…" : "Save" }}
          </button>
        </div>
      </form>
    </Modal>

    <!-- Add API key modal -->
    <Modal v-if="showAddKey" @close="showAddKey = false">
      <form @submit.prevent="createKeyConn">
        <h3 style="font-size: 1.25rem; margin: 0 0 0.75rem">Add API Key Connection</h3>
        <div style="display: grid; gap: 0.6rem">
          <div>
            <label style="font-family: var(--font-body); font-size: 0.9rem">Name</label>
            <input v-model="addKeyName" class="kid-input" placeholder="e.g. Primary" :disabled="creatingKey" />
          </div>
          <div>
            <label style="font-family: var(--font-body); font-size: 0.9rem">API Key</label>
            <input v-model="addKeyApi" type="password" class="kid-input" placeholder="sk-…" :disabled="creatingKey" />
          </div>
        </div>
        <div style="display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 1.25rem">
          <button type="button" class="kid-btn" :disabled="creatingKey" @click="showAddKey = false">Cancel</button>
          <button type="submit" class="kid-btn kid-btn--primary" :disabled="creatingKey || !addKeyName.trim() || !addKeyApi.trim()">
            {{ creatingKey ? "Adding…" : "Add" }}
          </button>
        </div>
      </form>
    </Modal>

    <!-- OAuth modal -->
    <Modal v-if="showOAuth" width="480px" @close="showOAuth = false">
      <h3 style="font-size: 1.25rem; margin: 0 0 0.75rem">OAuth Login — {{ providerId }}</h3>
      <div style="display: grid; gap: 0.6rem">
        <button class="kid-btn kid-btn--primary" :disabled="oauthBusy" @click="startOAuth">
          <span class="material-symbols-outlined" style="font-size: 16px">open_in_new</span>
          {{ oauthAuthUrl ? "Re-open authorize page" : "1. Open authorize page" }}
        </button>
        <div v-if="oauthAuthUrl" style="font-family: var(--font-body); font-size: 0.85rem; color: var(--color-text-muted)">
          Authorize in the browser, then paste the code (claude) or the full callback URL below.
        </div>
        <textarea v-if="oauthAuthUrl" v-model="oauthCode" class="kid-input" rows="3" placeholder="code or callback URL…" :disabled="oauthBusy" />
      </div>
      <div style="display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 1.25rem">
        <button class="kid-btn" :disabled="oauthBusy" @click="showOAuth = false">Cancel</button>
        <button class="kid-btn kid-btn--primary" :disabled="oauthBusy || !oauthAuthUrl || !oauthCode.trim()" @click="exchangeOAuth">
          {{ oauthBusy ? "Working…" : "2. Connect" }}
        </button>
      </div>
    </Modal>

    <ConfirmModal
      v-if="confirmDelete"
      title="Delete Connection"
      :message="`Delete '${confirmDelete.name ?? confirmDelete.id.slice(0, 8)}'? This cannot be undone.`"
      confirm-label="Delete"
      danger
      @close="confirmDelete = null"
      @confirm="doRemove"
    />
  </div>
</template>
