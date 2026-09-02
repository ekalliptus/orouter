// CLI Tools — Node CLIToolsPageClient layout: grid of tool summary cards
// (icon, name, status pill, chevron) opening a detail modal with the
// copy-paste snippet for that tool.
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { api } from "@/lib/api";
import { toast } from "@/lib/state";
import Badge from "@/components/Badge.vue";
import Modal from "@/components/Modal.vue";

interface ApiKey { id: string; key: string; name: string; isActive?: boolean }
interface ToolDef {
  id: string;
  name: string;
  description: string;
  icon: string;
  color: string;
  snippet: (b: string, k: string, m: string) => string;
}

const keys = ref<ApiKey[]>([]);
const keyId = ref("");
const model = ref("openrouter/openai/gpt-4o-mini");
const activeTool = ref<ToolDef | null>(null);

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
  { id: "claude", name: "Claude Code", description: "Anthropic CLI agent", icon: "terminal", color: "#d97706", snippet: (b, k, m) => `export ANTHROPIC_BASE_URL="${b}"\nexport ANTHROPIC_AUTH_TOKEN="${k}"\nexport ANTHROPIC_MODEL="${m}"\nclaude` },
  { id: "codex", name: "OpenAI Codex CLI / App", description: "OpenAI coding agent", icon: "smart_toy", color: "#0ea5e9", snippet: (b, k, m) => `# ~/.codex/config.toml\nmodel = "${m}"\nmodel_provider = "orouter"\n\n[model_providers.orouter]\nname = "ORouter"\nbase_url = "${b}/v1"\nenv_key = "OROUTER_API_KEY"\n\n# then: export OROUTER_API_KEY="${k}" && codex` },
  { id: "cursor", name: "Cursor", description: "AI IDE", icon: "edit_note", color: "#6366f1", snippet: (b, k) => `Cursor → Settings → Models → OpenAI API Key\nBase URL: ${b}/v1\nAPI Key:  ${k}` },
  { id: "windsurf", name: "Windsurf", description: "AI IDE", icon: "surfing", color: "#22c55e", snippet: (b, k) => `Windsurf → Settings → Models → OpenAI Base URL\nBase URL: ${b}/v1\nAPI Key:  ${k}` },
  { id: "cline", name: "Cline", description: "VS Code agent extension", icon: "integration_instructions", color: "#8b5cf6", snippet: (b, k, m) => `Cline → Settings → API Provider: OpenAI Compatible\nBase URL: ${b}/v1\nAPI Key:  ${k}\nModel:    ${m}` },
  { id: "kilo", name: "Kilo Code", description: "VS Code agent extension", icon: "code_blocks", color: "#f59e0b", snippet: (b, k, m) => `Kilo → Settings → OpenAI Compatible\nBase URL: ${b}/v1\nAPI Key:  ${k}\nModel:    ${m}` },
  { id: "opencode", name: "OpenCode", description: "Terminal coding agent", icon: "keyboard", color: "#14b8a6", snippet: (b, k) => `opencode auth login\n→ Provider: OpenAI-compatible\nBase URL: ${b}/v1\nAPI Key:  ${k}` },
  { id: "gemini-cli", name: "Gemini CLI", description: "Google CLI agent", icon: "diamond", color: "#3b82f6", snippet: (b, k, m) => `GEMINI_API_KEY="${k}"\nGOOGLE_GENAI_BASE_URL="${b}/v1"\nMODEL="${m}" gemini` },
  { id: "continue", name: "Continue", description: "VS Code assistant", icon: "extension", color: "#ec4899", snippet: (b, k, m) => `config.yaml:\nmodels:\n  - name: ORouter\n    provider: openai\n    apiBase: ${b}/v1\n    apiKey: ${k}\n    model: ${m}` },
  { id: "generic", name: "Generic OpenAI SDK", description: "Any OpenAI-compatible client", icon: "code", color: "#64748b", snippet: (b, k, m) => `curl ${b}/v1/chat/completions \\\n  -H "Authorization: Bearer ${k}" \\\n  -H "Content-Type: application/json" \\\n  -d '{"model":"${m}","messages":[{"role":"user","content":"hi"}]}'` },
];

function copyText(text: string, label: string) {
  navigator.clipboard?.writeText(text).then(
    () => toast.success(`Copied ${label}`),
    () => toast.error("Failed to copy"),
  );
}
</script>

<template>
  <div class="fade-in flex flex-col gap-4" style="max-width: 1100px">
    <!-- Shared pickers -->
    <div class="kid-card" style="display: flex; flex-wrap: wrap; gap: 0.6rem; align-items: center; padding: 0.8rem 1rem">
      <label style="font-family: var(--font-body); font-weight: 700">API Key:</label>
      <select v-model="keyId" class="kid-input" style="width: auto; min-width: 200px">
        <option v-if="keys.length === 0" value="">(no keys — create one in Endpoint page)</option>
        <option v-for="k in keys" :key="k.id" :value="k.id">{{ k.name }} {{ k.isActive === false ? "(paused)" : "" }}</option>
      </select>
      <label style="font-family: var(--font-body); font-weight: 700">Model:</label>
      <input v-model="model" class="kid-input" style="width: auto; min-width: 230px" placeholder="provider/model" />
    </div>

    <!-- Tool grid -->
    <div style="display: grid; gap: 1rem; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr))">
      <button
        v-for="t in TOOLS"
        :key="t.id"
        class="kid-card"
        style="display: flex; align-items: center; gap: 0.75rem; text-align: left; cursor: pointer; color: inherit"
        @click="activeTool = t"
      >
        <span
          class="material-symbols-outlined"
          style="width: 32px; height: 32px; display: flex; align-items: center; justify-content: center; font-size: 20px"
          :style="{ color: t.color, background: `color-mix(in srgb, ${t.color} 15%, transparent)` }"
        >{{ t.icon }}</span>
        <div style="flex: 1; min-width: 0">
          <strong style="display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap">{{ t.name }}</strong>
          <span style="font-family: var(--font-body); font-size: 0.82rem; color: var(--color-text-muted)">{{ t.description }}</span>
        </div>
        <Badge variant="neutral" size="sm">manual</Badge>
        <span class="material-symbols-outlined" style="font-size: 18px; color: var(--color-text-muted)">chevron_right</span>
      </button>
    </div>

    <!-- Detail modal -->
    <Modal v-if="activeTool" width="560px" @close="activeTool = null">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.75rem">
        <h3 style="font-size: 1.25rem; margin: 0">{{ activeTool.name }}</h3>
        <Badge variant="warning" size="sm">copy-paste setup</Badge>
      </div>
      <p style="font-family: var(--font-body); color: var(--color-text-muted); margin: 0 0 0.75rem">
        Point {{ activeTool.name }} at this ORouter instance with the configuration below.
      </p>
      <pre style="font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.78rem; background: var(--color-bg-alt); border: 1px solid var(--nb-border); padding: 0.7rem; overflow-x: auto; white-space: pre-wrap; word-break: break-word">{{ activeTool.snippet(baseUrl, selectedKey, model) }}</pre>
      <div style="display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 1rem">
        <button class="kid-btn" @click="activeTool = null">Close</button>
        <button class="kid-btn kid-btn--primary" @click="copyText(activeTool.snippet(baseUrl, selectedKey, model), `${activeTool.name} config`)">
          <span class="material-symbols-outlined" style="font-size: 15px">content_copy</span> Copy config
        </button>
      </div>
    </Modal>
  </div>
</template>
