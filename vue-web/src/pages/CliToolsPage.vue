// CLI Tools: pick key + model, copy ready-to-paste env/config snippets.
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { api } from "@/lib/api";
import { toast } from "@/lib/state";
import Badge from "@/components/Badge.vue";

interface ApiKey { id: string; key: string; name: string; isActive?: boolean }
interface ToolDef {
  id: string;
  name: string;
  description: string;
  icon: string;
  snippet: (b: string, k: string, m: string) => string;
}

const keys = ref<ApiKey[]>([]);
const keyId = ref("");
const model = ref("openrouter/openai/gpt-4o-mini");

onMounted(async () => {
  try {
    const data = await api.get<{ keys: ApiKey[] }>("/api/keys");
    keys.value = data.keys ?? [];
    const active = keys.value.find((k) => k.isActive !== false);
    if (active) keyId.value = active.id;
  } catch { /* leave empty */ }
});

const selectedKey = computed(() => keys.value.find((k) => k.id === keyId.value)?.key ?? "<your-sk-key>");
const baseUrl = `${window.location.protocol}//${window.location.host}`;

const TOOLS: ToolDef[] = [
  {
    id: "claude",
    name: "Claude Code",
    description: "Anthropic CLI agent — point it at ORouter with env vars.",
    icon: "terminal",
    snippet: (b, k, m) => [
      "# Claude Code → ORouter",
      `export ANTHROPIC_BASE_URL="${b}"`,
      `export ANTHROPIC_AUTH_TOKEN="${k}"`,
      `export ANTHROPIC_MODEL="${m}"`,
      "claude",
    ].join("\n"),
  },
  {
    id: "codex",
    name: "Codex CLI",
    description: "OpenAI Codex CLI via OpenAI-compatible endpoint.",
    icon: "smart_toy",
    snippet: (b, k, m) => [
      "# ~/.codex/config.toml",
      `model = "${m}"`,
      'model_provider = "orouter"',
      "",
      "[model_providers.orouter]",
      'name = "ORouter"',
      `base_url = "${b}/v1"`,
      'env_key = "OROUTER_API_KEY"',
      "",
      `# then: export OROUTER_API_KEY="${k}" && codex`,
    ].join("\n"),
  },
  {
    id: "cursor",
    name: "Cursor / Windsurf",
    description: "Override OpenAI Base URL in Model settings.",
    icon: "edit_note",
    snippet: (b, k) => [
      "Cursor → Settings → Models → OpenAI API Key",
      `Base URL: ${b}/v1`,
      `API Key:  ${k}`,
      "(enable the models you want, then verify with a small chat)",
    ].join("\n"),
  },
  {
    id: "generic-openai",
    name: "Generic OpenAI SDK",
    description: "Any OpenAI-compatible client (curl, SDK, LangChain…).",
    icon: "code",
    snippet: (b, k, m) => [
      `curl ${b}/v1/chat/completions \\`,
      `  -H "Authorization: Bearer ${k}" \\`,
      '  -H "Content-Type: application/json" \\',
      `  -d '{"model":"${m}","messages":[{"role":"user","content":"hi"}]}'`,
    ].join("\n"),
  },
];

function copyText(text: string, label: string) {
  navigator.clipboard?.writeText(text).then(
    () => toast.success(`Copied ${label}`),
    () => toast.error("Failed to copy"),
  );
}
</script>

<template>
  <div class="fade-in flex flex-col gap-4" style="max-width: 1000px">
    <!-- Pickers -->
    <div class="kid-card" style="display: flex; flex-wrap: wrap; gap: 0.6rem; align-items: center; padding: 0.8rem 1rem">
      <label style="font-family: var(--font-body); font-weight: 700">API Key:</label>
      <select v-model="keyId" class="kid-input" style="width: auto; min-width: 220px">
        <option v-if="keys.length === 0" value="">(no keys — create one in Endpoint page)</option>
        <option v-for="k in keys" :key="k.id" :value="k.id">{{ k.name }} {{ k.isActive === false ? "(disabled)" : "" }}</option>
      </select>
      <label style="font-family: var(--font-body); font-weight: 700">Model:</label>
      <input v-model="model" class="kid-input" style="width: auto; min-width: 240px" placeholder="provider/model" />
      <Badge v-if="keyId" variant="success" size="sm" dot>ready</Badge>
    </div>

    <div style="display: grid; gap: 1rem; grid-template-columns: repeat(auto-fill, minmax(420px, 1fr))">
      <div v-for="t in TOOLS" :key="t.id" class="kid-card">
        <div style="display: flex; justify-content: space-between; align-items: flex-start; gap: 0.5rem">
          <div style="display: flex; gap: 0.6rem; align-items: center">
            <span class="material-symbols-outlined" style="font-size: 28px; color: var(--color-primary)">{{ t.icon }}</span>
            <div>
              <strong style="font-size: 1.15rem">{{ t.name }}</strong>
              <div style="font-family: var(--font-body); color: var(--color-text-muted); font-size: 0.9rem">{{ t.description }}</div>
            </div>
          </div>
          <button class="kid-btn kid-btn--primary" style="padding: 0.25rem 0.6rem" @click="copyText(t.snippet(baseUrl, selectedKey, model), `${t.name} config`)">
            <span class="material-symbols-outlined" style="font-size: 14px">content_copy</span> Copy
          </button>
        </div>
        <pre style="font-family: var(--font-body); font-size: 0.8rem; margin-top: 0.75rem; background: var(--color-bg-alt); border: 1px solid var(--nb-border); padding: 0.6rem 0.7rem; overflow-x: auto; white-space: pre-wrap; word-break: break-word">{{ t.snippet(baseUrl, selectedKey, model) }}</pre>
      </div>
    </div>
  </div>
</template>
