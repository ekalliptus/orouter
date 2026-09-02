// Token Saver: RTK / caveman / ponytail / headroom / pxpipe toggles.
// Settings persist natively; the compression engines themselves run in the
// Node engine (hybrid mode) — the page is honest about that.
<script setup lang="ts">
import { onMounted, ref } from "vue";
import { api } from "@/lib/api";
import { toast } from "@/lib/state";
import Toggle from "@/components/Toggle.vue";

const settings = ref<Record<string, unknown> | null>(null);
const headroomUrl = ref("");

function bool(key: string): boolean {
  return !!settings.value?.[key];
}
function str(key: string): string {
  return String(settings.value?.[key] ?? "");
}

onMounted(async () => {
  try {
    settings.value = await api.get<Record<string, unknown>>("/api/settings");
    headroomUrl.value = String(settings.value?.headroomUrl ?? "");
  } catch {
    toast.error("Failed to load settings");
  }
});

async function patch(patchObj: Record<string, unknown>, okMsg: string) {
  try {
    settings.value = { ...(settings.value ?? {}), ...patchObj };
    await api.patch("/api/settings", patchObj);
    toast.success(okMsg);
  } catch {
    toast.error("Failed to save");
  }
}
</script>

<template>
  <div class="fade-in flex flex-col gap-4" style="max-width: 720px">
    <div v-if="!settings" class="kid-card"><p style="font-family: var(--font-body)">Loading…</p></div>

    <template v-if="settings">
      <div class="kid-card">
        <div style="font-weight: 700; font-size: 1.15rem; margin-bottom: 0.5rem">RTK Compression</div>
        <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem">
          <div style="font-family: var(--font-body); color: var(--color-text-muted); font-size: 0.95rem">
            Compress tool_result content in-flight to cut prompt tokens.
          </div>
          <Toggle :checked="bool('rtkEnabled')" @change="(v) => patch({ rtkEnabled: v }, 'rtkEnabled updated')" />
        </div>
      </div>

      <div class="kid-card">
        <div style="font-weight: 700; font-size: 1.15rem; margin-bottom: 0.75rem">Caveman Mode</div>
        <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem">
          <div style="font-family: var(--font-body); color: var(--color-text-muted); font-size: 0.95rem">Rewrite verbose content into terse caveman speak.</div>
          <Toggle :checked="bool('cavemanEnabled')" @change="(v) => patch({ cavemanEnabled: v }, 'cavemanEnabled updated')" />
        </div>
        <div v-if="bool('cavemanEnabled')" style="margin-top: 0.75rem; max-width: 220px">
          <label style="font-family: var(--font-body); font-size: 0.9rem">Level</label>
          <select class="kid-input" :value="str('cavemanLevel') || 'full'" @change="patch({ cavemanLevel: ($event.target as HTMLSelectElement).value }, 'cavemanLevel updated')">
            <option value="lite">lite</option>
            <option value="full">full</option>
          </select>
        </div>
      </div>

      <div class="kid-card">
        <div style="font-weight: 700; font-size: 1.15rem; margin-bottom: 0.75rem">Ponytail Mode</div>
        <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem">
          <div style="font-family: var(--font-body); color: var(--color-text-muted); font-size: 0.95rem">Extra summarization pass for long tool outputs.</div>
          <Toggle :checked="bool('ponytailEnabled')" @change="(v) => patch({ ponytailEnabled: v }, 'ponytailEnabled updated')" />
        </div>
      </div>

      <div class="kid-card">
        <div style="font-weight: 700; font-size: 1.15rem; margin-bottom: 0.75rem">Headroom</div>
        <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem">
          <div style="font-family: var(--font-body); color: var(--color-text-muted); font-size: 0.95rem">
            Route through a local headroom proxy sidecar for context compression.
          </div>
          <Toggle :checked="bool('headroomEnabled')" @change="(v) => patch({ headroomEnabled: v }, 'headroomEnabled updated')" />
        </div>
        <div v-if="bool('headroomEnabled')" style="margin-top: 0.75rem; display: grid; gap: 0.6rem">
          <input v-model="headroomUrl" class="kid-input" placeholder="http://localhost:8787" />
          <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem">
            <label style="display: flex; gap: 0.5rem; align-items: center; font-family: var(--font-body); font-size: 0.92rem">
              <input type="checkbox" :checked="bool('headroomCompressUserMessages')" @change="patch({ headroomCompressUserMessages: ($event.target as HTMLInputElement).checked }, 'headroomCompressUserMessages updated')" />
              Compress user messages too
            </label>
            <button class="kid-btn" @click="patch({ headroomUrl: headroomUrl.trim() }, 'Headroom URL saved')">Save URL</button>
          </div>
        </div>
      </div>

      <div class="kid-card">
        <div style="font-weight: 700; font-size: 1.15rem; margin-bottom: 0.75rem">PXPIPE</div>
        <div style="display: flex; justify-content: space-between; align-items: center; gap: 1rem">
          <div style="font-family: var(--font-body); color: var(--color-text-muted); font-size: 0.95rem">
            Big-prompt preprocessing pipeline (kicks in above pxpipeMinChars).
          </div>
          <Toggle :checked="bool('pxpipeEnabled')" @change="(v) => patch({ pxpipeEnabled: v }, 'pxpipeEnabled updated')" />
        </div>
        <div v-if="bool('pxpipeEnabled')" style="margin-top: 0.75rem; display: grid; gap: 0.6rem; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr))">
          <div>
            <label style="font-family: var(--font-body); font-size: 0.9rem">Min chars</label>
            <input class="kid-input" type="number" :value="str('pxpipeMinChars')" @change="patch({ pxpipeMinChars: parseInt(($event.target as HTMLInputElement).value, 10) || 0 }, 'pxpipeMinChars updated')" />
          </div>
          <div>
            <label style="font-family: var(--font-body); font-size: 0.9rem">Timeout (ms)</label>
            <input class="kid-input" type="number" :value="str('pxpipeTimeoutMs')" @change="patch({ pxpipeTimeoutMs: parseInt(($event.target as HTMLInputElement).value, 10) || 0 }, 'pxpipeTimeoutMs updated')" />
          </div>
        </div>
      </div>

      <div class="kid-card" style="background: color-mix(in srgb, var(--color-info) 8%, var(--color-surface))">
        <div style="font-family: var(--font-body); font-size: 0.9rem; color: var(--color-text-muted)">
          ℹ️ These toggles persist natively. The compression engines themselves run in the Node
          engine — start hybrid mode (NODE_UPSTREAM) for them to take effect on live traffic.
        </div>
      </div>
    </template>
  </div>
</template>
