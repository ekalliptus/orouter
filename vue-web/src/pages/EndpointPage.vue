// Endpoint & Key: OpenAI base URL bar, API keys table with show/copy/toggle/
// delete, and endpoint security toggles. Backed by /api/keys + /api/settings.
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { api } from "@/lib/api";
import { toast } from "@/lib/state";
import Badge from "@/components/Badge.vue";
import Toggle from "@/components/Toggle.vue";

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

const endpointUrl = `${window.location.protocol}//${window.location.host}/v1`;

async function load() {
  loading.value = true;
  try {
    const [keysRes, settingsRes] = await Promise.all([
      api.get<{ keys: ApiKey[] }>("/api/keys"),
      api.get<Record<string, unknown>>("/api/settings"),
    ]);
    keys.value = keysRes.keys ?? [];
    settings.value = settingsRes;
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

async function createKey() {
  if (!newKeyName.value.trim()) return;
  creating.value = true;
  try {
    const created = await api.post<ApiKey>("/api/keys", { name: newKeyName.value.trim() });
    keys.value = [created, ...keys.value];
    toast.success(`API Key "${created.name}" created!`);
    newKeyName.value = "";
    showAddModal.value = false;
  } catch {
    toast.error("Failed to create API Key");
  } finally {
    creating.value = false;
  }
}

async function deleteKey(k: ApiKey) {
  if (!confirm(`Delete API Key "${k.name}"?`)) return;
  try {
    await api.del(`/api/keys/${k.id}`);
    keys.value = keys.value.filter((x) => x.id !== k.id);
    toast.success("Key deleted");
  } catch {
    toast.error("Failed to delete key");
  }
}

async function toggleKey(k: ApiKey) {
  const next = !(k.isActive !== false);
  try {
    await api.put(`/api/keys/${k.id}`, { isActive: next });
    keys.value = keys.value.map((x) => (x.id === k.id ? { ...x, isActive: next } : x));
    toast.success(`Key "${k.name}" ${next ? "enabled" : "disabled"}`);
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
</script>

<template>
  <div class="fade-in flex flex-col gap-6" style="max-width: 1000px">
    <!-- Endpoint URL Box -->
    <div class="kid-card" style="background-color: var(--color-brand-50); border-color: var(--color-brand-500)">
      <div style="display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 1rem">
        <div>
          <div style="font-size: 0.85rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.05em; color: var(--color-brand-700)">
            OpenAI Base URL
          </div>
          <code style="font-size: 1.25rem; font-weight: 700; background: transparent">{{ endpointUrl }}</code>
        </div>
        <button class="kid-btn kid-btn--accent" style="padding: 0.3rem 0.75rem" @click="copyText(endpointUrl, 'Base URL')">
          <span class="material-symbols-outlined" style="font-size: 16px">content_copy</span> Copy Base URL
        </button>
      </div>
    </div>

    <!-- Keys -->
    <div class="kid-card">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; flex-wrap: wrap; gap: 0.5rem">
        <div>
          <h2 style="font-size: 1.3rem; margin: 0">API Keys ({{ keys.length }})</h2>
          <span style="font-family: var(--font-body); color: var(--color-text-muted); font-size: 0.95rem">Authenticate your developer tools against ORouter.</span>
        </div>
        <button class="kid-btn kid-btn--primary" style="padding: 0.3rem 0.75rem" @click="showAddModal = true">＋ Create New Key</button>
      </div>

      <p v-if="loading" style="font-family: var(--font-body)">Loading API keys…</p>

      <div v-if="!loading && keys.length === 0" style="text-align: center; padding: 2rem 1rem; font-family: var(--font-body)">
        <p style="margin: 0">No API keys yet. Create your first key to start making AI calls!</p>
      </div>

      <div v-if="!loading && keys.length > 0" style="overflow-x: auto">
        <table style="width: 100%; border-collapse: collapse; text-align: left; font-family: var(--font-body)">
          <thead>
            <tr style="border-bottom: 3px solid var(--nb-border); font-size: 0.7rem; text-transform: uppercase; letter-spacing: 0.05em; color: var(--color-text-muted)">
              <th style="padding: 0.6rem 0.8rem">Name</th>
              <th style="padding: 0.6rem 0.8rem">API Key</th>
              <th style="padding: 0.6rem 0.8rem">Status</th>
              <th style="padding: 0.6rem 0.8rem; text-align: right">Actions</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="k in keys" :key="k.id" style="border-bottom: 2px solid var(--color-surface-3)">
              <td style="padding: 0.75rem 0.8rem; font-weight: 700">{{ k.name }}</td>
              <td style="padding: 0.75rem 0.8rem">
                <code style="background: var(--color-bg-alt); padding: 0.2rem 0.4rem; border: 1px solid var(--nb-border); font-size: 0.9rem">
                  {{ visibleKeys.has(k.id) ? k.key : k.key.slice(0, 10) + "..." + k.key.slice(-6) }}
                </code>
              </td>
              <td style="padding: 0.75rem 0.8rem">
                <div style="display: inline-flex; align-items: center; gap: 0.5rem">
                  <Badge :variant="k.isActive !== false ? 'success' : 'danger'" size="sm" dot>
                    {{ k.isActive !== false ? "Active" : "Disabled" }}
                  </Badge>
                  <Toggle :checked="k.isActive !== false" @change="() => toggleKey(k)" />
                </div>
              </td>
              <td style="padding: 0.75rem 0.8rem; text-align: right">
                <div style="display: inline-flex; gap: 0.4rem">
                  <button class="kid-btn" style="padding: 0.25rem 0.55rem; font-size: 0.78rem" @click="toggleVisibility(k.id)">
                    <span class="material-symbols-outlined" style="font-size: 14px">{{ visibleKeys.has(k.id) ? "visibility_off" : "visibility" }}</span>
                  </button>
                  <button class="kid-btn kid-btn--accent" style="padding: 0.25rem 0.55rem; font-size: 0.78rem" @click="copyText(k.key, 'API Key')">
                    <span class="material-symbols-outlined" style="font-size: 14px">content_copy</span>
                  </button>
                  <button class="kid-btn" style="padding: 0.25rem 0.55rem; font-size: 0.78rem; background: var(--color-danger); color: #fff" @click="deleteKey(k)">
                    <span class="material-symbols-outlined" style="font-size: 14px">delete</span>
                  </button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- Security -->
    <div v-if="settings" class="kid-card">
      <h2 style="font-size: 1.3rem; margin: 0 0 1rem">Endpoint Security Settings</h2>
      <div style="display: flex; flex-direction: column; gap: 1rem">
        <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem">
          <div>
            <strong>Require API Key for Requests</strong>
            <p style="font-family: var(--font-body); color: var(--color-text-muted); margin: 0; font-size: 0.95rem">
              Fail-closed: /v1/* endpoints reject requests without a valid sk- key.
            </p>
          </div>
          <Toggle :checked="requireApiKey" @change="(v) => patchSetting('requireApiKey', v)" />
        </div>
        <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem; border-top: 2px dashed var(--color-surface-3); padding-top: 1rem">
          <div>
            <strong>Require Dashboard Login</strong>
            <p style="font-family: var(--font-body); color: var(--color-text-muted); margin: 0; font-size: 0.95rem">
              Protect web dashboard access with password authentication.
            </p>
          </div>
          <Toggle :checked="requireLogin" @change="(v) => patchSetting('requireLogin', v)" />
        </div>
      </div>
    </div>

    <!-- Create modal -->
    <div
      v-if="showAddModal"
      style="position: fixed; inset: 0; background: rgba(0,0,0,0.5); z-index: 100; display: flex; align-items: center; justify-content: center; padding: 1rem"
      @click.self="showAddModal = false"
    >
      <form class="kid-card kid-wobble" style="width: min(400px, 100%); background: var(--color-surface)" @submit.prevent="createKey">
        <h3 style="font-size: 1.3rem; margin: 0 0 0.75rem">＋ Create New API Key</h3>
        <label style="display: block; font-family: var(--font-body); margin-bottom: 0.4rem">Key Name</label>
        <input v-model="newKeyName" class="kid-input" placeholder="e.g. My Cursor IDE" :disabled="creating" autofocus />
        <div style="display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 1.25rem">
          <button type="button" class="kid-btn" :disabled="creating" @click="showAddModal = false">Cancel</button>
          <button type="submit" class="kid-btn kid-btn--primary" :disabled="creating || !newKeyName.trim()">
            {{ creating ? "Creating…" : "Create Key" }}
          </button>
        </div>
      </form>
    </div>
  </div>
</template>
