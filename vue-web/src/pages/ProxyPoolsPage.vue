// Proxy Pools: CRUD + per-pool live test (exit IP through the proxy).
<script setup lang="ts">
import { onMounted, ref } from "vue";
import { api } from "@/lib/api";
import { toast } from "@/lib/state";
import Badge from "@/components/Badge.vue";

interface Pool {
  id: string;
  name?: string;
  proxyUrl?: string;
  noProxy?: string;
  type?: string;
  strictProxy?: boolean;
  isActive?: boolean;
  testStatus?: string;
  lastError?: string | null;
  lastTestedAt?: string | null;
  exitIp?: string | null;
}

const pools = ref<Pool[]>([]);
const loading = ref(true);
const showForm = ref(false);
const editingId = ref<string | null>(null);
const name = ref("");
const proxyUrl = ref("");
const poolType = ref("http");
const strictProxy = ref(false);
const isActive = ref(true);
const saving = ref(false);
const testingId = ref<string | null>(null);

async function load() {
  loading.value = true;
  try {
    const data = await api.get<{ pools: Pool[] }>("/api/proxy-pools");
    pools.value = (data.pools ?? []).map((p) => ({ ...p, exitIp: p.exitIp ?? null }));
  } catch {
    toast.error("Failed to load pools");
  } finally {
    loading.value = false;
  }
}
onMounted(load);

function openAdd() {
  editingId.value = null;
  name.value = "";
  proxyUrl.value = "";
  poolType.value = "http";
  strictProxy.value = false;
  isActive.value = true;
  showForm.value = true;
}

function openEdit(p: Pool) {
  editingId.value = p.id;
  name.value = p.name ?? "";
  proxyUrl.value = p.proxyUrl ?? "";
  poolType.value = p.type ?? "http";
  strictProxy.value = p.strictProxy === true;
  isActive.value = p.isActive !== false;
  showForm.value = true;
}

async function save() {
  if (!name.value.trim() || !proxyUrl.value.trim()) return;
  saving.value = true;
  const body: Record<string, unknown> = {
    name: name.value.trim(),
    proxyUrl: proxyUrl.value.trim(),
    type: poolType.value,
    strictProxy: strictProxy.value,
    isActive: isActive.value,
  };
  if (editingId.value) body.id = editingId.value;
  try {
    await api.post("/api/proxy-pools", body);
    toast.success(editingId.value ? "Pool updated" : "Pool created");
    showForm.value = false;
    await load();
  } catch (e) {
    toast.error(e instanceof Error && e.message ? e.message : "Failed to save pool");
  } finally {
    saving.value = false;
  }
}

async function remove(p: Pool) {
  if (!confirm(`Delete pool "${p.name ?? p.id}"?`)) return;
  try {
    await api.del(`/api/proxy-pools/${p.id}`);
    toast.success("Pool deleted");
    await load();
  } catch {
    toast.error("Failed to delete pool");
  }
}

async function test(p: Pool) {
  testingId.value = p.id;
  try {
    const r = await api.post<{ valid: boolean; exitIp: string | null; error: string | null }>(
      `/api/proxy-pools/${p.id}/test`,
    );
    if (r.valid) toast.success(`Pool "${p.name ?? p.id}" OK — exit IP ${r.exitIp ?? "?"}`);
    else toast.error(`Pool "${p.name ?? p.id}" failed: ${r.error ?? "unreachable"}`, "Pool Test");
    await load();
  } catch {
    toast.error("Test request failed");
  } finally {
    testingId.value = null;
  }
}

function maskUrl(url?: string) {
  if (!url) return "—";
  try {
    const u = new URL(url);
    return `${u.protocol}//${u.hostname}:${u.port || (u.protocol === "https:" ? 443 : 80)}`;
  } catch {
    return url;
  }
}
</script>

