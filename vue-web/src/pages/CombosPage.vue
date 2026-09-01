// Combos: list + create (name + one-model-per-line) + delete.
<script setup lang="ts">
import { onMounted, ref } from "vue";
import { api } from "@/lib/api";
import { toast } from "@/lib/state";

interface Combo {
  id: string;
  name: string;
  kind?: string;
  models: string[];
  createdAt: string;
}

const combos = ref<Combo[]>([]);
const loading = ref(true);
const name = ref("");
const modelsInput = ref("");
const creating = ref(false);

async function load() {
  loading.value = true;
  try {
    const data = await api.get<{ combos?: Combo[] }>("/api/combos");
    combos.value = data.combos ?? [];
  } catch {
    toast.error("Failed to fetch combos");
  } finally {
    loading.value = false;
  }
}
onMounted(load);

async function create() {
  if (!name.value.trim()) return;
  creating.value = true;
  const models = modelsInput.value.split("\n").map((s) => s.trim()).filter(Boolean);
  try {
    await api.post("/api/combos", { name: name.value.trim(), models });
    toast.success(`Combo "${name.value.trim()}" created`);
    name.value = "";
    modelsInput.value = "";
    await load();
  } catch (e) {
    toast.error(e instanceof Error && e.message ? e.message : "Failed to create combo");
  } finally {
    creating.value = false;
  }
}

async function remove(c: Combo) {
  if (!confirm(`Delete combo "${c.name}"?`)) return;
  try {
    await api.del(`/api/combos/${c.id}`);
    toast.success("Combo deleted");
    combos.value = combos.value.filter((x) => x.id !== c.id);
  } catch {
    toast.error("Failed to delete combo");
  }
}
</script>

<template>
  <div class="fade-in flex flex-col gap-4">
    <!-- Create -->
    <form class="kid-card kid-wobble" style="max-width: 640px" @submit.prevent="create">
      <div style="font-weight: 700; font-size: 1.15rem; margin-bottom: 0.5rem">＋ Create Combo</div>
      <div style="display: grid; gap: 0.6rem">
        <div>
          <label style="font-family: var(--font-body); font-size: 0.9rem">Combo name</label>
          <input v-model="name" class="kid-input" placeholder="e.g. gaskeun" :disabled="creating" />
        </div>
        <div>
          <label style="font-family: var(--font-body); font-size: 0.9rem">Models (one per line, e.g. openrouter/openai/gpt-4o)</label>
          <textarea v-model="modelsInput" class="kid-input" rows="3" placeholder="openrouter/openai/gpt-4o&#10;deepseek/deepseek-chat" :disabled="creating" />
        </div>
      </div>
      <button type="submit" class="kid-btn kid-btn--primary" style="margin-top: 0.75rem" :disabled="creating || !name.trim()">
        {{ creating ? "Creating…" : "Create Combo" }}
      </button>
    </form>

    <p v-if="loading" style="font-family: var(--font-body)">Loading combos…</p>

    <div v-if="!loading && combos.length === 0" class="kid-card" style="text-align: center; max-width: 640px">
      <p style="font-family: var(--font-body)">No combos yet. Create one above!</p>
    </div>

    <div style="display: grid; gap: 1rem; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr))">
      <div v-for="c in combos" :key="c.id" class="kid-card">
        <div style="display: flex; justify-content: space-between; align-items: baseline">
          <strong style="font-size: 1.15rem">{{ c.name }}</strong>
          <button class="kid-btn" style="padding: 0.2rem 0.5rem; background: var(--color-danger); color: #fff" @click="remove(c)">
            <span class="material-symbols-outlined" style="font-size: 14px">delete</span>
          </button>
        </div>
        <div style="font-family: var(--font-body); color: var(--color-text-muted); margin-top: 0.5rem">{{ c.models.length }} model(s):</div>
        <ul style="font-family: var(--font-body); font-size: 0.95rem; padding-left: 1.2rem; margin: 0.25rem 0">
          <li v-for="(m, idx) in c.models" :key="idx"><code>{{ m }}</code></li>
        </ul>
      </div>
    </div>
  </div>
</template>
