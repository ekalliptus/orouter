// Combos — Node layout: header explainer + strategy list; combo cards with
// model chips + per-combo strategy select + copy/edit/delete; ComboFormModal
// with an ordered chain editor (↑↓/remove/add, name regex validation).
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { api } from "@/lib/api";
import { toast } from "@/lib/state";
import Modal from "@/components/Modal.vue";
import ConfirmModal from "@/components/ConfirmModal.vue";

interface Combo {
  id: string;
  name: string;
  kind?: string;
  models: string[];
  createdAt: string;
}

const combos = ref<Combo[]>([]);
const loading = ref(true);

const showForm = ref(false);
const editingId = ref<string | null>(null);
const comboName = ref("");
const chain = ref<string[]>([]);
const newModel = ref("");
const saving = ref(false);

const confirmDelete = ref<Combo | null>(null);

const NAME_RE = /^[a-zA-Z0-9_.\-]+$/;
const nameError = computed(() => {
  if (!comboName.value.trim()) return "Name is required";
  if (!NAME_RE.test(comboName.value.trim())) return "Only letters, numbers, -, _ and . allowed";
  return null;
});

async function load() {
  loading.value = true;
  try {
    const data = await api.get<{ combos?: Combo[] }>("/api/combos");
    combos.value = (data.combos ?? []).filter((c) => !c.kind || c.kind === "llm");
  } catch {
    toast.error("Failed to fetch combos");
  } finally {
    loading.value = false;
  }
}
onMounted(load);

function openCreate() {
  editingId.value = null;
  comboName.value = "";
  chain.value = [];
  showForm.value = true;
}

function openEdit(c: Combo) {
  editingId.value = c.id;
  comboName.value = c.name;
  chain.value = [...c.models];
  showForm.value = true;
}

function addModel() {
  const m = newModel.value.trim();
  if (!m) return;
  chain.value.push(m);
  newModel.value = "";
}

function moveModel(i: number, dir: -1 | 1) {
  const j = i + dir;
  if (j < 0 || j >= chain.value.length) return;
  [chain.value[i], chain.value[j]] = [chain.value[j], chain.value[i]];
}

async function save() {
  if (nameError.value) return;
  saving.value = true;
  const payload = { name: comboName.value.trim(), models: chain.value, kind: "llm" };
  try {
    if (editingId.value) {
      await api.put(`/api/combos/${editingId.value}`, payload);
      toast.success("Combo saved");
    } else {
      await api.post("/api/combos", payload);
      toast.success(`Combo "${payload.name}" created`);
    }
    showForm.value = false;
    await load();
  } catch (e) {
    toast.error(e instanceof Error && e.message ? e.message : "Failed to save combo");
  } finally {
    saving.value = false;
  }
}

async function remove(c: Combo) {
  confirmDelete.value = null;
  try {
    await api.del(`/api/combos/${c.id}`);
    toast.success("Combo deleted");
    combos.value = combos.value.filter((x) => x.id !== c.id);
  } catch {
    toast.error("Failed to delete combo");
  }
}

function copyText(text: string) {
  navigator.clipboard?.writeText(text).then(() => toast.success(`Copied "${text}"`), () => toast.error("Failed to copy"));
}

const strategies = [
  { value: "fallback", label: "Fallback — try in order" },
  { value: "round-robin", label: "Round Robin — rotate" },
  { value: "fill-first", label: "Fill First — drain one account" },
];

function strategyOf(c: Combo): string {
  // Per-combo strategies live in settings.comboStrategies map (read side).
  return c.kind && c.kind !== "llm" ? c.kind : "fallback";
}
</script>

