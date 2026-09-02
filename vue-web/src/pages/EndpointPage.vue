// Endpoint & Key — Node EndpointPageClient layout:
// Card "API Endpoint" (Local / Tunnel / Tailscale rows) + security banners
// + Card "API Keys" (require-key toggle, key rows with pause/delete,
// Create modal + one-time created-key reveal modal).
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { api } from "@/lib/api";
import { toast } from "@/lib/state";
import Badge from "@/components/Badge.vue";
import Toggle from "@/components/Toggle.vue";
import Modal from "@/components/Modal.vue";
import ConfirmModal from "@/components/ConfirmModal.vue";

interface ApiKey {
  id: string;
  key: string;
  name: string;
  machineId?: string;
  isActive?: boolean;
  createdAt: string;
}

const keys = ref<ApiKey[]>([]);
const loading = ref(true);
const settings = ref<Record<string, unknown> | null>(null);

const showAddModal = ref(false);
const newKeyName = ref("");
const creating = ref(false);
const visibleKeys = ref(new Set<string>());
const createdKey = ref<ApiKey | null>(null);
const confirmDelete = ref<ApiKey | null>(null);
const confirmPause = ref<ApiKey | null>(null);

const endpointUrl = `${window.location.protocol}//${window.location.host}/v1`;
const lanUrl = computed(() => {
  // Best-effort LAN hint — the Rust server binds loopback by default, so the
  // tunnel row is the remote path; show the hostname variant for LAN cards.
  return endpointUrl;
});

async function load() {
  loading.value = true;
  try {
    let list = (await api.get<{ keys: ApiKey[] }>("/api/keys")).keys ?? [];
    // Node parity: a fresh install auto-creates a Default Key.
    if (list.length === 0) {
      await api.post("/api/keys", { name: "Default Key" });
      list = (await api.get<{ keys: ApiKey[] }>("/api/keys")).keys ?? [];
    }
    keys.value = list;
    settings.value = await api.get<Record<string, unknown>>("/api/settings");
  } catch {
    toast.error("Failed to load keys/settings");
  } finally {
    loading.value = false;
  }
}
onMounted(load);

function copyText(text: string, label: string) {
  navigator.clipboard?.writeText(text).then(
    () => toast.success(`Copied ${label} to clipboard`),
    () => toast.error("Failed to copy"),
  );
}

