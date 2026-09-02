// Translator playground: send a test payload through the router and inspect
// the raw response. Native Rust serves OpenAI-format passthrough; for other
// formats the payload is forwarded to the Node engine in hybrid mode.
<script setup lang="ts">
import { onMounted, ref } from "vue";
import { toast } from "@/lib/state";

const model = ref("openrouter/openai/gpt-4o-mini");
const system = ref("You are a helpful assistant.");
const user = ref("Say hi in one short sentence.");
const stream = ref(false);
const sending = ref(false);
const response = ref("");
const status = ref("");

async function send() {
  if (sending.value) return;
  sending.value = true;
  response.value = "";
  status.value = "";
  const started = performance.now();
  try {
    const res = await fetch("/v1/chat/completions", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "include",
      body: JSON.stringify({
        model: model.value,
        messages: [
          { role: "system", content: system.value },
          { role: "user", content: user.value },
        ],
        stream: stream.value,
      }),
    });
    status.value = `HTTP ${res.status} · ${Math.round(performance.now() - started)}ms`;
    if (!stream.value) {
      const text = await res.text();
      try {
        response.value = JSON.stringify(JSON.parse(text), null, 2);
      } catch {
        response.value = text;
      }
    } else {
      const reader = res.body?.getReader();
      const dec = new TextDecoder();
      let acc = "";
      for (;;) {
        const { done, value } = await reader!.read();
        if (done) break;
        acc += dec.decode(value, { stream: true });
        response.value = acc;
      }
    }
  } catch (e) {
    status.value = "network error";
    response.value = String(e);
  } finally {
    sending.value = false;
  }
}

// ---- Translation capture dumps ----

interface DumpFile { name: string; size: number }
const dumps = ref<DumpFile[]>([]);
const dumpContent = ref("");

async function loadDumps() {
  try {
    const r = await fetch("/api/translator/dumps", { credentials: "include" });
    const data = (await r.json()) as { files?: DumpFile[] };
    dumps.value = data.files ?? [];
  } catch {
    toast.error("Failed to list dumps");
  }
}

async function viewDump(name: string) {
  try {
    dumpContent.value = await fetch(`/api/translator/dumps/${encodeURIComponent(name)}`, {
      credentials: "include",
    }).then((r) => r.text());
  } catch {
    toast.error("Failed to load dump");
  }
}

onMounted(() => {
  loadDumps();
});
</script>

<template>
  <div class="fade-in flex flex-col gap-4" style="max-width: 1000px">
    <div class="kid-card" style="display: grid; gap: 0.6rem">
      <div>
        <label style="font-family: var(--font-body); font-size: 0.9rem">Model (provider/model)</label>
        <input v-model="model" class="kid-input" placeholder="openrouter/openai/gpt-4o-mini" />
      </div>
      <div>
        <label style="font-family: var(--font-body); font-size: 0.9rem">System</label>
        <input v-model="system" class="kid-input" />
      </div>
      <div>
        <label style="font-family: var(--font-body); font-size: 0.9rem">User message</label>
        <textarea v-model="user" class="kid-input" rows="2" />
      </div>
      <div style="display: flex; gap: 1rem; align-items: center">
        <label style="display: flex; gap: 0.4rem; align-items: center; font-family: var(--font-body)">
          <input v-model="stream" type="checkbox" /> stream (SSE)
        </label>
        <button class="kid-btn kid-btn--primary" style="padding: 0.35rem 0.9rem" :disabled="sending || !model.trim()" @click="send">
          <span class="material-symbols-outlined" style="font-size: 16px">send</span>
          {{ sending ? "Sending…" : "Send test payload" }}
        </button>
        <span v-if="status" style="font-family: var(--font-body); color: var(--color-text-muted)">{{ status }}</span>
      </div>
    </div>

    <div v-if="response" class="kid-card" style="padding: 0">
      <div style="padding: 0.5rem 0.8rem; font-family: var(--font-body); font-size: 0.85rem; color: var(--color-text-muted); border-bottom: 2px solid var(--color-surface-3)">
        Raw response
      </div>
      <pre style="font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.78rem; padding: 0.8rem; overflow-x: auto; white-space: pre-wrap; word-break: break-word; margin: 0">{{ response }}</pre>
    </div>

    <!-- Translation capture dumps (Node hybrid runs write these) -->
    <div class="kid-card" style="padding: 0">
      <div style="display: flex; justify-content: space-between; align-items: center; padding: 0.5rem 0.8rem; border-bottom: 2px solid var(--color-surface-3)">
        <span style="font-family: var(--font-body); font-size: 0.85rem; color: var(--color-text-muted)">
          Translation pipeline dumps (logs/translator — written by hybrid Node runs)
        </span>
        <button class="kid-btn" style="padding: 0.2rem 0.55rem; font-size: 0.8rem" @click="loadDumps">
          <span class="material-symbols-outlined" style="font-size: 14px">refresh</span>
        </button>
      </div>
      <div v-if="dumps.length === 0" style="padding: 1rem; font-family: var(--font-body); font-size: 0.9rem; color: var(--color-text-muted)">
        No captures yet. Run a chat through hybrid mode with ENABLE_TRANSLATOR=true, then refresh.
      </div>
      <div v-else style="display: flex; flex-wrap: wrap; gap: 0.4rem; padding: 0.7rem 0.8rem">
        <button v-for="d in dumps" :key="d.name" class="kid-btn" style="padding: 0.2rem 0.55rem; font-size: 0.8rem" @click="viewDump(d.name)">
          {{ d.name }} <span style="color: var(--color-text-muted)">({{ d.size }}b)</span>
        </button>
      </div>
      <pre v-if="dumpContent" style="font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.75rem; padding: 0.8rem; overflow-x: auto; white-space: pre-wrap; word-break: break-word; margin: 0; border-top: 2px solid var(--color-surface-3); max-height: 40vh; overflow-y: auto">{{ dumpContent }}</pre>
    </div>

    <div class="kid-card" style="background: color-mix(in srgb, var(--color-info) 8%, var(--color-surface))">
      <div style="font-family: var(--font-body); font-size: 0.9rem; color: var(--color-text-muted)">
        ℹ️ Native Rust serves OpenAI-format passthrough. Requests for Claude/Gemini-format
        providers need the Node translator — start hybrid mode (NODE_UPSTREAM) for those.
      </div>
    </div>
  </div>
</template>