<template>
  <div class="fade-in flex flex-col gap-4" style="max-width: 1000px">
    <!-- Header row -->
    <div style="display: flex; justify-content: space-between; align-items: flex-start; flex-wrap: wrap; gap: 0.75rem">
      <div style="font-family: var(--font-body); color: var(--color-text-muted); font-size: 0.95rem">
        Group models under one name, then pick a strategy per combo:
        <ul style="margin: 0.3rem 0 0; padding-left: 1.2rem">
          <li><strong>Fallback</strong> — tries models in order</li>
          <li><strong>Round Robin</strong> — rotates between models</li>
          <li><strong>Fill First</strong> — drains one model before moving on</li>
        </ul>
      </div>
      <button class="kid-btn kid-btn--primary" style="padding: 0.35rem 0.9rem" @click="openCreate">
        <span class="material-symbols-outlined" style="font-size: 16px">add</span> Create Combo
      </button>
    </div>

    <p v-if="loading" style="font-family: var(--font-body)">Loading combos…</p>

    <div v-if="!loading && combos.length === 0" class="kid-card" style="text-align: center; padding: 2.5rem 1rem">
      <span class="material-symbols-outlined" style="font-size: 36px; color: var(--color-text-muted)">layers</span>
      <p style="font-family: var(--font-body); margin: 0.4rem 0 0">No combos yet — create model combos with fallback support.</p>
    </div>

    <!-- Combo cards -->
    <div style="display: grid; gap: 1rem; grid-template-columns: repeat(auto-fill, minmax(340px, 1fr))">
      <div v-for="c in combos" :key="c.id" class="kid-card">
        <div style="display: flex; justify-content: space-between; align-items: flex-start; gap: 0.5rem">
          <div style="display: flex; gap: 0.7rem; align-items: center; min-width: 0">
            <span class="material-symbols-outlined" style="font-size: 24px; color: var(--color-primary)">layers</span>
            <code style="font-weight: 700; font-size: 1rem">{{ c.name }}</code>
          </div>
          <div style="display: flex; gap: 0.3rem">
            <button class="kid-btn" style="padding: 0.2rem 0.4rem" title="Copy combo name" @click="copyText(c.name)">
              <span class="material-symbols-outlined" style="font-size: 14px">content_copy</span>
            </button>
            <button class="kid-btn" style="padding: 0.2rem 0.4rem" title="Edit" @click="openEdit(c)">
              <span class="material-symbols-outlined" style="font-size: 14px">edit</span>
            </button>
            <button class="kid-btn" style="padding: 0.2rem 0.4rem; background: var(--color-danger); color: #fff" title="Delete" @click="confirmDelete = c">
              <span class="material-symbols-outlined" style="font-size: 14px">delete</span>
            </button>
          </div>
        </div>

        <div style="display: flex; flex-wrap: wrap; gap: 0.35rem; margin-top: 0.6rem">
          <template v-if="c.models.length > 0">
            <code v-for="(m, i) in c.models.slice(0, 3)" :key="i" style="font-size: 0.78rem; background: var(--color-bg-alt); border: 1px solid var(--nb-border); padding: 0.15rem 0.4rem">{{ m }}</code>
            <span v-if="c.models.length > 3" style="font-family: var(--font-body); font-size: 0.8rem; color: var(--color-text-muted)">+{{ c.models.length - 3 }} more</span>
          </template>
          <i v-else style="font-family: var(--font-body); font-size: 0.85rem; color: var(--color-text-muted)">No models</i>
        </div>

        <div style="margin-top: 0.75rem">
          <label style="font-family: var(--font-body); font-size: 0.82rem; color: var(--color-text-muted)">Strategy</label>
          <select class="kid-input" style="margin-top: 0.2rem" :value="strategyOf(c)" disabled title="Per-combo strategy editing needs the Node engine">
            <option v-for="s in strategies" :key="s.value" :value="s.value">{{ s.label }}</option>
          </select>
        </div>
      </div>
    </div>

    <!-- Chain editor modal -->
    <Modal v-if="showForm" width="520px" @close="showForm = false">
      <form @submit.prevent="save">
        <h3 style="font-size: 1.25rem; margin: 0 0 0.75rem">{{ editingId ? "Edit Combo" : "Create Combo" }}</h3>
        <label style="display: block; font-family: var(--font-body); margin-bottom: 0.3rem">Combo Name</label>
        <input v-model="comboName" class="kid-input" placeholder="my-combo" :disabled="saving" />
        <div v-if="comboName && nameError" style="font-family: var(--font-body); font-size: 0.85rem; color: var(--color-danger); margin-top: 0.25rem">{{ nameError }}</div>

        <label style="display: block; font-family: var(--font-body); margin: 0.9rem 0 0.3rem">Models</label>
        <div v-if="chain.length === 0" style="border: 2px dashed var(--color-surface-3); padding: 1rem; text-align: center; font-family: var(--font-body); color: var(--color-text-muted)">
          No models added yet
        </div>
        <div v-else style="display: flex; flex-direction: column; gap: 0.35rem">
          <div v-for="(m, i) in chain" :key="i" style="display: flex; align-items: center; gap: 0.4rem">
            <span class="console-label" style="min-width: 18px">{{ i + 1 }}</span>
            <code style="flex: 1; font-size: 0.82rem; background: var(--color-bg-alt); border: 1px solid var(--nb-border); padding: 0.25rem 0.45rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap">{{ m }}</code>
            <button type="button" class="kid-btn" style="padding: 0.15rem 0.35rem" :disabled="i === 0" @click="moveModel(i, -1)">↑</button>
            <button type="button" class="kid-btn" style="padding: 0.15rem 0.35rem" :disabled="i === chain.length - 1" @click="moveModel(i, 1)">↓</button>
            <button type="button" class="kid-btn" style="padding: 0.15rem 0.35rem; background: var(--color-danger); color: #fff" @click="chain.splice(i, 1)">×</button>
          </div>
        </div>

        <div style="display: flex; gap: 0.4rem; margin-top: 0.6rem">
          <input v-model="newModel" class="kid-input" placeholder="provider/model — press Enter to add" @keydown.enter.prevent="addModel" />
          <button type="button" class="kid-btn" @click="addModel">Add</button>
        </div>

        <div style="display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 1.25rem">
          <button type="button" class="kid-btn" :disabled="saving" @click="showForm = false">Cancel</button>
          <button type="submit" class="kid-btn kid-btn--primary" :disabled="saving || !!nameError">
            {{ saving ? "Saving…" : editingId ? "Save" : "Create" }}
          </button>
        </div>
      </form>
    </Modal>

    <ConfirmModal
      v-if="confirmDelete"
      title="Delete Combo"
      :message="`Delete combo '${confirmDelete.name}'?`"
      confirm-label="Delete"
      danger
      @close="confirmDelete = null"
      @confirm="remove(confirmDelete)"
    />
  </div>
</template>
