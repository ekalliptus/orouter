// Providers: search, add connection, edit modal, enable/disable, priority
// up/down (server renumbers), per-connection test + test-all, delete.
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { api } from "@/lib/api";
import { toast } from "@/lib/state";
import Badge from "@/components/Badge.vue";
import Toggle from "@/components/Toggle.vue";

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

const PROVIDER_CATEGORIES: Record<string, string[]> = {
  "API Key Providers": [
    "openrouter", "openai", "deepseek", "groq", "mistral", "xai",
    "anthropic", "gemini", "together", "fireworks", "siliconflow", "cohere",
    "nebius", "cerebras", "chutes", "perplexity",
  ],
  "Free / Open Providers": ["ollama-local", "kilo", "free-tier"],
};

const PROVIDER_OPTIONS = [
  { value: "openrouter", label: "OpenRouter (Recommended)" },
  { value: "openai", label: "OpenAI" },
  { value: "deepseek", label: "DeepSeek" },
  { value: "groq", label: "Groq" },
  { value: "anthropic", label: "Anthropic Claude" },
  { value: "gemini", label: "Google Gemini" },
  { value: "mistral", label: "Mistral AI" },
  { value: "xai", label: "xAI (Grok)" },
  { value: "together", label: "Together AI" },
  { value: "fireworks", label: "Fireworks AI" },
  { value: "siliconflow", label: "SiliconFlow" },
  { value: "cohere", label: "Cohere" },
  { value: "tokenrouter", label: "TokenRouter" },
  { value: "ollama-local", label: "Ollama (Local)" },
];

const providers = ref<Connection[]>([]);
const loading = ref(true);
const search = ref("");
const showAddForm = ref(false);
const selectedProvider = ref("openrouter");
const apiKey = ref("");
const name = ref("");
const creating = ref(false);
const testingId = ref<string | null>(null);
const testAllRunning = ref(false);

const editing = ref<Connection | null>(null);
const editName = ref("");
const editEmail = ref("");
const editApiKey = ref("");
const savingEdit = ref(false);

// OAuth login flow state
const OAUTH_PROVIDERS = ["claude", "codex", "antigravity"];
const showOAuthModal = ref(false);
const oauthProvider = ref("claude");
const oauthState = ref("");
const oauthAuthUrl = ref("");
const oauthCodeInput = ref("");
const oauthBusy = ref(false);
const oauthError = ref<string | null>(null);

async function load() {
  loading.value = true;
  try {
    const data = await api.get<{ connections: Connection[] }>("/api/providers");
    providers.value = data.connections ?? [];
  } catch {
    toast.error("Failed to fetch providers");
  } finally {
    loading.value = false;
  }
}
onMounted(load);

const filtered = computed(() => {
  const q = search.value.trim().toLowerCase();
  if (!q) return providers.value;
  return providers.value.filter(
    (p) => (p.name ?? "").toLowerCase().includes(q) || p.provider.toLowerCase().includes(q),
  );
});

const activeCount = computed(() => providers.value.filter((p) => p.isActive !== false && p.testStatus === "active").length);
const errorCount = computed(() => providers.value.filter((p) => p.testStatus === "error").length);

function iconSrc(p: Connection) {
  return `/providers/${p.provider}.png`;
}

async function createProvider() {
  if (!selectedProvider.value.trim() || !apiKey.value.trim() || !name.value.trim()) return;
  creating.value = true;
  try {
    await api.post("/api/providers", {
      provider: selectedProvider.value.trim(),
      apiKey: apiKey.value.trim(),
      name: name.value.trim(),
    });
    toast.success(`Added provider "${name.value.trim()}"`);
    apiKey.value = "";
    name.value = "";
    showAddForm.value = false;
    await load();
  } catch {
    toast.error("Failed to add provider");
  } finally {
    creating.value = false;
  }
}

function openEdit(p: Connection) {
  editing.value = p;
  editName.value = p.name ?? "";
  editEmail.value = typeof p.email === "string" ? p.email : "";
  editApiKey.value = "";
}