<template>
  <div class="fade-in flex flex-col gap-4" style="max-width: 900px">
    <div style="display: flex; justify-content: flex-end">
      <button class="kid-btn kid-btn--primary" style="padding: 0.3rem 0.75rem" @click="openAdd">
        ＋ Add Pool
      </button>
    </div>

    <p v-if="loading" style="font-family: var(--font-body)">Loading pools…</p>

    <div v-if="!loading && pools.length === 0" class="kid-card" style="text-align: center; padding: 2.5rem 1rem">
      <p style="font-family: var(--font-body)">No proxy pools yet. Add one to route provider traffic through it.</p>
    </div>

    <div style="display: grid; gap: 1rem; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr))">
      <div v-for="p in pools" :key="p.id" class="kid-card" :style="p.isActive === false ? { opacity: 0.55 } : undefined">
        <div style="display: flex; justify-content: space-between; align-items: flex-start">
          <div>
            <strong style="font-size: 1.15rem">{{ p.name ?? p.id }}</strong>
            <div style="font-family: var(--font-body); color: var(--color-text-muted); font-size: 0.9rem">
              {{ maskUrl(p.proxyUrl) }} · {{ p.type ?? "http" }}
            </div>
          </div>
          <Badge :variant="p.testStatus === 'active' ? 'success' : p.testStatus === 'error' ? 'danger' : 'neutral'" size="sm" dot>
            {{ p.testStatus ?? "unknown" }}
          </Badge>
        </div>

        <div v-if="p.lastError && p.testStatus === 'error'" style="font-family: var(--font-body); font-size: 0.85rem; color: var(--color-danger); margin-top: 0.5rem">
          {{ p.lastError }}
        </div>
        <div v-if="p.exitIp" style="font-family: var(--font-body); font-size: 0.85rem; color: var(--color-text-muted); margin-top: 0.4rem">
          exit IP: <code>{{ p.exitIp }}</code>
        </div>

        <div style="display: flex; gap: 0.4rem; justify-content: flex-end; margin-top: 0.85rem">
          <button class="kid-btn" style="padding: 0.25rem 0.5rem" @click="openEdit(p)">
            <span class="material-symbols-outlined" style="font-size: 16px">edit</span>
          </button>
          <button class="kid-btn kid-btn--accent" style="padding: 0.25rem 0.5rem" :disabled="testingId === p.id" @click="test(p)">
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

    <!-- Add/Edit modal -->
    <div
      v-if="showForm"
      style="position: fixed; inset: 0; background: rgba(0,0,0,0.5); z-index: 100; display: flex; align-items: center; justify-content: center; padding: 1rem"
      @click.self="showForm = false"
    >
      <form class="kid-card kid-wobble" style="width: min(440px, 100%); background: var(--color-surface)" @submit.prevent="save">
        <h3 style="font-size: 1.3rem; margin: 0 0 0.75rem">{{ editingId ? "Edit Pool" : "＋ Add Proxy Pool" }}</h3>
        <div style="display: grid; gap: 0.6rem">
          <div>
            <label style="font-family: var(--font-body); font-size: 0.9rem">Name</label>
            <input v-model="name" class="kid-input" :disabled="saving" />
          </div>
          <div>
            <label style="font-family: var(--font-body); font-size: 0.9rem">Proxy URL (http/socks5)</label>
            <input v-model="proxyUrl" class="kid-input" placeholder="http://user:pass@host:port" :disabled="saving" />
          </div>
          <div>
            <label style="font-family: var(--font-body); font-size: 0.9rem">Type</label>
            <select v-model="poolType" class="kid-input" :disabled="saving">
              <option value="http">http</option>
              <option value="socks5">socks5</option>
            </select>
          </div>
          <div style="display: flex; gap: 1.5rem; align-items: center; font-family: var(--font-body)">
            <label style="display: flex; gap: 0.4rem; align-items: center">
              <input v-model="isActive" type="checkbox" /> Active
            </label>
            <label style="display: flex; gap: 0.4rem; align-items: center">
              <input v-model="strictProxy" type="checkbox" /> Strict (fail when down)
            </label>
          </div>
        </div>
        <div style="display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 1.25rem">
          <button type="button" class="kid-btn" :disabled="saving" @click="showForm = false">Cancel</button>
          <button type="submit" class="kid-btn kid-btn--primary" :disabled="saving || !name.trim() || !proxyUrl.trim()">
            {{ saving ? "Saving…" : "Save Pool" }}
          </button>
        </div>
      </form>
    </div>
  </div>
</template>