function toggleVisibility(id: string) {
  const next = new Set(visibleKeys.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  visibleKeys.value = next;
}

function maskKey(k: ApiKey) {
  return visibleKeys.value.has(k.id) ? k.key : `${k.key.slice(0, 6)}•••${k.key.slice(-4)}`;
}

async function createKey() {
  if (!newKeyName.value.trim()) return;
  creating.value = true;
  try {
    const created = await api.post<ApiKey>("/api/keys", { name: newKeyName.value.trim() });
    keys.value = [created, ...keys.value];
    createdKey.value = created;
    newKeyName.value = "";
    showAddModal.value = false;
  } catch {
    toast.error("Failed to create API Key");
  } finally {
    creating.value = false;
  }
}

async function doDelete() {
  const k = confirmDelete.value;
  if (!k) return;
  confirmDelete.value = null;
  try {
    await api.del(`/api/keys/${k.id}`);
    keys.value = keys.value.filter((x) => x.id !== k.id);
    toast.success("Key deleted");
  } catch {
    toast.error("Failed to delete key");
  }
}

async function doPause() {
  const k = confirmPause.value;
  if (!k) return;
  confirmPause.value = null;
  const next = k.isActive !== false ? false : true;
  try {
    await api.put(`/api/keys/${k.id}`, { isActive: next });
    keys.value = keys.value.map((x) => (x.id === k.id ? { ...x, isActive: next } : x));
    toast.success(next ? `Key "${k.name}" resumed` : `Key "${k.name}" paused`);
  } catch {
    toast.error("Failed to update key");
  }
}

async function patchSetting(key: string, value: boolean) {
  try {
    settings.value = { ...(settings.value ?? {}), [key]: value };
    await api.patch("/api/settings", { [key]: value });
    toast.success(`Updated ${key}`);
  } catch {
    toast.error("Failed to update setting");
  }
}

const requireApiKey = computed(() => !!settings.value?.requireApiKey);
const requireLogin = computed(() => !!settings.value?.requireLogin);
const fmtDate = (iso: string) => {
  try {
    return new Date(iso).toLocaleDateString();
  } catch {
    return iso;
  }
};
</script>

<template>
  <div class="fade-in flex flex-col gap-6" style="max-width: 1000px">
    <!-- Card 1: API Endpoint -->
    <div class="kid-card">
      <h2 style="display: flex; align-items: center; gap: 0.5rem; font-size: 1.25rem; margin: 0 0 1rem">
        <span class="material-symbols-outlined" style="font-size: 20px; color: var(--color-primary)">api</span>
        API Endpoint
      </h2>

      <!-- Local row -->
      <div style="display: flex; align-items: center; gap: 0.6rem; margin-bottom: 0.6rem; flex-wrap: wrap">
        <span class="console-label" style="min-width: 88px">Local</span>
        <input class="kid-input" readonly :value="endpointUrl" style="flex: 1; min-width: 220px; font-family: ui-monospace, Menlo, monospace" />
        <button class="kid-btn" style="padding: 0.3rem 0.6rem" @click="copyText(endpointUrl, 'Local URL')">
          <span class="material-symbols-outlined" style="font-size: 16px">content_copy</span>
        </button>
      </div>

      <!-- Tunnel row (needs Node engine) -->
      <div style="display: flex; align-items: center; gap: 0.6rem; margin-bottom: 0.6rem; flex-wrap: wrap">
        <span class="console-label" style="min-width: 88px">Tunnel</span>
        <input class="kid-input" readonly value="—" style="flex: 1; min-width: 220px" />
        <Badge variant="neutral" size="sm">requires Node engine</Badge>
      </div>

      <!-- Tailscale row (needs Node engine) -->
      <div style="display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap">
        <span class="console-label" style="min-width: 88px">Tailscale</span>
        <input class="kid-input" readonly value="—" style="flex: 1; min-width: 220px" />
        <Badge variant="neutral" size="sm">requires Node engine</Badge>
      </div>

      <!-- Security banners (Node parity) -->
      <div
        v-if="!requireLogin"
        style="margin-top: 0.9rem; padding: 0.55rem 0.8rem; border: 1px solid var(--color-warning); background: color-mix(in srgb, var(--color-warning) 12%, transparent); font-family: var(--font-body); font-size: 0.9rem"
      >
        ⚠ Dashboard login is disabled — anyone on this machine can change settings.
      </div>
      <div
        v-if="settings && !requireApiKey"
        style="margin-top: 0.6rem; padding: 0.55rem 0.8rem; border: 1px solid var(--color-danger); background: color-mix(in srgb, var(--color-danger) 10%, transparent); font-family: var(--font-body); font-size: 0.9rem"
      >
        Require API key is disabled — /v1/* endpoints accept unauthenticated requests.
      </div>
      <div v-if="settings" style="display: flex; justify-content: space-between; align-items: center; gap: 1rem; margin-top: 0.9rem">
        <span style="font-family: var(--font-body); font-size: 0.95rem">Allow dashboard access via tunnel</span>
        <Toggle :checked="!!settings.tunnelDashboardAccess" @change="(v) => patchSetting('tunnelDashboardAccess', v)" />
      </div>
    </div>

    <!-- Card 2: API Keys -->
    <div class="kid-card">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.75rem; flex-wrap: wrap; gap: 0.5rem">
        <h2 style="display: flex; align-items: center; gap: 0.5rem; font-size: 1.25rem; margin: 0">
          <span class="material-symbols-outlined" style="font-size: 20px; color: var(--color-primary)">vpn_key</span>
          API Keys
        </h2>
        <button class="kid-btn kid-btn--primary" style="padding: 0.3rem 0.75rem" @click="showAddModal = true">Create Key</button>
      </div>

      <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem; padding: 0.5rem 0; border-top: 1px solid var(--color-border-subtle)">
        <div>
          <strong style="font-size: 0.98rem">Require API key</strong>
          <div style="font-family: var(--font-body); font-size: 0.88rem; color: var(--color-text-muted)">Requests without a valid key will be rejected</div>
        </div>
        <Toggle :checked="requireApiKey" @change="(v) => patchSetting('requireApiKey', v)" />
      </div>

      <p v-if="loading" style="font-family: var(--font-body)">Loading API keys…</p>

      <div v-if="!loading && keys.length === 0" style="text-align: center; padding: 2rem 1rem">
        <div style="font-size: 2rem">🔑</div>
        <p style="font-family: var(--font-body); margin: 0.4rem 0">No API keys yet</p>
        <button class="kid-btn kid-btn--primary" style="padding: 0.3rem 0.75rem" @click="showAddModal = true">Create Key</button>
      </div>

      <div v-if="!loading && keys.length > 0" style="margin-top: 0.5rem">
        <div
          v-for="k in keys"
          :key="k.id"
          style="display: flex; align-items: center; gap: 0.8rem; padding: 0.65rem 0; border-top: 1px solid var(--color-border-subtle); flex-wrap: wrap"
        >
          <strong style="min-width: 120px">{{ k.name }}</strong>
          <code style="background: var(--color-bg-alt); padding: 0.2rem 0.45rem; border: 1px solid var(--nb-border); font-size: 0.85rem">
            {{ maskKey(k) }}
          </code>
          <button class="kid-btn" style="padding: 0.2rem 0.4rem" @click="toggleVisibility(k.id)">
            <span class="material-symbols-outlined" style="font-size: 15px">{{ visibleKeys.has(k.id) ? "visibility_off" : "visibility" }}</span>
          </button>
          <button class="kid-btn" style="padding: 0.2rem 0.4rem" @click="copyText(k.key, 'API Key')">
            <span class="material-symbols-outlined" style="font-size: 15px">content_copy</span>
          </button>
          <span style="font-family: var(--font-body); font-size: 0.82rem; color: var(--color-text-muted)">Created {{ fmtDate(k.createdAt) }}</span>
          <span v-if="k.isActive === false" style="font-family: var(--font-body); font-size: 0.85rem; color: #d97706; font-weight: 700">Paused</span>
          <div style="margin-left: auto; display: flex; align-items: center; gap: 0.5rem">
            <Toggle :checked="k.isActive !== false" @change="() => (confirmPause = k)" />
            <button class="kid-btn" style="padding: 0.2rem 0.4rem; background: var(--color-danger); color: #fff" @click="confirmDelete = k">
              <span class="material-symbols-outlined" style="font-size: 15px">delete</span>
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Create modal -->
    <Modal v-if="showAddModal" @close="showAddModal = false">
      <form @submit.prevent="createKey">
        <h3 style="font-size: 1.25rem; margin: 0 0 0.75rem">Create API Key</h3>
        <label style="display: block; font-family: var(--font-body); margin-bottom: 0.4rem">Key Name</label>
        <input v-model="newKeyName" class="kid-input" placeholder="Production Key" :disabled="creating" autofocus />
        <div style="display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 1.25rem">
          <button type="button" class="kid-btn" :disabled="creating" @click="showAddModal = false">Cancel</button>
          <button type="submit" class="kid-btn kid-btn--primary" :disabled="creating || !newKeyName.trim()">
            {{ creating ? "Creating…" : "Create" }}
          </button>
        </div>
      </form>
    </Modal>

    <!-- Created-key reveal (one-time) -->
    <Modal v-if="createdKey" width="460px" @close="createdKey = null">
      <h3 style="font-size: 1.2rem; margin: 0 0 0.5rem">Save this key now!</h3>
      <p style="font-family: var(--font-body); color: #d97706; font-weight: 700; margin: 0 0 0.75rem">
        This is the only time the full key is shown.
      </p>
      <code style="display: block; background: var(--color-bg-alt); border: 1px solid var(--nb-border); padding: 0.5rem 0.7rem; font-size: 0.9rem; word-break: break-all">
        {{ createdKey.key }}
      </code>
      <div style="display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 1rem">
        <button class="kid-btn" @click="copyText(createdKey.key, 'API Key')">
          <span class="material-symbols-outlined" style="font-size: 15px">content_copy</span> Copy
        </button>
        <button class="kid-btn kid-btn--primary" @click="createdKey = null">Done</button>
      </div>
    </Modal>

    <ConfirmModal
      v-if="confirmDelete"
      title="Delete API Key"
      :message="`Delete key '${confirmDelete.name}'? This cannot be undone.`"
      confirm-label="Delete"
      danger
      @close="confirmDelete = null"
      @confirm="doDelete"
    />
    <ConfirmModal
      v-if="confirmPause"
      :title="confirmPause.isActive !== false ? 'Pause API Key' : 'Resume API Key'"
      :message="`Key '${confirmPause.name}' will ${confirmPause.isActive !== false ? 'stop accepting requests' : 'accept requests again'}.`"
      :confirm-label="confirmPause.isActive !== false ? 'Pause' : 'Resume'"
      :danger="confirmPause.isActive !== false"
      @close="confirmPause = null"
      @confirm="doPause"
    />
  </div>
</template>