async function saveEdit() {
  if (!editing.value) return;
  savingEdit.value = true;
  const patch: Record<string, unknown> = { name: editName.value.trim() };
  if (editEmail.value.trim()) patch.email = editEmail.value.trim();
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

async function toggleActive(p: Connection) {
  const next = !(p.isActive !== false);
  try {
    await api.put(`/api/providers/${p.id}`, { isActive: next });
    toast.success(`"${p.name ?? p.provider}" ${next ? "enabled" : "disabled"}`);
    await load();
  } catch {
    toast.error("Failed to update connection");
  }
}

async function move(p: Connection, dir: -1 | 1) {
  const siblings = providers.value
    .filter((x) => x.provider === p.provider)
    .sort((a, b) => (a.priority ?? 99) - (b.priority ?? 99));
  const idx = siblings.findIndex((x) => x.id === p.id);
  const swap = siblings[idx + dir];
  if (!swap) return;
  try {
    await api.put(`/api/providers/${p.id}`, { priority: swap.priority ?? 99 });
    await load();
  } catch {
    toast.error("Failed to reorder");
  }
}

async function testOne(p: Connection): Promise<boolean> {
  testingId.value = p.id;
  try {
    const r = await api.post<{ valid: boolean; error: string | null }>(`/api/providers/${p.id}/test`);
    return !!r.valid;
  } catch {
    return false;
  } finally {
    testingId.value = null;
  }
}

async function handleTest(p: Connection) {
  const ok = await testOne(p);
  await load();
  if (ok) toast.success(`"${p.name ?? p.provider}" connection is working!`);
  else toast.error(`"${p.name ?? p.provider}" failed`, "Test Connection");
}

async function testAll() {
  testAllRunning.value = true;
  let pass = 0;
  let fail = 0;
  for (const p of filtered.value) {
    if (await testOne(p)) pass += 1;
    else fail += 1;
  }
  testAllRunning.value = false;
  await load();
  toast.success(`Test all done: ${pass} pass, ${fail} fail`, "Batch Test");
}

async function remove(p: Connection) {
  if (!confirm(`Delete connection "${p.name ?? p.provider}"?`)) return;
  try {
    await api.del(`/api/providers/${p.id}`);
    toast.success("Connection deleted");
    await load();
  } catch {
    toast.error("Failed to delete connection");
  }
}

// ---- OAuth login (claude / codex / antigravity) ----

async function startOAuth() {
  oauthBusy.value = true;
  oauthError.value = null;
  oauthAuthUrl.value = "";
  oauthState.value = "";
  oauthCodeInput.value = "";
  try {
    const r = await api.post<{ authUrl: string; state: string }>(`/api/oauth/${oauthProvider.value}/start`);
    oauthState.value = r.state;
    oauthAuthUrl.value = r.authUrl;
    window.open(r.authUrl, "_blank");
  } catch {
    oauthError.value = "Failed to start OAuth flow";
  } finally {
    oauthBusy.value = false;
  }
}

async function exchangeOAuth() {
  if (!oauthCodeInput.value.trim() || !oauthState.value) return;
  oauthBusy.value = true;
  oauthError.value = null;
  try {
    await api.post(`/api/oauth/${oauthProvider.value}/exchange`, {
      state: oauthState.value,
      code: oauthCodeInput.value.trim(),
    });
    toast.success(`${oauthProvider.value} account connected!`);
    showOAuthModal.value = false;
    await load();
  } catch (e) {
    oauthError.value = e instanceof Error && e.message ? e.message : "Exchange failed";
  } finally {
    oauthBusy.value = false;
  }
}

function isOAuth(p: Connection) {
  return p.authType === "oauth";
}

function expiryInfo(p: Connection) {
  const exp = typeof p.expiresAt === "string" ? p.expiresAt : null;
  if (!exp) return null;
  const secs = Math.floor(new Date(exp).getTime() / 1000) - Math.floor(Date.now() / 1000);
  return { expired: secs <= 0, inHours: Math.round(secs / 360) / 10 };
}

async function refreshOAuth(p: Connection) {
  toast.info(`Refreshing ${p.name ?? p.provider} token…`);
  try {
    const r = await api.post<{ expiresAt: string }>(`/api/oauth/${p.provider}/refresh`, { connectionId: p.id });
    toast.success(`Token refreshed — valid until ${new Date(r.expiresAt).toLocaleString()}`);
    await load();
  } catch (e) {
    toast.error(e instanceof Error && e.message ? e.message : "Refresh failed", "Token Refresh");
  }
}
</script>

<template>
  <div class="fade-in flex flex-col gap-6" style="max-width: 1000px">
    <!-- Actions row -->
    <div style="display: flex; justify-content: flex-end; align-items: center; flex-wrap: wrap; gap: 0.5rem">
      <Badge variant="success" dot>{{ activeCount }} Active</Badge>
      <Badge v-if="errorCount > 0" variant="danger" dot>{{ errorCount }} Error</Badge>
      <button class="kid-btn kid-btn--accent" style="padding: 0.3rem 0.75rem" :disabled="testAllRunning || filtered.length === 0" @click="testAll">
        <span class="material-symbols-outlined" style="font-size: 16px">science</span>
        {{ testAllRunning ? "Testing all…" : "Test All" }}
      </button>
      <button class="kid-btn kid-btn--primary" style="padding: 0.3rem 0.75rem" @click="showAddForm = !showAddForm">
        {{ showAddForm ? "✕ Close Form" : "＋ Add Provider" }}
      </button>
      <button class="kid-btn" style="padding: 0.3rem 0.75rem" @click="showOAuthModal = true">
        <span class="material-symbols-outlined" style="font-size: 16px">key</span> OAuth Login
      </button>
    </div>

    <input v-model="search" class="kid-input" placeholder="Search connections…" style="max-width: 360px" />

    <!-- Add form -->
    <div v-if="showAddForm" class="kid-card" style="background-color: var(--color-brand-50); border-color: var(--color-brand-500)">
      <h2 style="font-size: 1.25rem; margin: 0 0 1rem">＋ Add New AI Provider Connection</h2>
      <form style="display: grid; gap: 0.75rem" @submit.prevent="createProvider">
        <div style="display: grid; gap: 0.75rem; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr))">
          <div>
            <label style="display: block; font-family: var(--font-body); margin-bottom: 0.2rem; font-weight: 700">Provider</label>
            <select v-model="selectedProvider" class="kid-input" :disabled="creating">
              <option v-for="o in PROVIDER_OPTIONS" :key="o.value" :value="o.value">{{ o.label }}</option>
            </select>
          </div>
          <div>
            <label style="display: block; font-family: var(--font-body); margin-bottom: 0.2rem; font-weight: 700">Connection Name</label>
            <input v-model="name" class="kid-input" placeholder="e.g. Primary OpenRouter" :disabled="creating" />
          </div>
          <div style="grid-column: 1 / -1">
            <label style="display: block; font-family: var(--font-body); margin-bottom: 0.2rem; font-weight: 700">API Key / Token</label>
            <input v-model="apiKey" type="password" class="kid-input" placeholder="sk-..." :disabled="creating" />
          </div>
        </div>
        <div style="display: flex; justify-content: flex-end; gap: 0.5rem">
          <button type="button" class="kid-btn" :disabled="creating" @click="showAddForm = false">Cancel</button>
          <button type="submit" class="kid-btn kid-btn--primary" :disabled="creating || !name.trim() || !apiKey.trim()">
            {{ creating ? "Adding…" : "Save Connection" }}
          </button>
        </div>
      </form>
    </div>

    <p v-if="loading" style="font-family: var(--font-body)">Loading providers…</p>

    <div v-if="!loading && filtered.length === 0" class="kid-card" style="text-align: center; padding: 3rem 1rem">
      <h2 style="margin: 0.5rem 0">{{ providers.length === 0 ? "No AI Providers Configured" : "No matches" }}</h2>
      <p style="font-family: var(--font-body); color: var(--color-text-muted)">
        {{ providers.length === 0 ? "Add an OpenRouter, OpenAI, or DeepSeek API key to start routing AI requests!" : "Try a different search term." }}
      </p>
      <button v-if="providers.length === 0" class="kid-btn kid-btn--primary" style="margin-top: 1rem" @click="showAddForm = true">
        ＋ Add Your First Provider
      </button>
    </div>

    <div v-for="(cat, catName) in PROVIDER_CATEGORIES" :key="catName" style="margin-bottom: 1.5rem">
      <template v-if="filtered.filter((p) => cat.includes(p.provider) || catName.includes('API Key')).length > 0">
        <h2 style="font-size: 1.15rem; margin: 0 0 0.75rem; color: var(--color-text-muted)">{{ catName }}</h2>
        <div style="display: grid; gap: 1rem; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr))">
          <div
            v-for="p in filtered.filter((p) => cat.includes(p.provider) || catName.includes('API Key'))"
            :key="p.id"
            class="kid-card"
            :style="p.isActive === false ? { opacity: 0.55 } : undefined"
          >
            <div style="display: flex; justify-content: space-between; align-items: flex-start; gap: 0.5rem">
              <div style="display: flex; gap: 0.75rem; align-items: center">
                <img :src="iconSrc(p)" alt="" style="width: 36px; height: 36px; object-fit: contain" @error="($event.target as HTMLImageElement).style.visibility = 'hidden'" />
                <div>
                  <strong style="font-size: 1.2rem; display: block">{{ p.name ?? p.provider }}</strong>
                  <span style="font-family: var(--font-body); color: var(--color-text-muted); font-size: 0.9rem">
                    {{ p.provider }} · priority {{ p.priority ?? 99 }}
                  </span>
                </div>
              </div>
              <Badge :variant="p.testStatus === 'active' ? 'success' : p.testStatus === 'error' ? 'danger' : 'neutral'" size="sm" dot>
                {{ p.testStatus ?? "unknown" }}
              </Badge>
            </div>

                        <div
                          v-if="p.lastError"
                          style="font-family: var(--font-body); font-size: 0.85rem; color: var(--color-danger); margin-top: 0.5rem; background: var(--color-bg-alt); padding: 0.3rem 0.5rem; border: 1px solid var(--nb-border)"
                        >
                          {{ p.lastError }}
                        </div>
                        <div v-if="isOAuth(p) && expiryInfo(p)" style="font-family: var(--font-body); font-size: 0.85rem; margin-top: 0.5rem" :style="expiryInfo(p)!.expired ? { color: 'var(--color-danger)' } : { color: 'var(--color-text-muted)' }">
                          <span class="material-symbols-outlined" style="font-size: 13px; vertical-align: middle">schedule</span>
                          token {{ expiryInfo(p)!.expired ? "EXPIRED" : `expires in ~${expiryInfo(p)!.inHours}h` }}
                        </div>

            <div style="display: flex; align-items: center; justify-content: space-between; margin-top: 0.85rem; gap: 0.5rem; flex-wrap: wrap">
              <div style="display: flex; align-items: center; gap: 0.4rem">
                <button class="kid-btn" style="padding: 0.2rem 0.5rem; font-size: 0.8rem" title="Raise priority" @click="move(p, -1)">▲</button>
                <button class="kid-btn" style="padding: 0.2rem 0.5rem; font-size: 0.8rem" title="Lower priority" @click="move(p, 1)">▼</button>
                <Toggle :checked="p.isActive !== false" @change="() => toggleActive(p)" />
              </div>
              <div style="display: flex; gap: 0.4rem">
                <button v-if="isOAuth(p)" class="kid-btn" style="padding: 0.25rem 0.5rem" title="Refresh token" @click="refreshOAuth(p)">
                  <span class="material-symbols-outlined" style="font-size: 16px">autorenew</span>
                </button>
                <button class="kid-btn" style="padding: 0.25rem 0.5rem" @click="openEdit(p)">
                  <span class="material-symbols-outlined" style="font-size: 16px">edit</span>
                </button>
                <button class="kid-btn kid-btn--accent" style="padding: 0.25rem 0.5rem" :disabled="testingId === p.id" @click="handleTest(p)">
                  <span class="material-symbols-outlined" :class="{ 'animate-spin': testingId === p.id }" style="font-size: 16px">
                    {{ testingId === p.id ? "progress_activity" : "science" }}
                  </span>
                </button>
                <button class="kid-btn" style="padding: 0.25rem 0.5rem; background: var(--color-danger); color: #fff" @click="remove(p)">
                  <span class="material-symbols-outlined" style="font-size: 16px">delete</span>
                </button>
              </div>
            </div>
          </div>
        </div>
      </template>
    </div>

    <!-- Edit modal -->
    <div
      v-if="editing"
      style="position: fixed; inset: 0; background: rgba(0,0,0,0.5); z-index: 100; display: flex; align-items: center; justify-content: center; padding: 1rem"
      @click.self="editing = null"
    >
      <form class="kid-card kid-wobble" style="width: min(420px, 100%); background: var(--color-surface)" @submit.prevent="saveEdit">
        <h3 style="font-size: 1.3rem; margin: 0 0 0.75rem">Edit Connection</h3>
        <div style="display: grid; gap: 0.6rem">
          <div>
            <label style="font-family: var(--font-body); font-size: 0.9rem">Name</label>
            <input v-model="editName" class="kid-input" :disabled="savingEdit" />
          </div>
          <div>
            <label style="font-family: var(--font-body); font-size: 0.9rem">Email (optional)</label>
            <input v-model="editEmail" type="email" class="kid-input" :disabled="savingEdit" />
          </div>
          <div>
            <label style="font-family: var(--font-body); font-size: 0.9rem">New API Key (leave blank to keep)</label>
            <input v-model="editApiKey" type="password" class="kid-input" placeholder="sk-…" :disabled="savingEdit" />
          </div>
        </div>
        <div style="display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 1.25rem">
          <button type="button" class="kid-btn" :disabled="savingEdit" @click="editing = null">Cancel</button>
          <button type="submit" class="kid-btn kid-btn--primary" :disabled="savingEdit || !editName.trim()">
            {{ savingEdit ? "Saving…" : "Save Changes" }}
          </button>
        </div>
      </form>
    </div>
    <!-- OAuth Login modal -->
    <div
      v-if="showOAuthModal"
      style="position: fixed; inset: 0; background: rgba(0,0,0,0.5); z-index: 100; display: flex; align-items: center; justify-content: center; padding: 1rem"
      @click.self="showOAuthModal = false"
    >
      <div class="kid-card kid-wobble" style="width: min(480px, 100%); background: var(--color-surface)">
        <h3 style="font-size: 1.3rem; margin: 0 0 0.75rem">OAuth Login</h3>
        <div style="display: grid; gap: 0.6rem">
          <div>
            <label style="font-family: var(--font-body); font-size: 0.9rem">Provider</label>
            <select v-model="oauthProvider" class="kid-input" :disabled="oauthBusy">
              <option v-for="p in OAUTH_PROVIDERS" :key="p" :value="p">{{ p }}</option>
            </select>
          </div>
          <button class="kid-btn kid-btn--primary" :disabled="oauthBusy" @click="startOAuth">
            <span class="material-symbols-outlined" style="font-size: 16px">open_in_new</span>
            {{ oauthAuthUrl ? "Re-open authorize page" : "1. Open authorize page" }}
          </button>
          <div v-if="oauthAuthUrl" style="font-family: var(--font-body); font-size: 0.85rem; color: var(--color-text-muted)">
            Authorize in the browser tab, then paste the <strong>code</strong> (claude) or the
            <strong>full callback URL</strong> (codex / antigravity) below.
          </div>
          <div v-if="oauthAuthUrl">
            <label style="font-family: var(--font-body); font-size: 0.9rem">2. Paste code / callback URL</label>
            <textarea v-model="oauthCodeInput" class="kid-input" rows="3" placeholder="e.g. http://localhost:1455/auth/callback?code=…&state=…" :disabled="oauthBusy" />
          </div>
          <div v-if="oauthError" style="font-family: var(--font-body); font-size: 0.9rem; color: var(--color-danger)">{{ oauthError }}</div>
        </div>
        <div style="display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 1.25rem">
          <button type="button" class="kid-btn" :disabled="oauthBusy" @click="showOAuthModal = false">Cancel</button>
          <button class="kid-btn kid-btn--primary" :disabled="oauthBusy || !oauthAuthUrl || !oauthCodeInput.trim()" @click="exchangeOAuth">
            {{ oauthBusy ? "Working…" : "3. Connect" }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
